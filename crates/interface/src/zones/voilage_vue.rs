//! Voilage de la vue : ce qui se dessine par-dessus (cube d'orientation et projection en
//! 3D, repère d'orientation du plan en 2D). Le bandeau de dessin est dans `outils/bandeau.rs`,
//! les messages dans `socle/messages.rs`.
use crate::*;

impl Application {
    /// Voilage de la zone de vue, tracé après toutes les superpositions de l'outil actif.
    pub(crate) fn voilage_vue(&mut self, ui: &mut egui::Ui, rect: egui::Rect, repere_plan: bool) {
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
            let r = moteur3d::orientation_plan::afficher(ui, rect, &mut self.camera, origine, self.alignement_plan, &positions, nord);
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

    /// Choix de la projection sous le cube : perspective, verticales droites (deux
    /// points), orthographique.
    pub(crate) fn choix_projection(&mut self, ui: &mut egui::Ui, vue: egui::Rect) {
        use moteur3d::camera::Projection;
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

    /// Cube d'orientation (vue 3D), dans le coin haut droit de la vue : clic sur une face,
    /// une arête ou un coin pour regarder le modèle depuis cette direction ; glisser pour
    /// tourner autour. Boussole en anneau à sa base : clic sur N, E, S ou O pour regarder
    /// depuis ce point cardinal (inclinaison gardée) ; glisser l'anneau pour tourner autour
    /// de la verticale.
    pub(crate) fn cube_orientation(&mut self, ui: &mut egui::Ui, vue: egui::Rect) {
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
}
