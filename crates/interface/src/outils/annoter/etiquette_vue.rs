//! Outil Étiquette du projet : un clic sur un élément de la vue native pose son étiquette
//! au point cliqué (une transaction) ; l'outil reste actif pour l'élément suivant. Au
//! survol, l'élément visé et le texte de l'étiquette s'affichent au curseur.
use crate::*;
use crate::saisie::candidats::Cible;
use modele::documentation::{Annotation, AnnotationForme};

impl Application {
    /// Outil Étiquette actif sur une vue native (onglet Annoter).
    pub(crate) fn etiquette_annotation_actif(&self) -> bool {
        self.onglet_ruban == OngletRuban::Annoter && self.natif.is_some() && self.loading.is_none()
            && self.ruban_edition.outil() == Some(crate::zones::ruban::Outil::Etiquette) && self.vue_native_id().is_some()
    }

    /// Élément étiquetable sous le curseur et son nom (texte de l'étiquette) : mur, élément
    /// horizontal, poteau ou poutre, élément importé.
    fn element_etiquetable(&self, rect: egui::Rect, q: egui::Pos2) -> Option<(String, String)> {
        let (p, _) = self.natif.as_ref()?;
        self.candidats_selection(rect, q).into_iter().find_map(|c| {
            let id = match c.cible {
                Cible::Objet(objet) => self.cle_native(self.cible_clic(objet))?,
                Cible::Natif(id) => id,
            };
            let nom = p.murs.iter().find(|m| m.id == id).map(|m| &m.nom)
                .or_else(|| p.dalles.iter().find(|d| d.id == id).map(|d| &d.nom))
                .or_else(|| p.barres.iter().find(|b| b.id == id).map(|b| &b.nom))
                .or_else(|| p.importes.iter().find(|e| e.id == id).map(|e| &e.nom))?;
            Some((id, nom.clone()))
        })
    }

    /// Clic de l'outil Étiquette : étiquette de l'élément visé, au point cliqué.
    pub(crate) fn placer_etiquette_vue(&mut self, rect: egui::Rect, q: egui::Pos2) -> Result<(), String> {
        let vue = self.vue_native_id().ok_or("Ouvrez un plan ou une coupe native pour annoter.")?;
        let (element, nom) = self.element_etiquetable(rect, q).ok_or("Étiquette : cliquer sur un mur, un élément horizontal, un poteau ou une poutre.")?;
        let point = self.point_annotation(rect, q).ok_or("Le plan de la vue est vu par la tranche : ouvrir un plan, une coupe ou une élévation.")?;
        let hauteur = self.bim.annotation_identique.as_ref()
            .filter(|a| matches!(a.forme, AnnotationForme::Etiquette { .. }))
            .map_or_else(|| self.ruban_edition.nombre_option(crate::zones::ruban::Outil::Etiquette, 0), |a| a.hauteur);
        self.executer_bim(commandes::Commande::Annoter {
            annotation: Annotation { id: String::new(), vue, hauteur, forme: AnnotationForme::Etiquette { element, point } },
        })?;
        self.status = format!("Étiquette « {nom} » créée — cliquer l'élément suivant ; Ctrl+Z pour annuler.");
        Ok(())
    }

    /// Survol de l'outil Étiquette : consigne, et nom de l'élément visé au curseur.
    pub(crate) fn apercu_etiquette_vue(&self, ui: &egui::Ui, rect: egui::Rect, survol: Option<egui::Pos2>) {
        if !self.message_visible() {
            crate::socle::messages::consigne_vue(ui, rect, self.dessin2d.bas_barre, "Étiquette : cliquer sur un élément, au point où placer son étiquette · Échap ou clic droit : quitter");
        }
        let Some(q) = survol.filter(|q| rect.contains(*q)) else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let (texte, couleur) = match self.element_etiquetable(rect, q) {
            Some((_, nom)) => (nom, Color32::from_rgb(0, 90, 180)),
            None => ("aucun élément à étiqueter".into(), Color32::from_gray(140)),
        };
        painter.text(q + egui::vec2(14.0, -6.0), egui::Align2::LEFT_BOTTOM, texte, egui::FontId::proportional(13.0), couleur);
    }
}
