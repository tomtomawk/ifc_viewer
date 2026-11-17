//! Bornes des valeurs lues dans un fichier IFC, appliquées avant tout calcul dont la
//! durée ou la mémoire en dépend : un fichier malformé donne une entité écartée, pas un
//! gel ni un épuisement de mémoire.
use std::f64::consts::TAU;

/// Degré maximal accepté pour une B-spline (IFC n'en fixe pas ; les modeleurs restent
/// très en dessous).
const DEGRE_MAX: usize = 32;
/// Nombre de parties d'un produit au-delà duquel les items mappés ne sont plus suivis.
pub(super) const PARTIES_MAX: usize = 50_000;
/// Nombre de triangles d'un solide au-delà duquel les items mappés ne sont plus suivis.
pub(super) const TRIANGLES_MAX: usize = 5_000_000;

/// Fin d'un arc partant de `t1` (radians) : `t2` ramené à un tour au plus de `t1`, du
/// côté du sens de parcours. None : paramètre non fini.
pub(super) fn fin_arc(t1: f64, t2: f64, direct: bool) -> Option<f64> {
    if !t1.is_finite() || !t2.is_finite() {
        return None;
    }
    let mut d = if direct { t2 - t1 } else { t1 - t2 };
    if d <= 1e-9 || d > TAU + 1e-9 {
        d = d.rem_euclid(TAU);
        if d <= 1e-9 {
            d += TAU;
        }
    }
    Some(if direct { t1 + d } else { t1 - d })
}

/// Vecteur de nœuds développé d'une B-spline de `n` points de contrôle. None : degré,
/// multiplicité ou nombre de nœuds incohérent (la courbe est alors rendue par son
/// polygone de contrôle).
pub(super) fn noeuds(valeurs: &[f64], multiplicites: &[i64], degre: i64, n: usize) -> Option<(usize, Vec<f64>)> {
    let degre = usize::try_from(degre).ok().filter(|d| (1..=DEGRE_MAX).contains(d) && *d < n)?;
    let attendu = n + degre + 1;
    let mut noeuds = Vec::with_capacity(attendu);
    for (k, m) in valeurs.iter().zip(multiplicites) {
        let m = usize::try_from(*m).ok().filter(|m| (1..=degre + 1).contains(m))?;
        if !k.is_finite() || noeuds.len() + m > attendu {
            return None;
        }
        noeuds.extend(std::iter::repeat_n(*k, m));
    }
    (noeuds.len() == attendu).then_some((degre, noeuds))
}

/// Angle de révolution borné à un tour. None : angle non fini.
pub(super) fn angle_revolution(angle: f64) -> Option<f64> {
    angle.is_finite().then(|| angle.clamp(-TAU, TAU))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_borne_a_un_tour_et_parametres_non_finis_refuses() {
        // cas courants inchangés
        assert_eq!(fin_arc(0.0, 1.0, true), Some(1.0));
        assert!((fin_arc(1.0, 0.0, true).unwrap() - TAU).abs() < 1e-12);
        assert!((fin_arc(0.0, 1.0, false).unwrap() - (1.0 - TAU)).abs() < 1e-12);
        assert!((fin_arc(0.5, 0.5, true).unwrap() - (0.5 + TAU)).abs() < 1e-12);
        // paramètres démesurés : un tour au plus, sans boucle
        let t = fin_arc(1e15, 0.0, true).unwrap();
        assert!(t > 1e15 && t - 1e15 <= TAU + 1e-6);
        let t = fin_arc(0.0, 1e15, true).unwrap();
        assert!(t > 0.0 && t <= TAU + 1e-6);
        assert_eq!(fin_arc(f64::INFINITY, 0.0, true), None);
        assert_eq!(fin_arc(0.0, f64::NAN, false), None);
    }

    #[test]
    fn noeuds_de_bspline_valides_ou_refuses() {
        // cubique à 4 points : nœuds 0,0,0,0,1,1,1,1
        assert_eq!(noeuds(&[0.0, 1.0], &[4, 4], 3, 4), Some((3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0])));
        assert_eq!(noeuds(&[0.0, 1.0], &[4, 4], -1, 4), None);
        assert_eq!(noeuds(&[0.0, 1.0], &[4, 4], 0, 4), None);
        assert_eq!(noeuds(&[0.0, 1.0], &[4, 4], 9, 4), None, "degré supérieur au nombre de points");
        assert_eq!(noeuds(&[0.0, 1.0], &[-4, 4], 3, 4), None);
        assert_eq!(noeuds(&[0.0, 1.0], &[i64::MAX, 4], 3, 4), None, "multiplicité démesurée : aucune allocation");
        assert_eq!(noeuds(&[0.0, 1.0], &[4, 3], 3, 4), None);
        assert_eq!(noeuds(&[0.0, f64::NAN], &[4, 4], 3, 4), None);
    }

    #[test]
    fn revolution_bornee_a_un_tour() {
        assert_eq!(angle_revolution(1.0), Some(1.0));
        assert_eq!(angle_revolution(1e12), Some(TAU));
        assert_eq!(angle_revolution(-1e12), Some(-TAU));
        assert_eq!(angle_revolution(f64::NAN), None);
        assert_eq!(angle_revolution(f64::INFINITY), None);
    }
}
