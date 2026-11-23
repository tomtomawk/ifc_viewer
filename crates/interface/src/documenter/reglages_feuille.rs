//! Propriétés du cartouche d'une feuille (position, ancrage, suppression) et de ses
//! annotations (texte, hauteur, points, type de trait).
use crate::*;
use commandes::Commande;
use modele::documentation as doc;

/// Position du cartouche : coin bas gauche, ancrage aux coins des marges, place par
/// défaut, suppression.
pub(crate) fn reglages_cartouche(ui: &mut egui::Ui, p: &modele::Projet, f: &doc::Feuille) -> Option<Commande> {
    let mut g = f.clone();
    let [l, h] = doc::taille_cartouche(p, f);
    let mut pos = doc::cadre_cartouche(p, f).map_or_else(|| doc::position_cartouche_defaut(p, f), |c| [c[0], c[1]]);
    let avant = pos;
    egui::Grid::new("cartouche_place").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
        ui.label("Position X");
        ui.add(egui::DragValue::new(&mut pos[0]).speed(1.0).max_decimals(1).suffix(" mm"));
        ui.end_row();
        ui.label("Position Y");
        ui.add(egui::DragValue::new(&mut pos[1]).speed(1.0).max_decimals(1).suffix(" mm"));
        ui.end_row();
        ui.label("Taille");
        ui.label(format!("{l:.0} × {h:.0} mm"));
        ui.end_row();
    });
    ui.label(RichText::new("Coin bas gauche du cartouche, depuis le coin bas gauche du papier").small().weak());
    let m = doc::MARGE;
    let coins = [("Haut gauche", [m, f.hauteur - m - h]), ("Haut droite", [f.largeur - m - l, f.hauteur - m - h]), ("Bas gauche", [m, m]), ("Bas droite", [f.largeur - m - l, m])];
    ui.horizontal_wrapped(|ui| {
        ui.label("Ancrer");
        for (nom, coin) in coins {
            if ui.small_button(nom).on_hover_text("Dans ce coin, contre la marge du papier").clicked() {
                pos = coin;
            }
        }
    });
    if pos != avant {
        g.position_cartouche = Some(pos);
    }
    ui.horizontal(|ui| {
        if ui.add_enabled(f.position_cartouche.is_some(), egui::Button::new("Place par défaut")).on_hover_text("En bas à droite, dans la marge").clicked() {
            g.position_cartouche = None;
        }
        if ui.button("Supprimer le cartouche").on_hover_text("Le retire de cette feuille (Suppr) ; Propriétés de la feuille pour le remettre").clicked() {
            g.sans_cartouche = true;
        }
    });
    ui.label(RichText::new("Glisser le cartouche pour le déplacer · double-clic pour modifier sa famille").small().weak());
    (g.position_cartouche != f.position_cartouche || g.sans_cartouche != f.sans_cartouche).then_some(Commande::Feuille { feuille: g })
}

/// Type de trait choisi parmi ceux du projet (vide : continu fin).
fn choix_type_trait(ui: &mut egui::Ui, p: &modele::Projet, id: &mut String) {
    let nom = p.types_traits.iter().find(|t| &t.id == id).map_or("Continu fin", |t| t.nom.as_str());
    egui::ComboBox::from_id_salt("trait_annotation_feuille").selected_text(nom).show_ui(ui, |ui| {
        if ui.selectable_label(id.is_empty(), "Continu fin").clicked() {
            id.clear();
        }
        for t in &p.types_traits {
            if ui.selectable_label(*id == t.id, &t.nom).clicked() {
                *id = t.id.clone();
            }
        }
    });
}

fn point(ui: &mut egui::Ui, nom: &str, q: &mut [f64; 2]) {
    ui.label(nom);
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut q[0]).speed(1.0).max_decimals(1).prefix("X ").suffix(" mm"));
        ui.add(egui::DragValue::new(&mut q[1]).speed(1.0).max_decimals(1).prefix("Y ").suffix(" mm"));
    });
    ui.end_row();
}

/// Annotation de feuille : texte et hauteur, points, type de trait, suppression.
pub(crate) fn reglages_annotation(ui: &mut egui::Ui, p: &modele::Projet, f: &doc::Feuille, i: usize) -> Option<Commande> {
    use doc::AnnotationForme as F;
    let mut g = f.clone();
    let a = &mut g.annotations[i];
    let titre = match &a.forme {
        F::Texte { .. } => "Texte",
        F::Ligne { .. } => "Ligne",
        F::Polyligne { fermee: true, .. } => "Polyligne fermée",
        F::Polyligne { .. } => "Polyligne",
        F::Cercle { .. } => "Cercle",
        F::Spline { .. } => "Spline",
        F::Arc { .. } => "Arc",
        F::Zone { .. } => "Zone",
        _ => "Annotation",
    };
    ui.heading(format!("Annotation · {titre}"));
    egui::Grid::new("annotation_feuille").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| match &mut a.forme {
        F::Texte { point: q, texte } => {
            ui.label("Texte");
            ui.add(egui::TextEdit::multiline(texte).desired_rows(2).desired_width(180.0));
            ui.end_row();
            ui.label("Hauteur");
            ui.add(egui::DragValue::new(&mut a.hauteur).speed(0.1).range(0.5..=100.0).max_decimals(1).suffix(" mm"));
            ui.end_row();
            point(ui, "Position", q);
        }
        F::Ligne { debut, fin } => {
            point(ui, "Début", debut);
            point(ui, "Fin", fin);
        }
        F::Cercle { centre, rayon, type_trait } => {
            point(ui, "Centre", centre);
            ui.label("Rayon");
            ui.add(egui::DragValue::new(rayon).speed(0.5).range(0.1..=5000.0).max_decimals(1).suffix(" mm"));
            ui.end_row();
            ui.label("Type de trait");
            choix_type_trait(ui, p, type_trait);
            ui.end_row();
        }
        F::Spline { type_trait, .. } => {
            ui.label("Type de trait");
            choix_type_trait(ui, p, type_trait);
            ui.end_row();
        }
        F::Polyligne { points, fermee, traits, .. } => {
            ui.label("Sommets");
            ui.label(points.len().to_string());
            ui.end_row();
            ui.label("Fermée");
            ui.checkbox(fermee, "");
            ui.end_row();
            ui.label("Type de trait");
            let mut t = traits.first().cloned().unwrap_or_default();
            let avant = t.clone();
            choix_type_trait(ui, p, &mut t);
            if t != avant {
                *traits = if t.is_empty() { Vec::new() } else { vec![t; points.len()] };
            }
            ui.end_row();
        }
        _ => {}
    });
    if matches!(a.forme, F::Texte { .. }) {
        ui.label(RichText::new("Champs : {projet}, {feuille}, {numero} · double-clic sur la feuille pour modifier en place").small().weak());
    }
    if ui.button("Supprimer l'annotation").clicked() {
        g.annotations.remove(i);
    }
    (g.annotations != f.annotations).then_some(Commande::Feuille { feuille: g })
}
