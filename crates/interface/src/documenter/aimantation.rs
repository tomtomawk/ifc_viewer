//! Aimantation d'une vue placée qu'on déplace sur une feuille : ses bords (et son
//! milieu) se calent sur les bords et les angles des cadres voisins — papier, cadre de
//! la feuille, cartouche, autres vues.

/// Décalage (mm du papier) qui cale `boite` [x0, y0, x1, y1] sur la cible la plus proche
/// à moins de `tol`, axe par axe ; un coin s'obtient en calant les deux axes.
pub(crate) fn aimanter(boite: [f64; 4], cibles_x: &[f64], cibles_y: &[f64], tol: f64) -> [f64; 2] {
    let axe = |a: f64, b: f64, cibles: &[f64]| -> f64 {
        [a, b, (a + b) * 0.5].into_iter()
            .flat_map(|bord| cibles.iter().map(move |c| c - bord))
            .filter(|d| d.abs() < tol)
            .min_by(|x, y| x.abs().total_cmp(&y.abs()))
            .unwrap_or(0.0)
    };
    [axe(boite[0], boite[2], cibles_x), axe(boite[1], boite[3], cibles_y)]
}

/// Bords d'une feuille de `largeur` × `hauteur` mm : papier et cadre intérieur.
pub(crate) fn bords_feuille(largeur: f64, hauteur: f64, marge: f64) -> (Vec<f64>, Vec<f64>) {
    (vec![0.0, marge, largeur - marge, largeur], vec![0.0, marge, hauteur - marge, hauteur])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// N° 1 : une vue lâchée près du coin du cadre s'y cale ; loin de tout bord, elle ne
    /// bouge pas.
    #[test]
    fn vue_calee_sur_le_coin_du_cadre() {
        let (xs, ys) = bords_feuille(420.0, 297.0, 10.0);
        assert_eq!(aimanter([12.5, 8.0, 112.5, 108.0], &xs, &ys, 4.0), [-2.5, 2.0]);
        assert_eq!(aimanter([100.0, 100.0, 200.0, 150.0], &xs, &ys, 4.0), [0.0, 0.0]);
        // bord droit calé sur le cadre, à 1 mm près
        assert_eq!(aimanter([300.0, 100.0, 411.0, 150.0], &xs, &ys, 4.0), [-1.0, 0.0]);
    }
}
