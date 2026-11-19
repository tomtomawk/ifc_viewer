//! Propriétés et suppression des vues du modèle natif.
use crate::*;
use crate::zones::ruban::selecteur::{self, ActionType};

#[derive(Debug, PartialEq, Eq)]
enum ActionGabarit {
    Affecter(Option<String>),
    Creer,
    Modifier(String),
    Dupliquer(String),
}

fn selecteur_gabarit_vue(
    ui: &mut egui::Ui,
    projet: &modele::Projet,
    actuel: Option<&str>,
) -> Option<ActionGabarit> {
    let gabarits = &projet.gabarits_vues;
    let courant = gabarits.iter().find(|g| Some(g.id.as_str()) == actuel);
    let titre = format!("Gabarit de vue · {}", courant.map_or("Aucun", |g| g.nom.as_str()));
    let echelle = courant.map(|g| format!("1:{}", g.echelle));
    let bouton = selecteur::entete(ui, &titre, echelle.as_deref(), &titre, None);
    let largeur = selecteur::largeur_liste(
        ui,
        gabarits.iter().map(|g| g.nom.as_str()).chain(["Aucun", "Créer à partir de cette vue…"]),
        56.0, 0, 0.0,
    );
    let mut action = None;
    selecteur::liste(&bouton, largeur, 0.0, |ui| {
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            let aucun = selecteur::Ligne {
                nom: "Aucun", courant: actuel.is_none(), etiquette: None, sous_titre: None, detail: None, actions: &[],
            };
            if selecteur::ligne(ui, &aucun, None).choisi {
                action = Some(ActionGabarit::Affecter(None));
            }
            for g in gabarits {
                let ligne = selecteur::Ligne {
                    nom: &g.nom, courant: actuel == Some(g.id.as_str()), etiquette: None, sous_titre: None,
                    detail: Some(format!("1:{}", g.echelle)),
                    actions: &[(ActionType::Dupliquer, "Dupliquer le gabarit"), (ActionType::Modifier, "Modifier le gabarit")],
                };
                let clic = selecteur::ligne(ui, &ligne, None);
                match clic.action {
                    Some(ActionType::Modifier) => action = Some(ActionGabarit::Modifier(g.id.clone())),
                    Some(ActionType::Dupliquer) => action = Some(ActionGabarit::Dupliquer(g.id.clone())),
                    None if clic.choisi => action = Some(ActionGabarit::Affecter(Some(g.id.clone()))),
                    None => {}
                }
            }
        });
        if selecteur::ligne_ajout(ui, "Créer à partir de cette vue…") {
            action = Some(ActionGabarit::Creer);
        }
    });
    action
}

#[cfg(test)]
mod tests_gabarit {
    use super::*;
    use modele::gabarits_vues::GabaritVue;

    fn rectangle_texte(sortie: &egui::FullOutput, libelle: &str) -> egui::Rect {
        sortie.shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == libelle => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
            }
            _ => None,
        }).unwrap_or_else(|| panic!("Texte absent : {libelle}"))
    }

    #[test]
    fn les_icones_agissent_sur_le_gabarit_survole_sans_laffecter() {
        let mut projet = modele::Projet::nouveau("Gabarits");
        projet.enregistrer_gabarit_vue(GabaritVue {
            id: "plan".into(), nom: "Plan détaillé".into(), echelle: 50.0,
            poche: "plein".into(), phase: None, variante: None,
        }).unwrap();
        for (distance, attendu) in [
            (19.0, ActionGabarit::Dupliquer("plan".into())),
            (43.0, ActionGabarit::Modifier("plan".into())),
        ] {
            let ctx = egui::Context::default();
            let mut action = None;
            let mut image = |events| { let mut s = ctx.run_ui(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 500.0))),
                events,
                ..Default::default()
            }, |ui| {
                ui.set_max_width(280.0);
                if let Some(clic) = selecteur_gabarit_vue(ui, &projet, None) {
                    action = Some(clic);
                }
            }); s.textures_delta.clear(); s };
            let entete = rectangle_texte(&image(vec![]), "Gabarit de vue · Aucun").center();
            let clic = |pos, pressed| vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
            }];
            image(clic(entete, true));
            image(clic(entete, false));
            let detail = rectangle_texte(&image(vec![]), "1:50");
            let icone = egui::pos2(detail.right() + 8.0 - distance, detail.center().y);
            image(vec![egui::Event::PointerMoved(icone)]);
            image(clic(icone, true));
            image(clic(icone, false));
            assert_eq!(action, Some(attendu));
        }
    }
}

