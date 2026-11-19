//! Réponses du pilote pour la collaboration : état résumé, clés des réservations,
//! identifiant de transaction. Fonctions pures, sans `Application`.
use ::collaboration::session::Copie;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Au-delà, une version d'objet en conflit n'est pas recopiée dans le résumé.
const TAILLE_VALEUR: usize = 2000;

fn valeur_courte(v: &Option<Value>) -> Value {
    match v {
        None => Value::Null,
        Some(v) => {
            let n = v.to_string().len();
            if n <= TAILLE_VALEUR { v.clone() } else { json!({ "omis_caracteres": n }) }
        }
    }
}

/// État de la copie sans les objets de la base ni l'aperçu de conflit (projet entier) :
/// révision, baux, propositions en attente (clés touchées) et conflits.
pub(crate) fn resume(copie: &Copie, etat: &str, synchronisation: bool) -> Value {
    let attente: Vec<Value> = copie
        .attente
        .iter()
        .map(|t| {
            json!({
                "id": t.id, "base": t.base, "auteur": t.auteur,
                "cles": t.changements.iter().map(|c| &c.cle).collect::<Vec<_>>(),
                "reservations": t.reservations, "textes": t.textes.keys().collect::<Vec<_>>(),
            })
        })
        .collect();
    let conflits: BTreeMap<&String, Vec<Value>> = copie
        .conflits
        .iter()
        .map(|(id, liste)| {
            let liste = liste
                .iter()
                .map(|c| json!({
                    "cle": c.cle, "raison": c.raison, "base": valeur_courte(&c.base),
                    "locale": valeur_courte(&c.locale), "acceptee": valeur_courte(&c.acceptee),
                }))
                .collect();
            (id, liste)
        })
        .collect();
    json!({
        "etat": etat,
        "synchronisation": synchronisation,
        "session": copie.session,
        "partage": copie.partage,
        "central": copie.central,
        "central_connu": copie.central_connu,
        "projet": copie.base.projet,
        "protocole": copie.base.protocole,
        "revision": copie.base.revision,
        "objets": copie.base.objets.len(),
        "textes": copie.base.textes.len(),
        "baux": copie.base.baux,
        "attente": attente,
        "conflits": conflits,
        "apercu_conflit": copie.apercu_conflit.is_some(),
        "annulations": copie.annulations.len(),
        "retablissements": copie.retablissements.len(),
        "editeurs": copie.editeurs.keys().collect::<Vec<_>>(),
    })
}

/// Clés `collection/id` des objets à réserver : chaque entrée est une clé exacte ou
/// l'identifiant natif d'un objet.
pub(crate) fn cles_objets(objets: &BTreeMap<String, Value>, ids: &[String]) -> Result<Vec<String>, String> {
    ids.iter()
        .map(|id| {
            if objets.contains_key(id) {
                return Ok(id.clone());
            }
            objets
                .iter()
                .find(|(_, v)| v.get("id").and_then(Value::as_str) == Some(id))
                .map(|(k, _)| k.clone())
                .ok_or_else(|| format!("Objet absent : {id}"))
        })
        .collect()
}

/// Identifiant complet d'une proposition en attente à partir d'un préfixe non ambigu.
pub(crate) fn transaction(copie: &Copie, prefixe: &str) -> Result<String, String> {
    let prefixe = prefixe.trim();
    if prefixe.is_empty() {
        return Err("Transaction requise".into());
    }
    let trouvees: Vec<&String> = copie.attente.iter().map(|t| &t.id).filter(|id| id.starts_with(prefixe)).collect();
    match trouvees.as_slice() {
        [id] => Ok((*id).clone()),
        [] => Err(format!("Proposition absente : {prefixe}")),
        _ => Err(format!("Préfixe ambigu : {prefixe} ({} propositions)", trouvees.len())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copie(attente: &[&str]) -> Copie {
        let mut c: Copie = serde_json::from_value(json!({
            "session": "s", "central": "c",
            "base": {"protocole": 3, "projet": "p", "revision": 2, "objets": {"barres/b-1": {"id": "b-1", "gros": "x".repeat(5000)}}, "textes": {}, "baux": {}, "acceptes": {}},
            "attente": [], "conflits": {}, "annulations": [], "retablissements": [],
        }))
        .unwrap();
        for id in attente {
            c.attente.push(serde_json::from_value(json!({
                "protocole": 3, "projet": "p", "id": id, "auteur": "a", "session": "s", "base": 2,
                "lectures": {}, "changements": [{"cle": "barres/b-1", "avant": null, "apres": null}],
                "ressources": [], "reservations": {}, "textes": {}, "compense": null,
            })).unwrap());
        }
        c
    }

    #[test]
    fn resume_sans_objets_ni_grosses_valeurs() {
        let mut c = copie(&["3856ce74-aaaa"]);
        c.conflits.insert("3856ce74-aaaa".into(), vec![::collaboration::Conflit {
            cle: "barres/b-1".into(), base: c.base.objets.get("barres/b-1").cloned(),
            locale: Some(json!({"id": "b-1"})), acceptee: None, raison: "r".into(),
        }]);
        let r = resume(&c, "Conflit", false);
        let texte = r.to_string();
        assert!(texte.len() < 1000, "{texte}");
        assert_eq!(r["objets"], 1);
        assert_eq!(r["attente"][0]["cles"][0], "barres/b-1");
        let conflit = &r["conflits"]["3856ce74-aaaa"][0];
        assert_eq!(conflit["locale"]["id"], "b-1");
        assert!(conflit["base"]["omis_caracteres"].as_u64().unwrap() > 5000);
    }

    #[test]
    fn cles_par_id_ou_par_cle() {
        let c = copie(&[]);
        let o = &c.base.objets;
        assert_eq!(cles_objets(o, &["b-1".into()]).unwrap(), ["barres/b-1"]);
        assert_eq!(cles_objets(o, &["barres/b-1".into()]).unwrap(), ["barres/b-1"]);
        assert!(cles_objets(o, &["barres/b-2".into()]).is_err());
    }

    #[test]
    fn transaction_par_prefixe() {
        let c = copie(&["3856ce74-aaaa", "3856ce75-bbbb"]);
        assert_eq!(transaction(&c, "3856ce74").unwrap(), "3856ce74-aaaa");
        assert_eq!(transaction(&c, "3856ce75-bbbb").unwrap(), "3856ce75-bbbb");
        assert!(transaction(&c, "3856ce7").unwrap_err().contains("ambigu"));
        assert!(transaction(&c, "ffff").is_err());
        assert!(transaction(&c, " ").is_err());
    }
}
