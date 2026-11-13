//! Affichage du ruban d'édition et contexte de création.
use super::*;

impl RubanEdition {
    pub fn afficher(
        &mut self,
        ui: &mut egui::Ui,
        menu: Menu,
        projet: Option<&crate::modele::Projet>,
    ) -> Option<Outil> {
        let mut action = None;
        let largeur = ui.available_width();
        let outil_options = self.selection.filter(|o| {
            groupes(menu)
                .iter()
                .any(|(_, outils)| outils.contains(&o.commande()))
        });
        // Les surfaces se règlent dans l'encart en tête de Propriétés.
        // Sans réglage (dessin 2D, actions), pas d'encart.
        let outil_options = outil_options.filter(|o| menu != Menu::Cartouche && o.commande() != Outil::Surfaces && !champs(*o).is_empty());
        let selection_options = outil_options.map(Outil::commande);
        let compact = largeur_commandes(ui, menu, &[], false);
        let reserve = if selection_options.is_some() { LARGEUR_REGLAGES + ESPACE_COLONNES } else { 0.0 };
        let serre = compact + reserve > largeur;
        let compact = if serre { largeur_commandes(ui, menu, &[], true) } else { compact };
        let options_inline =
            selection_options.is_some() && compact + LARGEUR_OPTIONS + ESPACE_COLONNES <= largeur;
        let largeur_options = if selection_options.is_none() {
            0.0
        } else if options_inline {
            LARGEUR_OPTIONS + ESPACE_COLONNES
        } else {
            LARGEUR_REGLAGES + ESPACE_COLONNES
        };
        let budget = largeur - largeur_options;
        let mut grands = Vec::new();
        // Les références et les commandes occasionnelles restent toujours empilées.
        // En réduisant la fenêtre, les outils les moins fréquents se replient d'abord.
        let mut candidats: Vec<_> = groupes(menu)
            .iter()
            .flat_map(|(_, outils)| outils.iter().copied())
            .filter(|o| o.priorite() > 0 && largeur >= 1500.0 - 200.0 * o.priorite() as f32)
            .collect();
        candidats.sort_by_key(|o| std::cmp::Reverse(o.priorite()));
        for outil in candidats {
            let mut essai = grands.clone();
            essai.push(outil);
            if largeur_commandes(ui, menu, &essai, serre) <= budget {
                grands = essai;
            }
        }
        // Défilement de secours pour les très petites surfaces / grandes polices.
        // À la taille minimale de l'application, toutes les commandes restent visibles.
        egui::ScrollArea::horizontal()
            .id_salt("ruban_commandes")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = espace_groupes(serre);
                ui.horizontal_top(|ui| {
                    for (indice, (nom, outils)) in groupes(menu).iter().enumerate() {
                        let colonnes = colonnes(ui, outils, &grands, serre);
                        let contient_selection =
                            selection_options.is_some_and(|o| outils.contains(&o));
                        let largeur_groupe = largeur_groupe(ui, nom, &colonnes)
                            + if contient_selection {
                                largeur_options
                            } else {
                                0.0
                            };
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(largeur_groupe, HAUTEUR_GROUPE),
                            Sense::hover(),
                        );
                        if indice > 0 {
                            let x = rect.left() - espace_groupes(serre) / 2.0;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(x, rect.top() + 4.0),
                                    egui::pos2(x, rect.bottom() - 4.0),
                                ],
                                ui.visuals().widgets.noninteractive.bg_stroke,
                            );
                        }
                        let mut x = rect.left();
                        for colonne in colonnes {
                            for (ligne, &outil) in colonne.outils.iter().enumerate() {
                                let hauteur = if colonne.grande {
                                    HAUTEUR_OUTILS
                                } else {
                                    HAUTEUR_OUTILS / 3.0
                                };
                                let bouton = egui::Rect::from_min_size(
                                    egui::pos2(x, rect.top() + ligne as f32 * hauteur),
                                    egui::vec2(colonne.largeur, hauteur),
                                );
                                // terre-plein : son propre bouton (Site), pas celui des éléments horizontaux
                                let actif = if self.edition_traces && menu == Menu::Modifier { self.commande_trace == Some(outil) } else { self.selection == Some(outil) || (self.selection != Some(Outil::TerrePlein) && self.selection.map(Outil::commande) == Some(outil)) };
                                // éléments horizontaux : un bouton ; la sorte se choisit dans le
                                // sélecteur de type de Propriétés
                                let disponible = matches!(outil, Outil::Terrain|Outil::TerrePlein|Outil::LimitePropriete|Outil::Mur|Outil::Surfaces|Outil::Poteau|Outil::Poutre|Outil::Escalier|Outil::GardeCorps|Outil::Niveau|Outil::Quadrillage|Outil::Ouverture|Outil::Composant|Outil::Piece|Outil::Arbre|Outil::Deplacer|Outil::Copier|Outil::Tourner|Outil::Symetrie|Outil::Reseau|Outil::Echelle|Outil::Decaler|Outil::Scinder|Outil::Demolir|Outil::Reproduire|Outil::ChangerHote|Outil::Inverser|Outil::Aligner|Outil::Ajuster|Outil::ModifierContour|Outil::CopierNiveau|Outil::Jonctions|Outil::AttacherToiture|Outil::CreerIdentique|Outil::Supprimer|Outil::Cadrer|Outil::Isoler|Outil::Masquer|Outil::MasquerVue|Outil::Afficher|Outil::Verrouiller|Outil::Deverrouiller|Outil::VuePlan|Outil::VueCoupe|Outil::Texte|Outil::Cote|Outil::CoteAngulaire|Outil::CoteDiametre|Outil::Altimetrie|Outil::Polyligne|Outil::Spline|Outil::Cercle|Outil::Arc|Outil::Zone|Outil::Etiquette|Outil::Feuilles|Outil::Nomenclatures);
                                let mut bouton_ui=ui.new_child(egui::UiBuilder::new().max_rect(bouton));
                                let raison = if !disponible && outil.forme_dessin().is_none() {
                                    Some("Outil prévu, pas encore disponible")
                                } else if projet.is_none() {
                                    Some("Ouvrir ou créer un projet pour utiliser cet outil")
                                } else if self.edition_traces && menu == Menu::Modifier {
                                    raison_trace(outil, self.cible_trace, self.trace_fixe, self.trace_sans_copie)
                                } else if outil == Outil::CopierNiveau && !self.selection_elements {
                                    Some("Sélectionner d'abord un ou plusieurs éléments")
                                } else if outil == Outil::Jonctions && !self.selection_murs {
                                    Some("Sélectionner d'abord un mur")
                                } else {
                                    None
                                };
                                if raison.is_some() {bouton_ui.disable();}
                                let reponse=bouton_outil(&mut bouton_ui,bouton,outil,colonne.grande,serre,actif);
                                if outil == Outil::Jonctions { self.bouton_jonctions = Some(reponse.clone()); }
                                if reponse.on_disabled_hover_text(raison.unwrap_or("")).clicked() {
                                    if outil == Outil::Jonctions { continue; }
                                    if self.edition_traces && menu == Menu::Modifier {
                                        action = Some(outil);
                                        continue;
                                    }
                                    if matches!(outil, Outil::ModifierContour | Outil::CopierNiveau | Outil::CreerIdentique | Outil::Supprimer | Outil::Cadrer | Outil::Isoler | Outil::Masquer | Outil::MasquerVue | Outil::Afficher | Outil::Verrouiller | Outil::Deverrouiller | Outil::Feuilles | Outil::Nomenclatures) {
                                        action = Some(outil);
                                        continue;
                                    }
                                    if outil == Outil::VueCoupe { self.coupe_identique = None; }
                                    if outil == Outil::Surfaces { self.surface_identique = None; }
                                    // l'ouverture garde le dernier type choisi (comme Revit)
                                    self.selection = Some(if outil == Outil::Surfaces {
                                        let surface = self.derniere_surface.unwrap_or(Outil::Dalle);
                                        self.derniere_surface = Some(surface);
                                        surface
                                    } else {
                                        outil
                                    });
                                }
                            }
                            x += colonne.largeur + ESPACE_COLONNES;
                            // Réglages directement à droite de la colonne de l'outil actif.
                            if colonne.outils.iter().any(|o| Some(*o) == selection_options) {
                                let outil = outil_options.unwrap();
                                let reglages = egui::Rect::from_min_size(
                                    egui::pos2(x, rect.top()),
                                    egui::vec2(
                                        if options_inline {
                                            LARGEUR_OPTIONS
                                        } else {
                                            LARGEUR_REGLAGES
                                        },
                                        HAUTEUR_OUTILS,
                                    ),
                                );
                                let mut options_ui = ui.new_child(
                                    egui::UiBuilder::new()
                                        .id_salt(("reglages", outil.commande()))
                                        .max_rect(reglages)
                                        .layout(egui::Layout::top_down(Align::Min)),
                                );
                                {
                                    let ui = &mut options_ui;
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    if options_inline {
                                        self.options_outil(ui, outil, projet);
                                    } else {
                                        let r = ui
                                            .add_sized(
                                                [LARGEUR_REGLAGES, HAUTEUR_OUTILS],
                                                egui::Button::new("..."),
                                            )
                                            .on_hover_text(format!("Réglages — {}", outil.nom()));
                                        egui::Popup::menu(&r)
                                            .close_behavior(
                                                egui::PopupCloseBehavior::CloseOnClickOutside,
                                            )
                                            .show(|ui| {
                                                ui.set_width(LARGEUR_OPTIONS);
                                                ui.label(
                                                    RichText::new(format!(
                                                        "Réglages — {}",
                                                        outil.nom()
                                                    ))
                                                    .strong(),
                                                );
                                                ui.separator();
                                                self.options_outil(ui, outil, projet);
                                            });
                                    }
                                }
                                x += largeur_options;
                            }
                        }
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.bottom() - 8.0),
                            egui::Align2::CENTER_CENTER,
                            *nom,
                            egui::FontId::proportional(11.0),
                            ui.visuals().weak_text_color(),
                        );
                    }
                    if menu == Menu::Cartouche {
                        bandeau_contenu_compact(ui, &[("Formes", formes_famille_planes())]);
                    }
                });
            });
        action
    }

    pub(super) fn proposer_ajout_creation(
        &mut self,
        ui: &mut egui::Ui,
        liste: crate::modele::edition::ListeCreation,
    ) {
        ui.separator();
        if ui
            .button(format!("+ Ajouter : {}…", liste.libelle()))
            .clicked()
        {
            self.ajout_creation = Some(liste);
            self.nom_creation.clear();
            self.jeu_creation.clear();
            self.erreur_creation = None;
            ui.close();
        }
    }

    pub fn reinitialiser_contexte_creation(&mut self) {
        self.phase_creation = None;
        self.sous_projet_creation = None;
        self.variante_creation = None;
        self.ajout_creation = None;
        self.erreur_creation = None;
    }

    /// Contexte commun aux créations, affiché indépendamment de l'outil actif.
    /// Seuls les ajouts modifient le document ; choisir une valeur règle les prochaines créations.
    pub fn contexte_creation(
        &mut self,
        ui: &mut egui::Ui,
        projet: &mut crate::modele::Projet,
    ) -> bool {
        use crate::modele::edition::ListeCreation;
        let phases = if projet.phases.is_empty() {
            vec!["Projet".into()]
        } else {
            projet.phases.clone()
        };
        let sous_projets = if projet.sous_projets.is_empty() {
            vec!["A".into()]
        } else {
            projet.sous_projets.clone()
        };
        if self
            .phase_creation
            .as_ref()
            .is_none_or(|v| !phases.contains(v))
        {
            self.phase_creation = phases.last().cloned();
        }
        if self
            .sous_projet_creation
            .as_ref()
            .is_none_or(|v| !sous_projets.contains(v))
        {
            self.sous_projet_creation = sous_projets.first().cloned();
        }
        if self
            .variante_creation
            .as_ref()
            .is_some_and(|id| !projet.variantes.iter().any(|v| &v.id == id))
        {
            self.variante_creation = None;
        }
        let largeur = ((ui.available_width() - 8.0) / 3.0).max(30.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.vertical(|ui| {
                ui.set_width(largeur);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(RichText::new("Phase").small()).truncate());
                egui::ComboBox::from_id_salt("phase_creation")
                    .width(largeur)
                    .truncate()
                    .selected_text(self.phase_creation.as_deref().unwrap_or("Projet"))
                    .show_ui(ui, |ui| {
                        for nom in &phases {
                            ui.selectable_value(&mut self.phase_creation, Some(nom.clone()), nom);
                        }
                        self.proposer_ajout_creation(ui, ListeCreation::Phase);
                    })
                    .response
                    .on_hover_text(format!(
                        "Phase des prochaines créations : {}",
                        self.phase_creation.as_deref().unwrap_or("Projet")
                    ));
            });
            ui.vertical(|ui| {
                ui.set_width(largeur);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(RichText::new("Sous-projet").small()).truncate());
                egui::ComboBox::from_id_salt("sous_projet_creation")
                    .width(largeur)
                    .truncate()
                    .selected_text(
                        self.sous_projet_creation
                            .as_deref()
                            .unwrap_or("A"),
                    )
                    .show_ui(ui, |ui| {
                        for nom in &sous_projets {
                            ui.selectable_value(
                                &mut self.sous_projet_creation,
                                Some(nom.clone()),
                                nom,
                            );
                        }
                        self.proposer_ajout_creation(ui, ListeCreation::SousProjet);
                    })
                    .response
                    .on_hover_text(format!(
                        "Sous-projet des prochaines créations : {}",
                        self.sous_projet_creation
                            .as_deref()
                            .unwrap_or("A")
                    ));
            });
            let nom = self
                .variante_creation
                .as_ref()
                .and_then(|id| projet.variantes.iter().find(|v| &v.id == id))
                .map_or_else(|| "Principale".into(), |v| v.libelle());
            ui.vertical(|ui| {
                ui.set_width(largeur);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(RichText::new("Variante").small()).truncate());
                egui::ComboBox::from_id_salt("variante_creation")
                    .width(largeur)
                    .truncate()
                    .selected_text(&nom)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.variante_creation, None, "Principale");
                        for v in &projet.variantes {
                            ui.selectable_value(
                                &mut self.variante_creation,
                                Some(v.id.clone()),
                                v.libelle(),
                            );
                        }
                        self.proposer_ajout_creation(ui, ListeCreation::Variante);
                    })
                    .response
                    .on_hover_text(format!("Variante des prochaines créations : {nom}"));
            });
        });
        let mut modifie = false;
        if let Some(liste) = self.ajout_creation {
            let mut ouvert = true;
            let mut valider = false;
            let mut annuler = false;
            egui::Window::new(format!("Ajouter : {}", liste.libelle()))
                .id(egui::Id::unique("ajout_contexte_creation"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .default_width(280.0)
                .open(&mut ouvert)
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    egui::Grid::new("grille_ajout_creation").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("Nom");
                        ui.add(egui::TextEdit::singleline(&mut self.nom_creation).desired_width(180.0).hint_text(liste.libelle()));
                        ui.end_row();
                        if liste == ListeCreation::Variante {
                            ui.label("Jeu");
                            ui.add(egui::TextEdit::singleline(&mut self.jeu_creation).desired_width(180.0).hint_text("Jeu de variantes"));
                            ui.end_row();
                        }
                    });
                    if let Some(erreur) = &self.erreur_creation {
                        ui.colored_label(ui.visuals().error_fg_color, erreur);
                    }
                    ui.add_space(4.0);
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), egui::Layout::right_to_left(Align::Center), |ui| {
                        annuler = ui.button("Annuler").clicked();
                        valider = ui.add_enabled(!self.nom_creation.trim().is_empty(), egui::Button::new(RichText::new("Ajouter").strong())).clicked();
                    });
                    // touches consommées : Échap ne doit pas aussi quitter l'outil en cours
                    let (entree, echap) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::Enter), i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)));
                    valider |= entree && !self.nom_creation.trim().is_empty();
                    annuler |= echap;
                });
            if valider {
                match projet.ajouter_choix_creation(liste, &self.nom_creation, &self.jeu_creation) {
                    Ok(valeur) => {
                        match liste {
                            ListeCreation::Phase => self.phase_creation = Some(valeur),
                            ListeCreation::SousProjet => self.sous_projet_creation = Some(valeur),
                            ListeCreation::Variante => self.variante_creation = Some(valeur),
                        }
                        modifie = true;
                        self.ajout_creation = None;
                    }
                    Err(e) => self.erreur_creation = Some(e),
                }
            }
            if !ouvert || annuler {
                self.ajout_creation = None;
            }
        }
        modifie
    }

}
