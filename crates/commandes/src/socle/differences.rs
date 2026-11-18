//! Différence entre deux états JSON d'un document : seuls les champs et les objets qui
//! changent sont gardés, avec leur valeur avant et après. Les tableaux d'objets portant
//! un `id` unique sont comparés objet par objet ; les autres valeurs sont gardées
//! entières. Une différence s'applique dans les deux sens (annuler : vers `avant`,
//! rétablir : vers `apres`).
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Difference {
    /// Valeur entière avant et après (`None` : champ ou objet absent).
    Valeur { avant: Option<Value>, apres: Option<Value> },
    /// Champs modifiés d'un objet.
    Objet(BTreeMap<String, Difference>),
    /// Tableau d'objets identifiés par `id`.
    Collection(Collection),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    /// Objets modifiés, ajoutés (`Valeur { avant: None }`) ou retirés, par texte JSON de
    /// leur identifiant.
    objets: BTreeMap<String, Difference>,
    /// Place des objets retirés dans l'état avant.
    places_avant: Vec<(String, usize)>,
    /// Place des objets ajoutés dans l'état après.
    places_apres: Vec<(String, usize)>,
    /// Ordre complet avant et après, seulement si l'ordre des objets gardés change.
    ordre: Option<(Vec<String>, Vec<String>)>,
}

/// Texte JSON de l'identifiant de chaque objet d'un tableau, si tous en ont un et
/// qu'ils sont uniques.
fn cles(t: &[Value]) -> Option<Vec<String>> {
    let cles: Vec<String> = t.iter().map(|o| o.as_object()?.get("id").map(Value::to_string)).collect::<Option<_>>()?;
    (cles.iter().collect::<HashSet<_>>().len() == cles.len()).then_some(cles)
}

fn cle(o: &Value) -> String {
    o.get("id").map(Value::to_string).unwrap_or_default()
}

/// Différence de `a` vers `b`, `None` s'ils sont égaux.
pub fn calculer(a: &Value, b: &Value) -> Option<Difference> {
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            let mut champs = BTreeMap::new();
            for (k, va) in ma {
                match mb.get(k) {
                    Some(vb) => {
                        if let Some(d) = calculer(va, vb) {
                            champs.insert(k.clone(), d);
                        }
                    }
                    None => {
                        champs.insert(k.clone(), Difference::Valeur { avant: Some(va.clone()), apres: None });
                    }
                }
            }
            for (k, vb) in mb {
                if !ma.contains_key(k) {
                    champs.insert(k.clone(), Difference::Valeur { avant: None, apres: Some(vb.clone()) });
                }
            }
            (!champs.is_empty()).then_some(Difference::Objet(champs))
        }
        (Value::Array(ta), Value::Array(tb)) if !(ta.is_empty() && tb.is_empty()) => match (cles(ta), cles(tb)) {
            (Some(ca), Some(cb)) => calculer_collection(ta, tb, &ca, &cb),
            _ => (ta != tb).then(|| Difference::Valeur { avant: Some(a.clone()), apres: Some(b.clone()) }),
        },
        _ => (a != b).then(|| Difference::Valeur { avant: Some(a.clone()), apres: Some(b.clone()) }),
    }
}

/// Objet d'une collection : valeur lue, ou texte JSON brut lu seulement s'il change.
trait Element {
    fn difference(&self, autre: &Self) -> Option<Difference>;
    fn valeur(&self) -> Value;
}

impl Element for Value {
    fn difference(&self, autre: &Self) -> Option<Difference> {
        calculer(self, autre)
    }
    fn valeur(&self) -> Value {
        self.clone()
    }
}

impl Element for &RawValue {
    fn difference(&self, autre: &Self) -> Option<Difference> {
        if self.get() == autre.get() {
            return None;
        }
        calculer(&self.valeur(), &autre.valeur())
    }
    fn valeur(&self) -> Value {
        // un texte écrit par serde_json se relit toujours
        serde_json::from_str(self.get()).unwrap_or(Value::Null)
    }
}

