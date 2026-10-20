//! Murs, niveaux et types de murs lus directement dans les partitions d'un RVT, et leur
//! traduction en scène du viewer (portage de `champs.murs`, `elements.niveaux`,
//! `elements.types_murs` et `geometrie.maillage` de `tools/rvt-research/`).
//!
//! Les objets courants sont ceux des ids de `Global/ElemTable`, une partition plus
//! récente remplaçant une plus ancienne. Comme les collecteurs de Revit, les éléments
//! d'un groupe non placé (`Element.m_unplacedOwnerId` ≠ −1) sont ignorés.

use super::champs::{self, Obj, Record, Val};
use super::geometrie::{self, FT, Maillage};
use super::objets::{elem_table, inflate, records, sections, unpad};
use super::schema::Schema;
use super::Cfb;
use crate::geom::{Part, ProductGeom, class_color};
use crate::model::IfcModel;
use crate::scene::{self, Scene};
use crate::step::{Entity, Id, IdMap, StepFile, Value, format_real};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

const WALL_USER_HEIGHT_PARAM: i64 = -1_001_105;
const WALL_BASE_OFFSET: i64 = -1_001_108;
const WALL_TOP_OFFSET: i64 = -1_001_109;

pub struct Niveau {
    pub id: i64,
    pub nom: String,
    /// Altitude en mm.
    pub altitude: f64,
}

pub struct TypeMur {
    pub nom: String,
    /// Somme des couches de la structure composée, en mm (absente pour les murs-rideaux
    /// et les murs empilés).
    pub epaisseur: Option<f64>,
}

pub struct Mur {
    pub id: i64,
    /// `SWall` (droit) ou `ArcWall` (courbe).
    pub classe: String,
    pub type_id: i64,
    pub niveau: i64,
    /// −1 : hauteur non contrainte.
    pub niveau_haut: i64,
    /// Hauteur en mm : celle que donne Revit, calculée depuis les niveaux si le mur est
    /// contraint en haut.
    pub hauteur: f64,
    pub decalage_bas: f64,
    pub decalage_haut: f64,
    /// Solide du `GElement` (section 0x67), en mm.
    pub geometrie: Result<Maillage, String>,
}

