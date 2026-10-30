//! Conversion d'un fichier Revit vers le modèle v0, à partir des seules données déjà
//! décodées par `rvt::maquette` (aucune lecture nouvelle du format).
//!
//! - Niveaux : nom, altitude.
//! - Murs (`SWall`, `ArcWall`) dont le type a une épaisseur : axe, type, niveaux,
//!   décalages. Le tracé lu (`m_pCurveDriver`) donne la droite ou le cercle porteur, pas
//!   les extrémités : elles et la ligne médiane viennent de la géométrie du mur, projetée
//!   sur ce tracé (ce qui rend le résultat indépendant de la ligne d'implantation choisie
//!   dans Revit). Les couches du type conservent leurs épaisseurs, matériaux et ordre.
//! - Portes et fenêtres hébergées par un mur converti : baie aux cotes du type (largeur,
//!   hauteur), centrée sur le point d'insertion ; allège = altitude du point d'insertion
//!   au-dessus du niveau du mur ; sans cotes de type, cotes de l'encombrement de la
//!   famille le long du mur.
//! - Sols et plafonds prismatiques : contour du dessus, épaisseur du type.
//! - Tout le reste (toits, escaliers, garde-corps, terrain, autres familles, formes d'un
//!   document de famille, murs-rideaux, sols en pente) : éléments importés.

use super::geometrie::{etendue, etendue_arc, prisme_vertical};
use super::{Axe, Conversion, Couche, Dalle, GenreOuverture, Haut, Mur, Niveau, Origine, Ouverture, Projet, SorteDalle, TypeCompose, TypeOuverture, identifiant, source};
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

fn couches_type(m: &Maquette, type_id: i64, epaisseur: f64) -> (Vec<Couche>, bool) {
    if let Some(couches) = m.types.get(&type_id).and_then(|t| t.couches.as_ref()) {
        let total: f64 = couches.iter().map(|c| c.epaisseur).sum();
        if total > 0.0 && (total - epaisseur).abs() <= 0.1 {
            return (couches.iter().map(|c| Couche {
                epaisseur: c.epaisseur,
                materiau: (c.materiau != -1).then(|| m.materiaux.get(&c.materiau).map(|m| m.nom.clone()).unwrap_or_else(|| format!("Matériau {}", c.materiau))),
                ..Default::default()
            }).collect(), true);
        }
    }
    (vec![Couche { materiau: None, epaisseur, ..Default::default() }], false)
}