/// Identifiant d'un objet, lu sans lire le reste.
#[derive(Deserialize)]
struct Cle<'a> {
    #[serde(borrow)]
    id: &'a RawValue,
}

fn cles_brutes(t: &[&RawValue]) -> Option<Vec<String>> {
    let cles: Vec<String> = t.iter().map(|r| serde_json::from_str::<Cle>(r.get()).ok().map(|c| c.id.get().to_string())).collect::<Option<_>>()?;
    (cles.iter().collect::<HashSet<_>>().len() == cles.len()).then_some(cles)
}

fn calculer_brut(a: &RawValue, b: &RawValue) -> Option<Difference> {
    if a.get() == b.get() {
        return None;
    }
    if a.get().starts_with('[') && b.get().starts_with('[') {
        fn lire(r: &RawValue) -> Option<Vec<&RawValue>> {
            serde_json::from_str(r.get()).ok()
        }
        if let (Some(ta), Some(tb)) = (lire(a), lire(b))
            && let (Some(ca), Some(cb)) = (cles_brutes(&ta), cles_brutes(&tb))
        {
            return calculer_collection(&ta, &tb, &ca, &cb);
        }
    }
    a.difference(&b)
}

/// `calculer` sur deux textes JSON écrits par serde_json (sans espaces) : les champs
/// de premier niveau et les objets des collections sont comparés texte à texte, seuls
/// ceux qui changent sont lus.
pub fn calculer_texte(a: &str, b: &str) -> Result<Option<Difference>, String> {
    type Brut<'a> = BTreeMap<String, &'a RawValue>;
    let (Ok(ma), Ok(mb)) = (serde_json::from_str::<Brut>(a), serde_json::from_str::<Brut>(b)) else {
        let lire = |t: &str| serde_json::from_str::<Value>(t).map_err(|e| e.to_string());
        return Ok(calculer(&lire(a)?, &lire(b)?));
    };
    let mut champs = BTreeMap::new();
    for (k, ra) in &ma {
        let d = match mb.get(k) {
            Some(rb) => calculer_brut(ra, rb),
            None => Some(Difference::Valeur { avant: Some(ra.valeur()), apres: None }),
        };
        if let Some(d) = d {
            champs.insert(k.clone(), d);
        }
    }
    for (k, rb) in &mb {
        if !ma.contains_key(k) {
            champs.insert(k.clone(), Difference::Valeur { avant: None, apres: Some(rb.valeur()) });
        }
    }
    Ok((!champs.is_empty()).then_some(Difference::Objet(champs)))
}

fn calculer_collection<E: Element>(ta: &[E], tb: &[E], ca: &[String], cb: &[String]) -> Option<Difference> {
    let ia: HashMap<&str, usize> = ca.iter().enumerate().map(|(i, c)| (c.as_str(), i)).collect();
    let ib: HashSet<&str> = cb.iter().map(String::as_str).collect();
    let mut c = Collection { objets: BTreeMap::new(), places_avant: Vec::new(), places_apres: Vec::new(), ordre: None };
    for (j, k) in cb.iter().enumerate() {
        match ia.get(k.as_str()) {
            Some(&i) => {
                if let Some(d) = ta[i].difference(&tb[j]) {
                    c.objets.insert(k.clone(), d);
                }
            }
            None => {
                c.objets.insert(k.clone(), Difference::Valeur { avant: None, apres: Some(tb[j].valeur()) });
                c.places_apres.push((k.clone(), j));
            }
        }
    }
    for (i, k) in ca.iter().enumerate() {
        if !ib.contains(k.as_str()) {
            c.objets.insert(k.clone(), Difference::Valeur { avant: Some(ta[i].valeur()), apres: None });
            c.places_avant.push((k.clone(), i));
        }
    }
    let gardes_a = ca.iter().filter(|k| ib.contains(k.as_str()));
    let gardes_b = cb.iter().filter(|k| ia.contains_key(k.as_str()));
    if !gardes_a.eq(gardes_b) {
        c.ordre = Some((ca.to_vec(), cb.to_vec()));
    }
    (!c.objets.is_empty() || c.ordre.is_some()).then_some(Difference::Collection(c))
}

