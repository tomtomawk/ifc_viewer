//! Arcs et cercles retrouvés dans une polyligne d'annotation, pour les exports (SVG `A`,
//! DXF `ARC`/`CIRCLE`).
//!
//! Les arcs des annotations (polylignes à `courbures`, cercles, éléments de dessin) sont
//! développés en points par `forme2d::points_arc` : pas angulaire constant d'au plus 5°,
//! rayon constant. Une suite d'au moins trois points qui respecte ces deux règles est
//! rendue comme un arc ; tout le reste reste en segments. Les arêtes issues de maillages
//! (rôles autres qu'annotation) ne passent pas par ici.
use glam::DVec2;

/// Pas angulaire maximal d'un arc développé (`forme2d::points_arc`).
const PAS_MAX: f64 = 5.0 * std::f64::consts::PI / 180.0 + 1e-9;

/// Morceau d'une polyligne d'export.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Morceau {
    Segment(DVec2, DVec2),
    /// Arc de `debut` à `fin` autour de `centre` ; `balayage` signé (radians, sens
    /// trigonométrique du repère des points).
    Arc { centre: DVec2, rayon: f64, debut: DVec2, fin: DVec2, angle_debut: f64, balayage: f64 },
    Cercle { centre: DVec2, rayon: f64 },
}

/// Centre du cercle passant par trois points (None : alignés).
fn centre(a: DVec2, b: DVec2, c: DVec2) -> Option<DVec2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    let (a2, b2, c2) = (a.length_squared(), b.length_squared(), c.length_squared());
    let o = DVec2::new(a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y), a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x));
    let echelle = (b - a).length().max((c - b).length());
    (d.abs() > 1e-12 * echelle * echelle).then(|| o / d)
}

/// Angle signé de `u` à `v`.
fn angle(u: DVec2, v: DVec2) -> f64 {
    u.perp_dot(v).atan2(u.dot(v))
}

/// Découpe de la polyligne `p` (fermée : le dernier point rejoint le premier) en segments,
/// arcs et cercles.
pub fn morceaux(p: &[DVec2], fermee: bool) -> Vec<Morceau> {
    let mut pts: Vec<DVec2> = p.to_vec();
    pts.dedup_by(|a, b| a.distance_squared(*b) < 1e-18);
    if fermee && pts.len() > 2 && pts[0].distance_squared(*pts.last().unwrap()) > 1e-18 {
        pts.push(pts[0]);
    }
    let n = pts.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < n {
        if let Some((j, o, r, pas)) = arc_depuis(&pts, i) {
            let total = pas * (j - i) as f64;
            if (total.abs() - std::f64::consts::TAU).abs() < 1e-6 && i == 0 && j == n - 1 {
                out.push(Morceau::Cercle { centre: o, rayon: r });
            } else {
                out.push(Morceau::Arc { centre: o, rayon: r, debut: pts[i], fin: pts[j], angle_debut: (pts[i] - o).to_angle(), balayage: total });
            }
            i = j;
        } else {
            out.push(Morceau::Segment(pts[i], pts[i + 1]));
            i += 1;
        }
    }
    out
}

/// Arc le plus long partant du point `i` : (dernier indice, centre, rayon, pas signé).
fn arc_depuis(p: &[DVec2], i: usize) -> Option<(usize, DVec2, f64, f64)> {
    let (a, b, c) = (*p.get(i)?, *p.get(i + 1)?, *p.get(i + 2)?);
    let o = centre(a, b, c)?;
    let r = (a - o).length();
    let pas = angle(a - o, b - o);
    if !(r.is_finite() && pas.abs() > 1e-9 && pas.abs() <= PAS_MAX) {
        return None;
    }
    let tol_r = 1e-6 * r.max(1.0);
    let mut j = i + 1;
    while j + 1 < p.len() {
        let q = p[j + 1];
        let d = angle(p[j] - o, q - o);
        if ((q - o).length() - r).abs() > tol_r || (d - pas).abs() > 1e-6 {
            break;
        }
        j += 1;
    }
    (j >= i + 2).then_some((j, o, r, pas))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forme2d::points_arc;

    #[test]
    fn arc_cercle_et_segments() {
        // segment, puis quart d'arc de rayon 2 autour de (0, 0)
        let mut p = vec![DVec2::new(5.0, 0.0)];
        p.extend(points_arc(DVec2::ZERO, 2.0, 0.0, std::f64::consts::FRAC_PI_2));
        let m = morceaux(&p, false);
        assert_eq!(m.len(), 2, "{m:?}");
        assert!(matches!(m[0], Morceau::Segment(..)));
        let Morceau::Arc { centre, rayon, balayage, .. } = m[1] else { panic!("arc attendu : {m:?}") };
        assert!(centre.length() < 1e-9 && (rayon - 2.0).abs() < 1e-9 && (balayage - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        // cercle fermé
        let c = points_arc(DVec2::new(1.0, 1.0), 3.0, 0.0, std::f64::consts::TAU);
        assert!(matches!(morceaux(&c[..c.len() - 1], true)[..], [Morceau::Cercle { .. }]));
        // polygone à 32 côtés (maillage) : segments
        let g: Vec<DVec2> = (0..32).map(|k| DVec2::from_angle(k as f64 * std::f64::consts::TAU / 32.0)).collect();
        assert!(morceaux(&g, true).iter().all(|m| matches!(m, Morceau::Segment(..))));
        // points alignés : segments
        assert_eq!(morceaux(&[DVec2::ZERO, DVec2::X, DVec2::X * 2.0], false).len(), 2);
    }
}
