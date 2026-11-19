//! Niveau de détail et type de la vue ouverte : quels éléments des familles elle montre
//! (`modele::famille::modelee::Affichage`), et réglages communs aux barres de vue du
//! projet et de l'éditeur de familles (échelle, niveau de détail).
use crate::*;

/// Champ d'échelle « 1 : N » des barres de vue (projet et famille).
pub(crate) fn champ_echelle(ui: &mut egui::Ui, echelle: &mut f64, actif: bool) -> egui::Response {
    ui.add_enabled(actif, egui::DragValue::new(echelle).speed(1.0).range(1.0..=10000.0).prefix("1 : "))
}

/// Menu du niveau de détail (Faible, Moyen, Élevé) des barres de vue ; vrai au changement.
pub(crate) fn menu_niveau_detail(ui: &mut egui::Ui, detail: &mut NiveauDetail, aide: &str) -> bool {
    let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::hover());
    icone_detail(ui.painter(), r.shrink(3.0), *detail);
    let mut change = false;
    ui.menu_button(detail.libelle(), |ui| {
        for d in NiveauDetail::TOUS {
            let (r, resp) = ui.allocate_exact_size(egui::vec2(120.0, 24.0), Sense::click());
            let v = ui.visuals();
            if d == *detail {
                ui.painter().rect_filled(r, 3.0, v.selection.bg_fill);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, 3.0, v.widgets.hovered.bg_fill);
            }
            icone_detail(ui.painter(), egui::Rect::from_min_size(r.min + egui::vec2(5.0, 4.0), egui::vec2(16.0, 16.0)), d);
            ui.painter().text(r.min + egui::vec2(30.0, 12.0), egui::Align2::LEFT_CENTER, d.libelle(), egui::TextStyle::Button.resolve(ui.style()), v.text_color());
            if resp.clicked() {
                change = d != *detail;
                *detail = d;
                ui.close();
            }
        }
    })
    .response
    .on_hover_text(aide);
    change
}

impl Application {
    /// Type de la vue ouverte : plan, coupe ou élévation, 3D (sans vue : 3D).
    pub(crate) fn type_vue(&self) -> TypeVue {
        match self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)).map(|v| v.genre.as_str()) {
            None | Some("Vue 3D") => TypeVue::Vue3d,
            Some("Plan d'étage" | "Plan de plafond" | "Plan de structure" | "Plan de site") => TypeVue::Plan,
            Some(_) => TypeVue::Coupe,
        }
    }

    /// Éléments des familles montrés par la vue ouverte.
    pub(crate) fn affichage(&self) -> Affichage {
        Affichage { detail: self.detail, vue: self.type_vue() }
    }

    /// Règle le niveau de détail de la vue ouverte (enregistré dans la vue native).
    pub(crate) fn regler_detail_vue(&mut self, detail: NiveauDetail) -> Result<(), String> {
        if let Some(id) = self.vue_native_id()
            && let Some((projet, _)) = &self.natif
            && let Some(i) = projet.vues.iter().position(|v| v.id == id && v.niveau_detail != detail)
        {
            let mut projet = projet.clone();
            projet.vues[i].niveau_detail = detail;
            self.appliquer_projet(projet)?;
        }
        self.detail = detail;
        self.visibility_dirty = true;
        self.coupe_dirty = true;
        Ok(())
    }

    /// Niveau de détail de la vue (Faible, Moyen, Élevé) : visibilité des éléments des
    /// familles selon leurs réglages.
    pub(crate) fn choix_detail(&mut self, ui: &mut egui::Ui) {
        let mut detail = self.detail;
        if menu_niveau_detail(ui, &mut detail, "Niveau de détail de la vue : montre les éléments des familles visibles à ce niveau") && let Err(e) = self.regler_detail_vue(detail) {
            self.error = Some(e);
        }
    }

    /// Pilote : `niveau_detail [faible|moyen|eleve]` lit ou règle celui de la vue ouverte.
    pub(crate) fn piloter_detail(&mut self, commande: &str, argument: &str) -> Option<Result<serde_json::Value, String>> {
        if commande != "niveau_detail" { return None; }
        Some((|| {
            if !argument.is_empty() {
                self.regler_detail_vue(NiveauDetail::depuis_cle(argument).ok_or("usage : niveau_detail faible|moyen|eleve")?)?;
            }
            Ok(serde_json::json!({"niveau_detail": self.detail.cle(), "type_vue": self.type_vue().libelle()}))
        })())
    }
}
