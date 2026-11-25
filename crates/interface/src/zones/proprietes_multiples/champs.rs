use super::donnees::Modification;
use crate::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn libelle(champ: &str) -> String {
    match champ {
        "niveau" => "Niveau de base".into(),
        "niveau_haut" => "Contrainte haute".into(),
        "type_barre" => "Type".into(),
        "type_remblai" => "Type de remblai".into(),
        "decalage" => "Décalage".into(),
        "decalage_bas" => "Décalage de la base".into(),
        "decalage_haut" => "Décalage du haut".into(),
        "allege" => "Allège".into(),
        "echelle" => "Échelle (1:n)".into(),
        "paillasse" => "Paillasse".into(),
        "numero" => "Numéro".into(),
        "epaisseur" => "Épaisseur".into(),
        _ => {
            let mut lettres = champ.chars();
            lettres.next().map_or_else(String::new, |c| {
                format!("{}{}", c.to_uppercase(), lettres.as_str().replace('_', " "))
            })
        }
    }
}

pub(super) type Choix = Vec<(Value, String)>;

fn options_enum<T: serde::Serialize>(
    items: impl IntoIterator<Item = T>,
    nom: impl Fn(T) -> &'static str,
) -> Choix {
    items
        .into_iter()
        .map(|v| (json!(&v), nom(v).to_owned()))
        .collect()
}

