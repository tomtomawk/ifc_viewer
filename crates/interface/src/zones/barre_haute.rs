//! Barre de titre (ruban, onglets) et bords de la fenêtre.
use crate::*;

impl Application {
    /// Ruban en haut : onglets, puis les commandes de l'onglet ouvert.
    pub(crate) fn top_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let maximise = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        // famille ouverte : son propre ruban, comme un document indépendant
        let mode_famille = self.mode_famille();
        let onglet_famille = matches!(self.onglet_ruban, OngletRuban::FamilleCreer | OngletRuban::FamilleModifier | OngletRuban::FamilleSortie);
        if mode_famille && !onglet_famille {
            self.onglet_ruban = OngletRuban::FamilleCreer;
        } else if !mode_famille && onglet_famille {
            self.onglet_ruban = OngletRuban::Debut;
        }
        // cadre du système : ni boutons de fenêtre ni zones de déplacement dessinés ici
        let natif = crate::socle::fenetre::cadre_natif();
        let (barre, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), Sense::hover());
        let titre = egui::Rect::from_min_size(barre.min, egui::vec2(108.0, barre.height()));
        let icone_id = egui::Id::unique("icone_application_texture");
        let texture = ctx.data(|d| d.get_temp::<egui::TextureHandle>(icone_id)).unwrap_or_else(|| {
            // icône embarquée illisible : pixel transparent plutôt qu'un arrêt
            let pixels = match eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icone.png")) {
                Ok(image) => egui::ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &image.rgba),
                Err(_) => egui::ColorImage::filled([1, 1], egui::Color32::TRANSPARENT),
            };
            let texture = ctx.load_texture("icone_application", pixels, egui::TextureOptions::LINEAR);
            ctx.data_mut(|d| d.insert_temp(icone_id, texture.clone()));
            texture
        });
        let rect_icone = egui::Rect::from_center_size(egui::pos2(titre.left() + 10.0, titre.center().y), egui::vec2(18.0, 18.0));
        ui.painter().image(texture.id(), rect_icone, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        ui.put(egui::Rect::from_min_max(egui::pos2(titre.left() + 23.0, titre.top()), titre.max), egui::Label::new(RichText::new("BIMedit").strong()));
        let mut zones_deplacement = vec![titre];
        let menus = egui::Rect::from_min_max(egui::pos2(titre.right() + 8.0, barre.top()), egui::pos2(barre.right() - if natif { 40.0 } else { 180.0 }, barre.bottom()));
        ui.scope_builder(egui::UiBuilder::new().id_salt("menu_fenetre").max_rect(menus).layout(egui::Layout::left_to_right(Align::Center)), |ui| {
            let onglets: &[(OngletRuban, &str)] = if mode_famille {
                &[(OngletRuban::FamilleCreer, "Créer"), (OngletRuban::FamilleModifier, "Modifier"), (OngletRuban::FamilleSortie, "Sortie")]
            } else {
                &[(OngletRuban::Debut, "Début"), (OngletRuban::Modeliser, "Modéliser"), (OngletRuban::Modifier, "Modifier"), (OngletRuban::Annoter, "Documenter"), (OngletRuban::Sortie, "Sortie")]
            };
            for &(o, nom) in onglets {
                let reponse = ui.selectable_label(self.onglet_ruban == o, RichText::new(nom).strong());
                if reponse.clicked() {
                    if o == OngletRuban::Modifier && self.dessin2d.edition_contour.is_none() {
                        self.selectionner_trace_en_creation();
                    }
                    self.onglet_ruban = o;
                }
            }
            let disponible = self.natif.is_some() && self.loading.is_none() && self.sauvegarde.is_none();
            if let Some(c)=&self.collaboration{let largeur=(ui.available_width()-90.0).clamp(0.0,220.0);ui.add_sized([largeur,20.0],egui::Label::new(RichText::new(format!("Partagé · {}",c.etat)).small()).truncate()).on_hover_text(&c.etat);}
            else if self.loading.is_some(){ui.label(RichText::new(&self.status).small());}
            let mut historique_choisi = None;
            for (retablir, titre) in [(false, "Annuler"), (true, "Rétablir")] {
                let famille = self.famille.as_ref().filter(|_| self.famille_active);
                let libelles = if let Some(ed)=famille {ed.libelles_historique(retablir)} else if let Some(c)=&self.collaboration {
                    let pile=if retablir{&c.copie.retablissements}else{&c.copie.annulations};
                    pile.last().map(|t|format!("Transaction {} — {}",t.id,if c.copie.base.acceptes.contains_key(&t.id){"partagée"}else{"locale"})).into_iter().collect()
                } else {self.historique.libelles(retablir)};
                let actif = (disponible || mode_famille) && !libelles.is_empty();
                let style = ui.visuals().widgets.inactive;
                egui::Frame::new().fill(style.bg_fill).stroke(style.bg_stroke).corner_radius(style.corner_radius).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let reponse = ui.add_enabled(actif, egui::Button::new("").min_size(egui::vec2(22.0, 20.0)).frame(false));
                        let centre = reponse.rect.center();
                        let sens = if retablir { -1.0 } else { 1.0 };
                        let couleur = if actif { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
                        let points: Vec<_> = [(-5.0, -2.0), (1.0, -2.0), (4.0, -1.0), (5.0, 2.0), (4.0, 5.0), (1.0, 6.0)].into_iter().map(|(x,y)| centre + egui::vec2(x * sens, y)).collect();
                        ui.painter().add(egui::Shape::line(points, egui::Stroke::new(1.5, couleur)));
                        let pointe = centre + egui::vec2(-5.0 * sens, -2.0);
                        ui.painter().add(egui::Shape::convex_polygon(vec![pointe + egui::vec2(-2.0*sens,0.0), pointe + egui::vec2(2.0*sens,-3.5), pointe + egui::vec2(2.0*sens,3.5)], couleur, egui::Stroke::NONE));
                        reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, actif, titre));
                        if reponse.on_hover_text(if retablir { "Rétablir (Ctrl+Maj+Z)" } else { "Annuler (Ctrl+Z)" }).clicked() { historique_choisi = Some((retablir, 1)); }
                        ui.add_enabled_ui(actif, |ui| {
                            ui.scope(|ui| {
                                ui.spacing_mut().button_padding = egui::Vec2::ZERO;
                                let bouton = egui::Button::new("").min_size(egui::vec2(16.0, 20.0)).frame(false);
                                let (reponse, _) = egui::containers::menu::MenuButton::from_button(bouton).ui(ui, |ui| {
                                    ui.label(if retablir { "Rétablir jusqu'à :" } else { "Annuler jusqu'à :" });
                                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                        for (i, nom) in libelles.iter().enumerate() {
                                            if ui.button(format!("{}  {}", i + 1, nom)).clicked() {
                                                historique_choisi = Some((retablir, i + 1));
                                                ui.close();
                                            }
                                        }
                                    });
                                });
                                let centre = reponse.rect.center();
                                let c = if actif { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
                                ui.painter().add(egui::Shape::convex_polygon(vec![centre + egui::vec2(-3.5,-2.0), centre + egui::vec2(3.5,-2.0), centre + egui::vec2(0.0,2.0)], c, egui::Stroke::NONE));
                                reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, actif, if retablir { "Historique des rétablissements" } else { "Historique des annulations" }));
                                reponse.on_hover_text(if retablir { "Rétablir plusieurs modifications" } else { "Annuler plusieurs modifications" });
                            });
                        });
                    });
                });
            }
            if let Some((retablir, nombre)) = historique_choisi { self.parcourir_historique_n(retablir, nombre); }
            self.barre_progression(ui, &mut zones_deplacement);
            self.bouton_journal(ui);
            let (libre, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(0.0), 32.0), Sense::hover());
            zones_deplacement.push(libre);
        });
        // Placé immédiatement avant les commandes de fenêtre, sans zone de glissement.
        let theme_x=barre.right()-if natif {36.0}else{174.0};
        let zone_theme=egui::Rect::from_min_size(egui::pos2(theme_x,barre.top()),egui::vec2(34.0,barre.height()));
        ui.scope_builder(egui::UiBuilder::new().id_salt("theme_fenetre").max_rect(zone_theme).layout(egui::Layout::left_to_right(Align::Center)),|ui| {
            // Comme photocraft : l'icône montre le mode, le clic passe au suivant,
            // le clic droit propose les trois modes.
            let actuel=self.options.theme_interface;
            let reponse=ui.add(egui::Button::new("").min_size(egui::vec2(26.0,24.0)));
            let (r,c)=(reponse.rect.shrink(5.0),ui.visuals().text_color());
            match actuel {
                config::options::ThemeInterface::Systeme=>icone_moniteur(ui.painter(),r,c),
                config::options::ThemeInterface::Clair=>icone_soleil(ui.painter(),r,c),
                config::options::ThemeInterface::Sombre=>icone_lune(ui.painter(),r,c),
            }
            let mut choix=None;
            if reponse.clicked() {choix=Some(actuel.suivant());}
            reponse.context_menu(|ui| {
                for t in config::options::ThemeInterface::TOUS {
                    if ui.selectable_label(t==actuel,t.nom()).clicked() {choix=Some(t);ui.close();}
                }
            });
            reponse.on_hover_text(format!("Mode d'apparence : {} (clic : {})",actuel.nom(),actuel.suivant().nom()));
            if let Some(t)=choix {self.changer_theme(&ctx,t);}
        });
        if natif {
            zones_deplacement.clear();
        }
        for (k, rect) in zones_deplacement.into_iter().enumerate() {
            let reponse = ui.interact(rect, ui.scope_id().with(("deplacer_fenetre", k)), Sense::click_and_drag());
            if reponse.double_clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximise));
            } else if reponse.drag_started_by(PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }
        for (k, aide) in ["Réduire", if maximise { "Restaurer" } else { "Agrandir" }, "Fermer l’application"].into_iter().enumerate().filter(|_| !natif) {
            let rect = egui::Rect::from_min_size(egui::pos2(barre.right() - 138.0 + k as f32 * 46.0, barre.top()), egui::vec2(46.0, 30.0));
            let reponse = ui.interact(rect, ui.scope_id().with(("bouton_fenetre", k)), Sense::click()).on_hover_text(aide);
            let survole = reponse.hovered() || reponse.is_pointer_button_down_on();
            let couleur = if survole && k == 2 { Color32::WHITE } else { ui.visuals().text_color() };
            if survole {
                ui.painter().rect_filled(rect, 0.0, if k == 2 { Color32::from_rgb(196, 43, 28) } else { Color32::from_gray(225) });
            }
            let centre = rect.center();
            let icone = egui::Rect::from_center_size(centre, egui::vec2(10.0, 10.0));
            let trait_fin = egui::Stroke::new(1.0, couleur);
            match k {
                0 => { ui.painter().line_segment([icone.left_center(), icone.right_center()], trait_fin); }
                1 if maximise => {
                    let arriere = egui::Rect::from_min_size(icone.min + egui::vec2(3.0, 0.0), egui::vec2(7.0, 7.0));
                    ui.painter().line_segment([arriere.left_top(), arriere.right_top()], trait_fin);
                    ui.painter().line_segment([arriere.right_top(), arriere.right_bottom()], trait_fin);
                    let avant = egui::Rect::from_min_size(icone.min + egui::vec2(0.0, 3.0), egui::vec2(7.0, 7.0));
                    ui.painter().rect_stroke(avant, 0.0, trait_fin, egui::StrokeKind::Inside);
                }
                1 => { ui.painter().rect_stroke(icone, 0.0, trait_fin, egui::StrokeKind::Inside); }
                _ => {
                    ui.painter().line_segment([icone.left_top(), icone.right_bottom()], trait_fin);
                    ui.painter().line_segment([icone.right_top(), icone.left_bottom()], trait_fin);
                }
            }
            if reponse.clicked() {
                if k == 2 && self.sauvegarde.is_some() {
                    self.error = Some("Un enregistrement est en cours ; attendez sa fin avant de quitter.".into());
                    continue;
                }
                if k == 2 && (self.natif_modifie || self.documents.values().any(|d| d.modifie)) {
                    self.confirmer_fermeture = Some((true, self.document_actif));
                    continue;
                }
                ctx.send_viewport_cmd(match k {
                    0 => egui::ViewportCommand::Minimized(true),
                    1 => egui::ViewportCommand::Maximized(!maximise),
                    _ => egui::ViewportCommand::Close,
                });
            }
        }
        ui.separator();
        ui.set_min_height(28.0);
        if mode_famille {
            self.ruban_famille(ui, self.onglet_ruban);
            return;
        }
        let edition = match self.onglet_ruban {
            OngletRuban::Modeliser => Some(crate::zones::ruban::Menu::Modeliser),
            OngletRuban::Modifier => Some(crate::zones::ruban::Menu::Modifier),
            OngletRuban::Annoter => Some(crate::zones::ruban::Menu::Annoter),
            _ => None,
        };
        if let Some(menu) = edition {
            let ids = self.selection_bim();
            self.ruban_edition.edition_traces = self.dessin2d.edition_contour.is_some() || (self.onglet_ruban == OngletRuban::Modifier && self.usage_dessin().is_some_and(|(u, o)| o == moteur2d::dao::OutilDessin::Polyligne && matches!(u, crate::outils::dessiner::dessin_plan::Usage::Annotation | crate::outils::dessiner::dessin_plan::Usage::Surface | crate::outils::dessiner::dessin_plan::Usage::LimitePropriete | crate::outils::dessiner::dessin_plan::Usage::Terrain | crate::outils::dessiner::dessin_plan::Usage::Escalier | crate::outils::dessiner::dessin_plan::Usage::GardeCorps)));
            (self.ruban_edition.cible_trace, self.ruban_edition.trace_fixe, self.ruban_edition.trace_sans_copie) = self.cible_trace();
            self.ruban_edition.commande_trace = self.outil_commande_trace();
            self.ruban_edition.selection_elements = !ids.is_empty() || self.selection_traces_active();
            self.ruban_edition.selection_murs = self.natif.as_ref().is_some_and(|(p, _)| ids.iter().any(|id| p.murs.iter().any(|m| &m.id == id) || p.assemblage(id).is_some()));
            self.ruban_edition.bouton_jonctions = None;
            let outil_avant = self.ruban_edition.outil();
            let action = self.ruban_edition.afficher(ui, menu, self.natif.as_ref().map(|n| &n.0));
            if menu == crate::zones::ruban::Menu::Modeliser && action.is_some_and(|o| o != crate::zones::ruban::Outil::LienIfc) {
                self.select(None, true);
            }
            if menu == crate::zones::ruban::Menu::Modifier && self.ruban_edition.edition_traces && let Some(o) = action {
                if self.dessin2d.edition_contour.is_none() { self.selectionner_trace_en_creation(); }
                self.commande_ruban_contour(o);
                return;
            }
            if self.dessin2d.edition_contour.is_some() { return; }
            if self.ruban_edition.outil() != outil_avant && self.usage_dessin().is_some_and(|(u, o)| o == moteur2d::dao::OutilDessin::Polyligne && matches!(u, crate::outils::dessiner::dessin_plan::Usage::Annotation | crate::outils::dessiner::dessin_plan::Usage::Surface | crate::outils::dessiner::dessin_plan::Usage::LimitePropriete | crate::outils::dessiner::dessin_plan::Usage::Terrain | crate::outils::dessiner::dessin_plan::Usage::Escalier | crate::outils::dessiner::dessin_plan::Usage::GardeCorps) && self.outil_cotation().is_none()) {
                self.onglet_ruban = OngletRuban::Modifier;
            }
            if let Some(r) = self.ruban_edition.bouton_jonctions.take() {
                egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| self.menu_jonctions(ui));
            }
            match action {
                Some(crate::zones::ruban::Outil::ModifierContour) => self.commencer_edition_contour(),
                Some(crate::zones::ruban::Outil::CopierNiveau) => {
                    if self.memoriser_copie_niveau(ids) { self.ouvrir_coller_niveau(); }
                }
                Some(crate::zones::ruban::Outil::CreerIdentique) => {
                    if let Err(e) = self.creer_identique() { self.error = Some(e); }
                }
                Some(crate::zones::ruban::Outil::Supprimer) => self.demander_suppression(),
                Some(crate::zones::ruban::Outil::Cadrer) => self.demarrer_cadrage_vue(),
                Some(o @ (crate::zones::ruban::Outil::Isoler | crate::zones::ruban::Outil::Masquer | crate::zones::ruban::Outil::MasquerVue)) => self.visibilite_selection(o),
                Some(crate::zones::ruban::Outil::Afficher) => self.basculer_afficher_masques(),
                Some(crate::zones::ruban::Outil::Verrouiller) => self.basculer_verrou_selection(),
                Some(crate::zones::ruban::Outil::Deverrouiller) => self.verrouiller_selection(false),
                Some(crate::zones::ruban::Outil::Feuilles) => self.commande_feuilles(),
                Some(crate::zones::ruban::Outil::VueDessin) => self.creer_vue(|p| p.ajouter_vue_dessin(50.0), true),
                Some(crate::zones::ruban::Outil::LienIfc) => self.inserer_lien_dialogue(&ctx),
                Some(crate::zones::ruban::Outil::Nomenclatures) => {
                    if self.natif.is_some() {
                        self.bim.documentation = true;
                    } else {
                        self.status = "Nomenclatures : ouvrir un projet natif.".into();
                    }
                }
                _ => {}
            }
            return;
        }
        match self.onglet_ruban {
            OngletRuban::Debut => {
                use crate::zones::ruban::{Commande as C, Icone as I};
                let fichier = vec![
                    C::new("nouveau", "Nouveau", I::Nouveau, "Créer un projet natif v0 (Ctrl+N)", self.loading.is_none()),
                    C::new("nouvelle_famille", "Nouvelle famille", I::Nouveau, "Créer une famille", self.loading.is_none()),
                    C::new("ouvrir", "Ouvrir", I::Ouvrir, "Ouvrir un projet natif, un IFC, un RVT ou un RFA (Ctrl+O)", true),
                    C::new("fermer", "Fermer", I::Fermer, "Fermer le document actif", self.scene.is_some() || self.rvt.is_some() || self.loading.is_some()),
                    C::new("collaborer", "Collaborer", I::Collaborer, "Enregistrer un projet partagé sur le serveur et créer sa copie locale", self.natif.is_some() && self.collaboration.is_none() && self.loading.is_none() && self.sauvegarde.is_none() && !self.saisie_active),
                    C::new("importer", "Importer", I::Importer, "Convertir un IFC ou RVT en nouveau document natif", self.loading.is_none()),
                ];
                let mut reglages = Vec::new();
                if self.rvt.is_some() && self.rvt_geom.is_ok() {
                    reglages.push(C { enfonce: self.rvt_sheet, ..C::new("fiche", "Fiche Revit", I::Fiche, "Métadonnées du fichier Revit", true) });
                }
                reglages.push(C::new("options", "Paramètres", I::Options, "Unités, navigation IFC, épaisseurs de traits, gabarit", true));
                let orientation = vec![
                    C { enfonce: self.orientation_nord_en_cours, ..C::new("nord", "Orienter le nord", I::Nord, "Définir le nord du projet en sélectionnant un trait dans une vue de plan", self.natif.is_some() && self.vue_en_plan() && self.dessin2d.edition_contour.is_none()) },
                    C::new("lieu", "Lieu", I::Lieu, "Lieu géographique du projet : ville ou coordonnées (soleil des ombres et du ciel)", self.natif.is_some()),
                    C::new("coordonnees", "Coordonnées", I::Coordonnees, "Coordonnées de l'origine du projet et cote NGF du niveau 0 : la modifier remonte ou descend le projet par rapport au terrain (aussi : cliquer l'origine dans une vue)", self.natif.is_some()),
                    C::new("contextes", "Phases et variantes", I::Options, "Gérer les phases, sous-projets et variantes du projet", self.natif.is_some()),
                ];
                for (cle, r) in crate::zones::ruban::bandeau(ui, &[("Fichier", fichier), ("Réglages", reglages), ("Projet", orientation)]) {
                    match cle {
                        "nouvelle_famille" => { if r.clicked() { self.nouvelle_famille(); } }
                        "options" => {
                            if r.clicked() {self.ouvrir_parametres();}
                        }
                        "lieu" => {
                            egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                                ui.label(RichText::new("Lieu du projet").strong());
                                ui.label(RichText::new("Position du soleil des ombres et du ciel ; le nord se règle avec « Orienter le nord ».").small().weak());
                                self.editeur_lieu(ui);
                            });
                        }
                        "coordonnees" => {
                            egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                                ui.label(RichText::new("Origine du projet").strong());
                                ui.label(RichText::new("Cote NGF du niveau 0 : le terrain, en cotes NGF, suit.").small().weak());
                                self.editeur_coordonnees(ui);
                            });
                        }
                        _ if !r.clicked() => {}
                        "collaborer" => self.collaborer_dialogue(&ctx),
                        "nouveau" => self.creer_projet(),
                        "ouvrir" => self.open_dialog(&ctx),
                        "fermer" => self.fermer(),
                        "importer" => self.importer_dialogue(&ctx),
                        "contextes" => self.ouvrir_gestion_contextes(),
                        "fiche" => self.rvt_sheet = !self.rvt_sheet,
                        "nord" => {
                            self.orientation_nord_en_cours = !self.orientation_nord_en_cours;
                            self.alignement_plan = false;
                            self.arete_alignement = None;
                            self.ruban_edition.annuler();
                            self.status = if self.orientation_nord_en_cours { "Nord : viser un trait et cliquer près de l'extrémité qui indique le nord · Échap : annuler" } else { "Orientation du nord annulée." }.into();
                        }
                        _ => {}
                    }
                }
            }
            OngletRuban::Modeliser | OngletRuban::Modifier | OngletRuban::Annoter | OngletRuban::FamilleCreer | OngletRuban::FamilleModifier | OngletRuban::FamilleSortie => {}
            OngletRuban::Sortie => {
                use crate::zones::ruban::{Commande as C, Icone as I};
                let disponible = (self.natif.is_some() || self.rvt_source.is_some()) && self.loading.is_none() && self.sauvegarde.is_none();
                let enregistrer = vec![
                    C::new("enregistrer", "Enregistrer", I::Enregistrer, "Enregistrer en modèle natif v0 (Ctrl+S)", disponible),
                    C::new("enregistrer_sous", "Enregistrer sous", I::EnregistrerSous, "Enregistrer le projet natif dans un autre fichier", self.natif.is_some() && self.collaboration.is_none()),
                ];
                let exporter = vec![
                    C::new("svg", "Dessin SVG", I::Exporter("SVG"), "Exporter le dessin de la vue en SVG (vue orthographique)", self.scene.is_some()),
                    C::new("pdf", "Dessin PDF", I::Exporter("PDF"), "Exporter le dessin de la vue en PDF (vue orthographique)", self.scene.is_some()),
                    C::new("ifc", "IFC4", I::Exporter("IFC"), "Exporter le projet natif en IFC4", self.natif.is_some()),
                    C::new("jeu_pdf", "Jeu de feuilles", I::Exporter("JEU"), "Exporter le jeu de feuilles ouvert (sinon le premier) : PDF (groupé ou un par feuille), DWG, DXF, IFC", self.natif.as_ref().is_some_and(|(p, _)| !p.feuilles.is_empty())),
                ];
                let partage_disponible=self.collaboration.as_ref().is_some_and(|c|!c.synchronisation_en_cours())&&self.loading.is_none()&&!self.saisie_active&&self.avant_saisie.is_none();
                let collaboration=vec![
                    C::new("synchro","Synchro",I::Synchro,"Publier les modifications locales et recevoir celles des collègues",partage_disponible),
                    C::new("gerer_collaboration","Gérer",I::Collaborer,"Réservations, reprise explicite, instantané et textes récupérables",self.collaboration.is_some()),
                ];
                for (cle, r) in crate::zones::ruban::bandeau(ui, &[("Enregistrer", enregistrer), ("Collaboration", collaboration), ("Exporter", exporter)]) {
                    if cle=="gerer_collaboration"{
                        egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui|self.menu_collaboration(ui));continue;
                    }
                    if !r.clicked() {
                        continue;
                    }
                    match cle {
                        "synchro" => self.demander_synchro(),
                        "enregistrer" => self.enregistrer_projet_arriere_plan(&ctx),
                        "enregistrer_sous" => self.enregistrer_sous(),
                        "svg" => self.exporter_svg_dialogue(&ctx),
                        "pdf" => self.export_pdf_dialogue(&ctx),
                        "ifc" => self.export_ifc_dialogue(&ctx),
                        "jeu_pdf" => {
                            let jeu = self.jeu_actif().or_else(|| self.natif.as_ref().and_then(|(p, _)| modele::documentation::jeux(p).into_iter().next()));
                            if let Some(jeu) = jeu {
                                self.exporter_jeu_dialogue(&jeu);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Redimensionnement natif sur les bords de la fenêtre sans décoration.
    pub(crate) fn bords_fenetre(&self, ctx: &egui::Context) {
        use egui::ResizeDirection::*;
        if crate::socle::fenetre::cadre_natif() {
            return;
        }
        let (rect, position, maximise, presse) = ctx.input(|i| (i.viewport_rect(), i.pointer.hover_pos(), i.viewport().maximized.unwrap_or(false), i.pointer.primary_pressed()));
        let Some(p) = position.filter(|p| rect.contains(*p) && !maximise) else { return };
        let gauche = p.x < rect.left() + 5.0;
        let droite = p.x > rect.right() - 5.0;
        let haut = p.y < rect.top() + 5.0;
        let bas = p.y > rect.bottom() - 5.0;
        let direction = match (gauche, droite, haut, bas) {
            (true, _, true, _) => NorthWest,
            (_, true, true, _) => NorthEast,
            (true, _, _, true) => SouthWest,
            (_, true, _, true) => SouthEast,
            (true, _, _, _) => West,
            (_, true, _, _) => East,
            (_, _, true, _) => North,
            (_, _, _, true) => South,
            _ => return,
        };
        ctx.set_cursor_icon(match direction {
            NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
            NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
            North | South => egui::CursorIcon::ResizeVertical,
            _ => egui::CursorIcon::ResizeHorizontal,
        });
        if presse {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }

}