impl Application {
    /// Identifiant stable des propriétés de contexte de la vue courante.
    pub(crate) fn id_vue_contexte(&self) -> Option<String> {
        self.id_vue_rendu()
    }

    /// Phase et variante enregistrées avec la vue, indépendamment des filtres temporaires.
    pub(crate) fn proprietes_contexte_vue(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.id_vue_contexte() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let mut filtre = projet.contextes_vues.get(&id).cloned().unwrap_or_default();
        let avant = filtre.clone();
        let gabarit = projet.gabarits_vue_affectes.get(&id).map(String::as_str);
        let mut action = None;
        egui::CollapsingHeader::new(RichText::new("Paramètres de la vue").strong()).id_salt("contexte_vue_native").default_open(true).show(ui, |ui| {
            action = selecteur_gabarit_vue(ui, projet, gabarit);
            egui::Grid::new("grille_contexte_vue").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Phase affichée").on_hover_text("Affiche les éléments créés au plus tard dans cette phase et pas encore démolis.");
                ui.add_enabled_ui(gabarit.is_none(), |ui| egui::ComboBox::from_id_salt("phase_vue_native").width(150.0).truncate().selected_text(filtre.phase.as_deref().unwrap_or("Toutes"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filtre.phase, None, "Toutes");
                        for phase in &projet.phases { ui.selectable_value(&mut filtre.phase, Some(phase.clone()), phase); }
                    }));
                ui.end_row();
                ui.label("Variante affichée").on_hover_text("Une seule variante est visible par jeu : celle choisie ici pour son jeu, la variante retenue pour les autres.");
                let nom = filtre.variante.as_ref().and_then(|id| projet.variantes.iter().find(|v| &v.id == id)).map_or("Variantes retenues".into(), |v| v.libelle());
                ui.add_enabled_ui(gabarit.is_none(), |ui| egui::ComboBox::from_id_salt("variante_vue_native").width(150.0).truncate().selected_text(nom).show_ui(ui, |ui| {
                    ui.selectable_value(&mut filtre.variante, None, "Variantes retenues");
                    for variante in &projet.variantes { ui.selectable_value(&mut filtre.variante, Some(variante.id.clone()), variante.libelle()); }
                }));
                ui.end_row();
            });
            if self.filtres_contextes != Default::default() {
                ui.label(RichText::new("L'affichage temporaire du gestionnaire prime sur les réglages de la vue.").small().weak());
            }
        });
        match action {
            Some(ActionGabarit::Affecter(choix)) if choix.as_deref() != gabarit => {
                let mut copie = projet.clone();
                match copie.affecter_gabarit_vue(&id, choix.as_deref()).and_then(|()| self.appliquer_projet(copie)) {
                    Ok(()) => { self.actualiser_gabarit_vue_actif(); self.status = "Gabarit de vue mis à jour.".into(); }
                    Err(e) => self.error = Some(e),
                }
                return;
            }
            Some(ActionGabarit::Creer) => { self.ouvrir_editeur_gabarit_vue(&id); return; }
            Some(ActionGabarit::Modifier(gabarit)) => { self.ouvrir_gabarit_par_id(&id, &gabarit, false); return; }
            Some(ActionGabarit::Dupliquer(gabarit)) => { self.ouvrir_gabarit_par_id(&id, &gabarit, true); return; }
            _ => {}
        }
        if filtre != avant {
            let mut copie = projet.clone();
            if filtre == Default::default() { copie.contextes_vues.remove(&id); } else { copie.contextes_vues.insert(id, filtre); }
            if let Err(e) = self.appliquer_projet(copie) { self.error = Some(e); }
        }
    }

    /// Propriétés de la vue native ouverte : nom, type, échelle, hauteur du plan de
    /// coupe (plan) ou profondeur (coupe, élévation), Retourner, Supprimer.
    pub(crate) fn proprietes_vue_native(&mut self, ui: &mut egui::Ui) {
        use modele::GenreVue;
        let Some((projet, _)) = &self.natif else { return };
        let Some(i) = self.vue_active else { return };
        let Some(numero) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.get(i)).map(|v| v.revit_id) else { return };
        let Some(vue) = projet.vues.iter().find(|v| modele::identifiants::numero_vue(&v.id) == numero).cloned() else { return };
        // nom en cours de saisie, propre à la vue
        if self.nom_vue.as_ref().is_none_or(|(id, _)| id != &vue.id) {
            self.nom_vue = Some((vue.id.clone(), vue.nom.clone()));
        }
        let mut nouvelle = vue.clone();
        let (mut renommer, mut retourner) = (false, false);
        let (mut maj_camera, mut retablir_camera) = (false, false);
        let sorte = match &vue.genre {
            GenreVue::Plan { .. } => "Plan d'étage",
            GenreVue::Coupe { elevation: true, .. } => "Élévation",
            GenreVue::Coupe { .. } => "Coupe",
            GenreVue::Camera { .. } => "Vue caméra",
        };
        egui::CollapsingHeader::new(RichText::new(format!("Vue · {sorte}")).strong()).id_salt("vue_native").default_open(true).show(ui, |ui| {
            egui::Grid::new("grille_vue_native").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Nom");
                if let Some((_, nom)) = &mut self.nom_vue {
                    let r = ui.add(egui::TextEdit::singleline(nom).desired_width(150.0));
                    renommer = r.lost_focus() && nom.trim() != vue.nom;
                }
                ui.end_row();
                if !vue.type_vue.is_empty() {
                    ui.label("Type");
                    ui.label(&vue.type_vue);
                    ui.end_row();
                }
                ui.label("Échelle");
                ui.add_enabled(projet.gabarit_vue(&vue.id).is_none(), egui::DragValue::new(&mut nouvelle.echelle).speed(1.0).range(1.0..=10_000.0).prefix("1 : "))
                    .on_disabled_hover_text("Échelle contrôlée par le gabarit de vue.");
                ui.end_row();
                let haut_niveau=&mut nouvelle.limite_haute_niveau;
                let haut_decalage=&mut nouvelle.limite_haute_decalage;
                match &mut nouvelle.genre {
                    GenreVue::Plan { niveau, coupe, bas_niveau, bas_decalage } => {
                        // Plan de coupe, limite basse et limite haute sont distincts.
                        let niveaux: Vec<(String, String)> = projet.niveaux.iter().map(|n| (n.id.clone(), n.nom.clone())).collect();
                        let nom = |id: &str| niveaux.iter().find(|n| n.0 == id).map_or(String::new(), |n| n.1.clone());
                        // ordre de la plage de vue : du haut vers le bas
                        ui.label("Limite haute").on_hover_text("Éléments au-dessus du plan de coupe, en pointillés ; seuls les éléments de ce niveau sont retenus.");
                        ui.horizontal(|ui| {
                            let mut choisi=haut_niveau.clone().unwrap_or_else(||niveau.clone());
                            let avant=choisi.clone();
                            egui::ComboBox::from_id_salt("plage_limite_haute").width(90.0).truncate().selected_text(nom(&choisi)).show_ui(ui,|ui| {
                                for (id,n) in &niveaux {ui.selectable_value(&mut choisi,id.clone(),n);}
                            });
                            if choisi!=avant {*haut_niveau=Some(choisi);*haut_decalage=0.0;}
                            ui.add(egui::DragValue::new(haut_decalage).speed(10.0).dimensions()).on_hover_text("Décalage de la limite haute par rapport au niveau choisi");
                        });
                        ui.end_row();
                        for (titre, aide, niv, dec) in [("Plan de coupe", "Niveau et décalage du plan de coupe", &mut *niveau, coupe), ("Limite basse", "Les éléments du niveau d'en dessous qui s'arrêtent ici ne sont pas montrés", bas_niveau, bas_decalage)] {
                            ui.label(titre).on_hover_text(aide);
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt(("plage_niveau", titre)).width(90.0).truncate().selected_text(nom(niv)).show_ui(ui, |ui| {
                                    for (id, n) in &niveaux {
                                        ui.selectable_value(niv, id.clone(), n);
                                    }
                                });
                                ui.add(egui::DragValue::new(dec).speed(10.0).dimensions()).on_hover_text("Décalage par rapport au niveau");
                            });
                            ui.end_row();
                        }
                    }
                    GenreVue::Coupe { profondeur, .. } => {
                        ui.label("Profondeur");
                        ui.add(egui::DragValue::new(profondeur).speed(50.0).range(100.0..=200_000.0).dimensions()).on_hover_text("Au-delà, le modèle n'est pas montré");
                        ui.end_row();
                    }
                    GenreVue::Camera { oeil, champ, .. } => {
                        ui.label("Altitude de l'œil").on_hover_text("Altitude de l'œil depuis le niveau 0 (en vue piéton : 1,60 m au-dessus du sol)");
                        ui.add(egui::DragValue::new(&mut oeil[2]).speed(10.0).dimensions());
                        ui.end_row();
                        ui.label("Champ de vision").on_hover_text("Angle vertical");
                        ui.add(egui::DragValue::new(champ).speed(0.5).range(15.0..=110.0).suffix(" °"));
                        ui.end_row();
                    }
                }
            });
            ui.horizontal(|ui| {
                if matches!(vue.genre, GenreVue::Coupe { .. }) {
                    retourner = ui.button("Retourner").on_hover_text("Regarder de l'autre côté de la ligne").clicked();
                }
                if matches!(vue.genre, GenreVue::Camera { .. }) {
                    maj_camera = ui.button("Mettre à jour la vue").on_hover_text("Enregistrer le point de vue actuel (position, regard, champ, projection, soleil, ciel) dans cette vue").clicked();
                    retablir_camera = ui.button("Rétablir").on_hover_text("Revenir au point de vue enregistré").clicked();
                }
            });
        });
        ui.separator();
        if maj_camera {
            if let Err(e) = self.mettre_a_jour_vue_camera() { self.error = Some(e); }
            return;
        }
        if retablir_camera {
            self.retablir_vue_camera();
            return;
        }
        if !(renommer || retourner || nouvelle != vue) {
            return;
        }
        let mut p = projet.clone();
        let resultat = if retourner {
            p.retourner_vue(&vue.id)
        } else if renommer {
            let nom = self.nom_vue.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
            p.renommer_vue(&vue.id, &nom)
        } else {
            *p.vues.iter_mut().find(|v| v.id == vue.id).unwrap() = nouvelle.clone();
            Ok(())
        };
        if let Err(e) = resultat.and_then(|_| self.appliquer_projet(p)) {
            self.error = Some(e);
            self.nom_vue = None;
            return;
        }
        if retourner {
            // la caméra prend la nouvelle direction de la vue (déjà ouverte), puis cadre
            if let Some(d) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.get(i)).map(|v| v.direction.as_vec3().normalize()) {
                self.camera.pitch = d.z.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
                self.camera.yaw = d.y.atan2(d.x);
                self.fit_all();
                self.memoriser_vue();
            }
        } else if let GenreVue::Plan { coupe, ref niveau, .. } = nouvelle.genre {
            self.coupe_niveau = Some(modele::identifiants::id_natif("niveau", niveau));
            self.echelle = nouvelle.echelle;
            self.coupe_hauteur = (coupe / 1000.0) as f32;
            self.coupe_dirty = true;
            self.visibility_dirty = true;
        } else {
            self.echelle = nouvelle.echelle;
        }
    }

    /// Supprime la vue native du navigateur d'indice `i` (Ctrl+Z la rétablit).
    pub(crate) fn supprimer_vue_native(&mut self, i: usize) {
        let Some((projet, _)) = &self.natif else { return };
        let Some(numero) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.get(i)).map(|v| v.revit_id) else { return };
        let Some(id) = projet.vues.iter().find(|v| modele::identifiants::numero_vue(&v.id) == numero).map(|v| v.id.clone()) else { return };
        let mut p = projet.clone();
        let ouverte = self.vue_active == Some(i);
        if let Err(e) = p.supprimer_vue(&id).and_then(|_| self.appliquer_projet(p)) {
            self.error = Some(e);
            return;
        }
        self.nom_vue = None;
        if ouverte {
            self.vue_active = None;
            self.activer_vue(0);
        }
        self.status = "Vue supprimée — Ctrl+Z pour annuler.".into();
    }

}
