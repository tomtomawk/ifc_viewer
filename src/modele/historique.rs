//! Transactions atomiques par document. Les instantanés sont bornés en mémoire.
use super::Projet;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Historique {
    avant: VecDeque<String>,
    apres: Vec<String>,
    sauvegarde: Option<String>,
    geste: bool,
    debut_geste: Option<String>,
}
impl Historique {
    pub fn enregistrer(&mut self, p: &Projet) -> Result<(), String> {
        self.geste(false);
        self.sauvegarde = Some(p.json()?);
        Ok(())
    }
    pub fn modifie(&self, p: &Projet) -> bool {
        p.json().ok().as_ref() != self.sauvegarde.as_ref()
    }
    pub fn peut_annuler(&self) -> bool {
        !self.avant.is_empty()
    }
    pub fn peut_retablir(&self) -> bool {
        !self.apres.is_empty()
    }
    pub fn geste(&mut self, actif: bool) {
        if !actif && let Some(s) = self.debut_geste.take() {
            self.avant.push_back(s);
            self.borner();
        }
        self.geste = actif;
    }
    fn borner(&mut self) {
        while self.avant.len() > 100
            || (self.avant.len() > 1
                && self.avant.iter().map(String::len).sum::<usize>() > 64 * 1024 * 1024)
        {
            self.avant.pop_front();
        }
    }
    pub fn pousser(&mut self, avant: &Projet, apres: &Projet) -> Result<(), String> {
        let a = avant.json()?;
        if a == apres.json()? {
            return Ok(());
        }
        if self.geste {
            if self.debut_geste.is_none() {
                self.debut_geste = Some(a);
            }
        } else {
            self.avant.push_back(a);
        }
        self.apres.clear();
        self.borner();
        Ok(())
    }
    // Le contrôleur valide et reconstruit d'abord la scène, puis déplace le curseur.
    pub fn cible(&self, retablir: bool) -> Result<Projet, String> {
        let s = if retablir {
            self.apres.last()
        } else {
            self.avant.back()
        }
        .ok_or("Historique vide.")?;
        Projet::lire_json(s)
    }
    pub fn confirmer(&mut self, actuel: &Projet, retablir: bool) -> Result<(), String> {
        let s = actuel.json()?;
        if retablir {
            self.apres.pop().ok_or("Historique vide.")?;
            self.avant.push_back(s);
        } else {
            self.avant.pop_back().ok_or("Historique vide.")?;
            self.apres.push(s);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sauvegarde_annulation_branche_et_documents_isoles() {
        let a = Projet::nouveau("a");
        let b = Projet::nouveau("b");
        let c = Projet::nouveau("c");
        let mut h = Historique::default();
        let mut autre = Historique::default();
        h.enregistrer(&a).unwrap();
        autre.enregistrer(&c).unwrap();
        h.pousser(&a, &b).unwrap();
        assert!(h.modifie(&b));
        let retour = h.cible(false).unwrap();
        h.confirmer(&b, false).unwrap();
        assert!(!h.modifie(&retour));
        assert!(h.peut_retablir());
        assert!(!autre.peut_annuler());
        h.pousser(&retour, &c).unwrap();
        assert!(!h.peut_retablir());
    }
}
