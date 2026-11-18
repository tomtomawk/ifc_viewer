//! Fusion d'un projet importé (IFC converti) dans le projet ouvert : niveaux rapprochés,
//! types, matériaux et profils réutilisés s'ils sont identiques, identifiants renouvelés.
use crate::socle::interne::*;
use serde::Serialize;
use serde_json::{Map, Value};

/// Tolérance de rapprochement des niveaux par altitude (mm).
const TOLERANCE_NIVEAU: f64 = 10.0;

pub(crate) trait ExecutionFusion {
    fn fusionner(&mut self, import: Projet, decalage: [f64; 3]) -> Result<Vec<String>, String>;
}

/// Contenu d'un élément sans son identité (identifiant, nom, origine) : sert à reconnaître
/// deux types ou matériaux identiques.
fn contenu<T: Serialize>(x: &T) -> Value {
    let mut v = serde_json::to_value(x).unwrap_or(Value::Null);
    if let Value::Object(o) = &mut v {
        for cle in ["id", "nom", "origine"] { o.remove(cle); }
    }
    v
}

/// Nom libre parmi `pris` : `nom`, sinon « nom (import) », « nom (import 2) »…
fn nom_libre(nom: &str, pris: &HashSet<String>) -> String {
    if !pris.contains(nom) { return nom.to_string(); }
    let mut n = 1;
    loop {
        let candidat = if n == 1 { format!("{nom} (import)") } else { format!("{nom} (import {n})") };
        if !pris.contains(&candidat) { return candidat; }
        n += 1;
    }
}

/// Remplace les identifiants (valeurs et clés) selon `table`, sauf sous les clés `nom`
/// et `origine` (textes libres et références à la source).
fn renommer_ids(v: &mut Value, table: &HashMap<String, String>) {
    match v {
        Value::String(s) => { if let Some(n) = table.get(s.as_str()) { *s = n.clone(); } }
        Value::Array(a) => a.iter_mut().for_each(|x| renommer_ids(x, table)),
        Value::Object(o) => {
            let anciens = std::mem::take(o);
            let mut nouveau = Map::new();
            for (cle, mut val) in anciens {
                if cle != "nom" && cle != "origine" { renommer_ids(&mut val, table); }
                nouveau.insert(table.get(&cle).cloned().unwrap_or(cle), val);
            }
            *o = nouveau;
        }
        _ => {}
    }
}

/// Remplace la valeur texte des clés `cle` selon `table` (noms de matériaux, sources).
fn renommer_cle(v: &mut Value, cle: &str, table: &HashMap<String, String>) {
    match v {
        Value::Array(a) => a.iter_mut().for_each(|x| renommer_cle(x, cle, table)),
        Value::Object(o) => {
            for (k, val) in o.iter_mut() {
                if k == cle && let Value::String(s) = val && let Some(n) = table.get(s.as_str()) {
                    *s = n.clone();
                    continue;
                }
                renommer_cle(val, cle, table);
            }
        }
        _ => {}
    }
}

fn nouvel_id(ancien: &str) -> String {
    let genre = ancien.split(['-', ':']).next().filter(|g| !g.is_empty() && g.len() <= 24 && g.chars().all(|c| c.is_ascii_alphabetic() || c == '_')).unwrap_or("import");
    format!("{genre}-{}", noyau::uuid::uuid())
}

/// Rapproche des types identifiés par leur contenu : identifiant existant si un type
/// identique existe, sinon nouvel identifiant et nom distinct.
fn rapprocher<T: Serialize>(existants: &[T], importes: &mut [T], id: fn(&mut T) -> &mut String, nom: fn(&mut T) -> &mut String, table: &mut HashMap<String, String>, existants_ids: impl Fn(&T) -> (String, String)) {
    let connus: Vec<(Value, String)> = existants.iter().map(|e| (contenu(e), existants_ids(e).0)).collect();
    let mut noms: HashSet<String> = existants.iter().map(|e| existants_ids(e).1).collect();
    for t in importes.iter_mut() {
        let c = contenu(t);
        let ancien = id(t).clone();
        if let Some((_, existant)) = connus.iter().find(|(v, _)| *v == c) {
            table.insert(ancien, existant.clone());
        } else {
            let n = nom_libre(nom(t), &noms);
            noms.insert(n.clone());
            *nom(t) = n;
            table.insert(ancien.clone(), nouvel_id(&ancien));
        }
    }
}

macro_rules! rapprocher_types {
    ($self:ident, $imp:ident, $champ:ident, $table:ident) => {
        rapprocher(&$self.$champ, &mut $imp.$champ, |t| &mut t.id, |t| &mut t.nom, &mut $table, |t| (t.id.clone(), t.nom.clone()))
    };
}

