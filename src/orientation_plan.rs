//! Repère d'orientation plat des plans : la caméra tourne sans déplacer sa cible.
use crate::camera::Camera;
use eframe::egui::{self, Align2, Color32, PointerButton, Sense, Stroke, pos2, vec2};
use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, TAU};

pub fn zone(vue: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(
        pos2(vue.right() - 164.0, vue.top() + 8.0),
        vec2(156.0, 194.0),
    )
}

fn angle(camera: &Camera) -> f32 {
    (camera.yaw
        + FRAC_PI_2
        + if camera.pitch < 0.0 {
            std::f32::consts::PI
        } else {
            0.0
        })
    .rem_euclid(TAU)
}

fn orienter(camera: &mut Camera, angle: f32) {
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

/// Retourne vrai lorsque l'orientation a changé. Les autres propriétés de caméra restent intactes.
pub fn afficher(ui: &mut egui::Ui, vue: egui::Rect, camera: &mut Camera, origine: f32) -> bool {
    let zone = zone(vue);
    let cercle = egui::Rect::from_min_size(zone.min, vec2(156.0, 150.0));
    let centre = cercle.center();
    let resp = ui.interact(
        cercle,
        ui.make_persistent_id("orientation_plan"),
        Sense::click_and_drag(),
    );
    let avant = camera.yaw;
    let pointer = resp.interact_pointer_pos();
    if resp.dragged_by(PointerButton::Primary) {
        if let Some(position) = pointer {
            let precedent = position - ui.input(|i| i.pointer.delta());
            let a = position - centre;
            let b = precedent - centre;
            if a.length() > 12.0 && b.length() > 12.0 {
                let delta = ecart(a.angle(), b.angle()) * camera.pitch.signum();
                orienter(camera, angle(camera) + delta);
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
    let cible = survol.and_then(|p| {
        cardinaux
            .iter()
            .position(|(_, v, _)| p.distance(direction(*v, 59.0)) < 13.0)
    });
    if resp.clicked() {
        if let Some(i) = cible {
            orienter(camera, cardinaux[i].2);
        } else if pointer.is_some_and(|p| p.distance(centre) < 25.0) {
            camera.yaw = origine;
        }
    }
    let p = ui.painter_at(zone);
    let encre = ui.visuals().text_color();
    let bleu = ui.visuals().selection.stroke.color;
    p.circle_filled(
        centre,
        70.0,
        Color32::from_rgba_unmultiplied(248, 249, 251, 235),
    );
    p.circle_stroke(centre, 70.0, Stroke::new(1.0, encre));
    p.circle_stroke(centre, 48.0, Stroke::new(1.0, Color32::from_gray(170)));
    for k in 0..24 {
        let a = k as f32 * TAU / 24.0;
        let d = vec2(a.cos(), a.sin());
        p.line_segment(
            [centre + d * 66.0, centre + d * 70.0],
            Stroke::new(1.0, Color32::from_gray(140)),
        );
    }
    for (i, (nom, v, _)) in cardinaux.iter().enumerate() {
        let point = direction(*v, 59.0);
        if cible == Some(i) {
            p.circle_filled(point, 12.0, ui.visuals().selection.bg_fill);
        }
        p.text(
            point,
            Align2::CENTER_CENTER,
            nom,
            egui::FontId::proportional(14.0),
            if *nom == "N" {
                Color32::from_rgb(200, 45, 45)
            } else {
                encre
            },
        );
    }
    // Axes du modèle projetés dans le même repère que le plan.
    for (nom, v, couleur) in [
        ("X", Vec3::X, Color32::from_rgb(190, 55, 55)),
        ("Y", Vec3::Y, Color32::from_rgb(40, 135, 75)),
    ] {
        let bout = direction(v, 40.0);
        let d = (bout - centre).normalized();
        let n = vec2(-d.y, d.x);
        p.line_segment([centre, bout], Stroke::new(1.8, couleur));
        p.line_segment([bout, bout - d * 6.0 + n * 3.0], Stroke::new(1.8, couleur));
        p.line_segment([bout, bout - d * 6.0 - n * 3.0], Stroke::new(1.8, couleur));
        p.text(
            bout + n * 9.0,
            Align2::CENTER_CENTER,
            nom,
            egui::FontId::proportional(11.0),
            couleur,
        );
    }
    let reset = egui::Rect::from_center_size(centre, vec2(42.0, 24.0));
    p.rect_filled(reset, 3.0, ui.visuals().widgets.inactive.bg_fill);
    p.rect_stroke(reset, 3.0, Stroke::new(1.0, bleu), egui::StrokeKind::Inside);
    p.text(
        centre,
        Align2::CENTER_CENTER,
        "PLAN",
        egui::FontId::proportional(11.0),
        encre,
    );
    resp.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text(
        "Glisser pour tourner le plan · N/E/S/O : direction en haut · PLAN : orientation initiale",
    );
    let mut degres = angle(camera).to_degrees();
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("angle_plan")
            .max_rect(egui::Rect::from_min_size(
                zone.min + vec2(0.0, 154.0),
                vec2(156.0, 26.0),
            )),
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                if ui
                    .small_button("−90°")
                    .on_hover_text("Tourner de 90° vers la gauche")
                    .clicked()
                {
                    orienter(camera, angle(camera) - FRAC_PI_2);
                }
                if ui
                    .add(
                        egui::DragValue::new(&mut degres)
                            .speed(1.0)
                            .suffix("°")
                            .max_decimals(1),
                    )
                    .on_hover_text("Angle du plan par rapport au nord du modèle")
                    .changed()
                {
                    orienter(camera, degres.to_radians());
                }
                if ui
                    .small_button("+90°")
                    .on_hover_text("Tourner de 90° vers la droite")
                    .clicked()
                {
                    orienter(camera, angle(camera) + FRAC_PI_2);
                }
            });
        },
    );
    if camera.yaw != avant {
        ui.ctx().request_repaint();
        true
    } else {
        false
    }
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
                    afficher(ui, vue, camera, -FRAC_PI_2);
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
        let centre = zone(vue).min + vec2(78.0, 75.0);
        image(&ctx, &mut camera, vec![]);
        image(&ctx, &mut camera, vec![]);
        let est = centre + vec2(59.0, 0.0);
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
        let haut = centre + vec2(0.0, -68.0);
        let droite = centre + vec2(68.0, 0.0);
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
