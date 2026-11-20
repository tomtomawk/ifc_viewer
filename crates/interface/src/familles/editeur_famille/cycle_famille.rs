//! Cycle de sélection de l'éditeur de familles : même mécanisme que les vues du projet
//! (`saisie/cycle.rs` : présélection en bleu au survol, Tab pour le suivant, clic pour
//! sélectionner). Candidats sous le curseur, dans l'ordre de priorité de l'éditeur : textes
//! et traits 2D, cotes, cotes de mesure, formes, volumes (pleins puis vides), plans.
use super::*;

/// Candidat : élément, nom affiché, traits à surligner.
pub(super) type CandidatFamille = (SelectionFamille, String, Vec<[egui::Pos2; 2]>);

/// Zones de sélection à l'écran construites par le canevas.
pub(super) struct Zones<'a> {
    pub textes_2d: &'a [(usize, egui::Rect)],
    pub lignes_2d: &'a [(usize, Vec<egui::Pos2>)],
    pub etiquettes: &'a [(usize, egui::Rect)],
    pub lignes_mesures: &'a [(usize, Vec<[egui::Pos2; 2]>)],
    pub contours: &'a [(usize, Vec<egui::Pos2>)],
    /// Volumes dans l'ordre de priorité (pleins de devant d'abord, vides ensuite).
    pub volumes: Vec<(usize, egui::Rect)>,
    /// Plans de référence : nom, vertical, coordonnée écran.
    pub plans: &'a [(String, bool, f32)],
    pub cadre: egui::Rect,
}

fn cote_rect(r: egui::Rect) -> Vec<[egui::Pos2; 2]> {
    vec![[r.left_top(), r.right_top()], [r.right_top(), r.right_bottom()], [r.right_bottom(), r.left_bottom()], [r.left_bottom(), r.left_top()]]
}

fn ferme(p: &[egui::Pos2]) -> Vec<[egui::Pos2; 2]> {
    (0..p.len()).map(|i| [p[i], p[(i + 1) % p.len()]]).collect()
}

/// Candidats sous le point écran `q`, du premier au dernier.
pub(super) fn candidats(ed: &EditeurFamille, z: &Zones, q: egui::Pos2) -> Vec<CandidatFamille> {
    let f = &ed.famille;
    let nom_2d = |k: usize| f.elements_2d.get(k).map_or("Élément 2D", |e| ancrage::genre_element_2d(&e.forme)).to_string();
    // (rang de la sorte, distance, candidat)
    let mut liste: Vec<(u8, f32, CandidatFamille)> = Vec::new();
    for (k, r) in z.textes_2d.iter().rev().filter(|(_, r)| r.contains(q)) {
        liste.push((0, 0.0, (SelectionFamille::Element2D(*k), nom_2d(*k), cote_rect(*r))));
    }
    for (k, l) in z.lignes_2d {
        let segments: Vec<[egui::Pos2; 2]> = l.windows(2).map(|w| [w[0], w[1]]).collect();
        let d = segments.iter().map(|[a, b]| panneau::distance_segment(q, *a, *b)).fold(f32::MAX, f32::min);
        if d < 6.0 {
            liste.push((0, d, (SelectionFamille::Element2D(*k), nom_2d(*k), segments)));
        }
    }
    for (k, r) in z.etiquettes.iter().filter(|(_, r)| r.contains(q)) {
        liste.push((1, 0.0, (SelectionFamille::Contrainte(*k), "Cote".into(), cote_rect(*r))));
    }
    for (k, l) in z.lignes_mesures {
        let d = l.iter().map(|[a, b]| panneau::distance_segment(q, *a, *b)).fold(f32::MAX, f32::min);
        if d < 6.0 {
            liste.push((2, d, (SelectionFamille::CoteMesure(*k), "Cote de mesure".into(), l.clone())));
        }
    }
    // un objet l'emporte sur les plans qui le traversent (les plans courent sur toute la vue)
    for (k, p) in z.contours.iter().rev().filter(|(_, p)| panneau::dans_polygone(q, p)) {
        let nom = f.extrusions.get(*k).map_or_else(|| "Forme".to_string(), |e| format!("Forme : {}", e.nom));
        liste.push((3, 0.0, (SelectionFamille::Extrusion(*k), nom, ferme(p))));
    }
    for (k, r) in z.volumes.iter().filter(|(_, r)| r.contains(q)) {
        let nom = f.volumes.get(*k).map_or_else(|| "Volume".to_string(), |v| format!("Volume : {}", v.nom));
        liste.push((4, 0.0, (SelectionFamille::Volume(*k), nom, cote_rect(*r))));
    }
    for (n, vertical, c) in z.plans {
        let d = if *vertical { (q.x - c).abs() } else { (q.y - c).abs() };
        if d < 7.0 {
            let trait_ = if *vertical { [egui::pos2(*c, z.cadre.top()), egui::pos2(*c, z.cadre.bottom())] } else { [egui::pos2(z.cadre.left(), *c), egui::pos2(z.cadre.right(), *c)] };
            liste.push((5, d, (SelectionFamille::Plan(n.clone()), format!("Plan : {n}"), vec![trait_])));
        }
    }
    liste.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut vus = Vec::new();
    liste.into_iter().filter(|(_, _, c)| !vus.contains(&c.0) && { vus.push(c.0.clone()); true }).map(|(_, _, c)| c).collect()
}

/// Présélection au survol et Tab (outil Sélection) ; dessine le candidat courant.
pub(super) fn survol(ui: &mut egui::Ui, ed: &mut EditeurFamille, resp: &egui::Response, painter: &egui::Painter, z: &Zones, contexte: u64) {
    let appui = ui.input(|i| i.pointer.any_down());
    match resp.hover_pos().filter(|_| ed.outil == OutilFamille::Selection && !appui && ed.fenetre.is_none()) {
        Some(q) => {
            let mut cycle = ed.cycle.take();
            crate::saisie::cycle::actualiser(&mut cycle, q, contexte, || candidats(ed, z, q));
            ed.cycle = cycle;
        }
        None if !appui => ed.cycle = None,
        None => {}
    }
    if resp.hovered() && ed.cycle.as_ref().is_some_and(|c| c.candidats.len() > 1) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
        && let Some(c) = ed.cycle.as_mut() { c.suivant(); }
    if let (Some(c), Some(q)) = (&ed.cycle, resp.hover_pos()) && let Some((_, libelle, segments)) = c.courant() {
        crate::saisie::cycle::dessiner(painter, q, segments, c.indice, c.candidats.len(), libelle);
    }
}

/// Élément choisi par un clic en `q` : le candidat présélectionné, sinon le premier.
pub(super) fn au_clic(ed: &EditeurFamille, z: &Zones, q: egui::Pos2) -> SelectionFamille {
    crate::saisie::cycle::au_clic(&ed.cycle, q).map(|c| c.0).or_else(|| candidats(ed, z, q).into_iter().next().map(|c| c.0)).unwrap_or(SelectionFamille::Aucune)
}
