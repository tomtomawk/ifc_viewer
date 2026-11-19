//! Cycle de sélection au survol, commun au projet et à l'éditeur de familles, comme dans
//! Revit : les éléments sous le curseur (à quelques pixels près) sont rangés du plus
//! proche au plus lointain ; le premier est présélectionné (surligné en bleu), Tab passe
//! au suivant, le clic sélectionne celui qui est surligné. Bouger la souris de plus de
//! `RAYON` pixels recommence la liste.
use crate::*;

/// Déplacement de la souris (pixels) au-delà duquel la liste est recalculée.
pub(crate) const RAYON: f32 = 3.0;
/// Couleur de la présélection.
pub(crate) const SURVOL: Color32 = Color32::from_rgb(40, 115, 240);

/// Candidats sous le curseur et rang du candidat présélectionné.
#[derive(Clone)]
pub(crate) struct Cycle<T> {
    pub position: egui::Pos2,
    /// Contexte du calcul (caméra, révision du modèle…) : la liste est recalculée s'il change.
    pub contexte: u64,
    pub candidats: Vec<T>,
    pub indice: usize,
}

impl<T> Cycle<T> {
    pub(crate) fn courant(&self) -> Option<&T> {
        self.candidats.get(self.indice)
    }

    /// Tab : candidat suivant (retour au premier après le dernier).
    pub(crate) fn suivant(&mut self) {
        if !self.candidats.is_empty() {
            self.indice = (self.indice + 1) % self.candidats.len();
        }
    }

    fn valide(&self, q: egui::Pos2, contexte: u64) -> bool {
        self.contexte == contexte && self.position.distance(q) <= RAYON
    }
}

/// Garde le cycle si la souris n'a pas bougé (ni le contexte), sinon le recalcule.
pub(crate) fn actualiser<T>(cycle: &mut Option<Cycle<T>>, q: egui::Pos2, contexte: u64, candidats: impl FnOnce() -> Vec<T>) {
    if cycle.as_ref().is_some_and(|c| c.valide(q, contexte)) {
        return;
    }
    let candidats = candidats();
    *cycle = (!candidats.is_empty()).then_some(Cycle { position: q, contexte, candidats, indice: 0 });
}

/// Candidat présélectionné si le clic `q` tombe là où le cycle a été calculé.
pub(crate) fn au_clic<T: Clone>(cycle: &Option<Cycle<T>>, q: egui::Pos2) -> Option<T> {
    cycle.as_ref().filter(|c| c.position.distance(q) <= RAYON).and_then(|c| c.courant().cloned())
}

/// Traits du candidat présélectionné, et bulle « nom · 2/3 · Tab » s'il y en a plusieurs.
pub(crate) fn dessiner(painter: &egui::Painter, q: egui::Pos2, segments: &[[egui::Pos2; 2]], indice: usize, nombre: usize, libelle: &str) {
    for [a, b] in segments {
        painter.line_segment([*a, *b], egui::Stroke::new(3.0, SURVOL));
    }
    // un seul candidat : surlignage seul
    if nombre < 2 {
        return;
    }
    let texte = format!("{libelle} · {}/{nombre} · Tab", indice + 1);
    let police = egui::FontId::proportional(11.0);
    let galerie = painter.layout_no_wrap(texte, police, Color32::WHITE);
    let r = egui::Rect::from_min_size(q + egui::vec2(14.0, 16.0), galerie.size()).expand(3.0);
    painter.rect_filled(r, 3.0, SURVOL);
    painter.galley(r.min + egui::vec2(3.0, 3.0), galerie, Color32::WHITE);
}

/// Empreinte d'un contexte de calcul.
pub(crate) fn empreinte(valeurs: &[u64]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    valeurs.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_fait_tourner_et_bouger_recommence() {
        let mut c: Option<Cycle<&str>> = None;
        let q = egui::pos2(100.0, 100.0);
        actualiser(&mut c, q, 1, || vec!["mur", "dalle", "polyligne"]);
        assert_eq!(c.as_ref().unwrap().courant(), Some(&"mur"));
        c.as_mut().unwrap().suivant();
        // petit tremblement : la liste et le rang restent
        actualiser(&mut c, q + egui::vec2(1.0, 1.0), 1, || vec!["autre"]);
        assert_eq!(au_clic(&c, q + egui::vec2(2.0, 0.0)), Some("dalle"));
        c.as_mut().unwrap().suivant();
        c.as_mut().unwrap().suivant();
        assert_eq!(c.as_ref().unwrap().courant(), Some(&"mur"));
        // souris déplacée, ou caméra changée : liste recalculée, premier candidat
        actualiser(&mut c, q + egui::vec2(10.0, 0.0), 1, || vec!["poteau"]);
        assert_eq!(c.as_ref().unwrap().courant(), Some(&"poteau"));
        actualiser(&mut c, q + egui::vec2(10.0, 0.0), 2, Vec::new);
        assert!(c.is_none());
        // clic ailleurs : pas de candidat présélectionné
        actualiser(&mut c, q, 1, || vec!["mur"]);
        assert_eq!(au_clic(&c, q + egui::vec2(20.0, 0.0)), None);
    }
}
