//! Briques communes des sélecteurs de type (mur, élément horizontal, ouverture, coupe,
//! garde-corps, cartouche…), sur le modèle du sélecteur de mur : en-tête fermé
//! « catégorie · nom » avec détail à droite et flèche ; liste en fenêtre surgissante,
//! onglets de catégorie à icône, lignes vignette · nom · détail avec actions au survol,
//! ligne « + » de création en fin de liste.
use super::icones::{icone_action_type, triangle_bas};
use eframe::egui::{self, Color32, RichText, Sense};

/// Hauteur d'une ligne de type (une ligne de texte ; deux avec un sous-titre).
pub(crate) const HAUTEUR_LIGNE: f32 = 28.0;
pub(crate) const HAUTEUR_LIGNE_DOUBLE: f32 = 38.0;
/// Hauteur de l'en-tête fermé.
pub(crate) const HAUTEUR_ENTETE: f32 = 34.0;
/// Zone de la vignette d'une ligne, depuis son coin haut gauche.
const VIGNETTE_LIGNE: (f32, f32) = (40.0, 20.0);
/// Abscisse du nom dans une ligne.
const X_NOM: f32 = 58.0;
/// Hauteur d'un onglet de catégorie.
const HAUTEUR_ONGLET: f32 = 50.0;
/// Hauteur de la liste hors lignes : onglets, aide, séparateurs, ligne « + ».
pub(crate) const HAUTEUR_CADRE: f32 = 116.0;

pub(crate) fn largeur_texte(ui: &egui::Ui, t: &str, taille: f32) -> f32 {
    ui.fonts_mut(|x| x.layout_no_wrap(t.to_string(), egui::FontId::proportional(taille), Color32::WHITE).size().x)
}

