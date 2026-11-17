//! Éléments 2D ancrés aux plans de référence dans l'éditeur de famille : création par
//! l'outil Polyligne, poignées de sommets, insertion et retrait de sommets, translation.
use super::*;
use modele::documentation::AnnotationForme;
use modele::famille::modelee::Element2DFamille;

/// Nom du genre d'un élément 2D dans les listes de l'éditeur.
pub(super) fn genre_element_2d(forme: &AnnotationForme) -> &'static str {
    match forme {
        AnnotationForme::Polyligne { fermee: true, .. } => "Polyligne fermée",
        AnnotationForme::Polyligne { .. } => "Polyligne",
        AnnotationForme::Arc { .. } => "Arc",
        AnnotationForme::Cercle { .. } => "Cercle",
        AnnotationForme::Spline { .. } => "Spline",
        AnnotationForme::Ligne { .. } => "Ligne",
        AnnotationForme::Texte { .. } => "Texte",
        AnnotationForme::Etiquette { .. } => "Libellé",
        _ => "Élément",
    }
}

impl EditeurFamille {
    /// Ajoute une polyligne ancrée dans la vue active et la sélectionne.
    pub(super) fn ajouter_polyligne_ancree(&mut self, points: Vec<PointEsquisse>, fermee: bool) {
        let element = Element2DFamille::polyligne(self.vue.esquisse(), points, fermee);
        self.famille.elements_2d.push(element);
        self.selection = SelectionFamille::Element2D(self.famille.elements_2d.len() - 1);
    }

    /// Élément 2D ancré `k` de la vue active.
    pub(super) fn element_ancre(&self, k: usize) -> Option<&Element2DFamille> {
        self.famille.elements_2d.get(k).filter(|e| e.est_ancre() && e.vue == self.vue.esquisse())
    }

    /// Poignées d'un élément ancré : (indice du sommet, h, v, milieu d'arête à insérer).
    pub(super) fn poignees_ancrees(&self, r: &Resolution, k: usize) -> Vec<(usize, (f64, f64), bool)> {
        let Some(e) = self.element_ancre(k) else { return Vec::new() };
        let pts: Vec<(f64, f64)> = e.ancrage.iter().map(|p| self.famille.point_esquisse(r, p)).collect();
        let fermee = matches!(&e.forme, AnnotationForme::Polyligne { fermee: true, .. } | AnnotationForme::Spline { fermee: true, .. });
        let insertion = matches!(&e.forme, AnnotationForme::Polyligne { .. } | AnnotationForme::Spline { .. });
        let mut out = Vec::new();
        for (i, &q) in pts.iter().enumerate() {
            out.push((i, q, false));
            if insertion && (i + 1 < pts.len() || fermee) {
                let b = pts[(i + 1) % pts.len()];
                out.push((i + 1, ((q.0 + b.0) / 2.0, (q.1 + b.1) / 2.0), true));
            }
        }
        out
    }

    pub(super) fn inserer_sommet_ancre(&mut self, k: usize, i: usize) {
        if let Some(e) = self.famille.elements_2d.get_mut(k) { inserer_sommet(e, i); }
    }

    pub(super) fn supprimer_sommet_ancre(&mut self, k: usize, i: usize) -> bool {
        self.famille.elements_2d.get_mut(k).is_some_and(|e| supprimer_sommet(e, i))
    }

    /// Translate un élément 2D : décalages de l'ancrage, ou coordonnées fixes.
    pub(super) fn translater_element_2d(&mut self, k: usize, dh: f64, dv: f64) {
        let e = &mut self.famille.elements_2d[k];
        if e.est_ancre() {
            for p in &mut e.ancrage { p.h.decalage += dh; p.v.decalage += dv; }
        } else {
            for p in crate::outils::dessiner::dessin_plan::points_forme_mut(&mut e.forme) { p[0] += dh; p[1] += dv; }
        }
    }
}

/// Insère un sommet ancré (copie du précédent) d'une polyligne ou d'une spline.
fn inserer_sommet(e: &mut Element2DFamille, i: usize) {
    if i == 0 || i > e.ancrage.len() { return; }
    let (points, courbures, traits) = match &mut e.forme {
        AnnotationForme::Polyligne { points, courbures, traits, .. } => (points, Some(courbures), Some(traits)),
        AnnotationForme::Spline { points, .. } => (points, None, None),
        _ => return,
    };
    e.ancrage.insert(i, e.ancrage[i - 1].clone());
    points.insert(i, points[i - 1]);
    if let Some(c) = courbures.filter(|c| i - 1 < c.len()) { c.insert(i - 1, 0.0); }
    if let Some(t) = traits.filter(|t| i - 1 < t.len()) { let x = t[i - 1].clone(); t.insert(i - 1, x); }
}

/// Retire un sommet ancré ; faux si le nombre minimal est atteint (polyligne ou spline
/// ouverte : deux ; fermée : trois ; un arc garde ses trois points).
fn supprimer_sommet(e: &mut Element2DFamille, i: usize) -> bool {
    let (points, minimum, courbures, traits) = match &mut e.forme {
        AnnotationForme::Polyligne { points, fermee, courbures, traits, .. } => { let m = if *fermee { 3 } else { 2 }; (points, m, Some(courbures), Some(traits)) }
        AnnotationForme::Spline { points, fermee } => { let m = if *fermee { 3 } else { 2 }; (points, m, None, None) }
        _ => return false,
    };
    if i >= e.ancrage.len() || e.ancrage.len() <= minimum { return false; }
    e.ancrage.remove(i);
    points.remove(i);
    let j = i.min(points.len());
    if let Some(c) = courbures.filter(|c| j < c.len()) { c.remove(j); }
    if let Some(t) = traits.filter(|t| j < t.len()) { t.remove(j); }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sommets_ancres_inseres_et_retires() {
        let mut f = FamilleModelee::gabarit_porte();
        let k = f.elements_2d.iter().position(|e| e.vue == VueEsquisse::Plan && matches!(e.forme, AnnotationForme::Polyligne { .. })).unwrap();
        let e = &mut f.elements_2d[k];
        inserer_sommet(e, 1);
        assert_eq!(e.ancrage.len(), 3);
        assert!(supprimer_sommet(e, 1));
        assert!(!supprimer_sommet(e, 0));
        f.valider(900.0, 2150.0, 200.0).unwrap();
    }
}
