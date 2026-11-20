//! Liens IFC dans la scène du projet natif : le fichier lié est lu une fois (cache par
//! chemin, date et taille), ses éléments sont placés dans le repère du projet et
//! rattachés à un niveau du projet, ce qui les montre en 3D, en plan (vue du niveau) et en
//! coupe comme les éléments natifs. Ils n'ont pas d'identifiant natif : aucune commande
//! d'édition ne les atteint.
use formats::ifc::geom::{self, GeomStats, Part, ProductGeom, Rgba};
use glam::DVec3;
use ifc_base::semantique::IfcModel;
use ifc_base::step::{Entity, StepFile, Value};
use modele::identifiants::id_natif;
use modele::liens::LienIfc;
use modele::Projet;
use noyau::aretes::Aretes;
use noyau::Id;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

/// Classes sans objet physique à montrer (volumes de pièces, réservations, annotations).
const ECARTEES: &[&str] = &["IFCSPACE", "IFCOPENINGELEMENT", "IFCOPENINGSTANDARDCASE", "IFCVIRTUALELEMENT", "IFCANNOTATION"];

/// Tolérance (m) au-dessus de la base d'un élément pour le rattacher à un niveau : une
/// dalle dont le dessus est au niveau, une poutre sous le plancher du dessus restent au
/// niveau qu'elles bordent.
const TOLERANCE_NIVEAU: f64 = 0.3;

/// Élément d'un fichier lié, dans le repère du fichier (mètres).
pub struct ElementLie {
    pub step: Id,
    pub classe: Arc<str>,
    pub nom: String,
    pub guid: String,
    pub parts: Vec<(Rgba, Vec<[DVec3; 3]>)>,
    pub aretes: Aretes,
    pub min_z: f64,
}

/// Contenu géométrique d'un fichier IFC lié.
pub struct FichierLie {
    pub elements: Vec<ElementLie>,
    /// Entités dont la géométrie n'est pas prise en charge (nombre par classe).
    pub ignores: Vec<(String, usize)>,
    /// Boîte englobante (m, repère du fichier).
    pub min: DVec3,
    pub max: DVec3,
}

type Cache = Mutex<HashMap<PathBuf, (SystemTime, u64, Arc<FichierLie>)>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Fichier lié, lu ou repris du cache s'il n'a pas changé sur le disque. Le premier
/// appel peut être long (géométrie de tout le fichier) : l'interface le fait en
/// arrière-plan avant d'ajouter le lien.
pub fn charger(chemin: &Path) -> Result<Arc<FichierLie>, String> {
    let meta = std::fs::metadata(chemin).map_err(|e| format!("Lien {} : {e}", chemin.display()))?;
    let (date, taille) = (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len());
    if let Some((d, t, f)) = cache().lock().unwrap_or_else(|e| e.into_inner()).get(chemin)
        && *d == date && *t == taille
    {
        return Ok(f.clone());
    }
    let fichier = Arc::new(lire(chemin)?);
    cache().lock().unwrap_or_else(|e| e.into_inner()).insert(chemin.to_path_buf(), (date, taille, fichier.clone()));
    Ok(fichier)
}

fn lire(chemin: &Path) -> Result<FichierLie, String> {
    let data = std::fs::read(chemin).map_err(|e| format!("Lien {} : {e}", chemin.display()))?;
    if data.starts_with(b"PK") {
        return Err(format!("Lien {} : IFC-ZIP non pris en charge, décompresser d'abord.", chemin.display()));
    }
    let model = IfcModel::new(StepFile::parse(&data).map_err(|e| format!("Lien {} : {e}", chemin.display()))?);
    drop(data);
    let stats = GeomStats::default();
    let pool = rayon::ThreadPoolBuilder::new().stack_size(64 << 20).build().map_err(|e| e.to_string())?;
    let mut geoms: Vec<ProductGeom> = pool.install(|| {
        model.products.par_iter()
            .filter(|&&id| model.get(id).is_some_and(|e| !ECARTEES.contains(&&*e.ty)))
            .filter_map(|&id| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| geom::product_geometry(&model, &stats, id))).ok().flatten())
            .collect()
    });
    geoms.sort_by_key(|g| g.id);
    let elements: Vec<ElementLie> = geoms.into_par_iter().filter_map(|g| {
        let e = model.get(g.id)?;
        let parts: Vec<(Rgba, Vec<[DVec3; 3]>)> = g.parts.into_iter().filter(|p| !p.tris.is_empty()).map(|p| (p.color, p.tris)).collect();
        let tous: Vec<[DVec3; 3]> = parts.iter().flat_map(|p| p.1.iter().copied()).collect();
        let min_z = tous.iter().flatten().map(|v| v.z).fold(f64::INFINITY, f64::min);
        (!tous.is_empty()).then(|| ElementLie {
            step: g.id,
            classe: e.ty.clone(),
            nom: model.name(g.id),
            guid: e.arg(0).as_str().unwrap_or_default().to_string(),
            aretes: noyau::aretes::aretes(&tous),
            parts,
            min_z,
        })
    }).collect();
    let (mut min, mut max) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    for v in elements.iter().flat_map(|e| e.parts.iter().flat_map(|p| p.1.iter().flatten())) {
        min = min.min(*v);
        max = max.max(*v);
    }
    let mut ignores: Vec<(String, usize)> = stats.unsupported.into_inner().unwrap_or_else(|e| e.into_inner()).into_iter().collect();
    ignores.sort_by_key(|a| std::cmp::Reverse(a.1));
    Ok(FichierLie { elements, ignores, min, max })
}

