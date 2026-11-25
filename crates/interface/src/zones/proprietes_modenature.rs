//! Propriétés d'un profilé de mur sélectionné (occurrence) ; celles de son type sont dans
//! la section « Propriétés du type » (`proprietes_type.rs`).
use crate::*;
use modele::elements::modenature::Modenature;
use modele::Projet;

/// Champs de l'occurrence ; rend le projet modifié.
pub(crate) fn proprietes(ui: &mut egui::Ui, projet: &Projet, m: &Modenature) -> Option<Projet> {
    let mut m = m.clone();
    let mut change = false;
    let t = projet.type_modenature(&m.type_modenature);
    let genre = if t.is_some_and(|t| t.creux) { "creux" } else { "relief" };
    ui.label(RichText::new(format!("Profilé de mur ({genre}) · {}", m.nom)).strong());
    let longueur = projet.face_modenature(&m).map(|f| f.0.longueur()).unwrap_or(0.0);
    egui::Grid::new("proprietes_modenature").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
        ui.label("Nom");
        change |= ui.text_edit_singleline(&mut m.nom).lost_focus();
        ui.end_row();
        ui.label("Face");
        egui::ComboBox::from_id_salt("modenature_face").selected_text(if m.interieur { "Intérieure" } else { "Extérieure" }).show_ui(ui, |ui| {
            change |= ui.selectable_value(&mut m.interieur, false, "Extérieure").changed();
            change |= ui.selectable_value(&mut m.interieur, true, "Intérieure").changed();
        });
        ui.end_row();
        ui.label("Hauteur");
        change |= ui.add(egui::DragValue::new(&mut m.hauteur).speed(10.0).dimensions()).changed();
        ui.end_row();
        ui.label("Depuis");
        egui::ComboBox::from_id_salt("modenature_depuis").selected_text(if m.depuis_haut { "Le haut du mur" } else { "La base du mur" }).show_ui(ui, |ui| {
            change |= ui.selectable_value(&mut m.depuis_haut, false, "La base du mur").changed();
            change |= ui.selectable_value(&mut m.depuis_haut, true, "Le haut du mur").changed();
        });
        ui.end_row();
        let mut debut = m.debut.unwrap_or(0.0);
        let mut fin = m.fin.unwrap_or(longueur);
        ui.label("Début");
        if ui.add(egui::DragValue::new(&mut debut).speed(10.0).range(0.0..=longueur).dimensions()).changed() { m.debut = Some(debut); change = true; }
        ui.end_row();
        ui.label("Fin");
        if ui.add(egui::DragValue::new(&mut fin).speed(10.0).range(0.0..=longueur).dimensions()).changed() { m.fin = Some(fin); change = true; }
        ui.end_row();
    });
    if (m.debut.is_some() || m.fin.is_some()) && ui.small_button("Toute la longueur du mur").clicked() {
        (m.debut, m.fin) = (None, None);
        change = true;
    }
    if !change { return None; }
    let mut q = projet.clone();
    let x = q.modenatures.iter_mut().find(|x| x.id == m.id)?;
    *x = m;
    Some(q)
}
