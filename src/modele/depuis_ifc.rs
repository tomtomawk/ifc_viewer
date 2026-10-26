//! Conversion d'un fichier IFC vers le modèle v0.
//!
//! - Niveaux : `IfcBuildingStorey` (nom, élévation).
//! - Murs (`IfcWall`, `IfcWallStandardCase`, `IfcWallElementedCase`) : direction et
//!   extrémités de la représentation « Axis » (droite, ou arc), sinon de la plus longue
//!   arête horizontale de la géométrie ; ligne médiane, base et hauteur de la géométrie ;
//!   couches du jeu de couches de matériaux (`IfcMaterialLayerSetUsage`,
//!   `IfcMaterialLayerSet`, du mur ou de son type), sinon une couche de l'épaisseur lue
//!   dans la géométrie.
//! - Ouvertures : `IfcRelVoidsElement` d'un mur converti ; baie = encombrement de
//!   l'`IfcOpeningElement` dans le repère du mur (borné au mur) ; porte ou fenêtre selon
//!   l'élément qui la remplit (`IfcRelFillsElement`), dont la géométrie est gardée.
//! - Dalles (`IfcSlab`) prismatiques : contour du dessus, couches comme pour les murs.
//! - Le reste, avec une géométrie : éléments importés.

use super::geometrie::{cercle, direction_principale, etendue, etendue_arc, prisme_vertical};
use super::{Axe, Conversion, Couche, Dalle, GenreOuverture, Haut, Mur, Niveau, Origine, Ouverture, Projet, TypeCompose, identifiant, source};
use crate::geom::{self, GeomStats};
use crate::model::{IfcModel, pretty_type};
use crate::step::{Id, StepFile};
use glam::{DVec2, DVec3};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MURS: &[&str] = &["IFCWALL", "IFCWALLSTANDARDCASE", "IFCWALLELEMENTEDCASE"];
const DALLES: &[&str] = &["IFCSLAB", "IFCSLABSTANDARDCASE", "IFCSLABELEMENTEDCASE"];
/// Produits sans équivalent physique à importer.
const IGNORES: &[&str] = &["IFCOPENINGELEMENT", "IFCOPENINGSTANDARDCASE", "IFCSPACE", "IFCANNOTATION", "IFCVIRTUALELEMENT", "IFCGRID"];

pub fn convertir(chemin: &Path, progress: &dyn Fn(String)) -> Result<(Projet, Vec<super::Mesure>), String> {
    progress(format!("Lecture de {}…", chemin.display()));
    let data = std::fs::read(chemin).map_err(|e| format!("Lecture impossible : {e}"))?;
    if data.starts_with(b"PK") {
        return Err("Fichier IFC-ZIP (.ifczip) non pris en charge : décompresser d'abord.".into());
    }
    let model = IfcModel::new(StepFile::parse(&data)?);
    drop(data);
    progress(format!("Géométrie de {} produits…", model.products.len()));
    let stats = GeomStats::default();
    let pool = rayon::ThreadPoolBuilder::new().stack_size(64 << 20).build().map_err(|e| e.to_string())?;
    // triangles de chaque produit, en mm
    let geometries: HashMap<Id, Vec<[DVec3; 3]>> = pool.install(|| {
        model
            .products
            .par_iter()
            .filter_map(|&id| {
                let g = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| geom::product_geometry(&model, &stats, id))).ok().flatten()?;
                Some((id, g.parts.iter().flat_map(|p| p.tris.iter().map(|t| t.map(|v| v * 1000.0))).collect()))
            })
            .collect()
    });
    let nom = chemin.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut c = Conversion { projet: Projet::nouveau(&nom), ..Default::default() };
    c.projet.sources.push(source("s1", chemin, "IFC"));
    progress("Conversion…".into());
    convertir_modele(&model, &geometries, &mut c);
    c.valider();
    Ok((c.projet, c.mesures))
}

/// Jeu de couches associé à un élément ou à son type : entité du jeu, nom, couches (mm).
fn couches(m: &IfcModel, id: Id) -> Option<(Id, String, Vec<Couche>)> {
    let mat = m.material_of.get(&id).or_else(|| m.material_of.get(m.type_of.get(&id)?))?;
    let mut e = m.get(*mat)?;
    let mut jeu = *mat;
    if e.is("IFCMATERIALLAYERSETUSAGE") {
        jeu = e.arg(0).as_ref()?;
        e = m.get(jeu)?;
    }
    if !e.is("IFCMATERIALLAYERSET") {
        return None;
    }
    let couches: Vec<Couche> = e
        .arg(0)
        .refs()
        .filter_map(|l| m.get(l))
        .map(|l| Couche { materiau: l.arg(0).as_ref().and_then(|x| m.get(x)).and_then(|x| x.arg(0).as_str()).map(str::to_string), epaisseur: l.arg(1).as_f64().unwrap_or(0.0) * m.length_scale * 1000.0 })
        .collect();
    let total: f64 = couches.iter().map(|c| c.epaisseur).sum();
    (total > 0.0).then(|| (jeu, e.arg(1).as_str().map(str::to_string).unwrap_or_default(), couches))
}

