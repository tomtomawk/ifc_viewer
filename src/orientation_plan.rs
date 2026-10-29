//! Repère d'orientation plat des plans : la caméra tourne sans déplacer sa cible.
use crate::camera::Camera;
use eframe::egui::{self, Align2, Color32, PointerButton, Sense, Stroke, pos2, vec2};
use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, TAU};

/// Anneau (rayons extérieur et intérieur), rayon des lettres N/E/S/O, rayon des
/// flèches de rotation autour de l'anneau, taille de la zone (anneau puis deux lignes
/// de boutons).
const RAYON: f32 = 50.0;
const RAYON_INTERIEUR: f32 = 34.0;
const RAYON_CARDINAUX: f32 = 42.0;
const RAYON_FLECHES: f32 = 58.0;
const LARGEUR: f32 = 132.0;
const HAUTEUR_ANNEAU: f32 = 124.0;
/// Demi-ouverture des flèches de rotation (degrés), de part et d'autre de l'horizontale.
const OUVERTURE_FLECHES: f32 = 28.0;
/// Aimantation du glisser : multiples de 15°, à 2° près (Maj : rotation libre).
const PAS_AIMANT: f32 = 15.0;
const TOLERANCE_AIMANT: f32 = 2.0;

pub fn zone(vue: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(pos2(vue.right() - LARGEUR - 8.0, vue.top() + 8.0), vec2(LARGEUR, HAUTEUR_ANNEAU + 52.0))
}

fn centre(zone: egui::Rect) -> egui::Pos2 {
    pos2(zone.center().x, zone.top() + HAUTEUR_ANNEAU * 0.5)
}

/// Flèche de rotation sous le point (écran) : −1 à gauche (sens antihoraire), +1 à droite.
fn fleche(p: egui::Pos2, centre: egui::Pos2) -> Option<f32> {
    let d = p - centre;
    if !(RAYON + 1.0..=RAYON_FLECHES + 8.0).contains(&d.length()) {
        return None;
    }
    let a = d.y.atan2(d.x).to_degrees();
    if a.abs() <= OUVERTURE_FLECHES + 6.0 {
        Some(1.0)
    } else if 180.0 - a.abs() <= OUVERTURE_FLECHES + 6.0 {
        Some(-1.0)
    } else {
        None
    }
}

/// Arc fléché concentrique à l'anneau, de `de` à `vers` (degrés écran, y vers le bas),
/// pointe au bout `vers`.
fn arc_fleche(p: &egui::Painter, centre: egui::Pos2, de: f32, vers: f32, trait_: Stroke) {
    let n = 16;
    let point = |a: f32| centre + vec2(a.to_radians().cos(), a.to_radians().sin()) * RAYON_FLECHES;
    let pts: Vec<egui::Pos2> = (0..=n).map(|k| point(de + (vers - de) * k as f32 / n as f32)).collect();
    p.add(egui::Shape::line(pts, trait_));
    let bout = point(vers);
    let tangente = (point(vers) - point(vers - (vers - de).signum() * 2.0)).normalized();
    let normale = vec2(-tangente.y, tangente.x);
    p.add(egui::Shape::convex_polygon(
        vec![bout + tangente * 5.0, bout - tangente * 3.0 + normale * 5.0, bout - tangente * 3.0 - normale * 5.0],
        trait_.color,
        Stroke::NONE,
    ));
}

pub fn angle(camera: &Camera) -> f32 {
    (camera.yaw
        + FRAC_PI_2
        + if camera.pitch < 0.0 {
            std::f32::consts::PI
        } else {
            0.0
        })
    .rem_euclid(TAU)
}

pub fn orienter(camera: &mut Camera, angle: f32) {
    if angle.is_finite() {
        camera.yaw = angle.rem_euclid(TAU)
            - FRAC_PI_2
            - if camera.pitch < 0.0 {
                std::f32::consts::PI
            } else {
                0.0
            };
    }
}

