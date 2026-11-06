//! Sélection : clic, rectangle, cadrage sur les objets.
use super::*;

impl ViewerApp {
    pub(super) fn select(&mut self, id: Option<Id>, reveal: bool) {
        self.bim.selection.clear();
        self.bim.origine = false;
        self.selection_multiple.clear();
        self.coupe_dirty = true;
        self.selected = id;
        if reveal {
            self.reveal = id;
            if id.is_some() {
                self.left_tab = LeftTab::Tree;
                self.filter.clear();
            }
        }
    }

    /// Propriétés éditables d'un mur natif (mur seul ou couche d'assemblage) ou d'un
    /// assemblage. Retourne le projet modifié s'il a changé.
    /// Sélecteur de type de l'élément natif sélectionné (seul) : mur composé (couches
    /// reconstruites d'après le type), mur seul (types à une couche), couche d'un mur
    /// composé (type du mur composé), élément horizontal, porte ou fenêtre. Projet
    /// modifié si un autre type est choisi.
    pub(super) fn selecteur_type_selection(&mut self, ui: &mut egui::Ui) -> Option<crate::modele::Projet> {
        use crate::modele::edition::commandes::Commande;
        let ids = self.selection_bim();
        let [id] = ids.as_slice() else { return None };
        let (p, _) = self.natif.as_ref()?;
        // (élément, type actuel, types proposés (id, nom, détail), genre)
        enum Cible { Assemblage(String), Mur(String), Dalle(String), Ouverture(String) }
        let mur = p.murs.iter().find(|m| &m.id == id);
        let (cible, actuel, types): (Cible, String, Vec<(String, String, String)>) = if let Some(a) = p.assemblage(id).or_else(|| mur.and_then(|m| m.assemblage.as_deref()).and_then(|a| p.assemblage(a))) {
            let types = p.types_murs.iter().filter(|t| t.couches.len() > 1).map(|t| (t.id.clone(), t.nom.clone(), format!("{} couches · {}", t.couches.len(), crate::config::options::unite().afficher(t.epaisseur())))).collect();
            (Cible::Assemblage(a.id.clone()), a.type_mur.clone(), types)
        } else if let Some(m) = mur {
            let types = p.types_murs.iter().filter(|t| t.couches.len() == 1).map(|t| (t.id.clone(), t.nom.clone(), crate::config::options::unite().afficher(t.epaisseur()))).collect();
            (Cible::Mur(m.id.clone()), m.type_mur.clone(), types)
        } else if let Some(d) = p.dalles.iter().find(|d| &d.id == id) {
            let types = p.types_dalles.iter().map(|t| (t.id.clone(), t.nom.clone(), crate::config::options::unite().afficher(t.epaisseur()))).collect();
            (Cible::Dalle(d.id.clone()), d.type_dalle.clone(), types)
        } else if let Some(o) = p.ouvertures.iter().find(|o| &o.id == id) {
            // portes et fenêtres : même objet, rangé par catégorie (libre à part)
            use crate::modele::GenreOuverture as G;
            let libre = o.genre == G::Libre;
            let mut types: Vec<_> = p.types_ouvertures.iter().filter(|t| (t.genre == G::Libre) == libre).collect();
            types.sort_by_key(|t| t.genre != G::Porte);
            let types = types.into_iter().map(|t| {
                let categorie = match t.genre { G::Porte => "Porte", G::Fenetre => "Fenêtre", G::Libre => "Libre" };
                (t.id.clone(), t.nom.clone(), format!("{categorie} · {} × {}", crate::config::options::unite().afficher(t.largeur), crate::config::options::unite().afficher(t.hauteur)))
            }).collect();
            (Cible::Ouverture(o.id.clone()), o.type_ouverture.clone().unwrap_or_default(), types)
        } else {
            return None;
        };
        if matches!(cible,Cible::Mur(_) | Cible::Assemblage(_)) {
            // Le même widget que pour la création, avec le type du mur sélectionné.
            let projet=p.clone();
            let type_creation=self.ruban_edition.type_mur.clone();
            self.ruban_edition.type_mur=Some(actuel.clone());
            self.ruban_edition.selecteur_type_mur(ui,&projet,false);
            let mut q=projet.clone();
            let gabarit=self.ruban_edition.prendre_type_gabarit();
            let choisi=if let Some(t)=gabarit {
                match self.ruban_edition.definir_type_mur(&mut q,&t.nom,t.couches,t.categorie) {
                    Ok(id)=>Some(id),Err(e)=>{self.error=Some(e);None}
                }
            } else {self.ruban_edition.type_mur.clone()};
            self.ruban_edition.type_mur=type_creation;
            let Some(choisi)=choisi.filter(|id|id!=&actuel) else{return None};
            match cible {
                Cible::Assemblage(a)=>{
                    let axe=match crate::modele::volumes::solides::hote(&q,&a) {Ok((axe,_,_))=>axe,Err(e)=>{self.error=Some(e);return None}};
                    let decalage=q.type_mur(&choisi).and_then(|t|t.decalages_couches().first().copied()).unwrap_or(0.0);
                    let premiere=q.assemblage(&a).and_then(|x|x.couches.first()).cloned();
                    if let Some(x)=q.assemblages.iter_mut().find(|x|x.id==a) {x.type_mur=choisi;}
                    if let Some(m)=q.murs.iter_mut().find(|m|Some(&m.id)==premiere.as_ref()) {m.axe=crate::modele::murs::assemblage::decaler_axe(&axe,decalage);}
                    match q.executer(Commande::ActualiserType{assemblage:a}) {
                        Ok((r,_))=>q=r,Err(e)=>{self.error=Some(e);return None}
                    }
                    if let Err(e)=q.recaler_menuiseries(&projet).and_then(|_|q.verifier()) {self.error=Some(e);return None;}
                }
                Cible::Mur(id)=>{
                    if let Some(m)=q.murs.iter_mut().find(|m|m.id==id) {m.type_mur=choisi;}
                    if let Err(e)=q.eclater_mur(&id).and_then(|_|q.recaler_menuiseries(&projet)).and_then(|_|q.verifier()) {
                        self.error=Some(e);return None;
                    }
                }
                _=>unreachable!(),
            }
            return Some(q);
        }
        let nom_actuel = types.iter().find(|t| t.0 == actuel).map_or_else(|| p.type_mur(&actuel).or_else(|| p.type_dalle(&actuel)).map_or("Type".to_string(), |t| t.nom.clone()), |t| t.1.clone());
        let mut choisi = actuel.clone();
        egui::ComboBox::from_id_salt("selecteur_type_selection")
            .width(ui.available_width() - 8.0)
            .truncate()
            .selected_text(RichText::new(&nom_actuel).strong())
            .show_ui(ui, |ui| {
                for (tid, nom, detail) in &types {
                    ui.selectable_value(&mut choisi, tid.clone(), format!("{nom} — {detail}"));
                }
            })
            .response
            .on_hover_text(match cible {
                Cible::Assemblage(_) => "Type du mur composé : ses couches sont reconstruites d'après le type choisi",
                _ => "Type de l'élément sélectionné",
            });
        ui.separator();
        if choisi == actuel {
            return None;
        }
        let mut q = p.clone();
        match cible {
            Cible::Assemblage(a) => {
                if let Some(x) = q.assemblages.iter_mut().find(|x| x.id == a) {
                    x.type_mur = choisi;
                }
                match q.executer(Commande::ActualiserType { assemblage: a }) {
                    Ok((r, _)) => q = r,
                    Err(e) => {
                        self.error = Some(e);
                        return None;
                    }
                }
            }
            Cible::Mur(m) => q.murs.iter_mut().filter(|x| x.id == m).for_each(|x| x.type_mur = choisi.clone()),
            Cible::Dalle(d) => q.dalles.iter_mut().filter(|x| x.id == d).for_each(|x| x.type_dalle = choisi.clone()),
            Cible::Ouverture(o) => {
                let t = q.types_ouvertures.iter().find(|t| t.id == choisi).cloned();
                if let (Some(x), Some(t)) = (q.ouvertures.iter_mut().find(|x| x.id == o), t) {
                    x.type_ouverture = Some(t.id);
                    (x.largeur, x.hauteur, x.genre) = (t.largeur, t.hauteur, t.genre);
                }
            }
        }
        Some(q)
    }

