//! Gestion directe des phases, sous-projets et variantes : chaque ligne porte ses actions.
use super::*;
use crate::modele::edition::ListeCreation;

#[derive(Clone)]
pub(super) struct GestionContextes {
    liste: ListeCreation,
    /// Renommage en cours : (clé, texte saisi).
    renommage: Option<(String, String)>,
    /// Renommage d'un jeu de variantes : (jeu, texte saisi).
    renommage_jeu: Option<(String, String)>,
    /// Suppression à confirmer : (clé, entrée de remplacement).
    suppression: Option<(String, Option<String>)>,
    /// Adoption à confirmer : variante à verser dans le modèle principal.
    adoption: Option<String>,
    nouveau: String,
    erreur: Option<String>,
}

impl Default for GestionContextes {
    fn default() -> Self {
        Self { liste: ListeCreation::Phase, renommage: None, renommage_jeu: None, suppression: None, adoption: None, nouveau: String::new(), erreur: None }
    }
}

enum Action {
    Ajouter(String, String),
    Renommer(String, String),
    Supprimer(String, Option<String>),
    Deplacer(String, isize),
    RenommerJeu(String, String),
    Retenir(String),
    Adopter(String),
}

/// Ligne d'une entrée, préparée avant le dessin.
struct Entree { cle: String, nom: String, jeu: String, retenue: bool, usages: usize }

#[derive(Clone, Copy, PartialEq)]
enum Picto { Etoile, Oeil, Monter, Descendre }

/// Bouton à pictogramme dessiné (sans caractère spécial, absent de certaines polices) ;
/// `actif` : état coché, `permis` : cliquable.
fn pictogramme(ui: &mut egui::Ui, genre: Picto, actif: bool, permis: bool) -> egui::Response {
    use egui::{Pos2, Stroke};
    let (r, resp) = ui.allocate_exact_size(egui::vec2(20.0, 18.0), if permis { egui::Sense::click() } else { egui::Sense::hover() });
    let v = ui.visuals();
    if actif {
        ui.painter().rect_filled(r, 3.0, v.selection.bg_fill);
    } else if permis && resp.hovered() {
        ui.painter().rect_filled(r, 3.0, v.widgets.hovered.bg_fill);
    }
    let c = if !permis { v.weak_text_color().gamma_multiply(0.5) } else if actif { v.selection.stroke.color } else { v.text_color() };
    let (m, trait_) = (r.center(), Stroke::new(1.3, c));
    let p = ui.painter();
    match genre {
        Picto::Etoile => {
            let points: Vec<Pos2> = (0..10).map(|k| {
                let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
                m + egui::vec2(a.cos(), a.sin()) * if k % 2 == 0 { 7.0 } else { 3.0 }
            }).collect();
            if actif {
                // étoile pleine : cinq branches autour du pentagone central (formes convexes)
                let centre: Vec<Pos2> = points.iter().skip(1).step_by(2).copied().collect();
                p.add(egui::Shape::convex_polygon(centre, c, Stroke::NONE));
                for k in (0..10).step_by(2) {
                    p.add(egui::Shape::convex_polygon(vec![points[(k + 9) % 10], points[k], points[k + 1]], c, Stroke::NONE));
                }
            } else {
                p.add(egui::Shape::closed_line(points, Stroke::new(1.0, c)));
            }
        }
        Picto::Oeil => {
            let arc = |sens: f32| -> Vec<Pos2> { (0..=8).map(|k| { let t = k as f32 / 8.0; m + egui::vec2(-7.0 + 14.0 * t, sens * 4.5 * (std::f32::consts::PI * t).sin()) }).collect() };
            p.add(egui::Shape::line(arc(-1.0), trait_));
            p.add(egui::Shape::line(arc(1.0), trait_));
            p.circle_filled(m, 2.0, c);
        }
        Picto::Monter | Picto::Descendre => {
            let s = if genre == Picto::Monter { -1.0 } else { 1.0 };
            p.line_segment([m - egui::vec2(0.0, 5.0 * s), m + egui::vec2(0.0, 5.0 * s)], trait_);
            p.line_segment([m + egui::vec2(-4.0, 1.0 * s), m + egui::vec2(0.0, 5.0 * s)], trait_);
            p.line_segment([m + egui::vec2(4.0, 1.0 * s), m + egui::vec2(0.0, 5.0 * s)], trait_);
        }
    }
    resp
}

