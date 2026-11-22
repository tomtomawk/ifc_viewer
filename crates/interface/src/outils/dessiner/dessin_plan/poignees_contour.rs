//! Axes de repérage figés au début du glissement d'un contour.
use super::*;
use crate::saisie::reperage::{axes_poignee, directions_au_point};

impl Application {
    pub(super) fn glisser_contour(
        &mut self,
        rect: egui::Rect,
        glisse: GlisseContour,
        brut: DVec2,
        maj: bool,
        temps: f64,
    ) {
        let (vise, accroche) =
            self.point_dessin_guide(rect, brut, maj, temps, None, &glisse.guides());
        let p = if accroche == Accroche::Libre {
            brut
        } else {
            vise
        };
        self.dessin2d.point = Some((p, accroche));
        match glisse {
            GlisseContour::Sommet {
                contour, sommet, ..
            } => {
                let p = if accroche == Accroche::Libre {
                    (p * 10.0).round() / 10.0
                } else {
                    p
                };
                self.dessin2d.dalle[contour][sommet] = p.to_array();
            }
            GlisseContour::Segment {
                contour,
                segment,
                origine,
                a,
                b,
            } => {
                let delta = p - origine;
                let points = &mut self.dessin2d.dalle[contour];
                let suivant = (segment + 1) % points.len();
                points[segment] = (a + delta).to_array();
                points[suivant] = (b + delta).to_array();
            }
        }
    }
}

impl GlisseContour {
    pub(super) fn sommet(
        dessin: &EtatDessin2D,
        contour: usize,
        sommet: usize,
        ferme: bool,
    ) -> Self {
        let points = &dessin.dalle[contour];
        let origine = DVec2::from_array(points[sommet]);
        let precedent = if sommet > 0 {
            Some(sommet - 1)
        } else {
            ferme.then_some(points.len() - 1)
        };
        let suivant = (sommet + 1 < points.len() || ferme).then_some(sommet);
        let directions = [precedent, suivant].map(|i| {
            let Some(i) = i else { return DVec2::ZERO };
            let a = DVec2::from_array(points[i]);
            let b = DVec2::from_array(points[(i + 1) % points.len()]);
            let geom = noyau::forme2d::geom_segment(a, b, dessin.courbure(contour, i));
            directions_au_point(origine, &[geom])
                .first()
                .copied()
                .unwrap_or(DVec2::ZERO)
        });
        Self::Sommet {
            contour,
            sommet,
            origine,
            directions,
        }
    }

    pub(super) fn guides(self) -> Vec<(DVec2, DVec2)> {
        let (origine, directions) = match self {
            Self::Sommet {
                origine,
                directions,
                ..
            } => (origine, directions),
            Self::Segment { origine, a, b, .. } => (origine, [b - a, DVec2::ZERO]),
        };
        directions
            .into_iter()
            .flat_map(|direction| axes_poignee(origine, direction))
            .collect()
    }

    /// Une arête mobile ne doit pas attirer la poignée vers sa position précédente.
    pub(super) fn deplace_segment(
        self,
        contour: usize,
        segment: usize,
        suivant: usize,
        n: usize,
    ) -> bool {
        match self {
            Self::Sommet {
                contour: c, sommet, ..
            } => c == contour && (segment == sommet || suivant == sommet),
            Self::Segment {
                contour: c,
                segment: s,
                ..
            } => {
                c == contour
                    && [s, (s + 1) % n]
                        .iter()
                        .any(|&sommet| segment == sommet || suivant == sommet)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_guides_d_un_sommet_restent_sur_les_aretes_initiales() {
        let mut dessin = EtatDessin2D::default();
        dessin
            .dalle
            .push(vec![[10.0, 20.0], [13.0, 24.0], [18.0, 24.0]]);
        let glisse = GlisseContour::sommet(&dessin, 0, 1, false);
        dessin.dalle[0][1] = [31.0, 42.0];
        let guides = glisse.guides();
        assert_eq!(guides.len(), 4);
        assert!(
            guides
                .iter()
                .all(|(origine, _)| *origine == DVec2::new(13.0, 24.0))
        );
        assert!(
            guides
                .iter()
                .any(|(_, d)| d.dot(DVec2::new(3.0, 4.0).normalize()).abs() > 0.999)
        );
        assert!(glisse.deplace_segment(0, 0, 1, 3));
        assert!(glisse.deplace_segment(0, 1, 2, 3));
        assert!(!glisse.deplace_segment(1, 0, 1, 3));
    }
}
