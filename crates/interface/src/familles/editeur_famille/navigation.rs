//! Navigation des vues planes de famille, alignée sur les vues de projet : zoom au
//! curseur (même pas de molette), panoramique au bouton du milieu, double-clic du milieu
//! pour cadrer, même fond, et barre de vue sous le canevas avec l'échelle de la vue.
use super::*;

/// Échelle par défaut des vues de famille (1:20, comme les gabarits de famille Revit).
pub(crate) const ECHELLE_VUE_FAMILLE: f64 = 20.0;

#[derive(Clone, Copy)]
pub(crate) struct NavigationFamille {
    pub(super) zoom: f64,
    pub(super) decalage: (f64, f64),
    /// Échelle de la vue (1:N) : hauteur des textes et libellés des familles
    /// modélisées, en mm du papier. Les cartouches et étiquettes sont dessinés en mm du
    /// papier et l'ignorent.
    pub(super) echelle_vue: f64,
}

impl Default for NavigationFamille {
    fn default() -> Self { Self { zoom: 1.0, decalage: (0.0, 0.0), echelle_vue: ECHELLE_VUE_FAMILLE } }
}

impl NavigationFamille {
    pub(crate) fn cadre(&self, base: &Cadre) -> Cadre {
        Cadre { centre: base.centre, milieu: (base.milieu.0 + self.decalage.0, base.milieu.1 + self.decalage.1), echelle: base.echelle * self.zoom }
    }

    /// Revient au cadrage complet de la vue, en gardant son échelle.
    pub(crate) fn cadrer(&mut self) {
        *self = Self { echelle_vue: self.echelle_vue, ..Self::default() };
    }

    pub(crate) fn naviguer(&mut self, base: &Cadre, curseur: egui::Pos2, molette: f32, pan: egui::Vec2) {
        let avant = self.cadre(base);
        if molette != 0.0 {
            let point = avant.modele(curseur);
            self.zoom = (self.zoom * crate::saisie::accrochage::facteur_molette(molette)).clamp(0.05, 100.0);
            let apres = self.cadre(base).modele(curseur);
            self.decalage.0 += point.0 - apres.0;
            self.decalage.1 += point.1 - apres.1;
        }
        let echelle = base.echelle * self.zoom;
        self.decalage.0 -= pan.x as f64 / echelle;
        self.decalage.1 += pan.y as f64 / echelle;
    }