fn ecart(a: f32, b: f32) -> f32 {
    (a - b + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI
}

/// Angle aimanté au multiple de 15° le plus proche s'il est à moins de 2°.
fn aimanter(a: f32) -> f32 {
    let pas = PAS_AIMANT.to_radians();
    let rond = (a / pas).round() * pas;
    if ecart(a, rond).abs() < TOLERANCE_AIMANT.to_radians() { rond } else { a }
}

/// Ce que l'utilisateur a fait sur le repère.
#[derive(Default)]
pub struct Reponse {
    /// L'orientation de la caméra a changé.
    pub change: bool,
    /// « Aligner » cliqué : le prochain clic dans la vue choisit l'arête à mettre à
    /// l'horizontale.
    pub aligner: bool,
    /// « Enregistrer la position » cliqué : mémoriser l'orientation en cours.
    pub enregistrer: bool,
    /// Position enregistrée à supprimer (indice).
    pub supprimer: Option<usize>,
}

/// Repère plat : anneau à glisser (aimanté tous les 15°, Maj : libre), N/E/S/O pour
/// mettre une direction en haut, PLAN pour l'orientation initiale, flèches autour de
/// l'anneau pour tourner de 90°, angle saisi, positions enregistrées (`positions` :
/// nom et angle) et Aligner (sur une arête). `alignement` : mode Aligner en cours.
pub fn afficher(ui: &mut egui::Ui, vue: egui::Rect, camera: &mut Camera, origine: f32, alignement: bool, positions: &[(String, f32)]) -> Reponse {
    let zone = zone(vue);
    let anneau = egui::Rect::from_min_size(zone.min, vec2(LARGEUR, HAUTEUR_ANNEAU));
    let centre = centre(zone);
    let id = ui.make_persistent_id("orientation_plan");
    let resp = ui.interact(anneau, id, Sense::click_and_drag());
    let avant = camera.yaw;
    let mut reponse = Reponse::default();
    let pointer = resp.interact_pointer_pos();
    let sur_anneau = |p: egui::Pos2| (RAYON_INTERIEUR - 2.0..=RAYON + 2.0).contains(&(p - centre).length());
    // angle brut accumulé pendant le glisser (None : glisser commencé hors de l'anneau),
    // l'affichage étant aimanté
    let brut_id = id.with("brut");
    if resp.drag_started_by(PointerButton::Primary) {
        let depart = ui.input(|i| i.pointer.press_origin()).is_some_and(sur_anneau);
        ui.data_mut(|d| d.insert_temp(brut_id, depart.then(|| angle(camera))));
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let (Some(position), Some(brut)) = (pointer, ui.data(|d| d.get_temp::<Option<f32>>(brut_id)).flatten()) {
            let precedent = position - ui.input(|i| i.pointer.delta());
            let a = position - centre;
            let b = precedent - centre;
            if a.length() > 12.0 && b.length() > 12.0 {
                let brut = brut + ecart(a.angle(), b.angle()) * camera.pitch.signum();
                ui.data_mut(|d| d.insert_temp(brut_id, Some(brut)));
                let libre = ui.input(|i| i.modifiers.shift);
                orienter(camera, if libre { brut } else { aimanter(brut) });
            }
        }
    }
    let rotation = camera.view();
    let direction = |v: Vec3, rayon: f32| {
        let q = rotation.transform_vector3(v);
        centre + vec2(q.x, -q.y).normalized() * rayon
    };
    let cardinaux = [
        ("N", Vec3::Y, 0.0),
        ("E", Vec3::X, -FRAC_PI_2),
        ("S", Vec3::NEG_Y, std::f32::consts::PI),
        ("O", Vec3::NEG_X, FRAC_PI_2),
    ];
    let survol = resp.hover_pos();
    let cible = survol.and_then(|p| cardinaux.iter().position(|(_, v, _)| p.distance(direction(*v, RAYON_CARDINAUX)) < 9.0));
    let fleche_survolee = survol.and_then(|p| fleche(p, centre));
    let sur_plan = |p: egui::Pos2| (p - centre).abs().x < 17.0 && (p - centre).abs().y < 10.0;
    if resp.clicked() {
        if let Some(i) = cible {
            orienter(camera, cardinaux[i].2);
        } else if let Some(sens) = pointer.and_then(|p| fleche(p, centre)) {
            orienter(camera, angle(camera) + sens * FRAC_PI_2);
        } else if pointer.is_some_and(sur_plan) {
            camera.yaw = origine;
        }
    }
    let p = ui.painter_at(zone);
    let encre = ui.visuals().text_color();
    let bleu = ui.visuals().selection.stroke.color;
    // anneau : bande entre les deux rayons, le plan reste visible au centre
    let milieu = (RAYON + RAYON_INTERIEUR) * 0.5;
    let survol_anneau = survol.is_some_and(sur_anneau) && cible.is_none();
    let fond = if survol_anneau || resp.dragged() { Color32::from_rgba_unmultiplied(214, 226, 244, 245) } else { Color32::from_rgba_unmultiplied(232, 235, 240, 240) };
    p.circle_stroke(centre, milieu, Stroke::new(RAYON - RAYON_INTERIEUR, fond));
    p.circle_stroke(centre, RAYON, Stroke::new(1.0, encre));
    p.circle_stroke(centre, RAYON_INTERIEUR, Stroke::new(1.0, Color32::from_gray(150)));
    // graduations tous les 15° sur le bord extérieur, tournant avec le plan
    for k in 0..24 {
        if k % 6 == 0 {
            continue;
        }
        let a = k as f32 * TAU / 24.0;
        let d = (direction(Vec3::new(a.cos(), a.sin(), 0.0), 1.0) - centre).normalized();
        let long = if k % 3 == 0 { 5.0 } else { 3.0 };
        p.line_segment([centre + d * (RAYON - long), centre + d * RAYON], Stroke::new(1.0, Color32::from_gray(130)));
    }
    for (i, (nom, v, _)) in cardinaux.iter().enumerate() {
        let point = direction(*v, RAYON_CARDINAUX);
        if cible == Some(i) {
            p.circle_filled(point, 8.0, ui.visuals().selection.bg_fill);
        }
        p.text(point, Align2::CENTER_CENTER, nom, egui::FontId::proportional(12.0), if *nom == "N" { Color32::from_rgb(200, 45, 45) } else { encre });
    }
    // flèches concentriques : quart de tour antihoraire à gauche, horaire à droite
    for (sens, de, vers) in [(-1.0, 180.0 + OUVERTURE_FLECHES, 180.0 - OUVERTURE_FLECHES), (1.0, -OUVERTURE_FLECHES, OUVERTURE_FLECHES)] {
        let couleur = if fleche_survolee == Some(sens) { bleu } else { Color32::from_gray(110) };
        arc_fleche(&p, centre, de, vers, Stroke::new(if fleche_survolee == Some(sens) { 3.0 } else { 2.2 }, couleur));
    }
    // axes du modèle projetés dans le même repère que le plan
    for (v, couleur) in [(Vec3::X, Color32::from_rgb(190, 55, 55)), (Vec3::Y, Color32::from_rgb(40, 135, 75))] {
        let bout = direction(v, 24.0);
        let d = (bout - centre).normalized();
        let n = vec2(-d.y, d.x);
        p.line_segment([centre, bout], Stroke::new(1.5, couleur));
        p.line_segment([bout, bout - d * 5.0 + n * 2.5], Stroke::new(1.5, couleur));
        p.line_segment([bout, bout - d * 5.0 - n * 2.5], Stroke::new(1.5, couleur));
    }
    let reset = egui::Rect::from_center_size(centre, vec2(34.0, 18.0));
    let survol_plan = survol.is_some_and(sur_plan);
    p.rect_filled(reset, 3.0, if survol_plan { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill });
    p.rect_stroke(reset, 3.0, Stroke::new(1.0, bleu), egui::StrokeKind::Inside);
    p.text(centre, Align2::CENTER_CENTER, "PLAN", egui::FontId::proportional(10.0), encre);
    let curseur = if cible.is_some() || survol_plan || fleche_survolee.is_some() {
        egui::CursorIcon::PointingHand
    } else if survol_anneau {
        egui::CursorIcon::Grab
    } else {
        egui::CursorIcon::Default
    };
    let aide = match fleche_survolee {
        Some(s) if s < 0.0 => "Tourner le plan d'un quart de tour (sens antihoraire)",
        Some(_) => "Tourner le plan d'un quart de tour (sens horaire)",
        None => "Glisser l'anneau pour tourner le plan (aimanté tous les 15°, Maj : libre) · N/E/S/O : direction en haut · PLAN : orientation initiale",
    };
    resp.on_hover_cursor(curseur).on_hover_text(aide);
    let mut degres = angle(camera).to_degrees();
    ui.scope_builder(
        egui::UiBuilder::new().id_salt("angle_plan").max_rect(egui::Rect::from_min_size(zone.min + vec2(0.0, HAUTEUR_ANNEAU + 2.0), vec2(LARGEUR, 50.0))),
        |ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            ui.horizontal(|ui| {
                let r = ui.add_sized([LARGEUR - 66.0, 18.0], egui::DragValue::new(&mut degres).speed(1.0).suffix("°").max_decimals(1));
                if r.on_hover_text("Angle du plan par rapport au nord du modèle").changed() {
                    orienter(camera, degres.to_radians());
                }
                ui.menu_button(egui::RichText::new("Positions").size(11.0), |ui| {
                    if ui.button("Enregistrer la position").on_hover_text("Mémoriser l'orientation en cours du plan").clicked() {
                        reponse.enregistrer = true;
                        ui.close();
                    }
                    if !positions.is_empty() {
                        ui.separator();
                    }
                    for (i, (nom, a)) in positions.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.button(format!("{nom} · {:.1}°", a.to_degrees())).on_hover_text("Revenir à cette orientation").clicked() {
                                orienter(camera, *a);
                                ui.close();
                            }
                            if ui.small_button("Supprimer").clicked() {
                                reponse.supprimer = Some(i);
                                ui.close();
                            }
                        });
                    }
                })
                .response
                .on_hover_text("Orientations enregistrées du plan");
            });
            let texte = if alignement { "Viser une arête…" } else { "Aligner sur…" };
            if ui
                .add_sized([LARGEUR, 20.0], egui::Button::new(egui::RichText::new(texte).size(11.0)).selected(alignement))
                .on_hover_text("Viser ensuite une arête du plan (surlignée) et cliquer : elle devient horizontale (Échap : annuler)")
                .clicked()
            {
                reponse.aligner = true;
            }
        },
    );
    if camera.yaw != avant {
        ui.ctx().request_repaint();
        reponse.change = true;
    }
    reponse
}

