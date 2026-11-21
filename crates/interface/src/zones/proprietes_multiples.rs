//! Édition directe d'un champ sur toutes les occurrences compatibles.
use crate::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

mod champs;
mod donnees;

pub(super) use champs::saisie;
use champs::{champ_ui, choix, libelle, options, valeur_ui};
use donnees::{Modification, appliquer_champ, groupes};

fn defaut(champ: &str) -> Value {
    match champ {
        "retournee" | "inversee" | "elevation" | "talus_dessus" => json!(false),
        "coupe_active" => json!(true),
        "orientation_plan" | "decalage_haut" => json!(0.0),
        "sorte" => json!("dalle"),
        "sous_face" => json!("crantee"),
        "arase" | "justification" => json!("axe"),
        "niveau_detail" => json!("moyen"),
        "type_vue" | "etiquette" | "type_trait" => json!(""),
        _ => Value::Null,
    }
}

fn exclus(champ: &str) -> bool {
    matches!(
        champ,
        "id" | "origine"
            | "forme"
            | "genre"
            | "contrainte"
            | "assemblage"
            | "couches"
            | "mur"
            | "escalier"
            | "vue"
            | "element"
            | "aretes"
            | "phase"
            | "sous_projet"
            | "variante"
            | "maillage"
            | "representation"
            | "donnees"
            | "valeurs"
            | "materiaux"
            | "categorie"
            | "raison"
            | "elements_masques"
            | "contour_cadrage"
            | "type_mur"
            | "type_dalle"
            | "type_ouverture"
            | "type_terrain"
            | "type_composant"
            | "type_garde_corps"
            | "type_trait"
            | "traits"
            | "haut"
            | "axe"
            | "contour"
            | "contours"
            | "trace"
            | "pentes"
            | "remblais"
    )
}

fn champs_murs(
    ui: &mut egui::Ui,
    p: &modele::Projet,
    objets: &[Value],
    edit: &mut Option<Modification>,
) {
    let niveaux: Vec<_> = objets.iter().map(|v| v["haut"]["niveau"].clone()).collect();
    let options = std::iter::once((Value::Null, "Libre".into()))
        .chain(p.niveaux.iter().map(|n| (json!(n.id), n.nom.clone())))
        .collect();
    ui.label("Contrainte haute");
    if let Some(v) = ui
        .push_id("contrainte_haute", |ui| choix(ui, &niveaux, &options))
        .inner
    {
        *edit = Some(Modification::ContrainteHaute(v));
    }
    ui.end_row();
    for (cle, label) in [
        ("hauteur", "Hauteur libre"),
        ("decalage", "Décalage du haut"),
    ] {
        // Le champ a un sens commun après le choix du mode de contrainte.
        if objets.iter().all(|v| v["haut"][cle].is_number()) {
            champ_ui(
                ui,
                label,
                &mut vec!["haut".into(), cle.into()],
                objets.iter().map(|v| v["haut"][cle].clone()).collect(),
                Vec::new(),
                edit,
            );
        }
    }
}

fn champs(p: &modele::Projet, groupe: &str, variante: &str, objets: &[Value]) -> BTreeSet<String> {
    let mut cles: BTreeSet<_> = objets
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|o| o.keys().filter(|k| !exclus(k)).cloned())
        .collect();
    let optionnels: &[&str] = match (groupe, variante) {
        ("Éléments horizontaux", _) => &["sorte", "talus_dessus", "type_remblai"],
        ("Poutres", _) => &["type_poutre", "arase", "justification"],
        ("Escaliers", _) => &["niveau_haut", "decalage_haut", "sous_face", "balance"],
        ("Portes" | "Fenêtres" | "Ouvertures libres", _) => &["retournee", "inversee"],
        ("Vues", "plan") => &[
            "niveau_detail",
            "orientation_plan",
            "fond_plan",
            "limite_haute_niveau",
        ],
        ("Vues", "coupe") => &["niveau_detail", "type_vue", "coupe_active", "elevation"],
        _ => &[],
    };
    cles.extend(optionnels.iter().map(|c| (*c).to_owned()));
    if groupe == "Éléments horizontaux" {
        if objets.iter().any(|v| v["sorte"] == "terre_plein") {
            cles.remove("sorte");
        }
        if !objets.iter().all(|v| v["sorte"] == "terre_plein") {
            cles.remove("talus_dessus");
            cles.remove("type_remblai");
        }
    }
    if groupe == "Éléments importés" {
        cles.retain(|c| matches!(c.as_str(), "nom" | "niveau"));
    }
    if matches!(groupe, "Portes" | "Fenêtres" | "Ouvertures libres")
        && objets.iter().any(|v| !v["representation"].is_null())
    {
        cles.remove("largeur");
        cles.remove("hauteur");
    }
    if groupe == "Vues" {
        cles.insert("poche_vue".into());
    }
    if p.types_vues.is_empty() {
        cles.remove("type_vue");
    }
    if groupe == "Annotations" && variante == "polyligne" {
        cles.remove("remplissage");
    }
    if groupe == "Terrains" {
        cles.remove("points");
    }
    if groupe == "Pièces" {
        cles.extend(["etiquette".into()]);
    }
    cles
}

