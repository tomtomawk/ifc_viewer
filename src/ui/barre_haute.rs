//! Barre de titre, barre de vue, détail, visibilité par catégorie.
use super::*;

impl ViewerApp {
    /// Ruban en haut : onglets, puis les commandes de l'onglet ouvert.
    pub(super) fn top_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let maximise = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        // famille ouverte : son propre ruban, comme un document indépendant
        let mode_famille = self.mode_famille();
        let onglet_famille = matches!(self.onglet_ruban, OngletRuban::FamilleCreer | OngletRuban::FamilleSortie);
        if mode_famille && !onglet_famille {
            self.onglet_ruban = OngletRuban::FamilleCreer;
        } else if !mode_famille && onglet_famille {
            self.onglet_ruban = OngletRuban::Debut;
        }
        // cadre du système : ni boutons de fenêtre ni zones de déplacement dessinés ici
        let natif = crate::cadre_natif();
        let (barre, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), Sense::hover());
        let titre = egui::Rect::from_min_size(barre.min, egui::vec2(108.0, barre.height()));
        let icone_id = egui::Id::unique("icone_application_texture");
        let texture = ctx.data(|d| d.get_temp::<egui::TextureHandle>(icone_id)).unwrap_or_else(|| {
            let image = eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icone.png")).expect("Icône intégrée invalide");
            let pixels = egui::ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &image.rgba);
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
            let onglets: &[(OngletRuban, &str)] = if self.cartouche_actif().is_some() {
                &[(OngletRuban::FamilleCreer, "Documenter"), (OngletRuban::FamilleSortie, "Sortie")]
            } else if mode_famille {
                &[(OngletRuban::FamilleCreer, "Créer"), (OngletRuban::FamilleSortie, "Sortie")]
            } else {
                &[(OngletRuban::Debut, "Début"), (OngletRuban::Modeliser, "Modéliser"), (OngletRuban::Modifier, "Modifier"), (OngletRuban::Annoter, "Documenter"), (OngletRuban::Sortie, "Sortie")]
            };
            // pendant une création (outil actif), Modifier est grisé
            let creation = self.ruban_edition.a_un_outil() && self.dessin2d.edition_contour.is_none();
            for &(o, nom) in onglets {
                let actif = !(creation && o == OngletRuban::Modifier) || self.onglet_ruban == o;
                let reponse = ui.add_enabled_ui(actif, |ui| ui.selectable_label(self.onglet_ruban == o, RichText::new(nom).strong()))
                    .inner.on_disabled_hover_text("Terminez ou quittez la création en cours (Échap) pour modifier.");
                if reponse.clicked() {
                    self.onglet_ruban = o;
                }
            }
            let disponible = self.natif.is_some() && self.loading.is_none() && self.sauvegarde.is_none();
            if let Some(c)=&self.collaboration{let largeur=(ui.available_width()-90.0).clamp(0.0,220.0);ui.add_sized([largeur,20.0],egui::Label::new(RichText::new(format!("Partagé · {}",c.etat)).small()).truncate()).on_hover_text(&c.etat);}
            else if self.loading.is_some(){ui.label(RichText::new(&self.status).small());}
            let mut historique_choisi = None;
            for (retablir, titre) in [(false, "Annuler"), (true, "Rétablir")] {
                let famille = self.famille.as_ref().filter(|_| self.famille_active);
                let libelles = if self.famille_plane_active.is_some() { Vec::new() } else if let Some(ed)=famille {ed.libelles_historique(retablir)} else if let Some(c)=&self.collaboration {
                    let pile=if retablir{&c.copie.retablissements}else{&c.copie.annulations};
                    pile.last().map(|t|format!("Transaction {} — {}",t.id,if c.copie.base.acceptes.contains_key(&t.id){"partagée"}else{"locale"})).into_iter().collect()
                } else {self.historique.libelles(retablir)};
                let actif = disponible && !libelles.is_empty();
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
            if self.loading.is_some() || self.sauvegarde.is_some() {
                ui.spinner();
                let texte = self.sauvegarde.as_ref().map_or(&self.status, |s| &s.progression);
                let reponse = ui.add_sized([ui.available_width().max(0.0), 24.0], egui::Label::new(texte).truncate()).on_hover_text(texte);
                zones_deplacement.push(reponse.rect);
            }
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
                crate::config::options::ThemeInterface::Systeme=>icone_moniteur(ui.painter(),r,c),
                crate::config::options::ThemeInterface::Clair=>icone_soleil(ui.painter(),r,c),
                crate::config::options::ThemeInterface::Sombre=>icone_lune(ui.painter(),r,c),
            }
            let mut choix=None;
            if reponse.clicked() {choix=Some(actuel.suivant());}
            reponse.context_menu(|ui| {
                for t in crate::config::options::ThemeInterface::TOUS {
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
                if k == 2 && self.edition_plane_modifiee(None).is_none() && (self.natif_modifie || self.documents.values().any(|d| d.modifie)) {
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
            let sortie = self.onglet_ruban == OngletRuban::FamilleSortie;
            if self.famille_plane_active.is_some() { self.ruban_famille_plane(ui, sortie); }
            else { self.ruban_famille(ui, sortie); }
            return;
        }
        let edition = match self.onglet_ruban {
            OngletRuban::Modeliser => Some(crate::ui::ruban::Menu::Modeliser),
            OngletRuban::Modifier => Some(crate::ui::ruban::Menu::Modifier),
            OngletRuban::Annoter => Some(crate::ui::ruban::Menu::Annoter),
            _ => None,
        };
        if self.dessin2d.edition_contour.is_some() {
            ui.label(RichText::new("Modification du tracé").strong());
            ui.label("Sélectionner un trait puis Suppr · cliquer une extrémité ouverte pour reprendre · glisser un trait ou un sommet");
            ui.label("Double-clic sur un trait : ajouter un sommet · clic droit sur un sommet : supprimer");
            ui.label("Les pentes restent sur les arêtes inchangées ; elles sont retirées des arêtes modifiées.");
            return;
        }
        if let Some(menu) = edition {
            let ids = self.selection_bim();
            self.ruban_edition.selection_elements = !ids.is_empty();
            self.ruban_edition.selection_murs = self.natif.as_ref().is_some_and(|(p, _)| ids.iter().any(|id| p.murs.iter().any(|m| &m.id == id) || p.assemblage(id).is_some()));
            self.ruban_edition.bouton_jonctions = None;
            self.ruban_edition.modifier_famille = menu == crate::ui::ruban::Menu::Modifier
                && self.cible_famille().is_some();
            let action = self.ruban_edition.afficher(ui, menu, self.natif.as_ref().map(|n| &n.0));
            if std::mem::take(&mut self.ruban_edition.ouvrir_famille) {
                self.modifier_famille_selectionnee();
                return;
            }
            if let Some(r) = self.ruban_edition.bouton_jonctions.take() {
                egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| self.menu_jonctions(ui));
            }
            match action {
                Some(crate::ui::ruban::Outil::ModifierContour) => self.commencer_edition_contour(),
                Some(crate::ui::ruban::Outil::CopierNiveau) => {
                    self.memoriser_copie_niveau(ids);
                    self.ouvrir_coller_niveau();
                }
                Some(crate::ui::ruban::Outil::CreerIdentique) => {
                    if let Err(e) = self.creer_identique() { self.error = Some(e); }
                }
                Some(crate::ui::ruban::Outil::Supprimer) => self.demander_suppression(),
                Some(crate::ui::ruban::Outil::Cadrer) => self.demarrer_cadrage_vue(),
                Some(o @ (crate::ui::ruban::Outil::Isoler | crate::ui::ruban::Outil::Masquer | crate::ui::ruban::Outil::MasquerVue)) => self.visibilite_selection(o),
                Some(crate::ui::ruban::Outil::Afficher) => self.basculer_afficher_masques(),
                Some(crate::ui::ruban::Outil::Verrouiller) => self.basculer_verrou_selection(),
                Some(crate::ui::ruban::Outil::Deverrouiller) => self.verrouiller_selection(false),
                Some(crate::ui::ruban::Outil::Feuilles) => self.commande_feuilles(),
                Some(crate::ui::ruban::Outil::Nomenclatures) => {
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
                use crate::ui::ruban::{Commande as C, Icone as I};
                let fichier = vec![
                    C::new("nouveau", "Nouveau", I::Nouveau, "Créer un projet natif v0 (Ctrl+N)", self.loading.is_none()),
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
                for (cle, r) in crate::ui::ruban::bandeau(ui, &[("Fichier", fichier), ("Réglages", reglages), ("Projet", orientation)]) {
                    match cle {
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
            OngletRuban::Modeliser | OngletRuban::Modifier | OngletRuban::Annoter | OngletRuban::FamilleCreer | OngletRuban::FamilleSortie => {}
            OngletRuban::Sortie => {
                use crate::ui::ruban::{Commande as C, Icone as I};
                let disponible = (self.natif.is_some() || self.rvt_source.is_some()) && self.loading.is_none() && self.sauvegarde.is_none();
                let enregistrer = vec![
                    C::new("enregistrer", "Enregistrer", I::Enregistrer, "Enregistrer en modèle natif v0 (Ctrl+S)", disponible),
                    C::new("enregistrer_sous", "Enregistrer sous", I::EnregistrerSous, "Enregistrer le projet natif dans un autre fichier", self.natif.is_some() && self.collaboration.is_none()),
                ];
                let exporter = vec![
                    C::new("svg", "Dessin SVG", I::Exporter("SVG"), "Exporter le dessin de la vue en SVG (vue orthographique)", self.scene.is_some()),
                    C::new("pdf", "Dessin PDF", I::Exporter("PDF"), "Exporter le dessin de la vue en PDF (vue orthographique)", self.scene.is_some()),
                    C::new("ifc", "IFC4", I::Exporter("IFC"), "Exporter le projet natif en IFC4", self.natif.is_some()),
                    C::new("jeu_pdf", "Jeu PDF", I::Exporter("PDF"), "Exporter le jeu de feuilles ouvert (sinon le premier) en un PDF, une page par feuille", self.natif.as_ref().is_some_and(|(p, _)| !p.feuilles.is_empty())),
                ];
                let partage_disponible=self.collaboration.as_ref().is_some_and(|c|!c.synchronisation_en_cours())&&self.loading.is_none()&&!self.saisie_active&&self.avant_saisie.is_none();
                let collaboration=vec![
                    C::new("synchro","Synchro",I::Synchro,"Publier les modifications locales et recevoir celles des collègues",partage_disponible),
                    C::new("gerer_collaboration","Gérer",I::Collaborer,"Réservations, reprise explicite, instantané et textes récupérables",self.collaboration.is_some()),
                ];
                for (cle, r) in crate::ui::ruban::bandeau(ui, &[("Enregistrer", enregistrer), ("Collaboration", collaboration), ("Exporter", exporter)]) {
                    if cle=="gerer_collaboration"{
                        egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui|self.menu_collaboration(ui));continue;
                    }
                    if !r.clicked() {
                        continue;
                    }
                    match cle {
                        "synchro" => self.demander_synchro(),
                        "enregistrer" => self.enregistrer_projet(&ctx),
                        "enregistrer_sous" => self.enregistrer_sous(),
                        "svg" => self.exporter_svg_dialogue(),
                        "pdf" => self.export_pdf_dialogue(),
                        "ifc" => self.export_ifc_dialogue(),
                        "jeu_pdf" => {
                            let jeu = self.jeu_actif().or_else(|| self.natif.as_ref().and_then(|(p, _)| crate::modele::documentation::jeux(p).into_iter().next()));
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
    pub(super) fn bords_fenetre(&self, ctx: &egui::Context) {
        use egui::ResizeDirection::*;
        if crate::cadre_natif() {
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

    /// Vue 3D : modèle libre (IFC, ou fichier Revit sans vue ouverte) ou vue 3D Revit ;
    /// pas un plan, une coupe, une élévation ni une feuille.
    pub(super) fn vue_3d(&self) -> bool {
        match self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            Some(v) => v.genre == "Vue 3D",
            None => true,
        }
    }

    /// Plans Revit et vues orthographiques du modèle regardées verticalement.
    pub(super) fn vue_en_plan(&self) -> bool {
        if let Some(v) = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            return matches!(v.genre.as_str(), "Plan d'étage" | "Plan de plafond" | "Plan de structure" | "Plan de site");
        }
        self.camera.ortho && self.camera.pitch.abs() > 1.56
    }

    pub(super) fn orientation_initiale_plan(&self) -> f32 {
        self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i))
            .map_or(-std::f32::consts::FRAC_PI_2, |v| (-v.haut.y as f32).atan2(-v.haut.x as f32))
    }

    /// Glisser dans la vue : en 3D, orbite à Maj + bouton du milieu (comme Revit),
    /// panoramique au bouton du milieu, regard sur place (vol) au bouton droit ; le
    /// bouton gauche ne navigue pas (sélection, rectangle). Panoramique aux boutons
    /// gauche, droit et milieu dans une vue 2D (plan, coupe, élévation, feuille), dont
    /// l'orientation est celle de la vue Revit.
    pub(super) fn glisser(&mut self, bouton: PointerButton, d: egui::Vec2, shift: bool, hauteur: f32) {
        if self.vue_3d() {
            match bouton {
                PointerButton::Middle if shift => self.camera.orbit(d.x, d.y),
                PointerButton::Middle => self.camera.pan(d.x, d.y, hauteur),
                PointerButton::Secondary => {
                    self.camera.ortho = false;
                    self.camera.regarder(d.x, d.y);
                }
                _ => {}
            }
        } else {
            self.camera.pan(d.x, d.y, hauteur);
        }
    }

    /// Navigation en vol (vue 3D) : tant que le bouton droit est enfoncé sur la vue, la
    /// souris oriente le regard, W/S (Z/S en AZERTY) avancent et reculent, A/D (Q/D) vont à
    /// gauche et à droite, Q/E (A/E) descendent et montent ; Maj triple la vitesse, la
    /// molette la règle. Comme dans la vue Scène d'Unity.
    pub(super) fn voler(&mut self, ui: &egui::Ui, resp: &egui::Response) {
        let enfonce = self.vue_3d() && resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.button_down(PointerButton::Secondary));
        if !enfonce {
            self.vol.actif = false;
            self.vol.touches.clear();
            return;
        }
        if !self.vol.actif {
            self.vol.actif = true;
            self.camera.ortho = false;
            self.camera.rapprocher_cible(if self.pieton { 3.0 } else { (self.camera.scene_radius * 0.25).max(1.0) });
        }
        let (delta, molette, maj, dt) = ui.input(|i| {
            for e in &i.events {
                if let egui::Event::Key { key, physical_key, pressed, .. } = e {
                    let k = physical_key.unwrap_or(*key);
                    if *pressed {
                        self.vol.touches.insert(k);
                    } else {
                        self.vol.touches.remove(&k);
                    }
                }
            }
            (i.pointer.delta(), i.smooth_scroll_delta.y, i.modifiers.shift, i.stable_dt.min(0.1))
        });
        if delta != egui::Vec2::ZERO {
            self.camera.regarder(delta.x, delta.y);
        }
        if molette != 0.0 {
            self.vol.vitesse = (self.vol.vitesse * (molette * 0.004).exp()).clamp(0.02, 50.0);
        }
        let dir = self.vol.direction();
        if dir != Vec3::ZERO {
            let f = self.camera.forward();
            let droite = f.cross(Vec3::Z).normalize_or(Vec3::X);
            if self.pieton {
                // marche : à l'horizontale, 1,4 m/s (×3 avec Maj), l'œil suit le sol
                let avant = Vec3::new(f.x, f.y, 0.0).normalize_or(Vec3::X);
                let v = 1.4 * self.vol.vitesse * if maj { 3.0 } else { 1.0 };
                self.marcher((avant * dir.x + droite * dir.y) * v * dt);
            } else {
                let v = (self.camera.scene_radius * 0.3).max(1.0) * self.vol.vitesse * if maj { 3.0 } else { 1.0 };
                self.camera.deplacer((f * dir.x + droite * dir.y + Vec3::Z * dir.z) * v * dt);
            }
            ui.ctx().request_repaint();
        }
    }

    /// Barre de la vue, sous la vue comme celle de Revit : échelle (celle de la vue
    /// ouverte, qui règle aussi le dessin 2D), cadrage, projection en vue 3D.
    pub(super) fn barre_vue(&mut self, ui: &mut egui::Ui) {
        let gabaritee = self.vue_gabaritee();
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(self.scene.is_some(), |ui| {
                let feuille = self.jeu_actif().is_some();
                let mut echelle = if feuille { 1.0 } else { self.echelle };
                let r = ui.add_enabled(!gabaritee && !feuille, egui::DragValue::new(&mut echelle).speed(1.0).range(1.0..=10000.0).prefix("1 : "));
                if r.on_disabled_hover_text(if feuille { "Sélectionner la vue placée pour régler son échelle dans Propriétés." } else { "Échelle contrôlée par le gabarit de vue." })
                    .on_hover_text("Échelle de la vue, y compris sur les feuilles et dans les exports.")
                    .changed() && let Err(e) = self.regler_echelle_vue(echelle) {
                    self.error = Some(e);
                }
                ui.separator();
                self.choix_detail(ui);
                ui.separator();
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
                if self.vue_en_plan() {self.choix_fond_plan(ui);}
                if ui.selectable_label(self.traits_fins, "Traits fins").on_hover_text("Tous les traits à un pixel, sans les épaisseurs des plumes").clicked() {
                    self.traits_fins = !self.traits_fins;
                    self.coupe_dirty = true;
                }
                ui.separator();
                if ui.button("Cadre").on_hover_text("Définir le contour du plan ; F ajuste le zoom").clicked() {
                    self.demarrer_cadrage_vue();
                }
                if self.vue_native_id().and_then(|id| self.natif.as_ref()?.0.vues.iter().find(|v| v.id == id)).is_some_and(|v| !v.contour_cadrage.is_empty())
                    && ui.button("Retirer le cadrage").clicked() { self.retirer_cadrage_vue(); }
                ui.separator();
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
                if self.vue_3d() {
                    ui.separator();
                    let mut zone = self.boite_active;
                    if ui.toggle_value(&mut zone, "Zone de coupe").on_hover_text("Boîte qui découpe le modèle dans la vue 3D ; ses faces se règlent aux poignées ou dans les propriétés").changed() {
                        self.activer_boite(zone);
                    }
                }
            });
        });
    }

    /// Niveau de détail de la vue (Faible, Moyen, Élevé), celui de la vue Revit ouverte.
    /// La visibilité des géométries des familles selon le niveau de détail n'est pas
    /// encore lue : le choix n'a pas d'effet sur l'affichage.
    pub(super) fn choix_detail(&mut self, ui: &mut egui::Ui) {
        let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::hover());
        icone_detail(ui.painter(), r.shrink(3.0), self.detail);
        ui.menu_button(self.detail.libelle(), |ui| {
            for d in NiveauDetail::TOUS {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(120.0, 24.0), Sense::click());
                let v = ui.visuals();
                if d == self.detail {
                    ui.painter().rect_filled(r, 3.0, v.selection.bg_fill);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 3.0, v.widgets.hovered.bg_fill);
                }
                icone_detail(ui.painter(), egui::Rect::from_min_size(r.min + egui::vec2(5.0, 4.0), egui::vec2(16.0, 16.0)), d);
                ui.painter().text(r.min + egui::vec2(30.0, 12.0), egui::Align2::LEFT_CENTER, d.libelle(), egui::TextStyle::Button.resolve(ui.style()), v.text_color());
                if resp.clicked() {
                    self.detail = d;
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("Niveau de détail de la vue (lu dans la vue Revit). Sans effet sur l'affichage pour l'instant : la visibilité des géométries de familles par niveau de détail n'est pas encore lue");
    }

    /// Catégorie montrée : une de ses clés au moins n'est pas masquée, ni sa classe.
    pub(super) fn categorie_visible(&self, c: &Categorie) -> bool {
        c.cles.iter().any(|cle| !self.hidden_classes.contains(&**cle) && !self.hidden_classes.contains(classe_cle(cle)))
    }

    /// Affiche ou masque la catégorie `k` de la boîte de visibilité.
    pub(super) fn montrer_categorie(&mut self, k: usize, montrer: bool) {
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
    pub(super) fn fenetre_visibilite(&mut self, ctx: &egui::Context) {
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