    /// Élément sélectionné par un clic sur l'objet `id` : une couche d'assemblage
    /// sélectionne son assemblage, sauf si l'assemblage (ou l'une de ses couches) l'est
    /// déjà : le clic suivant entre dans l'assemblage et prend la couche.
    pub(super) fn cible_clic(&self, id: Id) -> Id {
        let Some(s) = &self.scene else { return id };
        let Some(&parent) = s.model.parent.get(&id) else { return id };
        if s.model.ty(parent) != "IFCELEMENTASSEMBLY" {
            return id;
        }
        let dedans = self.selected.is_some_and(|sel| sel == parent || s.model.parent.get(&sel) == Some(&parent));
        if dedans { id } else { parent }
    }

    /// Tab : de l'assemblage sélectionné à sa première couche, puis couche suivante,
    /// puis retour à l'assemblage.
    pub(super) fn tab_selection(&mut self) {
        let Some(s) = &self.scene else { return };
        let Some(sel) = self.selected else { return };
        let (assemblage, courant) = if s.model.ty(sel) == "IFCELEMENTASSEMBLY" {
            (sel, None)
        } else {
            match s.model.parent.get(&sel) {
                Some(&p) if s.model.ty(p) == "IFCELEMENTASSEMBLY" => (p, Some(sel)),
                _ => return,
            }
        };
        let couches = s.model.children.get(&assemblage).cloned().unwrap_or_default();
        let suivant = match courant {
            None => couches.first().copied(),
            Some(c) => couches.iter().position(|&x| x == c).and_then(|k| couches.get(k + 1).copied()),
        };
        self.select(Some(suivant.unwrap_or(assemblage)), true);
    }

