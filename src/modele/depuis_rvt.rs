//! Conversion d'un fichier Revit vers le modèle v0, à partir des seules données déjà
//! décodées par `rvt::maquette` (aucune lecture nouvelle du format).
//!
//! - Niveaux : nom, altitude.
//! - Murs (`SWall`, `ArcWall`) dont le type a une épaisseur : axe, type, niveaux,
//!   décalages. Le tracé lu (`m_pCurveDriver`) donne la droite ou le cercle porteur, pas
//!   les extrémités : elles et la ligne médiane viennent de la géométrie du mur, projetée
//!   sur ce tracé (ce qui rend le résultat indépendant de la ligne d'implantation choisie
//!   dans Revit). Les couches du type ne sont pas lues : une couche de l'épaisseur totale.
//! - Portes et fenêtres hébergées par un mur converti : baie aux cotes du type (largeur,
//!   hauteur), centrée sur le point d'insertion ; allège = altitude du point d'insertion
//!   au-dessus du niveau du mur ; sans cotes de type, cotes de l'encombrement de la
//!   famille le long du mur.
//! - Sols et plafonds prismatiques : contour du dessus, épaisseur du type.
//! - Tout le reste (toits, escaliers, garde-corps, terrain, autres familles, formes d'un
//!   document de famille, murs-rideaux, sols en pente) : éléments importés.

use super::geometrie::{etendue, etendue_arc, prisme_vertical};
use super::{Axe, Conversion, Couche, Dalle, GenreOuverture, Haut, Mur, Niveau, Origine, Ouverture, Projet, TypeCompose, TypeOuverture, identifiant, source};
use crate::rvt::maquette::{self, Geometrie, Maquette, Trace};
use glam::{DVec2, DVec3};
use std::collections::HashMap;
use std::path::Path;

/// Triangles (mm) d'une géométrie lue.
fn triangles(g: &Geometrie) -> Vec<[DVec3; 3]> {
    match g {
        Ok(m) => m.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| m.sommets[i]))).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn convertir(chemin: &Path, progress: &dyn Fn(String)) -> Result<(Projet, Vec<super::Mesure>), String> {
    let m = maquette::lire(chemin, progress)?;
    let nom = chemin.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut c = Conversion { projet: Projet::nouveau(&nom), ..Default::default() };
    c.projet.sources.push(source("s1", chemin, "RVT"));
    convertir_maquette(&m, &mut c);
    c.valider();
    Ok((c.projet, c.mesures))
}

