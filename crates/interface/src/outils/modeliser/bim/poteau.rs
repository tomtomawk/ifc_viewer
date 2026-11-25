//! Palette de création des poteaux et résolution de leur limite haute au placement.
use super::*;
use crate::outils::croquis::{self, CroquisVertical};

impl Application {
    /// Emplacement « schéma » du poteau : section et croquis des deux limites.
    pub(crate) fn schema_poteau(&mut self, ui: &mut egui::Ui) {
        let Some((projet, _)) = self.natif.as_ref() else { return };
        let niveau_vue = self.niveau_trace();
        let b = &mut self.bim.barre;
        let sommet = &mut self.bim.niveau_haut_poteau;
        let decalage_haut = &mut self.bim.decalage_haut_poteau;
        if sommet
            .as_ref()
            .is_some_and(|id| projet.niveau(id).is_none())
        {
            *sommet = None;
        }
        let mut base = projet
            .niveau(&b.niveau)
            .map(|n| n.id.clone())
            .or(niveau_vue);
        let mut hauteur = b.fin[2] - b.debut[2];
        ui.label(RichText::new("Base et sommet").strong());
        croquis::cadre(ui, |ui| {
            let croquis = CroquisVertical::lignes(ui, |ui, haut| {
                if haut {
                    croquis::champ_niveau(
                        ui,
                        "niveau_haut_poteau",
                        &projet.niveaux,
                        sommet,
                        "Libre",
                        Some("Libre (hauteur)"),
                    );
                } else {
                    croquis::champ_niveau(
                        ui,
                        "niveau_bas_poteau",
                        &projet.niveaux,
                        &mut base,
                        "Choisir un niveau",
                        None,
                    );
                }
                let (valeur, info) = match (haut, sommet.is_some()) {
                    (true, true) => {
                        (egui::DragValue::new(decalage_haut), "Décalage du sommet")
                    }
                    (true, false) => (
                        egui::DragValue::new(&mut hauteur).range(1.0..=f64::MAX),
                        "Hauteur du poteau",
                    ),
                    (false, _) => {
                        (egui::DragValue::new(&mut b.debut[2]), "Décalage de la base")
                    }
                };
                ui.add_sized([72.0, 20.0], valeur.speed(10.0).dimensions())
                    .on_hover_text(info);
            });
            b.niveau = base.clone().unwrap_or_default();
            b.fin[2] = b.debut[2] + hauteur;
            let calcul = sommet_poteau(projet, b, sommet.as_deref(), *decalage_haut);
            let [haut, bas] = croquis.references();
            let decalage_bas = croquis::decalage_vertical(b.debut[2]);
            let decalage_haut = sommet
                .as_ref()
                .map_or(decalage_bas, |_| croquis::decalage_vertical(*decalage_haut));
            croquis.peindre(
                ui,
                [haut + decalage_haut, bas + decalage_bas],
                23.0,
                calcul.as_ref().ok().map(|fin| fin - b.debut[2]),
            );
        });
        if let Err(erreur) = sommet_poteau(projet, b, sommet.as_deref(), *decalage_haut) {
            ui.colored_label(ui.visuals().error_fg_color, erreur);
        }
    }

    /// Emplacement « paramètres » du poteau : nom, section, rotation, placement par coordonnées.
    pub(crate) fn parametres_poteau(&mut self, ui: &mut egui::Ui) {
        let b = &mut self.bim.barre;
        let mut creation = false;
        crate::zones::palette_creation::titre_parametres(ui, "Poteau");
        egui::Grid::new("grille_creation_poteau")
            .num_columns(2)
            .striped(true)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label("Nom");
                ui.add(egui::TextEdit::singleline(&mut b.nom).desired_width(115.0));
                ui.end_row();
                ui.label("Rotation");
                ui.add(
                    egui::DragValue::new(&mut b.rotation)
                        .speed(1.0)
                        .suffix(" °"),
                );
                ui.end_row();
            });
        ui.label(
            RichText::new("Cliquez dans la vue pour placer le poteau.")
                .small()
                .weak(),
        );
        ui.collapsing("Placement par coordonnées", |ui| {
            for (nom, coordonnee) in ["X", "Y"].into_iter().zip(&mut b.debut) {
                ui.horizontal(|ui| {
                    ui.label(nom);
                    ui.add(egui::DragValue::new(coordonnee).speed(10.0).dimensions());
                });
            }
            creation = ui.button("Créer le poteau").clicked();
        });
        if !creation {
            return;
        }
        match self.poteau_regle([self.bim.barre.debut[0], self.bim.barre.debut[1]]) {
            Ok(barre) => {
                if let Err(e) = self.executer_bim(commandes::Commande::Barre { barre }) {
                    self.error = Some(e);
                }
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(super) fn poteau_regle(&self, point: [f64; 2]) -> Result<modele::Barre, String> {
        let (projet, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let mut b = self.bim.barre.clone();
        if !projet.types_poteaux.iter().any(|t| t.id == b.type_barre) {
            return Err("Choisissez un type de poteau dans la palette.".into());
        }
        b.id.clear();
        b.debut[..2].copy_from_slice(&point);
        let z = sommet_poteau(
            projet,
            &b,
            self.bim.niveau_haut_poteau.as_deref(),
            self.bim.decalage_haut_poteau,
        )?;
        b.fin = [point[0], point[1], z];
        Ok(b)
    }
}

fn sommet_poteau(
    projet: &modele::Projet,
    b: &modele::Barre,
    sommet: Option<&str>,
    decalage: f64,
) -> Result<f64, &'static str> {
    let base = projet
        .niveau(&b.niveau)
        .ok_or("Choisissez le niveau de base.")?;
    let z = match sommet {
        Some(id) => {
            projet
                .niveau(id)
                .ok_or("Choisissez le niveau du sommet.")?
                .altitude
                + decalage
                - base.altitude
        }
        None => b.fin[2],
    };
    if !z.is_finite() || z - b.debut[2] < 1.0 {
        return Err("Le sommet doit dépasser la base d’au moins 1 mm.");
    }
    Ok(z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hauteur_libre_et_sommet_sur_un_autre_niveau() {
        let mut projet = modele::Projet::nouveau("Poteau");
        for (id, altitude) in [("base", 1000.0), ("haut", 4500.0)] {
            projet.niveaux.push(modele::Niveau {
                id: id.into(),
                nom: id.into(),
                altitude,
                origine: None,
            });
        }
        let mut barre = EtatBim::default().barre;
        barre.niveau = "base".into();
        barre.debut[2] = 100.0;
        barre.fin[2] = 2900.0;
        assert_eq!(sommet_poteau(&projet, &barre, None, 0.0), Ok(2900.0));
        assert_eq!(
            sommet_poteau(&projet, &barre, Some("haut"), -200.0),
            Ok(3300.0)
        );
        assert!(sommet_poteau(&projet, &barre, Some("base"), 0.0).is_err());
    }
}