pub fn convertir(chemin: &Path, progress: &dyn Fn(String)) -> Result<(Projet, Vec<super::Mesure>), String> {
    let m = maquette::lire(chemin, progress)?;
    let nom = chemin.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut c = Conversion { projet: Projet::nouveau(&nom), ..Default::default() };
    c.projet.sources.push(source("s1", chemin, "RVT"));
    if let Ok(infos) = maquette::infos_document(chemin) {
        c.projet.sous_projets = infos.sous_projets;
    }
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

    c.projet.phases = m.ordre_phases.iter().filter_map(|id| m.phases.get(id).cloned()).collect();

    c.projet.variantes = m.variantes.values().filter(|v| !v.jeu).map(|v| super::Variante {
        id: ident("variante", v.id), nom: v.nom.clone(),
        jeu: m.variantes.get(&v.lien).map_or_else(String::new, |j| j.nom.clone()),
    }).collect();
    c.projet.variantes.sort_by(|a,b| a.jeu.cmp(&b.jeu).then(a.nom.cmp(&b.nom)).then(a.id.cmp(&b.id)));

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

    for x in m.materiaux.values() {
        c.couleurs.insert(x.nom.clone(), x.couleur);
    }
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
                let (couches, couches_detaillees) = couches_type(m, w.type_id, e);
                c.projet.types_murs.push(TypeCompose { id: id.clone(), nom: nom_type(w.type_id), couches, couches_detaillees, origine: origine(w.type_id, "WallType") });
                id
            })
            .clone();
        let haut = match niveau(w.niveau_haut) {
            Some(n) => Haut::Niveau { niveau: n, decalage: w.decalage_haut },
            None => Haut::Libre { hauteur: w.hauteur },
        };
        let id = ident("mur", w.id);
        c.lire(&id, &tris, "Murs");
        // côté extérieur vérifié par les matériaux des faces : couches en assemblage
        if let (Some(ext), Axe::Droit { debut, fin }) = (maquette::direction_exterieure(m, w), &axe) {
            let gauche = DVec2::new(-(fin[1] - debut[1]), fin[0] - debut[0]);
            c.sens_couches.insert(id.clone(), ext.dot(gauche) < 0.0);
        }
        murs.insert(w.id, c.projet.murs.len());
        c.projet.murs.push(Mur { variante: m.variante_de.get(&w.id).filter(|id| m.variantes.get(id).is_some_and(|v| !v.jeu)).map(|&id| ident("variante", id)), phase: None, sous_projet: None, id, nom, type_mur, assemblage: None, niveau: base, decalage_bas: w.decalage_bas, haut, axe, origine: origine(w.id, &w.classe) });
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
                    c.projet.types_ouvertures.push(TypeOuverture { id: id.clone(), nom: nom_type(i.type_id), genre, largeur: l, hauteur: h, famille: None, modele: None, origine: origine(i.type_id, "FamilySymbol") });
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
            valeurs: Default::default(),
                    retournee: false,
                    inversee: false,
            origine: origine(i.id, "FamilyInstance"),
        });
    }

    // sols et plafonds
    let mut types_dalles: HashMap<i64, String> = HashMap::new();
    for d in &m.dalles {
        let tris = triangles(&d.geometrie);
        let sorte = if d.classe == "Ceiling" { SorteDalle::Plafond } else { SorteDalle::Dalle };
        let categorie = sorte.categorie();
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
                let (couches, couches_detaillees) = couches_type(m, d.type_id, e);
                c.projet.types_dalles.push(TypeCompose { id: id.clone(), nom: nom_type(d.type_id), couches, couches_detaillees, origine: origine(d.type_id, "FloorType") });
                id
            })
            .clone();
        let id = ident("dalle", d.id);
        c.lire(&id, &tris, categorie);
        c.projet.dalles.push(Dalle { variante: None, phase: None, sous_projet: None, id, nom, type_dalle, niveau: base.0.clone(), sorte, decalage: if sorte.sous_face() { p.bas - base.1 } else { p.haut - base.1 }, contours: p.contours, pentes: Vec::new(), origine: origine(d.id, &d.classe) });
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
    vues_et_feuilles(m, c, &|v: i64| ident("vue", v), &|f: i64| ident("feuille", f), &niveau);
    nomenclatures(m, c, &|v: i64| ident("nomenclature", v));
    let ignores = &mut c.projet.ignores;
    for (k, n) in [("Pièces, surfaces et zones", m.pieces.len()), ("Quadrillages", m.grilles.len()), ("Éléments 2D des vues", m.elements_2d.len())] {
        if n > 0 {
            *ignores.entry(k.into()).or_default() += n;
        }
    }
}

/// Catégorie native d'une catégorie Revit, pour le filtre d'une nomenclature (les
/// autres catégories gardent leur nom, celui des éléments importés).
fn categorie_native(revit: &str) -> String {
    match revit {
        "Sols" => "Dalle",
        "Plafonds" => "Plafond",
        "Toits" => "Toiture",
        "Poteaux porteurs" | "Poteaux" => "Poteaux",
        "Ossature" => "Poutres",
        autre => autre,
    }
    .into()
}

/// Nomenclatures de Revit en nomenclatures natives : nom, catégorie (nom natif),
/// détail de chaque occurrence (sinon regroupée par type), champs que le modèle natif
/// calcule (type, niveau, nombre, longueur, hauteur, largeur, épaisseur, surface,
/// volume). Les autres champs, filtres et tris sont comptés non repris ; les
/// nomenclatures de clés sont ignorées.
fn nomenclatures(m: &Maquette, c: &mut Conversion, id_nomenclature: &dyn Fn(i64) -> String) {
    use super::documentation::{Colonne, Nomenclature};
    let mut ignorees: std::collections::BTreeMap<String, usize> = Default::default();
    for v in m.vues.iter().filter(|v| !v.gabarit) {
        let Some(d) = &v.nomenclature else { continue };
        if d.cle {
            *ignorees.entry("Nomenclatures de clés".into()).or_default() += 1;
            continue;
        }
        let categorie = categorie_native(&m.nom_categorie(d.categorie));
        let regrouper = !d.detail;
        let mut colonnes: Vec<Colonne> = Vec::new();
        for champ in d.champs.iter().filter(|x| !x.masque) {
            match Colonne::depuis_titre(&champ.titre) {
                Some(col) if !colonnes.contains(&col) => colonnes.push(col),
                Some(_) => {}
                None => *ignorees.entry("Champs de nomenclature non repris".into()).or_default() += 1,
            }
        }
        // première colonne : le type (regroupée) ou l'élément
        if !colonnes.contains(&Colonne::Type) && !colonnes.contains(&Colonne::Element) {
            colonnes.insert(0, if regrouper { Colonne::Type } else { Colonne::Element });
        }
        if !d.filtres.is_empty() {
            *ignorees.entry("Filtres de nomenclature non repris".into()).or_default() += d.filtres.len();
        }
        if !d.tris.is_empty() {
            *ignorees.entry("Tris de nomenclature non repris (tri par colonnes)".into()).or_default() += 1;
        }
        c.projet.nomenclatures.push(Nomenclature { id: id_nomenclature(v.id), nom: v.nom.clone(), categorie, regrouper, colonnes });
    }
    for (k, n) in ignorees {
        *c.projet.ignores.entry(k).or_default() += n;
    }
}