pub struct Maquette {
    pub niveaux: Vec<Niveau>,
    pub types: HashMap<i64, TypeMur>,
    pub murs: Vec<Mur>,
    /// Objets retenus mais illisibles (données d'add-ins…).
    pub illisibles: Vec<(i64, String)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Sorte {
    Mur,
    Niveau,
    TypeMur,
}

/// Lit le schéma, la table des éléments et les partitions, puis les murs et leur
/// géométrie.
pub fn lire(path: &Path, progress: &dyn Fn(String)) -> Result<Maquette, String> {
    let mut cfb = Cfb::open(path)?;
    let raw = cfb.read("Formats/Latest")?.ok_or("flux Formats/Latest absent")?;
    let sch = Schema::parse(&inflate(&unpad(&raw)).map_err(|e| format!("Formats/Latest : {e}"))?)?;
    let ids8 = sch.ids8();
    let table = elem_table(&cfb.read("Global/ElemTable")?.ok_or("flux Global/ElemTable absent")?)?;

    let sortes: HashMap<u16, Sorte> = sch
        .classes
        .iter()
        .filter_map(|c| {
            let s = if c.name == "SWall" || c.name == "ArcWall" {
                Sorte::Mur
            } else if sch.inherits(c.id, "Level") {
                Sorte::Niveau
            } else if sch.inherits(c.id, "WallType") {
                Sorte::TypeMur
            } else {
                return None;
            };
            Some((c.id, s))
        })
        .collect();

    let mut parts: Vec<(u64, String)> = cfb
        .entries()
        .into_iter()
        .filter(|(p, dir, _)| !dir && p.starts_with("/Partitions/"))
        .filter_map(|(p, _, _)| Some((p.rsplit('/').next()?.parse().ok()?, p)))
        .collect();
    parts.sort();

    let mut objets: HashMap<i64, (Sorte, Vec<u8>)> = HashMap::new();
    let mut complements: HashMap<i64, Vec<u8>> = HashMap::new();
    for (k, (_, p)) in parts.iter().enumerate() {
        progress(format!("Partition {} / {} ({p})…", k + 1, parts.len()));
        let raw = cfb.read(p)?.ok_or_else(|| format!("flux {p} illisible"))?;
        let sec = sections(&raw).map_err(|e| format!("{p} : {e}"))?;
        let vide = Vec::new();
        for (eid, body) in records(sec.get(&0x66).unwrap_or(&vide), true, ids8).map_err(|e| format!("{p}, section 0x66 : {e}"))? {
            if let Some(&s) = champs::class_id(body).and_then(|c| sortes.get(&c)).filter(|_| table.contains(&eid)) {
                objets.insert(eid, (s, body.to_vec()));
            }
        }
        for (eid, body) in records(sec.get(&0x67).unwrap_or(&vide), true, ids8).map_err(|e| format!("{p}, section 0x67 : {e}"))? {
            if objets.get(&eid).is_some_and(|o| o.0 == Sorte::Mur) {
                complements.insert(eid, body.to_vec());
            }
        }
    }

    let mut illisibles = Vec::new();
    let mut lus: Vec<(i64, Sorte, Record)> = Vec::new();
    let mut ids: Vec<&i64> = objets.keys().collect();
    ids.sort();
    for &eid in ids {
        let (s, body) = &objets[&eid];
        match champs::read(&sch, body) {
            Ok(r) if place(&r.root) => lus.push((eid, *s, r)),
            Ok(_) => {}
            Err(e) => illisibles.push((eid, e)),
        }
    }

    let mut niveaux: Vec<Niveau> = lus
        .iter()
        .filter(|(_, s, _)| *s == Sorte::Niveau)
        .filter_map(|(eid, _, r)| {
            let z = r.field_body(&r.root, "m_pSurface")?.get("m_origin")?.reals()?.get(2).copied()?;
            Some(Niveau { id: *eid, nom: r.root.get("m_text")?.str()?.to_string(), altitude: z * FT })
        })
        .collect();
    niveaux.sort_by(|a, b| a.altitude.total_cmp(&b.altitude).then(a.id.cmp(&b.id)));
    let altitude: HashMap<i64, f64> = niveaux.iter().map(|n| (n.id, n.altitude)).collect();

    let types = lus
        .iter()
        .filter(|(_, s, _)| *s == Sorte::TypeMur)
        .filter_map(|(eid, _, r)| {
            let nom = r.field_body(&r.root, "m_symbolInfo")?.get("m_name")?.str()?.to_string();
            let epaisseur = r.field_body(&r.root, "m_pCompoundStructure").map(|cs| {
                let couches = cs.get("m_layers").map(Val::list).unwrap_or_default();
                couches.iter().filter_map(|c| c.obj()?.get("m_layerWidth")?.real()).sum::<f64>() * FT
            });
            Some((*eid, TypeMur { nom, epaisseur }))
        })
        .collect();

    let murs_lus: Vec<&(i64, Sorte, Record)> = lus.iter().filter(|(_, s, _)| *s == Sorte::Mur).collect();
    progress(format!("Géométrie de {} murs…", murs_lus.len()));
    let murs = murs_lus
        .par_iter()
        .map(|(eid, _, r)| {
            let mut m = mur(*eid, r, &altitude);
            m.geometrie = match complements.get(eid) {
                None => Err(format!("mur {eid} sans enregistrement 0x67")),
                Some(b) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| champs::read(&sch, b).and_then(|rec| geometrie::maillage(&rec, false))))
                    .unwrap_or_else(|_| Err("erreur interne".into()))
                    .and_then(|g| if g.faces.is_empty() { Err("aucun solide fermé dans le GElement".into()) } else { Ok(g) }),
            };
            m
        })
        .collect();
    Ok(Maquette { niveaux, types, murs, illisibles })
}

/// Élément placé : hors d'un groupe non placé.
fn place(o: &Obj) -> bool {
    o.get("m_unplacedOwnerId").and_then(Val::element_id).is_none_or(|g| g == -1)
}

fn id_of(o: &Obj, name: &str) -> i64 {
    o.get(name).and_then(Val::element_id).unwrap_or(-1)
}

