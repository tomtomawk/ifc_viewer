//! Propriétés et suppression des vues du modèle natif.
use super::*;

impl ViewerApp {
    /// Identifiant stable des propriétés de contexte de la vue courante.
    pub(super) fn id_vue_contexte(&self) -> Option<String> {
        self.id_vue_rendu()
    }

    /// Phase et variante enregistrées avec la vue, indépendamment des filtres temporaires.
    pub(super) fn proprietes_contexte_vue(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.id_vue_contexte() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let mut filtre = projet.contextes_vues.get(&id).cloned().unwrap_or_default();
        let avant = filtre.clone();
        let mut gabarit = projet.gabarits_vue_affectes.get(&id).cloned();
        let gabarit_avant = gabarit.clone();
        let mut creer_gabarit = false;
        let mut modifier_gabarit = None;
        egui::CollapsingHeader::new(RichText::new("Paramètres de la vue").strong()).id_salt("contexte_vue_native").default_open(true).show(ui, |ui| {
            egui::Grid::new("grille_contexte_vue").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Gabarit de vue");
                ui.horizontal(|ui| {
                    let nom = gabarit.as_ref().and_then(|id| projet.gabarits_vues.iter().find(|g| &g.id == id)).map_or("Aucun", |g| g.nom.as_str());
                    egui::ComboBox::from_id_salt("gabarit_vue_native").width(100.0).truncate().selected_text(nom).show_ui(ui, |ui| {
                        ui.selectable_value(&mut gabarit, None, "Aucun");
                        for g in &projet.gabarits_vues { ui.selectable_value(&mut gabarit, Some(g.id.clone()), &g.nom); }
                    });
                    if ui.small_button("+").on_hover_text("Créer un gabarit à partir de cette vue").clicked() { creer_gabarit = true; }
                    ui.menu_button("…", |ui| {
                        ui.label("Modifier ou supprimer un gabarit");
                        if projet.gabarits_vues.is_empty() { ui.weak("Aucun gabarit"); }
                        for g in &projet.gabarits_vues {
                            if ui.button(&g.nom).clicked() { modifier_gabarit = Some(g.id.clone()); ui.close(); }
                        }
                    }).response.on_hover_text("Gérer les gabarits du projet");
                });
                ui.end_row();
                ui.label("Phase affichée").on_hover_text("Affiche les éléments créés au plus tard dans cette phase et pas encore démolis.");
                ui.add_enabled_ui(gabarit.is_none(), |ui| egui::ComboBox::from_id_salt("phase_vue_native").width(150.0).truncate().selected_text(filtre.phase.as_deref().unwrap_or("Toutes"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filtre.phase, None, "Toutes");
                        for phase in &projet.phases { ui.selectable_value(&mut filtre.phase, Some(phase.clone()), phase); }
                    }));
                ui.end_row();
                ui.label("Variante affichée").on_hover_text("Une seule variante est visible par jeu : celle choisie ici pour son jeu, la variante retenue (★) pour les autres.");
                let nom = filtre.variante.as_ref().and_then(|id| projet.variantes.iter().find(|v| &v.id == id)).map_or("Retenues ★".into(), |v| v.libelle());
                ui.add_enabled_ui(gabarit.is_none(), |ui| egui::ComboBox::from_id_salt("variante_vue_native").width(150.0).truncate().selected_text(nom).show_ui(ui, |ui| {
                    ui.selectable_value(&mut filtre.variante, None, "Retenues ★");
                    for variante in &projet.variantes { ui.selectable_value(&mut filtre.variante, Some(variante.id.clone()), variante.libelle()); }
                }));
                ui.end_row();
            });
            if self.filtres_contextes != Default::default() {
                ui.label(RichText::new("L'affichage temporaire du gestionnaire prime sur les réglages de la vue.").small().weak());
            }
        });
        if gabarit != gabarit_avant {
            let mut copie = projet.clone();
            match copie.affecter_gabarit_vue(&id, gabarit.as_deref()).and_then(|()| self.appliquer_projet(copie)) {
                Ok(()) => { self.actualiser_gabarit_vue_actif(); self.status = "Gabarit de vue mis à jour.".into(); }
                Err(e) => self.error = Some(e),
            }
            return;
        }
        if creer_gabarit { self.ouvrir_editeur_gabarit_vue(&id, false); return; }
        if let Some(gabarit) = modifier_gabarit { self.ouvrir_gabarit_par_id(&id, &gabarit); return; }
        if filtre != avant {
            let mut copie = projet.clone();
            if filtre == Default::default() { copie.contextes_vues.remove(&id); } else { copie.contextes_vues.insert(id, filtre); }
            if let Err(e) = self.appliquer_projet(copie) { self.error = Some(e); }
        }
    }

    /// Propriétés de la vue native ouverte : nom, type, échelle, hauteur du plan de
    /// coupe (plan) ou profondeur (coupe, élévation), Retourner, Supprimer.
    pub(super) fn proprietes_vue_native(&mut self, ui: &mut egui::Ui) {
        use crate::modele::GenreVue;
        let Some((projet, _)) = &self.natif else { return };
        let Some(i) = self.vue_active else { return };
        let Some(numero) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.get(i)).map(|v| v.revit_id) else { return };
        let Some(vue) = projet.vues.iter().find(|v| scene::numero_vue(&v.id) == numero).cloned() else { return };
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
            self.coupe_niveau = Some(scene::id_natif("niveau", niveau));
            self.echelle = nouvelle.echelle;
            self.coupe_hauteur = (coupe / 1000.0) as f32;
            self.coupe_dirty = true;
            self.visibility_dirty = true;
        } else {
            self.echelle = nouvelle.echelle;
        }
    }

    /// Supprime la vue native du navigateur d'indice `i` (Ctrl+Z la rétablit).
    pub(super) fn supprimer_vue_native(&mut self, i: usize) {
        let Some((projet, _)) = &self.natif else { return };
        let Some(numero) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.get(i)).map(|v| v.revit_id) else { return };
        let Some(id) = projet.vues.iter().find(|v| scene::numero_vue(&v.id) == numero).map(|v| v.id.clone()) else { return };
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
