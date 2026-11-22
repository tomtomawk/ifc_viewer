//! Présentation des propositions retenues. Le protocole conserve ses détails d’origine.
use ::collaboration::{Conflit, session::Copie};
use commandes::objets::Objets;
use eframe::egui;
use serde_json::Value;
use std::collections::BTreeSet;

const EFFET: &str = "Transaction entière retenue après refus de précondition";

pub(super) enum Choix {
    Partage(String),
    Local(String),
}

fn categorie(cle: &str) -> Option<&'static str> {
    Some(match cle {
        "murs" => "Mur",
        "assemblages" => "Mur composé",
        "dalles" => "Dalle, plafond ou toiture",
        "terrains" => "Terrain",
        "niveaux" => "Niveau",
        "barres" => "Poteau ou poutre",
        "composants" => "Composant",
        "ouvertures" => "Ouverture",
        "escaliers" => "Escalier",
        "garde_corps" => "Garde-corps",
        "quadrillages" => "Quadrillage",
        "pieces" => "Pièce",
        "vues" => "Vue",
        "annotations" => "Annotation",
        "feuilles" => "Feuille",
        "nomenclatures" => "Nomenclature",
        "materiaux" => "Matériau",
        "motifs" => "Motif",
        "types_murs" => "Type de mur",
        "types_dalles" => "Type de dalle",
        "types_composants" => "Type de composant",
        "types_ouvertures" => "Type d’ouverture",
        "types_terrains" => "Type de terrain",
        "types_vues" => "Type de vue",
        "types_traits" => "Type de trait",
        "profils" => "Profil",
        "phases" => "Phases du projet",
        "sous_projets" => "Sous-projets",
        "attaches_toitures" => "Attache à une toiture",
        "contextes" => "Contexte de l’élément",
        "verrous" => "Verrou de l’élément",
        _ => return None,
    })
}

