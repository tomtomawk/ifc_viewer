//! Segments d'arc des tracés en cours : courbure de chaque segment des contours de
//! `dalle` et des contours laissés ouverts, tenue à jour quand leurs sommets changent.
//!
//! `courbures[c][i]` est la courbure du segment partant du sommet `i` du contour `c`
//! (0 ou absent : droit) ; l'entrée du dernier sommet d'un contour ouvert ne sert pas.
use super::*;

/// Courbures d'un contour ouvert de `sommets` sommets parcouru dans l'autre sens.
pub(super) fn inverser_courbures(k: &[f64], sommets: usize) -> Vec<f64> {
    let mut r: Vec<f64> = (0..sommets.saturating_sub(1)).rev().map(|i| -k.get(i).copied().unwrap_or(0.0)).collect();
    r.resize(sommets, 0.0);
    r
}

/// Courbures d'un contour ouvert de `sommets` sommets prolongé par un tracé de
/// `trace.len() + 1` points parti de son premier ou de son dernier sommet.
pub(super) fn raccorder_courbures(k: &[f64], sommets: usize, premier: bool, trace: &[f64]) -> Vec<f64> {
    let mut k: Vec<f64> = k.to_vec();
    k.resize(sommets, 0.0);
    if premier {
        let mut r: Vec<f64> = trace.iter().rev().map(|x| -x).collect();
        r.extend(k);
        r
    } else {
        k.truncate(sommets.saturating_sub(1));
        k.extend(trace.iter().copied());
        k.push(0.0);
        k
    }
}

/// Courbures d'un contour fermé par la réunion de ses extrémités : le sommet `sommet`
/// (premier ou dernier), doublon de l'autre extrémité, est retiré.
pub(super) fn fermer_courbures(k: &[f64], sommets: usize, sommet: usize) -> Vec<f64> {
    let mut k: Vec<f64> = k.to_vec();
    k.resize(sommets, 0.0);
    if sommet == 0 {
        // le premier segment devient celui de fermeture, depuis le dernier sommet
        let premier = k.remove(0);
        if let Some(dernier) = k.last_mut() { *dernier = premier; }
    } else {
        k.pop();
    }
    k
}

/// Courbures du contour `c`, une par sommet.
pub(super) fn du_contour(contours: &[Vec<[f64; 2]>], courbures: &[Vec<f64>], c: usize) -> Vec<f64> {
    let n = contours.get(c).map_or(0, Vec::len);
    (0..n).map(|i| courbures.get(c).and_then(|k| k.get(i)).copied().unwrap_or(0.0)).collect()
}

/// Remplace les courbures du contour `c`.
pub(super) fn definir(courbures: &mut Vec<Vec<f64>>, c: usize, k: Vec<f64>) {
    if courbures.len() <= c {
        courbures.resize(c + 1, Vec::new());
    }
    courbures[c] = k;
}

/// Retire les courbures du contour `c` (contour supprimé).
pub(super) fn retirer(courbures: &mut Vec<Vec<f64>>, c: usize) {
    if c < courbures.len() {
        courbures.remove(c);
    }
}

/// Insère les courbures d'un contour ajouté au rang `c`.
pub(super) fn inserer(courbures: &mut Vec<Vec<f64>>, c: usize, k: Vec<f64>) {
    if courbures.len() < c {
        courbures.resize(c, Vec::new());
    }
    courbures.insert(c, k);
}

impl EtatDessin2D {
    /// Courbure du segment partant du sommet `i` du contour `c` (0 : droit).
    pub(super) fn courbure(&self, c: usize, i: usize) -> f64 {
        self.courbures.get(c).and_then(|k| k.get(i)).copied().unwrap_or(0.0)
    }

    /// Courbures du contour `c`, une par sommet.
    pub(super) fn courbures_contour(&self, c: usize) -> Vec<f64> {
        du_contour(&self.dalle, &self.courbures, c)
    }

    /// Remplace les courbures du contour `c`.
    pub(super) fn definir_courbures(&mut self, c: usize, k: Vec<f64>) {
        definir(&mut self.courbures, c, k);
    }

    /// Retire les courbures du contour `c` (contour supprimé).
    pub(super) fn retirer_courbures(&mut self, c: usize) {
        retirer(&mut self.courbures, c);
    }

    /// Ajoute un contour et les courbures de ses segments.
    pub(super) fn pousser_contour(&mut self, points: Vec<[f64; 2]>, k: Vec<f64>) {
        self.courbures.resize(self.dalle.len(), Vec::new());
        self.dalle.push(points);
        self.courbures.push(k);
    }

