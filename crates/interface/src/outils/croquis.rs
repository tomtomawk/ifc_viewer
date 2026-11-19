//! Présentation commune des croquis des palettes de création.
use crate::*;

pub(crate) const BLEU: Color32 = Color32::from_rgb(18, 158, 205);

pub(crate) fn cadre(ui: &mut egui::Ui, contenu: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            contenu(ui);
        });
}

/// Croquis vertical à gauche, lignes de réglage des limites à droite.
pub(crate) struct CroquisVertical {
    origine: egui::Pos2,
    centres: [f32; 2],
    champs: f32,
}

impl CroquisVertical {
    pub(crate) fn lignes(ui: &mut egui::Ui, mut limite: impl FnMut(&mut egui::Ui, bool)) -> Self {
        let mut croquis = Self {
            origine: ui.cursor().min,
            centres: [0.0; 2],
            champs: 0.0,
        };
        ui.add_space(14.0);
        ui.horizontal_top(|ui| {
            ui.add_space(64.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 16.0;
                for haut in [true, false] {
                    let ligne = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add_sized(
                            [30.0, 20.0],
                            egui::Label::new(
                                RichText::new(if haut { "Haut" } else { "Bas" }).color(BLEU),
                            ),
                        );
                        limite(ui, haut);
                    });
                    croquis.champs = ligne.response.rect.left();
                    croquis.centres[usize::from(!haut)] = ligne.response.rect.center().y;
                }
            });
        });
        ui.add_space(14.0);
        croquis
    }

    pub(crate) fn references(&self) -> [f32; 2] {
        self.centres.map(|y| y + 3.0)
    }

    pub(crate) fn peindre(
        &self,
        ui: &egui::Ui,
        [haut, bas]: [f32; 2],
        largeur: f32,
        mesure: Option<f64>,
    ) {
        use egui::{Align2, FontId, Stroke, pos2, vec2};
        let p = ui.painter();
        let bord = Stroke::new(1.0, BLEU);
        let profondeur = vec2(4.5, -3.0);
        let x1 = self.origine.x + 44.0;
        let x0 = x1 - largeur;
        let (a, h, b, bb) = (pos2(x0, haut), pos2(x1, haut), pos2(x1, bas), pos2(x0, bas));
        p.add(egui::Shape::convex_polygon(
            vec![a, h, b, bb],
            Color32::from_rgb(246, 251, 253),
            bord,
        ));
        p.add(egui::Shape::convex_polygon(
            vec![h, h + profondeur, b + profondeur, b],
            Color32::from_rgb(222, 242, 249),
            bord,
        ));
        p.add(egui::Shape::convex_polygon(
            vec![a, a + profondeur, h + profondeur, h],
            Color32::from_rgb(236, 248, 252),
            bord,
        ));
        let rappel = Stroke::new(0.8, BLEU.gamma_multiply(0.8));
        let reference = self.references();
        if (haut - reference[0]).abs() > 0.01 || (bas - reference[1]).abs() > 0.01 {
            let r = egui::Rect::from_min_max(pos2(x0, reference[0]), pos2(x1, reference[1]));
            p.add(egui::Shape::dashed_line(
                &[
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                    r.left_top(),
                ],
                rappel,
                2.5,
                2.5,
            ));
        }
        for y in self.centres {
            let debut = pos2(x1 + profondeur.x, y);
            p.add(egui::Shape::dashed_line(
                &[debut, pos2(self.champs - 3.0, y)],
                rappel,
                2.5,
                2.5,
            ));
            p.circle_filled(debut, 1.5, BLEU);
        }
        if let Some(mesure) = mesure {
            p.text(
                pos2((x0 + x1) * 0.5, (haut + bas) * 0.5),
                Align2::CENTER_CENTER,
                modele::unite::unite().afficher(mesure),
                FontId::proportional(8.0),
                Color32::from_gray(55),
            );
        }
    }
}

/// Le schéma laisse de la place aux champs, quelle que soit la valeur réelle.
pub(crate) fn decalage_vertical(valeur: f64) -> f32 {
    if valeur > 0.0 {
        -10.0
    } else if valeur < 0.0 {
        10.0
    } else {
        0.0
    }
}

pub(crate) fn champ_niveau(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    niveaux: &[modele::Niveau],
    valeur: &mut Option<String>,
    defaut: &str,
    choix_libre: Option<&str>,
) -> egui::Response {
    let nom = valeur
        .as_ref()
        .and_then(|id| niveaux.iter().find(|n| &n.id == id))
        .map_or(defaut, |n| n.nom.as_str());
    egui::ComboBox::from_id_salt(id)
        .width((ui.available_width() - 76.0).max(48.0))
        .truncate()
        .selected_text(nom)
        .show_ui(ui, |ui| {
            if let Some(texte) = choix_libre {
                ui.selectable_value(valeur, None, texte);
            }
            for n in niveaux {
                ui.selectable_value(valeur, Some(n.id.clone()), &n.nom);
            }
        })
        .response
}