fn mur(id: i64, r: &Record, altitude: &HashMap<i64, f64>) -> Mur {
    let o = &r.root;
    let mut params = HashMap::new();
    if let Some(pd) = r.field_body(o, "m_pParamValueSetDouble") {
        for x in pd.get("m_paramSet").map(Val::list).unwrap_or_default() {
            if let Some(x) = x.obj() {
                if let (Some(k), Some(v)) = (x.get("m_paramId").and_then(Val::element_id), x.get("m_value").and_then(Val::real)) {
                    params.insert(k, v);
                }
            }
        }
    }
    let p = |k| params.get(&k).copied().unwrap_or(0.0) * FT;
    let mut m = Mur {
        id,
        classe: o.class_name().to_string(),
        type_id: id_of(o, "m_WallAttributesId"),
        niveau: id_of(o, "m_assocLevelId"),
        niveau_haut: id_of(o, "m_upToLevelId"),
        hauteur: p(WALL_USER_HEIGHT_PARAM),
        decalage_bas: p(WALL_BASE_OFFSET),
        decalage_haut: p(WALL_TOP_OFFSET),
        geometrie: Err(String::new()),
    };
    if let (Some(h), Some(b)) = (altitude.get(&m.niveau_haut), altitude.get(&m.niveau)) {
        m.hauteur = h + m.decalage_haut - b - m.decalage_bas;
    }
    m
}

// ------------------------------------------------------------------ Scène

/// GlobalId synthétique de 22 caractères (les UniqueId de Revit ne sont pas décodés).
fn gid(prefix: &str, id: i64) -> Value {
    Value::Str(format!("{prefix}{:0>width$}", id, width = 22 - prefix.len()))
}

/// Ids distincts des matériaux de rendu des faces (`GFace.m_renderStyleId`).
fn materiaux(g: &Maillage) -> String {
    let mut ids: Vec<i64> = g.faces.iter().map(|f| f.materiau).filter(|&m| m != -1).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(", ")
}

fn mm(v: f64) -> Value {
    Value::Str(format!("{} mm", format_real((v * 10.0).round() / 10.0)))
}

struct Builder {
    entities: IdMap<Entity>,
}

impl Builder {
    fn add(&mut self, ty: &str, args: Vec<Value>) -> Id {
        let id = self.entities.len() as Id + 1;
        self.entities.insert(id, Entity { ty: Arc::from(ty), args });
        id
    }

    fn pset(&mut self, owner: i64, name: &str, props: Vec<(&str, Value)>) -> Id {
        let refs = props.into_iter().map(|(n, v)| Value::Ref(self.add("IFCPROPERTYSINGLEVALUE", vec![Value::Str(n.into()), Value::Null, v]))).collect();
        self.add("IFCPROPERTYSET", vec![gid("RVTPSET", owner), Value::Null, Value::Str(name.into()), Value::Null, Value::List(refs)])
    }
}

/// Lit les murs du fichier et construit la scène du viewer. Erreur si aucun mur n'a
/// pu être maillé.
pub fn load_scene(path: &Path, titre: &str, format: &str, progress: &dyn Fn(String)) -> Result<Scene, String> {
    let t0 = std::time::Instant::now();
    let m = lire(path, progress)?;
    let ko = m.murs.iter().filter(|w| w.geometrie.is_err()).count();
    if ko == m.murs.len() {
        return Err(match m.murs.len() {
            0 => "aucun mur dans le fichier".into(),
            n => format!("aucun des {n} murs n'a pu être maillé"),
        });
    }
    progress("Assemblage des tampons…".into());
    let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut s = scene(&m, titre, file_name, format);
    if ko > 0 {
        s.unsupported.push(("Murs Revit non maillés".into(), ko));
    }
    if !m.illisibles.is_empty() {
        s.unsupported.push(("Objets Revit illisibles".into(), m.illisibles.len()));
    }
    s.load_ms = t0.elapsed().as_millis();
    Ok(s)
}

