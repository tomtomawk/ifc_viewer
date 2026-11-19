//! Widgets du panneau BIM : choix de famille d'étiquette, de type de composant, barre, points.

use super::*;


/// Liste des types de composants ; vrai si le choix a changé.
pub(crate) fn choix_famille_etiquette(ui: &mut egui::Ui, salt: &str, choix: &mut String, familles: &[modele::documentation::FamilleEtiquette]) -> bool {
    let avant = choix.clone();
    egui::ComboBox::from_id_salt(salt)
        .selected_text(familles.iter().find(|f| &f.id == choix).map_or("Étiquette standard", |f| f.nom.as_str()))
        .show_ui(ui, |ui| {
            ui.selectable_value(choix, String::new(), "Étiquette standard");
            for f in familles {
                ui.selectable_value(choix, f.id.clone(), &f.nom);
            }
        });
    *choix != avant
}

pub(crate) fn choix_type_composant(ui: &mut egui::Ui, salt: &str, choix: &mut String, types: &[modele::TypeComposant]) -> bool {
    let avant = choix.clone();
    egui::ComboBox::from_id_salt(salt)
        .selected_text(types.iter().find(|t| &t.id == choix).map_or("Choisir un type", |t| t.nom.as_str()))
        .show_ui(ui, |ui| {
            for t in types {
                ui.selectable_value(choix, t.id.clone(), &t.nom);
            }
        });
    *choix != avant
}

pub(crate) fn barre_ui(ui: &mut egui::Ui, b: &mut modele::Barre, niveaux: &[modele::Niveau]) {
    use modele::GenreBarre;
    ui.horizontal(|ui| {
        ui.label("Nom");
        ui.text_edit_singleline(&mut b.nom);
    });
    egui::ComboBox::from_id_salt("niveau_structure")
        .selected_text(
            niveaux
                .iter()
                .find(|n| n.id == b.niveau)
                .map_or("Choisir un niveau", |n| n.nom.as_str()),
        )
        .show_ui(ui, |ui| {
            for n in niveaux {
                ui.selectable_value(&mut b.niveau, n.id.clone(), &n.nom);
            }
        });
    for (i, nom) in ["X départ", "Y départ", "Z départ (relatif au niveau)"]
        .iter()
        .enumerate()
    {
        ui.horizontal(|ui| {
            ui.label(*nom);
            ui.add(
                egui::DragValue::new(&mut b.debut[i])
                    .speed(10.0)
                    .dimensions(),
            );
        });
    }
    if b.genre == GenreBarre::Poteau {
        b.fin[0] = b.debut[0];
        b.fin[1] = b.debut[1];
        ui.horizontal(|ui| {
            ui.label("Z sommet (relatif au niveau)");
            ui.add(egui::DragValue::new(&mut b.fin[2]).speed(10.0).dimensions());
        });
        ui.horizontal(|ui| {
            ui.label("Rotation");
            ui.add(
                egui::DragValue::new(&mut b.rotation)
                    .speed(1.0)
                    .suffix(" °"),
            );
        });
    } else {
        b.fin[2] = b.debut[2];
        for (i, nom) in ["X arrivée", "Y arrivée"].iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(*nom);
                ui.add(egui::DragValue::new(&mut b.fin[i]).speed(10.0).dimensions());
            });
        }
    }
    for (nom, valeur) in [
        ("Largeur", &mut b.largeur),
        (
            if b.genre == GenreBarre::Poteau {
                "Profondeur"
            } else {
                "Hauteur de section"
            },
            &mut b.hauteur,
        ),
    ] {
        ui.horizontal(|ui| {
            ui.label(nom);
            ui.add(
                egui::DragValue::new(valeur)
                    .speed(5.0)
                    .range(1.0..=100000.0)
                    .dimensions(),
            );
        });
    }
}

pub(crate) fn points_ui(ui: &mut egui::Ui, points: &mut [[f64; 2]; 2]) {
    for (k, p) in points.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("Point {}", k + 1));
            for v in p {
                ui.add(egui::DragValue::new(v).speed(10.0).dimensions());
            }
        });
    }
}