    /// Ajoute un contour laissé ouvert et les courbures de ses segments.
    pub(super) fn pousser_ouvert(&mut self, points: Vec<DVec2>, k: Vec<f64>) {
        self.ouverts_courbures.resize(self.ouverts.len(), Vec::new());
        self.ouverts.push(points);
        self.ouverts_courbures.push(k);
    }

    /// Reprend un contour laissé ouvert : ses points et ses courbures.
    pub(super) fn retirer_ouvert(&mut self, i: usize) -> (Vec<DVec2>, Vec<f64>) {
        let points = self.ouverts.remove(i);
        let mut k = if i < self.ouverts_courbures.len() { self.ouverts_courbures.remove(i) } else { Vec::new() };
        k.resize(points.len(), 0.0);
        (points, k)
    }

    /// Oublie tous les contours en cours.
    pub(super) fn vider_contours(&mut self) {
        self.dalle.clear();
        self.courbures.clear();
        self.ouverts.clear();
        self.ouverts_courbures.clear();
    }

    /// Segments du contour `c` : extrémités et courbure.
    pub(super) fn segments_contour(&self, c: usize, fermee: bool) -> Vec<(DVec2, DVec2, f64)> {
        let points: Vec<DVec2> = self.dalle.get(c).map(|x| x.iter().map(|p| DVec2::from_array(*p)).collect()).unwrap_or_default();
        crate::noyau::forme2d::segments_polyligne(&points, fermee, &self.courbures_contour(c))
    }

    /// Courbures du contour `c` à enregistrer : une par segment, vide si tous sont droits.
    pub(super) fn courbures_enregistrees(&self, c: usize, fermee: bool) -> Vec<f64> {
        let k: Vec<f64> = self.segments_contour(c, fermee).into_iter().map(|s| s.2).collect();
        if k.iter().all(|x| *x == 0.0) { Vec::new() } else { k }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inversion_raccord_et_fermeture_gardent_les_arcs_sur_leurs_segments() {
        // trois sommets : arc sur le premier segment
        assert_eq!(inverser_courbures(&[0.5, 0.0, 0.0], 3), vec![-0.0, -0.5, 0.0]);
        assert_eq!(inverser_courbures(&[], 2), vec![-0.0, 0.0]);
        // prolongé par la fin : l'arc du tracé suit le dernier sommet
        assert_eq!(raccorder_courbures(&[0.5, 0.0, 0.0], 3, false, &[0.0, 0.25]), vec![0.5, 0.0, 0.0, 0.25, 0.0]);
        // prolongé par le début : le tracé est parcouru à l'envers
        assert_eq!(raccorder_courbures(&[0.5, 0.0, 0.0], 3, true, &[0.0, 0.25]), vec![-0.25, -0.0, 0.5, 0.0, 0.0]);
        // fermeture par le dernier sommet (doublon du premier) : rien ne bouge
        assert_eq!(fermer_courbures(&[0.1, 0.2, 0.3, 0.0], 4, 3), vec![0.1, 0.2, 0.3]);
        // par le premier : son segment devient le segment de fermeture
        assert_eq!(fermer_courbures(&[0.1, 0.2, 0.3, 0.0], 4, 0), vec![0.2, 0.3, 0.1]);
    }

    #[test]
    fn etat_aligne_les_courbures_sur_les_contours() {
        let mut d = EtatDessin2D::default();
        d.dalle.push(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
        d.pousser_contour(vec![[0.0, 0.0], [20.0, 0.0]], vec![1.0, 0.0]);
        assert_eq!(d.courbure(0, 0), 0.0);
        assert_eq!(d.courbure(1, 0), 1.0);

        assert_eq!(d.courbures_enregistrees(1, false), vec![1.0]);
        assert!(d.courbures_enregistrees(0, true).is_empty());
        d.dalle.remove(0);
        d.retirer_courbures(0);
        assert_eq!(d.courbure(0, 0), 1.0);
        inserer(&mut d.courbures, 3, vec![0.25]);
        assert_eq!(d.courbures.len(), 4);
        d.courbures.truncate(1);
        d.pousser_ouvert(vec![DVec2::ZERO, DVec2::X], vec![0.5]);
        assert_eq!(d.retirer_ouvert(0), (vec![DVec2::ZERO, DVec2::X], vec![0.5, 0.0]));
        d.vider_contours();
        assert!(d.dalle.is_empty() && d.courbures.is_empty());
    }
}