    /// Sélectionne les objets de la scène, y compris les couches des assemblages.
    pub(super) fn objets_selectionnes(&self) -> Vec<u8> {
        let Some(scene) = &self.scene else { return Vec::new() };
        let mut marques = vec![0; scene.objects.len()];
        let mut entites = Vec::new();
        if !self.bim.selection.is_empty() {
            let selection: HashSet<&str> = self.bim.selection.iter().map(String::as_str).collect();
            if let Some((p, _)) = &self.natif {
                for (genre, cle) in p.murs.iter().map(|x| ("mur", &x.id))
                    .chain(p.assemblages.iter().map(|x| ("assemblage", &x.id)))
                    .chain(p.dalles.iter().map(|x| ("dalle", &x.id)))
                    .chain(p.terrains.iter().map(|x| ("terrain", &x.id)))
                    .chain(p.limites_propriete.iter().map(|x| ("limite_propriete", &x.id)))
                    .chain(p.barres.iter().map(|x| ("barre", &x.id)))
                .chain(p.escaliers.iter().map(|x| ("escalier", &x.id)))
                .chain(p.garde_corps.iter().map(|x| ("garde_corps", &x.id)))
                    .chain(p.ouvertures.iter().map(|x| ("ouverture", &x.id)))
                    .chain(p.importes.iter().map(|x| ("importe", &x.id))) {
                    if selection.contains(cle.as_str()) {
                        entites.push(scene::id_natif(genre, cle));
                    }
                }
            }
        } else if !self.selection_multiple.is_empty() {
            entites.extend(self.selection_multiple.iter().copied());
        } else if let Some(id) = self.selected {
            entites.push(id);
        }
        for id in entites {
            let objets = if scene.model.ty(id) == "IFCELEMENTASSEMBLY" {
                self.objects_under(id)
            } else {
                scene.object_of.get(&id).copied().into_iter().collect()
            };
            for objet in objets {
                if let Some(marque) = marques.get_mut(objet as usize) { *marque = 255; }
            }
        }
        for &o in &self.objets_masques {
            if let Some(marque) = marques.get_mut(o as usize).filter(|m| **m == 0) { *marque = 128; }
        }
        // éléments démolis : teinte de démolition (octet 48)
        if let Some((p, _)) = &self.natif {
            if !p.demolitions.is_empty() {
                for (i, o) in scene.objects.iter().enumerate() {
                    if marques[i] == 0 && self.cle_native(self.cible_clic(o.id)).is_some_and(|id| p.demolitions.contains_key(&id)) {
                        marques[i] = 48;
                    }
                }
            }
        }
        marques
    }