impl ViewerApp {
    pub(super) fn ouvrir_gestion_contextes(&mut self) {
        if self.natif.is_some() { self.gestion_contextes = Some(GestionContextes::default()); }
    }

    /// Choisir une variante de création l'affiche aussitôt, sinon on dessinerait dans une variante masquée.
    pub(super) fn suivre_variante_creation(&mut self) {
        let creation = self.ruban_edition.variante_creation.clone();
        if creation == self.variante_creation_suivie { return; }
        self.variante_creation_suivie = creation.clone();
        let Some((projet, _)) = &self.natif else { return };
        let Some(v) = creation.as_ref().and_then(|id| projet.variantes.iter().find(|v| &v.id == id)) else { return };
        let visible = self.filtres_contextes.variante.as_ref() == Some(&v.id)
            || (self.filtres_contextes.variante.as_ref().and_then(|id| projet.variantes.iter().find(|x| &x.id == id)).is_none_or(|x| x.jeu != v.jeu)
                && projet.variante_retenue(&v.jeu).is_some_and(|r| r.id == v.id)
                && self.id_vue_contexte().and_then(|id| projet.contextes_vues.get(&id)).and_then(|f| f.variante.as_ref()).is_none());
        if !visible {
            self.status = format!("Variante « {} » affichée pour y dessiner.", v.libelle());
            self.filtres_contextes.variante = creation;
            self.visibility_dirty = true;
        }
    }