impl ExecutionFusion for Projet {
    fn fusionner(&mut self, mut imp: Projet, decalage: [f64; 3]) -> Result<Vec<String>, String> {
        imp.verifier().map_err(|e| format!("Projet importé non valide : {e}"))?;
        if !decalage.iter().all(|x| x.is_finite()) { return Err("Décalage non valide.".into()); }
        // sources : clés renumérotées à la suite de celles du projet
        let mut sources = HashMap::new();
        for s in &mut imp.sources {
            let mut n = self.sources.len() + sources.len() + 1;
            while self.sources.iter().any(|x| x.id == format!("s{n}")) { n += 1; }
            sources.insert(s.id.clone(), format!("s{n}"));
            s.id = format!("s{n}");
        }
        // matériaux (référencés par leur nom) : identiques réutilisés, sinon renommés
        let mut noms_mat: HashSet<String> = self.materiaux.iter().map(|m| m.nom.clone()).collect();
        let mut table_mat = HashMap::new();
        let mut materiaux = Vec::new();
        for mut m in std::mem::take(&mut imp.materiaux) {
            let c = contenu(&m);
            let identique = |e: &&Materiau| contenu(*e) == c;
            if let Some(e) = self.materiaux.iter().filter(identique).find(|e| e.nom == m.nom).or_else(|| self.materiaux.iter().find(identique)) {
                table_mat.insert(m.nom.clone(), e.nom.clone());
                continue;
            }
            let n = nom_libre(&m.nom, &noms_mat);
            noms_mat.insert(n.clone());
            table_mat.insert(m.nom.clone(), n.clone());
            m.nom = n;
            m.id = noyau::uuid::uuid();
            materiaux.push(m);
        }
        let mut v = serde_json::to_value(&imp).map_err(|e| e.to_string())?;
        renommer_cle(&mut v, "materiau", &table_mat);
        renommer_cle(&mut v, "source", &sources);
        imp = serde_json::from_value(v).map_err(|e| e.to_string())?;
        // profils, puis types (qui citent les profils par identifiant)
        let mut table = HashMap::new();
        rapprocher_types!(self, imp, profils, table);
        let mut v = serde_json::to_value(&imp).map_err(|e| e.to_string())?;
        renommer_ids(&mut v, &table);
        imp = serde_json::from_value(v).map_err(|e| e.to_string())?;
        rapprocher_types!(self, imp, types_murs, table);
        rapprocher_types!(self, imp, types_dalles, table);
        rapprocher_types!(self, imp, types_ouvertures, table);
        rapprocher_types!(self, imp, types_poutres, table);
        rapprocher_types!(self, imp, types_composants, table);
        rapprocher_types!(self, imp, types_garde_corps, table);
        // niveaux : même altitude (à 1 cm), sinon même nom, sinon ajoutés
        for n in &imp.niveaux {
            let existant = self.niveaux.iter().find(|e| (e.altitude - n.altitude).abs() <= TOLERANCE_NIVEAU)
                .or_else(|| self.niveaux.iter().find(|e| e.nom == n.nom));
            table.insert(n.id.clone(), existant.map(|e| e.id.clone()).unwrap_or_else(|| nouvel_id(&n.id)));
        }
        // éléments : tous les identifiants renouvelés
        let niveaux: HashSet<String> = imp.niveaux.iter().map(|n| n.id.clone()).collect();
        for id in imp.identifiants() {
            if !niveaux.contains(&id) { table.insert(id.clone(), nouvel_id(&id)); }
        }
        let mut v = serde_json::to_value(&imp).map_err(|e| e.to_string())?;
        renommer_ids(&mut v, &table);
        imp = serde_json::from_value(v).map_err(|e| e.to_string())?;
        // ajout : ce qui ne désigne pas déjà un objet du projet
        let connus = self.identifiants();
        macro_rules! ajouter {
            ($($champ:ident),*) => {{
                let mut ids = Vec::new();
                $(
                    let presents: HashSet<String> = self.$champ.iter().map(|x| x.id.clone()).collect();
                    for x in std::mem::take(&mut imp.$champ) {
                        if presents.contains(&x.id) { continue; }
                        if connus.contains(&x.id) { return Err(format!("Identifiant importé en collision : {}", x.id)); }
                        ids.push(x.id.clone());
                        self.$champ.push(x);
                    }
                )*
                ids
            }};
        }
        let _ = ajouter!(niveaux, profils, types_murs, types_dalles, types_ouvertures, types_poutres, types_composants, types_garde_corps);
        let elements = ajouter!(murs, assemblages, ouvertures, dalles, terrains, limites_propriete, barres, escaliers, pieces, composants, arbres, garde_corps, importes, quadrillages);
        self.materiaux.extend(materiaux);
        self.sources.extend(imp.sources);
        for (id, r) in imp.implantations { if elements.contains(&id) { self.implantations.insert(id, r); } }
        for (id, p) in imp.priorites_jonction { if elements.contains(&id) { self.priorites_jonction.insert(id, p); } }
        for (id, a) in imp.attaches_toitures { if elements.contains(&id) { self.attaches_toitures.insert(id, a); } }
        for (cat, n) in imp.ignores { *self.ignores.entry(cat).or_default() += n; }
        if decalage != [0.0; 3] && !elements.is_empty() {
            let racines: Vec<String> = elements.iter().filter(|id| !self.murs.iter().any(|m| &m.id == *id && m.assemblage.is_some())).cloned().collect();
            let set = self.dependances_avec(&racines, true)?;
            self.transformer(&set, decalage, [0.0; 2], 0.0)?;
        }
        Ok(elements)
    }
}
