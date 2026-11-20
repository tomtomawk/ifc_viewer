//! Escaliers balancés (n° 5) : marches tournantes à la place des paliers.
//!
//! Le giron reste constant sur la ligne de foulée. Sur chaque rive, la position du nez
//! de marche suit la ligne de foulée hors des tournants ; autour d'un tournant, sur
//! `balance` marches avant et après, elle est interpolée linéairement entre les deux
//! points de rive qui font face aux bornes de la zone (méthode du balancement par
//! proportion). Côté intérieur les girons se resserrent progressivement au lieu de
//! s'écraser sur une marche ; côté extérieur ils s'élargissent. Le contrôle refuse un
//! giron de rive inférieur à `GIRON_MIN_RIVE`.
use glam::DVec2;

/// Giron minimal mesuré sur une rive (mm).
pub const GIRON_MIN_RIVE: f64 = 100.0;

/// Polyligne et ses longueurs cumulées.
struct Ligne {
    points: Vec<DVec2>,
    cumul: Vec<f64>,
}

impl Ligne {
    fn new(points: Vec<DVec2>) -> Ligne {
        let mut cumul = vec![0.0];
        let mut total = 0.0;
        for w in points.windows(2) {
            total += w[0].distance(w[1]);
            cumul.push(total);
        }
        Ligne { points, cumul }
    }

    fn longueur(&self) -> f64 {
        self.cumul.last().copied().unwrap_or(0.0)
    }

    /// Point à l'abscisse curviligne `u` (bornée).
    fn point(&self, u: f64) -> DVec2 {
        let u = u.clamp(0.0, self.longueur());
        let i = self.cumul.windows(2).position(|c| u <= c[1]).unwrap_or(self.points.len() - 2);
        let l = self.cumul[i + 1] - self.cumul[i];
        let t = if l > 1e-12 { (u - self.cumul[i]) / l } else { 0.0 };
        self.points[i].lerp(self.points[i + 1], t)
    }

    /// Sommets strictement entre les abscisses `a` et `b`.
    fn sommets_entre(&self, a: f64, b: f64) -> Vec<DVec2> {
        (1..self.points.len() - 1).filter(|&i| self.cumul[i] > a + 1e-9 && self.cumul[i] < b - 1e-9).map(|i| self.points[i]).collect()
    }

    /// Abscisse, sur le segment `i`, du pied de la perpendiculaire issue de `p`.
    fn abscisse_sur(&self, i: usize, p: DVec2) -> f64 {
        let (a, b) = (self.points[i], self.points[i + 1]);
        let d = b - a;
        self.cumul[i] + (p - a).dot(d) / d.length().max(1e-12)
    }
}

/// Correspondance entre l'abscisse sur la ligne de foulée et l'abscisse sur une rive.
fn correspondance<'a>(foulee: &'a Ligne, rive: &'a Ligne, zone: f64) -> impl Fn(f64) -> f64 + 'a {
    let n = foulee.points.len();
    // bornes des zones de balancement de chaque tournant, sur la foulée et sur la rive
    let zones: Vec<(f64, f64, f64, f64)> = (1..n - 1).map(|i| {
        let s = foulee.cumul[i];
        let d = zone.min((foulee.cumul[i] - foulee.cumul[i - 1]) / 2.0).min((foulee.cumul[i + 1] - foulee.cumul[i]) / 2.0);
        let (s0, s1) = (s - d, s + d);
        (s0, s1, rive.abscisse_sur(i - 1, foulee.point(s0)), rive.abscisse_sur(i, foulee.point(s1)))
    }).collect();
    move |s: f64| {
        for &(s0, s1, u0, u1) in &zones {
            if s >= s0 && s <= s1 {
                return u0 + (u1 - u0) * (s - s0) / (s1 - s0).max(1e-12);
            }
        }
        let i = foulee.cumul.windows(2).position(|c| s <= c[1] + 1e-9).unwrap_or(n - 2);
        rive.abscisse_sur(i, foulee.point(s))
    }
}

/// Marches balancées : contour en plan de chaque marche (rive gauche puis rive droite,
/// coins de rive compris) pour `marches` marches de giron constant sur la foulée.
pub fn marches(trace: &[DVec2], gauche: &[DVec2], droite: &[DVec2], marches: usize, balance: usize) -> Result<Vec<Vec<DVec2>>, String> {
    Ok(marches_avec_arrivees(trace, gauche, droite, marches, balance)?
        .into_iter().map(|(contour, _)| contour).collect())
}

