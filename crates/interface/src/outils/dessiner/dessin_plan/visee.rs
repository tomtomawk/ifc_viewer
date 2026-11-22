//! Source d'accrochage et visée de dessin en vue projet.
use super::*;

impl Application {
    /// Point retenu pour le curseur `c` : accrochage, sinon ortho (Maj) ou repérage
    /// polaire (tous les 15°) depuis le point précédent, sinon le point libre.
    pub(crate) fn point_dessin(
        &mut self,
        rect: egui::Rect,
        c: DVec2,
        maj: bool,
        temps: f64,
    ) -> (DVec2, Accroche) {
        // arc en attente de sa courbure : le curseur reste libre (point de passage, ou
        // côté du rayon tapé), sans repérage depuis le dernier point
        let precedent = self
            .dessin2d
            .trace
            .as_ref()
            .filter(|t| !t.attend_courbure())
            .and_then(Trace::precedent);
        self.point_dessin_depuis(rect, c, maj, temps, precedent)
    }

    pub(crate) fn point_dessin_depuis(
        &mut self,
        rect: egui::Rect,
        c: DVec2,
        maj: bool,
        temps: f64,
        precedent: Option<DVec2>,
    ) -> (DVec2, Accroche) {
        self.point_dessin_guide(rect, c, maj, temps, precedent, &[])
    }

    pub(crate) fn point_dessin_guide(
        &mut self,
        rect: egui::Rect,
        c: DVec2,
        maj: bool,
        temps: f64,
        precedent: Option<DVec2>,
        guides_locaux: &[(DVec2, DVec2)],
    ) -> (DVec2, Accroche) {
        use crate::saisie::moteur_accrochage::{Geometrie, Visee, viser};
        // tolérance : 12 pixels en mm du plan de la vue
        let tol = self
            .vers_ecran(rect, c)
            .zip(self.vers_ecran(rect, c + DVec2::X * 1000.0))
            .map_or(200.0, |(a, b)| {
                crate::saisie::moteur_accrochage::TOLERANCE_PX * 1000.0
                    / (a.distance(b) as f64).max(1e-6)
            });
        let source = Geometrie {
            geoms: self.geoms_trace(),
            droites: Vec::new(),
        };
        let visee = Visee {
            tol,
            precedent,
            maj,
            temps,
            delai: self.options.delai_accrochage,
            angle_axes: self.angle_axes_plan(),
            trace_actif: self.dessin2d.trace.is_some(),
            guides_locaux,
            saisie: &self.dessin2d.saisie,
            facteur: modele::unite::unite().facteur(),
            pas: 10.0,
        };
        viser(&mut self.dessin2d.reperage, &source, c, &visee)
    }

    /// Source d'accrochage des vues du projet : géométrie de la vue, tracé en cours et
    /// contours en édition.
    fn geoms_trace(&mut self) -> Vec<Geom> {
        let mut geoms = self.geoms_dessin();
        // vue d'un motif de hachures : le cadre du carré (coins, milieux, bords)
        if let Some(c) = self.cadre_motif() {
            geoms.extend((0..4).map(|i| {
                Geom::Segment([DVec2::from_array(c[i]), DVec2::from_array(c[(i + 1) % 4])])
            }));
        }
        if let Some(t) = &self.dessin2d.trace {
            let k = t.courbures();
            geoms.extend(t.points.windows(2).enumerate().map(|(i, w)| {
                noyau::forme2d::geom_segment(w[0], w[1], k.get(i).copied().unwrap_or(0.0))
            }));
            if let [p] = t.points.as_slice() {
                geoms.push(Geom::Segment([*p, *p]));
            }
        }
        if let Some(e) = self.dessin2d.edition_contour.as_ref() {
            for ci in 0..self.dessin2d.dalle.len() {
                let fermee = e.fermee(ci);
                let n = self.dessin2d.dalle[ci].len();
                geoms.extend(
                    self.dessin2d
                        .segments_contour(ci, fermee)
                        .into_iter()
                        .enumerate()
                        .filter_map(|(i, (a, b, k))| {
                            (!e.glisse
                                .is_some_and(|g| g.deplace_segment(ci, i, (i + 1) % n, n)))
                            .then(|| noyau::forme2d::geom_segment(a, b, k))
                        }),
                );
            }
        }
        geoms
    }

    /// Poignée d’un mur ou d’une poutre : accrochages et cote depuis le point fixe.
    pub(crate) fn point_edition_axe(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        q: egui::Pos2,
        maj: bool,
        fixe: [f64; 2],
        axe: [[f64; 2]; 2],
    ) -> Option<[f64; 2]> {
        let c = self.depuis_ecran(rect, q)?;
        let fixe = DVec2::from_array(fixe);
        let mut trace = Trace::new(OutilDessin::Ligne);
        trace.points.push(fixe);
        self.dessin2d.trace = Some(trace);
        let [origine, appui] = axe.map(DVec2::from_array);
        let guides: Vec<_> =
            crate::saisie::reperage::axes_poignee(origine, appui - origine).collect();
        let point =
            self.point_dessin_guide(rect, c, maj, ui.input(|i| i.time), Some(fixe), &guides);
        self.dessin2d.curseur = Some(c);
        self.dessin2d.point = Some(point);
        if self.dessin2d.reperage.survol_accroche.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        Some(point.0.to_array())
    }
}