/// Niveau du projet auquel rattacher un élément dont la base est à `z` (m) : le plus
/// haut sous `z + TOLERANCE_NIVEAU`, sinon le plus bas.
fn niveau_de(niveaux: &[(f64, Id)], z: f64) -> Option<Id> {
    niveaux.iter().rev().find(|(a, _)| *a <= z + TOLERANCE_NIVEAU).or(niveaux.first()).map(|n| n.1)
}

/// Éléments liés à ajouter à la scène, avec leurs arêtes. Un lien illisible n'empêche
/// pas d'ouvrir le projet : il est compté dans les entités ignorées.
pub(crate) struct Ajouts {
    pub geoms: Vec<ProductGeom>,
    pub aretes: HashMap<Id, Aretes>,
    pub ignores: Vec<(String, usize)>,
}

/// Inscrit les éléments des liens visibles dans `model` (entité, niveau parent) et rend
/// leur géométrie placée dans le repère du projet.
pub(crate) fn ajouter(projet: &Projet, model: &mut IfcModel, racine: Id) -> Ajouts {
    let mut a = Ajouts { geoms: Vec::new(), aretes: HashMap::new(), ignores: Vec::new() };
    let mut niveaux: Vec<(f64, Id)> = projet.niveaux.iter().map(|n| (n.altitude * 0.001, id_natif("niveau", &n.id))).collect();
    niveaux.sort_by(|x, y| x.0.total_cmp(&y.0));
    for lien in projet.liens.iter().filter(|l| l.visible) {
        let fichier = match charger(Path::new(&lien.chemin)) {
            Ok(f) => f,
            Err(e) => { a.ignores.push((e, 1)); continue; }
        };
        a.ignores.extend(fichier.ignores.iter().map(|(c, n)| (format!("{} (lien {})", c, lien.nom), *n)));
        for e in &fichier.elements {
            placer(lien, e, &niveaux, racine, model, &mut a);
        }
    }
    a
}

fn placer(lien: &LienIfc, e: &ElementLie, niveaux: &[(f64, Id)], racine: Id, model: &mut IfcModel, a: &mut Ajouts) {
    let id = id_natif("lien", &format!("{}/{}", lien.id, e.step));
    let mut args = vec![Value::Null; 9];
    args[0] = Value::Str(format!("lien:{}:{}", lien.id, e.guid));
    args[2] = Value::Str(format!("{} · {}", lien.nom, e.nom));
    model.file.entities.insert(id, Entity { ty: e.classe.clone(), args });
    // base de l'élément après placement (la rotation garde les altitudes)
    let parent = niveau_de(niveaux, e.min_z + lien.decalage[2] * 0.001).unwrap_or(racine);
    model.parent.insert(id, parent);
    model.children.entry(parent).or_default().push(id);
    let t = |v: DVec3| lien.transformer(v);
    let parts = e.parts.iter().map(|(couleur, tris)| Part { color: *couleur, tris: tris.iter().map(|tri| tri.map(t)).collect() }).collect();
    a.geoms.push(ProductGeom { visibilite: Vec::new(), id, parts, lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
    a.aretes.insert(id, Aretes {
        vives: e.aretes.vives.iter().map(|s| s.map(t)).collect(),
        douces: e.aretes.douces.iter().map(|(s, n1, n2)| (s.map(t), lien.tourner(*n1), lien.tourner(*n2))).collect(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exemple() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4.ifc"))
    }

    #[test]
    fn niveau_le_plus_haut_sous_la_base() {
        let n = [(0.0, 1), (3.0, 2), (6.0, 3)];
        assert_eq!(niveau_de(&n, 0.0), Some(1));
        assert_eq!(niveau_de(&n, 2.8), Some(2), "dalle sous le niveau 2 : rattachée au niveau 2");
        assert_eq!(niveau_de(&n, 4.0), Some(2));
        assert_eq!(niveau_de(&n, -2.0), Some(1), "sous le plus bas : le plus bas");
        assert_eq!(niveau_de(&[], 1.0), None);
    }

    /// Lecture une fois, reprise du cache tant que le fichier ne change pas.
    #[test]
    fn fichier_lu_puis_repris_du_cache() {
        let f = charger(&exemple()).unwrap();
        assert!(!f.elements.is_empty());
        assert!(f.elements.iter().all(|e| !ECARTEES.contains(&&*e.classe)));
        assert!(Arc::ptr_eq(&f, &charger(&exemple()).unwrap()));
        assert!(charger(Path::new("/chemin/absent.ifc")).is_err());
    }
}