    pub(super) fn selectionner_rectangle(&mut self, vue: egui::Rect, fenetre: FenetreSelection) {
        let Some(scene) = &self.scene else { return };
        let rectangle = fenetre.rectangle();
        let croisement = fenetre.croisement();
        let mut resultats = crate::rendu::selection_rectangle::objets(scene, &self.visible, self.camera.view_proj(vue.width() / vue.height().max(1.0)), vue, rectangle, self.plan_coupe(), self.boite_appliquee(), &self.cadrage_vue(), &self.contour_cadrage_vue());
        // plan : les éléments de la plage haute (au-dessus de la coupe) se sélectionnent aussi
        if self.vue_en_plan() && let Some((haut, limite)) = self.masque_plage_haute() {
            let autres = crate::rendu::selection_rectangle::objets(scene, &haut, self.camera.view_proj(vue.width() / vue.height().max(1.0)), vue, rectangle, Some(Vec4::new(0., 0., -1., limite)), self.boite_appliquee(), &self.cadrage_vue(), &self.contour_cadrage_vue());
            for (r, a) in resultats.iter_mut().zip(autres) { if a.0 { *r = a; } }
        }
        let mut ids = crate::rendu::selection_rectangle::groupes(scene.objects.iter().zip(resultats).map(|(o, r)| {
            let id = scene.model.parent.get(&o.id).filter(|id| scene.model.ty(**id) == "IFCELEMENTASSEMBLY").copied().unwrap_or(o.id);
            (id, r)
        }), croisement);
        ids.sort_unstable();
        if fenetre.ajouter {
            let mut ancienne = self.selection_multiple.clone();
            if ancienne.is_empty() {
                if !self.bim.selection.is_empty() {
                    if let Some((p, _)) = &self.natif {
                        let cles: HashSet<&str> = self.bim.selection.iter().map(String::as_str).collect();
                        ancienne.extend(p.murs.iter().map(|x| ("mur", &x.id))
                            .chain(p.assemblages.iter().map(|x| ("assemblage", &x.id)))
                            .chain(p.dalles.iter().map(|x| ("dalle", &x.id)))
                            .chain(p.terrains.iter().map(|x| ("terrain", &x.id)))
                            .chain(p.limites_propriete.iter().map(|x| ("limite_propriete", &x.id)))
                            .chain(p.barres.iter().map(|x| ("barre", &x.id)))
                .chain(p.escaliers.iter().map(|x| ("escalier", &x.id)))
                .chain(p.garde_corps.iter().map(|x| ("garde_corps", &x.id)))
                            .chain(p.ouvertures.iter().map(|x| ("ouverture", &x.id)))
                            .chain(p.importes.iter().map(|x| ("importe", &x.id)))
                            .filter(|(_, id)| cles.contains(id.as_str())).map(|(genre, id)| scene::id_natif(genre, id)));
                    }
                } else if let Some(id) = self.selected { ancienne.push(id); }
            }
            ancienne.extend(ids); ancienne.sort_unstable(); ancienne.dedup(); ids = ancienne;
        }
        let mut vues = if fenetre.ajouter { self.bim.selection.clone() } else { Vec::new() };
        if self.vue_en_plan() {
            if let Some((p, _)) = &self.natif {
                let z = self.altitude_vue_native();
                let masques = self.vue_native_id().and_then(|id| p.vues.iter().find(|v| v.id == id)).map(|v| v.elements_masques.as_slice()).unwrap_or(&[]);
                for v in &p.vues {
                    if masques.contains(&v.id) { continue; }
                    if let crate::modele::GenreVue::Coupe { debut, fin, .. } = &v.genre {
                        if let (Some(a), Some(b)) = (self.ecran_plan(vue, *debut, z), self.ecran_plan(vue, *fin, z)) {
                            if crate::rendu::selection_rectangle::segment(rectangle, a, b, croisement) { vues.push(v.id.clone()); }
                        }
                    }
                }
                for g in &p.quadrillages {
                    if masques.contains(&g.id) { continue; }
                    if let (Some(a), Some(b)) = (self.ecran_plan(vue, g.debut, z), self.ecran_plan(vue, g.fin, z)) {
                        if crate::rendu::selection_rectangle::segment(rectangle, a, b, croisement) { vues.push(g.id.clone()); }
                    }
                }
            }
        }
        if let Some((p, _)) = &self.natif {
            vues.retain(|id| p.vues.iter().any(|v| v.id == *id) || p.quadrillages.iter().any(|g| g.id == *id) || p.annotations.iter().any(|a| a.id == *id));
        }
        for (id, bounds) in self.rectangles_annotations(vue) {
            if if croisement { rectangle.intersects(bounds) } else { rectangle.contains_rect(bounds) } { vues.push(id); }
        }
        vues.sort(); vues.dedup();
        self.selected = ids.first().copied();
        let recherches: HashSet<Id> = ids.iter().copied().collect();
        self.bim.selection = self.natif.as_ref().map(|(p, _)| {
            p.murs.iter().map(|x| ("mur", &x.id))
                .chain(p.assemblages.iter().map(|x| ("assemblage", &x.id)))
                .chain(p.dalles.iter().map(|x| ("dalle", &x.id)))
                .chain(p.terrains.iter().map(|x| ("terrain", &x.id)))
                .chain(p.limites_propriete.iter().map(|x| ("limite_propriete", &x.id)))
                .chain(p.barres.iter().map(|x| ("barre", &x.id)))
                .chain(p.escaliers.iter().map(|x| ("escalier", &x.id)))
                .chain(p.garde_corps.iter().map(|x| ("garde_corps", &x.id)))
                .chain(p.ouvertures.iter().map(|x| ("ouverture", &x.id)))
                .chain(p.importes.iter().map(|x| ("importe", &x.id)))
                .filter(|(genre, id)| recherches.contains(&scene::id_natif(genre, id)))
                .map(|(_, id)| id.clone()).collect()
        }).unwrap_or_default();
        self.bim.selection.extend(vues);
        self.coupe_dirty = true;
        self.selection_multiple = ids;
        self.status = format!("{} élément(s) sélectionné(s).", if self.natif.is_some() { self.bim.selection.len() } else { self.selection_multiple.len() });
    }

