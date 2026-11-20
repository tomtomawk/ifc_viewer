//! Réglages de l'outil actif dans le bandeau de dessin, pour les outils qui n'y avaient
//! rien (cotes, altimétrie, texte, étiquette, quadrillage, poteau, composant, arbre,
//! pièce, spline, cercle) : les mêmes valeurs que dans Propriétés, sur une ligne. Les
//! tracés (mur, dalle, polyligne…) et les outils de Modifier gardent leurs options.
use crate::*;
use crate::zones::ruban::Outil;

impl Application {
    /// L'outil actif a des réglages à montrer dans le bandeau (hors tracés et Modifier).
    pub(crate) fn a_options_outil(&self) -> bool {
        self.natif.is_some() && matches!(self.ruban_edition.outil(),
            Some(Outil::Cote | Outil::CoteAngulaire | Outil::Altimetrie | Outil::Texte | Outil::Etiquette | Outil::Quadrillage
                | Outil::Poteau | Outil::Composant | Outil::Arbre | Outil::Piece | Outil::Spline | Outil::Cercle))
    }

    /// Réglages de l'outil actif, sur une ligne.
    pub(crate) fn options_outil(&mut self, ui: &mut egui::Ui) {
        let Some(outil) = self.ruban_edition.outil() else { return };
        let dim = |ui: &mut egui::Ui, nom: &str, v: &mut f64| {
            ui.label(nom);
            ui.add(egui::DragValue::new(v).speed(10.0).dimensions());
        };
        match outil {
            Outil::Composant => {
                let c = &mut self.bim.composant;
                ui.label("Rotation");
                ui.add(egui::DragValue::new(&mut c.rotation).speed(1.0).suffix(" °")).on_hover_text("Espace : +90°");
                dim(ui, "Décalage", &mut c.decalage);
            }
            Outil::Arbre => {
                let c = &mut self.bim.vegetal;
                ui.label("Rotation");
                ui.add(egui::DragValue::new(&mut c.rotation).speed(1.0).suffix(" °"));
                dim(ui, "Décalage", &mut c.decalage);
            }
            Outil::Piece => {
                ui.label("Nom");
                ui.add(egui::TextEdit::singleline(&mut self.bim.nom_piece).desired_width(110.0));
            }
            Outil::Poteau => {
                let b = &mut self.bim.barre;
                dim(ui, "Largeur", &mut b.largeur);
                dim(ui, "Profondeur", &mut b.hauteur);
                ui.label("Rotation");
                ui.add(egui::DragValue::new(&mut b.rotation).speed(1.0).suffix(" °"));
            }
            Outil::Spline | Outil::Cercle => {
                let Some((projet, _)) = &self.natif else { return };
                ui.label("Type de trait");
                if let Some(id) = crate::outils::annoter::types_traits::choix_type_trait(ui, "trait_bandeau", Some(&self.dessin2d.trait_creation), &projet.types_traits) {
                    self.dessin2d.trait_creation = id;
                }
            }
            _ => {
                let projet = self.natif.as_ref().map(|(p, _)| p);
                self.ruban_edition.options_bandeau(ui, outil, projet);
            }
        }
    }
}