fn champs_aretes(
    ui: &mut egui::Ui,
    p: &modele::Projet,
    ids: &BTreeSet<&str>,
    edit: &mut Option<Modification>,
) {
    let dalles: Vec<_> = p
        .dalles
        .iter()
        .filter(|d| ids.contains(d.id.as_str()))
        .collect();
    if dalles.is_empty()
        || !dalles
            .iter()
            .all(|d| d.sorte.pente_possible() || d.sorte.talus())
    {
        return;
    }
    let contours = dalles
        .iter()
        .map(|d| d.contours.len())
        .min()
        .unwrap_or_default();
    for contour in 0..contours {
        let segments = dalles
            .iter()
            .map(|d| d.contours[contour].len())
            .min()
            .unwrap_or_default();
        for segment in 0..segments {
            let valeurs: Vec<_> = dalles
                .iter()
                .map(|d| json!(d.pente(contour, segment).unwrap_or(0.0)))
                .collect();
            ui.label(format!("Pente {}.{} (°)", contour + 1, segment + 1));
            if let Some(v) = ui
                .push_id(("pente", contour, segment), |ui| {
                    valeur_ui(ui, &valeurs, Vec::new(), false)
                })
                .inner
                && let Some(v) = v.as_f64()
            {
                *edit = Some(Modification::Arete {
                    contour,
                    segment,
                    pente: Some((v != 0.0).then_some(v)),
                    remblai: None,
                });
            }
            ui.end_row();
            if dalles
                .iter()
                .all(|d| d.sorte.talus() && d.pente(contour, segment).is_some())
            {
                let valeurs: Vec<_> = dalles
                    .iter()
                    .map(|d| json!(d.remblaye(contour, segment)))
                    .collect();
                ui.label(format!("Remblayer {}.{}", contour + 1, segment + 1));
                if let Some(v) = ui
                    .push_id(("remblai", contour, segment), |ui| {
                        valeur_ui(ui, &valeurs, Vec::new(), false)
                    })
                    .inner
                {
                    *edit = Some(Modification::Arete {
                        contour,
                        segment,
                        pente: None,
                        remblai: v.as_bool(),
                    });
                }
                ui.end_row();
            }
        }
    }
}

fn champs_occurrence(
    ui: &mut egui::Ui,
    p: &modele::Projet,
    objets: &[Value],
    edit: &mut Option<Modification>,
) {
    use modele::famille::modelee::{GenreParametre, Portee};

    let mut parametres = std::collections::BTreeMap::<_, Vec<_>>::new();
    for o in objets
        .iter()
        .filter_map(|v| p.ouvertures.iter().find(|o| v["id"] == o.id))
    {
        let Some(f) = p
            .types_ouvertures
            .iter()
            .find(|t| Some(&t.id) == o.type_ouverture.as_ref())
            .and_then(|t| t.modele.as_ref())
        else {
            continue;
        };
        let epaisseur = modele::volumes::solides::hote(p, &o.mur).map_or(200.0, |h| h.1);
        let resolution = std::rc::Rc::new(f.resoudre_occurrence(
            o.largeur,
            o.hauteur,
            epaisseur,
            &p.surcharges(o),
            &o.materiaux,
        ));
        for param in f
            .parametres
            .iter()
            .filter(|par| par.portee == Portee::Occurrence)
        {
            parametres
                .entry((&param.nom, param.genre as u8))
                .or_default()
                .push((param.genre, o, resolution.clone()));
        }
    }
    for ((nom, genre_id), cibles) in parametres {
        let genre = cibles[0].0;
        let resolutions = || cibles.iter().map(|(_, _, r)| r);
        let (table, valeurs, options) = match genre {
            GenreParametre::Texte => continue,
            GenreParametre::Materiau => (
                "materiaux",
                resolutions()
                    .map(|r| json!(r.materiaux.get(nom).map_or("", String::as_str)))
                    .collect::<Vec<_>>(),
                std::iter::once((json!(""), "Couleur du rôle".into()))
                    .chain(p.materiaux.iter().map(|m| (json!(m.nom), m.nom.clone())))
                    .collect(),
            ),
            GenreParametre::Longueur | GenreParametre::OuiNon => (
                "valeurs",
                resolutions()
                    .map(|r| {
                        json!(r.valeurs.get(nom).copied().unwrap_or(
                            if genre == GenreParametre::OuiNon {
                                1.0
                            } else {
                                0.0
                            }
                        ))
                    })
                    .collect(),
                if genre == GenreParametre::OuiNon {
                    vec![(json!(1.0), "Oui".into()), (json!(0.0), "Non".into())]
                } else {
                    Vec::new()
                },
            ),
        };
        ui.label(if cibles.len() == objets.len() {
            nom.clone()
        } else {
            format!("{nom} ({})", cibles.len())
        });
        ui.push_id(("occurrence", nom, genre_id), |ui| {
            ui.horizontal(|ui| {
                let mut valeur =
                    valeur_ui(ui, &valeurs, options, genre == GenreParametre::Longueur);
                if cibles.iter().any(|(_, o, _)| {
                    if table == "valeurs" {
                        o.valeurs.contains_key(nom)
                    } else {
                        o.materiaux.contains_key(nom)
                    }
                }) && ui.small_button("Défaut").clicked()
                {
                    valeur = Some(Value::Null);
                }
                if let Some(valeur) = valeur {
                    *edit = Some(Modification::ParametreOccurrence {
                        cibles: cibles.iter().map(|(_, o, _)| o.id.clone()).collect(),
                        table: table.into(),
                        nom: nom.clone(),
                        valeur,
                    });
                }
            });
        });
        ui.end_row();
    }
}