fn nom(v: &Value) -> Option<&str> {
    v.get("nom")
        .or_else(|| v.get("texte"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

fn objet(cle: &str, valeurs: &[Option<&Value>], objets: &Objets) -> String {
    let famille = cle
        .strip_prefix("@ordre/")
        .or_else(|| cle.strip_prefix("@/"))
        .unwrap_or(cle);
    let (famille, id) = famille.split_once('/').unwrap_or((famille, ""));
    let categorie = categorie(famille).unwrap_or("Donnée du projet");
    let nom = valeurs
        .iter()
        .flatten()
        .find_map(|v| nom(v))
        .or_else(|| objets.get(cle).and_then(nom))
        .or_else(|| {
            (!id.is_empty())
                .then(|| {
                    objets
                        .values()
                        .find(|v| v["id"].as_str() == Some(id))
                        .and_then(nom)
                })
                .flatten()
        });
    nom.map_or_else(
        || categorie.into(),
        |n| {
            let court: String = n.chars().take(80).collect();
            format!("{categorie} « {court} »")
        },
    )
}

fn expliquer(c: &Conflit, objets: &Objets) -> Option<String> {
    let cle = c.cle.as_str();
    let texte = if c.raison == EFFET {
        return None;
    } else if c.raison.starts_with("Réservation requise")
        || c.raison.starts_with("Réservation périmée")
    {
        "La réservation nécessaire pour modifier un élément a expiré ou n’est plus à vous.".into()
    } else if cle == "#spatial" || cle.starts_with("#voisinage3/") || cle.starts_with("#zones/") {
        "Un élément dans la même zone en plan, ou une donnée liée, a changé dans le projet partagé. BIMedit doit revérifier votre modification.".into()
    } else if cle.starts_with("#types-terrains-") || cle.starts_with("#usagers-terrain-") {
        "Le type de terrain par défaut ou les terrains qui l’utilisent ont changé dans le projet partagé.".into()
    } else if let Some(cle) = cle.strip_prefix("#usages/") {
        let cible = objet(cle, &[], objets);
        format!("{cible} : un élément lié a changé dans le projet partagé.")
    } else if c.raison == "Propositions concurrentes incompatibles" {
        let cible = objet(
            cle,
            &[c.locale.as_ref(), c.acceptee.as_ref(), c.base.as_ref()],
            objets,
        );
        if c.acceptee.is_none() && c.base.is_some() {
            format!(
                "{cible} · Cet élément a été retiré du projet partagé depuis votre modification."
            )
        } else if c.locale.is_none() && c.base.is_some() {
            format!(
                "{cible} a changé dans le projet partagé alors que votre proposition le supprime."
            )
        } else {
            format!("{cible} : votre modification et la version partagée nécessitent un choix.")
        }
    } else if cle == "protocole" {
        "Cette proposition est liée à une version ou à un état incompatible du projet.".into()
    } else if cle == "validation" {
        "Cette proposition ne peut pas être appliquée au projet partagé dans son état actuel."
            .into()
    } else if c.raison == "Dépendance ou collection de relations modifiée" {
        let cible = objet(cle, &[], objets);
        format!("{cible} : une donnée utilisée par votre modification a changé.")
    } else {
        "BIMedit n’a pas pu publier cette proposition. Le motif précis figure dans les détails techniques.".into()
    };
    Some(texte)
}

fn valeur(v: Option<&Value>, longueur: bool, champ: &str, objets: &Objets) -> Option<String> {
    let Some(v) = v.filter(|v| !v.is_null()) else {
        return Some("Non renseigné".into());
    };
    if longueur {
        return v.as_f64().map(|n| modele::unite::unite().afficher(n));
    }
    match v {
        Value::Bool(v) => Some(if *v { "Oui" } else { "Non" }.into()),
        Value::String(s) => {
            if champ == "niveau" {
                return Some(
                    objets
                        .get(&format!("niveaux/{s}"))
                        .and_then(nom)
                        .unwrap_or("Niveau introuvable")
                        .into(),
                );
            }
            Some(s.chars().take(100).collect())
        }
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn champs(local: &Value, partage: &Value, objets: &Objets) -> Vec<(&'static str, String, String)> {
    let mut lignes = Vec::new();
    for (cle, libelle, longueur) in [
        ("nom", "Nom", false),
        ("numero", "Numéro", false),
        ("texte", "Texte", false),
        ("niveau", "Niveau", false),
        ("phase", "Phase", false),
        ("sous_projet", "Sous-projet", false),
        ("altitude", "Altitude", true),
        ("largeur", "Largeur", true),
        ("hauteur", "Hauteur", true),
        ("hauteur_marche", "Hauteur de marche", true),
        ("profondeur", "Profondeur", true),
        ("allege", "Allège", true),
        ("decalage_bas", "Décalage de la base", true),
        ("decalage", "Décalage", true),
        ("echelle", "Échelle", false),
    ] {
        let (a, b) = (local.get(cle), partage.get(cle));
        if a != b
            && let (Some(a), Some(b)) = (
                valeur(a, longueur, cle, objets),
                valeur(b, longueur, cle, objets),
            )
        {
            lignes.push((libelle, a, b));
        }
    }
    if local.get("haut") != partage.get("haut") {
        let haut = |v: &Value| {
            let Some(h) = v.get("haut") else {
                return "Non renseignée".into();
            };
            match h["contrainte"].as_str() {
                Some("libre") => h["hauteur"]
                    .as_f64()
                    .map(|n| format!("Libre, {}", modele::unite::unite().afficher(n)))
                    .unwrap_or_else(|| "Libre".into()),
                Some("niveau") => {
                    let niveau =
                        valeur(h.get("niveau"), false, "niveau", objets).unwrap_or_default();
                    let decalage = h["decalage"].as_f64().unwrap_or_default();
                    format!(
                        "{niveau}, décalage {}",
                        modele::unite::unite().afficher(decalage)
                    )
                }
                _ => "Non renseignée".into(),
            }
        };
        lignes.push(("Contrainte haute", haut(local), haut(partage)));
    }
    for (cle, libelle) in [
        ("axe", "Tracé de l’axe"),
        ("contours", "Contour"),
        ("pentes", "Pentes"),
        ("position", "Position"),
    ] {
        if local.get(cle) != partage.get(cle) {
            lignes.push((libelle, String::new(), String::new()));
        }
    }
    lignes
}

pub(super) fn fenetre(ctx: &egui::Context, copie: &Copie, libre: bool) -> Option<Choix> {
    if copie.conflits.is_empty() {
        return None;
    }
    let mut choix = None;
    let objets = &copie.base.objets;
    egui::Window::new("Modifications à vérifier")
        .id(egui::Id::unique("conflits_collaboration"))
        .default_width(620.0)
        .default_pos(ctx.content_rect().center() - egui::vec2(310.0, 165.0))
        .show(ctx, |ui| {
            let n = copie.conflits.len();
            ui.label(if n == 1 {
                "Une proposition n’a pas été publiée.".into()
            } else {
                format!("{n} propositions n’ont pas été publiées.")
            });
            if copie.apercu_conflit.is_some() {
                ui.label("Vos modifications sont conservées dans votre copie locale et restent visibles ici.");
            }
            ui.label("Le choix s’applique à toute la proposition.");
            egui::ScrollArea::vertical().max_height(570.0).show(ui, |ui| {
                for (i, (id, conflits)) in copie.conflits.iter().enumerate() {
                    ui.push_id(id, |ui| {
                        let proposition = copie.attente.iter().find(|t| &t.id == id);
                        let changements = proposition.map_or(&[][..], |t| t.changements.as_slice());
                        let titre = match changements {
                            [c] => objet(&c.cle, &[c.apres.as_ref(), c.avant.as_ref()], objets),
                            c if c.len() > 1 => format!("Proposition {} · {} modifications", i + 1, c.len()),
                            _ => format!("Proposition {}", i + 1),
                        };
                        ui.separator();
                        ui.heading(titre);
                        let causes: BTreeSet<_> = conflits.iter().filter_map(|c| expliquer(c, objets)).collect();
                        for cause in causes { ui.label(cause); }
                        egui::CollapsingHeader::new("Modifications de la proposition")
                            .default_open(n == 1 && changements.len() <= 4)
                            .show(ui, |ui| {
                            for c in changements {
                                let partage = ::collaboration::concurrence::valeur(objets, &c.cle);
                                let titre = objet(&c.cle, &[c.apres.as_ref(), partage, c.avant.as_ref()], objets);
                                let action = match (&c.avant, &c.apres) { (None, Some(_)) => "Ajouter", (_, None) => "Supprimer", _ => "Modifier" };
                                ui.strong(format!("{action} · {titre}"));
                                if let (Some(a), Some(b)) = (c.apres.as_ref(), partage) {
                                    for (champ, local, partage) in champs(a, b, objets) {
                                        if local.is_empty() && partage.is_empty() {
                                            ui.label(format!("{champ} : les deux versions diffèrent."));
                                        } else {
                                            ui.label(format!("{champ} · Votre proposition : {local} · Projet partagé : {partage}"));
                                        }
                                    }
                                } else if partage.is_none() && c.avant.is_some() {
                                    ui.label("Cet élément n’est plus dans le projet partagé.");
                                }
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            if ui.add_enabled(libre, egui::Button::new("Garder le projet partagé")).clicked() {
                                choix = Some(Choix::Partage(id.clone()));
                            }
                            if ui.add_enabled(libre && proposition.is_some(), egui::Button::new("Soumettre ma proposition à nouveau")).clicked() {
                                choix = Some(Choix::Local(id.clone()));
                            }
                        });
                        ui.label("Le premier choix retire votre proposition de la synchronisation et l’archive. Le second la remet en attente de validation.");
                        ui.collapsing("Détails techniques", |ui| {
                            ui.monospace(id);
                            ui.monospace(serde_json::to_string_pretty(conflits).unwrap_or_default());
                            if let Some(t) = proposition {
                                ui.collapsing("Transaction complète", |ui| {
                                    ui.monospace(serde_json::to_string_pretty(t).unwrap_or_default());
                                });
                            }
                        });
                    });
                }
            });
        });
    choix
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn raison_spatiale_sans_identifiant_et_effet_non_compte_comme_cause() {
        let mut conflit = Conflit {
            cle: "#voisinage3/[coordonnées]".into(),
            base: None,
            locale: None,
            acceptee: None,
            raison: "Dépendance ou collection de relations modifiée".into(),
        };
        let motif = expliquer(&conflit, &Objets::new()).unwrap();
        assert!(motif.contains("même zone en plan"));
        assert!(!motif.contains("#voisinage"));
        conflit.cle = "murs/m-1".into();
        conflit.raison = EFFET.into();
        assert_eq!(expliquer(&conflit, &Objets::new()), None);
    }

    #[test]
    fn conflit_identifie_le_mur_et_compare_les_hauteurs() {
        let conflit = Conflit {
            cle: "murs/m-1".into(),
            base: None,
            locale: Some(
                json!({"nom": "Façade nord", "haut": {"contrainte": "libre", "hauteur": 3000.0}}),
            ),
            acceptee: Some(
                json!({"nom": "Façade nord", "haut": {"contrainte": "libre", "hauteur": 3200.0}}),
            ),
            raison: "Propositions concurrentes incompatibles".into(),
        };
        let objets = Objets::new();
        let motif = expliquer(&conflit, &objets).unwrap();
        assert!(motif.contains("Mur « Façade nord »"));
        assert!(!motif.contains("m-1"));
        let l = champs(
            conflit.locale.as_ref().unwrap(),
            conflit.acceptee.as_ref().unwrap(),
            &objets,
        );
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].0, "Contrainte haute");
        assert_ne!(l[0].1, l[0].2);
    }
}
