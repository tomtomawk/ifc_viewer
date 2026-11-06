//! Ouverture, chargement, enregistrement et onglets de documents.
use super::*;

impl ViewerApp {
    pub(super) fn open(&mut self, path: PathBuf, ctx: &egui::Context) {
        let recuperation=crate::modele::echanges::enregistrement::chemin_recuperation(&path);
        let restaurer=if path.extension().is_some_and(|e|e.eq_ignore_ascii_case("json")) && recuperation.exists() {
            match rfd::MessageDialog::new().set_title("Récupération du projet").set_description("Une copie de récupération existe. Restaurer les modifications non enregistrées ?").set_buttons(rfd::MessageButtons::YesNoCancel).show() {
                rfd::MessageDialogResult::Yes=>true,
                rfd::MessageDialogResult::No=>false,
                _=>return,
            }
        } else {false};
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        self.partage_en_cours=false;
        self.loading = Some(rx);
        self.error = None;
        self.status = format!("Chargement de {}…", path.display());
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || {
                let progress = |s: String| {
                    let _ = tx.send(LoadMsg::Progress(s));
                    ctx.request_repaint();
                };
                if crate::modele::collaboration::partage::extension_partage(&path) {
                    let res=Self::charger_partage(&path,&progress);
                    let _=tx.send(LoadMsg::Partage(res,None));
                } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
                    match std::fs::read_to_string(if restaurer{&recuperation}else{&path}).map_err(|e|format!("Lecture impossible : {e}")) {
                        Ok(json) if crate::modele::collaboration::partage::contenu_partage(&json)=>{let _=tx.send(LoadMsg::Partage(Self::charger_partage(&path,&progress),None));},
                        texte=>{
                            let res=texte.and_then(|json|crate::modele::Projet::lire_json(&json))
                                .and_then(|projet|scene::depuis_natif(&projet,&path).map(|scene|(Box::new(scene),projet,path)));
                            let _=tx.send(LoadMsg::Natif(res));
                        }
                    }
                } else if is_revit_file(&path) {
                    progress(format!("Lecture des métadonnées Revit de {}…", path.display()));
                    let res = crate::modele::echanges::enregistrement::SourceRvt::ouvrir(&path).and_then(|source| rvt::load(&path).map(|info| {
                        let titre = info.title.clone().unwrap_or_else(|| info.file_name.clone());
                        let format = info.format().unwrap_or("?").to_string();
                        let scene = rvt::maquette::load_scene(&path, &titre, &format, &progress).map(Box::new);
                        (Box::new(info), scene, source)
                    }));
                    let _ = tx.send(LoadMsg::Revit(res));
                } else {
                    let res = scene::load(&path, &progress).map(Box::new);
                    let _ = tx.send(LoadMsg::Done(res));
                }
                ctx.request_repaint();
            })
            .expect("thread de chargement");
    }

    pub(super) fn poll_loading(&mut self) {
        let Some(rx) = &self.loading else { return };
        loop {
            let msg=match rx.try_recv() {
                Ok(msg)=>msg,
                Err(std::sync::mpsc::TryRecvError::Empty)=>return,
                Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
                    self.loading=None;self.partage_en_cours=false;self.error=Some("Le chargement a été interrompu ; le document précédent est conservé.".into());return;
                }
            };
            match msg {
                LoadMsg::Partage(Ok((scene,projet,chemin,session)),remplacer)=>{
                    self.loading=None;self.partage_en_cours=false;
                    if remplacer==Some(self.document_actif){self.installer_scene_courante(*scene);self.historique=Default::default();}
                    else{self.install_scene(*scene);}
                    let _=self.historique.enregistrer(&projet);
                    self.status=format!("{} — projet partagé",projet.nom);
                    self.natif=Some((projet,chemin));self.collaboration=Some(session);self.natif_modifie=false;self.avant_saisie=None;
                    self.ouvrir_plan_initial_natif();return;
                }
                LoadMsg::Partage(Err(e),_)=>{self.loading=None;self.partage_en_cours=false;self.error=Some(e);return;}
                LoadMsg::Natif(Ok((scene, projet, chemin))) => {
                    self.loading = None;
                    self.install_scene(*scene);
                    self.status = format!("{} — projet natif v0", projet.nom);
                    let _ = self.historique.enregistrer(&projet);
                    if let Ok(ancien)=std::fs::read_to_string(&chemin).map_err(|e|e.to_string()).and_then(|s|crate::modele::Projet::lire_json(&s)) {
                        let _=self.historique.enregistrer(&ancien);
                        self.natif_modifie=self.historique.modifie(&projet);
                    }
                    self.natif = Some((projet, chemin));
                    self.rouvrir_collaboration();
                    self.avant_saisie = None;
                    self.ouvrir_plan_initial_natif();
                    return;
                }
                LoadMsg::Progress(s) => self.status = s,
                LoadMsg::Done(Ok(scene)) => {
                    self.loading = None;
                    self.install_scene(*scene);
                    return;
                }
                LoadMsg::Revit(Ok((info, scene, source))) => {
                    self.loading = None;
                    let head = format!("{} — {} Revit {}", info.file_name, info.kind, info.format().unwrap_or("?"));
                    match scene {
                        Ok(scene) => {
                            let summary = format!(
                                "{} objets, {} triangles, lus dans le fichier en {:.2} s",
                                scene.objects.len(),
                                scene.triangles,
                                scene.load_ms as f64 / 1000.0
                            );
                            self.install_scene(*scene);
                            self.status = format!("{head} — {summary}");
                            self.rvt_geom = Ok(summary);
                            self.rvt_sheet = false;
                        }
                        Err(e) => {
                            self.nouveau_document();
                            self.vues_ouvertes.clear();
                            self.etats_vues.clear();
                            self.vue_active = None;
                            self.nomenclature_ouverte = None;
                            self.status = format!("{head} — métadonnées seules ({e})");
                            self.rvt_geom = Err(e);
                            self.rvt_sheet = true;
                        }
                    }
                    self.rvt_preview = None;
                    self.rvt = Some(*info);
                    self.rvt_source = Some(source);
                    self.rvt_sortie = None;
                    return;
                }
                LoadMsg::Natif(Err(e)) | LoadMsg::Revit(Err(e)) | LoadMsg::Done(Err(e)) => {
                    self.loading = None;
                    self.status = "Échec du chargement.".into();
                    self.error = Some(e);
                    return;
                }
            }
        }
    }

    pub(super) fn install_scene(&mut self, scene: Scene) {
        self.nouveau_document();
        self.installer_scene_courante(scene);
    }
    pub(super) fn installer_scene_courante(&mut self, scene: Scene) {
        if let Some(r) = self.renderer.as_mut() {
            r.upload_vertices(&scene.vertices, &scene.couleurs_rendu, &scene.motifs_surface);
        }
        self.status = format!(
            "{} — {} — {} entités, {} objets 3D, {} triangles — {:.2} s",
            scene.file_name,
            if scene.model.file.schema.is_empty() { "schéma ?" } else { &scene.model.file.schema },
            scene.model.file.entities.len(),
            scene.objects.len(),
            scene.triangles,
            scene.load_ms as f64 / 1000.0,
        );
        self.rvt = None;
        self.rvt_sheet = false;
        self.rvt_preview = None;
        self.camera = Camera::default();
        self.camera.scene_radius = (scene.max - scene.min).length() * 0.5;
        self.camera.fit(scene.min, scene.max);
        self.selected = None;
        self.selection_multiple.clear();
        self.fenetre_selection = None;
        self.reveal = None;
        self.nomenclature_ouverte = None;
        self.point_clique = None;
        self.filter.clear();
        self.coupe = false;
        self.coupe_dirty = true;
        self.vol = Vol::default();
        self.hidden_objects.clear();
        self.masques_utilisateur.clear();
        self.hidden_storeys.clear();
        self.isolated = None;
        self.hidden_classes = HIDDEN_BY_DEFAULT.iter().map(|s| s.to_string()).collect();
        // fichier sans objet visible par défaut (famille 2D de Revit : traits et textes
        // seulement) : on montre les annotations
        if scene.objects.iter().all(|o| self.hidden_classes.contains(&*o.class)) {
            self.hidden_classes.remove("IFCANNOTATION");
        }
        // cadrage sur les objets visibles par défaut (les vues de dessin et les feuilles
        // d'un projet Revit sont loin du modèle)
        let (mut mn, mut mx) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for o in scene.objects.iter().filter(|o| !self.hidden_classes.contains(&*o.class)) {
            mn = mn.min(o.min);
            mx = mx.max(o.max);
        }
        if mn.x <= mx.x {
            self.camera.scene_radius = (mx - mn).length() * 0.5;
            self.camera.fit(mn, mx);
        }
        self.visible = vec![true; scene.objects.len()];
        self.vue_active = None;
        self.style = StyleVisuel::Graphique;
        self.vues_ouvertes = vec![None];
        self.etats_vues.clear();
        self.coupe_niveau = None;
        self.liste_niveaux = niveaux(&scene);
        self.left_tab = if scene.navigateur.is_some() { LeftTab::Tout } else { LeftTab::Tree };
        self.liste_materiaux = match &scene.navigateur {
            Some(n) => n.materiaux.clone(),
            None => materiaux_ifc(&scene),
        };
        (self.categories, self.categorie_objet, self.cle_objet) = categories(&scene);
        self.boite = None;
        self.boite_active = false;
        self.scene = Some(scene);
        let secondaires = self.masques_variantes();
        self.hidden_objects.extend(secondaires);
        self.visibility_dirty = true;
        self.preparer_feuille(None);
    }

    pub(super) fn update_visibility(&mut self) {
        if !self.visibility_dirty {
            return;
        }
        self.visibility_dirty = false;
        let Some(scene) = &self.scene else { return };
        // coupe du mode dessin : objets du modèle entièrement du côté retiré écartés (au-dessus
        // d'un plan, devant la ligne d'une coupe)
        let plan = self.plan_coupe();
        let garde = |o: &scene::SceneObject| plan.is_none_or(|p| {
            let n = p.truncate();
            n.x * if n.x > 0.0 { o.max.x } else { o.min.x } + n.y * if n.y > 0.0 { o.max.y } else { o.min.y } + n.z * if n.z > 0.0 { o.max.z } else { o.min.z } + p.w >= 0.0
        });
        // limite basse du plan natif ouvert (repère de la scène) : ce qui se termine à cette
        // limite est celui du niveau d'en dessous, sauf les éléments horizontaux (dalles,
        // sols, toitures), qui appartiennent au niveau et dont le dessus affleure
        let limite_basse = self.coupe.then(|| self.vue_active.and_then(|i| scene.navigateur.as_ref()?.vues.get(i)?.limite_basse)).flatten().map(|z| (z - scene.origin.z) as f32);
        let dans_la_plage = |o: &scene::SceneObject| limite_basse.is_none_or(|bas| {
            if matches!(&*o.class, "IFCSLAB" | "IFCCOVERING" | "IFCROOF" | "IFCANNOTATION" | "IFCGEOGRAPHICELEMENT") { o.max.z >= bas - 0.002 } else { o.max.z > bas + 0.002 }
        });
        let masques_vue: HashSet<Id> = self.vue_native_id().and_then(|id| self.natif.as_ref()?.0.vues.iter().find(|v| v.id == id)).map(|vue| {
            scene.objects.iter().filter(|o| self.cle_native(o.id).is_some_and(|id| vue.elements_masques.contains(&id))
                || scene.model.parent.get(&o.id).and_then(|id| self.cle_native(*id)).is_some_and(|id| vue.elements_masques.contains(&id)))
                .map(|o| o.id).collect()
        }).unwrap_or_default();
        let mut opaque = Vec::new();
        let mut transp = Vec::new();
        let mut lines = Vec::new();
        let mut aretes = Vec::new();
        let mut objets_masques = HashSet::new();
        for (i, o) in scene.objects.iter().enumerate() {
            let en_edition = self.objet_en_edition().is_some_and(|(id, annotation)| !annotation && o.id == scene::id_natif("dalle", id))
                || self.terrain_en_edition().is_some_and(|id| o.id == scene::id_natif("terrain", id));
            // masqué par l'utilisateur (temporaire ou dans la vue) : montré en magenta dans le
            // mode « Afficher les éléments masqués »
            let masque_utilisateur = masques_vue.contains(&o.id) || (self.masques_utilisateur.contains(&(i as u32)) && self.hidden_objects.contains(&(i as u32)));
            let revele = self.afficher_masques && masque_utilisateur;
            if revele {
                objets_masques.insert(i as u32);
            }
            let vis = !en_edition && (revele || !masques_vue.contains(&o.id)) && !self.hidden_classes.contains(&*o.class)
                && !self.hidden_classes.contains(&*self.cle_objet[i])
                && (revele || !self.hidden_objects.contains(&(i as u32)))
                && o.storey.is_none_or(|s| !self.hidden_storeys.contains(&s))
                && self.isolated.as_ref().is_none_or(|iso| iso.contains(&(i as u32)))
                && dans_la_plage(o)
                && (garde(o) || &*o.class == "IFCANNOTATION");
            self.visible[i] = vis;
            if vis {
                opaque.extend(o.opaque.start..o.opaque.start + o.opaque.count);
                transp.extend(o.transparent.start..o.transparent.start + o.transparent.count);
                lines.extend(o.lines.start..o.lines.start + o.lines.count);
                aretes.extend(o.aretes.start..o.aretes.start + o.aretes.count);
            }
        }
        self.objets_masques = objets_masques;
        if let Some(r) = self.renderer.as_mut() {
            r.upload_indices(&opaque, &transp, &lines, &aretes);
        }
        self.coupe_dirty = true;
    }

    pub(super) fn creer_projet(&mut self) {
        if self.partage_en_cours {self.error=Some("Attendre la fin de la création du partage".into());return;}
        let Some(mut chemin) = rfd::FileDialog::new().set_title("Nouveau projet natif v0")
            .add_filter("Modèle natif v0", &["json"]).set_file_name("Nouveau projet.modele.json").save_file() else { return };
        if chemin.extension().is_none() {
            chemin.set_extension("modele.json");
        }
        let nom = chemin.file_stem().map(|n| n.to_string_lossy().trim_end_matches(".modele").to_string()).unwrap_or_else(|| "Nouveau projet".into());
        let projet = crate::modele::Projet::depuis_gabarit(&nom, &crate::config::gabarit::actif());
        let resultat = scene::depuis_natif(&projet, &chemin).and_then(|scene| {
            crate::modele::echanges::enregistrement::enregistrer_natif(&projet, &chemin)?;
            Ok(scene)
        });
        match resultat {
            Ok(scene) => {
                self.install_scene(scene);
                self.status = format!("{} — nouveau projet natif v0", projet.nom);
                let _ = self.historique.enregistrer(&projet);
                self.natif = Some((projet, chemin));
                self.avant_saisie = None;
                self.error = None;
                self.ouvrir_plan_initial_natif();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Le plan initial est ouvert seul ; la vue 3D nommée reste dans l'arborescence.
    pub(super) fn ouvrir_plan_initial_natif(&mut self) {
        let plan = self.natif.as_ref().and_then(|(p, _)| p.vues.iter().find(|v| matches!(v.genre, crate::modele::GenreVue::Plan { .. })).map(|v| scene::numero_vue(&v.id)));
        let indice = plan.and_then(|n| self.scene.as_ref()?.navigateur.as_ref()?.vues.iter().position(|v| v.revit_id == n));
        self.vues_ouvertes.clear();
        self.etats_vues.clear();
        self.vue_active = None;
        if let Some(i) = indice { self.activer_vue(i); }
    }

    pub(super) fn enregistrer_projet(&mut self, ctx: &egui::Context) {
        if self.loading.is_some() || self.sauvegarde.is_some() { return; }
        if self.natif.is_none() {
            self.enregistrer_rvt_dialogue(ctx);
            return;
        }
        let Some((projet, chemin)) = &self.natif else { return };
        match crate::modele::echanges::enregistrement::enregistrer_natif(projet, chemin) {
            Ok(()) => {
                self.natif_modifie = false;
                let _ = self.historique.enregistrer(projet);
                crate::modele::echanges::enregistrement::effacer_recuperation(chemin);
                self.status = format!("{} — enregistré", chemin.display());
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(super) fn confirmation_modifications(&mut self, ctx: &egui::Context) {
        let Some((application, document)) = self.confirmer_fermeture else { return };
        let mut action = None;
        egui::Modal::new(egui::Id::unique("modifications_non_enregistrees")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.heading("Enregistrer les modifications ?");
            ui.label(if application { "Des projets contiennent des modifications non enregistrées." } else { "Ce projet contient des modifications non enregistrées." });
            if let Some(e) = &self.error { ui.colored_label(Color32::from_rgb(180,45,40), e); }
            ui.horizontal(|ui| {
                if ui.button("Enregistrer").clicked() { action = Some(0); }
                if ui.button("Fermer sans enregistrer").clicked() { action = Some(1); }
                if ui.button("Annuler").clicked() { action = Some(2); }
            });
        });
        let Some(action) = action else { return };
        if action == 2 { self.confirmer_fermeture = None; return; }
        if !application { self.changer_document(document); }
        if action == 0 {
            if self.natif_modifie {
                self.enregistrer_projet(&ctx);
                if self.natif_modifie { return; }
            }
            if application {
                for d in self.documents.values_mut().filter(|d| d.modifie) {
                    if let Some((p, chemin)) = &d.natif {
                        if let Err(e) = crate::modele::echanges::enregistrement::enregistrer_natif(p, chemin) {
                            self.error = Some(format!("Enregistrement impossible de {} : {e}", chemin.display()));
                            return;
                        }
                    }
                    if let Some((p, chemin)) = &d.natif { let _ = d.historique.enregistrer(p); crate::modele::echanges::enregistrement::effacer_recuperation(chemin); }
                    d.modifie = false;
                }
            }
        }
        self.confirmer_fermeture = None;
        if action==1 {
            if let Some((_,p))=&self.natif {crate::modele::echanges::enregistrement::effacer_recuperation(p);}
            if application {for d in self.documents.values() {if let Some((_,p))=&d.natif {crate::modele::echanges::enregistrement::effacer_recuperation(p);}}}
        }
        if application {
            self.natif_modifie = false;
            for d in self.documents.values_mut() { d.modifie = false; }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else { self.fermer_document(); }
    }

    pub(super) fn enregistrer_rvt_dialogue(&mut self, ctx: &egui::Context) {
        let Some(source) = &self.rvt_source else { return };
        let sortie = match &self.rvt_sortie {
            Some(p) => p.clone(),
            None => {
                let nom = source.chemin.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Projet".into());
                let dialogue = rfd::FileDialog::new().set_title("Enregistrer le projet Revit en modèle natif v0")
                    .add_filter("Modèle natif v0", &["json"]).set_file_name(format!("{nom}.modele.json"));
                let Some(mut p) = dialogue.save_file() else { return };
                if p.extension().is_none() { p.set_extension("modele.json"); }
                p
            }
        };
        if let Err(e) = self.demarrer_enregistrement_rvt(ctx,sortie) { self.error = Some(e); }
    }

    pub(super) fn demarrer_enregistrement_rvt(&mut self, ctx: &egui::Context, sortie: PathBuf) -> Result<(), String> {
        if self.sauvegarde.is_some() { return Err("Un enregistrement est déjà en cours.".into()); }
        if self.loading.is_some() { return Err("Attendre la fin du chargement avant d’enregistrer.".into()); }
        let source = self.rvt_source.clone().ok_or("Aucun fichier Revit ouvert.")?;
        let (tx, rx) = channel();
        let contexte = ctx.clone();
        std::thread::Builder::new().name("enregistrer-v0".into()).stack_size(64 << 20).spawn(move || {
            let progress = |p| { let _ = tx.send(SaveMsg::Progress(p)); contexte.request_repaint(); };
            let resultat = crate::modele::echanges::enregistrement::enregistrer_rvt(&source,&sortie,&progress);
            let _ = tx.send(SaveMsg::Done(resultat));
            contexte.request_repaint();
        }).map_err(|e| format!("Démarrage de l’enregistrement impossible : {e}"))?;
        self.sauvegarde = Some(Sauvegarde { document: self.document_actif, progression: "Préparation de l’enregistrement v0…".into(), messages: rx });
        self.error = None;
        Ok(())
    }

    pub(super) fn poll_sauvegarde(&mut self) {
        loop {
            let Some(tache) = &mut self.sauvegarde else { return };
            let message = match tache.messages.try_recv() {
                Ok(m) => m,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => SaveMsg::Done(Err("L’enregistrement a été interrompu.".into())),
            };
            match message {
                SaveMsg::Progress(p) => tache.progression = p,
                SaveMsg::Done(resultat) => {
                    let document = self.sauvegarde.take().unwrap().document;
                    match resultat {
                        Ok(bilan) => {
                            let statut = format!("{} — enregistré en v0 : {} murs paramétriques, {} éléments importés",bilan.chemin.display(),bilan.murs,bilan.importes);
                            if document == self.document_actif {
                                self.rvt_sortie = Some(bilan.chemin.clone());
                                self.status = statut;
                            } else if let Some(d) = self.documents.get_mut(&document) {
                                d.rvt_sortie = Some(bilan.chemin.clone()); d.status = statut;
                            } else { self.status = statut; }
                            if let Err(e) = bilan.rapport { self.error = Some(format!("Le projet v0 est enregistré, mais son rapport n’a pas pu être écrit : {e}")); }
                        }
                        Err(e) => self.error = Some(e),
                    }
                    return;
                }
            }
        }
    }

    pub(super) fn open_dialog(&mut self, ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new()
             .add_filter("Projet BIMedit, IFC et Revit", &["bimpartage", "json", "ifc", "IFC", "rvt", "rfa", "rte", "rft"])
            .add_filter("Projet partagé BIMedit", &["bimpartage"])
            .add_filter("Modèle natif", &["json"])
            .add_filter("IFC", &["ifc", "IFC"])
            .add_filter("Revit (murs et métadonnées)", &["rvt", "rfa", "rte", "rft"])
            .pick_file()
        {
            self.open(p, ctx);
        }
    }

    // ------------------------------------------------------------------ UI

    pub(super) fn stocker_document(&mut self) {
        self.historique.geste(false);
        if self.scene.is_none() && self.rvt.is_none() {
            return;
        }
        self.memoriser_vue();
        let status = self.scene.as_ref().map(|s| format!("{} — {} objets 3D, {} triangles", s.file_name, s.objects.len(), s.triangles)).unwrap_or_else(|| self.status.clone());
        self.documents.insert(self.document_actif, DocumentOuvert {
            natif: self.natif.take(),
            historique: std::mem::take(&mut self.historique),
            collaboration: self.collaboration.take(),
            modifie: self.natif_modifie,
            scene: self.scene.take(),
            rvt: self.rvt.take(),
            rvt_source: self.rvt_source.take(),
            rvt_sortie: self.rvt_sortie.take(),
            rvt_geom: std::mem::replace(&mut self.rvt_geom, Err(String::new())),
            rvt_sheet: self.rvt_sheet,
            vues: std::mem::take(&mut self.vues_ouvertes),
            etats: std::mem::take(&mut self.etats_vues),
            active: self.vue_active,
            selected: self.selected,
            filtre: std::mem::take(&mut self.filter),
            onglet: self.left_tab,
            status,
        });
    }

    pub(super) fn nouveau_document(&mut self) {
        self.ajustement=None;
        self.stocker_document();
        self.bim = Default::default();
        self.selection_multiple.clear();
        self.fenetre_selection = None;
        self.natif_modifie = false;
        self.ruban_edition.niveau_mur = None;
        self.ruban_edition.niveau_haut_mur = None;
        self.ruban_edition.reinitialiser_contexte_creation();
        self.document_actif = self.prochain_document;
        self.prochain_document += 1;
    }

    pub(super) fn changer_document(&mut self, id: usize) {
        self.ajustement=None;
        self.orientation_nord_en_cours = false;
        self.arete_alignement = None;
        if id == self.document_actif {
            return;
        }
        let Some(d) = self.documents.remove(&id) else { return };
        self.stocker_document();
        self.document_actif = id;
        self.bim = Default::default();
        self.selection_multiple.clear();
        self.fenetre_selection = None;
        self.natif = d.natif;
        self.avant_saisie = None;
        self.historique = d.historique;
        self.collaboration = d.collaboration;
        self.natif_modifie = d.modifie;
        self.ruban_edition.niveau_mur = None;
        self.ruban_edition.niveau_haut_mur = None;
        self.ruban_edition.reinitialiser_contexte_creation();
        self.scene = d.scene;
        self.rvt = d.rvt;
        self.rvt_source = d.rvt_source;
        self.rvt_sortie = d.rvt_sortie;
        self.rvt_geom = d.rvt_geom;
        self.rvt_sheet = d.rvt_sheet;
        self.rvt_preview = None;
        self.vues_ouvertes = d.vues;
        self.etats_vues = d.etats;
        self.vue_active = d.active;
        self.selected = d.selected;
        self.reveal = None;
        self.filter = d.filtre;
        self.left_tab = d.onglet;
        self.status = d.status;
        self.nomenclature_ouverte = None;
        self.point_clique = None;
        self.passes_feuille.clear();
        self.objets_feuille.clear();
        self.voir_visibilite = false;
        self.show_unsupported = false;
        self.error = None;
        self.liste_niveaux.clear();
        self.liste_materiaux.clear();
        self.categories.clear();
        self.categorie_objet.clear();
        self.cle_objet.clear();
        self.visible.clear();
        if let Some(s) = &self.scene {
            self.liste_niveaux = niveaux(s);
            self.liste_materiaux = s.navigateur.as_ref().map_or_else(|| materiaux_ifc(s), |n| n.materiaux.clone());
            (self.categories, self.categorie_objet, self.cle_objet) = categories(s);
            self.visible = vec![true; s.objects.len()];
            if let Some(r) = self.renderer.as_mut() {
                r.upload_vertices(&s.vertices, &s.couleurs_rendu, &s.motifs_surface);
            }
        }
        self.restaurer_vue(self.vue_active);
        self.visibility_dirty = self.scene.is_some();
        self.coupe_dirty = true;
    }

    pub(super) fn fermer(&mut self) {
        self.orientation_nord_en_cours = false;
        self.arete_alignement = None;
        if self.natif_modifie {
            self.confirmer_fermeture = Some((false, self.document_actif));
            return;
        }
        self.fermer_document();
    }

    pub(super) fn fermer_document(&mut self) {
        self.ajustement=None;
        self.ruban_edition.annuler();
        self.dessin2d=Default::default();
        self.bim=Default::default();
        self.historique=Default::default();
        self.poignee_axe=None;
        if let Some(r)=self.renderer.as_mut() {
            r.upload_vertices(&[],&[],&[]);
            r.upload_coupe(&[],&[]);
            r.upload_passes(&[]);
        }
        self.partage_en_cours=false;
        self.collaboration = None;
        self.natif_modifie = false;
        self.natif = None;
        self.avant_saisie = None;
        self.vues_ouvertes.clear();
        self.etats_vues.clear();
        self.loading = None;
        self.scene = None;
        self.rvt = None;
        self.rvt_source = None;
        self.rvt_sortie = None;
        self.rvt_sheet = false;
        self.rvt_geom = Err(String::new());
        self.rvt_preview = None;
        self.selected = None;
        self.selection_multiple.clear();
        self.fenetre_selection = None;
        self.reveal = None;
        self.vue_active = None;
        self.point_clique = None;
        self.nomenclature_ouverte = None;
        self.passes_feuille.clear();
        self.objets_feuille.clear();
        self.hidden_classes.clear();
        self.hidden_objects.clear();
        self.masques_utilisateur.clear();
        self.hidden_storeys.clear();
        self.isolated = None;
        self.visible.clear();
        self.liste_niveaux.clear();
        self.liste_materiaux.clear();
        self.categories.clear();
        self.categorie_objet.clear();
        self.cle_objet.clear();
        self.filter.clear();
        self.voir_visibilite = false;
        self.show_unsupported = false;
        self.boite = None;
        self.boite_active = false;
        self.coupe = false;
        self.coupe_niveau = None;
        self.coupe_dirty = true;
        self.visibility_dirty = false;
        self.camera = Camera::default();
        self.vol = Vol::default();
        self.error = None;
        self.status = "Ouvrir un fichier IFC ou Revit (Ctrl+O), ou le glisser dans la fenêtre.".into();
        for tx in self.attente_ouverture.drain(..) {
            let _ = tx.send("Erreur : ouverture annulée par la fermeture du fichier.".into());
        }
        if let Some(&id) = self.documents.keys().next_back() {
            self.changer_document(id);
        }
    }

    /// Le dernier onglet représente aussi le document : garder sa vue jusqu'à
    /// la décision d'enregistrement, puis fermer réellement le projet.
    pub(super) fn fermer_onglet_projet(&mut self,id:usize,cible:Option<usize>) {
        let precedent=self.document_actif;
        self.changer_document(id);
        if let Some(k)=self.vues_ouvertes.iter().position(|v|*v==cible) {
            if self.vues_ouvertes.len()==1 {self.fermer();}
            else {
                let vue=self.vues_ouvertes.remove(k);
                self.etats_vues.remove(&vue);
                if self.vue_active==vue {
                    self.nomenclature_ouverte=None;
                    let suivante=self.vues_ouvertes[k.min(self.vues_ouvertes.len()-1)];
                    self.restaurer_vue(suivante);
                }
            }
        } else {self.fermer();}
        if precedent!=id {self.changer_document(precedent);}
    }

}
