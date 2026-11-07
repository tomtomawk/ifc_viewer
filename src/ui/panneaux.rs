//! Panneaux ancrables, soleil, styles, cube d'orientation, panneau gauche.
use super::*;

impl ViewerApp {
    /// Contenu d'un panneau déplaçable.
    pub(super) fn panneau(&mut self, ui: &mut egui::Ui, p: Panneau) {
        // Le contenu garde une largeur lisible sans imposer cette largeur au volet.
        // Le défilement permet de retrouver les champs même dans un volet très étroit.
        if self.ancrages[p as usize]!=Ancrage::Externe {
            self.volets[p as usize].taille=Some(ui.available_size());
        }
        let largeur = ui.available_width().max(280.0);
        egui::ScrollArea::both()
            .id_salt(("contenu_panneau", p.nom()))
            .min_scrolled_width(0.0)
            .min_scrolled_height(0.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(largeur);
                self.contenu_panneau(ui, p);
            });
    }

    pub(super) fn contenu_panneau(&mut self, ui: &mut egui::Ui, p: Panneau) {
        // onglet Famille : les panneaux montrent la famille
        if self.famille_active && self.famille.is_some() {
            match p {
                Panneau::Proprietes => self.proprietes_famille(ui),
                Panneau::Arborescence => self.arborescence_famille(ui),
            }
            return;
        }
        match p {
            Panneau::Proprietes => self.right_panel(ui),
            Panneau::Arborescence => self.left_panel(ui),
        }
    }

    /// Poignée de déplacement et menu pour détacher ou réancrer le volet.
    pub(super) fn entete_panneau(&mut self, ui: &mut egui::Ui, p: Panneau, titre: &str) {
        let i = p as usize;
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::hover());
            let resp = ui.interact(rect, egui::Id::unique(("entete_panneau", p.nom())), Sense::click_and_drag());
            let encre = ui.visuals().strong_text_color();
            ui.painter_at(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, titre, egui::FontId::proportional(15.0), encre);
            if resp.drag_started_by(PointerButton::Primary) {
                if let Some(q)=resp.interact_pointer_pos() {
                    let appui=ui.input(|i|i.pointer.press_origin()).unwrap_or(q);
                    let decalage=appui-rect.min;
                    let origine=ui.ctx().input(|i|i.viewport().inner_rect.map_or(egui::Pos2::ZERO,|r|r.min));
                    self.volets[i].position=Some(origine+(q-decalage).to_vec2());
                    self.deplacement_panneau=Some((p,decalage,ui.ctx().viewport_id()));
                    self.ancrer(p,Ancrage::Externe);
                }
            }
            self.volets[i].entete=Some(rect);
            self.menu_volet(&resp,p);
            if self.ancrages[i]==Ancrage::Externe && resp.double_clicked() {
                self.ancrer(p,if p==Panneau::Proprietes {Ancrage::Gauche}else{Ancrage::Droite});
            }
            let survol = if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab };
            resp.on_hover_cursor(survol).on_hover_text(if self.ancrages[i]==Ancrage::Externe {"Glisser pour déplacer ; double-clic pour réintégrer le volet"} else {"Glisser le titre pour détacher et déplacer le volet sur un autre écran"});
        });
    }

    pub(super) fn cible_ancrage(&self, q: egui::Pos2) -> Option<Ancrage> {
        volets::cible_volet(self.zone_panneaux,q)
    }

    /// Le suivi reste actif pendant le passage d'un panneau ancré à une fenêtre flottante.
    pub(super) fn deplacer_panneau(&mut self, ctx: &egui::Context) {
        let Some((p,decalage,source))=self.deplacement_panneau else{return};
        // Lire le viewport où le geste a commencé, aussi bien ancré que déjà flottant.
        if let Some((position,local,enfonce))=volets::suivre_pointeur(ctx,source,decalage) {
            self.volets[p as usize].position=Some(position);
            self.volets[p as usize].cible=self.cible_ancrage(local);
            if !enfonce {
                if let Some(a)=self.volets[p as usize].cible {self.ancrer(p,a);}
                self.deplacement_panneau=None;
                self.volets[p as usize].cible=None;
            }
        }
        ctx.request_repaint();
    }

    pub(super) fn apercu_ancrage(&self, ctx: &egui::Context) {
        let Some((p, _, _)) = self.deplacement_panneau else { return };
        let Some(cible) = self.volets[p as usize].cible else { return };
        let zone = self.zone_panneaux;
        let largeur = self.largeurs_panneaux[p as usize].min(zone.width() * 0.45);
        let x = if cible == Ancrage::Gauche { zone.left() } else { zone.right() - largeur };
        let rect = egui::Rect::from_min_size(egui::pos2(x, zone.top()), egui::vec2(largeur, zone.height()));
        let peintre = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("apercu_ancrage")));
        peintre.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(90, 140, 195, 45));
        peintre.rect_stroke(rect, 4.0, egui::Stroke::new(1.5, Color32::from_rgb(90, 140, 195)), egui::StrokeKind::Inside);
    }

    /// Ancre un panneau, ou le rend flottant au-dessus de la vue (à sa dernière place
    /// flottante, ramenée dans la vue).
    pub(super) fn ancrer(&mut self, p: Panneau, a: Ancrage) {
        let i = p as usize;
        if a == Ancrage::Flottant && self.ancrages[i] != a && self.pos_imposee[i].is_none() && self.rect_vue.is_positive() {
            let v = self.rect_vue;
            let q = self.pos_flottants[i];
            self.pos_imposee[i] = Some(egui::pos2(q.x.clamp(v.left(), (v.right() - 200.0).max(v.left())), q.y.clamp(v.top(), (v.bottom() - 200.0).max(v.top()))));
        }
        self.ancrages[i] = a;
        if a != Ancrage::Flottant {
            self.pos_imposee[i] = None;
        }
    }

    /// Ombres portées en cascades : la vue est découpée en tranches de profondeur (de
    /// quelques dizaines de centimètres à la fin de la scène), chacune avec sa carte
    /// d'ombres de 4096 px, petite près de l'œil (ombres fines) et grande au loin. Chaque
    /// carte englobe sa tranche (sphère, taille fixe : pas de frémissement quand la caméra
    /// tourne), calée sur sa grille de pixels, et toute la scène en profondeur (les objets
    /// hors champ portent aussi leur ombre).
    pub(super) fn soleil(&self, aspect: f32) -> Option<crate::rendu::gpu::Ombres> {
        use crate::rendu::gpu::{Cascade, NB_CASCADES, TAILLE_OMBRES};
        let s = self.scene.as_ref()?;
        let dir = self.direction_soleil();
        let c_scene = (s.min + s.max) * 0.5;
        let r_scene = ((s.max - s.min).length() * 0.5).max(0.1) * 1.02;
        let inv = self.camera.view_proj(aspect).inverse();
        let proj = self.camera.proj(aspect);
        let avant = self.camera.direction_vue();
        let oeil = self.camera.eye();
        // profondeurs de la vue (le long du regard) : de l'œil (ou du bord de la scène) à
        // la fin de la scène
        let d_scene = (c_scene - oeil).dot(avant);
        let (n, f) = if self.camera.ortho {
            ((d_scene - r_scene).max(0.0), d_scene + r_scene)
        } else {
            ((d_scene - r_scene).max(0.3), (d_scene + r_scene).clamp(5.0, 3000.0))
        };
        if f <= n {
            return None;
        }
        // découpage « pratique » : logarithmique près de l'œil, uniforme au loin
        let lambda = if self.camera.ortho { 0.0 } else { 0.85 };
        let coupe = |k: usize| {
            let t = k as f32 / NB_CASCADES as f32;
            lambda * n * (f / n).powf(t) + (1.0 - lambda) * (n + (f - n) * t)
        };
        // NDC z d'une profondeur de vue
        let ndc_z = |d: f32| {
            let c = proj * glam::Vec4::new(0.0, 0.0, -d, 1.0);
            c.z / c.w
        };
        let vue = glam::camera::rh::view::look_at_mat4(Vec3::ZERO, -dir, if dir.z.abs() > 0.99 { Vec3::Y } else { Vec3::Z });
        let coins_scene: Vec<Vec3> = (0..8).map(|k| Vec3::new(if k & 1 == 0 { s.min.x } else { s.max.x }, if k & 2 == 0 { s.min.y } else { s.max.y }, if k & 4 == 0 { s.min.z } else { s.max.z })).collect();
        let (zmin, zmax) = coins_scene.iter().map(|p| vue.transform_point3(*p).z).fold((f32::MAX, f32::MIN), |(a, b), z| (a.min(z), b.max(z)));
        let mut cascades = Vec::with_capacity(NB_CASCADES);
        for k in 0..NB_CASCADES {
            let (d0, d1) = (coupe(k), coupe(k + 1));
            let mut pts = Vec::with_capacity(8);
            for d in [d0, d1] {
                let z = ndc_z(d).clamp(-1.0, 1.0);
                for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    pts.push(inv.project_point3(Vec3::new(x, y, z)));
                }
            }
            let centre = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
            // rayon arrondi : taille de carte constante pour la tranche
            let rayon = (pts.iter().map(|p| p.distance(centre)).fold(0.0f32, f32::max) * 16.0).ceil() / 16.0;
            let rayon = rayon.min(r_scene).max(0.5);
            let texel = 2.0 * rayon / TAILLE_OMBRES as f32;
            let c = vue.transform_point3(centre);
            let (cx, cy) = ((c.x / texel).floor() * texel, (c.y / texel).floor() * texel);
            let ortho = glam::camera::rh::proj::opengl::orthographic(cx - rayon, cx + rayon, cy - rayon, cy + rayon, -zmax - 1.0, -zmin + 1.0);
            cascades.push(Cascade { matrice: ortho * vue, distance: d1, biais: 1.5 * texel });
        }
        Some(crate::rendu::gpu::Ombres { direction: dir, cascades })
    }

    /// Style visuel : bouton à l'icône du style en cours, menu des styles.
    pub(super) fn choix_style(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn choix_fond_plan(&mut self,ui:&mut egui::Ui) {
        let Some(id)=self.vue_native_id() else{return};
        let Some((p,_))=&self.natif else{return};
        let Some(v)=p.vues.iter().find(|v|v.id==id && matches!(v.genre,crate::modele::GenreVue::Plan{..})) else{return};
        let mut choisi=v.fond_plan.clone();
        let vues:Vec<_>=p.vues.iter().filter(|v|v.id!=id && matches!(v.genre,crate::modele::GenreVue::Plan{..})).map(|v|(v.id.clone(),v.nom.clone())).collect();
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

    /// Cube d'orientation (vue 3D), dans le coin haut droit de la vue : clic sur une face,
    /// une arête ou un coin pour regarder le modèle depuis cette direction ; glisser pour
    /// tourner autour. Boussole en anneau à sa base : clic sur N, E, S ou O pour regarder
    /// depuis ce point cardinal (inclinaison gardée) ; glisser l'anneau pour tourner autour
    /// de la verticale.
    /// Choix de la projection sous le cube : perspective, verticales droites (deux
    /// points), orthographique.
    pub(super) fn choix_projection(&mut self, ui: &mut egui::Ui, vue: egui::Rect) {
        use crate::rendu::camera::Projection;
        let largeur = 132.0;
        let zone = egui::Rect::from_min_size(egui::pos2(vue.right() - largeur - 8.0, vue.top() + 8.0 + 132.0 + 4.0), egui::vec2(largeur, 22.0));
        let actuelle = self.camera.projection();
        let mut choisie = actuelle;
        // fond : lisible sur le ciel comme sur le modèle
        let v = ui.visuals();
        ui.painter().rect_filled(zone.expand(1.0), 4.0, v.window_fill.gamma_multiply(0.85));
        ui.painter().rect_stroke(zone.expand(1.0), 4.0, v.window_stroke, egui::StrokeKind::Inside);
        ui.scope_builder(egui::UiBuilder::new().max_rect(zone), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (p, texte, aide) in [
                    (Projection::Perspective, "Persp.", "Perspective"),
                    (Projection::VerticalesDroites, "2 points", "Perspective à verticales droites (deux points) : les verticales restent verticales ; au-delà de 60° d'inclinaison, perspective ordinaire"),
                    (Projection::Orthographique, "Ortho", "Projection orthographique"),
                ] {
                    let r = ui.add_sized([42.0, 20.0], egui::Button::selectable(choisie == p, RichText::new(texte).small()));
                    if r.on_hover_text(aide).clicked() {
                        choisie = p;
                    }
                }
            });
        });
        if choisie != actuelle {
            self.camera.definir_projection(choisie);
            self.visibility_dirty = true;
            self.enregistrer_rendu_vue();
        }
        // vue piéton et enregistrement du point de vue
        let zone2 = zone.translate(egui::vec2(0.0, 26.0));
        let v = ui.visuals();
        ui.painter().rect_filled(zone2.expand(1.0), 4.0, v.window_fill.gamma_multiply(0.85));
        ui.painter().rect_stroke(zone2.expand(1.0), 4.0, v.window_stroke, egui::StrokeKind::Inside);
        let (mut pieton, mut vue) = (false, false);
        ui.scope_builder(egui::UiBuilder::new().max_rect(zone2), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                pieton = ui.add_sized([64.0, 20.0], egui::Button::selectable(self.pieton, RichText::new("Piéton").small()))
                    .on_hover_text("Vue piéton : œil à 1,60 m au-dessus du sol, la marche (clic droit + ZQSD) suit le terrain, les dalles et les marches")
                    .clicked();
                vue = ui.add_enabled(self.natif.is_some(), egui::Button::new(RichText::new("+ Vue").small()).min_size(egui::vec2(64.0, 20.0)))
                    .on_hover_text("Enregistrer ce point de vue comme vue 3D du projet (avec le style, le soleil et le ciel)")
                    .on_disabled_hover_text("Projet natif seulement")
                    .clicked();
            });
        });
        if pieton {
            self.basculer_pieton();
        }
        if vue && let Err(e) = self.enregistrer_vue_camera(None) {
            self.error = Some(e);
        }
    }

    pub(super) fn cube_orientation(&mut self, ui: &mut egui::Ui, vue: egui::Rect) {
        use egui::{Stroke, pos2};
        let cote = 132.0;
        let zone = egui::Rect::from_min_size(pos2(vue.right() - cote - 8.0, vue.top() + 8.0), egui::vec2(cote, cote));
        let resp = ui.interact(zone, ui.make_persistent_id("cube_orientation"), Sense::click_and_drag());
        // orientation réelle de la caméra (y compris inclinée en verticales droites)
        let rot = glam::camera::rh::view::look_at_mat4(self.camera.eye(), self.camera.target, self.camera.up());
        let centre = zone.center();
        let k = cote * 0.2;
        let ecran = |p: Vec3| {
            let q = rot.transform_vector3(p);
            pos2(centre.x + q.x * k, centre.y - q.y * k)
        };
        // faces : normale, axes (u, v) de leur grille 3 × 3, nom
        let faces = [
            (Vec3::Z, Vec3::X, Vec3::Y, "HAUT"),
            (Vec3::NEG_Z, Vec3::X, Vec3::NEG_Y, "BAS"),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z, "AVANT"),
            (Vec3::Y, Vec3::NEG_X, Vec3::Z, "ARRIÈRE"),
            (Vec3::X, Vec3::Y, Vec3::Z, "DROITE"),
            (Vec3::NEG_X, Vec3::NEG_Y, Vec3::Z, "GAUCHE"),
        ];
        let cellule = |n: Vec3, u: Vec3, v: Vec3, i: usize, j: usize| {
            let (a0, a1) = ([-1.0, -0.6, 0.6][i], [-0.6, 0.6, 1.0][i]);
            let (b0, b1) = ([-1.0, -0.6, 0.6][j], [-0.6, 0.6, 1.0][j]);
            [n + u * a0 + v * b0, n + u * a1 + v * b0, n + u * a1 + v * b1, n + u * a0 + v * b1].map(ecran)
        };
        let dedans = |p: egui::Pos2, q: &[egui::Pos2; 4]| {
            let signe = |a: egui::Pos2, b: egui::Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            let s: Vec<f32> = (0..4).map(|m| signe(q[m], q[(m + 1) % 4])).collect();
            s.iter().all(|x| *x >= 0.0) || s.iter().all(|x| *x <= 0.0)
        };
        // boussole : anneau dans le plan de la base du cube, quadrilatères (repère du
        // modèle) ; points cardinaux (nom, direction depuis le centre, libellé)
        const R0: f32 = 1.55;
        const R1: f32 = 1.95;
        const Z: f32 = -1.0;
        let n_seg = 64;
        let au = |a: f32, r: f32| Vec3::new(a.cos() * r, a.sin() * r, Z);
        let segments: Vec<[Vec3; 4]> = (0..n_seg)
            .map(|i| {
                let (a, b) = (i as f32 / n_seg as f32 * std::f32::consts::TAU, (i + 1) as f32 / n_seg as f32 * std::f32::consts::TAU);
                [au(a, R0), au(a, R1), au(b, R1), au(b, R0)]
            })
            .collect();
        let rotation_nord = glam::Quat::from_rotation_z(self.natif.as_ref().map_or(0.0, |(p, _)| p.orientation_nord as f32));
        let cardinaux = [("N", rotation_nord * Vec3::Y, "le nord"), ("E", rotation_nord * Vec3::X, "l'est"), ("S", rotation_nord * Vec3::NEG_Y, "le sud"), ("O", rotation_nord * Vec3::NEG_X, "l'ouest")];
        // lettre : centrée sur l'anneau, le haut vers l'extérieur
        let lettre = |d: Vec3| {
            let c = d * ((R0 + R1) / 2.0) + Vec3::Z * Z;
            (c, d.cross(Vec3::Z), d)
        };
        let lettre_quad = |d: Vec3| {
            let (c, u, v) = lettre(d);
            let h = 0.22;
            [c - u * h - v * h, c + u * h - v * h, c + u * h + v * h, c - u * h + v * h].map(ecran)
        };

        // ce que désigne un point : cellule du cube, sinon lettre, sinon anneau
        let visibles: Vec<_> = faces.iter().filter(|f| rot.transform_vector3(f.0).z > 1e-3).collect();
        enum Cible {
            Cellule(Vec3, Vec3, usize, usize),
            Lettre(usize),
            Anneau,
        }
        let cible = |p: egui::Pos2| -> Option<Cible> {
            let mut c = None;
            for f in &visibles {
                for i in 0..3 {
                    for j in 0..3 {
                        if dedans(p, &cellule(f.0, f.1, f.2, i, j)) {
                            c = Some(Cible::Cellule(f.0 + f.1 * (i as f32 - 1.0) + f.2 * (j as f32 - 1.0), f.0, i, j));
                        }
                    }
                }
            }
            c.or_else(|| cardinaux.iter().position(|x| dedans(p, &lettre_quad(x.1))).map(Cible::Lettre))
                .or_else(|| segments.iter().any(|q| dedans(p, &q.map(ecran))).then_some(Cible::Anneau))
        };
        let survol = resp.hover_pos().and_then(cible);
        // glisser : depuis l'anneau, autour de la verticale seulement
        if resp.dragged_by(PointerButton::Primary) {
            let d = resp.drag_delta();
            let depuis_anneau = ui.input(|i| i.pointer.press_origin()).and_then(cible).is_some_and(|c| matches!(c, Cible::Anneau | Cible::Lettre(_)));
            self.camera.orbit(d.x, if depuis_anneau { 0.0 } else { d.y });
        }

        let painter = ui.painter_at(zone);
        let encre = Color32::from_gray(70);
        let bleu = Color32::from_rgb(40, 110, 210);
        let anneau_survole = matches!(survol, Some(Cible::Anneau));
        let profondeur = |q: &[Vec3; 4]| rot.transform_vector3((q[0] + q[2]) / 2.0).z;
        let tracer_anneau = |devant: bool| {
            let fond = if anneau_survole { Color32::from_rgba_unmultiplied(170, 200, 240, 220) } else { Color32::from_rgba_unmultiplied(225, 225, 225, 210) };
            for q in segments.iter().filter(|q| (profondeur(q) > 0.0) == devant) {
                painter.add(egui::Shape::convex_polygon(q.map(ecran).to_vec(), fond, Stroke::NONE));
                // bords intérieur et extérieur
                painter.line_segment([ecran(q[0]), ecran(q[3])], Stroke::new(1.0, encre));
                painter.line_segment([ecran(q[1]), ecran(q[2])], Stroke::new(1.0, encre));
            }
            // lettres imprimées sur le dessus de l'anneau : illisibles (en miroir) vues
            // d'en dessous, écrasées vues par la tranche
            let dessus_visible = rot.transform_vector3(Vec3::Z).z > 0.08;
            for (i, (nom, d, _)) in cardinaux.iter().enumerate() {
                let (c, u, v) = lettre(*d);
                if !dessus_visible || (rot.transform_vector3(c).z > 0.0) != devant {
                    continue;
                }
                let couleur = if matches!(survol, Some(Cible::Lettre(j)) if j == i) { bleu } else if *nom == "N" { Color32::from_rgb(200, 40, 40) } else { encre };
                painter.add(texte_sur_face(nom, c, u, v, &ecran, couleur, 0.32));
            }
        };
        // anneau derrière le cube, cube, anneau devant
        tracer_anneau(false);
        for f in &visibles {
            let coins = [f.0 - f.1 - f.2, f.0 + f.1 - f.2, f.0 + f.1 + f.2, f.0 - f.1 + f.2].map(ecran);
            let face = rot.transform_vector3(f.0).z;
            let gris = (215.0 + 30.0 * face) as u8;
            painter.add(egui::Shape::convex_polygon(coins.to_vec(), Color32::from_rgba_unmultiplied(gris, gris, gris, 235), Stroke::new(1.0, encre)));
            if let Some(Cible::Cellule(_, n, i, j)) = survol.as_ref().filter(|c| matches!(c, Cible::Cellule(_, n, _, _) if *n == f.0)) {
                painter.add(egui::Shape::convex_polygon(cellule(*n, f.1, f.2, *i, *j).to_vec(), Color32::from_rgba_unmultiplied(80, 150, 230, 170), Stroke::NONE));
            }
            // nom imprimé sur la face : glyphes de la police (texte::ligne) posés dans le
            // plan de la face (u vers la droite, v vers le haut vu de l'extérieur), puis
            // projetés avec le cube ; caché sur une face presque de profil
            if face > 0.15 {
                painter.add(texte_sur_face(f.3, f.0, f.1, f.2, &ecran, encre, 0.34));
            }
        }
        tracer_anneau(true);

        let info = match &survol {
            Some(Cible::Cellule(d, ..)) => {
                let n = |x: f32, pos: &str, neg: &str| if x > 0.5 { pos.to_string() } else if x < -0.5 { neg.to_string() } else { String::new() };
                let t = [n(d.z, "haut", "bas"), n(-d.y, "avant", "arrière"), n(d.x, "droite", "gauche")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ");
                Some(format!("Vue depuis : {t}"))
            }
            Some(Cible::Lettre(i)) => Some(format!("Vue depuis {}", cardinaux[*i].2)),
            Some(Cible::Anneau) => Some("Glisser pour tourner autour de la verticale".to_string()),
            None => None,
        };
        let clic = resp.clicked();
        if let Some(t) = info {
            resp.on_hover_text(t);
        }
        if !clic {
            return;
        }
        match survol {
            Some(Cible::Cellule(d, ..)) => {
                // regarder depuis d vers le centre ; dessus et dessous : nord en haut
                let d = d.normalize();
                self.camera.pitch = d.z.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
                self.camera.yaw = if d.truncate().length() < 1e-4 { -std::f32::consts::FRAC_PI_2 } else { d.y.atan2(d.x) };
            }
            // depuis un point cardinal, inclinaison gardée
            Some(Cible::Lettre(i)) => self.camera.yaw = cardinaux[i].1.y.atan2(cardinaux[i].1.x),
            _ => {}
        }
    }

    /// Arborescence du projet (panneau de droite, comme dans Revit).
    pub(super) fn left_panel(&mut self, ui: &mut egui::Ui) {
        let nom = self.scene.as_ref().map(|s| s.file_name.clone()).unwrap_or_default();
        let titre = if nom.is_empty() { "Arborescence du projet".to_string() } else { format!("Arborescence du projet - {nom}") };
        self.entete_panneau(ui, Panneau::Arborescence, &titre);
        let a_nav = self.scene.as_ref().is_some_and(|s| s.navigateur.is_some());
        let onglets: &[LeftTab] = match (a_nav, self.options.navigation_ifc) {
            (false, _) => &LeftTab::IFC,
            (true, true) => &LeftTab::REVIT,
            (true, false) => &LeftTab::REVIT[..LeftTab::REVIT.len() - 2],
        };
        if !onglets.contains(&self.left_tab) {
            self.left_tab = onglets[0];
        }
        // barre d'icônes : une section ou un mode de l'arborescence chacune
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for &o in onglets {
                if o == LeftTab::Tree && a_nav {
                    ui.separator();
                }
                let (r, resp) = ui.allocate_exact_size(egui::vec2(25.0, 24.0), Sense::click());
                let actif = self.left_tab == o;
                let v = ui.visuals();
                let fond = if actif { v.selection.bg_fill } else if resp.hovered() { v.widgets.hovered.bg_fill } else { Color32::TRANSPARENT };
                let encre = if actif { v.selection.stroke.color } else { v.widgets.inactive.fg_stroke.color };
                ui.painter().rect_filled(r, 3.0, fond);
                icone(ui.painter(), r.shrink2(egui::vec2(4.5, 4.0)), o, encre);
                if resp.on_hover_text(o.libelle()).clicked() {
                    self.left_tab = o;
                }
            }
        });
        ui.label(RichText::new(self.left_tab.libelle()).small().weak());
        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Rechercher"));
        ui.separator();
        if self.scene.is_none() {
            ui.label(RichText::new("Aucun modèle chargé").weak());
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if self.left_tab.du_navigateur() {
                self.navigateur_ui(ui, self.left_tab);
                return;
            }
            if self.left_tab == LeftTab::Materiaux {
                self.materiaux_ui(ui);
                return;
            }
            if !self.filter.trim().is_empty() {
                self.filtered_list(ui);
                return;
            }
            match self.left_tab {
                LeftTab::Classes => self.class_list(ui),
                _ => {
                    let roots = self.tree_roots();
                    for r in roots {
                        self.tree_node(ui, r, 0);
                    }
                }
            }
        });
        self.reveal = None;
    }

}
