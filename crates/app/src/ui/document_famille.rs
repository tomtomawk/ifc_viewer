//! État de document commun aux familles planes et modélisées.
use crate::modele::edition::historique::{Historique, Instantane};
use std::path::PathBuf;

pub(super) struct DocumentFamille<T: Instantane> {
    pub ordre: usize,
    pub document: usize,
    pub autonome: bool,
    pub chemin_fichier: Option<PathBuf>,
    pub historique: Historique<T>,
    pub instantane: T,
}

impl<T: Instantane + Clone + PartialEq> DocumentFamille<T> {
    pub fn nouveau(ordre: usize, document: usize, autonome: bool, chemin_fichier: Option<PathBuf>, instantane: T) -> Self {
        Self { ordre, document, autonome, chemin_fichier, historique: Default::default(), instantane }
    }
    pub fn suivre(&mut self, etat: &T, geste: bool) -> Result<(), String> {
        self.historique.geste(geste);
        if &self.instantane != etat {
            self.historique.pousser(&self.instantane, etat)?;
            self.instantane = etat.clone();
        }
        Ok(())
    }
    pub fn cible(&mut self, actuel: &T, retablir: bool, nombre: usize) -> Result<T, String> {
        self.suivre(actuel, false)?;
        let cible = self.historique.cible_n(retablir, nombre)?;
        self.historique.confirmer_n(actuel, retablir, nombre)?;
        self.instantane = cible.clone();
        Ok(cible)
    }
}
