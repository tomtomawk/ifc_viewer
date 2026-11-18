//! Libellés des étapes d'historique : la seule collection modifiée et, si un seul objet
//! change, son nom.
use super::differences::Difference;
use serde_json::value::RawValue;
use std::collections::{HashMap, HashSet};

pub(crate) const CATEGORIES_PROJET: [(&str, &str); 20] = [
        ("niveaux", "niveau"),
        ("murs", "mur"),
        ("assemblages", "assemblage"),
        ("dalles", "élément horizontal"),
        ("barres", "structure"),
        ("pieces", "pièce"),
        ("composants", "composant"),
        ("arbres", "arbre"),
        ("escaliers", "escalier"),
        ("garde_corps", "garde-corps"),
        ("ouvertures", "ouverture"),
        ("importes", "élément importé"),
        ("vues", "vue"),
        ("quadrillages", "quadrillage"),
        ("annotations", "annotation"),
        ("feuilles", "feuille"),
        ("nomenclatures", "nomenclature"),
        ("types_murs", "type de mur"),
        ("types_dalles", "type de dalle"),
        ("materiaux", "matériau"),
];

/// Libellé d'une étape : la seule collection modifiée et, si un seul objet change,
/// son nom ; les objets sont appariés par `cle`.
pub(crate) fn libelle_collections(a: &serde_json::Value, b: &serde_json::Value, categories: &[(&str, &str)], cle: &str, defaut: &str) -> String {
    let mut changements = Vec::new();
    for &(champ, nom) in categories {
        let va = a.get(champ).and_then(|v| v.as_array());
        let vb = b.get(champ).and_then(|v| v.as_array());
        let na = va.map_or(0, Vec::len);
        let nb = vb.map_or(0, Vec::len);
        if na != nb || (na > 0 && va != vb) {
            changements.push((nom, na, nb, va, vb));
        }
    }
    match changements.as_slice() {
        [(nom, na, nb, va, vb)] => {
            let action = if nb > na {
                "Ajouter"
            } else if nb < na {
                "Supprimer"
            } else {
                "Modifier"
            };
            let (source, autre) = if nb < na { (va, vb) } else { (vb, va) };
            // objets de l'autre état par identifiant (une recherche linéaire par objet
            // coûtait 78 ms pour 1 920 murs)
            let index: HashMap<String, &serde_json::Value> = autre.into_iter().flat_map(|v| v.iter()).filter_map(|o| Some((o.get(cle)?.to_string(), o))).collect();
            let noms: Vec<&str> = source
                .into_iter()
                .flat_map(|v| v.iter())
                .filter(|objet| {
                    let Some(id) = objet.get(cle) else {
                        return false;
                    };
                    let precedent = index.get(&id.to_string()).copied();
                    if na == nb {
                        precedent.is_some_and(|p| p != *objet)
                    } else {
                        precedent.is_none()
                    }
                })
                .filter_map(|objet| {
                    objet
                        .get("nom")
                        .and_then(|n| n.as_str())
                        .filter(|n| !n.trim().is_empty())
                })
                .collect();
            if let [objet] = noms.as_slice() {
                format!("{action} {nom} « {objet} »")
            } else {
                format!("{action} {nom}")
            }
        }
        _ => defaut.into(),
    }
}

/// Identifiant (texte JSON) et nom d'un objet, le reste du texte étant seulement parcouru.
#[derive(serde::Deserialize)]
struct CleObjet<'a> {
    #[serde(borrow, default)]
    id: Option<&'a RawValue>,
    #[serde(default)]
    nom: Option<String>,
}

type Objets = Vec<(String, Option<String>)>;

/// Libellé d'une étape du projet, d'après sa différence : seules les collections
/// touchées sont parcourues, et seulement pour leurs identifiants et leurs noms.
pub(crate) fn libelle_projet(avant: &str, apres: &str, d: &Difference) -> String {
    const DEFAUT: &str = "Modifier le projet";
    let Difference::Objet(champs) = d else { return DEFAUT.into() };
    type Brut<'a> = HashMap<String, &'a RawValue>;
    let (Ok(a), Ok(b)) = (serde_json::from_str::<Brut>(avant), serde_json::from_str::<Brut>(apres)) else { return DEFAUT.into() };
    let lire = |m: &Brut, champ: &str| -> Objets {
        m.get(champ)
            .and_then(|r| serde_json::from_str::<Vec<CleObjet>>(r.get()).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|o| (o.id.map(|i| i.get().to_string()).unwrap_or_default(), o.nom))
            .collect()
    };
    let mut changees = Vec::new();
    for &(champ, nom) in &CATEGORIES_PROJET {
        let Some(sd) = champs.get(champ) else { continue };
        let (va, vb) = (lire(&a, champ), lire(&b, champ));
        if va.is_empty() && vb.is_empty() {
            continue;
        }
        changees.push((nom, sd, va, vb));
    }
    let [(nom, sd, va, vb)] = changees.as_slice() else { return DEFAUT.into() };
    let (na, nb) = (va.len(), vb.len());
    let action = if nb > na { "Ajouter" } else if nb < na { "Supprimer" } else { "Modifier" };
    let ids = |v: &Objets| v.iter().map(|(i, _)| i.clone()).collect::<HashSet<String>>();
    let voulues: HashSet<String> = match (sd, na == nb) {
        (Difference::Collection(c), true) => c.modifies().into_iter().collect(),
        (_, true) => HashSet::new(),
        (_, false) => {
            let (source, autre) = if nb > na { (vb, va) } else { (va, vb) };
            let autre = ids(autre);
            ids(source).into_iter().filter(|i| !autre.contains(i)).collect()
        }
    };
    let source = if nb < na { va } else { vb };
    let noms: Vec<&str> = source.iter().filter(|(i, _)| voulues.contains(i)).filter_map(|(_, n)| n.as_deref().filter(|n| !n.trim().is_empty())).collect();
    match noms.as_slice() {
        [objet] => format!("{action} {nom} « {objet} »"),
        _ => format!("{action} {nom}"),
    }
}
