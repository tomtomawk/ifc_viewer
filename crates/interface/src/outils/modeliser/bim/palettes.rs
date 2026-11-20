//! Emplacements de la palette de création pour les outils simples : composant, arbre,
//! pièce, niveau, quadrillage. Chaque outil fournit un sélecteur (si son objet a des types)
//! et ses paramètres ; `zones/palette_creation.rs` les appelle dans l'ordre commun.

use super::*;
use super::widgets::*;
use crate::zones::palette_creation::titre_parametres;
use crate::zones::ruban::Outil;
use commandes::Commande;

impl Application {
    /// Composant : type à placer (table par défaut ou famille du projet, hors végétation).
    pub(crate) fn selecteur_composant(&mut self, ui: &mut egui::Ui) {
        self.selecteur_type_composant(ui, false);
    }

    /// Types proposés par l'outil Composant (`vegetal` faux) ou Arbre (vrai) : ceux du
    /// projet, puis les types livrés avec le code qui n'y sont pas encore.
    pub(crate) fn types_outil_composant(&self, vegetal: bool) -> Vec<modele::TypeComposant> {
        let mut types: Vec<modele::TypeComposant> = self.natif.as_ref().map(|(p, _)| p.types_composants.iter().filter(|t| t.vegetal() == vegetal).cloned().collect()).unwrap_or_default();
        for t in modele::elements::composant::types_par_defaut().into_iter().filter(|t| t.vegetal() == vegetal) {
            if !types.iter().any(|x| x.id == t.id) { types.push(t); }
        }
        types
    }

    fn selecteur_type_composant(&mut self, ui: &mut egui::Ui, vegetal: bool) {
        let types = self.types_outil_composant(vegetal);
        let Some((p, _)) = &self.natif else { return };
        let c = if vegetal { &mut self.bim.vegetal } else { &mut self.bim.composant };
        if !types.iter().any(|t| t.id == c.type_composant) {
            c.type_composant = types[0].id.clone();
        }
        let choix = choix_type_composant(ui, &mut c.type_composant, &types, vegetal);
        if let Some(source) = choix.dupliquer.and_then(|id| types.iter().find(|t| t.id == id)) {
            let mut q = p.clone();
            let id = dupliquer_type_composant(&mut q, source);
            match self.appliquer_projet(q) {
                Ok(()) => if vegetal { self.bim.vegetal.type_composant = id } else { self.bim.composant.type_composant = id },
                Err(e) => self.error = Some(e),
            }
        } else if choix.modifier.is_some() || choix.nouvelle {
            self.ouvrir_editeur_composant(choix.modifier, vegetal);
        }
    }

    pub(crate) fn parametres_composant(&mut self, ui: &mut egui::Ui) {
        titre_parametres(ui, "Composant");
        parametres_placement(ui, "outil_composant", &mut self.bim.composant);
    }

    /// Arbre : famille de la catégorie Végétation (ses dimensions sont celles du type).
    pub(crate) fn selecteur_arbre(&mut self, ui: &mut egui::Ui) {
        self.selecteur_type_composant(ui, true);
    }

    pub(crate) fn parametres_arbre(&mut self, ui: &mut egui::Ui) {
        titre_parametres(ui, "Arbre");
        parametres_placement(ui, "outil_arbre", &mut self.bim.vegetal);
    }

    /// Pièce : famille d'étiquette.
    pub(crate) fn selecteur_piece(&mut self, ui: &mut egui::Ui) {
        let familles = self.natif.as_ref().map(|(p, _)| p.familles_etiquettes.clone()).unwrap_or_default();
        choix_famille_etiquette(ui, &mut self.bim.etiquette_piece, &familles);
        let ouvrir = ui.button("Modifier l'étiquette").on_hover_text("Éditer cette famille d'étiquette").clicked();
        if ouvrir {
            self.editer_famille_etiquette(Some(self.bim.etiquette_piece.clone()), None);
        }
    }

    pub(crate) fn parametres_piece(&mut self, ui: &mut egui::Ui) {
        titre_parametres(ui, "Pièce");
        egui::Grid::new("outil_piece").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Nom");
            ui.text_edit_singleline(&mut self.bim.nom_piece);
            ui.end_row();
        });
    }

    /// Niveau : liste des niveaux (nom, altitude) et ajout d'un niveau au-dessus du plus
    /// haut ; dans une coupe ou une élévation, le niveau se place aussi au clic
    /// (`outils/modeliser/niveau.rs`).
    pub(crate) fn parametres_niveau(&mut self, ui: &mut egui::Ui) {
        titre_parametres(ui, "Niveau");
        let Some((p, _)) = &self.natif else { return };
        let mut commande = None;
        for n in &p.niveaux {
            ui.push_id(&n.id, |ui| {
                let mut nom = n.nom.clone();
                let mut altitude = n.altitude;
                ui.horizontal(|ui| {
                    // largeur relative : l'altitude reste visible quel que soit le panneau
                    let largeur = (ui.available_width() - 90.0).clamp(40.0, 180.0);
                    ui.add(egui::TextEdit::singleline(&mut nom).desired_width(largeur));
                    ui.add(egui::DragValue::new(&mut altitude).speed(10.0).dimensions());
                });
                if nom != n.nom || altitude != n.altitude {
                    commande = Some(Commande::ModifierNiveau { id: n.id.clone(), nom, altitude });
                }
            });
        }
        // nouveau niveau au-dessus du plus haut, à la hauteur du dernier étage (3 m s'il n'y en a qu'un)
        let mut alts: Vec<f64> = p.niveaux.iter().map(|n| n.altitude).collect();
        alts.sort_by(f64::total_cmp);
        let etage = match alts.as_slice() {
            [.., a, b] if b - a > 1.0 => b - a,
            _ => 3000.0,
        };
        let altitude = alts.last().map_or(0.0, |h| h + etage);
        let nom = crate::outils::modeliser::niveau::nom_niveau_suivant(p);
        if ui.button("Ajouter un niveau").on_hover_text(format!("« {nom} » à {}", modele::unite::unite().afficher(altitude))).clicked() {
            commande = Some(Commande::Niveau { nom, altitude });
        }
        self.executer_palette(commande);
    }

    /// Quadrillage : repère de la prochaine ligne ; la ligne se trace en deux clics dans
    /// une vue en plan (`outils/modeliser/quadrillage.rs`).
    pub(crate) fn parametres_quadrillage(&mut self, ui: &mut egui::Ui) {
        self.ruban_edition.section_parametres(ui, Outil::Quadrillage, self.natif.as_ref().map(|(p, _)| p));
        if !self.vue_en_plan() {
            ui.label("Ouvrez une vue en plan pour tracer le quadrillage.");
        }
    }

    fn executer_palette(&mut self, commande: Option<Commande>) {
        if let Some(c) = commande
            && let Err(e) = self.executer_bim(c)
        {
            self.error = Some(e);
        }
    }
}

/// Rotation et décalage du composant à placer.
fn parametres_placement(ui: &mut egui::Ui, id: &str, c: &mut modele::Composant) {
    egui::Grid::new(id).num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
        ui.label("Rotation");
        ui.add(egui::DragValue::new(&mut c.rotation).speed(1.0).suffix(" °"));
        ui.end_row();
        ui.label("Décalage").on_hover_text("Hauteur par rapport au niveau de la vue");
        ui.add(egui::DragValue::new(&mut c.decalage).speed(10.0).dimensions());
        ui.end_row();
    });
}