/// Scène du viewer : un modèle IFC synthétique (projet, niveaux, murs, types,
/// propriétés) pour l'arborescence et les propriétés, et les maillages des murs.
pub fn scene(m: &Maquette, titre: &str, file_name: String, format: &str) -> Scene {
    let mut b = Builder { entities: IdMap::default() };
    let n = Value::Null;
    let projet = b.add("IFCPROJECT", vec![gid("RVTPROJET", 0), n.clone(), Value::Str(titre.into())]);
    let nom_niveau: HashMap<i64, &str> = m.niveaux.iter().map(|l| (l.id, l.nom.as_str())).collect();
    let mut etage: HashMap<i64, Id> = HashMap::new();
    for l in &m.niveaux {
        let args = vec![gid("RVTNIV", l.id), n.clone(), Value::Str(l.nom.clone()), n.clone(), n.clone(), n.clone(), n.clone(), n.clone(), Value::Enum("ELEMENT".into()), Value::Real(l.altitude / 1000.0)];
        etage.insert(l.id, b.add("IFCBUILDINGSTOREY", args));
    }
    let storeys: Vec<Value> = m.niveaux.iter().map(|l| Value::Ref(etage[&l.id])).collect();
    b.add("IFCRELAGGREGATES", vec![gid("RVTAGG", 0), n.clone(), n.clone(), n.clone(), Value::Ref(projet), Value::List(storeys)]);
    let shape = b.add("IFCPRODUCTDEFINITIONSHAPE", vec![n.clone(), n.clone(), Value::List(vec![])]);

    let mut types: HashMap<i64, Id> = HashMap::new();
    let mut contenus: HashMap<Id, Vec<Value>> = HashMap::new();
    let mut geoms = Vec::new();
    let color = class_color("IFCWALL");
    for mur in &m.murs {
        let Ok(g) = &mur.geometrie else { continue };
        let ty = m.types.get(&mur.type_id);
        let nom_type = ty.map(|t| t.nom.clone()).unwrap_or_else(|| format!("Type {}", mur.type_id));
        let id = b.add(
            "IFCWALL",
            vec![gid("RVT", mur.id), n.clone(), Value::Str(format!("{nom_type} [{}]", mur.id)), n.clone(), Value::Str(nom_type.clone()), n.clone(), Value::Ref(shape), Value::Str(mur.id.to_string())],
        );
        let c = g.controle();
        let niveau = |l: i64| nom_niveau.get(&l).map(|s| s.to_string()).unwrap_or_else(|| l.to_string());
        let props = vec![
            ("ElementId", Value::Str(mur.id.to_string())),
            ("Classe Revit", Value::Str(mur.classe.clone())),
            ("Type", Value::Str(format!("{nom_type} [{}]", mur.type_id))),
            ("Contrainte inférieure", Value::Str(niveau(mur.niveau))),
            ("Contrainte supérieure", Value::Str(if mur.niveau_haut == -1 { "Non contraint".into() } else { niveau(mur.niveau_haut) })),
            ("Hauteur", mm(mur.hauteur)),
            ("Décalage inférieur", mm(mur.decalage_bas)),
            ("Décalage supérieur", mm(mur.decalage_haut)),
            ("Faces", Value::Str(g.faces.len().to_string())),
            ("Matériaux des faces (ElementId)", Value::Str(materiaux(g))),
        ];
        let pset = b.pset(mur.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", mur.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        let qv = b.add("IFCQUANTITYVOLUME", vec![Value::Str("Volume".into()), n.clone(), n.clone(), Value::Real(c.volume_m3)]);
        let qa = b.add("IFCQUANTITYAREA", vec![Value::Str("Aire des faces".into()), n.clone(), n.clone(), Value::Real(c.aire_m2)]);
        let qto = b.add("IFCELEMENTQUANTITY", vec![gid("RVTQTO", mur.id), n.clone(), Value::Str("Géométrie Revit (GElement)".into()), n.clone(), n.clone(), Value::List(vec![Value::Ref(qv), Value::Ref(qa)])]);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEFQ", mur.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(qto)]);
        let t = match types.get(&mur.type_id) {
            Some(&t) => t,
            None => {
                let mut props = vec![("ElementId", Value::Str(mur.type_id.to_string()))];
                if let Some(e) = ty.and_then(|t| t.epaisseur) {
                    props.push(("Épaisseur", mm(e)));
                }
                let pset = b.pset(mur.type_id, "Revit", props);
                let t = b.add("IFCWALLTYPE", vec![gid("RVTTYPE", mur.type_id), n.clone(), Value::Str(nom_type), n.clone(), n.clone(), Value::List(vec![Value::Ref(pset)])]);
                types.insert(mur.type_id, t);
                t
            }
        };
        b.add("IFCRELDEFINESBYTYPE", vec![gid("RVTRELT", mur.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(t)]);
        contenus.entry(etage.get(&mur.niveau).copied().unwrap_or(projet)).or_default().push(Value::Ref(id));
        let tris = g.triangles().map(|t| t.map(|p| p / 1000.0)).collect();
        geoms.push(ProductGeom { id, parts: vec![Part { color, tris }] });
    }
    let mut contenus: Vec<(Id, Vec<Value>)> = contenus.into_iter().collect();
    contenus.sort_by_key(|c| c.0);
    for (k, (s, items)) in contenus.into_iter().enumerate() {
        b.add("IFCRELCONTAINEDINSPATIALSTRUCTURE", vec![gid("RVTCONT", k as i64), n.clone(), n.clone(), n.clone(), Value::List(items), Value::Ref(s)]);
    }
    let model = IfcModel::new(StepFile { schema: format!("REVIT {format}"), entities: b.entities });
    scene::assemble(model, geoms, file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Fichier d'essai hors dépôt (docs/PROTOCOLE-RVT.md) ; dossier remplaçable par la
    /// variable d'environnement RVT_ESSAIS. Absent : le test est sauté.
    fn essai(dossier: &str, nom: &str) -> Option<PathBuf> {
        let base = std::env::var("RVT_ESSAIS").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(r"C:\Users\theba\Desktop\Revit"));
        let p = base.join(dossier).join(nom);
        if p.exists() {
            Some(p)
        } else {
            eprintln!("fichier d'essai absent, test sauté : {}", p.display());
            None
        }
    }

    /// Essai E04 : mur de 12 345,6 × 200 × 2 718,2 mm, valeurs validées contre Revit 2026.
    #[test]
    fn e04_volume_et_boite() {
        let Some(p) = essai(r"Essais-format\2026", "E04_mur.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.murs.len(), 1);
        let mur = &m.murs[0];
        assert_eq!((mur.id, mur.classe.as_str(), mur.niveau, mur.type_id), (2797, "SWall", 1959, 1990));
        assert!((mur.hauteur - 2718.2).abs() < 0.05, "hauteur {}", mur.hauteur);
        let g = mur.geometrie.as_ref().unwrap();
        let c = g.controle();
        assert_eq!((g.sommets.len(), g.faces.len(), c.triangles, c.non_appariees), (8, 6, 12, 0));
        assert!((c.volume_m3 - 6.711562).abs() < 1e-6, "volume {}", c.volume_m3);
        assert!((c.aire_m2 - 73.1411).abs() < 1e-4, "aire {}", c.aire_m2);
        for (v, att) in [(c.min, [0.0, -100.0, 0.0]), (c.max, [12_345.6, 100.0, 2_718.2])] {
            for k in 0..3 {
                assert!((v[k] - att[k]).abs() < 1e-3, "boîte {v:?} au lieu de {att:?}");
            }
        }
        let t = &m.types[&1990];
        assert!((t.epaisseur.unwrap() - 200.0).abs() < 1e-6);
    }

    /// Projet1.rvt : 59 murs validés contre Revit 2026 (volume, aire et faces) ;
    /// totaux donnés par `geometrie.py`.
    #[test]
    fn projet1_59_murs() {
        let Some(p) = essai("Archives", "Projet1.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.murs.len(), 59);
        assert_eq!(m.niveaux.len(), 3);
        let (mut vol, mut tris) = (0.0, 0);
        for mur in &m.murs {
            let c = mur.geometrie.as_ref().unwrap_or_else(|e| panic!("mur {} : {e}", mur.id)).controle();
            assert_eq!(c.non_appariees, 0, "mur {} non fermé", mur.id);
            vol += c.volume_m3;
            tris += c.triangles;
        }
        assert!((vol - 257.974221).abs() < 1e-5, "volume total {vol}");
        assert_eq!(tris, 3768);
        let s = scene(&m, "Projet1", "Projet1.rvt".into(), "2026");
        assert_eq!(s.objects.len(), 59);
        assert_eq!(s.triangles, 3768);
    }
}
