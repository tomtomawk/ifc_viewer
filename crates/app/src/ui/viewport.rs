//! Zone de vue : interaction souris, rendu, superpositions.
use super::*;

impl ViewerApp {
    pub(super) fn viewport(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        // fenêtre réduite : on garde la dernière zone, taille des captures hors écran
        if rect.is_positive() {
            self.rect_vue = rect;
        }
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        if self.scene.is_none() {
            ui.painter().rect_filled(rect,0.0,ui.visuals().panel_fill);
            ui.painter().text(rect.center(),egui::Align2::CENTER_CENTER,"Ouvrir ou créer un projet",egui::FontId::proportional(16.0),ui.visuals().weak_text_color());
            return;
        }
        let aspect = rect.width() / rect.height().max(1.0);
        let mode_ajuster=self.preparer_outil_ajuster();
        let mode_attache = self.onglet_ruban == OngletRuban::Modifier && self.ruban_edition.outil() == Some(crate::ui::ruban::Outil::AttacherToiture) && self.natif.is_some() && self.loading.is_none();
        if mode_attache && !self.bim.attache_toiture_active {
            let selection = self.selection_bim();
            let projet = &self.natif.as_ref().unwrap().0;
            self.bim.murs_a_attacher = selection.into_iter().filter_map(|id| {
                if projet.assemblage(&id).is_some() { Some(id) }
                else { projet.murs.iter().find(|m| m.id == id).map(|m| m.assemblage.clone().unwrap_or(id)) }
            }).collect();
            self.bim.murs_a_attacher.sort();
            self.bim.murs_a_attacher.dedup();
        } else if !mode_attache && self.bim.attache_toiture_active {
            self.bim.murs_a_attacher.clear();
        }
        self.bim.attache_toiture_active = mode_attache;
        let mode_mur = self.onglet_ruban == OngletRuban::Modeliser && self.ruban_edition.mur_actif() && self.natif.is_some() && self.loading.is_none();
        // dalle (ou sol) : contour cliqué sur le plan de son dessus
        let dalle = if matches!(self.onglet_ruban, OngletRuban::Modeliser | OngletRuban::Modifier) && self.natif.is_some() && self.loading.is_none() { self.ruban_edition.dalle_active() } else { None };
        // coupe : ligne à deux clics, sur le plan du niveau de la vue ouverte
        let outil_coupe = if self.onglet_ruban == OngletRuban::Annoter && self.natif.is_some() && self.loading.is_none() { self.ruban_edition.coupe_active() } else { None };
        // une coupe se trace dans une vue en plan (ou une vue de dessus)
        let coupe_hors_plan = outil_coupe.is_some() && !self.vue_en_plan();
        let outil_coupe = outil_coupe.filter(|_| !coupe_hors_plan);
        if coupe_hors_plan {
            ui.painter().text(rect.left_top() + egui::vec2(12.0, 34.0), egui::Align2::LEFT_TOP, "Coupe / élévation : ouvrir une vue en plan pour tracer la ligne", egui::FontId::proportional(13.0), Color32::from_rgb(55, 130, 200));
        }
        if outil_coupe.is_none() { self.coupe_depart = None; }
        let altitude_coupe = self.altitude_vue_native();
        let structure = if self.onglet_ruban == OngletRuban::Modeliser && self.natif.is_some() && self.loading.is_none() {
            match self.ruban_edition.outil() {
                Some(crate::ui::ruban::Outil::Poteau) => Some(crate::modele::GenreBarre::Poteau),
                Some(crate::ui::ruban::Outil::Poutre) => Some(crate::modele::GenreBarre::Poutre),
                _ => None,
            }
        } else { None };
        if structure != Some(crate::modele::GenreBarre::Poutre) { self.bim.depart_structure = None; }
        // Composant (vrai) ou Pièce (faux) : un clic place l'élément au niveau de la vue
        let placement = if self.onglet_ruban == OngletRuban::Modeliser && self.natif.is_some() && self.loading.is_none() {
            match self.ruban_edition.outil() {
                Some(crate::ui::ruban::Outil::Composant) => Some(true),
                Some(crate::ui::ruban::Outil::Piece) | Some(crate::ui::ruban::Outil::Arbre) => Some(false),
                _ => None,
            }
        } else { None };
        // arbre : placé comme une pièce (point au niveau de la vue)
        let arbre = placement == Some(false) && self.ruban_edition.outil() == Some(crate::ui::ruban::Outil::Arbre);
        let altitude_placement = self.altitude_vue_native() + if placement == Some(true) { self.bim.composant.decalage } else if arbre { self.bim.arbre.decalage } else { 0.0 };
        let altitude_structure = structure.and_then(|_| {
            let p = &self.natif.as_ref()?.0;
            Some(p.niveau(&self.bim.barre.niveau)?.altitude + self.bim.barre.debut[2])
        });
        // Texte : un clic le place dans la vue native, comme dans l'éditeur de famille
        let texte_annotation = self.onglet_ruban == OngletRuban::Annoter && self.natif.is_some() && self.loading.is_none()
            && self.ruban_edition.outil() == Some(crate::ui::ruban::Outil::Texte) && self.vue_native_id().is_some();
        // Déplacer, Copier, Tourner, Aligner : socle commun (transformer.rs)
        self.suivre_transformation();
        let deplacement = self.transfo.outil;
        let mode_trace = mode_attache || mode_ajuster || mode_mur || dalle.is_some() || outil_coupe.is_some() || structure.is_some() || placement.is_some() || texte_annotation || deplacement.is_some() || self.outil_dessin().is_some() || self.dessin2d.edition_contour.is_some() || self.alignement_plan || self.orientation_nord_en_cours;
        // axe du mur sélectionné : une extrémité saisie se déplace (accrochages du tracé)
        let edition_arc=self.souris_arc_mur(ui,rect,&resp,mode_trace);
        let axe = if mode_trace || edition_arc { None } else { self.axe_selection() };
        if let Some((id, a, b, z, _)) = &axe {
            let (ea, eb) = (self.ecran_plan(rect, *a, *z), self.ecran_plan(rect, *b, *z));
            if resp.drag_started_by(PointerButton::Primary)
                && let Some(q) = ui.input(|i| i.pointer.press_origin())
            {
                let proche = |e: Option<egui::Pos2>| e.is_some_and(|e| e.distance(q) <= 9.0);
                if proche(eb) {
                    self.cote_axe = None;
                    self.dessin2d.saisie.clear();
                    self.poignee_axe = Some((id.clone(), true, *b));
                } else if proche(ea) {
                    self.cote_axe = None;
                    self.dessin2d.saisie.clear();
                    self.poignee_axe = Some((id.clone(), false, *a));
                }
            }
            if let (Some((_, fin, _)), Some(q)) = (&self.poignee_axe, resp.interact_pointer_pos().or(resp.hover_pos())) {
                let fixe = if *fin { *a } else { *b };
                let shift = ui.input(|i| i.modifiers.shift);
                let origine = if *fin { *b } else { *a };
                if let Some(p) = self.point_edition_axe(ui,rect,q,shift,fixe,origine)
                    && let Some(x) = &mut self.poignee_axe
                {
                    x.2 = p;
                }
            }
        } else {
            self.poignee_axe = None;
        }
        if self.poignee_axe.is_some() && (ui.input(|i|i.key_pressed(egui::Key::Escape))||resp.clicked_by(PointerButton::Secondary)) {
            self.poignee_axe=None;
            self.terminer_edition_axe();
        }
        if resp.drag_stopped() && let Some((id, fin, p)) = self.poignee_axe.take() {
            let saisie_valide = self.dessin2d.saisie.is_empty() || self.dessin2d.saisie.replace(',', ".").parse::<f64>().is_ok_and(|v| v.is_finite() && v > 0.0);
            self.terminer_edition_axe();
            let fixe = axe.as_ref().map(|(_, a, b, _, _)| if fin { *a } else { *b });
            if !saisie_valide { self.error = Some("Saisissez une longueur positive et finie.".into()); }
            else if let Some((projet, _)) = &self.natif {
                let resultat=(|| {
                    let mut copie=projet.clone();
                    copie.deplacer_extremite_justification(&id,fin,p)?;
                    copie.recaler_menuiseries(projet)?;
                    copie.verifier()?;
                    Ok::<_,String>(copie)
                })();
                match resultat.and_then(|p|self.appliquer_projet(p)) {
                    Ok(())=>{
                        if let Some(fixe) = fixe {
                            let longueur = (p[0] - fixe[0]).hypot(p[1] - fixe[1]) / crate::config::options::unite().facteur();
                            self.cote_axe = Some((id, fin, crate::formats::ifc::step::format_real(longueur)));
                        }
                        self.status="Mur modifié — cliquer la cote pour saisir une longueur.".into();
                    },
                    Err(e)=>self.error=Some(e),
                }
            }
        }
        // poignée du milieu (déplacer) et flèches d'inversion du mur sélectionné
        let poignee_milieu = self.poignee_axe.is_none() && !mode_trace && !edition_arc && self.poignee_milieu(ui, rect, &resp, &axe);
        let poignee_annotation = self.suivre_poignee_annotation(ui, rect, &resp, mode_trace || edition_arc || poignee_milieu || self.poignee_axe.is_some());
        let mode_trace = mode_trace || self.poignee_axe.is_some() || edition_arc || poignee_milieu || self.glisse_milieu.is_some() || poignee_annotation;
        // clic droit hors commande : répéter, annuler, sélection, visibilité, verrou, démolir
        if !mode_trace && self.ruban_edition.outil().is_none() && !self.afficher_masques {
            self.menu_contextuel(&resp);
        }
        if mode_trace && resp.hovered() {
            ui.ctx().set_cursor_icon(if self.selection_traces_active() { egui::CursorIcon::Default } else { egui::CursorIcon::Crosshair });
        }
        let ndc = |p: egui::Pos2| {
            Vec2::new((p.x - rect.left()) / rect.width() * 2.0 - 1.0, 1.0 - (p.y - rect.top()) / rect.height() * 2.0)
        };

        let repere_plan = self.scene.is_some() && self.vue_en_plan();
        let sur_repere_plan = repere_plan && ui.input(|i| {
            let zone = crate::rendu::orientation_plan::zone(rect);
            i.pointer.hover_pos().is_some_and(|p| zone.contains(p))
                || (i.pointer.any_down() && i.pointer.press_origin().is_some_and(|p| zone.contains(p)))
        });
        if self.scene.is_some() && !sur_repere_plan {
            let pick = |app: &Self, p: egui::Pos2| -> Option<(u32, Vec3)> {
                if !app.point_dans_cadrage(rect, p) { return None; }
                let s = app.scene.as_ref()?;
                let n = ndc(p);
                let (o, d) = app.camera.ray(n.x, n.y, aspect);
                s.pick(o, d, &app.visible, app.plan_coupe(), app.boite_appliquee()).map(|(obj, t)| (obj, o + d * t))
            };

            if mode_trace { self.fenetre_selection = None; }
            if !mode_trace && self.fenetre_selection.is_none() && resp.drag_started_by(PointerButton::Primary) {
                if let Some(debut) = ui.input(|i| i.pointer.press_origin()) {
                    if pick(self, debut).is_none() {
                        self.fenetre_selection = Some(FenetreSelection { debut, fin: debut, ajouter: ui.input(|i| i.modifiers.ctrl || i.modifiers.command) });
                    }
                }
            }
            let rectangle_actif = self.fenetre_selection.is_some();
            // Orbite autour du point visé (vue 3D seulement).
            if !mode_trace && !rectangle_actif && resp.drag_started_by(PointerButton::Middle) && ui.input(|i| i.modifiers.shift) && self.vue_3d() {
                if let Some((_, p)) = resp.interact_pointer_pos().and_then(|pp| pick(self, pp)) {
                    self.camera.set_pivot(p);
                }
            }
            let shift = ui.input(|i| i.modifiers.shift);
            if !mode_trace && !rectangle_actif { self.voler(ui, &resp); } else { self.vol.actif = false; }
            for b in [PointerButton::Primary, PointerButton::Secondary, PointerButton::Middle] {
                if (mode_trace || rectangle_actif) && b != PointerButton::Middle { continue; }
                if resp.dragged_by(b) && !(self.vol.actif && b == PointerButton::Secondary) {
                    self.glisser(b, resp.drag_delta(), shift, rect.height());
                }
            }

            if self.vol.actif {
                // molette : vitesse du vol, pas de zoom
            } else if let Some(hp) = resp.hover_pos() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let factor = 1.0 / accrochage::facteur_molette(scroll) as f32;
                    let target = pick(self, hp).map(|(_, p)| p).or_else(|| {
                        // Point sur le plan de visée à la distance de la cible.
                        let n = ndc(hp);
                        let (o, dir) = self.camera.ray(n.x, n.y, aspect);
                        let f = self.camera.forward();
                        let t = (self.camera.target - o).dot(f) / dir.dot(f).max(1e-6);
                        Some(o + dir * t)
                    });
                    self.camera.zoom(factor, if self.camera.ortho { None } else { target });
                }
            }

