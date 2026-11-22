//! Reprise explicite des champs modifiés lors d'une résolution locale.
use super::session::{Copie, overlay};
use super::stockage::Etat;
use super::*;

pub(super) fn cible(etat: &Etat, proposition: &Transaction) -> Resultat<Projet> {
    let mut objets = etat.objets.clone();
    for c in concurrence::decomposer(proposition.changements.clone()) {
        let courant = concurrence::valeur(&objets, &c.cle);
        let valeur = if c.cle.starts_with("@ordre/") {
            reprendre_ordre(&c, courant)
        } else {
            reprendre(c.avant.as_ref(), c.apres.as_ref(), courant, false)?
        };
        concurrence::affecter(&mut objets, &c.cle, valeur);
    }
    projet(&objets)
}

fn reprendre_ordre(c: &Changement, courant: Option<&Value>) -> Option<Value> {
    if let Some(valeur) = fusion(c, courant) {
        return valeur;
    }
    let (Some(Value::Array(avant)), Some(Value::Array(local)), Some(Value::Array(central))) =
        (&c.avant, &c.apres, courant)
    else {
        return c.apres.clone();
    };
    let mut ordre: Vec<_> = central
        .iter()
        .filter(|id| !avant.contains(id) || local.contains(id))
        .cloned()
        .collect();
    let connus: Vec<_> = ordre
        .iter()
        .filter(|id| avant.contains(id))
        .cloned()
        .collect();
    let ordre_initial = avant.iter().filter(|id| connus.contains(*id));
    let ordre_local = local.iter().filter(|id| connus.contains(*id));
    if !ordre_initial.eq(ordre_local.clone()) {
        for (case, id) in ordre
            .iter_mut()
            .filter(|id| connus.contains(*id))
            .zip(ordre_local)
        {
            *case = id.clone();
        }
    }
    for (i, id) in local.iter().enumerate() {
        if !avant.contains(id) && !ordre.contains(id) {
            let place = local[i + 1..]
                .iter()
                .find_map(|suivant| ordre.iter().position(|id| id == suivant))
                .unwrap_or(ordre.len());
            ordre.insert(place, id.clone());
        }
    }
    Some(Value::Array(ordre))
}

fn reprendre(
    avant: Option<&Value>,
    local: Option<&Value>,
    central: Option<&Value>,
    imbrique: bool,
) -> Resultat<Option<Value>> {
    if avant == local {
        return Ok(central.cloned());
    }
    let (Some(Value::Object(avant)), Some(Value::Object(local)), Some(Value::Object(central))) =
        (avant, local, central)
    else {
        return Ok(local.cloned());
    };
    // Les variantes d'une même valeur structurée forment un ensemble.
    if imbrique {
        for tag in ["ancrage", "contrainte", "forme", "genre", "source"] {
            if let (Some(a), Some(b), Some(c)) = (avant.get(tag), local.get(tag), central.get(tag))
                && a.is_string()
                && b.is_string()
                && c.is_string()
                && b != c
            {
                if a == b {
                    return Err(
                        "Le type d’un paramètre modifié a changé dans le projet partagé.".into(),
                    );
                }
                return Ok(Some(Value::Object(local.clone())));
            }
        }
    }
    let mut resultat = central.clone();
    for cle in avant.keys().chain(local.keys()).collect::<BTreeSet<_>>() {
        if avant.get(cle) == local.get(cle) {
            continue;
        }
        match reprendre(avant.get(cle), local.get(cle), central.get(cle), true)? {
            Some(v) => {
                resultat.insert(cle.clone(), v);
            }
            None => {
                resultat.remove(cle);
            }
        }
    }
    Ok(Some(Value::Object(resultat)))
}

pub(super) fn revalider(copie: &mut Copie) {
    let mut etat = copie.base.clone();
    for t in &copie.attente {
        if copie.conflits.contains_key(&t.id) {
            continue;
        }
        match overlay(&etat, std::slice::from_ref(t)) {
            Ok(suivant) => etat = suivant,
            Err(e) => {
                copie.conflits.insert(
                    t.id.clone(),
                    vec![Conflit {
                        cle: "réapplication".into(),
                        base: None,
                        locale: None,
                        acceptee: None,
                        raison: e,
                    }],
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reprise_de_l_ordre_preserve_les_ajouts_et_reclassements_independants() {
        for (local, central, attendu) in [
            (
                json!(["a", "b", "d"]),
                json!(["b", "a", "c"]),
                json!(["b", "a", "c", "d"]),
            ),
            (
                json!(["b", "a"]),
                json!(["a", "c", "b"]),
                json!(["b", "c", "a"]),
            ),
        ] {
            let c = Changement {
                cle: "@ordre/niveaux".into(),
                avant: Some(json!(["a", "b"])),
                apres: Some(local),
            };
            assert_eq!(reprendre_ordre(&c, Some(&central)), Some(attendu));
        }
    }
}