/// Angle du plan qui met la direction `d` (plan horizontal du modèle) à
/// l'horizontale de l'écran, la plus proche de l'angle actuel `actuel`.
pub fn angle_pour_horizontale(camera: &Camera, d: glam::Vec2) -> Option<f32> {
    if d.length_squared() < 1e-12 {
        return None;
    }
    // angle θ : la direction (cos θ, sin θ) va à droite de l'écran (θ = 0 : X à droite)
    let base = d.y.atan2(d.x);
    let actuel = angle(camera);
    // la direction ou son opposée, la plus proche de l'orientation actuelle
    [base, base + std::f32::consts::PI]
        .into_iter()
        .min_by(|a, b| ecart(*a, actuel).abs().total_cmp(&ecart(*b, actuel).abs()))
        .map(|a| a.rem_euclid(TAU))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::MAX_PITCH;

    fn image(ctx: &egui::Context, camera: &mut Camera, events: Vec<egui::Event>) {
        let vue = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 600.0));
        let mut sortie = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(vue),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    afficher(ui, vue, camera, -FRAC_PI_2, false, &[]);
                });
            },
        );
        sortie.textures_delta.clear();
    }

    fn bouton(position: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn clic_cardinal_glissement_et_retour_initial() {
        let ctx = egui::Context::default();
        let mut camera = Camera {
            pitch: MAX_PITCH,
            yaw: -FRAC_PI_2,
            ortho: true,
            ..Default::default()
        };
        let initiale = camera.clone();
        let vue = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 600.0));
        let centre = super::centre(zone(vue));
        image(&ctx, &mut camera, vec![]);
        image(&ctx, &mut camera, vec![]);
        let est = centre + vec2(RAYON_CARDINAUX, 0.0);
        image(
            &ctx,
            &mut camera,
            vec![egui::Event::PointerMoved(est), bouton(est, true)],
        );
        image(&ctx, &mut camera, vec![bouton(est, false)]);
        assert!(
            ecart(angle(&camera), -FRAC_PI_2).abs() < 1e-5,
            "Clic E : {}",
            angle(&camera)
        );
        // Tourner l'anneau d'un quart de tour dans le sens horaire.
        let haut = centre + vec2(0.0, -(RAYON - 3.0));
        let droite = centre + vec2(RAYON - 3.0, 0.0);
        image(
            &ctx,
            &mut camera,
            vec![egui::Event::PointerMoved(haut), bouton(haut, true)],
        );
        image(&ctx, &mut camera, vec![egui::Event::PointerMoved(droite)]);
        image(&ctx, &mut camera, vec![bouton(droite, false)]);
        assert!(
            ecart(angle(&camera), 0.0).abs() < 1e-4,
            "Anneau : {}",
            angle(&camera)
        );
        // flèche de droite : un quart de tour de plus ; de gauche : retour
        let fleche_droite = centre + vec2(RAYON_FLECHES, 0.0);
        image(&ctx, &mut camera, vec![egui::Event::PointerMoved(fleche_droite), bouton(fleche_droite, true)]);
        image(&ctx, &mut camera, vec![bouton(fleche_droite, false)]);
        assert!(ecart(angle(&camera), FRAC_PI_2).abs() < 1e-4, "Flèche : {}", angle(&camera));
        let fleche_gauche = centre - vec2(RAYON_FLECHES, 0.0);
        image(&ctx, &mut camera, vec![egui::Event::PointerMoved(fleche_gauche), bouton(fleche_gauche, true)]);
        image(&ctx, &mut camera, vec![bouton(fleche_gauche, false)]);
        assert!(ecart(angle(&camera), 0.0).abs() < 1e-4, "Flèche : {}", angle(&camera));
        // glisser commencé au centre (hors de l'anneau) : pas de rotation
        let avant = camera.yaw;
        let pres = centre + vec2(14.0, 14.0);
        image(&ctx, &mut camera, vec![egui::Event::PointerMoved(pres), bouton(pres, true)]);
        image(&ctx, &mut camera, vec![egui::Event::PointerMoved(centre + vec2(-20.0, 14.0))]);
        image(&ctx, &mut camera, vec![bouton(centre + vec2(-20.0, 14.0), false)]);
        assert_eq!(camera.yaw, avant);
        orienter(&mut camera, 0.7);
        image(
            &ctx,
            &mut camera,
            vec![egui::Event::PointerMoved(centre), bouton(centre, true)],
        );
        image(&ctx, &mut camera, vec![bouton(centre, false)]);
        assert_eq!(camera.yaw, initiale.yaw);
        assert_eq!(camera.target, initiale.target);
        assert_eq!(camera.dist, initiale.dist);
        assert_eq!(camera.pitch, initiale.pitch);
    }

    #[test]
    fn aligner_met_la_direction_a_l_horizontale() {
        for pitch in [MAX_PITCH, -MAX_PITCH] {
            let mut camera = Camera { pitch, yaw: -FRAC_PI_2, ortho: true, ..Default::default() };
            for d in [glam::Vec2::new(1.0, 0.0), glam::Vec2::new(1.0, 1.0), glam::Vec2::new(-0.3, 2.0), glam::Vec2::new(0.0, -1.0)] {
                let a = angle_pour_horizontale(&camera, d).unwrap();
                orienter(&mut camera, a);
                let q = camera.view().transform_vector3(d.extend(0.0));
                assert!(q.y.abs() < 1e-4 * q.length(), "{pitch} {d:?} : {q:?}");
            }
        }
        assert!(angle_pour_horizontale(&Camera::default(), glam::Vec2::ZERO).is_none());
        assert!((aimanter(46.5_f32.to_radians()).to_degrees() - 45.0).abs() < 1e-3);
        assert!((aimanter(40.0_f32.to_radians()).to_degrees() - 40.0).abs() < 1e-3);
    }

    #[test]
    fn rotation_conserve_cadrage_et_projection() {
        for pitch in [MAX_PITCH, -MAX_PITCH] {
            let mut camera = Camera {
                pitch,
                yaw: -FRAC_PI_2,
                ortho: true,
                target: Vec3::new(12.0, 25.0, 3.0),
                ..Default::default()
            };
            let avant = camera.clone();
            for degres in [0.0_f32, 90.0, 37.5, -45.0, 720.0] {
                orienter(&mut camera, degres.to_radians());
                assert!(ecart(angle(&camera), degres.to_radians()).abs() < 1e-5);
                assert_eq!(camera.target, avant.target);
                assert_eq!(camera.pitch, avant.pitch);
                assert_eq!(camera.dist, avant.dist);
                assert_eq!(camera.ortho, avant.ortho);
                assert!(camera.view_proj(1.5).is_finite());
            }
            let yaw = camera.yaw;
            orienter(&mut camera, f32::NAN);
            assert_eq!(camera.yaw, yaw);
        }
        assert!(
            ecart(179_f32.to_radians(), -179_f32.to_radians())
                .to_degrees()
                .abs()
                < 2.01
        );
    }
}
