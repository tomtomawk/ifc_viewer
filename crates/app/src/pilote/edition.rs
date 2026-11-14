//! Modification atomique du document par JSON Patch (RFC 6902).
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modification {
    pub empreinte: String,
    pub operations: Vec<Operation>,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum Operation {
    Add { path: String, value: Value },
    Replace { path: String, value: Value },
    Remove { path: String },
    Test { path: String, value: Value },
    Copy { from: String, path: String },
    Move { from: String, path: String },
}

fn parent<'a>(v: &'a mut Value, path: &str) -> Result<(&'a mut Value, String), String> {
    let (p, cle) = path.rsplit_once('/').ok_or("Chemin JSON Pointer requis")?;
    if cle.replace("~0", "").replace("~1", "").contains('~') {
        return Err("Échappement JSON Pointer invalide".into());
    }
    let cle = cle.replace("~1", "/").replace("~0", "~");
    Ok((
        v.pointer_mut(p)
            .ok_or_else(|| format!("Parent absent : {p}"))?,
        cle,
    ))
}

fn index(cle: &str, taille: usize, ajouter: bool) -> Result<usize, String> {
    if ajouter && cle == "-" {
        return Ok(taille);
    }
    if cle.is_empty() || !cle.bytes().all(|c| c.is_ascii_digit()) {
        return Err("Indice de tableau invalide".into());
    }
    if cle.len() > 1 && cle.starts_with('0') {
        return Err("Indice avec zéro initial".into());
    }
    let i = cle
        .parse::<usize>()
        .map_err(|_| "Indice de tableau invalide")?;
    if i > taille || (!ajouter && i == taille) {
        return Err("Indice hors tableau".into());
    }
    Ok(i)
}

fn retirer(v: &mut Value, path: &str) -> Result<Value, String> {
    if path.is_empty() {
        return Ok(std::mem::replace(v, Value::Null));
    }
    let (parent, cle) = parent(v, path)?;
    match parent {
        Value::Object(m) => m
            .remove(&cle)
            .ok_or_else(|| format!("Champ absent : {path}")),
        Value::Array(a) => {
            let i = index(&cle, a.len(), false)?;
            Ok(a.remove(i))
        }
        _ => Err("Le parent doit être un objet ou un tableau".into()),
    }
}

fn ajouter(v: &mut Value, path: &str, value: Value) -> Result<(), String> {
    if path.is_empty() {
        *v = value;
        return Ok(());
    }
    let (parent, cle) = parent(v, path)?;
    match parent {
        Value::Object(m) => {
            m.insert(cle, value);
        }
        Value::Array(a) => {
            let i = index(&cle, a.len(), true)?;
            a.insert(i, value);
        }
        _ => return Err("Le parent doit être un objet ou un tableau".into()),
    }
    Ok(())
}

pub fn modifier(
    actuel: &crate::modele::Projet,
    m: Modification,
) -> Result<crate::modele::Projet, String> {
    let mut v = serde_json::to_value(actuel).map_err(|e| e.to_string())?;
    if noyau::hash::hash(&v) != m.empreinte {
        return Err("Le projet a changé : relire projet_natif avant de modifier".into());
    }
    if m.operations.is_empty() {
        return Err("Aucune opération".into());
    }
    for op in m.operations {
        match op {
            Operation::Test { path, value } => {
                if v.pointer(&path) != Some(&value) {
                    return Err(format!("Test échoué : {path}"));
                }
            }
            Operation::Add { path, value } => ajouter(&mut v, &path, value)?,
            Operation::Replace { path, value } => {
                *v.pointer_mut(&path)
                    .ok_or_else(|| format!("Champ absent : {path}"))? = value;
            }
            Operation::Remove { path } => {
                retirer(&mut v, &path)?;
            }
            Operation::Copy { from, path } => {
                let value = v.pointer(&from).ok_or("Source absente")?.clone();
                ajouter(&mut v, &path, value)?;
            }
            Operation::Move { from, path } => {
                if path.starts_with(&format!("{from}/")) {
                    return Err("Déplacement dans son propre enfant".into());
                }
                let value = retirer(&mut v, &from)?;
                ajouter(&mut v, &path, value)?;
            }
        }
    }
    let mut ignores = Vec::new();
    let p: crate::modele::Projet =
        serde_ignored::deserialize(v, |path| ignores.push(path.to_string()))
            .map_err(|e| e.to_string())?;
    if !ignores.is_empty() {
        return Err(format!("Champs inconnus : {}", ignores.join(", ")));
    }
    if p.identite != actuel.identite || p.format != actuel.format || p.version != actuel.version {
        return Err("Identité, format et version du projet sont immuables".into());
    }
    p.verifier()?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn projet() -> crate::modele::Projet {
        crate::modele::Projet::depuis_gabarit("Test", &crate::config::gabarit::Gabarit::defaut())
    }
    fn modification(p: &crate::modele::Projet, operations: Value) -> Modification {
        serde_json::from_value(json!({"empreinte":noyau::hash::hash(&serde_json::to_value(p).unwrap()),"operations":operations})).unwrap()
    }
    #[test]
    fn transaction_atomique_et_concurrence() {
        let p = projet();
        let q = modifier(
            &p,
            modification(&p, json!([{"op":"replace","path":"/nom","value":"Agence"}])),
        )
        .unwrap();
        assert_eq!(q.nom, "Agence");
        assert!(
            modifier(
                &q,
                modification(&p, json!([{"op":"replace","path":"/nom","value":"Périmé"}]))
            )
            .is_err()
        );
        assert!(modifier(&p, modification(&p, json!([{"op":"replace","path":"/nom","value":"Perdu"},{"op":"test","path":"/nom","value":"Autre"}]))).is_err());
        assert_eq!(p.nom, "Test");
    }
    #[test]
    fn identite_et_champs_inconnus_refuses() {
        let p = projet();
        for (path, value) in [
            ("/identite", json!("autre")),
            ("/inconnu", json!(null)),
            ("/niveaux/0/altitudee", json!(100)),
        ] {
            assert!(
                modifier(
                    &p,
                    modification(&p, json!([{"op":"add","path":path,"value":value}]))
                )
                .is_err()
            );
        }
    }
    #[test]
    fn tableaux_et_pointeurs() {
        let mut v = json!({"a/b":[1,2],"~":3});
        ajouter(&mut v, "/a~1b/-", json!(4)).unwrap();
        assert_eq!(retirer(&mut v, "/a~1b/1").unwrap(), json!(2));
        assert_eq!(retirer(&mut v, "/~0").unwrap(), json!(3));
        assert!(retirer(&mut v, "/a~1b/01").is_err());
    }
}
