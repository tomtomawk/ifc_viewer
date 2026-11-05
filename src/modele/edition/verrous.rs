//! Verrouillage des éléments : un élément verrouillé n'est ni déplacé, ni modifié, ni
//! supprimé. Il reste sélectionnable et peut être copié.
use super::commandes::Commande;
use crate::modele::Projet;
use std::collections::BTreeSet;

impl Projet {
    /// Vrai si l'élément, ou le mur composé qui le contient, est verrouillé.
    pub fn verrouille(&self, id: &str) -> bool {
        self.verrous.contains_key(id)
            || self.assemblages.iter().any(|a| self.verrous.contains_key(&a.id) && a.couches.iter().any(|c| c == id))
    }
    /// Identifiants protégés : éléments verrouillés et couches des murs composés verrouillés.
    pub fn ids_verrouilles(&self) -> BTreeSet<String> {
        let mut ids: BTreeSet<String> = self.verrous.keys().cloned().collect();
        for a in &self.assemblages {
            if self.verrous.contains_key(&a.id) {
                ids.extend(a.couches.iter().cloned());
            }
        }
        ids
    }
    /// Retire les éléments verrouillés d'une commande qui transforme une sélection.
    /// Une sélection entièrement verrouillée est refusée.
    pub(crate) fn filtrer_verrouilles(&self, mut c: Commande) -> Result<Commande, String> {
        if self.verrous.is_empty() {
            return Ok(c);
        }
        let ids = match &mut c {
            Commande::Deplacer { ids, .. }
            | Commande::Tourner { ids, .. }
            | Commande::Aligner { ids, .. }
            | Commande::Attacher { ids, .. }
            | Commande::Supprimer { ids } => ids,
            _ => return Ok(c),
        };
        let avant = ids.len();
        ids.retain(|id| !self.verrouille(id));
        if ids.is_empty() && avant > 0 {
            return Err(message(avant));
        }
        Ok(c)
    }
}

pub fn message(n: usize) -> String {
    if n == 1 {
        "Élément verrouillé : le déverrouiller avant de le modifier.".into()
    } else {
        format!("{n} éléments verrouillés : les déverrouiller avant de les modifier.")
    }
}

/// Refuse toute modification ou suppression d'un élément verrouillé entre `avant` et
/// `apres` : couvre les commandes et les éditions directes des propriétés.
pub fn verifier(avant: &Projet, apres: &Projet) -> Result<(), String> {
    let ids = avant.ids_verrouilles();
    if ids.is_empty() {
        return Ok(());
    }
    use crate::modele::collaboration::{concurrence, objets};
    let (a, b) = (objets(avant)?, objets(apres)?);
    let mut touches = BTreeSet::new();
    for (cle, v) in &a {
        if let Some(nom) = cle.strip_prefix("@/") {
            // relations par objet (contexte, implantation…) : seule la ligne de l'élément compte
            if nom != "verrous" && concurrence::carte_objet(nom) {
                if let Some(m) = v.as_object() {
                    for id in ids.iter().filter(|id| m.contains_key(id.as_str())) {
                        if b.get(cle).and_then(|x| x.get(id.as_str())) != m.get(id.as_str()) {
                            touches.insert(id.clone());
                        }
                    }
                }
            }
            continue;
        }
        if cle.starts_with('@') {
            continue;
        }
        let Some((_, id)) = cle.split_once('/') else { continue };
        if ids.contains(id) && b.get(cle) != Some(v) {
            touches.insert(id.to_string());
        }
    }
    if touches.is_empty() { Ok(()) } else { Err(message(touches.len())) }
}

#[cfg(test)]
mod tests {
    use crate::modele::edition::commandes::Commande;
    use crate::modele::edition::ParametresMur;
    use crate::modele::Projet;

    fn projet() -> (Projet, String, String) {
        let mut p = Projet::nouveau("Verrous");
        p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur::default()).unwrap();
        p.ajouter_mur([0.0, 3000.0], [5000.0, 3000.0], &ParametresMur::default()).unwrap();
        let (a, b) = (p.murs[0].id.clone(), p.murs[1].id.clone());
        (p, a, b)
    }

    #[test]
    fn verrouille_refuse_deplacement_suppression_et_edition_directe() {
        let (p, a, _) = projet();
        let (p, _) = p.executer(Commande::Verrouiller { ids: vec![a.clone()], verrou: true }).unwrap();
        assert!(p.verrouille(&a));
        assert!(p.executer(Commande::Deplacer { ids: vec![a.clone()], delta: [100.0, 0.0, 0.0] }).is_err());
        assert!(p.executer(Commande::Supprimer { ids: vec![a.clone()] }).is_err());
        let mut q = p.clone();
        q.murs[0].nom = "Renommé".into();
        assert!(super::verifier(&p, &q).is_err());
        // copier reste permis
        assert!(p.executer(Commande::Copier { ids: vec![a.clone()], delta: [0.0, 1000.0, 0.0] }).is_ok());
    }

    #[test]
    fn selection_mixte_ignore_le_verrouille_et_deverrouiller_libere() {
        let (p, a, b) = projet();
        let (p, _) = p.executer(Commande::Verrouiller { ids: vec![a.clone()], verrou: true }).unwrap();
        let (q, _) = p.executer(Commande::Deplacer { ids: vec![a.clone(), b.clone()], delta: [0.0, 100.0, 0.0] }).unwrap();
        let json = |p: &Projet, i: usize| serde_json::to_value(&p.murs[i]).unwrap();
        assert_eq!(json(&q, 0), json(&p, 0));
        assert_ne!(json(&q, 1), json(&p, 1));
        let (p, _) = p.executer(Commande::Verrouiller { ids: vec![a.clone()], verrou: false }).unwrap();
        assert!(p.executer(Commande::Supprimer { ids: vec![a] }).is_ok());
    }
}