    /// Objets 3D d'un nœud et de ses descendants.
    pub(super) fn objects_under(&self, id: Id) -> Vec<u32> {
        let Some(s) = &self.scene else { return vec![] };
        let mut out = Vec::new();
        let mut stack = vec![id];
        let mut seen = HashSet::new();
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            if let Some(&o) = s.object_of.get(&n) {
                out.push(o);
            }
            if let Some(ch) = s.model.children.get(&n) {
                stack.extend(ch.iter().copied());
            }
        }
        out
    }

    pub(super) fn frame_objects(&mut self, objs: &[u32]) {
        let Some(s) = &self.scene else { return };
        let mut mn = Vec3::splat(f32::MAX);
        let mut mx = Vec3::splat(f32::MIN);
        for &o in objs {
            let o = &s.objects[o as usize];
            mn = mn.min(o.min);
            mx = mx.max(o.max);
        }
        if mn.x <= mx.x {
            self.camera.fit(mn, mx);
        }
    }

    pub(super) fn fit_all(&mut self) {
        let Some(s) = &self.scene else { return };
        let objs: Vec<u32> = (0..s.objects.len() as u32).filter(|&i| self.visible[i as usize]).collect();
        if objs.is_empty() {
            let (a, b) = (s.min, s.max);
            self.camera.fit(a, b);
        } else {
            self.frame_objects(&objs);
        }
    }

}