/// Ligne de coupe native d'une coupe ou élévation Revit verticale cadrée (mm) : début,
/// fin, profondeur, direction de la ligne. La ligne est dans le plan proche du cadrage
/// (plan de coupe), sur la largeur du cadrage ; elle suit la droite de la vue, retournée
/// s'il le faut pour regarder du même côté (une coupe native regarde à gauche de sa
/// ligne). None si le cadrage est trop étroit.
fn ligne_de_coupe(cadre: &crate::rvt::vues::Cadre, droite: DVec3, direction: DVec3) -> Option<(DVec3, DVec3, f64, DVec3)> {
    // profondeur : axe du cadrage qui s'éloigne de l'observateur
    let mut a = cadre.axes[2].normalize_or_zero();
    let (mut pres, mut loin) = (cadre.min[2], cadre.max[2]);
    if a.dot(direction) > 0.0 {
        a = -a;
        (pres, loin) = (-cadre.max[2], -cadre.min[2]);
    }
    let p0 = cadre.origine + a * pres;
    // largeur : coins du cadrage le long de la droite de la vue
    let r = DVec3::new(droite.x, droite.y, 0.0).normalize_or_zero();
    let (mut s0, mut s1) = (f64::MAX, f64::MIN);
    for k in 0..8 {
        let q = cadre.origine
            + cadre.axes[0] * if k & 1 == 0 { cadre.min[0] } else { cadre.max[0] }
            + cadre.axes[1] * if k & 2 == 0 { cadre.min[1] } else { cadre.max[1] }
            + cadre.axes[2] * if k & 4 == 0 { cadre.min[2] } else { cadre.max[2] };
        let t = (q - p0).dot(r);
        (s0, s1) = (s0.min(t), s1.max(t));
    }
    if r == DVec3::ZERO || s1 - s0 < 100.0 {
        return None;
    }
    let u = if r.cross(DVec3::Z).dot(direction) >= 0.0 { r } else { -r };
    let (debut, fin) = if u == r { (p0 + r * s0, p0 + r * s1) } else { (p0 + r * s1, p0 + r * s0) };
    let profondeur = if cadre.loin && loin > pres { loin - pres } else { 50_000.0 };
    Some((debut, fin, profondeur, u))
}

/// Position native (mm du papier, repère de la feuille Revit) d'une vue placée : sur la
/// feuille Revit, p ↦ décalage + (p − o)·(droite, haut) × échelle ; sur la feuille
/// native, p ↦ position + (p − o_n)·(u_n, haut) / échelle native, avec droite = u_n.
fn position_placee(decalage: DVec3, echelle: f64, origine: DVec3, haut: DVec3, o_n: DVec3, u_n: DVec3) -> [f64; 2] {
    let d = o_n - origine;
    [decalage.x + d.dot(u_n) * echelle, decalage.y + d.dot(haut) * echelle]
}