pub fn convertir_modele(m: &IfcModel, geometries: &HashMap<Id, Vec<[DVec3; 3]>>, c: &mut Conversion) {
    let fichier = c.projet.sources[0].fichier.clone();
    let cle = |id: Id| {
        let g = m.global_id(id);
        if g.is_empty() { format!("{fichier}:#{id}") } else { g }
    };
    let ident = |genre: &str, id: Id| identifiant(genre, &cle(id));
    let origine = |id: Id| Some(Origine { source: "s1".into(), id: format!("#{id}"), guid: Some(m.global_id(id)).filter(|g| !g.is_empty()), classe: m.ty(id).to_string() });
    let vide: Vec<[DVec3; 3]> = Vec::new();
    let tris = |id: Id| geometries.get(&id).unwrap_or(&vide);

    // niveaux
    let mut niveaux: HashMap<Id, (String, f64)> = HashMap::new();
    let mut ids: Vec<Id> = m.file.entities.iter().filter(|(_, e)| e.is("IFCBUILDINGSTOREY")).map(|(&id, _)| id).collect();
    ids.sort_unstable();
    for id in ids {
        let altitude = m.get(id).and_then(|e| e.arg(9).as_f64()).unwrap_or(0.0) * m.length_scale * 1000.0;
        let nid = ident("niveau", id);
        niveaux.insert(id, (nid.clone(), altitude));
        c.projet.niveaux.push(Niveau { id: nid, nom: m.name(id), altitude, origine: origine(id) });
    }
    c.projet.niveaux.sort_by(|a, b| a.altitude.total_cmp(&b.altitude));
    let niveau = |id: Id| m.storey_of(id).and_then(|s| niveaux.get(&s)).cloned();

    let mut consommes: HashSet<Id> = HashSet::new();
    let mut types_murs: HashMap<String, String> = HashMap::new();
    let mut produits: Vec<Id> = m.products.clone();
    produits.sort_unstable();

    // murs et leurs ouvertures
    for &id in produits.iter().filter(|&&id| MURS.contains(&m.ty(id))) {
        consommes.insert(id);
        let t = tris(id);
        let nom = if m.name(id).is_empty() { format!("{} #{id}", pretty_type(m.ty(id))) } else { m.name(id) };
        let Some((niv, alt)) = niveau(id) else {
            c.importer(nom, "Murs".into(), None, t, "pas de niveau (IfcBuildingStorey)", origine(id));
            continue;
        };
        if t.is_empty() {
            *c.projet.ignores.entry("Murs (sans géométrie)".into()).or_default() += 1;
            continue;
        }
        // axe : représentation Axis, sinon arête horizontale la plus longue
        let pts: Vec<DVec2> = geom::axe(m, id).unwrap_or_default().into_iter().map(|p| p.truncate() * 1000.0).collect();
        let arc = if pts.len() >= 3 {
            let (a, b, d) = (pts[0], pts[pts.len() / 2], pts[pts.len() - 1]);
            cercle(a, b, d).filter(|(ctr, r)| pts.iter().all(|p| ((*p - *ctr).length() - r).abs() < 1.0))
        } else {
            None
        };
        let (axe, epaisseur_lue, z) = if let Some((ctr, _)) = arc {
            let (r, depart, balayage, z) = etendue_arc(t, ctr);
            let rm = (r[0] + r[1]) / 2.0;
            let p = |a: f64| (ctr + DVec2::new(a.cos(), a.sin()) * rm).to_array();
            (Axe::Arc { debut: p(depart), fin: p(depart + balayage), centre: ctr.to_array(), direct: true }, r[1] - r[0], z)
        } else {
            let (o, u, bornes) = match pts.as_slice() {
                [a, .., b] if pts.len() == 2 && (*b - *a).length() > 1.0 => (*a, (*b - *a).normalize(), Some((0.0, (*b - *a).length()))),
                [_, _, _, ..] => {
                    c.importer(nom, "Murs".into(), Some(niv), t, "axe en polyligne (ni droite ni arc)", origine(id));
                    continue;
                }
                _ => match direction_principale(t) {
                    Some(u) => (t[0][0].truncate(), u, None),
                    None => {
                        c.importer(nom, "Murs".into(), Some(niv), t, "direction du mur introuvable", origine(id));
                        continue;
                    }
                },
            };
            let (s, tt, z) = etendue(t, o, u);
            let (s0, s1) = bornes.unwrap_or((s[0], s[1]));
            let milieu = o + u.perp() * ((tt[0] + tt[1]) / 2.0);
            (Axe::Droit { debut: (milieu + u * s0).to_array(), fin: (milieu + u * s1).to_array() }, tt[1] - tt[0], z)
        };
        // type : jeu de couches, sinon épaisseur lue
        let (cle_type, nom_type, couches, detaillees) = match couches(m, id) {
            Some((jeu, n, cs)) => {
                let total: f64 = cs.iter().map(|x| x.epaisseur).sum();
                (format!("jeu:{}", cle(jeu)), if n.is_empty() { format!("Mur {total:.0} mm") } else { n }, cs, true)
            }
            None => (format!("e:{epaisseur_lue:.1}"), format!("Mur {epaisseur_lue:.0} mm"), vec![Couche { materiau: None, epaisseur: epaisseur_lue }], false),
        };
        let type_mur = types_murs
            .entry(cle_type.clone())
            .or_insert_with(|| {
                let tid = identifiant("type_mur", &format!("{fichier}:{cle_type}"));
                c.projet.types_murs.push(TypeCompose { id: tid.clone(), nom: nom_type, couches, couches_detaillees: detaillees, origine: None });
                tid
            })
            .clone();
        let mid = ident("mur", id);
        c.lire(&mid, t, "Murs");
        let mur = Mur { variante: None, phase: None, sous_projet: None, id: mid.clone(), nom, type_mur, niveau: niv, decalage_bas: z[0] - alt, haut: Haut::Libre { hauteur: z[1] - z[0] }, axe, origine: origine(id) };
        // ouvertures
        for &o in m.openings.get(&id).map(Vec::as_slice).unwrap_or_default() {
            consommes.insert(o);
            let to = tris(o);
            if to.is_empty() {
                continue;
            }
            let s: Vec<f64> = to.iter().flatten().map(|p| mur.axe.abscisse(p.truncate().to_array())).collect();
            let (s0, s1) = s.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
            let (z0, z1) = to.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
            let (z0, z1) = (z0.max(z[0]), z1.min(z[1]));
            if s1 - s0 < 1.0 || z1 - z0 < 1.0 {
                continue;
            }
            let remplissage = m.fillings.get(&o).and_then(|f| f.first().copied());
            let genre = match remplissage.map(|f| m.ty(f)) {
                Some("IFCDOOR" | "IFCDOORSTANDARDCASE") => GenreOuverture::Porte,
                Some("IFCWINDOW" | "IFCWINDOWSTANDARDCASE") => GenreOuverture::Fenetre,
                _ => GenreOuverture::Baie,
            };
            let source = remplissage.unwrap_or(o);
            if let Some(f) = remplissage {
                consommes.insert(f);
            }
            let rep = remplissage.map(tris).filter(|x| !x.is_empty()).map(|x| super::Maillage::depuis_triangles(x));
            c.projet.ouvertures.push(Ouverture {
                id: ident("ouverture", source),
                nom: m.name(source),
                genre,
                mur: mid.clone(),
                type_ouverture: None,
                abscisse: (s0 + s1) / 2.0,
                largeur: s1 - s0,
                hauteur: z1 - z0,
                allege: z0 - alt,
                representation: rep,
                origine: origine(source),
            });
        }
        c.projet.murs.push(mur);
    }

    // dalles
    let mut types_dalles: HashMap<String, String> = HashMap::new();
    for &id in produits.iter().filter(|&&id| DALLES.contains(&m.ty(id))) {
        consommes.insert(id);
        let t = tris(id);
        let nom = if m.name(id).is_empty() { format!("{} #{id}", pretty_type(m.ty(id))) } else { m.name(id) };
        let prisme = prisme_vertical(t);
        let (Some(p), Some((niv, alt))) = (prisme, niveau(id)) else {
            let raison = if t.is_empty() { "géométrie non lue" } else if niveau(id).is_none() { "pas de niveau (IfcBuildingStorey)" } else { "pas un prisme vertical (pente, forme)" };
            c.importer(nom, "Dalles".into(), niveau(id).map(|n| n.0), t, raison, origine(id));
            continue;
        };
        let lue = p.haut - p.bas;
        let (cle_type, nom_type, couches, detaillees) = match couches(m, id) {
            Some((jeu, n, cs)) => (format!("jeu:{}", cle(jeu)), if n.is_empty() { format!("Dalle {lue:.0} mm") } else { n }, cs, true),
            None => (format!("e:{lue:.1}"), format!("Dalle {lue:.0} mm"), vec![Couche { materiau: None, epaisseur: lue }], false),
        };
        let type_dalle = types_dalles
            .entry(cle_type.clone())
            .or_insert_with(|| {
                let tid = identifiant("type_dalle", &format!("{fichier}:{cle_type}"));
                c.projet.types_dalles.push(TypeCompose { id: tid.clone(), nom: nom_type, couches, couches_detaillees: detaillees, origine: None });
                tid
            })
            .clone();
        let did = ident("dalle", id);
        c.lire(&did, t, "Dalles");
        c.projet.dalles.push(Dalle { id: did, nom, type_dalle, niveau: niv, decalage: p.haut - alt, contours: p.contours, origine: origine(id) });
    }

    // le reste
    for &id in &produits {
        if consommes.contains(&id) {
            continue;
        }
        let ty = m.ty(id);
        if IGNORES.contains(&ty) {
            *c.projet.ignores.entry(pretty_type(ty)).or_default() += 1;
            continue;
        }
        let nom = if m.name(id).is_empty() { format!("{} #{id}", pretty_type(ty)) } else { m.name(id) };
        c.importer(nom, pretty_type(ty), niveau(id).map(|n| n.0), tris(id), "pas de modèle paramétrique en v0", origine(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mur droit à axe et couches avec une porte, mur courbe, dalle à couches, poteau.
    #[test]
    fn echantillon_murs() {
        let (p, mesures) = convertir(Path::new("samples/test_ifc4_murs.ifc"), &|_| {}).unwrap();
        assert_eq!(p.niveaux.len(), 2);
        assert_eq!(p.murs.len(), 2, "{:?}", p.importes.iter().map(|i| &i.raison).collect::<Vec<_>>());
        let droit = p.murs.iter().find(|m| m.nom == "Mur droit").unwrap();
        let Axe::Droit { debut, fin } = droit.axe else { panic!() };
        assert!(debut[0].abs() < 1e-6 && debut[1].abs() < 1e-6 && (fin[0] - 6000.0).abs() < 1e-6 && fin[1].abs() < 1e-6, "{:?}", droit.axe);
        let t = p.type_mur(&droit.type_mur).unwrap();
        assert!(t.couches_detaillees && t.couches.len() == 2 && (t.epaisseur() - 250.0).abs() < 1e-9);
        assert_eq!(t.couches[0].materiau.as_deref(), Some("Béton"));
        assert_eq!(droit.haut, Haut::Libre { hauteur: 2700.0 });
        let porte = &p.ouvertures[0];
        assert_eq!(porte.genre, GenreOuverture::Porte);
        assert!((porte.abscisse - 1450.0).abs() < 1e-6 && (porte.largeur - 900.0).abs() < 1e-6 && (porte.hauteur - 2100.0).abs() < 1e-6 && porte.allege.abs() < 1e-6);
        assert!(porte.representation.is_some());
        assert!((p.volume_mur(droit).unwrap() - 3.5775e9).abs() < 1e3);
        let courbe = p.murs.iter().find(|m| m.nom == "Mur courbe").unwrap();
        let Axe::Arc { centre, .. } = courbe.axe else { panic!("{:?}", courbe.axe) };
        assert!((centre[0] - 8000.0).abs() < 0.05 && centre[1].abs() < 0.05, "{centre:?}");
        assert!((courbe.axe.longueur() - std::f64::consts::FRAC_PI_2 * 3000.0).abs() < 1.0, "{}", courbe.axe.longueur());
        assert_eq!(p.dalles.len(), 1);
        assert!((p.volume_dalle(&p.dalles[0]).unwrap() - 4.8e9).abs() < 1e3);
        assert!((p.dalles[0].decalage).abs() < 1e-6);
        // poteau importé, porte consommée par son ouverture
        assert_eq!(p.importes.len(), 1);
        assert_eq!(p.importes[0].categorie, "IfcColumn");
        // mur droit et dalle exacts ; mur courbe : la géométrie lue est facettée, les
        // paramètres donnent l'arc exact (écart de l'ordre du pour cent)
        assert!(mesures.iter().all(|x| x.retenu), "{mesures:?}");
        assert!(mesures.iter().filter(|x| x.nom != "Mur courbe").all(|x| x.ecart().abs() < 1e-6), "{mesures:?}");
        assert!(mesures.iter().any(|x| x.nom == "Mur courbe" && x.ecart() > 0.0 && x.ecart() < 0.03), "{mesures:?}");
    }

    /// Échantillon historique : mur sans axe ni couches (direction lue dans la géométrie),
    /// percé d'une baie ; dalle au contour courbe.
    #[test]
    fn echantillon_historique() {
        let (p, mesures) = convertir(Path::new("samples/test_ifc4.ifc"), &|_| {}).unwrap();
        assert_eq!(p.murs.len(), 1);
        assert_eq!(p.ouvertures.len(), 1);
        assert_eq!(p.ouvertures[0].genre, GenreOuverture::Baie);
        assert!((p.volume_mur(&p.murs[0]).unwrap() - 2.76e9).abs() < 1e6);
        assert_eq!(p.dalles.len(), 1);
        assert!(mesures.iter().all(|x| x.retenu), "{mesures:?}");
    }
}
