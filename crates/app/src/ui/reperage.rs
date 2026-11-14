//! Accrochage et guides temporaires communs aux canevas de dessin.
use glam::DVec2;
use crate::dessin::dao::{self as d2, Accroche};
use noyau::forme2d::{Geom};

/// Directions des éléments passant par un point de référence, pour leurs guides.
pub(super) fn directions_au_point(p: DVec2, geoms: &[Geom]) -> Vec<DVec2> {
    geoms.iter().filter_map(|g| match g {
        Geom::Segment([a, b]) => {
            let d = *b - *a;
            let l2 = d.length_squared();
            if l2 < 1e-12 { return None; }
            let t = (p - *a).dot(d) / l2;
            ((-1e-8..=1.0 + 1e-8).contains(&t) && (*a + d * t).distance(p) < 1e-5).then(|| d.normalize())
        }
        Geom::Cercle { centre, rayon } | Geom::Arc { centre, rayon, .. } => {
            let rayon_point = p - *centre;
            (rayon_point.length() > 1e-9 && (rayon_point.length() - rayon).abs() < 1e-5)
                .then(|| rayon_point.normalize().perp())
        }
        Geom::Courbe(_) => None,
    }).collect()
}

#[derive(Default)]
pub(super) struct Reperage {
    pub survol_accroche: Option<(DVec2, f64)>,
    pub survol_segment: Option<(DVec2, DVec2, f64, bool)>,
    pub reference: Option<DVec2>,
    pub directions_reference: Vec<DVec2>,
    pub guides: Vec<(DVec2, DVec2)>,
}

impl Reperage {
    pub fn peindre(&self, painter: &eframe::egui::Painter, ecran: impl Fn(DVec2) -> eframe::egui::Pos2) {
        use eframe::egui::{Color32, Shape, Stroke};
        let couleur = Color32::from_rgb(20, 150, 90);
        for &(origine, direction) in &self.guides {
            let a = ecran(origine);
            let d = (ecran(origine + direction * 1000.0) - a).normalized();
            painter.add(Shape::dashed_line(&[a - d * 4000.0, a + d * 4000.0], Stroke::new(1.0, couleur), 6.0, 4.0));
        }
        if let Some(p) = self.reference { painter.circle_stroke(ecran(p), 5.0, Stroke::new(1.5, couleur)); }
    }

    pub fn calculer(&mut self, c: DVec2, tol: f64, geoms: &[Geom], precedent: Option<DVec2>, maj: bool, temps: f64, delai: f64, angle_axes: f64, trace_actif: bool) -> Option<(DVec2, Accroche)> {
        self.guides.clear();
        let accroche = (!maj).then(|| d2::accrocher(c, tol, geoms, precedent)).flatten();
        if let Some((p, genre)) = accroche {
            self.survol_segment = None;
            match self.survol_accroche {
                Some((ancien, debut)) if ancien.distance(p) < tol * 0.1 => {
                    if temps - debut >= delai && self.reference != Some(p) {
                        self.reference = Some(p);
                        self.directions_reference = directions_au_point(p, geoms);
                    }
                }
                _ => self.survol_accroche = Some((p, temps)),
            }
            if genre == Accroche::Perpendiculaire && let Some(depart) = precedent {
                let d = p - depart;
                if d.length_squared() > 1e-12 { self.guides.push((p, d.normalize())); }
            }
            return Some((p, genre));
        }
        self.survol_accroche = None;
        // Un segment ne fournit des guides qu'après un survol volontaire de son tracé.
        let segment = geoms.iter().filter_map(|g| {
            let Geom::Segment([a, b]) = g else { return None };
            let d = *b - *a;
            let l2 = d.length_squared();
            if l2 < 1e-12 { return None; }
            let t = ((c - *a).dot(d) / l2).clamp(0.0, 1.0);
            let p = *a + d * t;
            let distance = p.distance(c);
            (distance < tol * 0.6).then_some((distance, *a, *b, p))
        }).min_by(|a, b| a.0.total_cmp(&b.0));
        if !maj && let Some((_, a, b, p)) = segment {
            match self.survol_segment {
                Some((ancien_a, ancien_b, debut, verrouille)) if ancien_a == a && ancien_b == b => {
                    if !verrouille && temps - debut >= delai {
                        self.reference = Some(p);
                        self.directions_reference = vec![(b - a).normalize()];
                        self.survol_segment = Some((a, b, debut, true));
                    }
                }
                _ => self.survol_segment = Some((a, b, temps, false)),
            }
        } else { self.survol_segment = None; }
        if maj {
            self.guides.clear();
        } else {
            let th = angle_axes;
            let u = DVec2::from_angle(th);
            let v = DVec2::new(-u.y, u.x);
            let mut lignes = Vec::new();
            if trace_actif {
                if let Some(p) = precedent { lignes.extend([(p, u), (p, v)]); }
            }
            if let Some(p) = self.reference {
                lignes.extend([(p, u), (p, v)]);
                for &d in &self.directions_reference { lignes.extend([(p, d), (p, d.perp())]); }
            }
            // Une intersection de deux guides est prioritaire sur une projection simple.
            let mut meilleur: Option<(f64, DVec2, Vec<(DVec2, DVec2)>)> = None;
            for i in 0..lignes.len() {
                for j in i + 1..lignes.len() {
                    let (a, d) = lignes[i];
                    let (b, e) = lignes[j];
                    let det = d.perp_dot(e);
                    if det.abs() < 1e-9 { continue; }
                    let p = a + d * ((b - a).perp_dot(e) / det);
                    let distance = p.distance(c);
                    if distance < tol && meilleur.as_ref().is_none_or(|m| distance < m.0) {
                        meilleur = Some((distance, p, vec![lignes[i], lignes[j]]));
                    }
                }
            }
            if let Some((_, p, guides)) = meilleur {
                self.guides = guides;
                return Some((p, Accroche::Intersection));
            }
            let mut proche = None;
            for (a, d) in lignes {
                let p = a + d * (c - a).dot(d);
                let distance = p.distance(c);
                if distance < tol && proche.as_ref().is_none_or(|(dist, _, _)| distance < *dist) {
                    proche = Some((distance, p, (a, d)));
                }
            }
            if let Some((_, p, guide)) = proche {
                self.guides.push(guide);
                return Some((p, Accroche::Ortho));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survol_retarde_cree_un_guide_depuis_un_segment() {
        let mut repere = Reperage::default();
        let geoms = [Geom::Segment([DVec2::ZERO, DVec2::new(100.0, 0.0)])];
        let p = DVec2::new(30.0, 0.2);
        repere.calculer(p, 2.0, &geoms, None, false, 1.0, 0.8, 0.0, true);
        assert!(repere.reference.is_none());
        repere.calculer(p, 2.0, &geoms, None, false, 1.81, 0.8, 0.0, true);
        assert_eq!(repere.reference, Some(DVec2::new(30.0, 0.0)));
        let (point, genre) = repere.calculer(DVec2::new(30.5, 30.0), 2.0, &geoms, None, false, 2.0, 0.8, 0.0, true).unwrap();
        assert_eq!(genre, Accroche::Ortho);
        assert_eq!(point, DVec2::new(30.0, 30.0));
        assert!(!repere.guides.is_empty());
        assert!(repere.calculer(DVec2::new(30.5, 30.0), 2.0, &geoms, None, true, 2.1, 0.8, 0.0, true).is_none());
        assert!(repere.guides.is_empty());
    }
}