impl Collection {
    /// Identifiants (texte JSON) des objets présents avant et après, modifiés.
    pub(crate) fn modifies(&self) -> Vec<String> {
        self.objets.iter().filter(|(_, d)| !matches!(d, Difference::Valeur { avant: None, .. } | Difference::Valeur { apres: None, .. })).map(|(k, _)| k.clone()).collect()
    }
}

fn cote(avant: &Option<Value>, apres: &Option<Value>, vers_apres: bool) -> Option<Value> {
    if vers_apres { apres.clone() } else { avant.clone() }
}

/// Applique la différence à `v`, vers l'état après (`vers_apres`) ou avant.
pub fn appliquer(v: &mut Value, d: &Difference, vers_apres: bool) -> Result<(), String> {
    match d {
        Difference::Valeur { avant, apres } => {
            *v = cote(avant, apres, vers_apres).unwrap_or(Value::Null);
            Ok(())
        }
        Difference::Objet(champs) => {
            let m = v.as_object_mut().ok_or("Historique incohérent : objet attendu.")?;
            for (k, d) in champs {
                match d {
                    Difference::Valeur { avant, apres } => match cote(avant, apres, vers_apres) {
                        Some(x) => {
                            m.insert(k.clone(), x);
                        }
                        None => {
                            m.remove(k);
                        }
                    },
                    _ => appliquer(m.get_mut(k).ok_or_else(|| format!("Historique incohérent : champ « {k} » absent."))?, d, vers_apres)?,
                }
            }
            Ok(())
        }
        Difference::Collection(c) => appliquer_collection(v, c, vers_apres),
    }
}

fn appliquer_collection(v: &mut Value, c: &Collection, vers_apres: bool) -> Result<(), String> {
    let t = v.as_array_mut().ok_or("Historique incohérent : tableau attendu.")?;
    let (retires, ajoutes) = if vers_apres { (&c.places_avant, &c.places_apres) } else { (&c.places_apres, &c.places_avant) };
    if !retires.is_empty() {
        let r: HashSet<&str> = retires.iter().map(|(k, _)| k.as_str()).collect();
        t.retain(|o| !r.contains(cle(o).as_str()));
    }
    let a_modifier: Vec<(&String, &Difference)> = c.objets.iter().filter(|(_, d)| !matches!(d, Difference::Valeur { avant: None, .. } | Difference::Valeur { apres: None, .. })).collect();
    if !a_modifier.is_empty() {
        let index: HashMap<String, usize> = t.iter().enumerate().map(|(i, o)| (cle(o), i)).collect();
        for (k, d) in a_modifier {
            let i = *index.get(k).ok_or_else(|| format!("Historique incohérent : objet {k} absent."))?;
            appliquer(&mut t[i], d, vers_apres)?;
        }
    }
    let mut places = ajoutes.clone();
    places.sort_by_key(|(_, i)| *i);
    for (k, i) in places {
        let Some(Difference::Valeur { avant, apres }) = c.objets.get(&k) else { return Err(format!("Historique incohérent : objet {k} sans valeur.")) };
        let o = cote(avant, apres, vers_apres).ok_or_else(|| format!("Historique incohérent : objet {k} sans valeur."))?;
        t.insert(i.min(t.len()), o);
    }
    if let Some((oa, ob)) = &c.ordre {
        let ordre = if vers_apres { ob } else { oa };
        let mut par_cle: HashMap<String, Value> = t.drain(..).map(|o| (cle(&o), o)).collect();
        for k in ordre {
            t.push(par_cle.remove(k).ok_or_else(|| format!("Historique incohérent : objet {k} absent."))?);
        }
    }
    Ok(())
}

/// Identifiant lisible : texte d'un identifiant chaîne, sinon son JSON.
fn lisible(cle: &str) -> String {
    serde_json::from_str::<String>(cle).unwrap_or_else(|_| cle.to_string())
}

