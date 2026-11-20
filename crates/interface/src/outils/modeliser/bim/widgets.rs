//! Widgets du panneau BIM : choix de famille d'étiquette, de type de composant, barre, points.

use super::*;


/// Sélecteur standard de la famille d'étiquette d'une pièce ; vrai si le choix a changé.
pub(crate) fn choix_famille_etiquette(ui: &mut egui::Ui, choix: &mut String, familles: &[modele::documentation::FamilleEtiquette]) -> bool {
    use crate::zones::ruban::selecteur;
    let noms: Vec<(String, &str)> = std::iter::once((String::new(), "Étiquette standard")).chain(familles.iter().map(|f| (f.id.clone(), f.nom.as_str()))).collect();
    let lignes: Vec<selecteur::Ligne> = noms.iter().map(|(id, nom)| selecteur::Ligne { nom, courant: id == choix, etiquette: None, sous_titre: None, detail: None, actions: &[] }).collect();
    let titre = format!("Pièce · {}", noms.iter().find(|(id, _)| id == choix).map_or("Étiquette standard", |(_, n)| n));
    let vignette = |pe: &egui::Painter, r: egui::Rect, _: usize, c: Color32| crate::zones::ruban::vignettes::piece(pe, r, c);
    let c = selecteur::simple(ui, &titre, None, &format!("{titre}\nFamille d'étiquette de la pièce."), &lignes, None, Some(&vignette));
    match c.ligne {
        Some(k) if noms[k].0 != *choix => { *choix = noms[k].0.clone(); true }
        _ => false,
    }
}

/// Choix fait dans le sélecteur de type de composant.
#[derive(Default)]
pub(crate) struct ChoixComposant {
    /// Le type courant a changé (`choix` mis à jour).
    pub change: bool,
    /// Type dont la famille est à ouvrir dans l'éditeur.
    pub modifier: Option<String>,
    /// Type à dupliquer.
    pub dupliquer: Option<String>,
    /// « + Nouvelle famille de composant… ».
    pub nouvelle: bool,
}

/// Sélecteur standard du type de composant : vignette aux proportions du type, Modifier
/// (famille dans l'éditeur) et Dupliquer au survol, « + Nouvelle famille… ».
pub(crate) fn choix_type_composant(ui: &mut egui::Ui, choix: &mut String, types: &[modele::TypeComposant]) -> ChoixComposant {
    use crate::zones::ruban::selecteur;
    let actions: &[(selecteur::ActionType, &str)] = &[(selecteur::ActionType::Dupliquer, "Dupliquer : nouveau type d'après celui-ci"), (selecteur::ActionType::Modifier, "Modifier la famille dans l'éditeur ; « Charger dans le projet » met à jour tous ses composants")];
    let details: Vec<String> = types.iter().map(|t| format!("{} × {}", modele::unite::unite().afficher(t.largeur), modele::unite::unite().afficher(t.profondeur))).collect();
    let lignes: Vec<selecteur::Ligne> = types.iter().zip(&details).map(|(t, d)| selecteur::Ligne { nom: &t.nom, courant: &t.id == choix, etiquette: None, sous_titre: None, detail: Some(d.clone()), actions }).collect();
    let titre = format!("Composant · {}", types.iter().find(|t| &t.id == choix).map_or("Choisir un type", |t| t.nom.as_str()));
    let vignette = |pe: &egui::Painter, r: egui::Rect, k: usize, c: Color32| crate::zones::ruban::vignettes::composant(pe, r, types[k].largeur, types[k].profondeur, c);
    let c = selecteur::simple(ui, &titre, None, &format!("{titre}\nType du composant."), &lignes, Some("Nouvelle famille de composant…"), Some(&vignette));
    let mut r = ChoixComposant { nouvelle: c.ajout, ..Default::default() };
    match c.action {
        Some((k, selecteur::ActionType::Modifier)) => r.modifier = Some(types[k].id.clone()),
        Some((k, selecteur::ActionType::Dupliquer)) => r.dupliquer = Some(types[k].id.clone()),
        None => {}
    }
    if let Some(k) = c.ligne.filter(|k| types[*k].id != *choix) {
        *choix = types[k].id.clone();
        r.change = true;
    }
    r
}

/// Copie d'un type de composant sous un nom et un identifiant libres, ajoutée au projet ;
/// renvoie son identifiant.
pub(crate) fn dupliquer_type_composant(p: &mut modele::Projet, source: &modele::TypeComposant) -> String {
    let mut k = 2;
    let nom = loop {
        let nom = format!("{} {k}", source.nom);
        if p.types_composants.iter().all(|t| t.nom != nom) { break nom; }
        k += 1;
    };
    let id = format!("type-composant-{}", noyau::uuid::uuid());
    p.types_composants.push(modele::TypeComposant { id: id.clone(), nom, ..source.clone() });
    id
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