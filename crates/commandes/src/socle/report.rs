//! Report d'une modification sur une sélection multiple, commun au projet et aux
//! familles : Propriétés montre l'élément principal ; ce qui y change est reporté, champ
//! par champ, sur les autres éléments sélectionnés de même sorte. Le nom, l'identité et la
//! géométrie propre de chaque élément (`PROPRES`) ne sont jamais reportés.
use modele::Projet;
use serde_json::{Map, Value};

/// Clés jamais reportées, à toute profondeur : identité, rattachement et géométrie propres
/// à chaque élément (projet et familles).
pub const PROPRES: &[&str] = &[
    // identité
    "id", "nom", "numero", "origine",
    // rattachement
    "assemblage", "couches", "mur", "escalier", "vue",
    // géométrie du projet
    "axe", "contours", "contour", "points", "point", "position", "debut", "fin", "trace", "abscisse", "rotation", "pentes", "talus_dessus", "remblais",
    // géométrie des familles
    "min", "max", "profil", "ancrage", "centre", "support", "chemin",
];

/// Un changement : chemin dans l'objet (clés et indices) et nouvelle valeur (`Null` :
/// champ retiré, la valeur par défaut revient à la lecture).
pub type Changement = (Vec<Value>, Value);

/// Changements de `avant` à `apres`, feuille par feuille, hors `PROPRES` ; une liste dont
/// la longueur change est reportée en entier.
pub fn differences(avant: &Value, apres: &Value) -> Vec<Changement> {
    let mut out = Vec::new();
    parcourir(avant, apres, &mut Vec::new(), &mut out);
    out
}

fn parcourir(avant: &Value, apres: &Value, chemin: &mut Vec<Value>, out: &mut Vec<Changement>) {
    match (avant, apres) {
        (Value::Object(a), Value::Object(b)) => {
            let cles: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for c in cles.into_iter().filter(|c| !PROPRES.contains(&c.as_str())) {
                chemin.push(Value::String(c.clone()));
                parcourir(a.get(c).unwrap_or(&Value::Null), b.get(c).unwrap_or(&Value::Null), chemin, out);
                chemin.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                chemin.push(Value::from(i));
                parcourir(x, y, chemin, out);
                chemin.pop();
            }
        }
        (a, b) if a != b => out.push((chemin.clone(), b.clone())),
        _ => {}
    }
}

/// Applique les changements à `cible` (objets créés au besoin) ; un indice de liste
/// absent de la cible est ignoré.
pub fn appliquer(cible: &mut Value, changements: &[Changement]) {
    for (chemin, valeur) in changements {
        ecrire(cible, chemin, valeur);
    }
}

fn ecrire(cible: &mut Value, chemin: &[Value], valeur: &Value) {
    let Some((tete, reste)) = chemin.split_first() else {
        *cible = valeur.clone();
        return;
    };
    match (cible, tete) {
        (Value::Object(o), Value::String(c)) if reste.is_empty() => {
            if valeur.is_null() {
                o.remove(c);
            } else {
                o.insert(c.clone(), valeur.clone());
            }
        }
        (Value::Object(o), Value::String(c)) => ecrire(o.entry(c.clone()).or_insert_with(|| Value::Object(Map::new())), reste, valeur),
        (Value::Array(l), Value::Number(i)) => {
            if let Some(x) = i.as_u64().and_then(|i| l.get_mut(i as usize)) {
                ecrire(x, reste, valeur);
            }
        }
        _ => {}
    }
}

/// Projet `apres` dont les éléments `autres` reçoivent ce qui a changé sur l'élément
/// `principal` depuis `avant`. Seuls les éléments de la même collection que le principal
/// (même sorte) sont touchés. `apres` inchangé si le principal n'a pas changé.
pub fn reporter_projet(avant: &Projet, apres: Projet, principal: &str, autres: &[String]) -> Result<Projet, String> {
    let json = |p: &Projet| serde_json::to_value(p).map_err(|e| e.to_string());
    let a_id = |o: &Value, id: &str| o.get("id").and_then(Value::as_str) == Some(id);
    let va = json(avant)?;
    let Some((collection, objet_avant)) = va.as_object().into_iter().flatten().find_map(|(cle, v)| v.as_array()?.iter().find(|o| a_id(o, principal)).map(|o| (cle.clone(), o))) else {
        return Ok(apres);
    };
    let mut vb = json(&apres)?;
    let Some(liste) = vb.get_mut(&collection).and_then(Value::as_array_mut) else { return Ok(apres) };
    let Some(objet_apres) = liste.iter().find(|o| a_id(o, principal)) else { return Ok(apres) };
    let changements = differences(objet_avant, objet_apres);
    if changements.is_empty() {
        return Ok(apres);
    }
    for o in liste.iter_mut().filter(|o| autres.iter().any(|id| id != principal && a_id(o, id))) {
        appliquer(o, &changements);
    }
    serde_json::from_value(vb).map_err(|e| format!("Report sur la sélection : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projet() -> Projet {
        Projet::lire_json(include_str!("../../../../samples/maison_native.modele.json")).unwrap()
    }

    #[test]
    fn champ_change_reporte_seul_sur_la_meme_sorte() {
        let p = projet();
        let murs: Vec<String> = p.murs.iter().filter(|m| m.assemblage.is_none()).take(3).map(|m| m.id.clone()).collect();
        assert_eq!(murs.len(), 3);
        let dalle = p.dalles[0].id.clone();
        let mut q = p.clone();
        let m0 = q.murs.iter_mut().find(|m| m.id == murs[0]).unwrap();
        m0.decalage_bas = 123.0;
        m0.nom = "Renommé".into();
        m0.axe = modele::Axe::Droit { debut: [1.0, 2.0], fin: [3.0, 4.0] };
        let mut autres = murs.clone();
        autres.push(dalle.clone());
        let r = reporter_projet(&p, q, &murs[0], &autres).unwrap();
        for id in &murs[1..] {
            let (avant, apres) = (p.murs.iter().find(|m| &m.id == id).unwrap(), r.murs.iter().find(|m| &m.id == id).unwrap());
            assert_eq!(apres.decalage_bas, 123.0);
            // nom et géométrie de chacun gardés
            assert_eq!((&apres.nom, &apres.axe), (&avant.nom, &avant.axe));
        }
        // autre sorte : intacte
        assert_eq!(serde_json::to_value(&r.dalles[0]).unwrap(), serde_json::to_value(&p.dalles[0]).unwrap());
    }

    #[test]
    fn champ_retire_et_liste_d_autre_longueur() {
        let avant = serde_json::json!({"a": {"x": 1, "y": 2}, "l": [1, 2], "nom": "A"});
        let apres = serde_json::json!({"a": {"x": 5, "y": 2}, "l": [1, 2, 3], "nom": "B"});
        let mut cible = serde_json::json!({"a": {"x": 0, "y": 9}, "l": [7], "nom": "C", "z": true});
        appliquer(&mut cible, &differences(&avant, &apres));
        assert_eq!(cible, serde_json::json!({"a": {"x": 5, "y": 9}, "l": [1, 2, 3], "nom": "C", "z": true}));
        // champ retiré du principal : retiré de la cible
        appliquer(&mut cible, &differences(&serde_json::json!({"z": true}), &serde_json::json!({})));
        assert!(cible.get("z").is_none());
    }
}
