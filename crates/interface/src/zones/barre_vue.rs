//! Barre du bas de la vue : échelle, détail, apparence, cadrage, visibilité ; la barre de
//! l'éditeur de familles est dans `familles/editeur_famille/navigation.rs`.
use crate::*;

impl Application {
    /// Barre du bas de la vue, selon ce qui est ouvert : famille (éditeur de familles) ou vue
    /// du projet (3D, plan, coupe, feuille).
    pub(crate) fn barre_vue(&mut self, ui: &mut egui::Ui) {
        if self.mode_famille() {
            self.barre_vue_famille(ui);
        } else {
            self.barre_vue_projet(ui);
        }
    }

    /// Les sections s'enchaînent dans l'ordre de la barre ; `plan` et `vue_3d` décident de
    /// celles qui s'y ajoutent (fond de plan, zone de coupe).
    fn barre_vue_projet(&mut self, ui: &mut egui::Ui) {
        let gabaritee = self.vue_gabaritee();
        let (plan, vue_3d) = (self.vue_en_plan(), self.vue_3d());
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(self.scene.is_some(), |ui| {
                self.section_echelle_detail(ui, gabaritee);
                ui.separator();
                self.section_apparence(ui, plan);
                ui.separator();
                self.section_cadrage(ui);
                ui.separator();
                self.section_visibilite(ui, vue_3d);
            });
        });
    }

    /// Échelle (celle de la vue ouverte, qui règle aussi le dessin 2D) et niveau de détail.
    fn section_echelle_detail(&mut self, ui: &mut egui::Ui, gabaritee: bool) {
        let feuille = self.jeu_actif().is_some();
        let mut echelle = if feuille { 1.0 } else { self.echelle };
        let r = crate::socle::detail::champ_echelle(ui, &mut echelle, !gabaritee && !feuille);
        if r.on_disabled_hover_text(if feuille { "Sélectionner la vue placée pour régler son échelle dans Propriétés." } else { "Échelle contrôlée par le gabarit de vue." })
            .on_hover_text("Échelle de la vue, y compris sur les feuilles et dans les exports.")
            .changed() && let Err(e) = self.regler_echelle_vue(echelle) {
            self.error = Some(e);
        }
        ui.separator();
        self.choix_detail(ui);
    }

    /// Apparence : style visuel, ombres du soleil et du ciel, fond de plan (plans), traits fins.
    fn section_apparence(&mut self, ui: &mut egui::Ui, plan: bool) {
        self.choix_style(ui);
        let (ombres,_) = egui::containers::menu::MenuButton::from_button(egui::Button::new("").min_size(egui::vec2(26.0,24.0))).ui(ui,|ui| {
            let mut change=ui.checkbox(&mut self.ombres_portees,"Ombres du soleil").changed();
            change |= ui.checkbox(&mut self.ombre_ambiante,"Ombre ambiante").changed();
            ui.separator();
            change |= self.reglages_soleil_vue(ui);
            ui.separator();
            if ui.button("Ciel…").on_hover_text("Ambiance, nuages, brume et intensités du ciel physique (style Réaliste)").clicked() {
                self.ciel_ouvert = true;
                ui.close();
            }
            if change {self.visibility_dirty=true;}
            if !ui.input(|i| i.pointer.any_down()) {self.enregistrer_rendu_vue();}
        });
        icone_soleil(ui.painter(),ombres.rect.shrink(5.0),ui.visuals().text_color());
        ombres.on_hover_text("Ombres");
        if plan {self.choix_fond_plan(ui);}
        if ui.selectable_label(self.traits_fins, "Traits fins").on_hover_text("Tous les traits à un pixel, sans les épaisseurs des plumes").clicked() {
            self.traits_fins = !self.traits_fins;
            self.coupe_dirty = true;
        }
    }

    /// Cadrage du contour de la vue.
    fn section_cadrage(&mut self, ui: &mut egui::Ui) {
        if ui.button("Cadre").on_hover_text("Définir le contour du plan ; F ajuste le zoom").clicked() {
            self.demarrer_cadrage_vue();
        }
        if self.vue_native_id().and_then(|id| self.natif.as_ref()?.0.vues.iter().find(|v| v.id == id)).is_some_and(|v| !v.contour_cadrage.is_empty())
            && ui.button("Retirer le cadrage").clicked() { self.retirer_cadrage_vue(); }
    }

    /// Visibilité par catégorie, rétablissement des éléments masqués, zone de coupe (vue 3D).
    fn section_visibilite(&mut self, ui: &mut egui::Ui, vue_3d: bool) {
        ui.toggle_value(&mut self.voir_visibilite, "Visibilité").on_hover_text("Catégories de modèle et d'annotation affichées dans la vue (V)");
        if ui.button("Tout afficher").on_hover_text("Montrer les éléments masqués et isolés").clicked() {
            self.retablir_elements_vue();
            self.hidden_objects.clear();
            self.masques_utilisateur.clear();
            self.hidden_storeys.clear();
            self.isolated = self.isolement_de_vue();
            self.isolement_utilisateur = false;
            self.hidden_objects.extend(self.masques_variantes());
            self.visibility_dirty = true;
        }
        if vue_3d {
            ui.separator();
            let mut zone = self.boite_active;
            if ui.toggle_value(&mut zone, "Zone de coupe").on_hover_text("Boîte qui découpe le modèle dans la vue 3D ; ses faces se règlent aux poignées ou dans les propriétés").changed() {
                self.activer_boite(zone);
            }
        }
    }

    /// Style visuel : bouton à l'icône du style en cours, menu des styles.
    pub(crate) fn choix_style(&mut self, ui: &mut egui::Ui) {
        let mut choisi = None;
        let (reponse,_) = egui::containers::menu::MenuButton::from_button(egui::Button::new("").min_size(egui::vec2(24.0,24.0))).ui(ui, |ui| {
            for st in StyleVisuel::TOUS {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(190.0, 24.0), Sense::click());
                let v = ui.visuals();
                if st == self.style {
                    ui.painter().rect_filled(r, 3.0, v.selection.bg_fill);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 3.0, v.widgets.hovered.bg_fill);
                }
                icone_style(ui.painter(), egui::Rect::from_min_size(r.min + egui::vec2(4.0, 3.0), egui::vec2(18.0, 18.0)), st);
                ui.painter().text(r.min + egui::vec2(30.0, 12.0), egui::Align2::LEFT_CENTER, st.libelle(), egui::TextStyle::Button.resolve(ui.style()), v.text_color());
                if resp.clicked() {
                    choisi = Some(st);
                    ui.close();
                }
            }
            ui.separator();
            let graphique = self.style == StyleVisuel::Graphique;
            let mut couleurs = !self.faces_blanches;
            let mut change = ui.add_enabled(graphique, egui::Checkbox::new(&mut couleurs, "Couleurs des faces"))
                .on_hover_text("Décoché : faces blanches, dessin au trait")
                .changed();
            self.faces_blanches = !couleurs;
            change |= ui.add_enabled(graphique && couleurs, egui::Checkbox::new(&mut self.eclairage, "Éclairage"))
                .on_hover_text("Décoché : couleurs en aplat")
                .changed();
            if change {
                self.visibility_dirty = true;
            }
        });
        icone_style(ui.painter(), reponse.rect.shrink(4.0), self.style);
        reponse.on_hover_text("Style visuel");
        if let Some(st) = choisi {
            self.style = st;
            self.visibility_dirty = true;
            self.coupe_dirty = true;
            self.enregistrer_rendu_vue();
        }
    }

    pub(crate) fn choix_fond_plan(&mut self,ui:&mut egui::Ui) {
        let Some(id)=self.vue_native_id() else{return};
        let Some((p,_))=&self.natif else{return};
        let Some(v)=p.vues.iter().find(|v|v.id==id && matches!(v.genre,modele::GenreVue::Plan{..})) else{return};
        let mut choisi=v.fond_plan.clone();
        let vues:Vec<_>=p.vues.iter().filter(|v|v.id!=id && matches!(v.genre,modele::GenreVue::Plan{..})).map(|v|(v.id.clone(),v.nom.clone())).collect();
        let nom=choisi.as_ref().and_then(|id|vues.iter().find(|v|&v.0==id)).map_or("Aucun",|v|v.1.as_str());
        egui::ComboBox::from_id_salt("fond_plan_vue").selected_text(format!("Fond : {nom}")).show_ui(ui,|ui| {
            ui.selectable_value(&mut choisi,None,"Aucun");
            for (id,nom) in vues {ui.selectable_value(&mut choisi,Some(id),nom);}
        });
        if choisi!=v.fond_plan {
            let mut projet=p.clone();
            if let Some(v)=projet.vues.iter_mut().find(|v|v.id==id) {v.fond_plan=choisi;}
            match self.appliquer_projet(projet) {Ok(())=>{self.coupe_dirty=true;self.visibility_dirty=true;},Err(e)=>self.error=Some(e)}
        }
    }

    /// Catégorie montrée : une de ses clés au moins n'est pas masquée, ni sa classe.
    pub(crate) fn categorie_visible(&self, c: &Categorie) -> bool {
        c.cles.iter().any(|cle| !self.hidden_classes.contains(&**cle) && !self.hidden_classes.contains(classe_cle(cle)))
    }

    /// Affiche ou masque la catégorie `k` de la boîte de visibilité.
    pub(crate) fn montrer_categorie(&mut self, k: usize, montrer: bool) {
        for cle in self.categories[k].cles.clone() {
            let classe = classe_cle(&cle);
            if !montrer {
                self.hidden_classes.insert(cle.to_string());
                continue;
            }
            // classe masquée en bloc (annotations à l'ouverture) : on la montre en
            // masquant ses autres catégories
            if &*cle != classe && self.hidden_classes.remove(classe) {
                for c in self.categories.iter().flat_map(|c| &c.cles).filter(|c| **c != cle && classe_cle(c) == classe) {
                    self.hidden_classes.insert(c.to_string());
                }
            }
            self.hidden_classes.remove(&*cle);
        }
        self.visibility_dirty = true;
        self.preparer_feuille(self.vue_active);
    }

    /// Boîte de visibilité, au bas de la vue : catégories de modèle et d'annotation des
    /// éléments de la vue ouverte (de toute la scène hors vue Revit), à cocher une à une
    /// ou par groupe.
    pub(crate) fn fenetre_visibilite(&mut self, ctx: &egui::Context) {
        if !self.voir_visibilite || self.scene.is_none() {
            return;
        }
        let mut nombres = vec![0usize; self.categories.len()];
        for (i, &k) in self.categorie_objet.iter().enumerate() {
            if self.isolated.as_ref().is_none_or(|iso| iso.contains(&(i as u32))) {
                nombres[k] += 1;
            }
        }
        // objets des vues placées d'une feuille
        for &i in &self.objets_feuille {
            nombres[self.categorie_objet[i as usize]] += 1;
        }
        let mut ouverte = true;
        let mut bascules = Vec::new();
        egui::Window::new("Visibilité")
            .open(&mut ouverte)
            .collapsible(false)
            .resizable(true)
            .default_size([480.0, 300.0])
            .pivot(egui::Align2::LEFT_BOTTOM)
            .default_pos(self.rect_vue.left_bottom() + egui::vec2(10.0, -10.0))
            .constrain_to(self.rect_vue)
            .show(ctx, |ui| {
                ui.columns(2, |colonnes| {
                    for (ui, annotation) in colonnes.iter_mut().zip([false, true]) {
                        let titre = if annotation { "Catégories d'annotation (2D)" } else { "Catégories de modèle" };
                        let ks: Vec<usize> = (0..self.categories.len()).filter(|&k| self.categories[k].annotation == annotation && nombres[k] > 0).collect();
                        let vues = ks.iter().filter(|&&k| self.categorie_visible(&self.categories[k])).count();
                        let mut toutes = !ks.is_empty() && vues == ks.len();
                        let groupe = egui::Checkbox::new(&mut toutes, RichText::new(titre).strong()).indeterminate(vues > 0 && vues < ks.len());
                        if ui.add_enabled(!ks.is_empty(), groupe).changed() {
                            bascules.extend(ks.iter().map(|&k| (k, toutes)));
                        }
                        ui.separator();
                        egui::ScrollArea::vertical().id_salt(("visibilite", annotation)).auto_shrink([false, false]).show(ui, |ui| {
                            if ks.is_empty() {
                                ui.label(RichText::new("Aucune dans la vue").weak());
                            }
                            for &k in &ks {
                                let mut vue = self.categorie_visible(&self.categories[k]);
                                if ui.checkbox(&mut vue, format!("{} ({})", self.categories[k].nom, nombres[k])).changed() {
                                    bascules.push((k, vue));
                                }
                            }
                        });
                    }
                });
            });
        self.voir_visibilite = ouverte;
        for (k, montrer) in bascules {
            self.montrer_categorie(k, montrer);
        }
    }
}
