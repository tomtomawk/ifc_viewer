//! Poignées d'extrémité de l'élément linéaire sélectionné (mur droit ou poutre) :
//! application du glissement ou de la longueur tapée, en une seule étape d'annulation.
use crate::*;

impl Application {
    /// Déplace l'extrémité `fin` (sinon le début) de la ligne de l'élément `id` au point
    /// `p` (mm, plan). Mur : ligne d'implantation, menuiseries recalées. Poutre :
    /// `Commande::Barre`, hauteurs des extrémités gardées. Rend « du mur » ou « de la
    /// poutre » pour le message d'état.
    pub(crate) fn deplacer_extremite_selection(&mut self, id: &str, fin: bool, p: [f64; 2]) -> Result<&'static str, String> {
        let projet = &self.natif.as_ref().ok_or("Aucun projet natif.")?.0;
        if let Some(b) = projet.barres.iter().find(|b| b.id == id && b.genre == modele::GenreBarre::Poutre) {
            let barre = b.avec_extremite(fin, p)?;
            self.executer_bim(commandes::Commande::Barre { barre })?;
            return Ok("de la poutre");
        }
        let mut copie = projet.clone();
        copie.deplacer_extremite_justification(id, fin, p)?;
        copie.recaler_menuiseries(projet)?;
        copie.verifier()?;
        self.appliquer_projet(copie)?;
        Ok("du mur")
    }

    /// Vrai si `id` est une poutre du projet natif (pas de flèches d'inversion).
    pub(crate) fn est_poutre(&self, id: &str) -> bool {
        self.natif.as_ref().is_some_and(|(p, _)| p.barres.iter().any(|b| b.id == id && b.genre == modele::GenreBarre::Poutre))
    }
}