    /// Gestes des vues de projet : molette au curseur, glisser au bouton du milieu,
    /// double-clic du milieu pour cadrer (même pendant un outil).
    pub(crate) fn souris(&mut self, ui: &egui::Ui, vue: &egui::Response, base: &Cadre) {
        if vue.double_clicked_by(PointerButton::Middle) {
            self.cadrer();
            return;
        }
        // Les réponses de glissement lisent elles-mêmes le contexte egui.
        let panoramique = vue.dragged_by(PointerButton::Middle);
        let pan = if panoramique { vue.drag_delta() } else { egui::Vec2::ZERO };
        let survole = vue.hovered();
        let (molette, curseur) = ui.input_mut(|i| {
            let molette = if survole { std::mem::take(&mut i.smooth_scroll_delta.y) } else { 0.0 };
            (molette, i.pointer.hover_pos().unwrap_or(base.centre))
        });
        self.naviguer(base, curseur, molette, pan);
        if panoramique {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
    }
}

/// Transformation de la vue : repère de la famille (h, v) ↔ écran.
pub(crate) struct Cadre {
    pub centre: egui::Pos2,
    pub milieu: (f64, f64),
    pub echelle: f64,
}

impl Cadre {
    pub(crate) fn ecran(&self, h: f64, v: f64) -> egui::Pos2 {
        self.centre + egui::vec2(((h - self.milieu.0) * self.echelle) as f32, (-(v - self.milieu.1) * self.echelle) as f32)
    }
    pub(crate) fn modele(&self, p: egui::Pos2) -> (f64, f64) {
        let d = p - self.centre;
        (self.milieu.0 + d.x as f64 / self.echelle, self.milieu.1 - d.y as f64 / self.echelle)
    }
}

/// Fond des vues de projet et de famille : blanc en style graphique, gris clair sinon.
pub(crate) fn fond_vue(style: moteur3d::gpu::StyleVisuel) -> [f32; 3] {
    if style.papier() { [1.0, 1.0, 1.0] } else { [0.925, 0.933, 0.945] }
}

pub(crate) fn couleur_fond_vue(style: moteur3d::gpu::StyleVisuel) -> Color32 {
    let [r, v, b] = fond_vue(style).map(|c| (c * 255.0).round() as u8);
    Color32::from_rgb(r, v, b)
}

/// Hauteur à l'écran (pixels) d'un texte de `hauteur` mm du papier : les cartouches et
/// étiquettes sont en mm du papier, les autres familles à l'échelle de leur vue.
pub(super) fn taille_texte(hauteur: f64, papier: bool, echelle_vue: f64, cadre: &Cadre) -> f32 {
    let modele = if papier { hauteur } else { hauteur * echelle_vue };
    (modele * cadre.echelle).clamp(1.0, 400.0) as f32
}

/// Étendue cadrée de la vue plane (h0, h1, v0, v1, marge) : plans de la famille (`h`, `v`)
/// et éléments 2D de la vue. Cartouches et étiquettes : leur seul dessin (et l'origine),
/// le mur générique n'ayant pas de sens sur le papier.
pub(super) fn cadrage_vue(ed: &EditeurFamille, r: &Resolution, h: [f64; 2], v: [f64; 2]) -> (f64, f64, f64, f64, f64) {
    use modele::documentation::AnnotationForme;
    use modele::famille::modelee::CategorieFamille as C;
    let papier = matches!(ed.famille.categorie, C::Cartouche | C::EtiquettePiece);
    let mut points = Vec::new();
    for e in ed.famille.elements_2d.iter().filter(|e| e.vue == ed.vue.esquisse()) {
        let e = ed.famille.element_resolu(r, e);
        if let AnnotationForme::Texte { point, texte } = &e.forme {
            // encombrement approché du texte : centré (étiquette) ou à gauche (cartouche)
            let (l, h) = (texte.chars().count() as f64 * e.hauteur * 0.6, e.hauteur);
            let l = if papier { l } else { 0.0 };
            points.extend([[point[0] - l, point[1]], [point[0] + l, point[1] + h]]);
        }
        points.extend(e.forme.forme_2d().into_iter().flat_map(|f| f.polylignes()).flatten().map(|p| [p.x, p.y]));
    }
    let depart = if papier && !points.is_empty() { ([0.0; 2], [0.0; 2]) } else { (h, v) };
    let ([h0, h1], [v0, v1]) = points.iter().fold(depart, |([a, b], [c, d]), p| ([a.min(p[0]), b.max(p[0])], [c.min(p[1]), d.max(p[1])]));
    let marge = if papier { (h1 - h0).max(v1 - v0) * 0.15 + 5.0 } else { (h1 - h0).max(v1 - v0) * 0.25 + 150.0 };
    (h0 - marge, h1 + marge, v0 - marge, v1 + marge, marge)
}

impl ViewerApp {
    /// Barre de la vue de famille, sous la vue comme celle du projet : échelle de la vue
    /// et cadrage.
    pub(crate) fn barre_vue_famille(&mut self, ui: &mut egui::Ui) {
        let Some(ed) = self.famille.as_mut() else { return };
        use modele::famille::modelee::CategorieFamille as C;
        let papier = matches!(ed.famille.categorie, C::Cartouche | C::EtiquettePiece);
        ui.horizontal_wrapped(|ui| {
            let vue_plane = !ed.vue_3d;
            let nav = &mut ed.navigation[ed.vue as usize];
            let mut echelle = if papier { 1.0 } else { nav.echelle_vue };
            let r = ui.add_enabled(vue_plane && !papier, egui::DragValue::new(&mut echelle).speed(1.0).range(1.0..=10000.0).prefix("1 : "));
            if r.on_disabled_hover_text(if papier { "Cartouche ou étiquette : dessin en millimètres du papier." } else { "Ouvrir une vue plane." })
                .on_hover_text("Échelle de la vue : hauteur des textes et libellés de la famille.")
                .changed() {
                nav.echelle_vue = echelle;
            }
            ui.separator();
            if ui.button("Cadrer").on_hover_text("Afficher toute la vue (double-clic du bouton du milieu)").clicked() {
                if ed.vue_3d { ed.cadrage_3d_initial = true; } else { nav.cadrer(); }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_conserve_le_point_sous_la_souris_et_pan_deplace_la_vue() {
        let base = Cadre { centre: egui::pos2(500., 400.), milieu: (0., 1000.), echelle: 0.2 };
        let mut n = NavigationFamille::default();
        let q = egui::pos2(650., 250.);
        let avant = n.cadre(&base).modele(q);
        n.naviguer(&base, q, 200., egui::Vec2::ZERO);
        let apres = n.cadre(&base).modele(q);
        assert!((avant.0 - apres.0).abs() < 1e-9 && (avant.1 - apres.1).abs() < 1e-9);
        let ecran = n.cadre(&base).ecran(avant.0, avant.1);
        n.naviguer(&base, q, 0., egui::vec2(30., -20.));
        let suivant = n.cadre(&base).ecran(avant.0, avant.1);
        assert!((suivant - ecran - egui::vec2(30., -20.)).length() < 1e-4);
        n.echelle_vue = 50.0;
        n.cadrer();
        assert_eq!((n.zoom, n.decalage, n.echelle_vue), (1.0, (0.0, 0.0), 50.0));
    }

    #[test]
    fn texte_a_l_echelle_de_la_vue_sauf_sur_le_papier() {
        let cadre = Cadre { centre: egui::Pos2::ZERO, milieu: (0.0, 0.0), echelle: 0.1 };
        // 2,5 mm à 1:20 : 50 mm du modèle, 5 pixels à 0,1 px/mm
        assert!((taille_texte(2.5, false, 20.0, &cadre) - 5.0).abs() < 1e-5);
        assert!((taille_texte(2.5, true, 20.0, &cadre) - 1.0).abs() < 1e-5);
        assert_eq!(fond_vue(moteur3d::gpu::StyleVisuel::Graphique), [1.0; 3]);
    }
}
