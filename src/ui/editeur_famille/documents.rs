//! Documents famille : ouverture, onglets, enregistrement, pilotage.
use super::*;

impl ViewerApp {
    /// Même moteur et même caméra que le projet, avec des tampons et une caméra propres à la famille.
    fn vue_3d_famille(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let Some(ed) = self.famille.as_mut() else { return };
        let solides = ed.famille.solides(&ed.resoudre());
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for (objet, solide) in solides.iter().enumerate().filter(|(_, s)| s.visible && !s.vide) {
            let couleur = solide.materiau_nom.as_ref().and_then(|nom| ed.materiaux_projet.iter().find(|(n, _)| n == nom).map(|(_, c)| *c)).unwrap_or([190, 198, 207]);
            for tri in &solide.tris {
                let points = tri.map(|p| (p / 1000.0).as_vec3());
                let n = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
                let normal = [(n.x*127.0) as i8, (n.y*127.0) as i8, (n.z*127.0) as i8, 0];
                for p in points {
                    min = min.min(p); max = max.max(p);
                    indices.push(vertices.len() as u32);
                    vertices.push(scene::Vertex { pos: p.to_array(), normal, color: [couleur[0], couleur[1], couleur[2], 255], obj: objet as u32 });
                }
            }
        }
        if ed.cadrage_3d_initial && !vertices.is_empty() {
            ed.camera_3d.fit(min, max);
            ed.cadrage_3d_initial = false;
        }
        if response.dragged_by(PointerButton::Middle) {
            let d = response.drag_delta();
            if ui.input(|i| i.modifiers.shift) { ed.camera_3d.orbit(d.x, d.y); }
            else { ed.camera_3d.pan(d.x, d.y, rect.height()); }
        }
        if response.dragged_by(PointerButton::Secondary) {
            let d = response.drag_delta();
            ed.camera_3d.regarder(d.x, d.y);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 { ed.camera_3d.zoom(1.0 / accrochage::facteur_molette(scroll) as f32, None); }
        }
        if response.clicked_by(PointerButton::Primary) && let Some(pos) = response.interact_pointer_pos() {
            let n = egui::vec2((pos.x-rect.left()) / rect.width()*2.0-1.0, 1.0-(pos.y-rect.top()) / rect.height()*2.0);
            let (origine, direction) = ed.camera_3d.ray(n.x, n.y, rect.width()/rect.height().max(1.0));
            let mut meilleur: Option<(f32, usize)> = None;
            for (objet, solide) in solides.iter().enumerate().filter(|(_, s)| s.visible && !s.vide) {
                for tri in &solide.tris {
                    let [a,b,c] = tri.map(|p| (p / 1000.0).as_vec3());
                    let e1=b-a; let e2=c-a; let h=direction.cross(e2); let det=e1.dot(h);
                    if det.abs()<1e-8 { continue; }
                    let f=1.0/det; let q=origine-a; let u=f*q.dot(h);
                    if !(0.0..=1.0).contains(&u) { continue; }
                    let v=f*direction.dot(q.cross(e1));
                    if v<0.0 || u+v>1.0 { continue; }
                    let t=f*e2.dot(q.cross(e1));
                    if t>0.0 && meilleur.is_none_or(|(distance,_)| t<distance) { meilleur=Some((t,objet)); }
                }
            }
            let selection = meilleur.map(|(_, k)| if k < ed.famille.volumes.len() { SelectionFamille::Volume(k) } else { SelectionFamille::Extrusion(k-ed.famille.volumes.len()) });
            ed.selectionner(selection.into_iter().collect());
        }
        let selection = match ed.selection { SelectionFamille::Volume(k) => Some(k), SelectionFamille::Extrusion(k) => Some(ed.famille.volumes.len()+k), _ => None };
        // Les paramètres d'affichage sont ceux du projet ; seuls la caméra et les découpes sont propres à la famille.
        std::mem::swap(&mut self.camera, &mut ed.camera_3d);
        let aspect = rect.width() / rect.height().max(1.0);
        let mut params = self.frame_params(aspect);
        params.selected = (0..solides.len()).map(|k| u8::from(selection == Some(k))).collect(); params.coupe = None; params.boite = None; params.cadrage.clear(); params.contour_vue.clear(); params.passes.clear();
        params.soleil = None; params.table_motifs.clear();
        if let Some(renderer) = self.renderer_famille.as_mut() {
            renderer.upload_vertices(&vertices, &vec![[0;4]; vertices.len()], &vec![[0;4]; vertices.len()]);
            renderer.upload_indices(&indices, &[], &[], &[]);
            renderer.upload_coupe(&[], &[]); renderer.upload_passes(&[]);
            let ppp = ui.ctx().pixels_per_point();
            if let Some(id) = renderer.paint(&params, (rect.width()*ppp).max(1.0) as u32, (rect.height()*ppp).max(1.0) as u32) {
                ui.painter().image(id, rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
        self.cube_orientation(ui, rect);
        if let Some(ed) = self.famille.as_mut() { std::mem::swap(&mut self.camera, &mut ed.camera_3d); }
        if vertices.is_empty() { ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Aucune forme pleine visible", egui::FontId::proportional(14.0), Color32::GRAY); }
    }

    pub(in crate::ui) fn nouvelle_famille(&mut self) {
        self.ouvrir_editeur_composant(None);
        if let Some(ed) = self.famille.as_mut() {
            ed.document_famille.autonome = true;
            ed.nom = "Nouvelle famille".into();
            ed.famille.volumes.clear();
            ed.famille.extrusions.clear();
            ed.famille.symboles.clear();
            ed.famille.cotes_mesure.clear();
            ed.marquer_enregistree();
            ed.historique_neuf();
        }
        self.onglet_ruban = OngletRuban::FamilleCreer;
    }

    /// Point d'entrée commun aux catégories ; les moteurs spécialisés restent des adaptateurs.
    pub(in crate::ui) fn categorie_famille_ui(&mut self, ui: &mut egui::Ui) {
        use super::super::familles_planes::FamillePlane;
        let plane = self.famille_plane_active;
        let (actuelle, modifiable) = if let Some(ed) = plane.and_then(|i| self.familles_planes.get(&i)) {
            if ed.element_selectionne() { return; }
            let c = match ed.contenu { FamillePlane::Profil {..} => 3, FamillePlane::Cartouche {..} => 4, FamillePlane::Etiquette {..} => 5 };
            (c, !ed.contenu_modifie() && ed.contenu.id().is_none())
        } else if let Some(ed) = self.famille.as_ref().filter(|_| self.famille_active) {
            if ed.selection != SelectionFamille::Aucune || !ed.multiple.is_empty() { return; }
            (match ed.famille.genre { GenreOuverture::Porte => 1, GenreOuverture::Fenetre => 2, _ => 0 }, !ed.contenu_modifie() && ed.type_id.is_none())
        } else { return; };
        let categories = ["Composant", "Porte", "Fenêtre", "Profil", "Cartouche", "Étiquette de pièce"];
        let mut choix = actuelle;
        ui.label("Catégorie de famille");
        ui.add_enabled_ui(modifiable, |ui| {
            egui::ComboBox::from_id_salt("categorie_famille_commune").selected_text(categories[actuelle]).show_ui(ui, |ui| {
                for (k, nom) in categories.iter().enumerate() { ui.selectable_value(&mut choix, k, *nom); }
            });
        }).response.on_hover_text("Choisir la catégorie avant de dessiner. Une famille modifiée ou déjà chargée conserve sa catégorie pour protéger ses données et ses occurrences.");
        if choix == actuelle { return; }
        let autonome = if let Some(ordre) = plane { self.familles_planes.get(&ordre).is_some_and(|ed| ed.document_famille.autonome) } else { self.famille.as_ref().is_some_and(|ed| ed.document_famille.autonome) };
        if plane.is_some() { self.fermer_famille_plane(); } else { self.fermer_famille(); }
        match choix {
            0 => self.nouvelle_famille(),
            1 | 2 => {
                self.ouvrir_editeur_famille_type(None, if choix == 1 { GenreOuverture::Porte } else { GenreOuverture::Fenetre });
                if let Some(ed) = self.famille.as_mut() {
                    ed.famille.volumes.clear(); ed.famille.extrusions.clear(); ed.famille.symboles.clear();
                    ed.marquer_enregistree(); ed.historique_neuf();
                }
            }
            3 => self.ouvrir_profil(None),
            4 => self.editer_cartouche(None, None),
            _ => self.editer_famille_etiquette(None, None),
        }
        if autonome {
            if let Some(ed) = self.famille_plane_active.and_then(|i| self.familles_planes.get_mut(&i)) { ed.document_famille.autonome = true; }
            else if let Some(ed) = self.famille.as_mut().filter(|_| self.famille_active) { ed.document_famille.autonome = true; }
        }
    }
    /// Contrat sans dialogue de l'éditeur ; même validation et chargement que le ruban.
    pub(in crate::ui) fn piloter_famille(&mut self, commande: &str, argument: &str) -> Option<Result<serde_json::Value, String>> {
        if let Some(ordre) = self.famille_plane_active {
            match commande {
                "famille_navigation" => return Some(self.navigation_cartouche(argument)),
                "famille_charger_projet" => return Some(self.enregistrer_famille_plane(argument == "nouveau").map(|_| {
                    serde_json::json!({"id": self.familles_planes[&ordre].contenu.id()})
                })),
                "famille_fermer" => {
                    if self.familles_planes[&ordre].modifie() && argument != "abandonner" {
                        return Some(Err("Famille modifiée : chargez-la dans le projet avant de fermer.".into()));
                    }
                    self.fermer_famille_plane();
                    return Some(Ok(serde_json::json!({})));
                }
                "famille_modifier" | "famille_document" => return Some(Err("Cette commande est réservée à l'éditeur des familles modélisées.".into())),
                _ => {}
            }
        }
        if commande=="famille_navigation" {
            return Some((|| {
                let ed=self.famille.as_mut().ok_or("Éditeur fermé")?;
                match argument {
                    "plan"=>ed.ouvrir_vue(VueFamille::Plan),"coupe"=>ed.ouvrir_vue(VueFamille::Coupe),"elevation"=>ed.ouvrir_vue(VueFamille::Elevation),
                    "cadrer"=>ed.navigation[ed.vue as usize]=Default::default(),
                    ""|"etat"=>{},_=>return Err("famille_navigation etat|plan|coupe|elevation|cadrer".into())
                }
                let n=ed.navigation[ed.vue as usize];
                Ok(serde_json::json!({"vue":ed.vue.nom(),"zoom":n.zoom,"decalage":[n.decalage.0,n.decalage.1],"rectangle":[ed.rect_navigation.min.x,ed.rect_navigation.min.y,ed.rect_navigation.max.x,ed.rect_navigation.max.y]}))
            })());
        }
        if !["famille_nouvelle","famille_document","famille_modifier","famille_charger_projet","famille_fermer"].contains(&commande) { return None; }
        Some((|| {
            self.pilote_disponible()?;
            match commande {
                "famille_nouvelle" => {
                    match argument {
                        "porte"=>self.ouvrir_editeur_famille_type(None,GenreOuverture::Porte),
                        "fenetre"=>self.ouvrir_editeur_famille_type(None,GenreOuverture::Fenetre),
                        "composant"=>self.ouvrir_editeur_composant(None),
                        a if a.starts_with("composant ")=>self.ouvrir_editeur_composant(Some(a[10..].trim().into())),
                        _=>return Err("genre : porte|fenetre|composant [type]".into())
                    }
                }
                "famille_modifier" => {
                    let f=crate::modele::famille::modelee::FichierFamille::lire(argument)?;
                    if f.nom.trim().is_empty() {return Err("Nom de famille vide".into());}
                    let ed=self.famille.as_mut().ok_or("Éditeur fermé")?;
                    f.famille.valider(f.largeur,f.hauteur,ed.epaisseur)?;
                    ed.nom=f.nom;ed.largeur=f.largeur;ed.hauteur=f.hauteur;ed.famille=f.famille;
                    // indices de sélection et de saisie : plus valables sur la nouvelle famille
                    ed.abandonner_saisie();
                    ed.selectionner(Vec::new());
                }
                "famille_charger_projet" => {
                    if !argument.is_empty() && argument!="nouveau" {return Err("usage : famille_charger_projet [nouveau]".into());}
                    let id=self.enregistrer_famille(argument=="nouveau")?;
                    return Ok(serde_json::json!({"id":id}));
                }
                "famille_fermer" => {
                    if !argument.is_empty() && argument!="abandonner" {return Err("usage : famille_fermer [abandonner]".into());}
                    if self.famille.as_ref().is_some_and(|e|e.modifiee()) && argument!="abandonner" {return Err("Famille modifiée : charger dans le projet ou famille_fermer abandonner".into());}
                    self.fermer_famille();
                    return Ok(serde_json::json!({}));
                }
                _=>{}
            }
            let ed=self.famille.as_ref().ok_or("Éditeur fermé")?;
            Ok(serde_json::json!({"fichier":ed.fichier(),"modifie":ed.modifiee(),"type":ed.type_id}))
        })())
    }
    /// Document au premier plan, tel que le pilote le voit : une famille (`famille:<rang>`)
    /// ou un projet (`<id>`).
    pub(in crate::ui) fn document_actif_pilote(&self) -> serde_json::Value {
        if let Some(ordre) = self.famille_plane_active
            && let Some(ed) = self.familles_planes.get(&ordre) {
            return serde_json::json!({"id": format!("famille:{ordre}"), "genre": "famille", "nom": ed.titre(), "type": ed.contenu.id(), "modifie": ed.modifie()});
        }
        match self.famille.as_ref().filter(|_| self.famille_active) {
            Some(ed) => serde_json::json!({"id": format!("famille:{}", ed.document_famille.ordre), "genre": "famille", "nom": titre_editeur(ed), "type": ed.type_id, "modifie": ed.modifiee()}),
            None => serde_json::json!({"id": self.document_actif.to_string(), "genre": "projet", "nom": self.scene.as_ref().map(|s| s.file_name.clone()), "modifie": self.natif_modifie}),
        }
    }

    /// Inscrit au registre tout changement de document actif.
    pub(in crate::ui) fn suivre_document_actif(&mut self) {
        let actuel = self.document_actif_pilote();
        let precedent = self.journal_documents.last().map(|e| e["document"]["id"].clone());
        if precedent.as_ref() != Some(&actuel["id"]) {
            let rang = self.journal_documents.last().and_then(|e| e["rang"].as_u64()).map_or(1, |r| r + 1);
            self.journal_documents.push(serde_json::json!({"rang": rang, "document": actuel, "precedent": precedent}));
            if self.journal_documents.len() > 100 {
                self.journal_documents.remove(0);
            }
        }
    }

    /// Ajoute le document actif à une réponse JSON du pilote.
    pub(in crate::ui) fn avec_document_actif(&self, reponse: String) -> String {
        match serde_json::from_str::<serde_json::Value>(&reponse) {
            Ok(serde_json::Value::Object(mut o)) if !o.contains_key("document_actif") => {
                o.insert("document_actif".into(), self.document_actif_pilote());
                serde_json::Value::Object(o).to_string()
            }
            _ => reponse,
        }
    }

    /// Documents ouverts pour le pilote : projets puis familles, et le registre.
    pub(in crate::ui) fn documents_pilote(&self) -> serde_json::Value {
        let famille_active = self.mode_famille();
        let mut d: Vec<serde_json::Value> = self.documents.iter().map(|(id, d)| serde_json::json!({"id": id.to_string(), "genre": "projet", "actif": false, "nom": d.scene.as_ref().map(|s| &s.file_name), "modifie": d.modifie})).collect();
        if self.scene.is_some() || self.rvt.is_some() {
            d.push(serde_json::json!({"id": self.document_actif.to_string(), "genre": "projet", "actif": !famille_active, "nom": self.scene.as_ref().map(|s| &s.file_name), "modifie": self.natif_modifie}));
        }
        let mut familles: Vec<&EditeurFamille> = self.famille.iter().chain(&self.familles_inactives).collect();
        familles.sort_by_key(|e| e.document_famille.ordre);
        for e in familles {
            let actif = self.famille_active && self.famille.as_ref().is_some_and(|f| f.document_famille.ordre == e.document_famille.ordre);
            d.push(serde_json::json!({"id": format!("famille:{}", e.document_famille.ordre), "genre": "famille", "actif": actif, "nom": titre_editeur(e), "type": e.type_id, "modifie": e.modifiee()}));
        }
        for (&ordre, ed) in &self.familles_planes {
            d.push(serde_json::json!({"id": format!("famille:{ordre}"), "genre": "famille", "actif": self.famille_plane_active == Some(ordre), "nom": ed.titre(), "type": ed.contenu.id(), "modifie": ed.modifie()}));
        }
        serde_json::json!({"documents": d, "actif": self.document_actif_pilote(), "registre": self.journal_documents})
    }

    /// Active un document par son identifiant de pilote (`famille:<rang>` ou `<id>`).
    pub(in crate::ui) fn activer_document_pilote(&mut self, id: &str) -> Result<(), String> {
        if let Some(rang) = id.strip_prefix("famille:") {
            let rang: usize = rang.parse().map_err(|_| "Rang de famille invalide")?;
            if !self.onglets_familles().iter().any(|o| o.0 == rang) {
                return Err("Famille absente".into());
            }
            self.activer_famille(rang);
            return Ok(());
        }
        let id: usize = id.parse().map_err(|_| "Identifiant de document requis : <id> ou famille:<rang>")?;
        if id != self.document_actif && !self.documents.contains_key(&id) {
            return Err("Document absent".into());
        }
        self.famille_active = false;
        self.famille_plane_active = None;
        self.changer_document(id);
        Ok(())
    }

    /// Éditeur sur la famille du type de la fenêtre `id` (gabarit si le type n'a pas de
    /// famille modélisée).
    pub(in crate::ui) fn ouvrir_editeur_famille(&mut self, id: &str) {
        let Some((p, _)) = &self.natif else { return };
        let Some(o) = p.ouvertures.iter().find(|o| o.id == id) else { return };
        let e = crate::modele::volumes::solides::hote(p, &o.mur).map_or(200.0, |h| h.1);
        let t = o.type_ouverture.clone();
        let (l, h, id, g) = (o.largeur, o.hauteur, o.id.clone(), o.genre);
        self.editeur_famille(t, Some(id), l, h, e, g);
    }

    /// Éditeur sur le type `t` (None : nouvelle famille de la catégorie `genre`), hors
    /// de toute ouverture placée.
    pub(in crate::ui) fn ouvrir_editeur_famille_type(&mut self, t: Option<String>, genre: GenreOuverture) {
        let (l, h) = if genre == GenreOuverture::Porte { (900.0, 2150.0) } else { (1200.0, 1350.0) };
        self.editeur_famille(t, None, l, h, 300.0, genre);
    }

    pub(in crate::ui) fn editeur_famille(&mut self, t: Option<String>, ouverture: Option<String>, largeur: f64, hauteur: f64, epaisseur: f64, genre: GenreOuverture) {
        // type déjà ouvert : son onglet passe au premier plan
        if let Some(ordre) = t.as_ref().and_then(|id| self.famille.iter().chain(&self.familles_inactives).find(|e| e.type_id.as_ref() == Some(id)).map(|e| e.document_famille.ordre)) {
            self.activer_famille(ordre);
            return;
        }
        // l'éditeur courant passe à l'arrière-plan
        if let Some(ed) = self.famille.take() {
            self.familles_inactives.push(ed);
        }
        self.prochain_ordre_famille += 1;
        let ordre = self.prochain_ordre_famille;
        let projet = self.natif.as_ref().map(|(p, _)| p);
        let t = t.and_then(|id| projet.and_then(|p| p.types_ouvertures.iter().find(|x| x.id == id)));
        let u = crate::config::options::unite();
        let (largeur, hauteur) = t.map_or((largeur, hauteur), |t| (t.largeur, t.hauteur));
        let genre = t.map_or(genre, |t| t.genre);
        let famille = t.and_then(|t| t.modele.clone()).unwrap_or_else(|| FamilleModelee::gabarit(genre));
        let prefixe = if famille.genre == GenreOuverture::Porte { "Porte" } else { "Fenêtre" };
        let partages = projet.map(|p| p.parametres_partages.clone()).unwrap_or_default();
        let materiaux_projet = projet.map(|p| p.materiaux.iter().map(|m| (m.nom.clone(), m.couleur)).collect()).unwrap_or_default();
        self.famille_active = true;
        self.famille_plane_active = None;
        self.famille = Some(EditeurFamille {
            document_famille: crate::ui::document_famille::DocumentFamille::nouveau(ordre, self.document_actif, self.natif.is_none(), None, FichierFamille { format: String::new(), nom: String::new(), largeur: 0.0, hauteur: 0.0, famille: FamilleModelee::gabarit_fenetre() }),
            ouverture,
            type_id: t.map(|t| t.id.clone()),
            nom: t.map_or_else(|| format!("{prefixe} {} × {}", u.afficher(largeur), u.afficher(hauteur)), |t| t.nom.clone()),
            largeur,
            hauteur,
            epaisseur,
            famille,
            esquisse: Vec::new(),
            axe_revolution: None,
            partages_origine: partages.clone(),
            partages,
            materiaux_projet,
            poignee: None,
            reference: (String::new(), 0.0, 0.0, FamilleModelee::gabarit_fenetre()),
            vue: VueFamille::Elevation,
            construction: None,
            vue_3d: false,
            camera_3d: Default::default(),
            cadrage_3d_initial: true,
            navigation: Default::default(),
            rect_navigation: egui::Rect::NOTHING,
            outil: OutilFamille::Selection,
            selection: SelectionFamille::Aucune,
            clics: Vec::new(),
            debut: None,
            deplace: None,
            message: None,
            erreur: None,
            cote_deplacee: None,
            multiple: Vec::new(),
            fenetre: None,
        });
        if let Some(ed) = self.famille.as_mut() {
            ed.marquer_enregistree();
            ed.historique_neuf();
        }
    }

    /// Éditeur sur un type de composant (None : nouvelle famille, sur la base de la table).
    /// Une famille de composant a la catégorie « libre » ; l'épaisseur y est la profondeur.
    pub(in crate::ui) fn ouvrir_editeur_composant(&mut self, t: Option<String>) {
        if let Some(ordre) = t.as_ref().and_then(|id| self.famille.iter().chain(&self.familles_inactives).find(|e| e.type_id.as_ref() == Some(id)).map(|e| e.document_famille.ordre)) {
            self.activer_famille(ordre);
            return;
        }
        let defaut = crate::modele::elements::composant::type_table();
        let (dans_projet, tc) = match t.as_ref().and_then(|id| self.natif.as_ref().and_then(|(p, _)| p.types_composants.iter().find(|x| &x.id == id))) {
            Some(x) => (true, x.clone()),
            // le type par défaut n'entre dans le projet qu'à son premier placement ou enregistrement
            None if t.as_deref() == Some(defaut.id.as_str()) => (false, defaut),
            None => (false, crate::modele::TypeComposant { id: String::new(), nom: "Nouveau composant".into(), ..defaut }),
        };
        self.editeur_famille(None, None, tc.largeur, tc.hauteur, tc.profondeur, GenreOuverture::Fenetre);
        if let Some(ed) = self.famille.as_mut() {
            ed.type_id = (!tc.id.is_empty()).then(|| tc.id.clone());
            ed.nom = tc.nom;
            ed.famille = tc.modele;
            ed.famille.genre = GenreOuverture::Libre;
            if dans_projet { ed.marquer_enregistree(); } else { ed.reference.0 = String::new(); }
            ed.historique_neuf();
        }
    }

    /// Enregistre la famille dans son type (ou un nouveau type) ; les fenêtres du type
    /// prennent ses dimensions ; la fenêtre d'origine prend le type.
    pub(in crate::ui) fn enregistrer_famille(&mut self, nouveau: bool) -> Result<String, String> {
        let ed = self.famille.as_ref().ok_or("Éditeur fermé.")?;
        if ed.nom.trim().is_empty() {
            return Err("Nommer le type.".into());
        }
        if self.natif.is_none() { return Err("Ouvrir un projet pour y charger la famille.".into()); }
        if !ed.document_famille.autonome && ed.document_famille.document != self.document_actif {
            return Err("Cette famille a été ouverte depuis un autre projet : revenir à ce projet pour la charger.".into());
        }
        ed.famille.valider(ed.largeur, ed.hauteur, ed.epaisseur)?;
        if ed.famille.genre == GenreOuverture::Libre {
            // famille de composant : son type (créé ou mis à jour), donc tous ses composants
            let id = if nouveau { String::new() } else { ed.type_id.clone().unwrap_or_default() };
            let t = crate::modele::TypeComposant { id, nom: ed.nom.trim().into(), largeur: ed.largeur, profondeur: ed.epaisseur, hauteur: ed.hauteur, modele: ed.famille.clone() };
            let id = self.executer_bim(crate::modele::edition::commandes::Commande::TypeComposant { type_composant: t })?.into_iter().next().unwrap_or_default();
            if let Some(ed) = self.famille.as_mut() {
                ed.type_id = Some(id.clone());
                ed.document_famille.document = self.document_actif; ed.document_famille.autonome = false;
                ed.marquer_enregistree();
            }
            self.bim.composant.type_composant = id.clone();
            self.status = "Famille de composant chargée dans le projet.".into();
            return Ok(id);
        }
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let cible=ed.type_id.as_deref().filter(|_|!nouveau);
        let fichier=crate::modele::famille::modelee::FichierFamille{format:crate::modele::famille::modelee::FORMAT_FAMILLE.into(),nom:ed.nom.clone(),largeur:ed.largeur,hauteur:ed.hauteur,famille:ed.famille.clone()};
        let modifies: BTreeMap<String, f64> = ed.partages.iter().filter(|(k, v)| ed.partages_origine.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), *v)).collect();
        let conflits = ed.famille.resoudre(ed.largeur, ed.hauteur, ed.epaisseur).conflits.len();
        let (q,id)=p.avec_famille(cible,ed.ouverture.as_deref(),&fichier,ed.epaisseur,modifies)?;
        self.appliquer_projet(q)?;
        let partages = self.natif.as_ref().map(|(p, _)| p.parametres_partages.clone()).unwrap_or_default();
        if let Some(ed) = self.famille.as_mut() {
            ed.type_id = Some(id.clone());
            ed.document_famille.document = self.document_actif; ed.document_famille.autonome = false;
            ed.partages = partages.clone();
            ed.partages_origine = partages;
            ed.marquer_enregistree();
        }
        self.status = if conflits > 0 {
            format!("Famille chargée dans le projet, {conflits} contrainte(s) écartée(s) par conflit.")
        } else {
            "Famille chargée dans le projet — Ctrl+S (hors de la famille) pour enregistrer le projet.".into()
        };
        Ok(id)
    }

    /// Fenêtre de l'éditeur de familles.
    /// Titre de l'onglet de l'éditeur au premier plan : catégorie et type.
    pub(in crate::ui) fn titre_famille(&self) -> Option<String> {
        if let Some(ed) = self.famille_plane_active.and_then(|ordre| self.familles_planes.get(&ordre)) {
            return Some(ed.titre());
        }
        self.famille.as_ref().map(titre_editeur)
    }

    /// Ferme l'onglet de l'éditeur de familles (la famille non enregistrée est perdue).
    pub(in crate::ui) fn fermer_famille(&mut self) {
        // la famille ouverte juste avant prend sa place, s'il y en a une
        self.famille = self.familles_inactives.pop();
        self.famille_active = self.famille.is_some();
    }

    /// Onglets de familles : (rang, titre, type), dans l'ordre d'ouverture.
    pub(in crate::ui) fn onglets_familles(&self) -> Vec<(usize, String, Option<String>)> {
        let mut v: Vec<(usize, String, Option<String>)> = self.famille.iter().chain(&self.familles_inactives).map(|e| (e.document_famille.ordre, titre_editeur(e), e.type_id.clone())).collect();
        v.extend(self.familles_planes.iter().map(|(&ordre, ed)| (ordre, ed.titre(), ed.contenu.id())));
        v.sort_by_key(|o| o.0);
        v
    }

    /// Rang de la famille au premier plan.
    pub(in crate::ui) fn ordre_famille(&self) -> Option<usize> {
        self.famille_plane_active.or_else(|| self.famille.as_ref().map(|e| e.document_famille.ordre))
    }

    /// Met la famille de rang `ordre` au premier plan (onglet actif).
    pub(in crate::ui) fn activer_famille(&mut self, ordre: usize) {
        if let Some((document, autonome)) = self.familles_planes.get(&ordre).map(|ed| (ed.document_famille.document, ed.document_famille.autonome)) {
            if !autonome && document != self.document_actif && self.documents.contains_key(&document) { self.changer_document(document); }
            self.famille_active = false;
            self.famille_plane_active = Some(ordre);
            return;
        }
        self.famille_plane_active = None;
        // une famille modélisée revient avec le projet d'où elle a été ouverte
        if let Some(document) = self.famille.iter().chain(&self.familles_inactives).find(|e| e.document_famille.ordre == ordre && !e.document_famille.autonome).map(|e| e.document_famille.document)
            && document != self.document_actif && self.documents.contains_key(&document) {
            self.changer_document(document);
        }
        if self.famille.as_ref().map(|e| e.document_famille.ordre) != Some(ordre) {
            if let Some(k) = self.familles_inactives.iter().position(|e| e.document_famille.ordre == ordre) {
                let ed = self.familles_inactives.remove(k);
                if let Some(courante) = self.famille.replace(ed) {
                    self.familles_inactives.push(courante);
                }
            }
        }
        self.famille_active = self.famille.is_some();
    }

    /// Fermeture de l'onglet de rang `ordre` (confirmation si besoin).
    pub(in crate::ui) fn fermer_onglet_famille(&mut self, ordre: usize) {
        self.activer_famille(ordre);
        self.demander_fermeture_famille();
    }

    /// Vue centrale de l'onglet « Famille » : barre d'outils et vue de travail.
    pub(in crate::ui) fn cadrer_famille(&mut self) {
        if let Some(ed)=self.famille.as_mut(){ if ed.vue_3d { ed.cadrage_3d_initial = true; } else { ed.navigation[ed.vue as usize]=Default::default(); } }
    }

    pub(in crate::ui) fn vue_famille(&mut self, ui: &mut egui::Ui) {
        if self.famille.as_ref().is_some_and(|ed| ed.vue_3d) { self.vue_3d_famille(ui); return; }
        let Some(ed) = self.famille.as_mut() else { return };
        egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(6.0).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            // les vues s'ouvrent depuis l'arborescence, comme dans le projet
            ui.horizontal(|ui| {
                ui.label(RichText::new(if ed.vue_3d { "Vue 3D" } else { ed.vue.nom() }).strong());
                if ui.button("Cadrer").clicked(){ed.navigation[ed.vue as usize]=Default::default();}
                ui.label(RichText::new(if ed.vue_3d { "Glisser : faire tourner · dessin dans les vues planes" } else { "Molette : zoom · bouton central : déplacer · Échap : sélection" }).small().weak());
            });
            canevas(ui, ed);
            let rect = ed.rect_navigation;
            let mut bas = 0.0;
            if ed.outil != OutilFamille::Selection && rect.is_positive() {
                let mut valider = false;
                let mut annuler = false;
                let zone = egui::Area::new(egui::Id::unique("barre_commande_famille"))
                    .fixed_pos(rect.left_top() + egui::vec2(8.0, 8.0)).constrain_to(rect)
                    .show(ui.ctx(), |ui| egui::Frame::popup(ui.style()).show(ui, |ui| ui.horizontal(|ui| {
                        if ed.outil.est_forme() { valider = ui.button("✓ Valider").clicked(); }
                        annuler = ui.button("✕ Annuler").clicked();
                    })));
                bas = zone.response.rect.bottom() - rect.top();
                if valider {
                    let construction = ed.construction.and_then(|k| ed.famille.plans_construction.get(k)).cloned();
                    ed.erreur = ed.terminer_forme(construction).err();
                }
                if annuler { ed.choisir_outil(OutilFamille::Selection); ed.message = None; }
            }
            if let Some(m) = &ed.message { if rect.is_positive() { crate::ui::aides::annotation_vue(ui, rect, bas, m, false); } }
        });
    }

