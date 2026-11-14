//! Documenter : vues en coupe ou en élévation, feuilles, nomenclatures.
use crate::socle::interne::*;

/// Exécution des commandes de documentation.
pub(crate) trait ExecutionDocumenter {
    fn executer_documenter(&mut self, c: Commande) -> Result<Vec<String>, String>;
}

impl ExecutionDocumenter for Projet {
    fn executer_documenter(&mut self, c: Commande) -> Result<Vec<String>, String> {
        match c {
            Commande::ModifierVue { vue } => {
                if vue.nom.trim().is_empty()
                    || self
                        .vues
                        .iter()
                        .any(|v| v.id != vue.id && v.nom.eq_ignore_ascii_case(vue.nom.trim()))
                {
                    return Err("Le nom de la vue est vide ou déjà utilisé.".into());
                }
                let actuel = self
                    .vues
                    .iter_mut()
                    .find(|v| v.id == vue.id)
                    .ok_or("Vue absente.")?;
                if !matches!(actuel.genre, GenreVue::Coupe { .. })
                    || !matches!(vue.genre, GenreVue::Coupe { .. })
                {
                    return Err("Sélectionnez une coupe ou une élévation.".into());
                }
                let id = vue.id.clone();
                *actuel = vue;
                Ok(vec![id])
            }
            Commande::Feuille { mut feuille } => {
                if feuille.id.is_empty() {
                    feuille.id = identite(self, "feuille");
                }
                let id = feuille.id.clone();
                if let Some(a) = self.feuilles.iter_mut().find(|a| a.id == id) {
                    *a = feuille;
                } else {
                    self.feuilles.push(feuille);
                }
                Ok(vec![id])
            }
            Commande::Nomenclature { mut nomenclature } => {
                if nomenclature.id.is_empty() {
                    nomenclature.id = identite(self, "nomenclature");
                }
                let id = nomenclature.id.clone();
                if let Some(a) = self.nomenclatures.iter_mut().find(|a| a.id == id) {
                    *a = nomenclature;
                } else {
                    self.nomenclatures.push(nomenclature);
                }
                Ok(vec![id])
            }
            _ => unreachable!("commande hors du métier : la répartition est faite par socle/execution.rs"),
        }
    }
}