/// Fond d'une zone : sélection pour l'élément courant, survol sinon.
fn fond(ui: &egui::Ui, r: egui::Rect, courant: bool, survol: bool, arrondi: f32) {
    if courant || survol {
        ui.painter().rect_filled(r, arrondi, if courant { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
    }
}

/// En-tête fermé du sélecteur : vignette facultative, titre (« catégorie · nom »),
/// détail grisé à droite, flèche. Réponse cliquable : à passer à [`liste`].
pub(crate) fn entete(ui: &mut egui::Ui, titre: &str, detail: Option<&str>, bulle: &str, vignette: Option<&dyn Fn(&egui::Painter, egui::Rect, Color32)>) -> egui::Response {
    let largeur = ui.available_width();
    let hauteur = if vignette.is_some() { HAUTEUR_ENTETE } else { 30.0 };
    let (r, bouton) = ui.allocate_exact_size(egui::vec2(largeur, hauteur), Sense::click());
    let visuel = ui.style().interact(&bouton).clone();
    ui.painter().rect(r, 3.0, visuel.bg_fill, visuel.bg_stroke, egui::StrokeKind::Inside);
    let fleche = egui::pos2(r.right() - 12.0, r.center().y);
    triangle_bas(ui.painter(), fleche, visuel.fg_stroke.color);
    let mut droite = fleche.x - 10.0;
    if let Some(d) = detail.filter(|d| !d.is_empty()) {
        ui.painter().text(egui::pos2(droite, r.center().y), egui::Align2::RIGHT_CENTER, d, egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
        droite -= largeur_texte(ui, d, 11.0) + 8.0;
    }
    let mut gauche = r.left() + 10.0;
    if let Some(v) = vignette {
        v(ui.painter(), egui::Rect::from_min_size(r.min + egui::vec2(8.0, 5.0), egui::vec2(28.0, 24.0)), visuel.text_color());
        gauche = r.left() + 44.0;
    }
    let zone = egui::Rect::from_min_max(egui::pos2(gauche, r.top()), egui::pos2(droite.max(gauche), r.bottom()));
    ui.painter().with_clip_rect(zone).text(egui::pos2(zone.left(), r.center().y), egui::Align2::LEFT_CENTER, titre, egui::FontId::proportional(13.0), visuel.text_color());
    bouton.on_hover_text(bulle)
}

/// Liste surgissante sous l'en-tête : reste ouverte au clic sur un onglet, se ferme
/// au clic à l'extérieur ou sur un type (`ui.close()`). `hauteur_min` : hauteur fixe
/// pour que la liste ne saute pas d'un onglet à l'autre.
pub(crate) fn liste(bouton: &egui::Response, largeur: f32, hauteur_min: f32, contenu: impl FnOnce(&mut egui::Ui)) {
    egui::Popup::from_toggle_button_response(bouton)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(largeur)
        .show(|ui| {
            ui.set_min_width(largeur);
            ui.set_min_height(hauteur_min);
            contenu(ui);
        });
}

/// Largeur de liste : assez pour la vignette, le nom le plus long, le détail et les
/// onglets, jamais moins que l'en-tête.
pub(crate) fn largeur_liste(ui: &egui::Ui, noms: impl Iterator<Item = impl AsRef<str>>, detail: f32, onglets: usize, largeur_onglet: f32) -> f32 {
    let plus_long = noms.map(|n| largeur_texte(ui, n.as_ref(), 12.0)).fold(0.0f32, f32::max);
    (X_NOM + plus_long + 16.0 + detail + 8.0).max(ui.available_width()).max(onglets as f32 * (largeur_onglet + 2.0) + 12.0)
}

/// Onglets de catégorie (icône au-dessus du nom) suivis de l'aide de l'onglet actif.
/// Retourne l'onglet cliqué s'il change.
pub(crate) fn onglets<T: Copy + PartialEq>(ui: &mut egui::Ui, items: &[T], actif: T, largeur_onglet: f32, nom: impl Fn(T) -> String, aide: &str, icone: impl Fn(&egui::Painter, egui::Rect, T, Color32)) -> Option<T> {
    let mut choix = None;
    if items.len() > 1 {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for &c in items {
                let (r, reponse) = ui.allocate_exact_size(egui::vec2(largeur_onglet, HAUTEUR_ONGLET), Sense::click());
                let est_actif = actif == c;
                fond(ui, r, est_actif, reponse.hovered(), 4.0);
                icone(ui.painter(), egui::Rect::from_center_size(r.center_top() + egui::vec2(0.0, 17.0), egui::vec2(26.0, 26.0)), c, ui.visuals().text_color());
                ui.painter().text(r.center_bottom() - egui::vec2(0.0, 9.0), egui::Align2::CENTER_CENTER, nom(c), egui::FontId::proportional(12.0), ui.visuals().text_color());
                if reponse.clicked() && !est_actif {
                    choix = Some(c);
                    // liste redimensionnée à l'image suivante
                    ui.ctx().request_repaint();
                }
            }
        });
    }
    if !aide.is_empty() {
        ui.label(RichText::new(aide).small().weak());
    }
    ui.separator();
    choix
}

/// Action proposée au survol d'une ligne.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ActionType {
    Modifier,
    Dupliquer,
}

/// Ligne de type à tracer dans la liste.
pub(crate) struct Ligne<'a> {
    pub nom: &'a str,
    pub courant: bool,
    /// Mention discrète après le nom (« gabarit »).
    pub etiquette: Option<&'a str>,
    /// Seconde ligne grisée (aide du type).
    pub sous_titre: Option<&'a str>,
    /// Détail grisé à droite (épaisseur, cotes) ; remplacé au survol par les actions.
    pub detail: Option<String>,
    /// Actions au survol, de droite à gauche.
    pub actions: &'a [(ActionType, &'a str)],
}

/// Résultat d'un clic dans une ligne.
#[derive(Default, PartialEq, Eq, Debug)]
pub(crate) struct Clic {
    pub choisi: bool,
    pub action: Option<ActionType>,
}