pub(super) fn options(p: &modele::Projet, champ: &str, objets: &[Value]) -> Choix {
    use modele::elements::{
        escalier::{Remplissage, SousFace},
        poutre::{Arase, Justification},
    };
    match champ {
        "niveau" | "bas_niveau" | "niveau_haut" | "limite_haute_niveau" => {
            let mut choix = Vec::new();
            if matches!(champ, "niveau_haut" | "limite_haute_niveau")
                || objets.iter().any(|v| v.get(champ) == Some(&Value::Null))
            {
                choix.push((
                    Value::Null,
                    if champ == "niveau_haut" {
                        "Libre"
                    } else {
                        "Aucun"
                    }
                    .into(),
                ));
            }
            choix.extend(p.niveaux.iter().map(|n| (json!(n.id), n.nom.clone())));
            choix
        }
        "sorte" if !objets.iter().any(|v| v[champ] == "terre_plein") => options_enum(
            modele::SorteDalle::TOUTES
                .into_iter()
                .filter(|s| !s.talus()),
            modele::SorteDalle::nom,
        ),
        "sous_face" => options_enum(SousFace::CHOIX, SousFace::nom),
        "remplissage" if objets.iter().any(|v| v[champ].is_string()) => {
            options_enum(Remplissage::TOUS, Remplissage::nom)
        }
        "arase" => options_enum(Arase::TOUTES, Arase::libelle),
        "justification" => options_enum(Justification::TOUTES, Justification::libelle),
        "niveau_detail" => {
            use modele::famille::modelee::NiveauDetail;
            options_enum(NiveauDetail::TOUS, NiveauDetail::libelle)
        }
        "type_barre" => {
            let poteaux = objets.iter().any(|v| v["genre"] == "poteau");
            let types = if poteaux { &p.types_poteaux } else { &p.types_poutres };
            types.iter().map(|t| (json!(t.id), t.nom.clone())).collect()
        }
        "type_remblai" => std::iter::once((Value::Null, "Par défaut".into()))
            .chain(
                p.types_terrains_disponibles()
                    .into_iter()
                    .map(|t| (json!(t.id), t.nom)),
            )
            .collect(),
        "etiquette" => std::iter::once((json!(""), "Standard".into()))
            .chain(
                p.familles_etiquettes
                    .iter()
                    .map(|t| (json!(t.id), t.nom.clone())),
            )
            .collect(),
        "type_vue" => std::iter::once((json!(""), "Aucun".into()))
            .chain(p.types_vues.iter().map(|t| (json!(t.nom), t.nom.clone())))
            .collect(),
        "poche_vue" => {
            use moteur2d::Poche;
            std::iter::once((Value::Null, "Par défaut".into()))
                .chain(
                    [Poche::Hachure, Poche::Plein, Poche::Vide, Poche::Materiau]
                        .into_iter()
                        .map(|p| (json!(p.nom()), p.libelle().into())),
                )
                .collect()
        }
        "balance" => std::iter::once((Value::Null, "Paliers".into()))
            .chain((1..=12).map(|n| (json!(n), n.to_string())))
            .collect(),
        "fond_plan" => std::iter::once((Value::Null, "Aucun".into()))
            .chain(
                p.vues
                    .iter()
                    .filter(|v| matches!(v.genre, modele::GenreVue::Plan { .. }))
                    .map(|v| (json!(v.id), v.nom.clone())),
            )
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn choix(ui: &mut egui::Ui, valeurs: &[Value], options: &Choix) -> Option<Value> {
    let (actuel, varie) = crate::zones::communes::melange(valeurs.iter().cloned());
    let texte = if varie {
        "Varié"
    } else {
        options
            .iter()
            .find(|(v, _)| Some(v) == actuel.as_ref())
            .map_or("—", |(_, nom)| nom)
    };
    let mut nouveau = None;
    egui::ComboBox::from_id_salt("choix")
        .selected_text(texte)
        .show_ui(ui, |ui| {
            for (v, nom) in options {
                let actif = !varie && Some(v) == actuel.as_ref();
                if ui.selectable_label(actif, nom).clicked() && !actif {
                    nouveau = Some(v.clone());
                }
            }
        });
    nouveau
}

pub(in crate::zones) fn saisie(
    ui: &mut egui::Ui,
    valeurs: &[Value],
    numerique: bool,
    dimension: bool,
) -> Option<Value> {
    let (actuel, varie) = crate::zones::communes::melange(valeurs.iter().cloned());
    let unite = modele::unite::unite();
    let id = ui.scope_id().with("saisie");
    let mut texte = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| {
            if varie {
                return String::new();
            }
            match actuel {
                Some(Value::String(s)) => s,
                Some(Value::Number(n)) if dimension => {
                    unite.afficher(n.as_f64().unwrap_or_default())
                }
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            }
        });
    let reponse = ui.add(
        egui::TextEdit::singleline(&mut texte)
            .id(id)
            .desired_width(120.0)
            .hint_text(if varie { "Varié" } else { "—" }),
    );
    if reponse.changed() {
        ui.data_mut(|d| d.insert_temp(id, texte.clone()));
    }
    if !reponse.lost_focus() {
        return None;
    }
    ui.data_mut(|d| {
        let texte = d.get_temp::<String>(id);
        d.remove::<String>(id);
        texte
    })
    .and_then(|texte| {
        if !numerique {
            return Some(json!(texte));
        }
        let t = texte.trim();
        if t.is_empty() && valeurs.contains(&Value::Null) {
            return Some(Value::Null);
        }
        if !dimension && valeurs.iter().any(Value::is_u64) {
            return t.parse::<u64>().ok().map(|n| json!(n));
        }
        let (t, facteur) = if dimension {
            [("mm", 1.0), ("cm", 10.0), ("m", 1000.0)]
                .into_iter()
                .find_map(|(suffixe, facteur)| {
                    t.strip_suffix(suffixe).map(|s| (s.trim_end(), facteur))
                })
                .unwrap_or((t, unite.facteur()))
        } else {
            (t, 1.0)
        };
        t.replace(',', ".")
            .parse::<f64>()
            .ok()
            .map(|n| n * facteur)
            .filter(|n| n.is_finite())
            .map(|n| json!(n))
    })
}

pub(super) fn valeur_ui(
    ui: &mut egui::Ui,
    valeurs: &[Value],
    options: Choix,
    dimension: bool,
) -> Option<Value> {
    let exemple = valeurs.iter().find(|v| !v.is_null());
    if !options.is_empty() {
        choix(ui, valeurs, &options)
    } else if exemple.is_some_and(Value::is_boolean) {
        choix(
            ui,
            valeurs,
            &vec![(json!(true), "Oui".into()), (json!(false), "Non".into())],
        )
    } else {
        saisie(
            ui,
            valeurs,
            exemple.is_some_and(Value::is_number),
            dimension,
        )
    }
    .filter(|v| valeurs.iter().any(|x| x != v))
}

pub(super) fn champ_ui(
    ui: &mut egui::Ui,
    label: &str,
    chemin: &mut Vec<String>,
    valeurs: Vec<Value>,
    options: Choix,
    edit: &mut Option<Modification>,
) {
    if valeurs
        .iter()
        .all(|v| v.is_null() || v.is_boolean() || v.is_string() || v.is_number())
    {
        if valeurs.iter().all(Value::is_null) && options.is_empty() {
            return;
        }
        let dimension = chemin.first().is_some_and(|cle| {
            matches!(
                cle.as_str(),
                "haut"
                    | "altitude"
                    | "largeur"
                    | "hauteur"
                    | "hauteur_marche"
                    | "paillasse"
                    | "profondeur"
                    | "decalage"
                    | "decalage_bas"
                    | "decalage_haut"
                    | "abscisse"
                    | "allege"
                    | "rayon"
                    | "position"
                    | "point"
                    | "points"
                    | "repere"
                    | "centre"
                    | "debut"
                    | "fin"
                    | "coupe"
                    | "bas_decalage"
                    | "limite_haute_decalage"
            )
        });
        ui.label(label);
        if let Some(v) = ui
            .push_id(&*chemin, |ui| valeur_ui(ui, &valeurs, options, dimension))
            .inner
        {
            *edit = Some(Modification::Champ(chemin.clone(), v));
        }
        ui.end_row();
    } else if valeurs.iter().all(Value::is_array) {
        let n = valeurs
            .iter()
            .filter_map(Value::as_array)
            .map(Vec::len)
            .min()
            .unwrap_or(0);
        for i in 0..n {
            chemin.push(i.to_string());
            champ_ui(
                ui,
                &format!("{label} · {}", i + 1),
                chemin,
                valeurs.iter().map(|v| v[i].clone()).collect(),
                Vec::new(),
                edit,
            );
            chemin.pop();
        }
    } else if valeurs.iter().all(Value::is_object) {
        let cles: BTreeSet<_> = valeurs
            .iter()
            .filter_map(Value::as_object)
            .flat_map(|o| o.keys())
            .collect();
        for cle in cles
            .into_iter()
            .filter(|cle| !super::exclus(cle) && cle.as_str() != "ref")
        {
            chemin.push(cle.clone());
            champ_ui(
                ui,
                &format!("{label} · {}", libelle(cle)),
                chemin,
                valeurs.iter().map(|v| v[cle].clone()).collect(),
                Vec::new(),
                edit,
            );
            chemin.pop();
        }
    }
}
