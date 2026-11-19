//! Type de la vue active (3D, plan) et navigation à la souris : orbite, panoramique, vol.
use crate::*;

impl Application {
    /// Glisser dans la vue : en 3D, orbite à Maj + bouton du milieu (comme Revit),
    /// panoramique au bouton du milieu, regard sur place (vol) au bouton droit ; le
    /// bouton gauche ne navigue pas (sélection, rectangle). Panoramique aux boutons
    /// gauche, droit et milieu dans une vue 2D (plan, coupe, élévation, feuille), dont
    /// l'orientation est celle de la vue Revit.
    pub(crate) fn glisser(&mut self, bouton: PointerButton, d: egui::Vec2, shift: bool, hauteur: f32) {
        if self.vue_3d() {
            match bouton {
                PointerButton::Middle if shift => self.camera.orbit(d.x, d.y),
                PointerButton::Middle => self.camera.pan(d.x, d.y, hauteur),
                PointerButton::Secondary => {
                    self.camera.ortho = false;
                    self.camera.regarder(d.x, d.y);
                }
                _ => {}
            }
        } else {
            self.camera.pan(d.x, d.y, hauteur);
        }
    }

    /// Navigation en vol (vue 3D) : tant que le bouton droit est enfoncé sur la vue, la
    /// souris oriente le regard, W/S (Z/S en AZERTY) avancent et reculent, A/D (Q/D) vont à
    /// gauche et à droite, Q/E (A/E) descendent et montent ; Maj triple la vitesse, la
    /// molette la règle. Comme dans la vue Scène d'Unity.
    pub(crate) fn voler(&mut self, ui: &egui::Ui, resp: &egui::Response) {
        let enfonce = self.vue_3d() && resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.button_down(PointerButton::Secondary));
        if !enfonce {
            self.vol.actif = false;
            self.vol.touches.clear();
            return;
        }
        if !self.vol.actif {
            self.vol.actif = true;
            self.camera.ortho = false;
            self.camera.rapprocher_cible(if self.pieton { 3.0 } else { (self.camera.scene_radius * 0.25).max(1.0) });
        }
        let (delta, molette, maj, dt) = ui.input(|i| {
            for e in &i.events {
                if let egui::Event::Key { key, physical_key, pressed, .. } = e {
                    let k = physical_key.unwrap_or(*key);
                    if *pressed {
                        self.vol.touches.insert(k);
                    } else {
                        self.vol.touches.remove(&k);
                    }
                }
            }
            (i.pointer.delta(), i.smooth_scroll_delta.y, i.modifiers.shift, i.stable_dt.min(0.1))
        });
        if delta != egui::Vec2::ZERO {
            self.camera.regarder(delta.x, delta.y);
        }
        if molette != 0.0 {
            self.vol.vitesse = (self.vol.vitesse * (molette * 0.004).exp()).clamp(0.02, 50.0);
        }
        let dir = self.vol.direction();
        if dir != Vec3::ZERO {
            let f = self.camera.forward();
            let droite = f.cross(Vec3::Z).normalize_or(Vec3::X);
            if self.pieton {
                // marche : à l'horizontale, 1,4 m/s (×3 avec Maj), l'œil suit le sol
                let avant = Vec3::new(f.x, f.y, 0.0).normalize_or(Vec3::X);
                let v = 1.4 * self.vol.vitesse * if maj { 3.0 } else { 1.0 };
                self.marcher((avant * dir.x + droite * dir.y) * v * dt);
            } else {
                let v = (self.camera.scene_radius * 0.3).max(1.0) * self.vol.vitesse * if maj { 3.0 } else { 1.0 };
                self.camera.deplacer((f * dir.x + droite * dir.y + Vec3::Z * dir.z) * v * dt);
            }
            ui.ctx().request_repaint();
        }
    }

    /// Vue 3D : modèle libre (IFC, ou fichier Revit sans vue ouverte) ou vue 3D Revit ;
    /// pas un plan, une coupe, une élévation ni une feuille.
    pub(crate) fn vue_3d(&self) -> bool {
        match self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            Some(v) => v.genre == "Vue 3D",
            None => true,
        }
    }

    /// Plans Revit et vues orthographiques du modèle regardées verticalement.
    pub(crate) fn vue_en_plan(&self) -> bool {
        if let Some(v) = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            return matches!(v.genre.as_str(), "Plan d'étage" | "Plan de plafond" | "Plan de structure" | "Plan de site");
        }
        self.camera.ortho && self.camera.pitch.abs() > 1.56
    }

    pub(crate) fn orientation_initiale_plan(&self) -> f32 {
        self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i))
            .map_or(-std::f32::consts::FRAC_PI_2, |v| (-v.haut.y as f32).atan2(-v.haut.x as f32))
    }
}
