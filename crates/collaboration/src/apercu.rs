//! Projection visuelle des propositions refusées sur la dernière base de travail.
use super::stockage::Etat;
use super::{Conflit, Projet, Resultat, Transaction, concurrence, fusion};
use std::collections::BTreeMap;

pub(super) fn afficher(
    mut etat: Etat,
    attente: &[Transaction],
    conflits: &BTreeMap<String, Vec<Conflit>>,
) -> Resultat<Projet> {
    let mut visible = etat.projection()?;
    for t in attente.iter().filter(|t| conflits.contains_key(&t.id)) {
        if !etat.acceptes.contains_key(&t.id)
            && let Some((suivant, projet)) = superposer(&etat, t)
        {
            etat = suivant;
            visible = projet;
        }
    }
    Ok(visible)
}

fn superposer(etat: &Etat, t: &Transaction) -> Option<(Etat, Projet)> {
    if t.projet != etat.projet || !super::protocole_lisible(t.protocole) {
        return None;
    }
    let mut suivant = etat.clone();
    for c in concurrence::decomposer(t.changements.clone()) {
        if matches!(c.cle.as_str(), "@/identite" | "@/format" | "@/version") {
            return None;
        }
        // Une valeur partagée incompatible reste visible. La proposition est
        // indivisible et reste disponible dans le journal et les détails.
        let valeur = fusion(&c, concurrence::valeur(&etat.objets, &c.cle))?;
        concurrence::affecter(&mut suivant.objets, &c.cle, valeur);
    }
    for (id, mise_a_jour) in &t.textes {
        suivant
            .textes
            .entry(id.clone())
            .or_default()
            .recevoir(mise_a_jour)
            .ok()?;
    }
    let projet = suivant.projection().ok()?;
    Some((suivant, projet))
}
