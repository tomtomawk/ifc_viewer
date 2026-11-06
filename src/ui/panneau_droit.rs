//! Panneau droit, visibilité de la sélection, copie sur un niveau, cadrage.
use super::*;

impl ViewerApp {
    /// Propriétés de l'élément sélectionné (panneau de gauche, comme dans Revit).
    pub(super) fn right_panel(&mut self, ui: &mut egui::Ui) {
        self.fenetre_materiau(ui.ctx());
        self.fenetre_motifs(ui.ctx());
        self.fenetre_position_plan(ui.ctx());
        self.fenetre_plumes(ui.ctx());
        self.fenetre_famille_cartouche(ui.ctx());
        self.fenetre_famille_etiquette(ui.ctx());
        self.fenetre_types_traits(ui.ctx());
        self.entete_panneau(ui, Panneau::Proprietes, "Propriétés");
        if let Some(jeu) = self.jeu_actif() {
            egui::ScrollArea::vertical().show(ui, |ui| self.proprietes_jeu(ui, &jeu));
            return;
        }
        if self.natif.is_some() && self.bim.origine {
            ui.label(RichText::new("Origine du projet").strong());
            ui.label(RichText::new("Coordonnées partagées et cote NGF du niveau 0 ; les cotes du terrain sont NGF.").small().weak());
            self.editeur_coordonnees(ui);
            return;
        }
        if self.natif.is_some() {
            let ids = self.selection_bim();
            if ids.is_empty() {
                let mut contexte = self.natif.as_ref().unwrap().0.clone();
                if self.ruban_edition.contexte_creation(ui, &mut contexte) {
                    if let Err(e) = self.appliquer_projet(contexte) { self.error = Some(e); }
                }
            } else if let Some(projet) = self.contexte_selection(ui, &ids) {
                if let Err(e) = self.appliquer_projet(projet) { self.error = Some(e); }
            }
            if !ids.is_empty() && self.onglet_ruban == OngletRuban::Modifier {
                self.options_modifier(ui, true);
            }
            // Le type vient immédiatement après Phase / Sous-projet / Variante,
            // au même emplacement que pendant la création d'un mur.
            if self.jeu_actif().is_none() && !self.ruban_edition.mur_actif() {
                if let Some(projet)=self.selecteur_type_selection(ui) {
                    if let Err(e)=self.appliquer_projet(projet) {self.error=Some(e);}
                }
            }
            if self.proprietes_segment_edition(ui) {
                // polyligne en modification : le segment sélectionné et son type de trait
            } else if let Some(projet) = self.proprietes_formes_2d(ui, &ids).or_else(|| self.proprietes_terrains(ui, &ids)) {
                if let Err(e) = self.appliquer_projet(projet) { self.error = Some(e); }
            }
            if ids.len() > 1 {
                match self.proprietes_communes(ui, &ids) {
                    Ok(Some(projet)) => if let Err(e) = self.appliquer_projet(projet) { self.error = Some(e); },
                    Ok(None) => {}
                    Err(e) => self.error = Some(e),
                }
            }
        }
        if self.panneau_bim(ui) { return; }
        if self.natif.is_some() {
            let projet = &self.natif.as_ref().unwrap().0;
            // éditeur de type de mur : un type déjà porté par des murs les met à jour
            let mut copie = if self.ruban_edition.editeur_type_ouvert() { projet.clone() } else { crate::modele::Projet::default() };
            if self.ruban_edition.editeur_type_ouvert() && self.ruban_edition.editeur_type_mur(ui.ctx(), &mut copie) {
                if let Err(e) = self.appliquer_projet(copie) { self.error = Some(e); }
            } else if let Some(nom) = self.ruban_edition.prendre_materiau_a_editer() {
                // double-clic sur un matériau dans l'éditeur de type : fenêtre Matériau
                self.materiau_edite = Some(nom);
            } else if let Some(t) = self.ruban_edition.prendre_type_gabarit()
                && let Some((projet, _)) = &self.natif
            {
                let mut copie = projet.clone();
                let r = self.ruban_edition.definir_type_mur(&mut copie, &t.nom, t.couches, t.categorie).and_then(|_| self.appliquer_projet(copie));
                if let Err(e) = r { self.error = Some(e); }
            }
        }
        // création d'un escalier : comme le mur, ses réglages remplacent les propriétés de la vue
        if self.onglet_ruban == OngletRuban::Modeliser && self.ruban_edition.outil() == Some(crate::ui::ruban::Outil::Escalier) && self.selection_bim().is_empty() {
            self.schema_escalier(ui);
            return;
        }
        if self.onglet_ruban == OngletRuban::Modeliser && self.ruban_edition.proprietes_mur(ui, self.natif.as_ref().map(|(p, _)| p)) {
            return;
        }
        if self.onglet_ruban == OngletRuban::Modeliser && self.ruban_edition.proprietes_surfaces(ui, self.natif.as_ref().map(|(p, _)| p)) {
            return;
        }
        if self.onglet_ruban == OngletRuban::Annoter
            && let Some(choix) = self.ruban_edition.proprietes_vues(ui, self.natif.as_ref().map(|(p, _)| p))
        {
            if let Some((niveau, hauteur)) = choix {
                self.creer_vue(|p| p.ajouter_vue_plan(&niveau, hauteur), true);
            }
            return;
        }
        // sans navigation IFC, un modèle Revit ou natif n'expose pas ses données IFC
        let ifc = self.options.navigation_ifc || self.scene.as_ref().is_some_and(|s| s.navigateur.is_none());
        if let (Some(s), Some(id)) = (&self.scene, self.selected) {
            // en-tête façon Revit : classe (IFC) et type de l'élément
            let ty = s.model.type_of.get(&id).map(|t| s.model.name(*t)).filter(|t| !t.is_empty());
            let classe = pretty_type(s.model.ty(id)).trim_start_matches("Ifc").to_string();
            let entete = match (ifc, ty) {
                (true, Some(t)) => Some(format!("{classe} : {t}")),
                (true, None) => Some(classe),
                (false, t) => t,
            };
            // projet natif : le type est dans le sélecteur en tête du panneau
            if let Some(e) = entete.filter(|_| self.natif.is_none() || ifc) {
                ui.label(RichText::new(e).color(Color32::from_gray(70)));
                ui.separator();
            }
        }
        let element_selectionne = self.selected.is_some() || !self.selection_bim().is_empty();
        if self.natif.is_some() {
            self.proprietes_creation_polyligne(ui);
        }
        if self.natif.is_some() && !element_selectionne {
            self.proprietes_vue_native(ui);
        }
        if !element_selectionne && self.natif.is_none() && self.scene.is_some() && (self.coupe || !self.vue_3d()) {
            self.plage_de_vue(ui);
            ui.separator();
        }
        if self.boite_active && self.vue_3d() {
            self.proprietes_boite(ui);
            ui.separator();
        }
        let (Some(_), Some(id)) = (&self.scene, self.selected) else {
            if self.bim.selection.is_empty() {
                ui.label(RichText::new("Sélectionner un élément dans la vue ou l'arborescence.").weak());
            } else if self.onglet_ruban != OngletRuban::Modifier {
                ui.label(RichText::new(format!("{} éléments sélectionnés", self.bim.selection.len())).weak());
            }
            return;
        };
        if self.natif.is_some() {
            if let Some(projet) = self.proprietes_natif(ui, id) {
                if let Err(e) = self.appliquer_projet(projet) { self.error = Some(e); }
                return;
            }
        }
        // élément natif : tout est dans le panneau ci-dessus, pas d'attributs génériques
        if self.natif.is_some() && self.cle_native(id).is_some() {
            return;
        }
        let Some(s) = &self.scene else { return };
        let model = &s.model;
        let has_obj = s.object_of.contains_key(&id);
        let under = self.objects_under(id);
        if !has_obj && under.is_empty() {
            ui.label(RichText::new("Pas de géométrie affichable.").weak());
        }
        ui.add_space(4.0);
        let mut attrs = model.attributes(id);
        if !ifc {
            attrs.retain(|(k, _)| !matches!(k.as_str(), "Classe" | "Id STEP" | "GlobalId"));
        }
        let groups = model.property_groups(id);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("attrs").num_columns(2).striped(true).show(ui, |ui| {
                for (k, v) in attrs {
                    ui.label(RichText::new(k).strong());
                    ui.add(egui::Label::new(v).wrap());
                    ui.end_row();
                }
            });
            for (gi, g) in groups.into_iter().enumerate() {
                egui::CollapsingHeader::new(RichText::new(&g.name).strong())
                    .id_salt(("pset", id, gi))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new(("pgrid", id, gi)).num_columns(2).striped(true).show(ui, |ui| {
                            for (k, v) in g.props {
                                ui.label(k);
                                ui.add(egui::Label::new(v).wrap());
                                ui.end_row();
                            }
                        });
                    });
            }
        });
    }

    /// Cadrer, Isoler ou Masquer (ruban Modifier) : objets de la sélection courante.
    pub(super) fn visibilite_selection(&mut self, o: crate::ui::ruban::Outil) {
        use crate::ui::ruban::Outil;
        if o == Outil::MasquerVue {
            if !self.masquer_elements_vue() {
                self.status = "Masquer dans la vue : ouvrir une vue du projet (plan ou coupe).".into();
            }
            return;
        }
        // toutes les sélections (scène, multiple, éléments natifs) : objets marqués au rendu
        let mut objets: Vec<u32> = self.objets_selectionnes().iter().enumerate().filter(|(_, m)| **m == 255).map(|(i, _)| i as u32).collect();
        objets.sort_unstable();
        objets.dedup();
        match o {
            Outil::Cadrer if objets.is_empty() => self.fit_all(),
            Outil::Cadrer => self.frame_objects(&objets),
            _ if objets.is_empty() => self.status = "Sélectionner d'abord un élément.".into(),
            Outil::Isoler => self.isolate(objets),
            _ => self.hide(objets),
        }
    }

    pub(super) fn memoriser_copie_niveau(&mut self,ids:Vec<String>) {
        if ids.is_empty() {self.status="Sélectionnez les éléments à copier.".into();return;}
        self.copie_presse=Some((self.document_actif,ids));
        self.status="Sélection copiée : Ctrl+V pour choisir le niveau de destination.".into();
    }
    pub(super) fn ouvrir_coller_niveau(&mut self) {
        if self.copie_presse.as_ref().is_none_or(|(document,_)|*document!=self.document_actif) {
            self.status="Copiez d'abord une sélection dans ce projet.".into();return;
        }
        self.copie_niveau=Some(String::new());
    }
    pub(super) fn fenetre_coller_niveau(&mut self,ctx:&egui::Context) {
        let Some(mut niveau)=self.copie_niveau.take() else{return};
        let Some((document,ids))=self.copie_presse.clone() else{return};
        if document!=self.document_actif {return;}
        let Some((p,_))=&self.natif else{return};
        let niveaux:Vec<_>=p.niveaux.iter().map(|n|(n.id.clone(),n.nom.clone(),n.altitude)).collect();
        let mut ouvert=true;let mut valider=false;
        egui::Window::new("Coller sur un niveau").collapsible(false).resizable(false).open(&mut ouvert).show(ctx,|ui| {
            ui.label(format!("{} élément(s) sélectionné(s) · position en plan conservée",ids.len()));
            egui::ComboBox::from_id_salt("destination_copie_niveau").selected_text(niveaux.iter().find(|n|n.0==niveau).map_or("Choisir un niveau",|n|n.1.as_str())).show_ui(ui,|ui| {
                for (id,nom,z) in &niveaux {ui.selectable_value(&mut niveau,id.clone(),format!("{} · {}",nom,crate::config::options::unite().afficher(*z)));}
            });
            valider=ui.add_enabled(!niveau.is_empty(),egui::Button::new("Coller")).clicked();
        });
        if valider {
            match self.executer_bim(crate::modele::edition::commandes::Commande::CopierSurNiveau{ids,niveau}) {
                Ok(copies)=>self.status=format!("{} élément(s) collé(s) sur le niveau — Ctrl+Z pour annuler.",copies.len()),
                Err(e)=>self.error=Some(e),
            }
        } else if ouvert {self.copie_niveau=Some(niveau);}
    }

    pub(super) fn demarrer_cadrage_vue(&mut self) {
        if self.vue_en_plan() && self.vue_native_id().is_some() {
            self.reinitialiser_forme_cadrage();
            self.ruban_edition.activer_outil(crate::ui::ruban::Outil::Cadrer);
            self.status = "Tracez le contour de cadrage du plan, puis validez.".into();
        } else { self.fit_all(); }
    }

    pub(super) fn retirer_cadrage_vue(&mut self) {
        let Some(id) = self.vue_native_id() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let mut copie = projet.clone();
        if let Some(vue) = copie.vues.iter_mut().find(|v| v.id == id) { vue.contour_cadrage.clear(); }
        if let Err(e) = self.appliquer_projet(copie) { self.error = Some(e); }
        self.visibility_dirty = true;
        self.coupe_dirty = true;
    }

    pub(super) fn masquer_elements_vue(&mut self) -> bool {
        let Some(vue_id) = self.vue_native_id() else { return false };
        let ids = self.selection_bim();
        if ids.is_empty() { self.status = "Sélectionnez les éléments à masquer dans cette vue.".into(); return true; }
        let Some((projet, _)) = &self.natif else { return false };
        let mut copie = projet.clone();
        let Some(vue) = copie.vues.iter_mut().find(|v| v.id == vue_id) else { return false };
        for id in &ids { if !vue.elements_masques.contains(id) { vue.elements_masques.push(id.clone()); } }
        if let Err(e) = self.appliquer_projet(copie) { self.error = Some(e); return true; }
        self.select(None, false);
        self.visibility_dirty = true;
        self.coupe_dirty = true;
        self.memoriser_vue();
        self.status = "Éléments masqués dans cette vue ; Tout afficher les rétablit.".into();
        true
    }

    pub(super) fn retablir_elements_vue(&mut self) {
        let Some(id) = self.vue_native_id() else { return };
        let Some((projet, _)) = &self.natif else { return };
        if projet.vues.iter().find(|v| v.id == id).is_none_or(|v| v.elements_masques.is_empty()) { return; }
        let mut copie = projet.clone();
        copie.vues.iter_mut().find(|v| v.id == id).unwrap().elements_masques.clear();
        if let Err(e) = self.appliquer_projet(copie) { self.error = Some(e); }
        self.coupe_dirty = true;
    }

    pub(super) fn isolate(&mut self, objs: Vec<u32>) {
        if objs.is_empty() {
            return;
        }
        // Les classes masquées par défaut doivent rester visibles si on les isole.
        if let Some(s) = &self.scene {
            for &o in &objs {
                self.hidden_classes.remove(&*s.objects[o as usize].class);
                self.hidden_classes.remove(&*self.cle_objet[o as usize]);
            }
        }
        self.isolated = Some(objs.into_iter().collect());
        self.isolement_utilisateur = true;
        self.visibility_dirty = true;
    }

    /// Isolement propre à la vue ouverte (plan d'étage natif : objets de l'étage), sans
    /// isolement demandé par l'utilisateur.
    pub(super) fn isolement_de_vue(&self) -> Option<HashSet<u32>> {
        let i = self.vue_active?;
        let s = self.scene.as_ref()?;
        let v = s.navigateur.as_ref()?.vues.get(i)?;
        if self.natif.is_none() || v.genre != "Plan d'étage" { return None; }
        let objets: HashSet<u32> = s.objects.iter().enumerate().filter(|(_, o)| o.storey == v.etage || &*o.class == "IFCGEOGRAPHICELEMENT").map(|(i, _)| i as u32).collect();
        (!objets.is_empty()).then_some(objets)
    }

    /// Croix du cadre de visibilité temporaire : réaffiche ce qui a été masqué ou isolé
    /// temporairement ; les masquages enregistrés dans la vue restent.
    pub(super) fn quitter_visibilite_temporaire(&mut self) {
        let masques = std::mem::take(&mut self.masques_utilisateur);
        self.hidden_objects.retain(|o| !masques.contains(o));
        if std::mem::take(&mut self.isolement_utilisateur) {
            self.isolated = self.isolement_de_vue();
        }
        self.visibility_dirty = true;
        self.status = "Visibilité temporaire réinitialisée.".into();
    }

    pub(super) fn hide(&mut self, objs: Vec<u32>) {
        self.masques_utilisateur.extend(objs.iter().copied());
        self.hidden_objects.extend(objs);
        self.visibility_dirty = true;
        self.status = "Masqué temporairement — Afficher (AF) ou Tout afficher pour réafficher.".into();
    }

    /// Mode « Afficher les éléments masqués » : bascule.
    pub(super) fn basculer_afficher_masques(&mut self) {
        self.afficher_masques = !self.afficher_masques;
        self.visibility_dirty = true;
        self.status = if self.afficher_masques {
            "Éléments masqués en magenta : cliquer pour réafficher · Échap ou Afficher pour sortir".into()
        } else {
            "Mode Afficher terminé.".into()
        };
    }

    /// Mode Afficher : réaffiche l'objet masqué cliqué (temporaire ou dans la vue). Vrai si
    /// le clic a été consommé.
    pub(super) fn reafficher(&mut self, objet: u32) -> bool {
        if !self.afficher_masques || !self.objets_masques.contains(&objet) {
            return false;
        }
        let Some(s) = &self.scene else { return false };
        let id = s.objects[objet as usize].id;
        // élément entier : toutes ses parties (mur composé, menuiserie…)
        // même cible qu'un clic de sélection : l'élément, ou son mur composé
        let racine = self.cible_clic(id);
        let mut parties = self.objects_under(racine);
        parties.push(objet);
        for o in &parties {
            self.hidden_objects.remove(o);
            self.masques_utilisateur.remove(o);
        }
        if let (Some(cle), Some(vue_id)) = (self.cle_native(racine), self.vue_native_id()) {
            if let Some((projet, _)) = &self.natif {
                if projet.vues.iter().find(|v| v.id == vue_id).is_some_and(|v| v.elements_masques.contains(&cle)) {
                    let mut copie = projet.clone();
                    if let Some(v) = copie.vues.iter_mut().find(|v| v.id == vue_id) {
                        v.elements_masques.retain(|x| x != &cle);
                    }
                    if let Err(e) = self.appliquer_projet(copie) {
                        self.error = Some(e);
                    }
                    self.coupe_dirty = true;
                }
            }
        }
        self.visibility_dirty = true;
        self.status = "Élément réaffiché — cliquer un autre élément magenta, Échap pour sortir.".into();
        true
    }

}