/// Indice du premier sommet du bord d’arrivée, avant les deux rives du contour.
pub(super) fn marches_avec_arrivees(trace: &[DVec2], gauche: &[DVec2], droite: &[DVec2], marches: usize, balance: usize) -> Result<Vec<(Vec<DVec2>, usize)>, String> {
    if trace.len() < 2 || marches == 0 {
        return Err("Escalier balancé : ligne de foulée trop courte.".into());
    }
    let (foulee, g, d) = (Ligne::new(trace.to_vec()), Ligne::new(gauche.to_vec()), Ligne::new(droite.to_vec()));
    let giron = foulee.longueur() / marches as f64;
    let zone = balance as f64 * giron;
    let (fg, fd) = (correspondance(&foulee, &g, zone), correspondance(&foulee, &d, zone));
    let mut out = Vec::with_capacity(marches);
    for k in 0..marches {
        let (s0, s1) = (k as f64 * giron, (k + 1) as f64 * giron);
        let (ug0, ug1, ud0, ud1) = (fg(s0), fg(s1), fd(s0), fd(s1));
        for (u0, u1) in [(ug0, ug1), (ud0, ud1)] {
            if u1 - u0 < GIRON_MIN_RIVE {
                return Err(format!("Escalier balancé : giron de rive de {:.0} mm à la marche {} (minimum {GIRON_MIN_RIVE} mm) ; augmenter le nombre de marches balancées ou la longueur.", u1 - u0, k + 1));
            }
        }
        let mut contour = vec![g.point(ug0)];
        contour.extend(g.sommets_entre(ug0, ug1));
        let arrivee = contour.len();
        contour.push(g.point(ug1));
        contour.push(d.point(ud1));
        let mut cote_droit = d.sommets_entre(ud0, ud1);
        cote_droit.reverse();
        contour.extend(cote_droit);
        contour.push(d.point(ud0));
        out.push((contour, arrivee));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quart_tournant() -> (Vec<DVec2>, Vec<DVec2>, Vec<DVec2>) {
        // foulée en L : 3 m vers +x puis 3 m vers +y ; emmarchement 1 m
        let trace = vec![DVec2::ZERO, DVec2::new(3000.0, 0.0), DVec2::new(3000.0, 3000.0)];
        let gauche = vec![DVec2::new(0.0, 500.0), DVec2::new(2500.0, 500.0), DVec2::new(2500.0, 3000.0)];
        let droite = vec![DVec2::new(0.0, -500.0), DVec2::new(3500.0, -500.0), DVec2::new(3500.0, 3000.0)];
        (trace, gauche, droite)
    }

    fn aire(c: &[DVec2]) -> f64 {
        (0..c.len()).map(|i| c[i].perp_dot(c[(i + 1) % c.len()])).sum::<f64>().abs() / 2.0
    }

    /// Les marches couvrent toute l'emprise sans recouvrement ; côté intérieur, les
    /// girons décroissent jusqu'au tournant (balancement progressif) puis remontent.
    #[test]
    fn quart_tournant_balance() {
        let (t, g, d) = quart_tournant();
        let m = marches(&t, &g, &d, 24, 5).unwrap();
        assert_eq!(m.len(), 24);
        let total: f64 = m.iter().map(|c| aire(c)).sum();
        assert!((total - 2.0 * 3000.0 * 1000.0 + 0.0).abs() < 1.0, "{total}");
        // girons de la rive intérieure (gauche) : bord gauche de chaque marche
        let interieurs: Vec<f64> = m.iter().map(|c| c[0].distance(c[1])).collect();
        let (avant, apres) = interieurs.split_at(12);
        assert!(avant.windows(2).skip(8).all(|w| w[1] <= w[0] + 1e-6), "décroissance avant le tournant : {interieurs:?}");
        assert!(apres.windows(2).take(3).all(|w| w[1] >= w[0] - 1e-6), "croissance après : {interieurs:?}");
        assert!(interieurs.iter().all(|x| *x >= GIRON_MIN_RIVE));
    }

    /// Trop peu de marches balancées autour d'un tournant serré : le giron intérieur
    /// tombe sous le minimum (83 mm pour 3 marches sur 1 m d'emmarchement) : refus.
    #[test]
    fn controle_du_giron_de_rive() {
        let (t, g, d) = quart_tournant();
        for balance in [0, 3] {
            assert!(marches(&t, &g, &d, 24, balance).is_err());
        }
    }

    /// Escalier balancé complet : aucun palier, une marche par contremarche moins une,
    /// solide fermé, altitude croissante le long de la foulée.
    #[test]
    fn escalier_balance_complet() {
        use crate::{Escalier, Niveau, Projet};
        let mut p = Projet::nouveau("E");
        p.niveaux = vec![Niveau { id: "rdc".into(), nom: "RDC".into(), altitude: 0.0, origine: None }, Niveau { id: "r1".into(), nom: "R+1".into(), altitude: 2800.0, origine: None }];
        let e = Escalier { id: "e".into(), nom: "E".into(), niveau: "rdc".into(), niveau_haut: Some("r1".into()), decalage_bas: 0.0, decalage_haut: 0.0, hauteur: 2800.0, trace: vec![[0.0, 0.0], [3000.0, 0.0], [3000.0, 3000.0]], largeur: 1000.0, hauteur_marche: 175.0, paillasse: 150.0, sous_face: Default::default(), balance: Some(5) };
        let c = e.calcul(&p).unwrap();
        assert_eq!((c.contremarches, c.paliers), (16, 0));
        assert!((c.giron - 400.0).abs() < 1e-6);
        assert!(crate::volumes::geometrie::volume(&e.solide(&p).unwrap()) > 0.0);
        let z = |x: f64, y: f64| e.altitude_en(&p, DVec2::new(x, y)).unwrap();
        assert!(z(500.0, 0.0) < z(2900.0, 100.0) && z(2900.0, 100.0) < z(3000.0, 2500.0));
    }
}