impl Application {
    pub(crate) fn proprietes_multiples(
        &mut self,
        ui: &mut egui::Ui,
        ids: &[String],
    ) -> Result<(), String> {
        let Some((p, _)) = &self.natif else {
            return Ok(());
        };
        let groupes = groupes(p, ids)?;
        let mut projet_modifie = None;
        let mut communs = Vec::new();
        if groupes.len() > 1 {
            let tous: Vec<_> = groupes.values().flatten().cloned().collect();
            let cibles: BTreeSet<_> = tous.iter().filter_map(|v| v["id"].as_str()).collect();
            let mut edit = None;
            // Ces champs gardent le même sens d'une catégorie à l'autre.
            for cle in ["nom", "niveau"] {
                if (cle != "nom" || !groupes.keys().any(|(c, _)| *c == "Niveaux"))
                    && tous.iter().all(|v| v[cle].is_string())
                {
                    communs.push(cle);
                }
            }
            if !communs.is_empty() {
                ui.heading("Paramètres communs");
                egui::Grid::new("champs_communs")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for cle in &communs {
                            champ_ui(
                                ui,
                                &libelle(cle),
                                &mut vec![(*cle).into()],
                                tous.iter().map(|v| v[*cle].clone()).collect(),
                                options(p, cle, &tous),
                                &mut edit,
                            );
                        }
                    });
            }
            if let Some(edit) = edit {
                projet_modifie = Some(appliquer_champ(p, &cibles, &edit)?);
            }
        }
        for ((groupe, variante), objets) in &groupes {
            let cibles: BTreeSet<_> = objets.iter().filter_map(|v| v["id"].as_str()).collect();
            let mut edit = None;
            let titre = match (*groupe, variante.as_str()) {
                ("Annotations" | "Vues", v) if !v.is_empty() => {
                    format!("{groupe} · {} ({})", libelle(v), objets.len())
                }
                _ => format!("{groupe} ({})", objets.len()),
            };
            ui.push_id((groupe, variante, &cibles), |ui| {
                egui::CollapsingHeader::new(titre)
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("champs")
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                if *groupe == "Murs" {
                                    champs_murs(ui, p, objets, &mut edit);
                                }
                                if *groupe == "Éléments horizontaux" {
                                    champs_aretes(ui, p, &cibles, &mut edit);
                                }
                                if matches!(*groupe, "Portes" | "Fenêtres" | "Ouvertures libres") {
                                    champs_occurrence(ui, p, objets, &mut edit);
                                }
                                for cle in champs(p, groupe, variante, objets) {
                                    let valeurs = objets
                                        .iter()
                                        .map(|v| {
                                            v.get(&cle).cloned().unwrap_or_else(|| defaut(&cle))
                                        })
                                        .collect();
                                    let nom = if cle == "decalage"
                                        && objets.iter().all(|v| v["sorte"] == "toiture")
                                    {
                                        "Décalage à l’égout".into()
                                    } else {
                                        libelle(&cle)
                                    };
                                    champ_ui(
                                        ui,
                                        &nom,
                                        &mut vec![cle.clone()],
                                        valeurs,
                                        options(p, &cle, objets),
                                        &mut edit,
                                    );
                                }
                            });
                    });
            });
            if let Some(edit) = edit {
                let original = projet_modifie.as_ref().unwrap_or(p);
                projet_modifie = Some(appliquer_champ(original, &cibles, &edit)?);
            }
        }
        if let Some(projet) = projet_modifie {
            self.appliquer_projet(projet)?;
        }
        Ok(())
    }
}
