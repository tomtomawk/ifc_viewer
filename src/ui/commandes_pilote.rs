//! Commandes du pilote (MCP) : capture, état, exécution.
use super::*;

impl ViewerApp {
    /// Traite les commandes du pilote ; répond aux ouvertures terminées.
    pub(super) fn piloter(&mut self, ctx: &egui::Context) {
        if self.loading.is_none() && !self.attente_ouverture.is_empty() {
            let etat = match &self.error {
                Some(e) => pilote::erreur(e),
                None => self.etat(),
            };
            for tx in self.attente_ouverture.drain(..) {
                let _ = tx.send(etat.clone());
            }
        }
        for r in std::mem::take(&mut self.requetes) {
            let reponse = match r.commande.as_str() {
                "ouvrir" => {
                    let p = PathBuf::from(r.argument.trim_matches('"'));
                    if p.is_file() {
                        self.open(p, ctx);
                        self.attente_ouverture.push(r.reponse);
                        continue;
                    }
                    pilote::erreur(&format!("fichier introuvable : {}", r.argument))
                }
                "capture" => {
                    self.capture(ctx, &r.argument, r.reponse);
                    continue;
                }
                _ => self.commande(ctx, &r.commande, &r.argument),
            };
            let _ = r.reponse.send(self.avec_document_actif(reponse));
        }
    }

    pub(super) fn frame_params(&self, aspect: f32) -> FrameParams {
        let vp = self.camera.view_proj(aspect);
        let passes = self
            .passes_feuille
            .iter()
            .map(|p| {
                // rectangle de découpe : coins projetés, en coordonnées normalisées
                let ciseaux = p.cadre.map(|c| {
                    let coins = [[c[0], c[1]], [c[2], c[1]], [c[2], c[3]], [c[0], c[3]]].map(|[x, y]| vp.project_point3(Vec3::new(x, y, p.z)));
                    let (mn, mx) = coins.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
                    [mn.x, mn.y, mx.x, mx.y]
                });
                ParamsPasse { mvp: vp * p.vers_feuille, view_dir: -p.direction, ciseaux, plans: p.plans.clone() }
            })
            .collect();
        FrameParams {
            passes,
            mvp: vp,
            eye: self.camera.eye(),
            view_dir: self.camera.forward(),
            ortho: self.camera.ortho,
            selected: self.objets_selectionnes(),
            background: if self.style.papier() { [1.0, 1.0, 1.0] } else { [0.925, 0.933, 0.945] },
            style: self.style,
            faces_blanches: self.faces_blanches,
            eclairage: self.eclairage,
            mm: (self.echelle / 1000.0) as f32,
            table_motifs: self.scene.as_ref().map(|s| s.table_motifs.clone()).unwrap_or_default(),
            soleil: self.ombres_portees.then(|| self.soleil(aspect)).flatten(),
            // occultation : rayon de l'ordre du mètre, réduit de près, élargi de loin
            ao: self.ombre_ambiante.then(|| (self.camera.proj(aspect), (self.camera.dist * 0.04).clamp(0.15, 1.5))),
            coupe: self.plan_coupe(),
            boite: self.boite_appliquee(),
            cadrage: self.cadrage_vue(),
            contour_vue: self.contour_cadrage_vue(),
        }
    }

