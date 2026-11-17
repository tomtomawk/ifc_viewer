//! Données lues d'un élément importé, en lecture seule dans le panneau Propriétés :
//! matériau, couleur, jeux de propriétés et de quantités.
use crate::*;

/// Matériau et couleur, puis un bloc repliable par jeu (nom, valeur et unité).
pub(crate) fn afficher(ui: &mut egui::Ui, id: &str, d: &modele::importes::DonneesImportees) {
    if d.est_vide() {
        return;
    }
    ui.add_space(4.0);
    egui::Grid::new(("importe-donnees", id)).num_columns(2).striped(true).show(ui, |ui| {
        if let Some(m) = &d.materiau {
            ui.label(RichText::new("Matériau").strong());
            ui.add(egui::Label::new(m).wrap());
            ui.end_row();
        }
        if let Some([r, g, b, a]) = d.couleur {
            ui.label(RichText::new("Couleur").strong());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgba_unmultiplied(r, g, b, a));
                let opacite = if a < 255 { format!(" · opacité {} %", (a as f32 / 2.55).round()) } else { String::new() };
                ui.label(format!("{r}, {g}, {b}{opacite}"));
            });
            ui.end_row();
        }
    });
    for (i, j) in d.proprietes.iter().enumerate() {
        let titre = if j.quantites { format!("{} (quantités)", j.nom) } else { j.nom.clone() };
        egui::CollapsingHeader::new(RichText::new(titre).strong()).id_salt(("importe-jeu", id, i)).default_open(true).show(ui, |ui| {
            egui::Grid::new(("importe-grille", id, i)).num_columns(2).striped(true).show(ui, |ui| {
                for p in &j.proprietes {
                    ui.label(&p.nom);
                    ui.add(egui::Label::new(p.libelle()).wrap());
                    ui.end_row();
                }
            });
        });
    }
}
