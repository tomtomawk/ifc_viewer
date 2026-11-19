//! Placement par saisie dans la palette de création : abscisse d'une ouverture, contenu d'un
//! texte, points d'une étiquette ou d'une zone. Paramètres de la palette, après les champs
//! communs de l'outil.

use super::*;
use super::widgets::*;
use crate::zones::palette_creation::titre_parametres;
use crate::zones::ruban::Outil;
use commandes::Commande;

const SANS_VUE: &str = "Ouvrez un plan ou une coupe native pour annoter.";

impl Application {
    fn executer_saisie(&mut self, commande: Option<Commande>) {
        if let Some(c) = commande
            && let Err(e) = self.executer_bim(c)
        {
            self.error = Some(e);
        }
    }

    /// Ouverture : champs de l'outil, puis placement sur l'hôte sélectionné à une abscisse.
    pub(crate) fn parametres_ouverture(&mut self, ui: &mut egui::Ui) {
        self.ruban_edition.section_parametres(ui, Outil::Ouverture, self.natif.as_ref().map(|(p, _)| p));
        let ids = self.selection_bim();
        ui.label("Cliquez sur un mur pour placer l’ouverture, ou sélectionnez son hôte et saisissez l'abscisse.");
        ui.add(egui::DragValue::new(&mut self.bim.abscisse).speed(10.0).dimensions());
        let mut commande = None;
        if ui.add_enabled(ids.len() == 1, egui::Button::new("Placer")).clicked() {
            match self.verifier_type_ouverture() {
                Ok(()) => commande = Some(self.commande_ouverture(ids[0].clone(), self.bim.abscisse)),
                Err(e) => self.error = Some(e),
            }
        }
        self.executer_saisie(commande);
    }

    /// Texte : champs de l'outil et contenu ; le clic dans la vue place le texte (même
    /// parcours que dans l'éditeur de famille).
    pub(crate) fn parametres_texte(&mut self, ui: &mut egui::Ui) {
        self.ruban_edition.section_parametres(ui, Outil::Texte, self.natif.as_ref().map(|(p, _)| p));
        if self.vue_native_id().is_none() {
            ui.label(SANS_VUE);
            return;
        }
        egui::Grid::new("contenu_texte").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Contenu");
            ui.text_edit_singleline(&mut self.bim.texte);
            ui.end_row();
        });
        ui.label(RichText::new("Cliquez dans la vue pour placer le texte. Il reste sélectionné : contenu, champs, position et hauteur se règlent dans Propriétés.").small().weak());
    }

    pub(crate) fn parametres_etiquette(&mut self, ui: &mut egui::Ui) {
        self.ruban_edition.section_parametres(ui, Outil::Etiquette, self.natif.as_ref().map(|(p, _)| p));
        self.formulaire_annotation(ui, Outil::Etiquette);
    }

    pub(crate) fn parametres_zone(&mut self, ui: &mut egui::Ui) {
        self.ruban_edition.section_parametres(ui, Outil::Zone, self.natif.as_ref().map(|(p, _)| p));
        self.formulaire_annotation(ui, Outil::Zone);
    }

    /// Placement par coordonnées d'une étiquette ou d'une zone : deux points, « Ajouter à la vue ».
    fn formulaire_annotation(&mut self, ui: &mut egui::Ui, outil: Outil) {
        use modele::documentation::{Annotation, AnnotationForme as F};
        let Some(vue) = self.vue_native_id() else {
            ui.label(SANS_VUE);
            return;
        };
        titre_parametres(ui, "Placement");
        points_ui(ui, &mut self.bim.points);
        if ui.button("Utiliser le dernier point cliqué").clicked()
            && let Some(pt) = self.point_clique
        {
            let p = &self.natif.as_ref().unwrap().0;
            let v = p.vues.iter().find(|v| v.id == vue).unwrap();
            let (o, u, h) = modele::documentation::repere(p, v);
            let d = pt * 1000.0 - o;
            self.bim.points[0] = [d.dot(u), d.dot(h)];
        }
        let mut commande = None;
        if ui.button("Ajouter à la vue").clicked() {
            let [a, b] = self.bim.points;
            let ids = self.selection_bim();
            let mut forme = match outil {
                Outil::Etiquette => F::Etiquette { element: ids.first().cloned().unwrap_or_default(), point: a },
                Outil::Zone => F::Zone { contour: vec![a, [b[0], a[1]], b, [a[0], b[1]]] },
                _ => F::Ligne { debut: a, fin: b },
            };
            let mut hauteur = 2.5;
            if let Some(modele) = &self.bim.annotation_identique {
                hauteur = modele.hauteur;
                match &modele.forme {
                    F::Etiquette { element, .. } => forme = F::Etiquette { element: element.clone(), point: a },
                    F::Zone { contour } if !contour.is_empty() => {
                        let origine = contour[0];
                        forme = F::Zone { contour: contour.iter().map(|p| [p[0] - origine[0] + a[0], p[1] - origine[1] + a[1]]).collect() };
                    }
                    _ => {}
                }
            }
            commande = Some(Commande::Annoter { annotation: Annotation { id: String::new(), vue, hauteur, forme } });
        }
        ui.label(RichText::new("Une cote créée avec un mur sélectionné suit ses extrémités.").small().weak());
        self.executer_saisie(commande);
    }

    /// Une annotation « créée à l'identique » ne survit pas au changement d'outil ou d'onglet.
    pub(crate) fn oublier_annotation_identique(&mut self) {
        use modele::documentation::AnnotationForme as F;
        let outil = self.ruban_edition.outil();
        let Some(annotation) = &self.bim.annotation_identique else { return };
        let attendu = match annotation.forme {
            F::Texte { .. } => Outil::Texte,
            F::Ligne { .. } | F::Polyligne { .. } | F::Arc { .. } => Outil::Polyligne,
            F::Spline { .. } => Outil::Spline,
            F::Cercle { .. } => Outil::Cercle,
            F::Zone { .. } => Outil::Zone,
            F::Cote { .. } => Outil::Cote,
            F::Etiquette { .. } => Outil::Etiquette,
            F::CoteAngulaire { .. } => Outil::CoteAngulaire,
            F::CoteDiametre { .. } => Outil::CoteDiametre,
            F::Altimetrie { .. } => Outil::Altimetrie,
        };
        if self.onglet_ruban != OngletRuban::Annoter || outil != Some(attendu) {
            self.bim.annotation_identique = None;
        }
    }
}