/// Vues et feuilles de Revit en vues et feuilles natives. Plans d'étage : niveau et
/// hauteur du plan de coupe (plage de vue). Coupes et élévations verticales cadrées :
/// ligne le long du plan de coupe (bord proche du cadrage), largeur et profondeur du
/// cadrage, regard du même côté. Feuilles : numéro, nom, format du cartouche, vues
/// placées converties à la même position et la même échelle (repère du cartouche). Les
/// autres vues (3D, plafonds, dessin, légendes, nomenclatures) sont comptées ignorées.
fn vues_et_feuilles(m: &Maquette, c: &mut Conversion, id_vue: &dyn Fn(i64) -> String, id_feuille: &dyn Fn(i64) -> String, niveau: &dyn Fn(i64) -> Option<String>) {
    use super::{GenreVue, Vue as VueNative, documentation::{Feuille, VuePlacee, JEU_DEFAUT}};
    let altitude = |n: i64| m.niveaux.iter().find(|x| x.id == n).map(|x| x.altitude);
    let nom_type = |t: i64| m.types.get(&t).map(|x| x.nom.clone()).unwrap_or_default();
    // vue Revit → (vue native, origine native (mm), axe horizontal natif)
    let mut converties: HashMap<i64, (String, DVec3, DVec3)> = HashMap::new();
    let mut ignorees: std::collections::BTreeMap<String, usize> = Default::default();
    let mut noms: std::collections::HashSet<String> = c.projet.vues.iter().map(|v| v.nom.clone()).collect();
    let mut pivotes = 0;
    for v in m.vues.iter().filter(|v| !v.gabarit && v.genre != "Arborescence" && v.genre != "Feuille" && v.nomenclature.is_none()) {
        let genre = match v.genre {
            "Plan d'étage" => {
                let Some(n) = niveau(v.niveau) else {
                    *ignorees.entry("Plans d'étage sans niveau connu".into()).or_default() += 1;
                    continue;
                };
                // plan de coupe : décalage depuis son niveau de référence, ramené au niveau de la vue
                let coupe = v.plage_vue.map_or(1200.0, |p| {
                    let (r, d) = p[1];
                    let ecart = if r == -3 || r == v.niveau { 0.0 } else { altitude(r).zip(altitude(v.niveau)).map_or(0.0, |(a, b)| a - b) };
                    ecart + d
                });
                if (v.droite - DVec3::X).length() > 1e-3 {
                    pivotes += 1;
                }
                let z = altitude(v.niveau).unwrap_or(0.0) + coupe;
                converties.insert(v.id, (id_vue(v.id), DVec3::new(0.0, 0.0, z), DVec3::X));
                GenreVue::Plan { niveau: n, coupe }
            }
            "Coupe" | "Élévation" => {
                let ligne = v.cadre.as_ref().filter(|_| v.haut.z > 0.99).and_then(|c| ligne_de_coupe(c, v.droite, v.direction));
                let Some((debut, fin, profondeur, u)) = ligne else {
                    *ignorees.entry("Coupes et élévations inclinées ou sans cadrage".into()).or_default() += 1;
                    continue;
                };
                converties.insert(v.id, (id_vue(v.id), DVec3::new(debut.x, debut.y, 0.0), u));
                GenreVue::Coupe { debut: [debut.x, debut.y], fin: [fin.x, fin.y], profondeur, elevation: v.genre == "Élévation" }
            }
            autre => {
                *ignorees.entry(format!("Vues ({autre})")).or_default() += 1;
                continue;
            }
        };
        // nom unique parmi les vues natives
        let mut nom = v.nom.clone();
        let mut k = 2;
        while noms.contains(&nom) {
            nom = format!("{} ({k})", v.nom);
            k += 1;
        }
        noms.insert(nom.clone());
        c.projet.vues.push(VueNative { id: id_vue(v.id), nom, type_vue: nom_type(v.type_vue), echelle: if v.echelle > 0.0 { v.echelle } else { 100.0 }, genre });
    }
    if pivotes > 0 {
        *ignorees.entry("Orientation des plans pivotés (plans repris au nord du modèle)".into()).or_default() += pivotes;
    }
    // feuilles : repère du cartouche (coin bas gauche), vues placées converties
    let par_id: HashMap<i64, &crate::rvt::vues::Vue> = m.vues.iter().map(|v| (v.id, v)).collect();
    let mut feuilles: Vec<&crate::rvt::vues::Vue> = m.vues.iter().filter(|v| !v.gabarit && v.genre == "Feuille").collect();
    feuilles.sort_by(|a, b| a.numero.cmp(&b.numero).then(a.nom.cmp(&b.nom)));
    for f in feuilles {
        let e = crate::rvt::feuilles::etendue_cartouche(m, f).unwrap_or([0.0, 0.0, 841.0, 594.0]);
        let arrondi = |x: f64| (x * 10.0).round() / 10.0;
        let mut vues = Vec::new();
        for el in m.elements_2d.iter().filter(|el| el.vue == f.id) {
            let crate::rvt::vues::Contenu::FenetreDeVue { vue, decalage, echelle, .. } = &el.contenu else { continue };
            let (Some((id, o_n, u_n)), Some(pv)) = (converties.get(vue), par_id.get(vue)) else {
                *ignorees.entry("Vues placées sur des feuilles, non converties".into()).or_default() += 1;
                continue;
            };
            if *echelle <= 0.0 {
                continue;
            }
            let q = position_placee(*decalage, *echelle, pv.origine, pv.haut, *o_n, *u_n);
            let position = [arrondi(q[0] - e[0]), arrondi(q[1] - e[1])];
            vues.push(VuePlacee { vue: id.clone(), position, echelle: (1.0 / echelle).round().max(1.0) });
        }
        c.projet.feuilles.push(Feuille {
            id: id_feuille(f.id),
            numero: f.numero.clone(),
            nom: f.nom.clone(),
            jeu: JEU_DEFAUT.into(),
            largeur: arrondi(e[2] - e[0]).clamp(50.0, 5000.0),
            hauteur: arrondi(e[3] - e[1]).clamp(50.0, 5000.0),
            cartouche: String::new(),
            vues,
            nomenclatures: Vec::new(),
        });
    }
    for (k, n) in ignorees {
        *c.projet.ignores.entry(k).or_default() += n;
    }
}