pub fn convertir_maquette(m: &Maquette, c: &mut Conversion) {
    let origine = |id: i64, classe: &str| Some(Origine { source: "s1".into(), id: id.to_string(), guid: m.guid_ifc(id), classe: classe.into() });
    // identifiant stable : UniqueId de Revit s'il est connu, sinon l'ElementId
    let fichier = c.projet.sources[0].fichier.clone();
    let cle = |id: i64| m.unique_id(id).unwrap_or_else(|| format!("{fichier}:{id}"));
    let ident = |genre: &str, id: i64| identifiant(genre, &cle(id));

    // niveaux
    let mut niveaux: HashMap<i64, (String, f64)> = HashMap::new();
    for n in &m.niveaux {
        let id = ident("niveau", n.id);
        niveaux.insert(n.id, (id.clone(), n.altitude));
        c.projet.niveaux.push(Niveau { id, nom: n.nom.clone(), altitude: n.altitude, origine: origine(n.id, "Level") });
    }
    c.projet.niveaux.sort_by(|a, b| a.altitude.total_cmp(&b.altitude));
    let niveau = |id: i64| niveaux.get(&id).map(|n| n.0.clone());
    let nom_type = |t: i64| m.types.get(&t).map(|x| x.nom.clone()).unwrap_or_else(|| format!("Type {t}"));

    // murs
    let mut types_murs: HashMap<i64, String> = HashMap::new();
    let mut murs: HashMap<i64, usize> = HashMap::new();
    for w in &m.murs {
        let tris = triangles(&w.geometrie);
        let nom = format!("{} [{}]", nom_type(w.type_id), w.id);
        let epaisseur = m.types.get(&w.type_id).and_then(|t| t.epaisseur).filter(|e| *e > 0.0);
        let raison = match (&w.trace, epaisseur, niveau(w.niveau)) {
            (None, _, _) => Some("tracé du mur non lu"),
            (_, None, _) => Some("type sans épaisseur (mur-rideau, mur empilé)"),
            (_, _, None) => Some("niveau de base inconnu"),
            _ if tris.is_empty() => Some("géométrie non lue"),
            _ => None,
        };
        if let Some(r) = raison {
            c.importer(nom, "Murs".into(), niveau(w.niveau), &tris, r, origine(w.id, &w.classe));
            continue;
        }
        let (Some(trace), Some(e), Some(base)) = (&w.trace, epaisseur, niveau(w.niveau)) else { continue };
        let axe = match *trace {
            Trace::Ligne { origine: o, direction } => {
                let u = direction.truncate().normalize_or_zero();
                if u == DVec2::ZERO {
                    c.importer(nom, "Murs".into(), Some(base), &tris, "tracé vertical", origine(w.id, &w.classe));
                    continue;
                }
                let o = o.truncate();
                let (s, t, _) = etendue(&tris, o, u);
                let milieu = o + u.perp() * ((t[0] + t[1]) / 2.0);
                let (a, b) = (milieu + u * s[0], milieu + u * s[1]);
                Axe::Droit { debut: a.to_array(), fin: b.to_array() }
            }
            Trace::Arc { centre, .. } => {
                let ctr = centre.truncate();
                let (r, depart, balayage, _) = etendue_arc(&tris, ctr);
                let rm = (r[0] + r[1]) / 2.0;
                let pt = |a: f64| (ctr + DVec2::new(a.cos(), a.sin()) * rm).to_array();
                Axe::Arc { debut: pt(depart), fin: pt(depart + balayage), centre: ctr.to_array(), direct: true }
            }
        };
        let type_mur = types_murs
            .entry(w.type_id)
            .or_insert_with(|| {
                let id = ident("type_mur", w.type_id);
                c.projet.types_murs.push(TypeCompose { id: id.clone(), nom: nom_type(w.type_id), couches: vec![Couche { materiau: None, epaisseur: e }], couches_detaillees: false, origine: origine(w.type_id, "WallType") });
                id
            })
            .clone();
        let haut = match niveau(w.niveau_haut) {
            Some(n) => Haut::Niveau { niveau: n, decalage: w.decalage_haut },
            None => Haut::Libre { hauteur: w.hauteur },
        };
        let id = ident("mur", w.id);
        c.lire(&id, &tris, "Murs");
        murs.insert(w.id, c.projet.murs.len());
        c.projet.murs.push(Mur { id, nom, type_mur, niveau: base, decalage_bas: w.decalage_bas, haut, axe, origine: origine(w.id, &w.classe) });
    }

    // portes et fenêtres hébergées par un mur converti ; autres familles importées
    let mut types_ouv: HashMap<i64, String> = HashMap::new();
    for i in &m.instances {
        let tris = triangles(&i.geometrie);
        let (categorie, classe) = m.categorie(i);
        let nom = format!("{} [{}]", nom_type(i.type_id), i.id);
        let genre = match classe {
            "IFCDOOR" => Some(GenreOuverture::Porte),
            "IFCWINDOW" => Some(GenreOuverture::Fenetre),
            _ => None,
        };
        let hote = murs.get(&i.hote).map(|&k| c.projet.murs[k].clone());
        let (Some(genre), Some(mur)) = (genre, hote) else {
            let raison = if genre.is_some() { "hôte non converti en mur paramétrique" } else { "famille (pas de modèle paramétrique en v0)" };
            c.importer(nom, categorie, niveau(i.niveau), &tris, raison, origine(i.id, "FamilyInstance"));
            continue;
        };
        let alt_mur = c.projet.niveau(&mur.niveau).map_or(0.0, |n| n.altitude);
        let t = m.types.get(&i.type_id);
        let cotes = t.and_then(|t| Some((t.largeur.filter(|l| *l > 0.0)?, t.hauteur.filter(|h| *h > 0.0)?)));
        let (abscisse, largeur, hauteur, allege) = match cotes {
            Some((l, h)) => (mur.axe.abscisse(i.point.truncate().to_array()), l, h, i.point.z - alt_mur),
            None => {
                // encombrement de la famille le long du mur
                if tris.is_empty() {
                    c.importer(nom, categorie, niveau(i.niveau), &tris, "ni cotes de type ni géométrie", origine(i.id, "FamilyInstance"));
                    continue;
                }
                let s: Vec<f64> = tris.iter().flatten().map(|p| mur.axe.abscisse(p.truncate().to_array())).collect();
                let (s0, s1) = s.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
                let (z0, z1) = tris.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
                ((s0 + s1) / 2.0, s1 - s0, z1 - z0, z0 - alt_mur)
            }
        };
        let type_ouverture = cotes.map(|(l, h)| {
            types_ouv
                .entry(i.type_id)
                .or_insert_with(|| {
                    let id = ident("type_ouverture", i.type_id);
                    c.projet.types_ouvertures.push(TypeOuverture { id: id.clone(), nom: nom_type(i.type_id), genre, largeur: l, hauteur: h, origine: origine(i.type_id, "FamilySymbol") });
                    id
                })
                .clone()
        });
        let representation = (!tris.is_empty()).then(|| super::Maillage::depuis_triangles(&tris));
        c.projet.ouvertures.push(Ouverture {
            id: ident("ouverture", i.id),
            nom,
            genre,
            mur: mur.id.clone(),
            type_ouverture,
            abscisse,
            largeur,
            hauteur,
            allege,
            representation,
            origine: origine(i.id, "FamilyInstance"),
        });
    }

    // sols et plafonds
    let mut types_dalles: HashMap<i64, String> = HashMap::new();
    for d in &m.dalles {
        let tris = triangles(&d.geometrie);
        let categorie = if d.classe == "Ceiling" { "Plafonds" } else { "Sols" };
        let nom = format!("{} [{}]", nom_type(d.type_id), d.id);
        let prisme = prisme_vertical(&tris);
        let (Some(p), Some(base)) = (prisme, niveaux.get(&d.niveau)) else {
            let raison = if tris.is_empty() { "géométrie non lue" } else if niveau(d.niveau).is_none() { "niveau inconnu" } else { "pas un prisme vertical (pente, forme)" };
            c.importer(nom, categorie.into(), niveau(d.niveau), &tris, raison, origine(d.id, &d.classe));
            continue;
        };
        let e = m.types.get(&d.type_id).and_then(|t| t.epaisseur).filter(|e| *e > 0.0).unwrap_or(p.haut - p.bas);
        let type_dalle = types_dalles
            .entry(d.type_id)
            .or_insert_with(|| {
                let id = ident("type_dalle", d.type_id);
                c.projet.types_dalles.push(TypeCompose { id: id.clone(), nom: nom_type(d.type_id), couches: vec![Couche { materiau: None, epaisseur: e }], couches_detaillees: false, origine: origine(d.type_id, "FloorType") });
                id
            })
            .clone();
        let id = ident("dalle", d.id);
        c.lire(&id, &tris, categorie);
        c.projet.dalles.push(Dalle { id, nom, type_dalle, niveau: base.0.clone(), decalage: p.haut - base.1, contours: p.contours, origine: origine(d.id, &d.classe) });
    }

    // le reste : éléments importés
    let autres = m
        .toits
        .iter()
        .map(|x| ("Toits", x.id, x.type_id, x.niveau, &x.geometrie, x.classe.as_str()))
        .chain(m.toposolides.iter().map(|x| ("Toposolides", x.id, x.type_id, x.niveau, &x.geometrie, x.classe.as_str())))
        .chain(m.escaliers.iter().map(|x| ("Escaliers", x.id, x.type_id, x.niveau_bas, &x.geometrie, "Stairs")))
        .chain(m.garde_corps.iter().map(|x| ("Garde-corps", x.id, x.type_id, x.niveau, &x.geometrie, "Railing")))
        .chain(m.surfaces_topo.iter().map(|x| ("Surfaces topographiques", x.id, -1, -1, &x.geometrie, "SiteSurface")));
    for (categorie, id, type_id, niv, g, classe) in autres {
        let nom = if type_id >= 0 { format!("{} [{id}]", nom_type(type_id)) } else { format!("{categorie} [{id}]") };
        c.importer(nom, categorie.into(), niveau(niv), &triangles(g), "pas de modèle paramétrique en v0", origine(id, classe));
    }
    // document de famille : ses formes (pleines ; les vides sont déjà soustraits dans les
    // combinaisons)
    let membres: std::collections::HashSet<i64> = m.formes.iter().flat_map(|f| f.membres.iter().copied()).collect();
    for f in m.formes.iter().filter(|f| !membres.contains(&f.id)) {
        let categorie = format!("Formes ({})", m.categorie_famille(f.famille).0);
        c.importer(format!("{} [{}]", f.classe, f.id), categorie, niveau(f.niveau), &triangles(&f.geometrie), "forme de famille", origine(f.id, &f.classe));
    }
    let ignores = &mut c.projet.ignores;
    for (k, n) in [("Pièces, surfaces et zones", m.pieces.len()), ("Quadrillages", m.grilles.len()), ("Éléments 2D des vues", m.elements_2d.len())] {
        if n > 0 {
            *ignores.entry(k.into()).or_default() += n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Fichier des essais Revit (`RVT_ESSAIS`) ou du corpus (`RVT_CORPUS`), comme les
    /// tests de `rvt::maquette` ; absent : test sauté.
    fn fichier(variable: &str, defaut: &str, chemin: &str) -> Option<PathBuf> {
        let p = std::env::var(variable).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(defaut)).join(chemin);
        if p.exists() {
            Some(p)
        } else {
            eprintln!("fichier absent, test sauté : {}", p.display());
            None
        }
    }

    /// Conversion d'un projet : rapport écrit dans `exports/` (hors dépôt), projet relu.
    fn convertir_et_rapporter(p: &Path) -> Projet {
        let t0 = std::time::Instant::now();
        let (projet, mesures) = convertir(p, &|_| {}).unwrap();
        let rapport = super::super::rapport::markdown(&projet, &mesures, t0.elapsed().as_secs_f64());
        let nom = p.file_stem().unwrap().to_string_lossy();
        let _ = std::fs::create_dir_all("exports");
        let _ = std::fs::write(format!("exports/{nom}.rapport.md"), &rapport);
        eprintln!("{rapport}");
        let relu = Projet::lire_json(&projet.json().unwrap()).unwrap();
        assert_eq!(relu.murs.len(), projet.murs.len());
        projet
    }

    /// Projet1.rvt (59 murs) : la plupart des murs deviennent paramétriques.
    #[test]
    fn projet1() {
        let Some(p) = fichier("RVT_ESSAIS", r"C:\Users\theba\Desktop\Revit", r"Archives\Projet1.rvt") else { return };
        let projet = convertir_et_rapporter(&p);
        assert!(!projet.niveaux.is_empty());
        assert!(projet.murs.len() * 2 >= 59, "{} murs paramétriques sur 59", projet.murs.len());
    }

    /// Kerreut 2025.rvt (corpus de l'agence) : rapport seul.
    #[test]
    fn kerreut() {
        let Some(p) = fichier("RVT_CORPUS", r"C:\Users\theba\Documents\Thomas\ARCHI PERSO", r"PERROS-GUIREC - rue de Kerreut\01-TRAVAIL\Kerreut 2025.rvt") else { return };
        let projet = convertir_et_rapporter(&p);
        assert!(!projet.murs.is_empty());
    }

    /// Familles de fenêtres (`RVT_FAMILLES`, dossier de .rfa) : chaque document se
    /// convertit ; ses formes deviennent des éléments importés.
    #[test]
    fn familles() {
        let Ok(dossier) = std::env::var("RVT_FAMILLES") else {
            eprintln!("RVT_FAMILLES non défini, test sauté");
            return;
        };
        for e in std::fs::read_dir(dossier).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("rfa"))) {
            let (projet, _) = convertir(&e.path(), &|_| {}).unwrap();
            assert!(projet.importes.iter().any(|i| i.categorie.starts_with("Formes")), "{}", e.path().display());
        }
    }
}
