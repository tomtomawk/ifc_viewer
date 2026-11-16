//! Accrochage et guides temporaires communs aux canevas de dessin.
use glam::DVec2;
use moteur2d::dao::{self as d2, Accroche};
use noyau::forme2d::{Geom};

/// Directions des éléments passant par un point de référence, pour leurs guides.
pub(crate) fn directions_au_point(p: DVec2, geoms: &[Geom]) -> Vec<DVec2> {
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
pub(crate) struct Reperage {
    pub survol_accroche: Option<(DVec2, f64)>,
    pub survol_segment: Option<(DVec2, DVec2, f64, bool)>,
    pub reference: Option<DVec2>,
    pub directions_reference: Vec<DVec2>,
    pub guides: Vec<(DVec2, DVec2)>,
    /// Cercle support (centre, rayon) de l'arc survolé : trait de construction.
    pub cercle: Option<(DVec2, f64)>,
    /// Axes et faces acquis par survol : droites de guidage (origine, direction), deux
    /// au plus. On ne s'y accroche qu'à leurs intersections, jamais en un point
    /// quelconque de la droite.
    pub droites: Vec<(DVec2, DVec2)>,
}

/// Nombre de droites de guidage acquises gardées (la plus ancienne cède la place).
const DROITES_MAX: usize = 2;

/// Cercle support de l'arc ou du cercle le plus proche de `c`, à moins de `tol`.
pub(crate) fn cercle_survole(c: DVec2, tol: f64, geoms: &[Geom]) -> Option<(DVec2, f64)> {
    geoms.iter().filter_map(|g| match g {
        Geom::Arc { centre, rayon, .. } | Geom::Cercle { centre, rayon } => {
            let d = (c.distance(*centre) - rayon).abs();
            (d < tol * 0.6 && d2::distance(c, g).is_some_and(|d| d < tol * 0.6)).then_some((d, (*centre, *rayon)))
        }
        _ => None,
    }).min_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1)
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
        if let Some((centre, rayon)) = self.cercle {
            let points: Vec<_> = (0..=96).map(|i| ecran(centre + DVec2::from_angle(std::f64::consts::TAU * i as f64 / 96.0) * rayon)).collect();
            painter.add(Shape::dashed_line(&points, Stroke::new(1.0, couleur), 6.0, 4.0));
        }
    }

    /// Départ d'une distance tapée (n° 2) : le point de référence acquis, quand le point
    /// visé suit un guide qui en part (axe, perpendiculaire, direction d'un élément).
    /// La distance se mesure alors depuis ce point, le long du guide.
    pub fn depart_saisie(&self, point: DVec2) -> Option<DVec2> {
        let r = self.reference?;
        let sur_guide = self.guides.iter().any(|(o, d)| o.distance(r) < 1e-9 && d.perp_dot(point - r).abs() < 1e-6);
        (sur_guide && point.distance(r) > 1e-9).then_some(r)
    }

    /// Oublie les références et les guides (changement d'outil, fin de commande).
    pub fn oublier(&mut self) {
        *self = Reperage::default();
    }

    pub fn calculer(&mut self, c: DVec2, tol: f64, geoms: &[Geom], precedent: Option<DVec2>, maj: bool, temps: f64, delai: f64, angle_axes: f64, trace_actif: bool) -> Option<(DVec2, Accroche)> {
        self.guides.clear();
        self.cercle = (!maj).then(|| cercle_survole(c, tol, geoms)).flatten();
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
        if !maj && let Some((_, a, b, _)) = segment {
            match self.survol_segment {
                Some((ancien_a, ancien_b, debut, verrouille)) if ancien_a == a && ancien_b == b => {
                    if !verrouille && temps - debut >= delai {
                        // l'axe ou la face devient une droite de guidage, sans point verrouillé
                        let d = (b - a).normalize();
                        let deja = self.droites.iter().any(|(o, e)| e.perp_dot(d).abs() < 1e-9 && e.perp_dot(a - *o).abs() < 1e-6);
                        if !deja {
                            self.droites.push((a, d));
                            if self.droites.len() > DROITES_MAX { self.droites.remove(0); }
                        }
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
            // droites acquises : toujours affichées, accrochables à leurs intersections
            let projetables = lignes.len();
            lignes.extend(self.droites.iter().copied());
            self.guides.extend(self.droites.iter().copied());
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
                for g in guides { if !self.guides.contains(&g) { self.guides.push(g); } }
                return Some((p, Accroche::Intersection));
            }
            // projection : seulement sur les guides issus d'un point remarquable (point
            // précédent, référence), pas sur les droites acquises
            let mut proche = None;
            for (a, d) in lignes.into_iter().take(projetables) {
                let p = a + d * (c - a).dot(d);
                let distance = p.distance(c);
                if distance < tol && proche.as_ref().is_none_or(|(dist, _, _)| distance < *dist) {
                    proche = Some((distance, p, (a, d)));
                }
            }
            if let Some((_, p, guide)) = proche {
                if !self.guides.contains(&guide) { self.guides.push(guide); }
                return Some((p, Accroche::Ortho));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// N° 23 : un segment survolé devient une droite de guidage prolongée, sans point
    /// verrouillé ; on s'accroche à son intersection avec un autre guide, pas en un point
    /// quelconque de la droite.
    #[test]
    fn survol_retarde_acquiert_une_droite_sans_verrouillage() {
        let mut repere = Reperage::default();
        let geoms = [Geom::Segment([DVec2::ZERO, DVec2::new(100.0, 0.0)])];
        let p = DVec2::new(30.0, 0.2);
        repere.calculer(p, 2.0, &geoms, None, false, 1.0, 0.8, 0.0, true);
        assert!(repere.droites.is_empty());
        repere.calculer(p, 2.0, &geoms, None, false, 1.81, 0.8, 0.0, true);
        assert!(repere.reference.is_none(), "aucun point verrouillé");
        assert_eq!(repere.droites, vec![(DVec2::ZERO, DVec2::X)]);
        // prolongement de la droite, loin du segment : affichée mais pas accrochée seule
        assert!(repere.calculer(DVec2::new(300.0, 0.5), 2.0, &geoms, None, false, 2.0, 0.8, 0.0, true).is_none());
        assert!(repere.guides.contains(&(DVec2::ZERO, DVec2::X)));
        // intersection avec la verticale du point précédent : accrochée
        let (point, genre) = repere.calculer(DVec2::new(250.5, 0.8), 2.0, &geoms, Some(DVec2::new(250.0, 80.0)), false, 2.1, 0.8, 0.0, true).unwrap();
        assert_eq!((point, genre), (DVec2::new(250.0, 0.0), Accroche::Intersection));
        // Maj : aucun guide
        assert!(repere.calculer(DVec2::new(250.5, 0.8), 2.0, &geoms, None, true, 2.2, 0.8, 0.0, true).is_none());
        assert!(repere.guides.is_empty());
        repere.oublier();
        assert!(repere.droites.is_empty());
    }

    /// N° 2 : point accroché retenu comme référence, curseur sur son guide vertical :
    /// la distance tapée part de la référence ; hors guide, du point précédent.
    #[test]
    fn distance_tapee_depuis_le_point_accroche() {
        let mut repere = Reperage::default();
        let geoms = [Geom::Segment([DVec2::ZERO, DVec2::new(100.0, 0.0)])];
        // survol prolongé de l'extrémité : référence acquise
        repere.calculer(DVec2::new(100.5, 0.5), 2.0, &geoms, None, false, 0.0, 0.5, 0.0, true);
        repere.calculer(DVec2::new(100.5, 0.5), 2.0, &geoms, None, false, 1.0, 0.5, 0.0, true);
        assert_eq!(repere.reference, Some(DVec2::new(100.0, 0.0)));
        let (point, _) = repere.calculer(DVec2::new(100.6, 40.0), 2.0, &geoms, None, false, 1.1, 0.5, 0.0, true).unwrap();
        assert_eq!(repere.depart_saisie(point), Some(DVec2::new(100.0, 0.0)));
        let depart = repere.depart_saisie(point).unwrap();
        assert_eq!(moteur2d::dao::lire_saisie("25", Some(depart), Some(point - depart), 1.0).unwrap(), DVec2::new(100.0, 25.0));
        assert_eq!(repere.depart_saisie(DVec2::new(60.0, 40.0)), None);
    }
}

#[cfg(test)]
mod tests_cercle {
    use super::*;

    /// N° 25 : survoler une face courbe désigne son cercle support ; loin de la portion
    /// tracée de l'arc, rien.
    #[test]
    fn face_courbe_survolee_donne_son_cercle() {
        let face = Geom::Arc { centre: DVec2::ZERO, rayon: 5000.0, depart: 0.0, balayage: std::f64::consts::FRAC_PI_2 };
        let geoms = [Geom::Segment([DVec2::new(-9000.0, 0.0), DVec2::new(-8000.0, 0.0)]), face];
        let survol = DVec2::from_angle(0.7) * 5003.0;
        assert_eq!(cercle_survole(survol, 20.0, &geoms), Some((DVec2::ZERO, 5000.0)));
        assert_eq!(cercle_survole(DVec2::from_angle(3.0) * 5000.0, 20.0, &geoms), None);
        let mut r = Reperage::default();
        r.calculer(survol, 20.0, &geoms, None, false, 0.0, 1.0, 0.0, false);
        assert_eq!(r.cercle, Some((DVec2::ZERO, 5000.0)));
    }
}