#[cfg(test)]
mod tests_vues {
    use super::*;
    use crate::rvt::vues::Cadre;

    /// Coupe Revit regardant vers +Y (observateur au sud, droite = +X) : la ligne
    /// native va d'ouest en est sur le plan proche ; un point du modèle tombe au même
    /// endroit de la feuille avant et après conversion.
    #[test]
    fn coupe_et_position_sur_feuille() {
        let (droite, haut, direction) = (DVec3::X, DVec3::Z, DVec3::NEG_Y);
        // cadrage : origine (1000, 2000, 0), axes droite, haut, profondeur vers +Y
        let cadre = Cadre { origine: DVec3::new(1000.0, 2000.0, 0.0), axes: [DVec3::X, DVec3::Z, DVec3::Y], min: [-3000.0, -500.0, 0.0], max: [5000.0, 6000.0, 8000.0], actif: [true; 3], loin: true, annotation_active: false, annotation: [0.0; 4] };
        let (debut, fin, profondeur, u) = ligne_de_coupe(&cadre, droite, direction).unwrap();
        assert!((debut - DVec3::new(-2000.0, 2000.0, 0.0)).length() < 1e-9, "{debut}");
        assert!((fin - DVec3::new(6000.0, 2000.0, 0.0)).length() < 1e-9, "{fin}");
        assert_eq!((profondeur, u), (8000.0, DVec3::X));
        // regard natif : vers la gauche de la ligne, observateur du côté de `direction`
        assert!(u.cross(DVec3::Z).dot(direction) > 0.0);
        // feuille : même point du papier pour un point du modèle
        let (o, decalage, echelle) = (DVec3::new(1000.0, 2000.0, 1500.0), DVec3::new(300.0, 200.0, 0.0), 0.01);
        let o_n = DVec3::new(debut.x, debut.y, 0.0);
        let pos = position_placee(decalage, echelle, o, haut, o_n, u);
        for p in [DVec3::new(0.0, 2500.0, 0.0), DVec3::new(4321.0, 9000.0, 2800.0)] {
            let revit = [decalage.x + (p - o).dot(droite) * echelle, decalage.y + (p - o).dot(haut) * echelle];
            let natif = [pos[0] + (p - o_n).dot(u) * echelle, pos[1] + (p - o_n).dot(DVec3::Z) * echelle];
            assert!((revit[0] - natif[0]).abs() < 1e-9 && (revit[1] - natif[1]).abs() < 1e-9, "{revit:?} {natif:?}");
        }
        assert_eq!((categorie_native("Sols"), categorie_native("Murs"), categorie_native("Toits")), ("Dalle".into(), "Murs".into(), "Toiture".into()));
        // coupe regardant vers −Y : ligne retournée, toujours du bon côté
        let (_, _, _, u2) = ligne_de_coupe(&Cadre { axes: [DVec3::NEG_X, DVec3::Z, DVec3::NEG_Y], ..cadre }, DVec3::NEG_X, DVec3::Y).unwrap();
        assert!(u2.cross(DVec3::Z).dot(DVec3::Y) > 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Fichier du dossier du décodage (`RVT_ESSAIS`, par défaut
    /// `C:\Users\theba\Projets code\Decodage RVT`), comme les tests de `rvt::maquette` ;
    /// absent : test sauté.
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
        let Some(p) = fichier("RVT_ESSAIS", r"C:\Users\theba\Projets code\Decodage RVT", r"fichiers\Projet1.rvt") else { return };
        let projet = convertir_et_rapporter(&p);
        assert!(!projet.niveaux.is_empty());
        assert!(projet.murs.len() * 2 >= 59, "{} murs paramétriques sur 59", projet.murs.len());
    }

    /// Kerreut 2025.rvt (corpus de l'agence) : rapport seul.
    #[test]
    fn kerreut() {
        let Some(p) = fichier("RVT_ESSAIS", r"C:\Users\theba\Projets code\Decodage RVT", r"fichiers\Kerreut 2025.rvt") else { return };
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
