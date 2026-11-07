//! Mémorisation, onglets et activation des vues, préparation des feuilles.
use super::*;

impl ViewerApp {
    pub(super) fn memoriser_vue(&mut self) {
        if self.saisie_active || self.partage_en_cours { return; }
        if !self.vues_ouvertes.contains(&self.vue_active) {
            return;
        }
        // Les réglages qui définissent le dessin doivent aussi suivre la vue sur feuille
        // et survivre à la fermeture du document.
        let numero = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)).map(|v| v.revit_id);
        let altitude_coupe = self.plan_coupe().and_then(|p| self.scene.as_ref().map(|s| (p.w as f64 + s.origin.z) * 1000.0));
        let orientation = crate::rendu::orientation_plan::angle(&self.camera) as f64;
        let origine = self.scene.as_ref().map(|s| s.origin);
        if let (Some(numero), Some((projet, _))) = (numero, &mut self.natif) {
            if let Some(index) = projet.vues.iter().position(|v| scene::numero_vue(&v.id) == numero) {
                let mut vue = projet.vues[index].clone();
                if let crate::modele::GenreVue::Plan { niveau, coupe, .. } = &mut vue.genre {
                    if let Some(z) = altitude_coupe && let Some(n) = projet.niveau(niveau) { *coupe = (z - n.altitude).round(); }
                    if (vue.orientation_plan - orientation).abs() > 1e-5 { vue.orientation_plan = orientation; }
                }
                if self.coupe != vue.coupe_active.unwrap_or(true) { vue.coupe_active = Some(self.coupe); }
                // vue caméra : point de vue et champ de la caméra (mm, au mm près)
                if let (crate::modele::GenreVue::Camera { oeil, cible, champ }, Some(o)) = (&mut vue.genre, origine) {
                    let mm = |p: glam::Vec3| ((p.as_dvec3() + o) * 1000.0).round().to_array();
                    let (no, nc, nf) = (mm(self.camera.eye()), mm(self.camera.target), (self.camera.fov_y.to_degrees() as f64 * 10.0).round() / 10.0);
                    if *oeil != no || *cible != nc || (*champ - nf).abs() > 0.05 { (*oeil, *cible, *champ) = (no, nc, nf); }
                }
                let poche = self.poche.nom();
                if poche != vue.poche_vue.as_deref().unwrap_or("hachure") { vue.poche_vue = Some(poche.into()); }
                if vue != projet.vues[index] {
                    let avant = projet.clone();
                    projet.vues[index] = vue;
                    let resultat = if let Some(c) = &mut self.collaboration { c.editer(&avant, projet) } else { self.historique.pousser(&avant, projet) };
                    if let Err(e) = resultat { *projet = avant; self.error = Some(e); }
                    self.natif_modifie = true;
                    self.bim.revision = self.bim.revision.wrapping_add(1);
                }
            }
        }
        self.enregistrer_rendu_vue();
        self.etats_vues.insert(self.vue_active, EtatVue {
            camera: self.camera.clone(),
            hidden_classes: self.hidden_classes.clone(),
            hidden_objects: self.hidden_objects.clone(),
            hidden_storeys: self.hidden_storeys.clone(),
            isolated: self.isolated.clone(),
            style: self.style,
            faces_blanches: self.faces_blanches,
            eclairage: self.eclairage,
            traits_fins: self.traits_fins,
            ombres_portees: self.ombres_portees,
            ombre_ambiante: self.ombre_ambiante,
            soleil_jour: self.soleil_jour,
            soleil_mois: self.soleil_mois,
            soleil_heure: self.soleil_heure,
            ciel: self.ciel.clone(),
            coupe: self.coupe,
            coupe_niveau: self.coupe_niveau,
            coupe_hauteur: self.coupe_hauteur,
            poche: self.poche,
            echelle: self.echelle,
            detail: self.detail,
            boite: self.boite,
            boite_active: self.boite_active,
        });
    }

    pub(super) fn restaurer_vue(&mut self, vue: Option<usize>) {
        let Some(e) = self.etats_vues.get(&vue).cloned() else { return };
        self.vue_active = vue;
        self.nomenclature_ouverte = vue.filter(|&i| self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.get(i)).is_some_and(|v| v.genre == "Nomenclature"));
        self.camera = e.camera;
        self.hidden_classes = e.hidden_classes;
        self.hidden_objects = e.hidden_objects;
        self.hidden_storeys = e.hidden_storeys;
        self.isolated = e.isolated;
        self.style = e.style;
        self.faces_blanches = e.faces_blanches;
        self.eclairage = e.eclairage;
        self.traits_fins = e.traits_fins;
        self.ombres_portees = e.ombres_portees;
        self.ombre_ambiante = e.ombre_ambiante;
        self.soleil_jour = e.soleil_jour;
        self.soleil_mois = e.soleil_mois;
        self.soleil_heure = e.soleil_heure;
        self.ciel = e.ciel;
        self.coupe = e.coupe;
        self.coupe_niveau = e.coupe_niveau;
        self.coupe_hauteur = e.coupe_hauteur;
        self.poche = e.poche;
        self.echelle = e.echelle;
        self.detail = e.detail;
        self.boite = e.boite;
        self.boite_active = e.boite_active;
        self.vol = Vol::default();
        self.point_clique = None;
        self.visibility_dirty = true;
        self.coupe_dirty = true;
        self.preparer_feuille(vue);
    }

    /// Ferme les autres vues sans fermer les documents ni perdre leurs modifications.
    pub(super) fn garder_derniere_vue_par_document(&mut self) {
        for d in self.documents.values_mut() {
            if let Some(&derniere) = d.vues.last() {
                d.vues.retain(|&vue| vue == derniere);
                d.etats.retain(|&vue, _| vue == derniere);
                d.active = derniere;
            }
        }
        if self.vues_ouvertes.len() > 1 {
            if let Some(&derniere) = self.vues_ouvertes.last() {
                self.memoriser_vue();
                self.restaurer_vue(derniere);
                self.vues_ouvertes.retain(|&vue| vue == derniere);
                self.etats_vues.retain(|&vue, _| vue == derniere);
            }
        }
    }

    pub(super) fn onglets_vues(&mut self, ui: &mut egui::Ui) {
        /// Identifiant de l'onglet de l'éditeur de familles (hors des documents).
        const ONGLET_FAMILLE: usize = usize::MAX;
        let mut onglets = Vec::new();
        let mut titres = BTreeMap::new();
        let mut ajouter = |id: usize, scene: Option<&Scene>, rvt: Option<&RvtInfo>, vues: &[Option<usize>], modifie: bool| {
            let fichier = scene.map(|s| s.file_name.as_str()).or_else(|| rvt.map(|r| r.file_name.as_str())).unwrap_or("Modèle");
            titres.insert(id, fichier.to_string());
            for &vue in vues {
                let nom = vue.and_then(|i| scene?.navigateur.as_ref()?.vues.get(i)).map_or("Vue 3D", |v| v.nom.as_str());
                onglets.push((id, vue, format!("{nom}{}", if modifie { " *" } else { "" }), format!("{fichier} — {nom}")));
            }
            if vues.is_empty() {
                let nom = if scene.is_some() { "Aucune vue" } else { "Fiche Revit" };
                onglets.push((id, None, nom.to_string(), fichier.to_string()));
            }
        };
        for (&id, d) in &self.documents {
            ajouter(id, d.scene.as_ref(), d.rvt.as_ref(), &d.vues, d.modifie);
        }
        ajouter(self.document_actif, self.scene.as_ref(), self.rvt.as_ref(), &self.vues_ouvertes, self.natif_modifie);
        onglets.sort_by_key(|o| o.0);
        // éditeur de familles : un onglet à part, après les vues
        // éditeur de familles : un onglet par famille ouverte (rang dans `vue`)
        for (ordre, t, _) in self.onglets_familles() {
            titres.insert(ONGLET_FAMILLE, "Éditeur de familles".into());
            onglets.push((ONGLET_FAMILLE, Some(ordre), format!("Famille {t}"), format!("Éditeur de familles — {t}")));
        }
        let famille_active = self.famille_active && self.famille.is_some();
        let ordre_actif = self.ordre_famille();
        let est_actif = |id: usize, vue: Option<usize>, doc: usize, active: Option<usize>| if id == ONGLET_FAMILLE { famille_active && vue == ordre_actif } else { !famille_active && id == doc && vue == active };
        let mut activer = None;
        let mut fermer = None;
        // Les sélecteurs disposent de leurs propres emplacements, indépendants des onglets.
        let (barre, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::hover());
        let marge = 4.0;
        let fleche = 24.0;
        let bouton_fermeture = 28.0;
        let disponible = (barre.width() - 2.0 * (marge + fleche + 6.0) - bouton_fermeture).max(1.0);
        let rect_fermeture = egui::Rect::from_min_size(
            egui::pos2(barre.right() - marge - fleche - bouton_fermeture, barre.top() + 2.0),
            egui::vec2(24.0, 22.0),
        );
        let plusieurs_vues = self.vues_ouvertes.len() > 1 || self.documents.values().any(|d| d.vues.len() > 1);
        let fermeture = ui.put(rect_fermeture, egui::Button::new("").min_size(rect_fermeture.size()).sense(
            if plusieurs_vues { Sense::click() } else { Sense::hover() },
        )).on_hover_text("Fermer les autres onglets : garder la dernière vue ouverte de chaque document");
        fermeture.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, plusieurs_vues, "Fermer les autres onglets"));
        dessiner_croix_onglet(ui.painter(), rect_fermeture.center(), if plusieurs_vues { ui.visuals().text_color() } else { ui.visuals().weak_text_color() });
        let reduire = fermeture.clicked();
        let nombre = ((disponible / 180.0).floor() as usize).max(1).min(onglets.len().max(1));
        let actif = onglets.iter().position(|o| est_actif(o.0, o.1, self.document_actif, self.vue_active)).unwrap_or(0);
        let debut = actif.saturating_sub(nombre / 2).min(onglets.len().saturating_sub(nombre));
        let largeur = (disponible / nombre as f32).min(220.0);
        for droite in [false, true] {
            let x = if droite { barre.right() - marge - fleche } else { barre.left() + marge };
            let rect = egui::Rect::from_min_size(egui::pos2(x, barre.top() + 2.0), egui::vec2(fleche, 22.0));
            ui.scope_builder(egui::UiBuilder::new().id_salt(("selecteur_vues", droite)).max_rect(rect).layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)), |ui| {
                ui.spacing_mut().button_padding = egui::Vec2::ZERO;
                let bouton = egui::Button::new("").min_size(rect.size());
                let (reponse, _) = egui::containers::menu::MenuButton::from_button(bouton).ui(ui, |ui| {
                let mut precedent = None;
                for (id, vue, nom, infobulle) in &onglets {
                    if precedent != Some(*id) {
                        if precedent.is_some() {
                            ui.separator();
                        }
                        ui.label(RichText::new(&titres[id]).strong().color(couleur_document(*id)));
                        precedent = Some(*id);
                    }
                    if ui.selectable_label(est_actif(*id, *vue, self.document_actif, self.vue_active), RichText::new(nom).color(couleur_document(*id))).on_hover_text(infobulle).clicked() {
                        activer = Some((*id, *vue));
                        ui.close();
                    }
                }
                });
                let centre = rect.center();
                let sens = if droite { 1.0 } else { -1.0 };
                let points = vec![centre + egui::vec2(-sens * 2.0, -4.0), centre + egui::vec2(sens * 2.0, 0.0), centre + egui::vec2(-sens * 2.0, 4.0)];
                ui.painter().add(egui::Shape::convex_polygon(points, ui.visuals().text_color(), egui::Stroke::NONE));
                reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Liste des onglets ouverts"));
                reponse.on_hover_text("Liste des onglets ouverts");
            });
        }
        for (k, (id, vue, nom, infobulle)) in onglets.iter().skip(debut).take(nombre).enumerate() {
                        let x = barre.left() + marge + fleche + 6.0 + k as f32 * largeur;
                        let rect = egui::Rect::from_min_size(egui::pos2(x, barre.top() + 2.0), egui::vec2((largeur - 6.0).max(1.0), 22.0));
                        let couleur = couleur_document(*id);
                        let actif = est_actif(*id, *vue, self.document_actif, self.vue_active);
                        let diluer = |c: u8| ((c as u16 + 255 * if actif { 3 } else { 5 }) / if actif { 4 } else { 6 }) as u8;
                        let fond = Color32::from_rgb(diluer(couleur.r()), diluer(couleur.g()), diluer(couleur.b()));
                        let survole = ui.rect_contains_pointer(rect);
                        ui.painter().rect_filled(rect, 4.0, fond);
                        ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(if actif { 1.5 } else { 0.5 }, couleur), egui::StrokeKind::Inside);
                        let rect_croix = egui::Rect::from_min_max(egui::pos2((rect.right() - 21.0).max(rect.left()), rect.top() + 2.0), egui::pos2(rect.right() - 2.0, rect.bottom() - 2.0));
                        let rect_corps = egui::Rect::from_min_max(rect.min, egui::pos2(rect_croix.left(), rect.bottom()));
                        let reponse = ui.interact(rect_corps, ui.scope_id().with(("onglet_vue", id, vue)), Sense::click()).on_hover_text(infobulle);
                        let texte_rect = rect_corps.shrink2(egui::vec2(7.0, 2.0));
                        // texte non sélectionnable : le clic revient à l'onglet
                        ui.put(texte_rect, egui::Label::new(RichText::new(nom).color(ui.visuals().text_color())).truncate().selectable(false).sense(Sense::hover()));
                        if reponse.clicked() {
                            activer = Some((*id, *vue));
                        }
                        let fermer_clic = if survole {
                            let reponse = ui.put(rect_croix, egui::Button::new("").min_size(rect_croix.size()).frame(false)).on_hover_text("Fermer la vue");
                            reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Fermer la vue"));
                            dessiner_croix_onglet(ui.painter(), rect_croix.center(), ui.visuals().text_color());
                            reponse.clicked()
                        } else { false };
                        if reponse.clicked_by(PointerButton::Middle) || fermer_clic {
                            fermer = Some((*id, *vue));
                        }
                        if k + 1 < nombre && onglets.get(debut + k + 1).is_some_and(|suivant| suivant.0 != *id) {
                            let x = rect.right() + 3.0;
                            ui.painter().line_segment([egui::pos2(x, rect.top() + 4.0), egui::pos2(x, rect.bottom() - 4.0)], egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color));
                        }
        }
        if reduire {
            self.garder_derniere_vue_par_document();
        }
        if let Some((id, vue)) = activer {
            self.famille_active = id == ONGLET_FAMILLE;
            if let (ONGLET_FAMILLE, Some(ordre)) = (id, vue) {
                self.activer_famille(ordre);
            }
        }
        if let Some((id, vue)) = activer.filter(|o| o.0 != ONGLET_FAMILLE) {
            self.changer_document(id);
            if let Some(i) = vue {
                self.activer_vue(i);
            } else {
                self.memoriser_vue();
                self.restaurer_vue(None);
            }
        }
        if let Some((ONGLET_FAMILLE, Some(ordre))) = fermer {
            self.fermer_onglet_famille(ordre);
        }
        if let Some((id, cible)) = fermer.filter(|o| o.0 != ONGLET_FAMILLE) {
            self.fermer_onglet_projet(id,cible);
        }

        if self.scene.is_some() && self.vues_ouvertes.is_empty() && ui.button("Ouvrir la vue 3D du modèle").clicked() {
            if self.natif.is_some() {
                let indice = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.iter().position(|v| v.revit_id == scene::numero_vue("3d")));
                if let Some(i) = indice { self.activer_vue(i); return; }
            }
            self.vues_ouvertes.push(None);
            self.vue_active = None;
            self.nomenclature_ouverte = None;
            self.hidden_objects.clear();
        self.masques_utilisateur.clear();
            self.hidden_storeys.clear();
            self.hidden_classes = HIDDEN_BY_DEFAULT.iter().map(|s| s.to_string()).collect();
            self.isolated = None;
            self.isolement_utilisateur = false;
            self.style = StyleVisuel::Graphique;
            self.coupe = false;
            self.boite = None;
            self.boite_active = false;
            self.camera = Camera::default();
            if let Some(s) = &self.scene {
                self.camera.scene_radius = (s.max - s.min).length() * 0.5;
            }
            self.visibility_dirty = true;
            self.coupe_dirty = true;
            self.preparer_feuille(None);
            self.update_visibility();
            self.fit_all();
        }
    }

    /// Ouvre une vue Revit comme dans Revit : caméra orthographique orientée selon la
    /// vue ; plan : son niveau seul ; coupe, élévation : tout le modèle ; vue de dessin,
    /// feuille, légende : ses éléments 2D ; vue 3D : modèle entier. Les éléments 2D de la
    /// vue sont affichés.
    pub(super) fn activer_vue(&mut self, i: usize) {
        if self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.get(i)).is_none() {
            return;
        }
        if self.vue_active == Some(i) && self.vues_ouvertes.contains(&Some(i)) {
            return;
        }
        self.orientation_nord_en_cours = false;
        self.arete_alignement = None;
        self.memoriser_vue();
        if self.etats_vues.contains_key(&Some(i)) {
            self.restaurer_vue(Some(i));
            return;
        }
        self.vues_ouvertes.push(Some(i));
        self.nomenclature_ouverte = None;
        self.boite = None;
        self.boite_active = false;
        self.coupe = false;
        self.coupe_niveau = None;
        self.vol = Vol::default();
        self.point_clique = None;
        let Some(s) = &self.scene else { return };
        let Some(v) = s.navigateur.as_ref().and_then(|n| n.vues.get(i)) else { return };
        let annot = v.entite.map(|e| self.objects_under(e)).unwrap_or_default();
        let modele: Vec<u32> = (0..s.objects.len() as u32).filter(|&o| &*s.objects[o as usize].class != "IFCANNOTATION").collect();
        let (objs, cadre): (Vec<u32>, Vec<u32>) = match v.genre.as_str() {
            "Vue 3D" => (Vec::new(), modele.iter().copied().filter(|&o| !HIDDEN_BY_DEFAULT.contains(&&*s.objects[o as usize].class)).collect()),
            "Plan d'étage" | "Plan de plafond" => {
                let etage: Vec<u32> = modele.iter().copied().filter(|&o| (v.etage.is_some() && s.objects[o as usize].storey == v.etage) || &*s.objects[o as usize].class == "IFCGEOGRAPHICELEMENT").collect();
                let tous = etage.iter().chain(&annot).copied().collect();
                (tous, if etage.is_empty() { modele.clone() } else { etage })
            }
            "Coupe" | "Élévation" => (modele.iter().chain(&annot).copied().collect(), modele.clone()),
            "Jeu de feuilles" => {
                self.vue_active = Some(i);
                self.memoriser_vue();
                return;
            }
            "Nomenclature" => {
                if let Some(e) = v.entite {
                    self.select(Some(e), false);
                }
                self.nomenclature_ouverte = Some(i);
                self.vue_active = Some(i);
                self.memoriser_vue();
                return;
            }
            _ => (annot.clone(), annot.clone()),
        };
        let (d, haut, entite) = (v.direction.as_vec3(), v.haut.as_vec3(), v.entite);
        let genre_feuille = v.genre == "Feuille";
        let coupe_vue = v.coupe;
        let coupe_verticale = v.plan_coupe.is_some();
        // mode dessin pour les plans, coupes et élévations ; coupe à hauteur sur le niveau
        // d'un plan d'étage
        let (genre, etage, echelle) = (v.genre.clone(), v.etage, v.echelle);
        if let Some(d) = NiveauDetail::depuis_revit(v.detail) {
            self.detail = d;
        }
        match genre.as_str() {
            "Plan d'étage" | "Plan de plafond" | "Coupe" | "Élévation" => {
                // dessin au trait des plans, coupes et élévations
                self.style = StyleVisuel::Graphique;
                self.faces_blanches = true;
                self.coupe = genre == "Plan d'étage" || coupe_verticale;
                if self.coupe && !coupe_verticale {
                    self.coupe_niveau = etage;
                    // hauteur du plan de coupe de la plage de vue
                    if let Some(h) = coupe_vue {
                        self.coupe_hauteur = h as f32;
                    }
                }
            }
            // vue 3D : le style en cours, faces colorées en sortant d'un dessin
            "Vue 3D" | "Feuille" => self.faces_blanches = false,
            _ => {}
        }
        if let Some(vue) = self.natif.as_ref().and_then(|(p, _)| p.vues.iter().find(|vue| scene::numero_vue(&vue.id) == v.revit_id)) {
            if let Some(active) = vue.coupe_active { self.coupe = active; }
            self.poche = match vue.poche_vue.as_deref() {
                Some("plein") => Poche::Plein,
                Some("vide") => Poche::Vide,
                Some("materiau") => Poche::Materiau,
                _ => Poche::Hachure,
            };
        }
        // réglages de rendu enregistrés (style, ombres, soleil, ciel)
        let id_rendu = if scene::numero_vue("3d") == v.revit_id { Some("3d".to_string()) } else { self.natif.as_ref().and_then(|(p, _)| p.vues.iter().find(|vue| scene::numero_vue(&vue.id) == v.revit_id)).map(|vue| vue.id.clone()) };
        if let Some(id) = id_rendu {
            self.appliquer_rendu_vue(&id);
        }
        // échelle de la barre de vue : celle de la vue ouverte
        if echelle > 0.0 {
            self.echelle = echelle;
        }
        self.vue_active = Some(i);
        self.hidden_objects.clear();
        self.masques_utilisateur.clear();
        self.hidden_storeys.clear();
        // visibilité propre à chaque vue : celle par défaut, puis les objets isolés
        self.hidden_classes = HIDDEN_BY_DEFAULT.iter().map(|s| s.to_string()).collect();
        if objs.is_empty() {
            self.isolated = None;
            self.isolement_utilisateur = false;
            self.visibility_dirty = true;
        } else {
            self.isolate(objs);
            // isolement propre à la vue (objets de l'étage), pas un masquage temporaire
            self.isolement_utilisateur = false;
        }
        // filtre de phase de la vue : objets dont l'état dans la phase de la vue est masqué
        let masques = self.masques_de_phase(i);
        self.hidden_objects.extend(masques);
        let secondaires = self.masques_variantes();
        self.hidden_objects.extend(secondaires);
        // vues placées d'une feuille, selon la visibilité de la vue (annotations montrées)
        if genre_feuille && self.isolated.is_some() {
            self.hidden_classes.remove("IFCANNOTATION");
        }
        self.preparer_feuille(Some(i));
        // caméra : regarde vers −direction ; vue de dessus orientée par le haut de la vue
        self.camera.ortho = true;
        if d.length() > 0.5 {
            let d = d.normalize();
            self.camera.pitch = d.z.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
            self.camera.yaw = if d.z.abs() > 0.999 { (-haut.y).atan2(-haut.x) } else { d.y.atan2(d.x) };
        }
        if !cadre.is_empty() {
            self.frame_objects(&cadre);
        }
        if genre_feuille {
            self.cadrer_rectangle(&cadre);
        }
        // vue 3D : projection enregistrée, point de vue d'une vue caméra
        self.pieton = false;
        self.appliquer_camera_vue();
        if let Some(e) = entite {
            self.select(Some(e), false);
        }
        self.memoriser_vue();
    }

    /// Objets des options de variante secondaires (masqués par défaut, comme dans Revit).
    pub(super) fn masques_variantes(&self) -> Vec<u32> {
        let Some(s) = &self.scene else { return Vec::new() };
        let Some(nav) = &s.navigateur else { return Vec::new() };
        if nav.variantes_secondaires.is_empty() {
            return Vec::new();
        }
        (0..s.objects.len() as u32).filter(|&o| nav.variantes_secondaires.contains(&s.objects[o as usize].id)).collect()
    }

    /// Objets qu'une vue Revit ne montre pas selon sa phase et son filtre de phase (état
    /// de l'objet dans la phase, présentation « masqué » du filtre).
    pub(super) fn masques_de_phase(&self, i: usize) -> Vec<u32> {
        let Some(s) = &self.scene else { return Vec::new() };
        let Some(nav) = &s.navigateur else { return Vec::new() };
        let (Some(v), Some(filtre)) = (nav.vues.get(i), nav.vues.get(i).and_then(|v| nav.filtres_phase.get(&v.filtre_phase))) else { return Vec::new() };
        (0..s.objects.len() as u32)
            .filter(|&o| {
                nav.phases_objets.get(&s.objects[o as usize].id).is_some_and(|&(c, d)| filtre[scene::etat_de_phase(&nav.phases, c, d, v.phase)] == 0)
            })
            .collect()
    }

    /// Vue orthographique de dessus ajustée au rectangle des objets et des fenêtres de
    /// vue d'une feuille, selon les proportions de la vue.
    pub(super) fn cadrer_rectangle(&mut self, objs: &[u32]) {
        let Some(s) = &self.scene else { return };
        let (mut mn, mut mx) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for &o in objs {
            mn = mn.min(s.objects[o as usize].min);
            mx = mx.max(s.objects[o as usize].max);
        }
        // cadrages des vues placées ; à défaut (vues non cadrées), étendue de leurs objets
        let cadres: Vec<([f32; 4], f32)> = self.passes_feuille.iter().filter_map(|p| Some((p.cadre?, p.z))).collect();
        let rects = if cadres.is_empty() { self.passes_feuille.iter().map(|p| (p.etendue, p.z)).collect() } else { cadres };
        for (c, z) in rects {
            mn = mn.min(Vec3::new(c[0], c[1], z));
            mx = mx.max(Vec3::new(c[2], c[3], z));
        }
        if mn.x > mx.x {
            return;
        }
        let aspect = if self.rect_vue.height() > 0.0 { self.rect_vue.width() / self.rect_vue.height() } else { 1.0 };
        let (l, h) = (mx.x - mn.x, mx.y - mn.y);
        let demi = (h / 2.0).max(l / 2.0 / aspect) * 1.04;
        self.camera.target = (mn + mx) / 2.0;
        self.camera.dist = demi / (self.camera.fov_y / 2.0).tan();
    }

    /// Passes des fenêtres de vue de la feuille `i` (aucune pour une autre vue) : pour
    /// chaque vue placée, ses objets du modèle (cadrés et découpés en profondeur) puis ses
    /// annotations, tracés à leur place sur la feuille.
    pub(super) fn preparer_feuille(&mut self, i: Option<usize>) {
        let mut passes = Vec::new();
        let mut indices: Vec<[Vec<u32>; 3]> = Vec::new();
        let mut objets_feuille = Vec::new();
        if let Some(s) = &self.scene {
            let nav = s.navigateur.as_ref();
            let feuille = i.and_then(|i| nav?.vues.get(i)).filter(|v| v.genre == "Feuille");
            let o = s.origin;
            // catégorie montrée (boîte de visibilité)
            let montre = |k: &u32| !self.hidden_classes.contains(&*s.objects[*k as usize].class) && !self.hidden_classes.contains(&*self.cle_objet[*k as usize]);
            let modele: Vec<u32> = (0..s.objects.len() as u32).filter(|&k| !HIDDEN_BY_DEFAULT.contains(&&*s.objects[k as usize].class)).collect();
            for f in feuille.map(|v| v.fenetres.as_slice()).unwrap_or_default() {
                let Some(pv) = nav.and_then(|n| n.vues.iter().find(|x| x.revit_id == f.vue)) else { continue };
                // repère de la scène : q = p − origine, pour le modèle comme pour la feuille
                let m = DMat4::from_translation(-o) * f.vers_feuille * DMat4::from_translation(o);
                let rect = |c: [f64; 4]| [(c[0] - o.x) as f32, (c[1] - o.y) as f32, (c[2] - o.x) as f32, (c[3] - o.y) as f32];
                let plans = f.plans.iter().map(|p| Vec4::new(p.x as f32, p.y as f32, p.z as f32, (p.w + p.truncate().dot(o)) as f32)).collect();
                let dessin = matches!(pv.genre.as_str(), "Vue de dessin" | "Légende" | "Nomenclature");
                // objets du modèle, sauf ceux masqués dans la vue placée (id Revit : 8ᵉ
                // argument, l'étiquette, des produits de la scène)
                let masque = |k: &u32| match s.model.get(s.objects[*k as usize].id).and_then(|e| e.args.get(7)) {
                    Some(crate::formats::ifc::step::Value::Str(t)) => t.parse::<i64>().is_ok_and(|id| f.masques.contains(&id)),
                    _ => false,
                };
                let objets: Vec<u32> = if dessin { Vec::new() } else { modele.iter().filter(|k| !masque(k)).copied().collect() };
                // éléments de détail (traits, régions, composants de détail) découpés au
                // cadrage comme le modèle ; annotations (textes, cotes, symboles) non
                // découpées, sauf cadrage d'annotation
                let genre = |k: u32| match s.model.get(s.objects[k as usize].id).and_then(|e| e.args.get(4)) {
                    Some(crate::formats::ifc::step::Value::Str(g)) => g.clone(),
                    _ => String::new(),
                };
                let (detail, annot): (Vec<u32>, Vec<u32>) = pv.entite.map(|e| self.objects_under(e)).unwrap_or_default().into_iter().partition(|&k| DETAIL.contains(&genre(k).as_str()));
                let passes_vue = [(objets, f.cadre.map(rect), plans), (detail, f.cadre.map(rect), Vec::new()), (annot, f.cadre_annotations.map(rect), Vec::new())];
                for (mut objs, cadre, plans) in passes_vue {
                    objets_feuille.extend_from_slice(&objs);
                    objs.retain(montre);
                    if objs.is_empty() {
                        continue;
                    }
                    let mut ix: [Vec<u32>; 3] = Default::default();
                    for &k in &objs {
                        let ob = &s.objects[k as usize];
                        ix[0].extend(ob.opaque.start..ob.opaque.start + ob.opaque.count);
                        ix[1].extend(ob.transparent.start..ob.transparent.start + ob.transparent.count);
                        ix[2].extend(ob.lines.start..ob.lines.start + ob.lines.count);
                    }
                    indices.push(ix);
                    let mf = m.as_mat4();
                    let mut etendue = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                    for &k in &objs {
                        let ob = &s.objects[k as usize];
                        for c in 0..8 {
                            let q = Vec3::new([ob.min.x, ob.max.x][c & 1], [ob.min.y, ob.max.y][(c >> 1) & 1], [ob.min.z, ob.max.z][c >> 2]);
                            let q = mf.transform_point3(q);
                            etendue = [etendue[0].min(q.x), etendue[1].min(q.y), etendue[2].max(q.x), etendue[3].max(q.y)];
                        }
                    }
                    passes.push(PasseFeuille { vers_feuille: mf, cadre, etendue, z: -o.z as f32, plans, direction: f.direction.as_vec3() });
                }
            }
        }
        if let Some(r) = self.renderer.as_mut() {
            r.upload_passes(&indices);
        }
        self.passes_feuille = passes;
        // un objet de plusieurs vues placées compte une fois
        objets_feuille.sort_unstable();
        objets_feuille.dedup();
        self.objets_feuille = objets_feuille;
    }

}
