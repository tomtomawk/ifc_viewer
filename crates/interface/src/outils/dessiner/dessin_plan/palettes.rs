//! Emplacements de la palette de création pour les tracés : terrain (type, cote NGF) et
//! polyligne (type de trait).
use super::*;

impl Application {
    /// Emplacement « sélecteur » du terrain : type de terrain (couches sous la surface).
    pub(crate) fn selecteur_terrain(&mut self, ui: &mut egui::Ui) {
        let Some((p, _)) = &self.natif else { return };
        let types = p.types_terrains_disponibles();
        let actuel = self.dessin2d.terrain_type.as_ref().and_then(|id| types.iter().find(|t| &t.id == id)).or(types.first());
        ui.label(RichText::new("Type de terrain").small().weak());
        egui::ComboBox::from_id_salt("type_terrain_creation_proprietes").width((ui.available_width() - 8.0).max(1.0)).selected_text(actuel.map_or("Terrain", |t| t.nom.as_str())).show_ui(ui, |ui| {
            for t in &types {
                if ui.selectable_label(actuel.is_some_and(|a| a.id == t.id), &t.nom).clicked() {
                    self.dessin2d.terrain_type = (!p.types_terrains.is_empty()).then(|| t.id.clone());
                }
            }
        });
    }

    /// Emplacement « paramètres » du terrain : cote NGF donnée aux prochains points.
    pub(crate) fn parametres_terrain(&mut self, ui: &mut egui::Ui) {
        crate::zones::palette_creation::titre_parametres(ui, "Terrain");
        egui::Grid::new("parametres_terrain").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Cote NGF des points");
            ui.add(egui::DragValue::new(&mut self.dessin2d.terrain_altitude).speed(10.0).dimensions());
            ui.end_row();
        });
    }

    /// Polyligne : type de trait des segments de la polyligne créée (chaque segment se change
    /// ensuite dans Modifier).
    pub(crate) fn selecteur_trait_creation(&mut self, ui: &mut egui::Ui) {
        let Some((projet, _)) = &self.natif else { return };
        if !projet.types_traits.iter().any(|t| t.id == self.dessin2d.trait_creation) {
            self.dessin2d.trait_creation.clear();
        }
        if let Some(id) = crate::outils::annoter::types_traits::selecteur_type_trait(ui, Some(&self.dessin2d.trait_creation), projet, "Polyligne") {
            self.dessin2d.trait_creation = id;
        }
    }
}