    /// Panneau Propriétés dans l'onglet « Famille » : type, paramètres, contraintes,
    /// plans, volumes, extrusions, symboles ; état des contraintes ; chargement dans le
    /// projet et fichier.
    pub(in crate::ui) fn proprietes_famille(&mut self, ui: &mut egui::Ui) {
        self.entete_panneau(ui, Panneau::Proprietes, "Propriétés de la famille");
        let Some(ed) = self.famille.as_mut() else { return };
        let bas = 40.0;
        egui::ScrollArea::vertical().id_salt("famille_panneau").max_height((ui.available_height() - bas).max(160.0)).show(ui, |ui| panneau(ui, ed));
        ui.separator();
        let r = ed.resoudre();
        match (&ed.erreur, r.erreurs.first(), r.conflits.len()) {
            (Some(e), _, _) | (None, Some(e), _) => {
                ui.colored_label(Color32::from_rgb(200, 40, 40), e);
            }
            (None, None, n) if n > 0 => {
                ui.colored_label(Color32::from_rgb(200, 120, 20), format!("{n} contrainte(s) écartée(s) : priorité inférieure en conflit."));
            }
            _ => {
                ui.label(RichText::new("Contraintes satisfaites.").weak());
            }
        }
    }

    /// Arborescence dans l'onglet « Famille » : vues de la famille (clic : ouvrir),
    /// paramètres, plans, volumes, extrusions et symboles (clic : sélectionner), puis
    /// les éléments de la famille.
    pub(in crate::ui) fn arborescence_famille(&mut self, ui: &mut egui::Ui) {
        let titre = self.titre_famille().unwrap_or_default();
        self.entete_panneau(ui, Panneau::Arborescence, &format!("Arborescence - {titre}"));
        let Some(ed) = self.famille.as_mut() else { return };
        egui::ScrollArea::vertical().id_salt("famille_arborescence").auto_shrink([false, false]).show(ui, |ui| {
            if let Some(vue) = choisir_vue_famille(ui, (!ed.vue_3d).then_some(ed.vue), VueFamille::nom) { ed.ouvrir_vue(vue); }
            if ui.selectable_label(ed.vue_3d, "Vue 3D").clicked() {
                ed.vue_3d = true;
                ed.construction = None;
                ed.choisir_outil(OutilFamille::Selection);
            }
            egui::CollapsingHeader::new(RichText::new(format!("Paramètres ({})", ed.famille.parametres.len() + 3)).strong()).default_open(false).show(ui, |ui| {
                for n in [LARGEUR, HAUTEUR, EPAISSEUR] {
                    ui.label(RichText::new(format!("{n} (type / hôte)")).weak());
                }
                for p in &ed.famille.parametres {
                    ui.label(format!("{} · {} · {}", p.nom, p.genre.nom(), p.portee.nom()));
                }
            });
            egui::CollapsingHeader::new(RichText::new(format!("Plans de référence ({})", ed.famille.plans.len())).strong()).default_open(false).show(ui, |ui| {
                for p in &ed.famille.plans {
                    if ui.selectable_label(ed.choisi(&SelectionFamille::Plan(p.nom.clone())), format!("{} · {}", p.nom, p.axe.nom())).clicked() {
                        ed.selection = SelectionFamille::Plan(p.nom.clone());
                        ed.multiple.clear();
                    }
                }
            });
            egui::CollapsingHeader::new(RichText::new(format!("Volumes ({})", ed.famille.volumes.len())).strong()).default_open(true).show(ui, |ui| {
                for (k, v) in ed.famille.volumes.iter().enumerate() {
                    let texte = if v.vide { format!("{} (vide)", v.nom) } else { v.nom.clone() };
                    if ui.selectable_label(ed.choisi(&SelectionFamille::Volume(k)), texte).clicked() {
                        ed.selection = SelectionFamille::Volume(k);
                        ed.multiple.clear();
                    }
                }
            });
            egui::CollapsingHeader::new(RichText::new(format!("Formes à profil ({})", ed.famille.extrusions.len())).strong()).default_open(true).show(ui, |ui| {
                for (k, e) in ed.famille.extrusions.iter().enumerate() {
                    let texte = if e.vide { format!("{} (vide)", e.nom) } else { e.nom.clone() };
                    if ui.selectable_label(ed.choisi(&SelectionFamille::Extrusion(k)), texte).clicked() {
                        ed.selection = SelectionFamille::Extrusion(k);
                        ed.multiple.clear();
                        ed.esquisse.clear();
                        ed.axe_revolution = None;
                        ed.outil = OutilFamille::Selection;
                        ed.construction = e.construction.as_ref().and_then(|plan| ed.famille.plans_construction.iter().position(|p| &p.id == plan));
                        // la vue de son esquisse, pour ses poignées
                        ed.vue = match e.vue {
                            VueEsquisse::Elevation => VueFamille::Elevation,
                            VueEsquisse::Plan => VueFamille::Plan,
                            VueEsquisse::Coupe => VueFamille::Coupe,
                        };
                    }
                }
            });
            egui::CollapsingHeader::new(RichText::new(format!("Symboles 2D ({})", ed.famille.symboles.len())).strong()).default_open(false).show(ui, |ui| {
                for (k, sy) in ed.famille.symboles.iter().enumerate() {
                    let texte = format!("{} {} · {}", if sy.arc { "Arc" } else { "Ligne" }, k + 1, match sy.vue { VueEsquisse::Plan => "plan", VueEsquisse::Elevation => "élévation", VueEsquisse::Coupe => "coupe" });
                    if ui.selectable_label(ed.choisi(&SelectionFamille::Symbole(k)), texte).clicked() {
                        ed.selection = SelectionFamille::Symbole(k);
                        ed.multiple.clear();
                    }
                }
            });
        });
    }

