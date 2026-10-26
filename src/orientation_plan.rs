//! Repère d'orientation plat des plans : la caméra tourne sans déplacer sa cible.
use crate::camera::Camera;
use eframe::egui::{self, Align2, Color32, PointerButton, Sense, Stroke, pos2, vec2};
use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, TAU};

/// Rayon de l'anneau, rayon des lettres N/E/S/O et taille de la zone (anneau puis
/// une ligne de boutons).
const RAYON: f32 = 50.0;
const RAYON_CARDINAUX: f32 = 41.0;
const LARGEUR: f32 = 124.0;
const HAUTEUR_ANNEAU: f32 = 108.0;
/// Aimantation du glisser : multiples de 15°, à 2° près (Maj : rotation libre).
const PAS_AIMANT: f32 = 15.0;
const TOLERANCE_AIMANT: f32 = 2.0;

pub fn zone(vue: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(pos2(vue.right() - LARGEUR - 8.0, vue.top() + 8.0), vec2(LARGEUR, HAUTEUR_ANNEAU + 52.0))
}

fn centre(zone: egui::Rect) -> egui::Pos2 {
    pos2(zone.center().x, zone.top() + HAUTEUR_ANNEAU * 0.5)
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
    /// « Aligner » cliqué : le prochain clic dans la vue choisit l'élément à mettre à
    /// l'horizontale.
    pub aligner: bool,
}

/// Repère plat : anneau à glisser (aimanté tous les 15°, Maj : libre), N/E/S/O pour
/// mettre une direction en haut, PLAN pour l'orientation initiale, −90° / +90°,
/// angle saisi, et Aligner (sur un élément). `alignement` : mode Aligner en cours.
pub fn afficher(ui: &mut egui::Ui, vue: egui::Rect, camera: &mut Camera, origine: f32, alignement: bool) -> Reponse {
    let zone = zone(vue);
    let anneau = egui::Rect::from_min_size(zone.min, vec2(LARGEUR, HAUTEUR_ANNEAU));
    let centre = centre(zone);
    let id = ui.make_persistent_id("orientation_plan");
    let resp = ui.interact(anneau, id, Sense::click_and_drag());
    let avant = camera.yaw;
    let mut reponse = Reponse::default();
    let pointer = resp.interact_pointer_pos();
    // angle brut accumulé pendant le glisser, l'affichage étant aimanté
    let brut_id = id.with("brut");
    if resp.drag_started_by(PointerButton::Primary) {
        ui.data_mut(|d| d.insert_temp(brut_id, angle(camera)));
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let Some(position) = pointer {
            let precedent = position - ui.input(|i| i.pointer.delta());
            let a = position - centre;
            let b = precedent - centre;
            if a.length() > 12.0 && b.length() > 12.0 {
                let delta = ecart(a.angle(), b.angle()) * camera.pitch.signum();
                let brut = ui.data(|d| d.get_temp::<f32>(brut_id)).unwrap_or_else(|| angle(camera)) + delta;
                ui.data_mut(|d| d.insert_temp(brut_id, brut));
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
    let cible = survol.and_then(|p| cardinaux.iter().position(|(_, v, _)| p.distance(direction(*v, RAYON_CARDINAUX)) < 11.0));
    let sur_plan = |p: egui::Pos2| (p - centre).abs().x < 17.0 && (p - centre).abs().y < 10.0;
    if resp.clicked() {
        if let Some(i) = cible {
            orienter(camera, cardinaux[i].2);
        } else if pointer.is_some_and(sur_plan) {
            camera.yaw = origine;
        }
    }
    let p = ui.painter_at(zone);
    let encre = ui.visuals().text_color();
    let bleu = ui.visuals().selection.stroke.color;
    p.circle_filled(centre, RAYON, Color32::from_rgba_unmultiplied(248, 249, 251, 235));
    p.circle_stroke(centre, RAYON, Stroke::new(1.0, encre));
    // graduations tous les 15°, plus longues tous les 90°, tournant avec le plan
    for k in 0..24 {
        let a = k as f32 * TAU / 24.0;
        let d = (direction(Vec3::new(a.cos(), a.sin(), 0.0), 1.0) - centre).normalized();
        let long = if k % 6 == 0 { 7.0 } else { 3.5 };
        p.line_segment([centre + d * (RAYON - long), centre + d * RAYON], Stroke::new(1.0, Color32::from_gray(130)));
    }
    for (i, (nom, v, _)) in cardinaux.iter().enumerate() {
        let point = direction(*v, RAYON_CARDINAUX - 6.0);
        if cible == Some(i) {
            p.circle_filled(point, 9.0, ui.visuals().selection.bg_fill);
        }
        p.text(point, Align2::CENTER_CENTER, nom, egui::FontId::proportional(12.0), if *nom == "N" { Color32::from_rgb(200, 45, 45) } else { encre });
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
    resp.on_hover_cursor(if cible.is_some() || survol_plan { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Grab }).on_hover_text(
        "Glisser l'anneau pour tourner le plan (aimanté tous les 15°, Maj : libre) · N/E/S/O : direction en haut · PLAN : orientation initiale",
    );
    let mut degres = angle(camera).to_degrees();
    ui.scope_builder(
        egui::UiBuilder::new().id_salt("angle_plan").max_rect(egui::Rect::from_min_size(zone.min + vec2(0.0, HAUTEUR_ANNEAU + 2.0), vec2(LARGEUR, 50.0))),
        |ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            ui.horizontal(|ui| {
                if ui.small_button("−90°").on_hover_text("Tourner de 90° vers la gauche").clicked() {
                    orienter(camera, angle(camera) - FRAC_PI_2);
                }
                let r = ui.add_sized(
                    [LARGEUR - 84.0, 18.0],
                    egui::DragValue::new(&mut degres).speed(1.0).suffix("°").max_decimals(1),
                );
                if r.on_hover_text("Angle du plan par rapport au nord du modèle").changed() {
                    orienter(camera, degres.to_radians());
                }
                if ui.small_button("+90°").on_hover_text("Tourner de 90° vers la droite").clicked() {
                    orienter(camera, angle(camera) + FRAC_PI_2);
                }
            });
            let texte = if alignement { "Cliquer un élément…" } else { "Aligner sur…" };
            if ui
                .add_sized([LARGEUR, 20.0], egui::Button::new(egui::RichText::new(texte).size(11.0)).selected(alignement))
                .on_hover_text("Cliquer ensuite un mur ou un élément du plan : sa direction principale devient horizontale (Échap : annuler)")
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
                    afficher(ui, vue, camera, -FRAC_PI_2, false);
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
        let est = centre + vec2(RAYON_CARDINAUX - 6.0, 0.0);
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
