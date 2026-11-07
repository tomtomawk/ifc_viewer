//! Propriétés et suppression des vues du modèle natif.
use super::*;

impl ViewerApp {
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
                ui.add(egui::DragValue::new(&mut nouvelle.echelle).speed(1.0).range(1.0..=10_000.0).prefix("1 : "));
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
            });
        });
        ui.separator();
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
