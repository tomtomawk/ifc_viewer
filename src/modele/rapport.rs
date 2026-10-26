//! Rapport d'une conversion : part des éléments devenus paramétriques, raisons des
//! imports, écart de volume entre la géométrie lue et celle que donnent les paramètres.

use super::{GenreOuverture, Mesure, Projet, ECART_MAX};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Rapport en Markdown.
pub fn markdown(p: &Projet, mesures: &[Mesure], duree_s: f64) -> String {
    let mut s = String::new();
    let src = p.sources.first();
    let _ = writeln!(s, "# Conversion vers le modèle v0 : {}\n", src.map_or("?", |x| x.fichier.as_str()));
    if let Some(x) = src {
        let _ = writeln!(s, "Source : {} ({:.1} Mo), convertie en {duree_s:.1} s.\n", x.format, x.octets as f64 / 1e6);
    }

    // par catégorie : paramétriques et importés
    let mut cat: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    cat.entry("Niveaux".into()).or_default().0 += p.niveaux.len();
    // un assemblage compte pour un mur ; ses couches sont détaillées dans le résumé
    let murs = p.murs.iter().filter(|m| m.assemblage.is_none()).count() + p.assemblages.len();
    let couches = p.murs.len() - p.murs.iter().filter(|m| m.assemblage.is_none()).count();
    let non_eclates = p.murs.iter().filter(|m| m.assemblage.is_none() && p.type_mur(&m.type_mur).is_some_and(|t| t.couches.len() > 1)).count();
    cat.entry("Murs".into()).or_default().0 += murs;
    cat.entry("Dalles".into()).or_default().0 += p.dalles.len();
    for o in &p.ouvertures {
        let c = match o.genre {
            GenreOuverture::Porte => "Portes",
            GenreOuverture::Fenetre => "Fenêtres",
            GenreOuverture::Baie => "Baies",
        };
        cat.entry(c.into()).or_default().0 += 1;
    }
    for i in &p.importes {
        cat.entry(i.categorie.clone()).or_default().1 += 1;
    }
    let parametriques = murs + p.dalles.len() + p.ouvertures.len();
    let total = parametriques + p.importes.len();
    let taux = |a: usize, b: usize| if a + b == 0 { "—".to_string() } else { format!("{:.0} %", 100.0 * a as f64 / (a + b) as f64) };
    s.push_str("## Résumé\n\n| | Nombre |\n|---|---|\n");
    let _ = writeln!(s, "| Niveaux | {} |", p.niveaux.len());
    let _ = writeln!(s, "| Murs paramétriques | {murs} |");
    let _ = writeln!(s, "| dont murs composés (assemblages : un mur par couche) | {} ({couches} couches) |", p.assemblages.len());
    let _ = writeln!(s, "| Murs à couches gardés d'un bloc (côté extérieur non vérifiable) | {non_eclates} |");
    let _ = writeln!(s, "| Ouvertures (portes, fenêtres, baies) | {} |", p.ouvertures.len());
    let _ = writeln!(s, "| Dalles paramétriques | {} |", p.dalles.len());
    let _ = writeln!(s, "| Éléments importés (géométrie figée) | {} |", p.importes.len());
    let _ = writeln!(s, "| **Part paramétrique** (murs, ouvertures, dalles / tous les éléments à géométrie) | **{}** |\n", taux(parametriques, total - parametriques));

    s.push_str("## Par catégorie\n\n| Catégorie | Paramétriques | Importés | Part paramétrique |\n|---|---:|---:|---:|\n");
    for (c, (a, b)) in &cat {
        if a + b > 0 {
            let _ = writeln!(s, "| {c} | {a} | {b} | {} |", taux(*a, *b));
        }
    }

    // écarts de volume
    s.push_str("\n## Écart de volume : paramètres − géométrie lue\n\n");
    let _ = writeln!(
        s,
        "Écart relatif = (volume déduit des paramètres − volume du maillage lu) / volume lu. Un élément reconnu dont l'écart dépasse {:.0} % reste un élément importé.\n",
        100.0 * ECART_MAX
    );
    s.push_str("| Catégorie | Candidats | Retenus | |écart| ≤ 1 % | ≤ 5 % | > 5 % | Médiane |écart| | Volume lu (m³) | Volume paramètres (m³) | Écart global |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for categorie in ["Murs", "Dalles"] {
        let m: Vec<&Mesure> = mesures.iter().filter(|x| x.categorie == categorie).collect();
        if m.is_empty() {
            let _ = writeln!(s, "| {categorie} | 0 | | | | | | | | |");
            continue;
        }
        let mut e: Vec<f64> = m.iter().map(|x| x.ecart().abs()).collect();
        e.sort_by(f64::total_cmp);
        let compte = |seuil: f64| e.iter().filter(|x| **x <= seuil).count();
        let (vs, vp): (f64, f64) = m.iter().fold((0.0, 0.0), |(a, b), x| (a + x.source, b + x.parametres));
        let _ = writeln!(
            s,
            "| {categorie} | {} | {} | {} | {} | {} | {:.2} % | {:.3} | {:.3} | {:+.2} % |",
            m.len(),
            m.iter().filter(|x| x.retenu).count(),
            compte(0.01),
            compte(0.05),
            e.len() - compte(0.05),
            100.0 * e[e.len() / 2],
            vs / 1e9,
            vp / 1e9,
            if vs > 0.0 { 100.0 * (vp - vs) / vs } else { 0.0 }
        );
    }
    let mut pires: Vec<&Mesure> = mesures.iter().collect();
    pires.sort_by(|a, b| b.ecart().abs().total_cmp(&a.ecart().abs()));
    if !pires.is_empty() {
        s.push_str("\n### Écarts les plus forts\n\n| Catégorie | Élément | Lu (m³) | Paramètres (m³) | Écart | Retenu |\n|---|---|---:|---:|---:|---|\n");
        for x in pires.iter().take(25) {
            let _ = writeln!(s, "| {} | {} | {:.4} | {:.4} | {:+.1} % | {} |", x.categorie, x.nom, x.source / 1e9, x.parametres / 1e9, 100.0 * x.ecart(), if x.retenu { "oui" } else { "non" });
        }
    }

    // raisons des imports
    let mut raisons: BTreeMap<(String, String), usize> = BTreeMap::new();
    for i in &p.importes {
        let r = if i.raison.starts_with("écart de volume") { "écart de volume au-delà de la tolérance".to_string() } else { i.raison.clone() };
        *raisons.entry((i.categorie.clone(), r)).or_default() += 1;
    }
    if !raisons.is_empty() {
        s.push_str("\n## Raisons des imports\n\n| Catégorie | Raison | Nombre |\n|---|---|---:|\n");
        for ((c, r), n) in &raisons {
            let _ = writeln!(s, "| {c} | {r} | {n} |");
        }
    }
    if !p.ignores.is_empty() {
        s.push_str("\n## Non convertis\n\n| Élément | Nombre |\n|---|---:|\n");
        for (c, n) in &p.ignores {
            let _ = writeln!(s, "| {c} | {n} |");
        }
    }
    s.push_str("\n## Hypothèses de la v0\n\n");
    if src.is_some_and(|x| x.format == "RVT") {
        s.push_str(
            "- Murs : le tracé lu donne la droite ou le cercle porteur ; extrémités et ligne médiane viennent de la géométrie du mur projetée sur ce tracé. Aux angles, la géométrie lue inclut la jonction : l'axe s'étend jusqu'au bout de la jonction. Couches de la structure composée conservées dans l’ordre du type avec leurs matériaux et épaisseurs ; une couche globale non détaillée uniquement si la composition est indisponible.\n\
- Ouvertures : baie rectangulaire à travers tout le mur, aux cotes du type (largeur, hauteur de la famille), centrée sur le point d'insertion ; ces cotes peuvent différer de la réservation réellement découpée dans le mur.\n",
        );
    } else {
        s.push_str(
            "- Murs : direction et extrémités de la représentation Axis, sinon de la géométrie ; ligne médiane, base et hauteur de la géométrie. Couches du jeu de couches de matériaux, dans l'ordre du fichier.\n\
- Ouvertures : baie = encombrement de l'IfcOpeningElement borné au mur.\n",
        );
    }
    s.push_str("- Dalles : prismes verticaux seulement ; épaisseur du type.\n- Volume déduit des paramètres : longueur d'axe × épaisseur × hauteur − baies ; jonctions non modélisées. Un arc est exact dans les paramètres et facetté dans la géométrie lue (écart de l'ordre du pour cent).\n");
    s
}