    pub(super) fn fenetre_gestion_contextes(&mut self, ctx: &egui::Context) {
        self.suivre_variante_creation();
        let Some(mut etat) = self.gestion_contextes.take() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let mut ouvert = true;
        let mut action = None;
        let mut filtres = self.filtres_contextes.clone();
        let mut creation = match etat.liste {
            ListeCreation::Phase => self.ruban_edition.phase_creation.clone(),
            ListeCreation::SousProjet => self.ruban_edition.sous_projet_creation.clone(),
            ListeCreation::Variante => self.ruban_edition.variante_creation.clone(),
        };
        let creation_avant = creation.clone();
        let mut nouvel_onglet = None;
        let liste = etat.liste;
        let entrees: Vec<Entree> = match liste {
            ListeCreation::Phase => projet.phases.iter().map(|n| Entree { cle: n.clone(), nom: n.clone(), jeu: String::new(), retenue: false, usages: projet.utilisations_contexte(liste, n) }).collect(),
            ListeCreation::SousProjet => projet.sous_projets.iter().map(|n| Entree { cle: n.clone(), nom: n.clone(), jeu: String::new(), retenue: false, usages: projet.utilisations_contexte(liste, n) }).collect(),
            ListeCreation::Variante => projet.variantes.iter().map(|v| Entree {
                cle: v.id.clone(), nom: v.nom.clone(), jeu: v.jeu.clone(), usages: projet.utilisations_contexte(liste, &v.id),
                retenue: projet.variante_retenue(&v.jeu).is_some_and(|r| r.id == v.id),
            }).collect(),
        };
        let mut jeux: Vec<String> = Vec::new();
        for e in &entrees { if !jeux.contains(&e.jeu) { jeux.push(e.jeu.clone()); } }
        egui::Window::new("Phases, sous-projets et variantes")
            .id(egui::Id::unique("gestion_contextes_projet"))
            .default_size([560.0, 430.0]).min_width(480.0).open(&mut ouvert).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (l, libelle) in [(ListeCreation::Phase, "Phases"), (ListeCreation::SousProjet, "Sous-projets"), (ListeCreation::Variante, "Variantes")] {
                        if ui.selectable_label(liste == l, libelle).clicked() { nouvel_onglet = Some(l); }
                    }
                });
                ui.separator();
                ui.label(RichText::new(match liste {
                    ListeCreation::Phase => "Dans l'ordre chronologique. Rond : phase des prochaines créations. Œil : afficher le projet à cette phase.",
                    ListeCreation::SousProjet => "Rond : sous-projet des prochaines créations. Œil : n'afficher que ce sous-projet.",
                    ListeCreation::Variante => "Une seule variante visible par jeu. Étoile : variante retenue, affichée par défaut. Œil : afficher une autre variante. Rond : y dessiner.",
                }).small().weak());
                ui.add_space(4.0);
                egui::ScrollArea::vertical().id_salt("liste_contextes").max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                    if liste == ListeCreation::Variante {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut creation, None, "Modèle principal").on_hover_text("Créer les prochains éléments hors variante : ils restent visibles quelle que soit la variante affichée.");
                        });
                        if entrees.is_empty() { ui.weak("Aucun jeu de variantes. Créez un jeu ci-dessous : il reçoit une première variante, retenue par défaut."); }
                    }
                    for jeu in &jeux {
                        if liste == ListeCreation::Variante {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if let Some((_, texte)) = etat.renommage_jeu.as_mut().filter(|(j, _)| j == jeu) {
                                    let r = ui.add(egui::TextEdit::singleline(texte).desired_width(160.0));
                                    let valide = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    if ui.small_button("OK").clicked() || valide { action = Some(Action::RenommerJeu(jeu.clone(), texte.clone())); }
                                    if ui.small_button("Annuler").clicked() { etat.renommage_jeu = None; }
                                } else {
                                    ui.strong(jeu);
                                    if ui.small_button("Renommer").clicked() { etat.renommage_jeu = Some((jeu.clone(), jeu.clone())); }
                                    if let Some(retenue) = entrees.iter().find(|e| &e.jeu == jeu && e.retenue) {
                                        if ui.small_button("Adopter la retenue…").on_hover_text("Fin de l'étude : la variante retenue rejoint le modèle principal, les autres variantes du jeu et leurs éléments sont supprimés").clicked() {
                                            etat.adoption = Some(retenue.cle.clone());
                                        }
                                    }
                                    if ui.small_button("+ variante").on_hover_text("Ajouter une variante à ce jeu").clicked() {
                                        let n = entrees.iter().filter(|e| &e.jeu == jeu).count() + 1;
                                        action = Some(Action::Ajouter(format!("Variante {n}"), jeu.clone()));
                                    }
                                }
                            });
                        }
                        if let Some(e) = etat.adoption.as_ref().and_then(|a| entrees.iter().find(|e| &e.cle == a && &e.jeu == jeu)) {
                            let ecartes = projet.elements_variantes_ecartees(&e.cle).len();
                            let cle = e.cle.clone();
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(ui.visuals().warn_fg_color, format!("Adopter « {} » : ses éléments passent au modèle principal, {ecartes} élément(s) des autres variantes sont supprimés et le jeu disparaît.", e.nom));
                                if ui.small_button("Adopter").clicked() { action = Some(Action::Adopter(cle)); }
                                if ui.small_button("Annuler").clicked() { etat.adoption = None; }
                            });
                        }
                        egui::Grid::new(("grille_contextes", jeu)).num_columns(5).spacing([8.0, 4.0]).show(ui, |ui| {
                            for (rang, e) in entrees.iter().filter(|e| &e.jeu == jeu).enumerate() {
                                ui.horizontal(|ui| {
                                    if liste == ListeCreation::Variante {
                                        if pictogramme(ui, Picto::Etoile, e.retenue, true).on_hover_text("Variante retenue : affichée dans toutes les vues qui n'en choisissent pas une autre").clicked() && !e.retenue {
                                            action = Some(Action::Retenir(e.cle.clone()));
                                        }
                                    }
                                    ui.radio_value(&mut creation, Some(e.cle.clone()), "").on_hover_text("Les prochaines créations vont dans cette entrée");
                                    let (affiche, filtre) = match liste {
                                        ListeCreation::Phase => (filtres.phase.as_ref() == Some(&e.cle), &mut filtres.phase),
                                        ListeCreation::SousProjet => (filtres.sous_projet.as_ref() == Some(&e.cle), &mut filtres.sous_projet),
                                        ListeCreation::Variante => (filtres.variante.as_ref() == Some(&e.cle), &mut filtres.variante),
                                    };
                                    if pictogramme(ui, Picto::Oeil, affiche, true).on_hover_text("Affichage temporaire dans toutes les vues ; recliquer pour revenir aux réglages des vues").clicked() {
                                        *filtre = if affiche { None } else { Some(e.cle.clone()) };
                                    }
                                });
                                if let Some((_, texte)) = etat.renommage.as_mut().filter(|(c, _)| c == &e.cle) {
                                    let r = ui.add(egui::TextEdit::singleline(texte).desired_width(170.0));
                                    let valide = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    let saisie = texte.clone();
                                    ui.horizontal(|ui| {
                                        if ui.small_button("OK").clicked() || valide { action = Some(Action::Renommer(e.cle.clone(), saisie)); }
                                        if ui.small_button("Annuler").clicked() { etat.renommage = None; }
                                    });
                                } else {
                                    if ui.add(egui::Label::new(&e.nom).sense(egui::Sense::click())).on_hover_text("Double-clic pour renommer").double_clicked() {
                                        etat.renommage = Some((e.cle.clone(), e.nom.clone()));
                                    }
                                    ui.weak(format!("{} élément(s)", e.usages)).on_hover_text("Affectations directes, couches de murs et réglages de vues compris");
                                }
                                ui.horizontal(|ui| {
                                    if liste == ListeCreation::Phase {
                                        let n = entrees.len();
                                        if pictogramme(ui, Picto::Monter, false, rang > 1).on_hover_text("Avancer la phase dans la chronologie").clicked() { action = Some(Action::Deplacer(e.cle.clone(), -1)); }
                                        if pictogramme(ui, Picto::Descendre, false, rang > 0 && rang + 1 < n).on_hover_text("Reculer la phase dans la chronologie").clicked() { action = Some(Action::Deplacer(e.cle.clone(), 1)); }
                                    }
                                    if ui.small_button("Renommer").clicked() { etat.renommage = Some((e.cle.clone(), e.nom.clone())); }
                                    let fixe = liste == ListeCreation::Phase && e.cle == crate::modele::PHASE_EXISTANT || (liste != ListeCreation::Variante && entrees.len() < 2);
                                    if ui.add_enabled(!fixe, egui::Button::new("Supprimer").small()).clicked() {
                                        let autre = entrees.iter().find(|x| x.cle != e.cle && x.jeu == e.jeu).map(|x| x.cle.clone());
                                        if e.usages == 0 { action = Some(Action::Supprimer(e.cle.clone(), autre)); }
                                        else { etat.suppression = Some((e.cle.clone(), autre)); }
                                    }
                                });
                                ui.end_row();
                                if let Some((_, remplacement)) = etat.suppression.as_mut().filter(|(c, _)| c == &e.cle) {
                                    ui.label("");
                                    ui.label(format!("Réaffecter {} élément(s) vers", e.usages));
                                    let nom = remplacement.as_ref().and_then(|r| entrees.iter().find(|x| &x.cle == r)).map_or("Modèle principal", |x| x.nom.as_str());
                                    egui::ComboBox::from_id_salt("remplacement_contexte").selected_text(nom).width(130.0).show_ui(ui, |ui| {
                                        if liste == ListeCreation::Variante { ui.selectable_value(remplacement, None, "Modèle principal"); }
                                        for x in entrees.iter().filter(|x| x.cle != e.cle && x.jeu == e.jeu) { ui.selectable_value(remplacement, Some(x.cle.clone()), &x.nom); }
                                    });
                                    let choix = remplacement.clone();
                                    ui.horizontal(|ui| {
                                        if ui.small_button("Supprimer").clicked() { action = Some(Action::Supprimer(e.cle.clone(), choix)); }
                                        if ui.small_button("Annuler").clicked() { etat.suppression = None; }
                                    });
                                    ui.end_row();
                                }
                            }
                        });
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let (indice, bouton) = match liste {
                        ListeCreation::Phase => ("Nouvelle phase", "+ Ajouter la phase"),
                        ListeCreation::SousProjet => ("Nouveau sous-projet", "+ Ajouter le sous-projet"),
                        ListeCreation::Variante => ("Nouveau jeu (ex. Façade)", "+ Créer le jeu"),
                    };
                    let r = ui.add(egui::TextEdit::singleline(&mut etat.nouveau).desired_width(190.0).hint_text(indice));
                    let valide = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.button(bouton).clicked() || valide) && !etat.nouveau.trim().is_empty() {
                        action = Some(if liste == ListeCreation::Variante { Action::Ajouter("Variante 1".into(), etat.nouveau.clone()) } else { Action::Ajouter(etat.nouveau.clone(), String::new()) });
                    }
                    if filtres != Default::default() && ui.button("Tout afficher").on_hover_text("Annuler l'affichage temporaire (œil) et revenir aux réglages des vues").clicked() { filtres = Default::default(); }
                });
                if let Some(erreur) = &etat.erreur { ui.colored_label(ui.visuals().error_fg_color, erreur); }
            });
        if creation != creation_avant {
            match liste {
                ListeCreation::Phase => self.ruban_edition.phase_creation = creation,
                ListeCreation::SousProjet => self.ruban_edition.sous_projet_creation = creation,
                ListeCreation::Variante => self.ruban_edition.variante_creation = creation,
            }
        }
        if filtres != self.filtres_contextes { self.filtres_contextes = filtres; self.visibility_dirty = true; }
        if let Some(l) = nouvel_onglet { etat = GestionContextes { liste: l, ..Default::default() }; }
        if let Some(action) = action {
            let mut copie = projet.clone();
            let mut remappage: Option<(String, Option<String>)> = None;
            let resultat = match &action {
                Action::Ajouter(nom, jeu) => copie.ajouter_choix_creation(liste, nom, jeu).map(|_| ()),
                Action::Renommer(cle, nom) => {
                    let jeu = entrees.iter().find(|e| &e.cle == cle).map(|e| e.jeu.clone()).unwrap_or_default();
                    copie.renommer_contexte(liste, cle, nom, &jeu).map(|()| {
                        if liste != ListeCreation::Variante { remappage = Some((cle.clone(), Some(nom.trim().to_string()))); }
                    })
                }
                Action::Supprimer(cle, remplacement) => copie.supprimer_contexte(liste, cle, remplacement.as_deref()).map(|()| { remappage = Some((cle.clone(), remplacement.clone())); }),
                Action::Deplacer(cle, delta) => copie.deplacer_phase(cle, *delta),
                Action::RenommerJeu(ancien, nouveau) => copie.renommer_jeu_variantes(ancien, nouveau),
                Action::Retenir(cle) => copie.retenir_variante(cle),
                Action::Adopter(cle) => {
                    let du_jeu: Vec<String> = entrees.iter().filter(|e| entrees.iter().any(|x| &x.cle == cle && x.jeu == e.jeu)).map(|e| e.cle.clone()).collect();
                    copie.adopter_variante(cle).map(|_| {
                        if self.ruban_edition.variante_creation.as_ref().is_some_and(|v| du_jeu.contains(v)) { self.ruban_edition.variante_creation = None; }
                        if self.filtres_contextes.variante.as_ref().is_some_and(|v| du_jeu.contains(v)) { self.filtres_contextes.variante = None; }
                    })
                }
            };
            match resultat.and_then(|()| self.appliquer_projet(copie)) {
                Ok(()) => {
                    if let Some((ancien, nouveau)) = remappage {
                        let (creation, filtre) = match liste {
                            ListeCreation::Phase => (&mut self.ruban_edition.phase_creation, &mut self.filtres_contextes.phase),
                            ListeCreation::SousProjet => (&mut self.ruban_edition.sous_projet_creation, &mut self.filtres_contextes.sous_projet),
                            ListeCreation::Variante => (&mut self.ruban_edition.variante_creation, &mut self.filtres_contextes.variante),
                        };
                        if creation.as_deref() == Some(&ancien) { *creation = nouveau.clone(); }
                        if filtre.as_deref() == Some(&ancien) { *filtre = nouveau; }
                    }
                    if matches!(action, Action::Ajouter(..)) { etat.nouveau.clear(); }
                    etat.renommage = None; etat.renommage_jeu = None; etat.suppression = None; etat.adoption = None; etat.erreur = None;
                    self.visibility_dirty = true;
                    self.status = "Organisation du projet mise à jour.".into();
                }
                Err(e) => etat.erreur = Some(e),
            }
        }
        if ouvert { self.gestion_contextes = Some(etat); }
    }
}
