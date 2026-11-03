//! Transactions de valeurs persistantes, indépendantes de l'interface et du transport.
pub mod cli;
pub mod session;
pub mod stockage;
#[cfg(test)]
mod tests;
pub mod texte;

use super::Projet;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PROTOCOLE: u32 = 1;
pub type Objets = BTreeMap<String, Value>;
pub type Resultat<T> = Result<T, String>;
pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn empreinte(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn encoder<T: Serialize>(v: &T) -> Resultat<Vec<u8>> {
    // Value utilise des dictionnaires triés (serde_json sans preserve_order).
    serde_json::to_vec(&serde_json::to_value(v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn hash<T: Serialize>(v: &T) -> String {
    empreinte(&encoder(v).expect("valeur sérialisable"))
}

/// Listes d'objets identifiés : ordre canonique, sauf ordre métier des feuilles.
pub fn objets(p: &Projet) -> Resultat<Objets> {
    let Value::Object(root) = serde_json::to_value(p).map_err(|e| e.to_string())? else {
        unreachable!()
    };
    let mut out = Objets::new();
    for (collection, v) in root {
        if let Value::Array(liste) = &v {
            if collection_identifiee(&collection) {
                let mut ordre = Vec::new();
                for o in liste {
                    let id = o["id"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| format!("Identité absente : {collection}"))?;
                    let key = format!("{collection}/{id}");
                    let mut objet = o.clone();
                    if collection == "annotations"
                        && objet.get("forme").and_then(Value::as_str) == Some("texte")
                    {
                        objet.as_object_mut().unwrap().remove("texte");
                    }
                    if out.insert(key.clone(), objet).is_some() {
                        return Err(format!("Identité dupliquée : {key}"));
                    }
                    ordre.push(Value::String(id.into()));
                }
                out.insert(format!("@/{collection}"), Value::Array(Vec::new()));
                if matches!(
                    collection.as_str(),
                    "feuilles"
                        | "vues"
                        | "niveaux"
                        | "positions_plan"
                        | "types_terrains"
                        | "types_ouvertures"
                        | "types_murs"
                        | "types_dalles"
                        | "annotations"
                ) {
                    out.insert(format!("@ordre/{collection}"), Value::Array(ordre));
                }
                continue;
            }
        }
        // Les cartes de relations sont atomiques : détecte aussi ajout/suppression d'arête.
        out.insert(format!("@/{collection}"), v);
    }
    Ok(out)
}
fn collection_identifiee(c: &str) -> bool {
    matches!(
        c,
        "sources"
            | "variantes"
            | "niveaux"
            | "types_murs"
            | "types_dalles"
            | "types_ouvertures"
            | "murs"
            | "assemblages"
            | "materiaux"
            | "ouvertures"
            | "dalles"
            | "terrains"
            | "types_terrains"
            | "limites_propriete"
            | "barres"
            | "escaliers"
            | "garde_corps"
            | "vues"
            | "importes"
            | "quadrillages"
            | "annotations"
            | "feuilles"
            | "nomenclatures"
            | "motifs"
            | "familles_cartouches"
            | "positions_plan"
    )
}
pub fn projet(objets: &Objets) -> Resultat<Projet> {
    let mut root = serde_json::Map::new();
    let mut collections: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (k, v) in objets {
        let (c, _) = k.split_once('/').ok_or("Clé d'objet invalide")?;
        if c == "@" {
            root.insert(k[2..].into(), v.clone());
        } else if c != "@ordre" {
            let mut v = v.clone();
            if c == "annotations" && v.get("forme").and_then(Value::as_str) == Some("texte") {
                v.as_object_mut()
                    .unwrap()
                    .insert("texte".into(), Value::String(String::new()));
            }
            collections.entry(c.into()).or_default().push(v);
        }
    }
    for (c, mut liste) in collections {
        if let Some(Value::Array(ordre)) = objets.get(&format!("@ordre/{c}")) {
            liste.sort_by_key(|o| {
                ordre
                    .iter()
                    .position(|id| Some(id) == o.get("id"))
                    .unwrap_or(usize::MAX)
            });
        }
        root.insert(c, Value::Array(liste));
    }
    let p = Projet::lire_json(&Value::Object(root).to_string())?;
    p.verifier()?;
    Ok(p)
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Changement {
    pub cle: String,
    pub avant: Option<Value>,
    pub apres: Option<Value>,
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Bail {
    pub proprietaire: String,
    pub generation: u64,
    pub actif: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub protocole: u32,
    pub projet: String,
    pub id: String,
    pub auteur: String,
    pub session: String,
    pub base: u64,
    pub lectures: BTreeMap<String, String>,
    pub changements: Vec<Changement>,
    pub ressources: BTreeSet<String>,
    pub reservations: BTreeMap<String, u64>,
    pub textes: BTreeMap<String, Vec<u8>>,
    pub compense: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Conflit {
    pub cle: String,
    pub base: Option<Value>,
    pub locale: Option<Value>,
    pub acceptee: Option<Value>,
    pub raison: String,
}
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Index {
    pub utilisateurs: BTreeMap<String, BTreeSet<String>>,
}
impl Index {
    pub fn construire(o: &Objets) -> Self {
        let mut ids: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for (k, v) in o {
            if let Some(id) = v.get("id").and_then(Value::as_str) {
                ids.entry(id).or_default().insert(k);
            }
            if matches!(
                k.split('/').next().unwrap_or(""),
                "materiaux" | "motifs" | "familles_cartouches"
            ) {
                if let Some(nom) = v.get("nom").and_then(Value::as_str) {
                    ids.entry(nom).or_default().insert(k);
                }
            }
        }
        fn refs(v: &Value, ids: &BTreeMap<&str, BTreeSet<&str>>, out: &mut BTreeSet<String>) {
            match v {
                Value::String(s) => {
                    if let Some(cles) = ids.get(s.as_str()) {
                        out.extend(cles.iter().map(|k| (*k).to_string()));
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        refs(v, ids, out);
                    }
                }
                Value::Object(m) => {
                    for (k, v) in m {
                        if k != "id" && k != "nom" {
                            if let Some(cibles) = ids.get(k.as_str()) {
                                out.extend(cibles.iter().map(|k| (*k).to_string()));
                            }
                            refs(v, ids, out);
                        }
                    }
                }
                _ => (),
            }
        }
        let mut i = Self::default();
        for (k, v) in o {
            let mut r = BTreeSet::new();
            refs(v, &ids, &mut r);
            for cible in r {
                i.utilisateurs.entry(cible).or_default().insert(k.clone());
            }
        }
        i
    }
    pub fn empreinte(&self, cle: &str, o: &Objets) -> String {
        // Comprend les versions des utilisateurs, pas seulement les identités.
        hash(
            &self
                .utilisateurs
                .get(cle)
                .into_iter()
                .flatten()
                .map(|k| (k, o.get(k)))
                .collect::<BTreeMap<_, _>>(),
        )
    }
}

impl Transaction {
    pub fn entre(avant: &Projet, apres: &Projet, session: &str, base: u64) -> Resultat<Self> {
        if avant.identite != apres.identite {
            return Err("Identité projet modifiée".into());
        }
        apres.verifier()?;
        let (a, b) = (objets(avant)?, objets(apres)?);
        let keys: BTreeSet<_> = a.keys().chain(b.keys()).cloned().collect();
        let changements: Vec<_> = keys
            .into_iter()
            .filter(|k| a.get(k) != b.get(k))
            .map(|cle| Changement {
                avant: a.get(&cle).cloned(),
                apres: b.get(&cle).cloned(),
                cle,
            })
            .collect();
        let index = Index::construire(&a);
        let index_apres = Index::construire(&b);
        let mut lectures = BTreeMap::new();
        for c in &changements {
            for (cible, utilisateurs) in index
                .utilisateurs
                .iter()
                .chain(index_apres.utilisateurs.iter())
            {
                if cible != &c.cle && utilisateurs.contains(&c.cle) {
                    lectures.insert(cible.clone(), hash(&a.get(cible)));
                }
            }
        }
        // Domaine spatial conservateur : toute édition géométrique observe l'ensemble
        // de la géométrie et ses collections (les ajouts sont donc détectés).
        if changements.iter().any(|c| sensible(&c.cle)) {
            lectures.insert("#spatial".into(), spatial(&a));
        }
        for c in &changements {
            lectures.insert(format!("#usages/{}", c.cle), index.empreinte(&c.cle, &a));
        }
        fn ressources(v: &Value, out: &mut BTreeSet<String>) {
            match v {
                Value::Object(m) => {
                    for (k, v) in m {
                        if matches!(k.as_str(), "maillage" | "representation") && !v.is_null() {
                            out.insert(hash(v));
                        } else {
                            ressources(v, out);
                        }
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        ressources(v, out);
                    }
                }
                _ => (),
            }
        }
        let mut requises = BTreeSet::new();
        for c in &changements {
            for v in c.avant.iter().chain(c.apres.iter()) {
                ressources(v, &mut requises);
            }
        }
        Ok(Self {
            protocole: PROTOCOLE,
            projet: avant.identite.clone(),
            id: uuid(),
            auteur: std::env::var("USERNAME").unwrap_or_else(|_| session.into()),
            session: session.into(),
            base,
            lectures,
            changements,
            ressources: requises,
            reservations: BTreeMap::new(),
            textes: BTreeMap::new(),
            compense: None,
        })
    }
    pub fn inverse(&self, base: u64) -> Self {
        let mut t = self.clone();
        t.id = uuid();
        t.base = base;
        t.compense = Some(self.id.clone());
        for c in &mut t.changements {
            std::mem::swap(&mut c.avant, &mut c.apres);
        }
        t
    }
}
pub fn sensible(k: &str) -> bool {
    matches!(
        k.split('/').next().unwrap_or(""),
        "murs"
            | "assemblages"
            | "ouvertures"
            | "dalles"
            | "niveaux"
            | "types_murs"
            | "types_dalles"
            | "types_ouvertures"
            | "importes"
            | "terrains"
            | "barres"
            | "escaliers"
            | "garde_corps"
            | "types_terrains"
            | "materiaux"
            | "motifs"
            | "familles_cartouches"
    ) || matches!(
        k,
        "@/coordonnees_origine"
            | "@/orientation_nord"
            | "@/parametres_partages"
            | "@/attaches_toitures"
            | "@/priorites_jonction"
            | "@/implantations"
            | "@/contextes"
            | "@/materiaux"
            | "@/motifs"
            | "@/plumes"
    )
}
fn spatial(o: &Objets) -> String {
    hash(
        &o.iter()
            .filter(|(k, _)| sensible(k))
            .collect::<BTreeMap<_, _>>(),
    )
}
fn independant(k: &str, champ: &str) -> bool {
    k.starts_with("vues/") && matches!(champ, "nom" | "type_vue")
}
fn fusion(c: &Changement, courant: Option<&Value>) -> Option<Option<Value>> {
    if courant == c.avant.as_ref() || courant == c.apres.as_ref() {
        return Some(c.apres.clone());
    }
    if c.cle.starts_with("@ordre/") {
        if let (Some(Value::Array(a)), Some(Value::Array(l)), Some(Value::Array(r))) =
            (&c.avant, &c.apres, courant)
        {
            // Deux ajouts en fin de liste sont compatibles ; réordonner ou supprimer
            // en concurrence reste une décision métier explicite.
            if l.starts_with(a) && r.starts_with(a) {
                let mut fusion = r.clone();
                for id in &l[a.len()..] {
                    if !fusion.contains(id) {
                        fusion.push(id.clone());
                    }
                }
                return Some(Some(Value::Array(fusion)));
            }
        }
        return None;
    }
    let (Some(Value::Object(a)), Some(Value::Object(l)), Some(Value::Object(r))) =
        (&c.avant, &c.apres, courant)
    else {
        return None;
    };
    let champs: BTreeSet<_> = a.keys().chain(l.keys()).chain(r.keys()).collect();
    if champs
        .iter()
        .any(|champ| a.get(*champ) != r.get(*champ) && !independant(&c.cle, champ))
    {
        return None;
    }
    let mut out = r.clone();
    for champ in champs {
        if a.get(champ) == l.get(champ) {
            continue;
        }
        if !independant(&c.cle, champ)
            || (r.get(champ) != a.get(champ) && r.get(champ) != l.get(champ))
        {
            return None;
        }
        match l.get(champ) {
            Some(v) => {
                out.insert(champ.clone(), v.clone());
            }
            None => {
                out.remove(champ);
            }
        }
    }
    Some(Some(Value::Object(out)))
}
pub fn appliquer(
    t: &Transaction,
    courant: &Objets,
    revision: u64,
    projet_id: &str,
) -> Result<Objets, Vec<Conflit>> {
    let mut erreurs = Vec::new();
    let conflit = |cle: String, raison: &str| Conflit {
        cle,
        base: None,
        locale: None,
        acceptee: None,
        raison: raison.into(),
    };
    if t.protocole != PROTOCOLE || t.projet != projet_id || t.base > revision {
        erreurs.push(conflit(
            "protocole".into(),
            "Protocole, projet ou parent incompatible",
        ));
    }
    if t.changements
        .iter()
        .any(|c| matches!(c.cle.as_str(), "@/identite" | "@/format" | "@/version"))
    {
        erreurs.push(conflit(
            "protocole".into(),
            "Identité et schéma du projet sont immuables dans une transaction",
        ));
    }
    let index = Index::construire(courant);
    for (cle, attendu) in &t.lectures {
        let actuel = if cle == "#spatial" {
            spatial(courant)
        } else if let Some(k) = cle.strip_prefix("#usages/") {
            index.empreinte(k, courant)
        } else {
            hash(&courant.get(cle))
        };
        if &actuel != attendu {
            erreurs.push(conflit(
                cle.clone(),
                "Dépendance ou collection de relations modifiée",
            ));
        }
    }
    let mut out = courant.clone();
    for c in &t.changements {
        match fusion(c, courant.get(&c.cle)) {
            Some(Some(v)) => {
                out.insert(c.cle.clone(), v);
            }
            Some(None) => {
                out.remove(&c.cle);
            }
            None => erreurs.push(Conflit {
                cle: c.cle.clone(),
                base: c.avant.clone(),
                locale: c.apres.clone(),
                acceptee: courant.get(&c.cle).cloned(),
                raison: "Propositions concurrentes incompatibles".into(),
            }),
        }
    }
    if !erreurs.is_empty() && !t.changements.is_empty() {
        for c in &t.changements {
            if !erreurs.iter().any(|e| e.cle == c.cle) {
                erreurs.push(Conflit {
                    cle: c.cle.clone(),
                    base: c.avant.clone(),
                    locale: c.apres.clone(),
                    acceptee: courant.get(&c.cle).cloned(),
                    raison: "Transaction entière retenue après refus de précondition".into(),
                });
            }
        }
    }
    if erreurs.is_empty() {
        if let Err(e) = projet(&out) {
            erreurs.push(conflit("validation".into(), &e));
        }
    }
    if erreurs.is_empty() {
        Ok(out)
    } else {
        Err(erreurs)
    }
}