/// Identifiants des objets ajoutés, retirés ou modifiés par la différence (objets des
/// collections, à toute profondeur, y compris dans les valeurs gardées entières).
pub fn identifiants(d: &Difference, ids: &mut BTreeSet<String>) {
    fn dans_valeur(v: &Value, ids: &mut BTreeSet<String>) {
        match v {
            Value::Array(t) => t.iter().for_each(|o| dans_valeur(o, ids)),
            Value::Object(m) => {
                if let Some(id) = m.get("id") {
                    ids.insert(lisible(&id.to_string()));
                }
                m.values().for_each(|x| dans_valeur(x, ids));
            }
            _ => {}
        }
    }
    match d {
        Difference::Valeur { avant, apres } => avant.iter().chain(apres).for_each(|v| dans_valeur(v, ids)),
        Difference::Objet(champs) => champs.values().for_each(|d| identifiants(d, ids)),
        Difference::Collection(c) => {
            for (k, d) in &c.objets {
                ids.insert(lisible(k));
                identifiants(d, ids);
            }
            if let Some((oa, ob)) = &c.ordre {
                // objets seulement déplacés dans l'ordre
                let (a, b): (Vec<&String>, Vec<&String>) = (oa.iter().collect(), ob.iter().collect());
                for (x, y) in a.iter().zip(&b) {
                    if x != y {
                        ids.insert(lisible(x));
                        ids.insert(lisible(y));
                    }
                }
            }
        }
    }
}

/// Champs de premier niveau modifiés (par exemple `murs`, `nom`, `soleil`).
pub fn champs(d: &Difference) -> Vec<String> {
    match d {
        Difference::Objet(m) => m.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

/// Taille approximative d'une différence (octets du JSON), pour les mesures.
pub fn taille(d: &Difference) -> usize {
    serde_json::to_vec(d).map_or(0, |v| v.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn aller_retour(a: Value, b: Value) {
        let d = calculer(&a, &b).unwrap();
        assert_eq!(calculer_texte(&a.to_string(), &b.to_string()).unwrap().unwrap(), d);
        let mut x = a.clone();
        appliquer(&mut x, &d, true).unwrap();
        assert_eq!(x, b);
        appliquer(&mut x, &d, false).unwrap();
        assert_eq!(x, a);
    }

    #[test]
    fn ajouts_retraits_modifications_et_ordre() {
        let a = json!({"nom": "p", "murs": [{"id": "a", "x": 1}, {"id": "b", "x": 2}, {"id": "c", "x": 3}], "pts": [[0, 0], [1, 1]]});
        aller_retour(a.clone(), json!({"nom": "q", "murs": [{"id": "a", "x": 1}, {"id": "c", "x": 4}, {"id": "d"}], "pts": [[0, 0]]}));
        aller_retour(a.clone(), json!({"murs": [{"id": "c", "x": 3}, {"id": "a", "x": 1}, {"id": "b", "x": 2}], "soleil": {"h": 1}}));
        aller_retour(a.clone(), json!({"nom": "p", "murs": [], "pts": [[0, 0], [1, 1]]}));
        aller_retour(json!({"murs": []}), a);
        assert!(calculer(&json!({"t": []}), &json!({"t": []})).is_none());
    }

    #[test]
    fn seuls_les_objets_touches_sont_gardes() {
        let murs: Vec<Value> = (0..1000).map(|i| json!({"id": format!("m{i}"), "debut": [i, 0], "fin": [i, 100], "nom": "Mur"})).collect();
        let a = json!({ "murs": murs });
        let mut b = a.clone();
        b["murs"][500]["debut"] = json!([500, 10]);
        let d = calculer(&a, &b).unwrap();
        assert!(taille(&d) < 200, "{}", taille(&d));
        let mut ids = BTreeSet::new();
        identifiants(&d, &mut ids);
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), ["m500"]);
        assert_eq!(champs(&d), ["murs"]);
    }
}
