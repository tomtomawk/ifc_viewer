//! Projet vu comme une table d'objets identifiés (clé `collection/id`) : base de la
//! comparaison de deux états du projet (verrous ici, transactions dans `collaboration`).
//! Descendu de `collaboration` au lot 6 (chantier du découpage §5.13 b : `git show 9121974:docs/historique/ARCHITECTURE_CHANTIER_2026-11.md`) ; `collaboration`
//! ré-exporte ces symboles sous leurs anciens chemins.
use modele::Projet;
use serde_json::Value;
use std::collections::BTreeMap;

pub type Objets = BTreeMap<String, Value>;
/// Listes d'objets identifiés : ordre canonique, sauf ordre métier des feuilles.
pub fn objets(p: &Projet) -> Result<Objets, String> {
    let Value::Object(root) = serde_json::to_value(p).map_err(|e| e.to_string())? else {
        unreachable!()
    };
    let mut out = Objets::new();
    for (collection, v) in root {
        if let Value::Array(liste) = &v
            && collection_identifiee(&collection) {
                let mut ordre = Vec::new();
                for o in liste {
                    let id = o["id"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| format!("Identité absente : {collection}"))?;
                    let key = format!("{collection}/{id}");
                    let mut objet = o.clone();
                    if let Some(champs) = objet.as_object_mut()
                        && collection == "annotations"
                        && champs.get("forme").and_then(Value::as_str) == Some("texte")
                    {
                        champs.remove("texte");
                    }
                    if out.insert(key.clone(), objet).is_some() {
                        return Err(format!("Identité dupliquée : {key}"));
                    }
                    ordre.push(Value::String(id.into()));
                }
                out.insert(format!("@/{collection}"), Value::Array(Vec::new()));
                if politique_collection(&collection) == Some(PolitiqueCollection::IdentifieeOrdonnee) {
                    out.insert(format!("@ordre/{collection}"), Value::Array(ordre));
                }
                continue;
            }
        // Les cartes de relations sont atomiques : détecte aussi ajout/suppression d'arête.
        out.insert(format!("@/{collection}"), v);
    }
    Ok(out)
}
/// Politique de comparaison collaborative des collections sérialisées du projet.
/// Les collections atomiques sont comparées comme une seule valeur ; les collections
/// identifiées utilisent la clé `id`, et certaines conservent leur ordre métier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolitiqueCollection { Atomique, Identifiee, IdentifieeOrdonnee }

pub const COLLECTIONS: &[(&str, PolitiqueCollection)] = &[
    ("sources", PolitiqueCollection::Identifiee),
    ("gabarits_vues", PolitiqueCollection::Identifiee),
    ("variantes", PolitiqueCollection::Identifiee),
    ("niveaux", PolitiqueCollection::IdentifieeOrdonnee),
    ("types_murs", PolitiqueCollection::IdentifieeOrdonnee),
    ("types_dalles", PolitiqueCollection::IdentifieeOrdonnee),
    ("types_ouvertures", PolitiqueCollection::IdentifieeOrdonnee),
    ("murs", PolitiqueCollection::Identifiee),
    ("assemblages", PolitiqueCollection::Identifiee),
    ("materiaux", PolitiqueCollection::Identifiee),
    ("ouvertures", PolitiqueCollection::Identifiee),
    ("dalles", PolitiqueCollection::Identifiee),
    ("terrains", PolitiqueCollection::Identifiee),
    ("types_terrains", PolitiqueCollection::IdentifieeOrdonnee),
    ("limites_propriete", PolitiqueCollection::Identifiee),
    ("barres", PolitiqueCollection::Identifiee),
    ("pieces", PolitiqueCollection::Identifiee),
    ("composants", PolitiqueCollection::Identifiee),
    ("types_composants", PolitiqueCollection::Identifiee),
    ("profils", PolitiqueCollection::IdentifieeOrdonnee),
    ("escaliers", PolitiqueCollection::Identifiee),
    ("garde_corps", PolitiqueCollection::Identifiee),
    ("types_garde_corps", PolitiqueCollection::Identifiee),
    ("types_poutres", PolitiqueCollection::Identifiee),
    ("vues", PolitiqueCollection::IdentifieeOrdonnee),
    ("importes", PolitiqueCollection::Identifiee),
    ("liens", PolitiqueCollection::Identifiee),
    ("quadrillages", PolitiqueCollection::Identifiee),
    ("annotations", PolitiqueCollection::IdentifieeOrdonnee),
    ("feuilles", PolitiqueCollection::IdentifieeOrdonnee),
    ("nomenclatures", PolitiqueCollection::Identifiee),
    ("motifs", PolitiqueCollection::Identifiee),
    ("familles_cartouches", PolitiqueCollection::Identifiee),
    ("familles_etiquettes", PolitiqueCollection::Identifiee),
    ("types_traits", PolitiqueCollection::Identifiee),
    ("positions_plan", PolitiqueCollection::IdentifieeOrdonnee),
    ("phases", PolitiqueCollection::Atomique),
    ("sous_projets", PolitiqueCollection::Atomique),
    ("plumes", PolitiqueCollection::Atomique),
    ("familles_materiaux", PolitiqueCollection::Atomique),
    ("types_vues", PolitiqueCollection::Atomique),
];

fn politique_collection(c: &str) -> Option<PolitiqueCollection> {
    COLLECTIONS.iter().find_map(|(nom, politique)| (*nom == c).then_some(*politique))
}
fn collection_identifiee(c: &str) -> bool {
    matches!(politique_collection(c), Some(PolitiqueCollection::Identifiee | PolitiqueCollection::IdentifieeOrdonnee))
}

/// Vrai pour les cartes de relations comparées objet par objet (une entrée par élément).
pub fn carte_objet(nom: &str) -> bool {
    matches!(
        nom,
        "organisations_arborescence"
            | "rangements_arborescence"
            | "implantations"
            | "contextes"
            | "contextes_vues"
            | "gabarits_vue_affectes"
            | "verrous"
            | "demolitions"
            | "priorites_jonction"
            | "attaches_toitures"
            | "cartouches_feuilles"
            | "placements_surface"
    )
}
