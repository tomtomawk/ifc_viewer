//! Vue 3D de la famille : même moteur, même caméra et mêmes gestes que le projet, avec
//! une caméra propre à la famille ; plan de dessin actif avec sa grille, saisie des
//! profils et des plans par trois points (`plan_actif.rs`).
use super::*;
use plan_actif::Ecran3D;

impl ViewerApp {
    pub(super) fn vue_3d_famille(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let fond = ui.painter().add(egui::Shape::Noop);
        crate::socle::aides::curseur_dessin(ui, &response, self.famille.as_ref().is_some_and(|ed| ed.outil != OutilFamille::Selection || ed.choix_face));
        let Some(ed) = self.famille.as_mut() else { return };
        ed.rect_navigation = rect;
        let r = ed.resoudre();
        let scene = scene_famille(ed, &r);
        if let Some((min, max)) = scene.min && (ed.cadrage_3d_initial || response.double_clicked_by(PointerButton::Middle)) {
            ed.camera_3d.fit(min, max);
            ed.cadrage_3d_initial = false;
        }
        if response.dragged_by(PointerButton::Middle) {
            let d = response.drag_delta();
            if ui.input(|i| i.modifiers.shift) { ed.camera_3d.orbit(d.x, d.y); }
            else { ed.camera_3d.pan(d.x, d.y, rect.height()); }
        }
        if response.dragged_by(PointerButton::Secondary) {
            let d = response.drag_delta();
            ed.camera_3d.ortho = false;
            ed.camera_3d.regarder(d.x, d.y);
        }
        if let Some(pos) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            crate::socle::projection::zoom_molette(&mut ed.camera_3d, rect, pos, scroll, None);
        }
        if !ui.ctx().egui_wants_keyboard_input() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if ed.choix_face { ed.choix_face = false; }
            else if ed.trace_en_cours() || !ed.points_3d.is_empty() { ed.abandonner_saisie(); }
            else { ed.choisir_outil(OutilFamille::Selection); }
        }
        let camera = ed.camera_3d.clone();
        let ecran = Ecran3D { camera: &camera, rect };
        if ed.outil == OutilFamille::Selection && !ed.choix_face && let Some(pos) = response.interact_pointer_pos().filter(|_| response.clicked_by(PointerButton::Primary)) {
            let (origine, direction) = ecran.rayon(pos);
            let selection = ed.solide_touche(&r, origine, direction).map(|(_, k, _)| if k < ed.famille.volumes.len() { SelectionFamille::Volume(k) } else { SelectionFamille::Extrusion(k - ed.famille.volumes.len()) });
            ed.selectionner(selection.into_iter().collect());
        }
        let prims = ed.saisie_3d(ui, &response, &ecran, &r);
        let selection = match ed.selection { SelectionFamille::Volume(k) => Some(k), SelectionFamille::Extrusion(k) => Some(ed.famille.volumes.len() + k), _ => None };
        let message = ed.message.clone();
        let mut camera = camera.clone();
        let mm = (self.echelle / 1000.0) as f32;
        let (tris, traits) = moteur2d::sommets(&|_| None, &prims, (-camera.forward()).as_dvec3(), self.echelle, self.traits_fins);
        if let Some(image) = self.rendre_famille(ui, rect, &mut camera, &scene, (&tris, &traits), selection, mm) {
            ui.painter().set(fond, image);
        }
        // cube d'orientation du projet, sur la caméra de la famille
        std::mem::swap(&mut self.camera, &mut camera);
        self.cube_orientation(ui, rect);
        std::mem::swap(&mut self.camera, &mut camera);
        if let Some(ed) = self.famille.as_mut() { ed.camera_3d = camera; }
        if scene.opaque.is_empty() { ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Aucune forme pleine visible", egui::FontId::proportional(14.0), Color32::GRAY); }
        if let Some(m) = message { crate::socle::messages::consigne_vue(ui, rect, rect.bottom(), &m); }
    }
}