            // Le double-clic central cadre la vue entière, même pendant un outil.
            // Le double-clic gauche conserve son cadrage sur l'objet visé.
            if resp.double_clicked_by(PointerButton::Middle) {
                self.fit_all();
            }

            if rectangle_actif {
                let curseur = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()));
                if let Some(p) = curseur {
                    if let Some(f) = &mut self.fenetre_selection { f.fin = p.clamp(rect.min, rect.max); }
                }
                let annuler = ui.input(|i| i.key_pressed(egui::Key::Escape)) || resp.clicked_by(PointerButton::Secondary);
                if annuler { self.fenetre_selection = None; }
                else if resp.drag_stopped_by(PointerButton::Primary) {
                    if let Some(f) = self.fenetre_selection.take() { self.selectionner_rectangle(rect, f); }
                }
            } else if mode_attache {
                if resp.clicked_by(PointerButton::Secondary) { self.ruban_edition.annuler(); }
                if resp.clicked() && let Some(q) = resp.interact_pointer_pos() {
                    let cible = pick(self, q).and_then(|(objet, _)| self.scene.as_ref()?.objects.get(objet as usize).map(|o| o.id))
                        .and_then(|id| self.cle_native(self.cible_clic(id)));
                    if let Some(id) = cible {
                        let projet = &self.natif.as_ref().unwrap().0;
                        if projet.dalles.iter().any(|d| d.id == id && d.sorte == crate::modele::SorteDalle::Toiture) {
                            if self.bim.murs_a_attacher.is_empty() {
                                self.status = "Attacher au toit : choisissez d'abord un ou plusieurs murs.".into();
                            } else {
                                let ids = self.bim.murs_a_attacher.clone();
                                match self.executer_bim(crate::modele::edition::commandes::Commande::Attacher { ids, toiture: Some(id) }) {
                                    Ok(_) => { self.status = "Murs attachés à la sous-face de la toiture.".into(); self.ruban_edition.annuler(); }
                                    Err(e) => self.error = Some(e),
                                }
                            }
                        } else if let Some(mur) = projet.assemblage(&id).map(|a| a.id.clone()).or_else(|| projet.murs.iter().find(|m| m.id == id).map(|m| m.assemblage.clone().unwrap_or(id))) {
                            if !self.bim.murs_a_attacher.contains(&mur) { self.bim.murs_a_attacher.push(mur); }
                            self.status = "Mur ajouté : cliquez la toiture, ou ajoutez d'autres murs.".into();
                        } else { self.status = "Attacher au toit : cliquez un mur ou une toiture.".into(); }
                    }
                }
            } else if mode_ajuster {
                let hit=resp.hover_pos().or(resp.interact_pointer_pos()).and_then(|q|pick(self,q).map(|(obj,_)|obj));
                self.souris_ajuster(ui,rect,&resp,hit);
            } else if self.outil_dessin().is_some() {
                self.souris_dessin(ui, rect, &resp);
            } else if self.orientation_nord_en_cours {
                let curseur = resp.hover_pos().or(resp.interact_pointer_pos());
                let hit = curseur.and_then(|p| pick(self, p).map(|(obj, _)| (p, obj)));
                self.arete_alignement = curseur.and_then(|q| self.trait_annotation_vise(q, rect)).or_else(|| hit.and_then(|(p, obj)| self.arete_visee(p, rect, obj)));
                if resp.clicked_by(PointerButton::Secondary) {
                    self.orientation_nord_en_cours = false;
                    self.arete_alignement = None;
                    self.status = "Orientation du nord annulée.".into();
                } else if resp.clicked() {
                    let angle = self.arete_alignement.zip(curseur).and_then(|(trait_, q)| self.angle_nord_depuis_trait(trait_, q, rect));
                    if let (Some(angle), Some((p, _))) = (angle, &self.natif) {
                        let mut projet = p.clone();
                        projet.orientation_nord = angle;
                        match self.appliquer_projet(projet) {
                            Ok(()) => {
                                self.orientation_nord_en_cours = false;
                                self.arete_alignement = None;
                                self.status = "Nord du projet orienté — Ctrl+S pour enregistrer.".into();
                            }
                            Err(e) => self.error = Some(e),
                        }
                    } else { self.status = "Nord : viser un trait du plan, près de l'extrémité indiquant le nord.".into(); }
                }
            } else if self.alignement_plan {
                // Aligner (repère du plan) : l'arête visée (surlignée) devient horizontale ;
                // sans arête, la direction principale de l'élément
                let curseur = resp.hover_pos().or(resp.interact_pointer_pos());
                let hit = curseur.and_then(|p| pick(self, p).map(|(obj, _)| (p, obj)));
                self.arete_alignement = hit.and_then(|(p, obj)| self.arete_visee(p, rect, obj));
                if resp.clicked() {
                    let d = match self.arete_alignement {
                        Some([a, b]) => Some(glam::Vec2::new(b.x - a.x, b.y - a.y)),
                        None => hit.and_then(|(_, obj)| self.direction_principale(obj)),
                    };
                    match d.and_then(|d| crate::rendu::orientation_plan::angle_pour_horizontale(&self.camera, d)) {
                        Some(a) => {
                            crate::rendu::orientation_plan::orienter(&mut self.camera, a);
                            self.memoriser_vue();
                            self.alignement_plan = false;
                            self.arete_alignement = None;
                        }
                        None => self.status = "Aligner : viser une arête du plan (Échap : annuler)".into(),
                    }
                }
            } else if let Some(genre) = structure {
                if resp.clicked_by(PointerButton::Secondary) { self.bim.depart_structure = None; }
                if resp.clicked() {
                    let point = altitude_structure.and_then(|alt| resp.interact_pointer_pos().and_then(|q| self.point_tracage(q, rect, shift, self.bim.depart_structure, alt)));
                    if let Some((point, _)) = point {
                        match genre {
                            crate::modele::GenreBarre::Poteau => {
                                if let Err(e) = self.placer_structure(point, None) { self.error = Some(e); }
                            }
                            crate::modele::GenreBarre::Poutre => {
                                if let Some(debut) = self.bim.depart_structure {
                                    match self.placer_structure(debut, Some(point)) {
                                        Ok(()) => self.bim.depart_structure = None,
                                        Err(e) => self.error = Some(e),
                                    }
                                } else { self.bim.depart_structure = Some(point); }
                            }
                        }
                    } else {
                        self.error = Some(if altitude_structure.is_none() { "Choisissez un niveau pour la structure." } else { "Le plan de placement est parallèle à la vue. Utilisez une vue de dessus ou une vue 3D inclinée." }.into());
                    }
                }
            } else if texte_annotation {
                if resp.clicked_by(PointerButton::Secondary) { self.ruban_edition.annuler(); }
                if resp.clicked() && let Some(q) = resp.interact_pointer_pos()
                    && let Err(e) = self.placer_texte_vue(rect, q)
                {
                    self.error = Some(e);
                }
            } else if let Some(composant) = placement {
                if composant && ui.memory(|m| m.focused().is_none()) && ui.input(|i| i.key_pressed(egui::Key::Space)) {
                    self.bim.composant.rotation = (self.bim.composant.rotation + 90.0).rem_euclid(360.0);
                }
                if resp.clicked()
                    && let Some((point, _)) = resp.interact_pointer_pos().and_then(|q| self.point_tracage(q, rect, false, None, altitude_placement))
                {
                    let r = if composant { self.placer_composant(point) } else if arbre { self.placer_arbre(point) } else { self.placer_piece(point) };
                    match r {
                        Ok(()) => self.status = format!("{} créé(e) — Ctrl+Z pour annuler.", if composant { "Composant" } else if arbre { "Arbre" } else { "Pièce" }),
                        Err(e) => self.error = Some(e),
                    }
                }
            } else if let Some(outil) = deplacement {
                // élément sous le curseur : surligné avant le clic, cible d'Aligner
                let cible = |app: &Self, q: egui::Pos2| pick(app, q).and_then(|(obj, _)| app.scene.as_ref()?.objects.get(obj as usize).map(|o| o.id)).and_then(|id| app.cle_native(app.cible_clic(id)));
                // même cible qu'au clic : à défaut de face touchée (plan au trait), le mur sous le curseur
                self.transfo.cible = resp.hover_pos().and_then(|q| cible(self, q)).or_else(|| self.transfo.curseur.filter(|_| resp.hovered()).and_then(|p| self.mur_proche(p)));
                if resp.clicked_by(PointerButton::Secondary) && !self.transfo.reculer() { self.ruban_edition.annuler(); }
                if resp.clicked() && let Some(q) = resp.interact_pointer_pos() {
                    use crate::ui::ruban::Outil as O;
                    let depart = (matches!(outil, O::Deplacer | O::Copier | O::Symetrie | O::Echelle) || (outil == O::Reseau && !self.transfo.polaire)).then(|| self.transfo.points.first().copied()).flatten();
                    if let Some((p, _)) = self.point_tracage(q, rect, shift && depart.is_some(), depart, altitude_coupe) {
                        let c = cible(self, q);
                        self.clic_transformation(outil, p, q, rect, altitude_coupe, shift, c);
                    }
                }
            } else if let Some(prm) = &outil_coupe {
                if resp.clicked_by(PointerButton::Secondary) { self.coupe_depart = None; }
                if resp.clicked()
                    && let Some((p, _)) = resp.interact_pointer_pos().and_then(|q| self.point_tracage(q, rect, shift, self.coupe_depart, altitude_coupe))
                {
                    match self.coupe_depart.take() {
                        Some(a) => self.creer_vue(|projet| projet.ajouter_vue_coupe(a, p, prm.profondeur, prm.elevation, &prm.type_vue, prm.echelle), false),
                        None => self.coupe_depart = Some(p),
                    }
                }
            } else if resp.clicked() && self.afficher_masques && resp.interact_pointer_pos().and_then(|q| pick(self, q)).is_some_and(|(obj, _)| self.reafficher(obj)) {
                // mode Afficher : élément magenta réaffiché
            } else if resp.clicked() && self.dessiner_cadenas(ui, rect, resp.interact_pointer_pos()) {
                // clic sur un cadenas : élément déverrouillé
            } else if let Some(id) = resp.double_clicked().then(|| resp.interact_pointer_pos()).flatten()
                .and_then(|q| self.repere_coupe_proche(rect, q, altitude_coupe))
                .filter(|id| self.natif.as_ref().is_some_and(|(p, _)| p.vues.iter().any(|v| &v.id == id)))
            {
                // double-clic sur le repère d'une coupe ou d'une élévation : ouvrir la vue
                self.ouvrir_vue_native(&id);
            } else if resp.double_clicked() {
                match resp.interact_pointer_pos().and_then(|p| pick(self, p)) {
                    Some((obj, _)) => self.frame_objects(&[obj]),
                    None => self.fit_all(),
                }
            } else if let Some((c, k)) = resp.clicked().then(|| resp.interact_pointer_pos()).flatten().and_then(|q| {
                self.aretes_pente(rect).into_iter().find(|a| a.2.distance(q) <= 9.0).map(|a| (a.0, a.1))
            }) {
                // repère d'arête de l'élément sélectionné : la pente est ajoutée ou retirée
                self.basculer_pente(c, k);
            } else if resp.clicked() && resp.interact_pointer_pos().zip(self.origine_ecran(rect)).is_some_and(|(q, o)| q.distance(o) <= 10.0) {
                // origine du projet (trait invisible) : ses coordonnées dans Propriétés
                self.select(None, false);
                self.bim.origine = true;
            } else if resp.clicked() && resp.interact_pointer_pos().and_then(|q| self.limite_propriete_proche(rect, q)).is_some() {
                let id = self.limite_propriete_proche(rect, resp.interact_pointer_pos().unwrap()).unwrap();
                let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                let mut selection = if ctrl { self.selection_bim() } else { Vec::new() };
                if let Some(i) = selection.iter().position(|x| x == &id) { selection.remove(i); } else { selection.push(id); }
                let selection_id = selection.iter().find_map(|cle| self.scene.as_ref()?.objects.iter().find(|o| self.cle_native(o.id).as_deref() == Some(cle)).map(|o| o.id));
                self.select(selection_id, false);
                self.bim.selection = selection;
            } else if resp.clicked() && resp.interact_pointer_pos().is_some_and(|q| self.point_dans_cadrage(rect, q) && self.rectangles_annotations(rect).iter().any(|(_, r)| r.expand(4.0).contains(q))) {
                let q = resp.interact_pointer_pos().unwrap();
                let id = self.rectangles_annotations(rect).into_iter().find(|(_, r)| r.expand(4.0).contains(q)).unwrap().0;
                let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                let mut selection = if ctrl { self.selection_bim() } else { Vec::new() };
                if let Some(i) = selection.iter().position(|x| x == &id) { selection.remove(i); } else { selection.push(id); }
                self.select(None, false);
                self.bim.selection = selection;
            } else if resp.clicked() && self.vue_en_plan() && resp.interact_pointer_pos().and_then(|q| self.repere_coupe_proche(rect, q, altitude_coupe)).is_some() {
                let id = self.repere_coupe_proche(rect, resp.interact_pointer_pos().unwrap(), altitude_coupe).unwrap();
                let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                let mut selection = if ctrl { self.selection_bim() } else { Vec::new() };
                if let Some(i) = selection.iter().position(|x| x == &id) { selection.remove(i); } else { selection.push(id); }
                self.select(None, false);
                self.bim.selection = selection;
            } else if resp.clicked() {
                // Plan : une arête en pointillés de la plage haute (au-dessus de la coupe)
                // est prioritaire sur l'élément coupé ou vu dessous.
                let haut = (self.ruban_edition.outil()!=Some(crate::ui::ruban::Outil::Ouverture)).then(|| resp.interact_pointer_pos().and_then(|q| self.objet_plage_haute(rect, q))).flatten();
                let hit = match haut {
                    Some(obj) => Some((obj, self.scene.as_ref().map_or(Vec3::ZERO, |s| s.objects[obj as usize].min))),
                    None => resp.interact_pointer_pos().and_then(|p| pick(self, p)),
                };
                let id = hit.map(|(obj, _)| self.cible_clic(self.scene.as_ref().unwrap().objects[obj as usize].id));
                self.point_clique = hit.map(|(_, p)| p.as_dvec3() + self.scene.as_ref().unwrap().origin);
                if self.natif.is_some() && self.ruban_edition.outil()==Some(crate::ui::ruban::Outil::Ouverture) {
                    if let (Some(id),Some(p))=(id,self.point_clique) { self.placer_ouverture(id,[p.x*1000.0,p.y*1000.0]); }
                } else if id.is_none() {
                    // Un clic bref désélectionne seulement. Le rectangle est réservé
                    // au glissement avec le bouton gauche maintenu depuis le vide.
                    if !ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
                        self.select(None, false);
                    }
                } else {
                    if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) && self.natif.is_none() {
                        let mut ids = self.selection_multiple.clone();
                        if ids.is_empty() { if let Some(sel) = self.selected { ids.push(sel); } }
                        if let Some(id) = id { if let Some(pos) = ids.iter().position(|x| *x == id) { ids.remove(pos); } else { ids.push(id); } }
                        self.select(ids.first().copied(), true);
                        self.selection_multiple = ids;
                    } else if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
                        let mut selection=self.selection_bim();
                        if let Some(cle)=id.and_then(|id|self.cle_native(id)) {
                            if selection.contains(&cle){selection.retain(|s|s!=&cle);}else{selection.push(cle);}
                        }
                        self.select(if selection.is_empty(){None}else{id},true);
                        self.bim.selection=selection;
                    } else {self.select(id,true);}
                }
            }

            if (resp.hovered() || resp.has_focus()) && !self.vol.actif && ui.input(|i|i.key_pressed(egui::Key::Escape)) && !rectangle_actif {
                if self.isolated.is_some() {self.isolated=None;self.visibility_dirty=true;}
                else {self.select(None,false);}
            }
        }


        let mut t_vue = std::time::Instant::now();
        self.update_visibility();
        crate::noyau::chrono::chrono_lent("vue : visibilité", &mut t_vue);
        self.update_coupe();
        crate::noyau::chrono::chrono_lent("vue : coupe et dessin 2D", &mut t_vue);

        let params = self.frame_params(aspect);
        let ppp = ui.ctx().pixels_per_point();
        let (largeur, hauteur) = ((rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32);
        if largeur > 0 && hauteur > 0 && let Some(id) = self.renderer.as_mut().and_then(|r| r.paint(&params, largeur, hauteur)) {
            ui.painter().image(id, rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        }
        self.dessiner_poignees_annotation(ui, rect, mode_trace && !poignee_annotation);
        // confirmation d'enregistrement : 2,5 s en bas de la vue
        if let Some((texte, debut)) = &self.confirmation {
            let age = debut.elapsed().as_secs_f32();
            if age < 2.5 {
                let alpha = ((2.5 - age) / 0.5).min(1.0);
                let police = egui::FontId::proportional(13.0);
                let galet = ui.fonts_mut(|f| f.layout_no_wrap(texte.clone(), police.clone(), Color32::WHITE));
                let centre = egui::pos2(rect.center().x, rect.bottom() - 48.0);
                let fond = egui::Rect::from_center_size(centre, galet.size() + egui::vec2(24.0, 12.0));
                ui.painter().rect_filled(fond, 6.0, Color32::from_rgba_unmultiplied(40, 120, 60, (230.0 * alpha) as u8));
                ui.painter().text(centre, egui::Align2::CENTER_CENTER, texte.as_str(), police, Color32::from_white_alpha((255.0 * alpha) as u8));
                ui.ctx().request_repaint();
            } else {
                self.confirmation = None;
            }
        }
        if let Some(f) = self.fenetre_selection {
            f.dessiner(&ui.painter().with_clip_rect(rect));
        }
        if self.natif.is_some() && self.vue_en_plan() {
            self.reperes_coupes(ui, rect, altitude_coupe);
        }
        if self.natif.is_some() && self.jeu_actif().is_none() {
            self.traits_invisibles(ui, rect);
            self.saisie_origine(ui, rect);
        }
        self.barre_commande(ui, rect);
        if let Some(prm) = &outil_coupe {
            let point = resp.hover_pos().and_then(|q| self.point_tracage(q, rect, ui.input(|i| i.modifiers.shift), self.coupe_depart, altitude_coupe));
            self.apercu_coupe(ui, rect, point, altitude_coupe, prm);
        }
        if let Some(genre) = structure {
            let painter = ui.painter().with_clip_rect(rect);
            let couleur = Color32::from_rgb(55, 130, 200);
            let texte = match (genre, self.bim.depart_structure) {
                (crate::modele::GenreBarre::Poteau, _) => "Poteau : cliquer pour placer · molette : zoom",
                (_, None) => "Poutre : cliquer le départ · molette : zoom",
                (_, Some(_)) => "Poutre : cliquer l'arrivée · Maj : orthogonal · Échap ou clic droit : annuler",
            };
            if self.error.is_none() { super::aides::annotation_vue(ui, rect, self.dessin2d.bas_barre, texte, false); }
            if let Some(alt) = altitude_structure {
                let point = resp.hover_pos().and_then(|q| self.point_tracage(q, rect, ui.input(|i| i.modifiers.shift), self.bim.depart_structure, alt));
                if let Some((p, accroche)) = point {
                    if let Some(q) = self.ecran_plan(rect, p, alt) { marqueur(&painter, q, accroche, couleur, Color32::from_rgb(230, 120, 20)); }
                    if let Some(a) = self.bim.depart_structure {
                        if let (Some(x), Some(y)) = (self.ecran_plan(rect, a, alt), self.ecran_plan(rect, p, alt)) {
                            painter.line_segment([x, y], egui::Stroke::new(2.0, couleur));
                            painter.text(x.lerp(y, 0.5) + egui::vec2(0.0, -16.0), egui::Align2::CENTER_CENTER, crate::config::options::unite().afficher((p[0]-a[0]).hypot(p[1]-a[1])), egui::FontId::proportional(13.0), couleur);
                        }
                    }
                }
            }
        }
        if mode_attache {
            let texte = if self.bim.murs_a_attacher.is_empty() {
                "Attacher au toit : cliquer un mur, puis une toiture".to_string()
            } else {
                format!("Attacher au toit : {} mur(s) choisi(s) · cliquer une toiture ou ajouter un mur", self.bim.murs_a_attacher.len())
            };
            let painter = ui.painter().with_clip_rect(rect);
            if self.error.is_none() { super::aides::annotation_vue(ui, rect, self.dessin2d.bas_barre, &texte, false); }
            if let Some((projet, _)) = &self.natif {
                for id in &self.bim.murs_a_attacher {
                    if let Some((a, b, _)) = projet.axe_element(id) {
                        let z = projet.trajectoire_justification(id).map_or(0.0, |(_, z)| z);
                        if let (Some(a), Some(b)) = (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z)) {
                            painter.line_segment([a, b], egui::Stroke::new(3.0, Color32::from_rgb(230, 120, 20)));
                        }
                    }
                }
            }
        }
        if texte_annotation && let Some(q) = resp.hover_pos() {
            // aperçu du texte sous le curseur, ancré comme le texte créé
            let contenu = if self.bim.texte.trim().is_empty() { "Texte" } else { self.bim.texte.as_str() };
            ui.painter().with_clip_rect(rect).text(q, egui::Align2::LEFT_BOTTOM, contenu, egui::FontId::proportional(12.0), Color32::from_rgb(55, 130, 200));
        }
        if let Some(composant) = placement {
            let painter = ui.painter().with_clip_rect(rect);
            let couleur = Color32::from_rgb(55, 130, 200);
            let texte = if composant { "Composant : cliquer pour placer · Espace : rotation de 90° · molette : zoom" } else if arbre { "Arbre : cliquer pour planter" } else { "Pièce : cliquer dans un espace fermé par des murs" };
            if self.error.is_none() { super::aides::annotation_vue(ui, rect, self.dessin2d.bas_barre, texte, false); }
            if let Some((p, accroche)) = resp.hover_pos().and_then(|q| self.point_tracage(q, rect, false, None, altitude_placement)) {
                if let Some(q) = self.ecran_plan(rect, p, altitude_placement) { marqueur(&painter, q, accroche, couleur, Color32::from_rgb(230, 120, 20)); }
                if arbre {
                    // aperçu : cercle du houppier
                    let a = &self.bim.arbre;
                    let pts: Vec<egui::Pos2> = (0..36).filter_map(|k| {
                        let ang = k as f64 / 36.0 * std::f64::consts::TAU;
                        self.ecran_plan(rect, [p[0] + ang.cos() * a.diametre / 2.0, p[1] + ang.sin() * a.diametre / 2.0], altitude_placement)
                    }).collect();
                    if pts.len() == 36 {
                        painter.add(egui::Shape::closed_line(pts, egui::Stroke::new(1.5, Color32::from_rgb(60, 140, 60))));
                    }
                }
                if composant {
                    let c = &self.bim.composant;
                    let defaut = crate::modele::elements::composant::type_table();
                    let t = self.natif.as_ref().and_then(|(x, _)| x.types_composants.iter().find(|t| t.id == c.type_composant)).unwrap_or(&defaut);
                    let (s, co) = c.rotation.to_radians().sin_cos();
                    let (u, n) = ([co, s], [-s, co]);
                    let coin = |a: f64, b: f64| [p[0] + u[0] * a * t.largeur / 2.0 + n[0] * b * t.profondeur / 2.0, p[1] + u[1] * a * t.largeur / 2.0 + n[1] * b * t.profondeur / 2.0];
                    let coins: Vec<egui::Pos2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().filter_map(|&(a, b)| self.ecran_plan(rect, coin(a, b), altitude_placement)).collect();
                    if coins.len() == 4 {
                        painter.add(egui::Shape::closed_line(coins, egui::Stroke::new(2.0, couleur)));
                    }
                    // sens de la famille : trait vers l'avant (Y négatif, côté « Intérieur »)
                    if let (Some(a), Some(b)) = (self.ecran_plan(rect, p, altitude_placement), self.ecran_plan(rect, coin(0.0, -1.0), altitude_placement)) {
                        painter.line_segment([a, b], egui::Stroke::new(1.0, couleur));
                    }
                }
            }
        }
        self.dessiner_cadenas(ui, rect, None);
        let temporaire = !self.masques_utilisateur.is_empty() || self.isolement_utilisateur;
        if self.afficher_masques || temporaire {
            let couleur = if self.afficher_masques { Color32::from_rgb(235, 50, 190) } else { Color32::from_rgb(0, 150, 150) };
            ui.painter().rect_stroke(rect.shrink(2.0), 0.0, egui::Stroke::new(3.0, couleur), egui::StrokeKind::Inside);
            let texte = if self.afficher_masques { "ÉLÉMENTS MASQUÉS · clic pour réafficher · Échap pour sortir" } else { "VISIBILITÉ TEMPORAIRE" };
            // croix dans le coin bas droit du cadre (le haut porte barres et repère du plan) :
            // quitter le mode
            let croix = egui::Rect::from_center_size(egui::pos2(rect.right() - 20.0, rect.bottom() - 20.0), egui::vec2(24.0, 24.0));
            ui.painter().text(egui::pos2(croix.left() - 8.0, croix.center().y), egui::Align2::RIGHT_CENTER, texte, egui::FontId::proportional(12.0), couleur);
            let r = ui.interact(croix, egui::Id::unique("quitter_visibilite_temporaire"), Sense::click()).on_hover_text(if self.afficher_masques { "Quitter l'affichage des éléments masqués" } else { "Quitter le masquage temporaire : tout réafficher" });
            let fond = if r.hovered() { couleur } else { Color32::from_rgba_unmultiplied(255, 255, 255, 230) };
            ui.painter().rect_filled(croix, 4.0, fond);
            ui.painter().rect_stroke(croix, 4.0, egui::Stroke::new(1.5, couleur), egui::StrokeKind::Inside);
            dessiner_croix_onglet(ui.painter(), croix.center(), if r.hovered() { Color32::WHITE } else { couleur });
            if r.clicked() {
                if self.afficher_masques { self.basculer_afficher_masques(); } else { self.quitter_visibilite_temporaire(); }
            }
            self.croix_temporaire = Some(croix.center());
        } else {
            self.croix_temporaire = None;
        }
        if deplacement.is_some() {
            self.apercu_transformation(ui, rect, resp.hover_pos(), altitude_coupe);
        }
        if !mode_ajuster && dalle.is_none() && !mode_mur && outil_coupe.is_none() && structure.is_none() && self.dessin2d.edition_contour.is_none() {
            self.dessiner_aretes_pente(ui, rect, resp.hover_pos());
            self.editer_talus(ui, rect);
            self.dessiner_axe(ui, rect, resp.hover_pos());
            if self.poignee_axe.is_some() {self.dessiner_dessin2d(ui,rect);}
        }
        if self.alignement_plan || self.orientation_nord_en_cours {
            self.dessiner_arete_alignement(ui, rect);
        }
        if mode_ajuster {self.dessiner_ajustement(ui,rect);}
        if self.outil_dessin().is_some() {
            self.dessiner_dessin2d(ui, rect);
        }
        self.dessiner_instructions(ui, rect);
        self.cotes_terrain(ui, rect);
        self.cotes_pilotables(ui, rect);
        if self.scene.is_some() {
            self.poignees_boite(ui, rect);
        }
        if self.vol.actif {
            ui.painter().text(
                rect.left_bottom() + egui::vec2(10.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Vol · vitesse ×{:.2} (molette) · ZQSD/WASD, A/E : descendre/monter, Maj ×3", self.vol.vitesse),
                egui::FontId::proportional(12.0),
                ui.visuals().strong_text_color(),
            );
        }
        if repere_plan {
            let origine = self.orientation_initiale_plan();
            let positions = self.positions_plan_liste();
            let nord = self.natif.as_ref().map_or(0.0, |(p, _)| p.orientation_nord as f32);
            let r = crate::rendu::orientation_plan::afficher(ui, rect, &mut self.camera, origine, self.alignement_plan, &positions, nord);
            if r.aligner {
                self.alignement_plan = !self.alignement_plan;
                self.orientation_nord_en_cours = false;
                self.arete_alignement = None;
            }
            if r.nouvelle {
                self.nom_position_plan = Some(format!("Position {}", positions.len() + 1));
            }
            if let Some(i) = r.supprimer {
                self.supprimer_position_plan(i);
            }
            if r.change {
                self.memoriser_vue();
            }
        } else if self.scene.is_some() && self.vue_3d() {
            self.cube_orientation(ui, rect);
            self.choix_projection(ui, rect);
        }

        if self.scene.is_none() {
            let msg = if self.loading.is_some() { self.status.clone() } else { "Glisser un fichier .ifc ou .rvt ici\nou Ctrl+O".to_string() };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(20.0), Color32::from_gray(110));
        }
    }
}
