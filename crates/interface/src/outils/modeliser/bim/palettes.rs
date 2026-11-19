//! Emplacements de la palette de création pour les outils simples : composant, arbre,
//! pièce, niveau, quadrillage. Chaque outil fournit un sélecteur (si son objet a des types)
//! et ses paramètres ; `zones/palette_creation.rs` les appelle dans l'ordre commun.

use super::*;
use super::widgets::*;
use crate::zones::palette_creation::titre_parametres;
use crate::zones::ruban::Outil;
use commandes::Commande;

impl Application {
    /// Composant : type à placer (table par défaut ou famille du projet).
    pub(crate) fn selecteur_composant(&mut self, ui: &mut egui::Ui) {
        let Some((p, _)) = &self.natif else { return };
        let mut types = p.types_composants.clone();
        let defaut = modele::elements::composant::type_table();
        if !types.iter().any(|t| t.id == defaut.id) {
            types.insert(0, defaut);
        }
        let c = &mut self.bim.composant;
        if !types.iter().any(|t| t.id == c.type_composant) {
            c.type_composant = types[0].id.clone();
        }
        ui.label(RichText::new("Type de composant").small().weak());
        choix_type_composant(ui, "type_composant_outil", &mut c.type_composant, &types);
        let choisi = c.type_composant.clone();
        let mut ouvrir: Option<Option<String>> = None;
        ui.horizontal(|ui| {
            if ui.button("Modifier la famille").on_hover_text("Ouvre la famille du type dans l'éditeur ; « Charger dans le projet » met à jour tous ses composants").clicked() {
                ouvrir = Some(Some(choisi));
            }
            if ui.button("Nouvelle famille").on_hover_text("Nouvelle famille de composant dans l'éditeur, sur la base de la table").clicked() {
                ouvrir = Some(None);
            }
        });
        if let Some(t) = ouvrir {
            self.ouvrir_editeur_composant(t);
        }
    }

    pub(crate) fn parametres_composant(&mut self, ui: &mut egui::Ui) {
        titre_parametres(ui, "Composant");
        let c = &mut self.bim.composant;
        egui::Grid::new("outil_composant").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Rotation");
            ui.add(egui::DragValue::new(&mut c.rotation).speed(1.0).suffix(" °"));
            ui.end_row();
            ui.label("Décalage");
            ui.add(egui::DragValue::new(&mut c.decalage).speed(10.0).dimensions());
            ui.end_row();
        });
    }

    /// Arbre : essence (elle règle la hauteur et le houppier par défaut).
    pub(crate) fn selecteur_arbre(&mut self, ui: &mut egui::Ui) {
        use modele::elements::arbre::Essence;
        let a = &mut self.bim.arbre;
        ui.label(RichText::new("Essence").small().weak());
        let avant = a.essence;
        egui::ComboBox::from_id_salt("essence_arbre").width((ui.available_width() - 8.0).max(1.0)).selected_text(a.essence.nom()).show_ui(ui, |ui| {
            for e in Essence::TOUTES {
                ui.selectable_value(&mut a.essence, e, e.nom());
            }
        });
        if a.essence != avant {
            (a.hauteur, a.diametre) = a.essence.dimensions();
        }
    }

    pub(crate) fn parametres_arbre(&mut self, ui: &mut egui::Ui) {
        titre_parametres(ui, "Arbre");
        let a = &mut self.bim.arbre;
        egui::Grid::new("outil_arbre").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Hauteur");
            ui.add(egui::DragValue::new(&mut a.hauteur).speed(50.0).range(500.0..=60_000.0).dimensions());
            ui.end_row();
            ui.label("Houppier").on_hover_text("Diamètre du houppier");
            ui.add(egui::DragValue::new(&mut a.diametre).speed(50.0).range(300.0..=40_000.0).dimensions());
            ui.end_row();
            ui.label("Décalage").on_hover_text("Hauteur du pied par rapport au niveau de la vue");
            ui.add(egui::DragValue::new(&mut a.decalage).speed(10.0).dimensions());
            ui.end_row();
        });
    }

    /// Pièce : famille d'étiquette.
    pub(crate) fn selecteur_piece(&mut self, ui: &mut egui::Ui) {
        let familles = self.natif.as_ref().map(|(p, _)| p.familles_etiquettes.clone()).unwrap_or_default();
        ui.label(RichText::new("Étiquette").small().weak());
        let mut ouvrir = false;
        ui.horizontal(|ui| {
            choix_famille_etiquette(ui, "etiquette_piece_outil", &mut self.bim.etiquette_piece, &familles);
            ouvrir = ui.button("Modifier").on_hover_text("Éditer cette famille d'étiquette").clicked();
        });
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

    /// Niveau : liste des niveaux (nom, altitude) et ajout d'un niveau.
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
        let nom = if p.niveaux.is_empty() { "RDC".to_string() } else { (1..).map(|k| format!("R+{k}")).find(|n| !p.niveaux.iter().any(|x| &x.nom == n)).unwrap_or_default() };
        if ui.button("Ajouter un niveau").on_hover_text(format!("« {nom} » à {}", modele::unite::unite().afficher(altitude))).clicked() {
            commande = Some(Commande::Niveau { nom, altitude });
        }
        self.executer_palette(commande);
    }

    /// Quadrillage : repère et tracé (champs communs), puis les deux points et la création.
    pub(crate) fn parametres_quadrillage(&mut self, ui: &mut egui::Ui) {
        self.ruban_edition.section_parametres(ui, Outil::Quadrillage, self.natif.as_ref().map(|(p, _)| p));
        points_ui(ui, &mut self.bim.points);
        let mut commande = None;
        if ui.button("Créer le quadrillage droit").clicked() {
            commande = Some(Commande::Quadrillage {
                nom: self.ruban_edition.texte_option(Outil::Quadrillage, 0),
                debut: self.bim.points[0],
                fin: self.bim.points[1],
            });
        }
        self.executer_palette(commande);
    }

    fn executer_palette(&mut self, commande: Option<Commande>) {
        if let Some(c) = commande
            && let Err(e) = self.executer_bim(c)
        {
            self.error = Some(e);
        }
    }
}
