//! Accrochage et navigation communs aux vues du projet et à l'éditeur de familles :
//! choix du point accroché parmi des candidats (extrémité prioritaire), contraintes
//! depuis un point de départ (Maj : orthogonal, angles ronds de 15°), repères dessinés
//! et sensibilité de la molette.
use eframe::egui::{self, Color32};

/// Accrochage d'un point tracé.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum AccrocheMur {
    Grille,
    Extremite,
    Milieu,
    /// Croisement de deux lignes (plans de référence d'une famille).
    Intersection,
    Orthogonal,
    /// Angle rond depuis le départ (degrés).
    Angle(f64),
    /// Angle d'un mur (contour raccordé).
    Coin,
}

/// Rayon d'accrochage à l'écran (px).
pub(super) const RAYON: f32 = 10.0;

/// Zoom à la molette : facteur appliqué à l'échelle de la vue (> 1 : on s'approche).
pub(super) fn facteur_molette(defilement: f32) -> f64 {
    (defilement as f64 * 0.0018).exp()
}

/// Candidat le plus proche de `q` à moins de [`RAYON`] ; à distance égale,
/// l'extrémité et l'intersection l'emportent sur le milieu et l'angle.
pub(super) fn meilleur(q: egui::Pos2, candidats: impl IntoIterator<Item = ([f64; 2], egui::Pos2, AccrocheMur)>) -> Option<([f64; 2], AccrocheMur)> {
    let mut choix = None;
    let mut distance = RAYON;
    for (point, ecran, genre) in candidats {
        let d = q.distance(ecran) + if matches!(genre, AccrocheMur::Extremite | AccrocheMur::Intersection) { 0.0 } else { 0.5 };
        if d < distance {
            distance = d;
            choix = Some((point, if matches!(genre, AccrocheMur::Angle(_)) { AccrocheMur::Coin } else { genre }));
        }
    }
    choix
}

/// Point contraint depuis le départ `a` dans des axes tournés de `theta` : Maj,
/// l'axe dominant ; sinon un angle rond (15°) à moins de 3°. Longueur arrondie à `pas`.
pub(super) fn contraindre(a: [f64; 2], point: [f64; 2], theta: f64, orthogonal: bool, pas: f64) -> Option<([f64; 2], AccrocheMur)> {
    let (dx, dy) = (point[0] - a[0], point[1] - a[1]);
    let (u, v) = ([theta.cos(), theta.sin()], [-theta.sin(), theta.cos()]);
    let arrondi = |l: f64| (l / pas).round() * pas;
    if orthogonal {
        let (du, dv) = (dx * u[0] + dy * u[1], dx * v[0] + dy * v[1]);
        let (axe, l) = if du.abs() >= dv.abs() { (u, du) } else { (v, dv) };
        let l = arrondi(l);
        return Some(([a[0] + axe[0] * l, a[1] + axe[1] * l], AccrocheMur::Orthogonal));
    }
    let longueur = dx.hypot(dy);
    if longueur > 1.0 {
        let angle = (dy.atan2(dx) - theta).to_degrees();
        let rond = (angle / 15.0).round() * 15.0;
        if (angle - rond).abs() < 3.0 {
            let l = arrondi(longueur);
            let r = rond.to_radians() + theta;
            return Some(([a[0] + l * r.cos(), a[1] + l * r.sin()], AccrocheMur::Angle(rond)));
        }
    }
    None
}

/// Repère d'accrochage : carré (extrémité), triangle (milieu), croix (intersection),
/// losange (angle d'un mur), cercle sinon.
pub(super) fn marqueur(painter: &egui::Painter, q: egui::Pos2, acc: AccrocheMur, couleur: Color32, accent: Color32) {
    let t = egui::Stroke::new(2.0, accent);
    let h = 7.0;
    match acc {
        AccrocheMur::Extremite => {
            painter.rect_stroke(egui::Rect::from_center_size(q, egui::vec2(11.0, 11.0)), 0.0, t, egui::StrokeKind::Middle);
        }
        AccrocheMur::Milieu => {
            painter.add(egui::Shape::closed_line(vec![q + egui::vec2(0.0, -h), q + egui::vec2(h, h * 0.8), q + egui::vec2(-h, h * 0.8)], t));
        }
        AccrocheMur::Intersection => {
            painter.line_segment([q + egui::vec2(-h, -h), q + egui::vec2(h, h)], t);
            painter.line_segment([q + egui::vec2(-h, h), q + egui::vec2(h, -h)], t);
        }
        AccrocheMur::Coin => {
            painter.add(egui::Shape::closed_line(vec![q + egui::vec2(0.0, -h), q + egui::vec2(h, 0.0), q + egui::vec2(0.0, h), q + egui::vec2(-h, 0.0)], t));
        }
        _ => {
            painter.circle_stroke(q, 4.0, egui::Stroke::new(1.5, couleur));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extremite_prioritaire_et_contraintes() {
        let q = egui::pos2(100.0, 100.0);
        let c = [([0.0, 0.0], egui::pos2(103.0, 100.0), AccrocheMur::Milieu), ([1.0, 0.0], egui::pos2(103.2, 100.0), AccrocheMur::Extremite), ([2.0, 0.0], egui::pos2(200.0, 100.0), AccrocheMur::Extremite)];
        assert_eq!(meilleur(q, c), Some(([1.0, 0.0], AccrocheMur::Extremite)));
        assert_eq!(meilleur(q, [([0.0, 0.0], egui::pos2(150.0, 100.0), AccrocheMur::Extremite)]), None);
        assert_eq!(contraindre([0.0, 0.0], [1004.0, 230.0], 0.0, true, 10.0), Some(([1000.0, 0.0], AccrocheMur::Orthogonal)));
        let (p, a) = contraindre([0.0, 0.0], [1000.0, 1010.0], 0.0, false, 10.0).unwrap();
        assert_eq!(a, AccrocheMur::Angle(45.0));
        assert!((p[0] - p[1]).abs() < 1e-9);
    }
}
