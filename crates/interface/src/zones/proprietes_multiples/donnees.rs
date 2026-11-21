use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Groupes = BTreeMap<(&'static str, String), Vec<Value>>;

// La même liste sert à lire la sélection et à appliquer une modification. Les
// données de type partagées sont traitées par les sélecteurs dédiés.
macro_rules! collections {
    ($f:ident) => {
        $f!(murs, "Murs");
        $f!(assemblages, "Murs composés");
        $f!(dalles, "Éléments horizontaux");
        $f!(terrains, "Terrains");
        $f!(limites_propriete, "Limites de propriété");
        $f!(barres, "Barres");
        $f!(composants, "Composants");
        $f!(escaliers, "Escaliers");
        $f!(garde_corps, "Garde-corps");
        $f!(ouvertures, "Ouvertures");
        $f!(importes, "Éléments importés");
        $f!(niveaux, "Niveaux");
        $f!(pieces, "Pièces");
        $f!(quadrillages, "Quadrillages");
        $f!(annotations, "Annotations");
        $f!(vues, "Vues");
    };
}

pub(super) fn groupes(projet: &modele::Projet, ids: &[String]) -> Result<Groupes, String> {
    let mut cibles: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    // Les contraintes d'un mur composé résident sur ses couches.
    for a in projet.assemblages.iter().filter(|a| ids.contains(&a.id)) {
        cibles.extend(a.couches.iter().map(String::as_str));
    }
    let mut groupes = Groupes::new();
    macro_rules! lire {
        ($champ:ident, $libelle:literal) => {
            for objet in projet
                .$champ
                .iter()
                .filter(|o| cibles.contains(o.id.as_str()))
            {
                let v = encoder(objet)?;
                let variante = v
                    .get("forme")
                    .or_else(|| v.get("genre"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let libelle = match variante {
                    "poteau" => "Poteaux",
                    "poutre" => "Poutres",
                    "porte" => "Portes",
                    "fenetre" => "Fenêtres",
                    "libre" => "Ouvertures libres",
                    _ => $libelle,
                };
                groupes
                    .entry((libelle, variante.to_owned()))
                    .or_default()
                    .push(v);
            }
        };
    }
    collections!(lire);
    Ok(groupes)
}

fn encoder(objet: &impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(objet).map_err(|e| e.to_string())
}

fn appliquer(
    mut projet: modele::Projet,
    cibles: &BTreeSet<&str>,
    mut modifier: impl FnMut(&mut Value),
) -> Result<modele::Projet, String> {
    macro_rules! editer {
        ($champ:ident, $libelle:literal) => {
            for objet in projet
                .$champ
                .iter_mut()
                .filter(|o| cibles.contains(o.id.as_str()))
            {
                let mut v = encoder(objet)?;
                modifier(&mut v);
                *objet =
                    serde_json::from_value(v).map_err(|e| format!("Propriété invalide : {e}"))?;
            }
        };
    }
    collections!(editer);
    Ok(projet)
}

#[derive(Clone)]
pub(super) enum Modification {
    Champ(Vec<String>, Value),
    ContrainteHaute(Value),
    Arete {
        contour: usize,
        segment: usize,
        pente: Option<Option<f64>>,
        remblai: Option<bool>,
    },
    ParametreOccurrence {
        cibles: BTreeSet<String>,
        table: String,
        nom: String,
        valeur: Value,
    },
}

fn ecrire(v: &mut Value, chemin: &[String], valeur: Value) {
    if let Some((cle, suite)) = chemin.split_first() {
        if v.is_array() {
            if let Some(x) = cle.parse::<usize>().ok().and_then(|i| v.get_mut(i)) {
                ecrire(x, suite, valeur);
            }
        } else if v.is_object() {
            ecrire(&mut v[cle], suite, valeur);
        }
    } else {
        *v = valeur;
    }
}

pub(super) fn appliquer_champ(
    original: &modele::Projet,
    cibles: &BTreeSet<&str>,
    modification: &Modification,
) -> Result<modele::Projet, String> {
    if let Modification::Arete {
        contour,
        segment,
        pente,
        remblai,
    } = *modification
    {
        let mut projet = original.clone();
        for dalle in projet
            .dalles
            .iter_mut()
            .filter(|d| cibles.contains(d.id.as_str()))
        {
            if let Some(pente) = pente {
                dalle.definir_pente(contour, segment, pente);
                if pente.is_none() {
                    dalle.definir_remblai(contour, segment, false);
                }
            }
            if let Some(remblai) = remblai {
                dalle.definir_remblai(contour, segment, remblai);
            }
        }
        return Ok(projet);
    }
    let mut projet = appliquer(original.clone(), cibles, |v| match modification {
        Modification::Arete { .. } => unreachable!(),
        Modification::Champ(chemin, valeur) => ecrire(v, chemin, valeur.clone()),
        Modification::ParametreOccurrence {
            cibles,
            table,
            nom,
            valeur,
        } => {
            if !v["id"].as_str().is_some_and(|id| cibles.contains(id)) {
                return;
            }
            if valeur.is_null() {
                if let Some(map) = v.get_mut(table).and_then(Value::as_object_mut) {
                    map.remove(nom);
                }
            } else {
                if v[table].is_null() {
                    v[table] = json!({});
                }
                v[table][nom] = valeur.clone();
            }
        }
        Modification::ContrainteHaute(niveau) => {
            let haut = &v["haut"];
            if niveau.is_null() {
                if haut["contrainte"] == "niveau"
                    && let Some(mur) = original.murs.iter().find(|m| v["id"] == m.id)
                    && let Some((bas, haut)) = original.bas_haut(mur)
                {
                    v["haut"] = json!({"contrainte":"libre", "hauteur":haut-bas});
                }
            } else if haut["niveau"] != *niveau {
                let decalage = haut["decalage"].as_f64().unwrap_or(0.0);
                v["haut"] = json!({"contrainte":"niveau", "niveau":niveau, "decalage":decalage});
            }
        }
    })?;
    if let Modification::Champ(chemin, _) = modification
        && chemin.as_slice() == ["nom"]
    {
        for n in projet
            .niveaux
            .iter()
            .filter(|n| cibles.contains(n.id.as_str()))
        {
            if n.nom.trim().is_empty()
                || projet.niveaux.iter().any(|autre| {
                    autre.id != n.id && autre.nom.trim().eq_ignore_ascii_case(n.nom.trim())
                })
            {
                return Err("Chaque niveau doit avoir un nom distinct.".into());
            }
        }
    }
    projet.actualiser_pieces();
    Ok(projet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projet() -> modele::Projet {
        modele::Projet::lire_json(include_str!(
            "../../../../../samples/maison_native.bem"
        ))
        .unwrap()
    }

    #[test]
    fn contrainte_haute_rechoisie_sur_une_selection_mixte() {
        let mut p = projet();
        let niveau = "niveau-test-haut".to_owned();
        p.niveaux.push(modele::Niveau {
            id: niveau.clone(),
            nom: "Niveau de test haut".to_owned(),
            altitude: 10_000.0,
            origine: None,
        });
        p.murs[0].haut = modele::Haut::Niveau {
            niveau: niveau.clone(),
            decalage: 3500.0,
        };
        p.murs[1].haut = modele::Haut::Libre { hauteur: 2800.0 };
        let ids = p.murs[..2].iter().map(|m| m.id.as_str()).collect();
        let q = appliquer_champ(&p, &ids, &Modification::ContrainteHaute(json!(niveau))).unwrap();
        assert_eq!(q.murs[0].haut, p.murs[0].haut);
        assert_eq!(
            q.murs[1].haut,
            modele::Haut::Niveau {
                niveau,
                decalage: 0.0
            }
        );
        assert_eq!(q.murs[2].haut, p.murs[2].haut);
        q.verifier().unwrap();

        let libre = appliquer_champ(&q, &ids, &Modification::ContrainteHaute(Value::Null)).unwrap();
        for (avant, apres) in q.murs[..2].iter().zip(&libre.murs[..2]) {
            assert!(matches!(apres.haut, modele::Haut::Libre { .. }));
            assert_eq!(q.bas_haut(avant), libre.bas_haut(apres));
        }
        libre.verifier().unwrap();
    }

    #[test]
    fn ecriture_explicite_sur_plusieurs_categories() {
        let p = projet();
        let ids = BTreeSet::from([p.murs[0].id.as_str(), p.dalles[0].id.as_str()]);
        let q = appliquer_champ(
            &p,
            &ids,
            &Modification::Champ(vec!["nom".into()], json!(&p.murs[0].nom)),
        )
        .unwrap();
        assert_eq!(q.dalles[0].nom, p.murs[0].nom);
        assert_eq!(q.murs[1].nom, p.murs[1].nom);
        assert_eq!(q.dalles[0].contours, p.dalles[0].contours);
    }
}
