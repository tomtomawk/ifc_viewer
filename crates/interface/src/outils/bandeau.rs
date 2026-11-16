//! Bandeau de dessin commun : options de la commande active, puis Valider / Annuler,
//! en haut à gauche de la vue. Seul composant de ce rôle : la vue du projet, l'éditeur
//! de cartouche et l'éditeur de familles l'appellent tous. Les messages s'affichent
//! juste en dessous (`socle::messages`), à la hauteur qu'il renvoie.
use crate::*;

/// Boutons de fin de commande proposés par le bandeau.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct Fin {
    pub valider: bool,
    pub annuler: bool,
}

impl Fin {
    pub(crate) const AUCUNE: Fin = Fin { valider: false, annuler: false };
    pub(crate) const COMPLETE: Fin = Fin { valider: true, annuler: true };
}

/// Choix fait dans le bandeau pendant cette image.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Action {
    Valider,
    Annuler,
}

/// Ce que renvoie le bandeau : l'action choisie et le bas du bandeau, mesuré depuis le
/// haut de la vue (position des messages et consignes).
pub(crate) struct Retour {
    pub action: Option<Action>,
    pub bas: f32,
}

/// Affiche le bandeau dans `rect`. `options` dessine les options de la commande (dans
/// une zone défilante si elles dépassent la largeur) ; `None` : pas d'options.
pub(crate) fn bandeau(ui: &egui::Ui, rect: egui::Rect, id: &str, fin: Fin, options: Option<&mut dyn FnMut(&mut egui::Ui)>) -> Retour {
    let mut action = None;
    let boutons = fin.valider || fin.annuler;
    let zone = egui::Area::new(egui::Id::unique(("bandeau_dessin", id)))
        .fixed_pos(rect.left_top() + egui::vec2(8.0, 8.0))
        .constrain_to(rect)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let avec_options = options.is_some();
                    if let Some(options) = options {
                        egui::ScrollArea::horizontal().id_salt(("options_bandeau", id))
                            .max_width((rect.width() - if boutons { 120.0 } else { 30.0 }).max(80.0))
                            .max_height(75.0)
                            .auto_shrink([true, true])
                            .show(ui, |ui| ui.horizontal(|ui| options(ui)));
                    }
                    if boutons && avec_options { ui.separator(); }
                    if fin.valider && bouton(ui, true, "Valider").on_hover_text("Terminer et quitter la commande").clicked() {
                        action = Some(Action::Valider);
                    }
                    if fin.annuler && bouton(ui, false, "Annuler").on_hover_text("Abandonner et quitter la commande").clicked() {
                        action = Some(Action::Annuler);
                    }
                });
            });
        });
    Retour { action, bas: zone.response.rect.bottom() - rect.top() }
}

/// Pictogramme seul (coche verte, croix rouge), à la hauteur des autres commandes ; le
/// libellé est dans l'infobulle et l'accessibilité.
fn bouton(ui: &mut egui::Ui, valider: bool, libelle: &str) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    let (r, reponse) = ui.allocate_exact_size(egui::vec2(h * 1.4, h), Sense::click());
    if reponse.hovered() {
        ui.painter().rect_filled(r, 4.0, ui.visuals().widgets.hovered.bg_fill);
    }
    let c = r.center();
    let u = h / 26.0;
    let couleur = if valider { Color32::from_rgb(30, 150, 70) } else { Color32::from_rgb(200, 50, 45) };
    let t = egui::Stroke::new(2.0, couleur);
    if valider {
        ui.painter().add(egui::Shape::line(vec![c + egui::vec2(-6.0, 0.0) * u, c + egui::vec2(-2.0, 5.0) * u, c + egui::vec2(6.0, -5.0) * u], t));
    } else {
        ui.painter().line_segment([c + egui::vec2(-5.0, -5.0) * u, c + egui::vec2(5.0, 5.0) * u], t);
        ui.painter().line_segment([c + egui::vec2(-5.0, 5.0) * u, c + egui::vec2(5.0, -5.0) * u], t);
    }
    reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, libelle));
    reponse
}