    /// Portes et fenêtres du projet : (type, nom, catégorie), par catégorie puis nom.
    pub(in crate::ui) fn familles_projet(&self) -> Vec<(String, String, GenreOuverture)> {
        let Some((p, _)) = &self.natif else { return Vec::new() };
        let mut v: Vec<(String, String, GenreOuverture)> = p.types_ouvertures.iter().filter(|t| t.genre != GenreOuverture::Libre).map(|t| (t.id.clone(), t.nom.clone(), t.genre)).collect();
        v.extend(p.types_composants.iter().map(|t| (t.id.clone(), t.nom.clone(), GenreOuverture::Libre)));
        let rang = |g: GenreOuverture| match g { GenreOuverture::Porte => 0, GenreOuverture::Fenetre => 1, GenreOuverture::Libre => 2 };
        v.sort_by(|a, b| (rang(a.2), &a.1).cmp(&(rang(b.2), &b.1)));
        v
    }

    /// Enregistre la famille de l'éditeur dans un fichier.
    pub(in crate::ui) fn enregistrer_famille_fichier(&mut self, chemin: &std::path::Path) -> Result<(), String> {
        let ed = self.famille.as_mut().ok_or("Éditeur fermé.")?;
        ed.famille.valider(ed.largeur, ed.hauteur, ed.epaisseur)?;
        crate::modele::echanges::enregistrement::ecrire_atomique(chemin, ed.fichier().json()?.as_bytes())?;
        ed.document_famille.chemin_fichier = Some(chemin.to_path_buf());
        ed.marquer_enregistree();
        self.status = format!("Famille enregistrée : {}", chemin.display());
        Ok(())
    }

