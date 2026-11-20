//! Emplacements de la palette de création pour les tracés : terrain (type, cote NGF) et
//! polyligne (type de trait).
use super::*;

impl Application {
    /// Emplacement « sélecteur » du terrain : type de terrain (couches sous la surface).
    pub(crate) fn selecteur_terrain(&mut self, ui: &mut egui::Ui) {
        let Some((p, _)) = &self.natif else { return };
        let types = p.types_terrains_disponibles();
        let courant = self.dessin2d.terrain_type.clone().filter(|id| types.iter().any(|t| &t.id == id)).unwrap_or_else(|| types[0].id.clone());
        let choix = crate::zones::communes::selecteur_type_terrain(ui, p, Some(&courant), 1);
        if let Some(t) = choix.modifier {
            // même éditeur que les types de murs
            self.ruban_edition.ouvrir_editeur_type_terrain(p, &t);
        } else if let Some(source) = choix.dupliquer {
            let mut q = p.clone();
            if let Some(id) = crate::zones::communes::dupliquer_type_terrain(&mut q, &source) {
                match self.appliquer_projet(q) {
                    Ok(()) => self.dessin2d.terrain_type = Some(id),
                    Err(e) => self.error = Some(e),
                }
            }
        } else if let Some(id) = choix.choisi {
            // type par défaut : inscrit dans le projet à la création du terrain
            self.dessin2d.terrain_type = Some(id);
        }
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

    /// Polyligne, spline, cercle : type de trait du tracé créé (chaque segment d'une
    /// polyligne se change ensuite dans Modifier).
    pub(crate) fn selecteur_trait_creation(&mut self, ui: &mut egui::Ui) {
        let Some((projet, _)) = &self.natif else { return };
        if !projet.types_traits.iter().any(|t| t.id == self.dessin2d.trait_creation) {
            self.dessin2d.trait_creation.clear();
        }
        let objet = match self.ruban_edition.outil() {
            Some(crate::zones::ruban::Outil::Spline) => "Spline",
            Some(crate::zones::ruban::Outil::Cercle) => "Cercle",
            _ => "Polyligne",
        };
        if let Some(id) = crate::outils::annoter::types_traits::selecteur_type_trait(ui, Some(&self.dessin2d.trait_creation), projet, objet) {
            self.dessin2d.trait_creation = id;
        }
    }
}