/// Trace une ligne de type ; `vignette` dessine dans la zone prévue (rien si None).
/// Ferme la liste sur un choix ou une action.
pub(crate) fn ligne(ui: &mut egui::Ui, l: &Ligne, vignette: Option<&mut dyn FnMut(&mut egui::Ui, egui::Rect)>) -> Clic {
    let hauteur = if l.sous_titre.is_some() { HAUTEUR_LIGNE_DOUBLE } else { HAUTEUR_LIGNE };
    let (r, reponse) = ui.allocate_exact_size(egui::vec2(ui.available_width(), hauteur), Sense::click());
    let survol = ui.rect_contains_pointer(r);
    fond(ui, r, l.courant, survol, 3.0);
    if let Some(v) = vignette {
        v(ui, egui::Rect::from_min_size(egui::pos2(r.left() + 8.0, r.center().y - VIGNETTE_LIGNE.1 / 2.0), egui::vec2(VIGNETTE_LIGNE.0, VIGNETTE_LIGNE.1)));
    }
    let y_nom = if l.sous_titre.is_some() { r.top() + 12.0 } else { r.center().y };
    ui.painter().text(egui::pos2(r.left() + X_NOM, y_nom), egui::Align2::LEFT_CENTER, l.nom, egui::FontId::proportional(12.0), ui.visuals().text_color());
    if let Some(e) = l.etiquette {
        ui.painter().text(egui::pos2(r.left() + X_NOM + 8.0 + largeur_texte(ui, l.nom, 12.0), y_nom), egui::Align2::LEFT_CENTER, e, egui::FontId::proportional(10.0), ui.visuals().weak_text_color());
    }
    if let Some(s) = l.sous_titre {
        ui.painter().text(egui::pos2(r.left() + X_NOM, r.top() + 27.0), egui::Align2::LEFT_CENTER, s, egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
    }
    let mut clic = Clic::default();
    if survol && !l.actions.is_empty() {
        let mut x = r.right() - 8.0;
        for (n, &(action, bulle)) in l.actions.iter().enumerate() {
            let zone = egui::Rect::from_min_max(egui::pos2(x - 22.0, r.center().y - 11.0), egui::pos2(x, r.center().y + 11.0));
            x -= 24.0;
            let rep = ui.interact(zone, ui.scope_id().with(("action_type", l.nom, n)), Sense::click()).on_hover_text(bulle);
            if rep.hovered() {
                ui.painter().rect_filled(zone, 3.0, ui.visuals().widgets.active.bg_fill);
            }
            let c = if rep.hovered() { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
            icone_action_type(ui.painter(), zone.shrink(4.0), action == ActionType::Modifier, c);
            if rep.clicked() {
                clic.action = Some(action);
            }
        }
    } else if let Some(d) = &l.detail {
        ui.painter().text(r.right_center() - egui::vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, d, egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
    }
    clic.choisi = reponse.clicked() && clic.action.is_none();
    if clic.choisi || clic.action.is_some() {
        ui.close();
    }
    clic
}

/// Ligne de création en fin de liste (« + Nouveau type… », « + Charger… »).
pub(crate) fn ligne_ajout(ui: &mut egui::Ui, texte: &str) -> bool {
    ui.separator();
    let (r, reponse) = ui.allocate_exact_size(egui::vec2(ui.available_width(), HAUTEUR_LIGNE), Sense::click());
    fond(ui, r, false, reponse.hovered(), 3.0);
    ui.painter().text(egui::pos2(r.left() + 8.0 + VIGNETTE_LIGNE.0 / 2.0, r.center().y), egui::Align2::CENTER_CENTER, "+", egui::FontId::proportional(16.0), ui.visuals().strong_text_color());
    ui.painter().text(egui::pos2(r.left() + X_NOM, r.center().y), egui::Align2::LEFT_CENTER, texte, egui::FontId::proportional(12.0), ui.visuals().text_color());
    if reponse.clicked() {
        ui.close();
    }
    reponse.clicked()
}
