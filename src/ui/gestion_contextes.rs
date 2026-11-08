//! Gestion des listes de contexte et de leurs affectations dans le projet.
use super::*;
use crate::modele::edition::ListeCreation;

#[derive(Clone)]
pub(super) struct GestionContextes {
    liste: ListeCreation,
    selection: Option<String>,
    ajout: bool,
    nom: String,
    jeu: String,
    nouveau_jeu: String,
    remplacement: Option<String>,
    erreur: Option<String>,
}

impl Default for GestionContextes {
    fn default() -> Self {
        Self { liste: ListeCreation::Phase, selection: None, ajout: true, nom: String::new(), jeu: String::new(), nouveau_jeu: String::new(), remplacement: None, erreur: None }
    }
}

enum Action {
    Ajouter,
    Renommer(String),
    Supprimer(String),
    Deplacer(String, isize),
    RenommerJeu(String),
}

impl ViewerApp {
    pub(super) fn ouvrir_gestion_contextes(&mut self) {
        if self.natif.is_some() { self.gestion_contextes = Some(GestionContextes::default()); }
    }

    pub(super) fn fenetre_gestion_contextes(&mut self, ctx: &egui::Context) {
        let Some(mut etat) = self.gestion_contextes.take() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let mut ouvert = true;
        let mut action = None;
        let mut filtres = self.filtres_contextes.clone();
        let mut selection = None;
        let mut nouvel_onglet = None;
        egui::Window::new("Phases, sous-projets et variantes")
            .id(egui::Id::unique("gestion_contextes_projet"))
            .default_size([610.0, 430.0]).min_width(520.0).open(&mut ouvert).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (liste, libelle) in [(ListeCreation::Phase, "Phases"), (ListeCreation::SousProjet, "Sous-projets"), (ListeCreation::Variante, "Variantes")] {
                        if ui.selectable_label(etat.liste == liste, libelle).clicked() { nouvel_onglet = Some(liste); }
                    }
                });
                ui.separator();
                ui.label(match etat.liste {
                    ListeCreation::Phase => "L'ordre des phases définit la chronologie du projet.",
                    ListeCreation::SousProjet => "Les sous-projets regroupent les éléments du projet.",
                    ListeCreation::Variante => "Les variantes sont organisées en jeux ; les éléments sont affectés à une variante ou au modèle principal.",
                });
                let entrees: Vec<(String, String)> = match etat.liste {
                    ListeCreation::Phase => projet.phases.iter().map(|n| (n.clone(), n.clone())).collect(),
                    ListeCreation::SousProjet => projet.sous_projets.iter().map(|n| (n.clone(), n.clone())).collect(),
                    ListeCreation::Variante => {
                        let mut v: Vec<_> = projet.variantes.iter().map(|v| (v.id.clone(), v.libelle())).collect();
                        v.sort_by(|a,b| a.1.to_lowercase().cmp(&b.1.to_lowercase())); v
                    }
                };
                ui.columns(2, |colonnes| {
                    let ui = &mut colonnes[0];
                    ui.horizontal(|ui| {
                        ui.strong(format!("{} ({})", match etat.liste { ListeCreation::Phase => "Phases", ListeCreation::SousProjet => "Sous-projets", ListeCreation::Variante => "Variantes" }, entrees.len()));
                        if ui.button("+ Ajouter").clicked() { selection = Some(None); }
                    });
                    egui::ScrollArea::vertical().id_salt("liste_contextes").max_height(310.0).show(ui, |ui| {
                        for (cle, nom) in &entrees {
                            let nombre = projet.utilisations_contexte(etat.liste, cle);
                            if ui.selectable_label(etat.selection.as_ref() == Some(cle) && !etat.ajout, format!("{nom}  ·  {nombre}")).on_hover_text("Nombre d'affectations, y compris les couches de murs").clicked() {
                                selection = Some(Some(cle.clone()));
                            }
                        }
                    });
                    let ui = &mut colonnes[1];
                    ui.strong(if etat.ajout { "Nouvelle entrée" } else { "Entrée sélectionnée" });
                    ui.add_space(6.0);
                    ui.label("Nom");
                    ui.add(egui::TextEdit::singleline(&mut etat.nom).desired_width(f32::INFINITY));
                    if etat.liste == ListeCreation::Variante {
                        ui.label("Jeu de variantes");
                        let jeux: Vec<String> = {
                            let mut v: Vec<_> = projet.variantes.iter().map(|v| v.jeu.clone()).filter(|s| !s.is_empty()).collect();
                            v.sort(); v.dedup(); v
                        };
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut etat.jeu).desired_width(150.0).hint_text("Nom du jeu"));
                            egui::ComboBox::from_id_salt("choix_jeu_variante").selected_text("Jeux…").show_ui(ui, |ui| {
                                for jeu in &jeux { if ui.selectable_label(false, jeu).clicked() { etat.jeu = jeu.clone(); } }
                            });
                        });
                    }
                    ui.add_space(6.0);
                    if etat.ajout {
                        if ui.button("Ajouter").clicked() { action = Some(Action::Ajouter); }
                    } else if let Some(cle) = etat.selection.clone() {
                        if ui.button("Enregistrer les modifications").clicked() { action = Some(Action::Renommer(cle.clone())); }
                        if etat.liste == ListeCreation::Phase {
                            ui.horizontal(|ui| {
                                if ui.button("↑ Monter").clicked() { action = Some(Action::Deplacer(cle.clone(), -1)); }
                                if ui.button("↓ Descendre").clicked() { action = Some(Action::Deplacer(cle.clone(), 1)); }
                            });
                        }
                        if etat.liste == ListeCreation::Variante && let Some(variante) = projet.variantes.iter().find(|v| v.id == cle) {
                            ui.separator();
                            ui.label("Renommer tout le jeu");
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut etat.nouveau_jeu).desired_width(125.0));
                                if ui.button("Renommer").clicked() { action = Some(Action::RenommerJeu(variante.jeu.clone())); }
                            });
                        }
                        ui.separator();
                        ui.label(format!("Réaffecter {} affectation(s) vers :", projet.utilisations_contexte(etat.liste, &cle)));
                        let groupe = projet.variantes.iter().find(|v| v.id == cle).map(|v| v.jeu.as_str());
                        let options: Vec<_> = entrees.iter().filter(|(id, _)| id != &cle && (etat.liste != ListeCreation::Variante || projet.variantes.iter().find(|v| &v.id == id).is_some_and(|v| Some(v.jeu.as_str()) == groupe))).collect();
                        if etat.liste != ListeCreation::Variante && etat.remplacement.as_ref().is_none_or(|r| !options.iter().any(|(id,_)| id == r)) { etat.remplacement = options.first().map(|(id,_)| id.clone()); }
                        let libelle = etat.remplacement.as_ref().and_then(|id| options.iter().find(|(x,_)| x == id).map(|(_,n)| n.as_str())).unwrap_or(if etat.liste == ListeCreation::Variante { "Modèle principal" } else { "Aucun remplacement" });
                        egui::ComboBox::from_id_salt("remplacement_contexte").selected_text(libelle).width(200.0).show_ui(ui, |ui| {
                            if etat.liste == ListeCreation::Variante { ui.selectable_value(&mut etat.remplacement, None, "Modèle principal"); }
                            for (id, nom) in &options { ui.selectable_value(&mut etat.remplacement, Some(id.clone()), nom); }
                        });
                        if ui.button("Supprimer et réaffecter").clicked() { action = Some(Action::Supprimer(cle)); }
                    }
                });
                ui.separator();
                ui.collapsing("Affichage temporaire", |ui| {
                    ui.label("Ces filtres s'appliquent aux vues du projet pendant la session. Les éléments sans affectation restent visibles.");
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Phase");
                        egui::ComboBox::from_id_salt("filtre_phase_contexte").selected_text(filtres.phase.as_deref().unwrap_or("Toutes")).show_ui(ui, |ui| {
                            ui.selectable_value(&mut filtres.phase, None, "Toutes");
                            for phase in &projet.phases { ui.selectable_value(&mut filtres.phase, Some(phase.clone()), phase); }
                        });
                        ui.label("Sous-projet");
                        egui::ComboBox::from_id_salt("filtre_sous_projet_contexte").selected_text(filtres.sous_projet.as_deref().unwrap_or("Tous")).show_ui(ui, |ui| {
                            ui.selectable_value(&mut filtres.sous_projet, None, "Tous");
                            for sous_projet in &projet.sous_projets { ui.selectable_value(&mut filtres.sous_projet, Some(sous_projet.clone()), sous_projet); }
                        });
                        ui.label("Variante");
                        let nom = filtres.variante.as_ref().and_then(|id| projet.variantes.iter().find(|v| &v.id == id)).map_or("Toutes".into(), |v| v.libelle());
                        egui::ComboBox::from_id_salt("filtre_variante_contexte").selected_text(nom).show_ui(ui, |ui| {
                            ui.selectable_value(&mut filtres.variante, None, "Toutes");
                            for variante in &projet.variantes { ui.selectable_value(&mut filtres.variante, Some(variante.id.clone()), variante.libelle()); }
                        });
                    });
                    if ui.button("Tout afficher").clicked() { filtres = Default::default(); }
                });
                if let Some(erreur) = &etat.erreur { ui.separator(); ui.colored_label(ui.visuals().error_fg_color, erreur); }
            });
        if filtres != self.filtres_contextes { self.filtres_contextes = filtres; self.visibility_dirty = true; }
        if let Some(liste) = nouvel_onglet {
            etat.liste = liste; etat.selection = None; etat.ajout = true; etat.nom.clear(); etat.jeu.clear(); etat.nouveau_jeu.clear(); etat.remplacement = None; etat.erreur = None;
        }
        if let Some(sel) = selection {
            etat.selection = sel; etat.ajout = etat.selection.is_none(); etat.remplacement = None; etat.erreur = None;
            if let Some(cle) = &etat.selection {
                if etat.liste == ListeCreation::Variante {
                    if let Some(v) = projet.variantes.iter().find(|v| &v.id == cle) { etat.nom = v.nom.clone(); etat.jeu = v.jeu.clone(); etat.nouveau_jeu = v.jeu.clone(); }
                } else { etat.nom = cle.clone(); }
            } else { etat.nom.clear(); etat.jeu.clear(); etat.nouveau_jeu.clear(); }
        }
        if let Some(action) = action {
            let ancien_etat = etat.clone();
            let mut copie = projet.clone();
            let mut remappage: Option<(ListeCreation, String, Option<String>)> = None;
            let resultat = match action {
                Action::Ajouter => copie.ajouter_choix_creation(etat.liste, &etat.nom, &etat.jeu).map(|id| { etat.selection = Some(id); etat.ajout = false; }),
                Action::Renommer(cle) => copie.renommer_contexte(etat.liste, &cle, &etat.nom, &etat.jeu).map(|()| {
                    if etat.liste != ListeCreation::Variante {
                        let nouveau = etat.nom.trim().to_string();
                        remappage = Some((etat.liste, cle, Some(nouveau.clone())));
                        etat.selection = Some(nouveau);
                    }
                }),
                Action::Supprimer(cle) => copie.supprimer_contexte(etat.liste, &cle, etat.remplacement.as_deref()).map(|()| {
                    remappage = Some((etat.liste, cle, etat.remplacement.clone()));
                    etat.selection = None; etat.ajout = true; etat.nom.clear(); etat.jeu.clear(); etat.remplacement = None;
                }),
                Action::Deplacer(cle, delta) => copie.deplacer_phase(&cle, delta),
                Action::RenommerJeu(ancien) => copie.renommer_jeu_variantes(&ancien, &etat.nouveau_jeu).map(|()| { etat.jeu = etat.nouveau_jeu.trim().into(); }),
            };
            match resultat.and_then(|()| self.appliquer_projet(copie)) {
                Ok(()) => {
                    if let Some((liste, ancien, nouveau)) = remappage {
                        let (creation, filtre) = match liste {
                            ListeCreation::Phase => (&mut self.ruban_edition.phase_creation, &mut self.filtres_contextes.phase),
                            ListeCreation::SousProjet => (&mut self.ruban_edition.sous_projet_creation, &mut self.filtres_contextes.sous_projet),
                            ListeCreation::Variante => (&mut self.ruban_edition.variante_creation, &mut self.filtres_contextes.variante),
                        };
                        if creation.as_deref() == Some(&ancien) { *creation = nouveau.clone(); }
                        if filtre.as_deref() == Some(&ancien) { *filtre = nouveau; self.visibility_dirty = true; }
                    }
                    etat.erreur = None; self.status = "Organisation du projet mise à jour.".into();
                }
                Err(e) => { etat = ancien_etat; etat.erreur = Some(e); },
            }
        }
        if ouvert { self.gestion_contextes = Some(etat); }
    }
}