    /// Ouvre un fichier de famille dans l'éditeur (nouveau type, à charger ensuite).
    pub(in crate::ui) fn ouvrir_famille_fichier(&mut self, chemin: &std::path::Path) -> Result<(), String> {
        if let Some(ordre) = self.famille.iter().chain(&self.familles_inactives).find(|e| e.document_famille.autonome && e.document_famille.chemin_fichier.as_deref() == Some(chemin)).map(|e| e.document_famille.ordre) {
            self.activer_famille(ordre);
            return Ok(());
        }
        let texte = std::fs::read_to_string(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        let f = crate::modele::famille::modelee::FichierFamille::lire(&texte)?;
        self.editeur_famille(None, None, f.largeur, f.hauteur, 300.0, f.famille.genre);
        let ed = self.famille.as_mut().ok_or("Aucun projet natif.")?;
        (ed.nom, ed.famille) = (f.nom, f.famille);
        ed.document_famille.autonome = true;
        ed.document_famille.chemin_fichier = Some(chemin.to_path_buf());
        ed.marquer_enregistree();
        ed.historique_neuf();
        Ok(())
    }

    /// Charge un fichier de famille dans le projet : un nouveau type (nom rendu unique).
    pub(in crate::ui) fn charger_famille_fichier(&mut self, chemin: &std::path::Path) -> Result<String, String> {
        let texte = std::fs::read_to_string(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        let f = crate::modele::famille::modelee::FichierFamille::lire(&texte)?;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let mut q = p.clone();
        let mut nom = f.nom.clone();
        let mut k = 2;
        while q.types_ouvertures.iter().any(|t| t.nom == nom) {
            nom = format!("{} ({k})", f.nom);
            k += 1;
        }
        let id = crate::modele::collaboration::uuid();
        q.types_ouvertures.push(crate::modele::TypeOuverture { id: id.clone(), nom: nom.clone(), genre: f.famille.genre, largeur: f.largeur, hauteur: f.hauteur, famille: None, modele: Some(f.famille), origine: None, famille_nom: None });
        self.appliquer_projet(q)?;
        self.status = format!("Famille « {nom} » chargée dans le projet.");
        Ok(id)
    }

    /// Charger une famille : choix d'un fichier `.famille.json`, chargé dans le projet ;
    /// `placer` : le nouveau type devient celui des prochaines ouvertures.
    pub(in crate::ui) fn charger_famille_dialogue(&mut self, placer: bool) -> Result<(), String> {
        let Some(chemin) = rfd::FileDialog::new().add_filter("Famille", &["json"]).set_title("Charger une famille").pick_file() else { return Ok(()) };
        let id = self.charger_famille_fichier(&chemin)?;
        let Some(t) = self.natif.as_ref().and_then(|(p, _)| p.types_ouvertures.iter().find(|t| t.id == id)).cloned() else { return Ok(()) };
        if placer {
            use crate::ui::ruban::Outil;
            self.ruban_edition.definir_texte_option(Outil::Ouverture, 0, crate::ui::ruban::nom_genre(t.genre));
            self.ruban_edition.definir_nombre_option(Outil::Ouverture, 1, t.largeur);
            self.ruban_edition.definir_nombre_option(Outil::Ouverture, 2, t.hauteur);
            self.ruban_edition.ouverture_type_identique = Some(id);
        }
        Ok(())
    }

    /// Traite l'action demandée dans les panneaux de l'éditeur (après leur tracé).
    pub(in crate::ui) fn traiter_action_famille(&mut self) {
        let Some(action) = self.action_famille.take() else { return };
        let placement = self.famille.as_ref().is_some_and(|e| e.ouverture.is_none());
        let r = match action {
            ActionFamille::Charger(nouveau) => self.enregistrer_famille(nouveau).map(|id| {
                if placement {
                    self.ruban_edition.ouverture_type_identique = Some(id);
                }
            }),
            ActionFamille::Enregistrer if self.famille.as_ref().and_then(|e| e.document_famille.chemin_fichier.as_ref()).is_some() => {
                let chemin = self.famille.as_ref().and_then(|e| e.document_famille.chemin_fichier.clone()).unwrap();
                self.enregistrer_famille_fichier(&chemin)
            }
            ActionFamille::Enregistrer | ActionFamille::EnregistrerSous => {
                let nom = self.famille.as_ref().map_or_else(String::new, |e| e.nom.trim().to_string());
                match rfd::FileDialog::new().add_filter("Famille", &["json"]).set_file_name(format!("{nom}.famille.json")).save_file() {
                    Some(c) => self.enregistrer_famille_fichier(&c),
                    None => Ok(()),
                }
            }
            ActionFamille::Ouvrir => match rfd::FileDialog::new().add_filter("Famille", &["json"]).pick_file() {
                Some(c) => self.ouvrir_famille_fichier(&c),
                None => Ok(()),
            },
        };
        if let Some(ed) = self.famille.as_mut() {
            ed.erreur = r.err();
        }
    }

    /// Fermeture de l'onglet : confirmation si la famille n'est pas chargée dans le
    /// projet.
    pub(in crate::ui) fn demander_fermeture_famille(&mut self) {
        if let Some(ordre) = self.famille_plane_active {
            if self.familles_planes[&ordre].modifie() { self.confirmer_famille = true; }
            else { self.fermer_famille_plane(); }
            return;
        }
        if self.famille.as_ref().is_some_and(|e| e.modifiee()) {
            self.confirmer_famille = true;
        } else {
            self.fermer_famille();
        }
    }

    pub(in crate::ui) fn confirmation_famille(&mut self, ctx: &egui::Context) {
        if !self.confirmer_famille {
            return;
        }
        let plane = self.famille_plane_active.is_some();
        let autonome = if plane { self.famille_plane_active.and_then(|i| self.familles_planes.get(&i)).is_some_and(|e| e.document_famille.autonome) } else { self.famille.as_ref().is_some_and(|e| e.document_famille.autonome) };
        let nouveau = !plane && self.famille.as_ref().is_some_and(|e| e.type_id.is_none());
        let mut choix = None;
        egui::Modal::new(egui::Id::unique("fermer_famille")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Famille modifiée");
            ui.label(if autonome { "Les modifications ne sont pas enregistrées dans le fichier." } else { "Les modifications ne sont pas chargées dans le projet." });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(if autonome { "Enregistrer et fermer" } else if nouveau { "Créer le type et fermer" } else { "Charger et fermer" }).clicked() {
                    choix = Some(0);
                }
                if ui.button(if autonome { "Fermer sans enregistrer" } else { "Fermer sans charger" }).clicked() {
                    choix = Some(1);
                }
                if ui.button("Annuler").clicked() {
                    choix = Some(2);
                }
            });
        });
        if plane && let Some(choix) = choix {
            self.confirmer_famille = false;
            match choix {
                0 if autonome => match self.enregistrer_famille_plane_dialogue(false) {
                    Ok(true) => self.fermer_famille_plane(),
                    Ok(false) => {},
                    Err(e) => self.error = Some(e),
                },
                0 => match self.enregistrer_famille_plane(false) {
                    Ok(()) => self.fermer_famille_plane(),
                    Err(e) => self.error = Some(e),
                },
                1 => self.fermer_famille_plane(),
                _ => {}
            }
            return;
        }
        match choix {
            Some(0) => {
                self.confirmer_famille = false;
                if autonome {
                    let chemin = self.famille.as_ref().and_then(|e| e.document_famille.chemin_fichier.clone()).or_else(|| {
                        let nom = self.famille.as_ref().map_or("Famille", |e| e.nom.as_str());
                        rfd::FileDialog::new().add_filter("Famille", &["json"]).set_file_name(format!("{nom}.famille.json")).save_file()
                    });
                    if let Some(chemin) = chemin {
                        match self.enregistrer_famille_fichier(&chemin) {
                            Ok(()) => self.fermer_famille(),
                            Err(e) => if let Some(ed) = self.famille.as_mut() { ed.erreur = Some(e); },
                        }
                    }
                    return;
                }
                match self.enregistrer_famille(nouveau) {
                    Ok(_) => self.fermer_famille(),
                    Err(e) => {
                        if let Some(ed) = self.famille.as_mut() {
                            ed.erreur = Some(e);
                        }
                    }
                }
            }
            Some(1) => {
                self.confirmer_famille = false;
                self.fermer_famille();
            }
            Some(_) => self.confirmer_famille = false,
            None => {}
        }
    }
}

/// Titre d'onglet d'un éditeur : catégorie, type, « * » si non chargé.
pub(super) fn titre_editeur(ed: &EditeurFamille) -> String {
    let genre = match ed.famille.genre { GenreOuverture::Porte => "Porte", GenreOuverture::Libre => "Composant", GenreOuverture::Fenetre => "Fenêtre" };
    format!("{genre} : {}{}", ed.nom, if ed.modifiee() { " *" } else { "" })
}