    /// Capture en PNG, argument « vue|fenetre [LxH] chemin ». La vue 3D est rendue hors
    /// écran, à la taille de la vue ou à celle demandée, même fenêtre réduite ; la
    /// fenêtre entière (interface comprise) est lue à l'image suivante.
    pub(super) fn capture(&mut self, ctx: &egui::Context, argument: &str, reponse: Sender<String>) {
        let (zone, reste) = argument.trim().split_once(' ').unwrap_or((argument, ""));
        let mut reste = reste.trim();
        let mut taille = None;
        if let Some((t, apres)) = reste.split_once(' ') {
            if let Some((Ok(l), Ok(h))) = t.split_once('x').map(|(l, h)| (l.parse::<u32>(), h.parse::<u32>())) {
                taille = Some((l, h));
                reste = apres.trim();
            }
        }
        let chemin = reste.trim_matches('"').to_string();
        if chemin.is_empty() {
            let _ = reponse.send(pilote::erreur("usage : capture vue|fenetre [LxH] <chemin.png>"));
            return;
        }
        let ecrire = move |l: usize, h: usize, rgba: &[u8]| {
            let res = pilote::png(l, h, rgba).and_then(|png| std::fs::write(&chemin, png).map_err(|e| e.to_string()));
            match res {
                Ok(()) => pilote::succes(&[("chemin", pilote::texte(&chemin)), ("largeur", l.to_string()), ("hauteur", h.to_string())]),
                Err(e) => pilote::erreur(&e),
            }
        };
        if zone == "fenetre" {
            // egui ne lit pas une fenêtre de taille nulle (réduite)
            if ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
                let _ = reponse.send(pilote::erreur("fenêtre réduite : seule la vue 3D se capture (zone « vue »)"));
                return;
            }
            let attente = self.captures_en_attente.clone();
            attente.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            ctx.request_screenshot(move |image| {
                let [w, h] = image.size;
                let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| [p.r(), p.g(), p.b(), 255]).collect();
                let _ = reponse.send(ecrire(w, h, &rgba));
                attente.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            });
            ctx.request_repaint();
            return;
        }
        let ppp = ctx.pixels_per_point();
        let (l, h) = taille.unwrap_or(((self.rect_vue.width() * ppp).round() as u32, (self.rect_vue.height() * ppp).round() as u32));
        let (l, h) = (l.clamp(16, 4096), h.clamp(16, 4096));
        self.update_visibility();
        self.update_coupe();
        let params = self.frame_params(l as f32 / h as f32);
        let res = match self.renderer.as_mut() {
            Some(r) => r.capture(&params, l as i32, h as i32),
            None => Err("rendu wgpu indisponible".into()),
        };
        let _ = reponse.send(match res {
            Ok(rgba) => ecrire(l as usize, h as usize, &rgba),
            Err(e) => pilote::erreur(&e),
        });
    }

    /// État du viewer : fichier, vue, caméra, visibilité, sélection.
    pub(super) fn etat(&self) -> String {
        use pilote::{liste, objet, texte};
        let mut p = vec![
            ("fichier", self.scene.as_ref().map(|s| texte(&s.file_name)).unwrap_or_else(|| "null".into())),
            ("statut", texte(&self.status)),
            ("chargement", self.loading.is_some().to_string()),
            ("enregistrement", self.sauvegarde.is_some().to_string()),
            ("sortie_v0", self.natif.as_ref().map(|n| &n.1).or(self.rvt_sortie.as_ref()).map_or("null".into(), |p| texte(&p.to_string_lossy()))),
            ("fiche_revit", (self.rvt.is_some() && self.rvt_sheet).to_string()),
        ];
        p.push(("document_actif", self.document_actif_pilote().to_string()));
        p.push(("saisies_en_attente", (self.saisies_pilote.len()+usize::from(self.saisie_pilote_image)).to_string()));
        p.push(("selection_native", serde_json::to_string(&self.selection_bim()).unwrap()));
        p.push(("afficher_masques", self.afficher_masques.to_string()));
        p.push(("objets_masques", self.objets_masques.len().to_string()));
        p.push(("transformation", self.etat_transformation().to_string()));
        p.push(("accrochage", self.etat_accrochage().to_string()));
        p.push(("cadenas", serde_json::to_string(&self.cadenas_ecran.iter().map(|(id, q)| serde_json::json!({"id": id, "ecran": [q.x, q.y]})).collect::<Vec<_>>()).unwrap()));
        p.push(("outil", self.ruban_edition.outil().map_or("null".into(), |o|texte(o.nom()))));
        p.push(("rectangle_vue", serde_json::to_string(&[self.rect_vue.min.x,self.rect_vue.min.y,self.rect_vue.width(),self.rect_vue.height()]).unwrap()));
        if let Some(c)=&self.collaboration { p.push(("collaboration",serde_json::json!({"etat":c.etat,"revision":c.copie.base.revision,"en_attente":c.copie.attente.len(),"conflits":c.copie.conflits,"comparaison_validation_ms":c.mesure_validation_ms,"geometrie_ms":c.mesure_geometrie_ms,"preparation_transfert_gpu_ms":c.mesure_gpu_ms,"synchronisation_ms":c.mesure_sync_ms,"fichiers":c.mesures_fichiers}).to_string())); }
        if let Some(e) = &self.error {
            p.push(("erreur_chargement", texte(e)));
        }
        if let Some(s) = &self.scene {
            p.push(("schema", texte(&s.model.file.schema)));
            p.push(("repere_geographique",s.partage.as_ref().map(|r|texte(&r.nom)).unwrap_or_else(||"null".into())));
            p.push(("objets", s.objects.len().to_string()));
            p.push(("objets_visibles", self.visible.iter().filter(|v| **v).count().to_string()));
            p.push(("triangles", s.triangles.to_string()));
            p.push(("vues_revit", s.navigateur.as_ref().map_or(0, |n| n.vues.len()).to_string()));
        }
        if let Some(v) = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            p.push(("vue", objet(&[("indice", self.vue_active.unwrap_or(0).to_string()), ("genre", texte(&v.genre)), ("nom", texte(&v.nom)), ("echelle", v.echelle.to_string()), ("niveau_detail", texte(self.detail.libelle()))])));
        }
        if let (Some((a, b)), Some(s)) = (self.boite, &self.scene) {
            let (a, b) = (a.as_dvec3() + s.origin, b.as_dvec3() + s.origin);
            p.push(("zone_de_coupe", objet(&[("active", self.boite_active.to_string()), ("min", format!("[{:.3},{:.3},{:.3}]", a.x, a.y, a.z)), ("max", format!("[{:.3},{:.3},{:.3}]", b.x, b.y, b.z))])));
        }
        let c = &self.camera;
        p.push((
            "camera",
            objet(&[
                ("lacet_deg", format!("{:.2}", c.yaw.to_degrees())),
                ("tangage_deg", format!("{:.2}", c.pitch.to_degrees())),
                ("distance", format!("{:.3}", c.dist)),
                ("cible", format!("[{:.3},{:.3},{:.3}]", c.target.x, c.target.y, c.target.z)),
                ("ortho", c.ortho.to_string()),
            ]),
        ));
        p.push((
            "dessin",
            objet(&[
                ("actif", self.plan_coupe().is_some().to_string()),
                ("style", texte(self.style.nom())),
                ("coupe_z", self.plan_coupe().map_or("null".into(), |c| format!("{:.3}", c.w))),
                ("niveau", self.coupe_niveau.map_or("null".into(), |n| self.decrire(n))),
                ("hauteur", format!("{:.3}", self.coupe_hauteur)),
                ("poche", texte(self.poche.nom())),
                ("echelle", self.echelle.to_string()),
            ]),
        ));
        let mut masquees: Vec<&String> = self.hidden_classes.iter().collect();
        masquees.sort();
        p.push(("classes_masquees", liste(masquees.into_iter().map(|c| texte(c)))));
        p.push(("isolement", self.isolated.as_ref().map_or("null".into(), |i| i.len().to_string())));
        if self.natif.is_some() {
            p.push(("selection_native", liste(self.selection_bim().iter().map(|id| texte(id)))));
        }
        if !self.selection_multiple.is_empty() {
            p.push(("selections", liste(self.selection_multiple.iter().map(|id| self.decrire(*id)))));
        }
        if let Some(id) = self.selected {
            p.push(("selection", self.decrire(id)));
        }
        p.push(("vue_px", format!("[{:.0},{:.0}]", self.rect_vue.width(), self.rect_vue.height())));
        p.push(("panneaux", objet(&Panneau::TOUS.map(|q| (q.nom(), texte(self.ancrages[q as usize].nom()))))));
        p.push(("visibilite_ouverte", self.voir_visibilite.to_string()));
        pilote::succes(&p)
    }

    pub(super) fn decrire(&self, id: Id) -> String {
        let Some(s) = &self.scene else { return "null".into() };
        pilote::objet(&[("id", id.to_string()), ("classe", pilote::texte(s.model.ty(id))), ("nom", pilote::texte(&s.model.name(id)))])
    }

    /// Entité désignée par « #123 », « 123 » ou, à défaut, la sélection.
    pub(super) fn entite(&self, argument: &str) -> Result<Id, String> {
        let s = self.scene.as_ref().ok_or("aucun fichier ouvert")?;
        let a = argument.trim().trim_start_matches('#');
        let id = if a.is_empty() { self.selected.ok_or("aucune sélection")? } else { a.parse::<Id>().map_err(|_| format!("id invalide : {argument}"))? };
        if s.model.get(id).is_none() {
            return Err(format!("entité #{id} absente"));
        }
        Ok(id)
    }

    pub(super) fn commande(&mut self, ctx: &egui::Context, commande: &str, argument: &str) -> String {
        use pilote::{erreur, liste, objet, succes, texte};
        if matches!(commande,"editer"|"annuler"|"retablir"|"mur"|"dalle"|"pente"|"type_mur"|"vue_plan"|"coupe_vue"|"enregistrer"|"modifier_trace"|"famille_ouvrir"|"famille_charger"|"famille_enregistrer"|"collaborer"|"collaboration_associer") {
            if let Err(e)=self.pilote_disponible() {return erreur(&e);}
        }
        if let Some(r) = self.commande_complete(ctx, commande, argument) {
            return match r { Ok(v) => { let mut v = v; if v.get("ok").is_none() { v["ok"] = true.into(); } v.to_string() }, Err(e) => erreur(&e) };
        }
        match commande {
            "ruban" => {self.onglet_ruban=match argument{"debut"=>OngletRuban::Debut,"modeliser"=>OngletRuban::Modeliser,"modifier"=>OngletRuban::Modifier,"vue"=>OngletRuban::Vue,"documenter"|"annoter"=>OngletRuban::Annoter,"sortie"=>OngletRuban::Sortie,"famille_creer"=>OngletRuban::FamilleCreer,"famille_sortie"=>OngletRuban::FamilleSortie,_=>return erreur("usage : ruban debut|modeliser|modifier|vue|documenter|sortie|famille_creer|famille_sortie")};self.etat()},
            "collaborer" => match self.creer_partage(PathBuf::from(argument.trim_matches('"')),ctx){Ok(())=>succes(&[("etat",texte("Création du projet partagé en cours"))]),Err(e)=>erreur(&e)},
            "collaboration_associer" => match self.associer_collaboration(PathBuf::from(argument.trim_matches('"'))) { Ok(())=>succes(&[("etat",texte("Association locale effectuée ; synchronisation en cours"))]),Err(e)=>erreur(&e) },
            "collaboration_sync" | "collaboration_instantane" => match self.collaboration.as_mut().ok_or_else(||"Copie non associée".to_string()).and_then(|c|c.synchroniser(None,commande=="collaboration_instantane")) {Ok(())=>succes(&[("etat",texte("Synchronisation demandée"))]),Err(e)=>erreur(&e)},
            "etat" => self.etat(),
            // famille_enregistrer|famille_ouvrir|famille_charger <chemin> : fichier de famille
            "famille_enregistrer" | "famille_ouvrir" | "famille_charger" => {
                let chemin = std::path::Path::new(argument.trim());
                let r = match commande {
                    "famille_enregistrer" => self.enregistrer_famille_fichier(chemin).map(|_| String::new()),
                    "famille_ouvrir" => self.ouvrir_famille_fichier(chemin).map(|_| String::new()),
                    _ => self.charger_famille_fichier(chemin),
                };
                match r { Ok(id) => succes(&[("id", texte(&id))]), Err(e) => erreur(&e) }
            }
            "menu" => {
                use super::menu_vue::ActionMenu as A;
                let a = match argument.trim() {
                    "repeter" => match self.derniere_commande { Some(o) => A::Repeter(o), None => return erreur("aucune commande à répéter") },
                    "identiques_vue" => A::Identiques { dans_la_vue: true },
                    "identiques_projet" => A::Identiques { dans_la_vue: false },
                    "verrouiller" => A::Verrou(true),
                    "deverrouiller" => A::Verrou(false),
                    "demolir" => A::Demolir,
                    "inverser" => A::Inverser,
                    "masquer" => A::Masquer,
                    "masquer_vue" => A::MasquerVue,
                    "isoler" => A::Isoler,
                    g if g.starts_with("garder ") => match &g[7..] {
                        "murs" => A::Garder("Murs"), "dalles" => A::Garder("Dalles"), "ouvertures" => A::Garder("Ouvertures"), "structures" => A::Garder("Structures"),
                        _ => return erreur("garder murs|dalles|ouvertures|structures"),
                    },
                    _ => return erreur("usage : menu repeter|identiques_vue|identiques_projet|garder <catégorie>|verrouiller|deverrouiller|demolir|inverser|masquer|masquer_vue|isoler"),
                };
                self.error = None;
                self.action_menu(a);
                match &self.error { Some(e) => erreur(e), None => self.etat() }
            }
            "transformation" => match serde_json::from_str::<serde_json::Value>(argument).map_err(|e| e.to_string()).and_then(|v| self.regler_transformation(&v)) {
                Ok(()) => succes(&[("transformation", self.etat_transformation().to_string())]),
                Err(e) => erreur(&e),
            },
            "editer" => {
                let resultat=serde_json::from_str::<crate::modele::edition::commandes::Commande>(argument).map_err(|e|e.to_string()).and_then(|c|self.executer_bim(c));
                match resultat { Ok(ids)=>succes(&[("ids",serde_json::to_string(&ids).unwrap())]), Err(e)=>erreur(&e) }
            }
            "annuler" | "retablir" => {
                self.error=None; self.parcourir_historique(commande=="retablir");
                match &self.error { Some(e)=>erreur(e),None=>self.etat() }
            }
            "projet_natif" => match &self.natif {Some((p,_))=>match p.json() {Ok(s)=>succes(&[("empreinte",texte(&crate::modele::collaboration::hash(&serde_json::to_value(p).unwrap()))),("projet",s)]),Err(e)=>erreur(&e)},None=>erreur("Aucun projet natif.")},
            "vue_plan" => {
                // vue_plan <nom ou id du niveau> [hauteur de coupe mm]
                let (nom, h) = argument.rsplit_once(' ').and_then(|(n, h)| Some((n, h.parse::<f64>().ok()?))).unwrap_or((argument, 1200.0));
                let Some(niveau) = self.natif.as_ref().and_then(|(p, _)| p.niveaux.iter().find(|n| n.nom == nom.trim() || n.id == nom.trim())).map(|n| n.id.clone()) else { return erreur("niveau introuvable") };
                self.creer_vue(|p| p.ajouter_vue_plan(&niveau, h), true);
                match &self.error { Some(e) => erreur(e), None => self.etat() }
            }
            "coupe_vue" => {
                // coupe_vue <x0 y0 x1 y1> [profondeur mm] [elevation] (mm)
                let elevation = argument.contains("elevation");
                let Ok(v) = argument.split_whitespace().filter(|m| *m != "elevation").map(str::parse::<f64>).collect::<Result<Vec<_>, _>>() else { return erreur("usage : coupe_vue x0 y0 x1 y1 [profondeur] [elevation]") };
                if v.len() != 4 && v.len() != 5 { return erreur("usage : coupe_vue x0 y0 x1 y1 [profondeur] [elevation]") }
                let p = v.get(4).copied().unwrap_or(if elevation { 30_000.0 } else { 15_000.0 });
                let (nom, echelle) = if elevation { ("Élévation extérieure", 100.0) } else { ("Coupe de bâtiment", 100.0) };
                self.creer_vue(|projet| projet.ajouter_vue_coupe([v[0], v[1]], [v[2], v[3]], p, elevation, nom, echelle), false);
                match &self.error { Some(e) => erreur(e), None => self.etat() }
            }
            "dalle" => {
                // dalle <x y> <x y> <x y> … (mm) : type, niveau et décalage de l'outil Dalle / Sol
                let Ok(v) = argument.split_whitespace().map(str::parse::<f64>).collect::<Result<Vec<_>, _>>() else { return erreur("coordonnées de la dalle invalides") };
                if v.len() < 6 || v.len() % 2 != 0 { return erreur("usage : dalle <x0 y0> <x1 y1> <x2 y2> … (mm)") }
                let pts: Vec<[f64; 2]> = v.chunks(2).map(|c| [c[0], c[1]]).collect();
                self.onglet_ruban = OngletRuban::Modeliser;
                let Some(parametres) = self.ruban_edition.dalle_active().or_else(|| { self.ruban_edition.activer_dalle(); self.ruban_edition.dalle_active() }) else { return erreur("outil Dalle indisponible") };
                self.creer_dalle(&pts, &parametres);
                match &self.error { Some(e) => erreur(e), None => self.etat() }
            }
            "surface" => {
                // surface type <nom> | surface pente oui|non | surface <paramètre> <valeur> (outil actif)
                let (cle, reste) = argument.split_once(' ').map_or((argument, ""), |(a, b)| (a, b.trim()));
                let r = match cle {
                    "type" => self.ruban_edition.choisir_type_surface(reste),
                    "pente" => { self.ruban_edition.definir_pente_toiture(reste != "non"); Ok(()) }
                    _ => match reste.parse::<f64>() {
                        Ok(v) => self.ruban_edition.regler_surface(cle, v),
                        Err(_) => Err("usage : surface type <nom> | surface pente oui|non | surface <paramètre> <valeur>".into()),
                    },
                };
                match r { Ok(()) => self.etat(), Err(e) => erreur(&e) }
            }
            "modifier_trace" => {
                // modifier_trace <id natif> : Modifier tracé (contour, polyligne, terrain)
                let Some((projet, _)) = &self.natif else { return erreur("pas de projet natif ouvert") };
                if !projet.identifiants().contains(argument.trim()) { return erreur("élément introuvable") }
                self.select(None, false);
                self.bim.selection = vec![argument.trim().to_string()];
                self.commencer_edition_contour();
                self.etat()
            }
            "pente" => {
                // pente <id|dernier> <arête 1…n|toutes> <degrés|aucune>
                let v: Vec<&str> = argument.split_whitespace().collect();
                let [cible, arete, valeur] = v[..] else { return erreur("usage : pente <id|dernier> <arête|toutes> <degrés|aucune>") };
                let Some((projet, _)) = &self.natif else { return erreur("pas de projet natif ouvert") };
                let mut projet = projet.clone();
                let Some(d) = (if cible == "dernier" { projet.dalles.last_mut() } else { projet.dalles.iter_mut().find(|d| d.id == cible) }) else { return erreur("élément horizontal introuvable") };
                let pente = if valeur == "aucune" { None } else {
                    match valeur.parse::<f64>() { Ok(a) if a > 0.0 && a < 89.0 => Some(a), _ => return erreur("pente : degrés entre 0 et 89, ou « aucune »") }
                };
                let n = d.contours.first().map_or(0, |c| c.len());
                let aretes: Vec<usize> = if arete == "toutes" { (0..n).collect() } else {
                    match arete.parse::<usize>() { Ok(k) if k >= 1 && k <= n => vec![k - 1], _ => return erreur("numéro d'arête invalide") }
                };
                for k in aretes { d.definir_pente(0, k, pente); }
                match self.appliquer_projet(projet) { Ok(()) => self.etat(), Err(e) => erreur(&e) }
            }
            "type_mur" => {
                // type_mur <nom>|<matériau>:<épaisseur mm>[:structure|isolation|finition]|… (extérieur → intérieur)
                use crate::modele::{Couche, FonctionCouche};
                let mut parties = argument.split('|');
                let nom = parties.next().unwrap_or("").trim().to_string();
                let couches: Option<Vec<Couche>> = parties.map(|c| {
                    let mut x = c.split(':');
                    let materiau = x.next()?.trim().to_string();
                    let epaisseur = x.next()?.trim().parse().ok()?;
                    let fonction = match x.next().map(str::trim) {
                        Some("structure") => FonctionCouche::Structure,
                        Some("isolation") => FonctionCouche::Isolation,
                        Some("finition") => FonctionCouche::Finition,
                        _ => FonctionCouche::Autre,
                    };
                    Some(Couche { materiau: Some(materiau), epaisseur, fonction, plume: None, retour: false })
                }).collect();
                let (Some(couches), false) = (couches, nom.is_empty()) else { return erreur("usage : type_mur <nom>|<matériau>:<épaisseur>[:fonction]|…") };
                let Some((projet, _)) = &self.natif else { return erreur("pas de projet natif ouvert") };
                let mut projet = projet.clone();
                match self.ruban_edition.definir_type_mur(&mut projet, &nom, couches, None) {
                    Ok(id) => match self.appliquer_projet(projet) {
                        Ok(()) => succes(&[("type", texte(&id))]),
                        Err(e) => erreur(&e),
                    },
                    Err(e) => erreur(&e),
                }
            }
            "mur" => {
                let Ok(v) = argument.split_whitespace().map(str::parse::<f64>).collect::<Result<Vec<_>, _>>() else { return erreur("coordonnées du mur invalides") };
                if v.len() != 4 && v.len() != 6 { return erreur("usage : mur <x0 mm> <y0 mm> <x1 mm> <y1 mm> [hauteur mm epaisseur mm]") }
                let mut options = self.ruban_edition.parametres_mur();
                if v.len() == 6 { options.hauteur = v[4]; options.epaisseur = v[5]; }
                match self.ajouter_mur_natif([v[0],v[1]],[v[2],v[3]],&options) {
                    Ok(id) => succes(&[("mur",texte(&id))]),
                    Err(e) => erreur(&e),
                }
            }
            "enregistrer" => {
                if self.natif.is_none() && self.rvt_source.is_some() {
                    let sortie = if argument.trim().is_empty() {
                        let Some(p) = self.rvt_sortie.clone() else { return erreur("usage : enregistrer <chemin.modele.json> pour le premier enregistrement d’un RVT") };
                        p
                    } else { PathBuf::from(argument.trim().trim_matches('"')) };
                    return match self.demarrer_enregistrement_rvt(ctx,sortie) {
                        Ok(()) => self.etat(), Err(e) => erreur(&e),
                    };
                }
                if self.natif.is_none() { return erreur("aucun projet natif ou Revit ouvert") }
                if !argument.trim().is_empty() {
                    let chemin=PathBuf::from(argument.trim().trim_matches('"'));
                    let (p,ancien)=self.natif.as_ref().unwrap();
                    if self.collaboration.is_some() && &chemin!=ancien{return erreur("La copie collaborative conserve son chemin local");}
                    if let Err(e)=crate::modele::echanges::enregistrement::enregistrer_natif(p,&chemin){return erreur(&e);}
                    crate::modele::echanges::enregistrement::effacer_recuperation(ancien);
                    let _=self.historique.enregistrer(p);
                    self.finaliser_enregistrement_sous(chemin);
                } else {self.enregistrer_projet(&ctx);}
                if let Some(e) = &self.error { erreur(e) } else { self.etat() }
            }
            "taille" => {
                let v: Vec<f32> = argument.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let [l, h] = v[..] else { return erreur("usage : taille <largeur> <hauteur>") };
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(l, h)));
                succes(&[])
            }
            "reduire" | "restaurer" => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(commande == "reduire"));
                succes(&[])
            }
            "quitter" => {
                if let Err(e)=self.pilote_disponible() { return erreur(&e); }
                if !argument.is_empty() && argument!="abandonner" { return erreur("usage : quitter [abandonner]"); }
                if self.natif_modifie || self.documents.values().any(|d|d.modifie) {
                    if argument!="abandonner" { return erreur("Modifications non enregistrées : enregistrer les documents ou quitter abandonner"); }
                    if let Some((_,p))=&self.natif {crate::modele::echanges::enregistrement::effacer_recuperation(p);}
                    self.natif_modifie=false;
                    for d in self.documents.values_mut() {
                        if let Some((_,p))=&d.natif {crate::modele::echanges::enregistrement::effacer_recuperation(p);}
                        d.modifie=false;
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                succes(&[])
            }
            "fiche" => {
                self.rvt_sheet = self.rvt.is_some() && argument != "non";
                self.etat()
            }
            "onglet" => {
                let a = if argument == "projet" { "tout" } else { argument };
                let Some(&o) = LeftTab::REVIT.iter().find(|o| o.nom() == a) else {
                    return erreur("usage : onglet tout|vues|feuilles|nomenclatures|familles|materiaux|groupes|liens|modele|classes");
                };
                self.left_tab = o;
                self.etat()
            }
            "feuilles" => {
                let Some((p, _)) = &self.natif else { return erreur("aucun projet natif") };
                let jeux: Vec<_> = crate::modele::documentation::jeux(p).into_iter().map(|nom| {
                    let feuilles: Vec<_> = p.feuilles.iter().filter(|f| f.jeu == nom).collect();
                    serde_json::json!({"nom": nom, "feuilles": feuilles})
                }).collect();
                serde_json::json!({"ok":true, "jeux":jeux}).to_string()
            }
            "selection_feuille" => {
                let Ok(a) = serde_json::from_str::<serde_json::Value>(argument) else { return erreur("usage : selection_feuille {\"jeu\":\"nom\",\"feuille\":\"id\",\"vue\":0} ; feuille et vue facultatives") };
                let Some((p, _)) = &self.natif else { return erreur("aucun projet natif") };
                let jeu = a.get("jeu").and_then(|v| v.as_str()).map(str::to_string).or_else(|| self.jeu_actif());
                let Some(jeu) = jeu else { return erreur("indiquer le nom du jeu") };
                if !crate::modele::documentation::jeux(p).contains(&jeu) { return erreur("jeu introuvable") }
                let fid = a.get("feuille").and_then(|v| v.as_str()).map(str::to_string);
                let vue = if let Some(v) = a.get("vue") {
                    let Some(i) = v.as_u64().and_then(|n| usize::try_from(n).ok()) else { return erreur("vue doit être un indice entier positif") };
                    Some(i)
                } else { None };
                if let Some(fid) = &fid {
                    let Some(f) = p.feuilles.iter().find(|f| &f.id == fid && f.jeu == jeu) else { return erreur("feuille introuvable dans ce jeu") };
                    if vue.is_some_and(|i| i >= f.vues.len() + f.nomenclatures.len()) { return erreur("indice de vue ou nomenclature placée invalide") }
                } else if vue.is_some() { return erreur("indiquer la feuille contenant la vue") }
                self.ouvrir_jeu(&jeu, fid.clone());
                if let (Some(fid), Some(i)) = (fid, vue) { self.feuilles.selection = feuilles::SelectionFeuille::Vue(fid, i); }
                self.commande(ctx, "proprietes_feuille", "")
            }
            "proprietes_feuille" => {
                let Some(jeu) = self.jeu_actif() else { return erreur("aucun jeu de feuilles ouvert") };
                let Some((p, _)) = &self.natif else { return erreur("aucun projet natif") };
                let selection = match &self.feuilles.selection {
                    feuilles::SelectionFeuille::Aucune => serde_json::json!({"type":"jeu", "nom":jeu, "nombre_feuilles":p.feuilles.iter().filter(|f| f.jeu == jeu).count()}),
                    feuilles::SelectionFeuille::Feuille(id) => match p.feuilles.iter().find(|f| &f.id == id && f.jeu == jeu) {
                        Some(f) => serde_json::json!({"type":"feuille", "feuille":f, "famille_cartouche": p.cartouches_feuilles.get(id).map_or("Cartouche standard", String::as_str)}),
                        None => return erreur("feuille sélectionnée introuvable"),
                    },
                    feuilles::SelectionFeuille::Vue(id, i) => match p.feuilles.iter().find(|f| &f.id == id && f.jeu == jeu) {
                        Some(f) if *i < f.vues.len() => serde_json::json!({"type":"vue", "feuille":id, "indice":i, "proprietes":f.vues[*i]}),
                        Some(f) if *i < f.vues.len() + f.nomenclatures.len() => serde_json::json!({"type":"nomenclature", "feuille":id, "indice":i, "proprietes":f.nomenclatures[*i - f.vues.len()]}),
                        _ => return erreur("élément sélectionné introuvable"),
                    },
                };
                serde_json::json!({"ok":true, "jeu":jeu, "selection":selection}).to_string()
            }
            "vues" => {
                let Some(nav) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()) else { return erreur("pas de vues Revit") };
                let vues = nav.vues.iter().enumerate().map(|(i, v)| {
                    objet(&[("indice", i.to_string()), ("genre", texte(&v.genre)), ("type", texte(&v.type_vue)), ("nom", texte(&v.nom)), ("echelle", v.echelle.to_string()), ("numero", texte(&v.numero))])
                });
                succes(&[("vues", liste(vues))])
            }
            "vue" => {
                let Some(nav) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()) else { return erreur("pas de vues Revit") };
                let a = argument.to_lowercase();
                let nom = |v: &scene::NavVue| format!("{} : {}", v.genre, v.nom).to_lowercase();
                let i = argument
                    .parse::<usize>()
                    .ok()
                    .filter(|&i| i < nav.vues.len())
                    .or_else(|| nav.vues.iter().position(|v| v.nom.to_lowercase() == a || nom(v) == a))
                    .or_else(|| nav.vues.iter().position(|v| nom(v).contains(&a)));
                let Some(i) = i else { return erreur(&format!("vue introuvable : {argument}")) };
                self.rvt_sheet = false;
                self.activer_vue(i);
                self.etat()
            }
            "zone" => {
                // zone on|off|ajuster, ou zone <xmin> <ymin> <zmin> <xmax> <ymax> <zmax> (m, repère du fichier)
                let v: Vec<f64> = argument.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                match (argument.trim(), &v[..]) {
                    ("on", _) => self.activer_boite(true),
                    ("off", _) => self.activer_boite(false),
                    ("ajuster", _) => {
                        self.boite = None;
                        self.activer_boite(true);
                    }
                    (_, &[x0, y0, z0, x1, y1, z1]) => {
                        let o = self.scene.as_ref().map_or(glam::DVec3::ZERO, |s| s.origin);
                        self.boite_active = true;
                        self.regler_boite((glam::DVec3::new(x0, y0, z0) - o).as_vec3(), (glam::DVec3::new(x1, y1, z1) - o).as_vec3());
                    }
                    _ => return erreur("usage : zone on|off|ajuster, ou zone <xmin> <ymin> <zmin> <xmax> <ymax> <zmax>"),
                }
                self.etat()
            }
            "camera" => {
                // vue 2D : orientation et projection fixées par la vue Revit
                if !self.vue_3d() && !matches!(argument, "cadrer" | "ortho") {
                    return erreur("vue 2D : orientation fixe (cadrer seulement ; tout_afficher revient au modèle 3D)");
                }
                let c = &mut self.camera;
                match argument {
                    "dessus" => {
                        c.pitch = MAX_PITCH;
                        c.yaw = -std::f32::consts::FRAC_PI_2;
                    }
                    "face" => {
                        c.pitch = 0.0;
                        c.yaw = -std::f32::consts::FRAC_PI_2;
                    }
                    "iso" => {
                        c.pitch = 0.6;
                        c.yaw = -2.3;
                    }
                    "ortho" => c.ortho = true,
                    "perspective" => c.ortho = false,
                    "cadrer" => self.fit_all(),
                    _ => {
                        let v: Vec<f32> = argument.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                        if v.len() < 2 {
                            return erreur("usage : camera dessus|face|iso|cadrer|ortho|perspective, ou camera <lacet°> <tangage°> [distance]");
                        }
                        c.yaw = v[0].to_radians();
                        c.pitch = v[1].to_radians().clamp(-MAX_PITCH, MAX_PITCH);
                        if let Some(&d) = v.get(2) {
                            c.dist = d.max(1e-3);
                        }
                    }
                }
                self.etat()
            }
            "traits_fins" => {
                // traits_fins oui|non
                self.traits_fins = argument != "non";
                self.coupe_dirty = true;
                self.etat()
            }
            "dessin" => {
                // dessin oui : graphique au trait (faces blanches) ; non : faces colorées
                self.style = StyleVisuel::Graphique;
                self.faces_blanches = argument != "non";
                self.visibility_dirty = true;
                self.etat()
            }
            "ombres" => {
                // ombres portees|ambiante oui|non
                let (quoi, valeur) = argument.split_once(' ').unwrap_or((argument, "oui"));
                let oui = valeur.trim() != "non";
                match quoi {
                    "portees" | "portées" => self.ombres_portees = oui,
                    "ambiante" => self.ombre_ambiante = oui,
                    _ => return erreur("usage : ombres portees|ambiante oui|non"),
                }
                self.etat()
            }
            "style" => {
                // style graphique|realiste ; anciens noms : lignes_cachees, ombre, couleurs, rendu
                let (style, blanches, eclairage, ombres) = match argument {
                    "graphique" | "ombre" => (StyleVisuel::Graphique, false, true, None),
                    "lignes_cachees" => (StyleVisuel::Graphique, true, true, None),
                    "couleurs" => (StyleVisuel::Graphique, false, false, None),
                    "realiste" => (StyleVisuel::Realiste, false, true, None),
                    "rendu" => (StyleVisuel::Realiste, false, true, Some(true)),
                    _ => return erreur("usage : style graphique|realiste (lignes_cachees, couleurs, rendu)"),
                };
                self.style = style;
                self.faces_blanches = blanches;
                self.eclairage = eclairage;
                if let Some(o) = ombres {
                    self.ombres_portees = o;
                    self.ombre_ambiante = o;
                }
                self.visibility_dirty = true;
                self.coupe_dirty = true;
                self.etat()
            }
            "coupe" => {
                // coupe non | coupe <hauteur m> [niveau : id ou nom]
                if argument == "non" {
                    self.coupe = false;
                } else {
                    let (h, niveau) = argument.split_once(' ').unwrap_or((argument, ""));
                    if !h.is_empty() {
                        let Ok(h) = h.replace(',', ".").parse::<f32>() else { return erreur("usage : coupe non | coupe <hauteur m> [niveau]") };
                        self.coupe_hauteur = h;
                    }
                    let niveau = niveau.trim().trim_start_matches('#').to_lowercase();
                    if !niveau.is_empty() {
                        let n = self.niveaux().into_iter().find(|x| x.0.to_string() == niveau || x.1.to_lowercase() == niveau || x.1.to_lowercase().contains(&niveau));
                        let Some(n) = n else { return erreur(&format!("niveau introuvable : {niveau}")) };
                        self.coupe_niveau = Some(n.0);
                    }
                    self.style = StyleVisuel::Graphique;
                    self.faces_blanches = true;
                    self.coupe = true;
                }
                self.visibility_dirty = true;
                self.etat()
            }
            "exporter" => {
                // exporter <chemin.svg> : dessin vectoriel de la vue
                let chemin = argument.trim().trim_matches('"');
                if !chemin.to_lowercase().ends_with(".svg") {
                    return erreur("usage : exporter <chemin.svg>");
                }
                match self.exporter_svg(std::path::Path::new(chemin)) {
                    Ok(n) => succes(&[("chemin", texte(chemin)), ("primitives", n.to_string())]),
                    Err(e) => erreur(&e),
                }
            }
            "exporter_jeu" => {
                // exporter_jeu <chemin.pdf> [jeu] : jeu ouvert, sinon le premier
                let (chemin, jeu) = argument.trim().split_once(' ').map_or((argument.trim(), None), |(c, j)| (c, Some(j.trim().to_string())));
                let jeu = jeu.or_else(|| self.jeu_actif()).or_else(|| self.natif.as_ref().and_then(|(p, _)| crate::modele::documentation::jeux(p).into_iter().next()));
                let Some(jeu) = jeu else { return erreur("aucun jeu de feuilles") };
                if !chemin.to_lowercase().ends_with(".pdf") {
                    return erreur("usage : exporter_jeu <chemin.pdf> [jeu]");
                }
                match self.exporter_jeu(&jeu, std::path::Path::new(chemin)) {
                    Ok(n) => succes(&[("chemin", texte(chemin)), ("jeu", texte(&jeu)), ("pages", n.to_string())]),
                    Err(e) => erreur(&e),
                }
            }
            "niveaux" => {
                let n = self.niveaux().into_iter().map(|(id, nom, z)| objet(&[("id", id.to_string()), ("nom", texte(&nom)), ("z", format!("{z:.3}"))]));
                succes(&[("niveaux", liste(n))])
            }
            "poche" => {
                let Some(p) = Poche::depuis(argument) else { return erreur("usage : poche hachure|plein|vide") };
                self.poche = p;
                self.visibility_dirty = true;
                self.etat()
            }
            "echelle" => {
                let Some(e) = argument.trim_start_matches("1:").parse::<f64>().ok().filter(|e| *e > 0.0) else { return erreur("usage : echelle <dénominateur> (100 pour 1:100)") };
                self.echelle = e;
                self.visibility_dirty = true;
                self.etat()
            }
            "panneau" => {
                // panneau proprietes|arborescence gauche|droite|flottant|externe
                let (p, a) = argument.split_once(' ').unwrap_or((argument, ""));
                let p = Panneau::TOUS.into_iter().find(|x| x.nom() == p);
                if a.trim()=="fermer" {
                    let Some(p)=p else{return erreur("Volet inconnu")};
                    if self.ancrages[p as usize]!=Ancrage::Externe {return erreur("Le volet n'est pas une fenêtre externe")}
                    ctx.send_viewport_cmd_to(egui::ViewportId::from_hash_of(("volet_externe",p.nom())),egui::ViewportCommand::Close);
                    return self.etat();
                }
                let a = Ancrage::TOUS.into_iter().find(|x| x.nom() == a.trim());
                let (Some(p), Some(a)) = (p, a) else { return erreur("usage : panneau proprietes|arborescence gauche|droite|flottant|externe") };
                self.ancrer(p, a);
                self.etat()
            }
            "visibilite" => {
                // visibilite oui|non : boîte ; visibilite <catégorie> oui|non
                let (nom, oui) = match argument.rsplit_once(' ') {
                    Some((n, o)) if o == "oui" || o == "non" => (n.trim(), o == "oui"),
                    _ => ("", argument != "non"),
                };
                if nom.is_empty() {
                    self.voir_visibilite = oui;
                    return self.etat();
                }
                let n = nom.to_lowercase();
                let k = self.categories.iter().position(|c| c.nom.to_lowercase() == n).or_else(|| self.categories.iter().position(|c| c.nom.to_lowercase().contains(&n)));
                let Some(k) = k else { return erreur(&format!("catégorie introuvable : {nom}")) };
                self.montrer_categorie(k, oui);
                self.update_visibility();
                self.etat()
            }
            "categories" => {
                let mut nombres = vec![0usize; self.categories.len()];
                for &k in &self.categorie_objet {
                    nombres[k] += 1;
                }
                let c = self.categories.iter().zip(nombres).map(|(c, n)| {
                    objet(&[("nom", texte(&c.nom)), ("genre", texte(if c.annotation { "annotation" } else { "modele" })), ("objets", n.to_string()), ("visible", self.categorie_visible(c).to_string())])
                });
                succes(&[("categories", liste(c))])
            }
            "glisser" => {
                // glisser <dx> <dy> [gauche|droit|milieu] [maj] : comme à la souris (pixels)
                let mots: Vec<&str> = argument.split_whitespace().collect();
                let (Some(Ok(dx)), Some(Ok(dy))) = (mots.first().map(|x| x.parse::<f32>()), mots.get(1).map(|x| x.parse::<f32>())) else {
                    return erreur("usage : glisser <dx> <dy> [gauche|droit|milieu] [maj]");
                };
                let bouton = match mots.get(2).copied() {
                    Some("droit") => PointerButton::Secondary,
                    Some("milieu") => PointerButton::Middle,
                    _ => PointerButton::Primary,
                };
                self.glisser(bouton, egui::vec2(dx, dy), mots.contains(&"maj"), self.rect_vue.height());
                self.etat()
            }
            "zoom" => {
                let Ok(f) = argument.parse::<f32>() else { return erreur("usage : zoom <facteur> (2 = deux fois plus près)") };
                self.camera.zoom(1.0 / f.max(1e-3), None);
                self.etat()
            }
            "afficher" | "masquer" => {
                for c in argument.split([' ', ',']).filter(|c| !c.is_empty()) {
                    let c = c.to_uppercase();
                    if commande == "afficher" {
                        self.hidden_classes.remove(&c);
                    } else {
                        self.hidden_classes.insert(c);
                    }
                }
                self.visibility_dirty = true;
                self.etat()
            }
            "tout_afficher" => {
                self.memoriser_vue();
                self.hidden_objects.clear();
        self.masques_utilisateur.clear();
                self.hidden_storeys.clear();
                self.isolated = None;
                self.vue_active = None;
                self.nomenclature_ouverte = None;
                if !self.vues_ouvertes.contains(&None) {
                    self.vues_ouvertes.push(None);
                }
                self.memoriser_vue();
                self.visibility_dirty = true;
                self.etat()
            }
            "classes" => {
                let Some(s) = &self.scene else { return erreur("aucun fichier ouvert") };
                let mut n: BTreeMap<&str, usize> = BTreeMap::new();
                for o in &s.objects {
                    *n.entry(&*o.class).or_default() += 1;
                }
                let classes = n.into_iter().map(|(c, k)| objet(&[("classe", texte(c)), ("objets", k.to_string()), ("visible", (!self.hidden_classes.contains(c)).to_string())]));
                succes(&[("classes", liste(classes))])
            }
            "elements" => {
                // elements [CLASSE|*] [texte du nom]
                let Some(s) = &self.scene else { return erreur("aucun fichier ouvert") };
                let (classe, texte_nom) = argument.split_once(' ').unwrap_or((argument, ""));
                let (classe, texte_nom) = (classe.to_uppercase(), texte_nom.to_lowercase());
                let mut out = Vec::new();
                for o in &s.objects {
                    if (!classe.is_empty() && classe != "*" && *o.class != *classe) || (!texte_nom.is_empty() && !s.model.name(o.id).to_lowercase().contains(&texte_nom)) {
                        continue;
                    }
                    out.push(self.decrire(o.id));
                    if out.len() == 300 {
                        break;
                    }
                }
                succes(&[("elements", liste(out))])
            }
            // origine du projet : ses coordonnées se saisissent à côté d'elle
            "selectionner" if argument.trim() == "origine" => {
                self.select(None, false);
                self.bim.origine = true;
                self.etat()
            }
            "selectionner" => match self.entite(argument) {
                Ok(id) => {
                    self.select(Some(id), false);
                    self.etat()
                }
                Err(e) => erreur(&e),
            },
            "isoler" | "cadrer" => match self.entite(argument) {
                Ok(id) => {
                    let o = self.objects_under(id);
                    if o.is_empty() {
                        return erreur("pas de géométrie");
                    }
                    if commande == "isoler" {
                        self.isolate(o.clone());
                    }
                    self.frame_objects(&o);
                    self.etat()
                }
                Err(e) => erreur(&e),
            },
            "proprietes" => match self.entite(argument) {
                Ok(id) => {
                    let m = &self.scene.as_ref().unwrap().model;
                    let paires = |v: Vec<(String, String)>| objet(&v.iter().map(|(k, v)| (k.as_str(), texte(v))).collect::<Vec<_>>());
                    let groupes: Vec<String> = m.property_groups(id).into_iter().map(|g| objet(&[("nom", texte(&g.name)), ("proprietes", paires(g.props))])).collect();
                    succes(&[("entite", self.decrire(id)), ("attributs", paires(m.attributes(id))), ("groupes", liste(groupes))])
                }
                Err(e) => erreur(&e),
            },
            "objet" => {
                // objet x y : position dans la vue, de 0 à 1 depuis le coin haut gauche
                let v: Vec<f32> = argument.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let (&[x, y], Some(s)) = (&v[..], &self.scene) else { return erreur("usage : objet <x 0..1> <y 0..1>") };
                let aspect = self.rect_vue.width() / self.rect_vue.height().max(1.0);
                let (o, d) = self.camera.ray(x * 2.0 - 1.0, 1.0 - y * 2.0, aspect);
                match s.pick(o, d, &self.visible, self.plan_coupe(), self.boite_appliquee()) {
                    Some((obj, t)) => {
                        let p = o + d * t;
                        let f = p.as_dvec3() + s.origin;
                        let mut r = vec![("entite", self.decrire(s.objects[obj as usize].id)), ("point", format!("[{:.3},{:.3},{:.3}]", f.x, f.y, f.z))];
                        if let Some(g) = &s.partage {
                            let q = g.partage(f);
                            r.push(("partage", format!("[{:.3},{:.3},{:.3}]", q.x, q.y, q.z)));
                        }
                        succes(&r)
                    }
                    None => succes(&[("entite", "null".into())]),
                }
            }
            _ => erreur(&format!("commande inconnue : {commande}")),
        }
    }
}
