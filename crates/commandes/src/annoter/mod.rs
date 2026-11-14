//! Annoter : quadrillages, annotations (cotes, textes, étiquettes et tracés 2D : une
//! seule commande `Annoter`), niveaux ; cotes pilotantes (`cotes`).
pub mod cotes;
use crate::socle::interne::*;

/// Exécution des commandes d'annotation.
pub(crate) trait ExecutionAnnoter {
    fn executer_annoter(&mut self, c: Commande) -> Result<Vec<String>, String>;
}

impl ExecutionAnnoter for Projet {
    fn executer_annoter(&mut self, c: Commande) -> Result<Vec<String>, String> {
        match c {
            Commande::ModifierQuadrillage { quadrillage } => {
                let actuel = self
                    .quadrillages
                    .iter_mut()
                    .find(|g| g.id == quadrillage.id)
                    .ok_or("Quadrillage absent.")?;
                let id = quadrillage.id.clone();
                *actuel = quadrillage;
                Ok(vec![id])
            }
            Commande::Quadrillage { nom, debut, fin } => {
                let id = identite(self, "quadrillage");
                self.quadrillages.push(documentation::Quadrillage {
                    id: id.clone(),
                    nom,
                    debut,
                    fin,
                });
                Ok(vec![id])
            }
            Commande::Annoter { mut annotation } => {
                if annotation.id.is_empty() {
                    annotation.id = identite(self, "annotation");
                }
                let id = annotation.id.clone();
                if let Some(a) = self.annotations.iter_mut().find(|a| a.id == id) {
                    *a = annotation;
                } else {
                    self.annotations.push(annotation);
                }
                Ok(vec![id])
            }
            Commande::Niveau { nom, altitude } => {
                if nom.trim().is_empty()
                    || !altitude.is_finite()
                    || self
                        .niveaux
                        .iter()
                        .any(|n| n.nom.eq_ignore_ascii_case(nom.trim()))
                {
                    return Err("Nom de niveau vide/déjà utilisé ou altitude non valide.".into());
                }
                let id = identite(self, "niveau");
                self.niveaux.push(Niveau {
                    id: id.clone(),
                    nom: nom.trim().into(),
                    altitude,
                    origine: None,
                });
                Ok(vec![id])
            }
            Commande::ModifierNiveau { id, nom, altitude } => {
                if nom.trim().is_empty()
                    || !altitude.is_finite()
                    || self
                        .niveaux
                        .iter()
                        .any(|n| n.id != id && n.nom.eq_ignore_ascii_case(nom.trim()))
                {
                    return Err("Nom ou altitude non valide.".into());
                }
                let n = self
                    .niveaux
                    .iter_mut()
                    .find(|n| n.id == id)
                    .ok_or("Niveau absent.")?;
                n.nom = nom.trim().into();
                n.altitude = altitude;
                Ok(vec![id])
            }
            _ => unreachable!("commande hors du métier : la répartition est faite par socle/execution.rs"),
        }
    }
}
