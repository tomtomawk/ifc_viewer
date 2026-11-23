//! Présentation des propositions retenues. Le protocole conserve ses détails d’origine.
use ::collaboration::{Changement, Conflit, Transaction, session::Copie};
use commandes::objets::Objets;
use eframe::egui;
use serde_json::Value;
use std::collections::BTreeSet;

const EFFET: &str = "Transaction entière retenue après refus de précondition";

pub(super) enum Choix {
    Central(String),
    Local(String),
}

pub(super) fn boutons(ui: &mut egui::Ui, id: &str, libre: bool, local: bool) -> Option<Choix> {
    let central = ui
        .add_enabled(libre, egui::Button::new("Accepter le central"))
        .on_hover_text("Garde la version partagée et archive vos modifications pour ce conflit.")
        .clicked();
    let local = ui
        .add_enabled(libre && local, egui::Button::new("Conserver le local"))
        .on_hover_text("Relance la synchronisation pour publier vos modifications locales.")
        .clicked();
    if central {
        Some(Choix::Central(id.into()))
    } else {
        local.then(|| Choix::Local(id.into()))
    }
}

/// Ne retient que les identités encore présentes dans le projet affiché.
pub(super) fn elements_concernes(copie: &Copie, p: &modele::Projet) -> Vec<String> {
    let presents: BTreeSet<&str> = crate::socle::document_bim::elements_natifs(p)
        .map(|(_, id)| id.as_str())
        .chain(p.annotations.iter().map(|a| a.id.as_str()))
        .chain(p.pieces.iter().map(|a| a.id.as_str()))
        .chain(p.quadrillages.iter().map(|a| a.id.as_str()))
        .chain(p.vues.iter().map(|a| a.id.as_str()))
        .collect();
    let retenir = |ids: &mut BTreeSet<String>, cle: &str, valeurs: &[Option<&Value>]| {
        let cle = cle.strip_prefix("#usages/").unwrap_or(cle);
        if !cle.starts_with('#')
            && !cle.starts_with("@ordre/")
            && let Some((_, id)) = cle.rsplit_once('/')
            && presents.contains(id)
        {
            ids.insert(id.to_owned());
        }
        for v in valeurs.iter().flatten() {
            if let Some(id) = v["id"].as_str().filter(|id| presents.contains(id)) {
                ids.insert(id.to_owned());
            }
        }
    };
    let mut ids = BTreeSet::new();
    for (id, conflits) in &copie.conflits {
        let mut cibles = BTreeSet::new();
        for c in conflits.iter().filter(|c| c.raison != EFFET) {
            retenir(
                &mut cibles,
                &c.cle,
                &[c.locale.as_ref(), c.acceptee.as_ref(), c.base.as_ref()],
            );
        }
        // Un refus de zone ne donne pas d’identifiant. Montrer alors la proposition
        // locale plutôt que de sélectionner tous les éléments de la zone.
        if cibles.is_empty()
            && let Some(t) = copie.attente.iter().find(|t| &t.id == id)
        {
            for c in &t.changements {
                retenir(&mut cibles, &c.cle, &[c.avant.as_ref(), c.apres.as_ref()]);
            }
        }
        ids.extend(cibles);
    }
    ids.into_iter().collect()
}

fn categorie(cle: &str) -> Option<&'static str> {
    Some(match cle {
        "murs" => "Mur",
        "assemblages" => "Mur composé",
        "dalles" => "Dalle",
        "terrains" => "Terrain",
        "limites_propriete" => "Limite de propriété",
        "niveaux" => "Niveau",
        "barres" => "Poteau ou poutre",
        "composants" => "Composant",
        "ouvertures" => "Ouverture",
        "escaliers" => "Escalier",
        "garde_corps" => "Garde-corps",
        "importes" => "Élément importé",
        "liens" => "Lien",
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

fn cle_objet(cle: &str) -> (&str, &str) {
    let cle = cle
        .strip_prefix("@ordre/")
        .or_else(|| cle.strip_prefix("@/"))
        .unwrap_or(cle);
    cle.split_once('/').unwrap_or((cle, ""))
}

fn objet(cle: &str, valeurs: &[Option<&Value>], objets: &Objets) -> String {
    let (famille, id) = cle_objet(cle);
    let courant = ::collaboration::concurrence::valeur(objets, cle);
    let genre = |champ: &str| {
        valeurs
            .iter()
            .copied()
            .chain([courant])
            .flatten()
            .find_map(|v| v.get(champ).and_then(Value::as_str))
    };
    let categorie = match (famille, genre("genre"), genre("sorte")) {
        ("ouvertures", Some("porte"), _) => "Porte",
        ("ouvertures", Some("fenetre"), _) => "Fenêtre",
        ("barres", Some("poteau"), _) => "Poteau",
        ("barres", Some("poutre"), _) => "Poutre",
        ("dalles", _, Some("toiture")) => "Toiture",
        ("dalles", _, Some("plafond")) => "Plafond",
        ("dalles", _, Some("sol")) => "Sol",
        ("dalles", _, Some("terre_plein")) => "Terre-plein",
        _ => categorie(famille).unwrap_or("Donnée du projet"),
    };
    let nom = valeurs
        .iter()
        .flatten()
        .find_map(|v| nom(v))
        .or_else(|| courant.and_then(nom))
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

fn titre_changement(c: &Changement, objets: &Objets) -> String {
    objet(&c.cle, &[c.apres.as_ref(), c.avant.as_ref()], objets)
}

fn changements_visibles(t: Option<&Transaction>) -> impl Iterator<Item = &Changement> {
    t.into_iter().flat_map(|t| &t.changements).filter(|c| {
        let (famille, id) = cle_objet(&c.cle);
        !c.cle.starts_with("@ordre/") && (!id.is_empty() || categorie(famille).is_some())
    })
}

pub(super) fn titre(t: Option<&Transaction>, conflits: &[Conflit], objets: &Objets) -> String {
    let mut changements = changements_visibles(t);
    if let Some(c) = changements.next() {
        let premier = titre_changement(c, objets);
        return match changements.count() {
            0 => premier,
            1 => format!("{premier} et 1 autre élément"),
            n => format!("{premier} et {n} autres éléments"),
        };
    }
    if let Some(c) = t
        .into_iter()
        .flat_map(|t| &t.changements)
        .find(|c| categorie(cle_objet(&c.cle).0).is_some())
    {
        return titre_changement(c, objets);
    }
    conflits
        .iter()
        .find(|c| c.cle.contains('/') && !c.cle.starts_with('#'))
        .map_or_else(
            || "Projet partagé".into(),
            |c| {
                objet(
                    &c.cle,
                    &[c.locale.as_ref(), c.acceptee.as_ref(), c.base.as_ref()],
                    objets,
                )
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
        "La réservation a expiré ou appartient à un autre poste.".into()
    } else if cle == "#spatial" || cle.starts_with("#voisinage3/") || cle.starts_with("#zones/") {
        "La même zone en plan ou une donnée liée a changé.".into()
    } else if cle.starts_with("#types-terrains-") || cle.starts_with("#usagers-terrain-") {
        "Le type de terrain par défaut ou ses terrains ont changé.".into()
    } else if let Some(cle) = cle.strip_prefix("#usages/") {
        let cible = objet(cle, &[], objets);
        format!("{cible} · Un élément lié a changé.")
    } else if c.raison == "Propositions concurrentes incompatibles" {
        let cible = objet(
            cle,
            &[c.locale.as_ref(), c.acceptee.as_ref(), c.base.as_ref()],
            objets,
        );
        if c.acceptee.is_none() && c.base.is_some() {
            format!("{cible} · Supprimé du projet partagé.")
        } else if c.locale.is_none() && c.base.is_some() {
            format!("{cible} · Modifié dans le projet partagé, supprimé en local.")
        } else {
            format!("{cible} · Modifié des deux côtés.")
        }
    } else if cle == "protocole" {
        "Version du projet incompatible.".into()
    } else if cle == "validation" {
        "Modification incompatible avec le projet actuel.".into()
    } else if c.raison == "Dépendance ou collection de relations modifiée" {
        let cible = objet(cle, &[], objets);
        format!("{cible} · Une donnée liée a changé.")
    } else {
        "Publication bloquée.".into()
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

pub(super) fn fenetre(
    ctx: &egui::Context,
    copie: &Copie,
    libre: bool,
    ouvert: &mut bool,
) -> Option<Choix> {
    if copie.conflits.is_empty() {
        return None;
    }
    let mut choix = None;
    let objets = &copie.base.objets;
    egui::Window::new("Éléments en conflit")
        .id(egui::Id::unique("conflits_collaboration"))
        .open(ouvert)
        .default_width(620.0)
        .default_pos(ctx.content_rect().center() - egui::vec2(310.0, 165.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(570.0)
                .show(ui, |ui| {
                    for (id, conflits) in &copie.conflits {
                        ui.push_id(id, |ui| {
                            let proposition = copie.attente.iter().find(|t| &t.id == id);
                            let nombre = changements_visibles(proposition).count();
                            if nombre == 0 {
                                ui.heading(titre(proposition, conflits, objets));
                            }
                            for c in changements_visibles(proposition) {
                                ui.heading(titre_changement(c, objets));
                                let central = ::collaboration::concurrence::valeur(objets, &c.cle);
                                if c.apres.is_none() {
                                    ui.label("Suppression locale");
                                } else if c.avant.is_none() {
                                    ui.label("Ajout local");
                                } else if central.is_none() {
                                    ui.label("Supprimé du projet partagé");
                                }
                                if let (Some(a), Some(b)) = (c.apres.as_ref(), central) {
                                    for (champ, local, central) in champs(a, b, objets) {
                                        if local.is_empty() && central.is_empty() {
                                            ui.label(format!("{champ} différent"));
                                        } else {
                                            ui.label(format!(
                                                "{champ} · Local : {local} · Central : {central}"
                                            ));
                                        }
                                    }
                                }
                            }
                            let causes: BTreeSet<_> = conflits
                                .iter()
                                .filter_map(|c| expliquer(c, objets))
                                .collect();
                            for cause in causes {
                                ui.label(cause);
                            }
                            if let Some(action) = ui
                                .horizontal_wrapped(|ui| {
                                    boutons(ui, id, libre, proposition.is_some())
                                })
                                .inner
                            {
                                choix = Some(action);
                            }
                            ui.collapsing("Détails techniques", |ui| {
                                ui.monospace(id);
                                ui.monospace(
                                    serde_json::to_string_pretty(conflits).unwrap_or_default(),
                                );
                                if let Some(t) = proposition {
                                    ui.collapsing("Transaction complète", |ui| {
                                        ui.monospace(
                                            serde_json::to_string_pretty(t).unwrap_or_default(),
                                        );
                                    });
                                }
                            });
                            ui.separator();
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

    #[test]
    fn libelles_des_elements_utilisent_le_genre_et_le_nom() {
        let objets = Objets::from([
            (
                "ouvertures/o-1".into(),
                json!({"genre": "porte", "nom": "Entrée"}),
            ),
            (
                "dalles/d-1".into(),
                json!({"sorte": "toiture", "nom": "Garage"}),
            ),
        ]);
        assert_eq!(objet("ouvertures/o-1", &[], &objets), "Porte « Entrée »");
        assert_eq!(objet("dalles/d-1", &[], &objets), "Toiture « Garage »");
        assert_eq!(
            objet(
                "ouvertures/o-2",
                &[Some(&json!({"genre": "fenetre"}))],
                &objets
            ),
            "Fenêtre"
        );
    }

    #[test]
    fn boutons_de_resolution_ciblent_le_conflit_affiche() {
        for (libelle, libre, attendu) in [
            (
                "Accepter le central",
                true,
                Some(("central", "conflit-affiche".into())),
            ),
            (
                "Conserver le local",
                true,
                Some(("local", "conflit-affiche".into())),
            ),
            ("Conserver le local", false, None),
        ] {
            let ctx = egui::Context::default();
            let mut action = None;
            let mut image = |events| {
                let mut sortie = ctx.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        ui.horizontal_wrapped(|ui| {
                            action = boutons(ui, "conflit-affiche", libre, true);
                        });
                    },
                );
                // sans moteur de rendu : les textures de l'image ne sont envoyées nulle part
                sortie.textures_delta.clear();
                sortie
            };
            image(vec![]);
            let sortie = image(vec![]);
            let pos = sortie
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == libelle => {
                        Some(t.pos + t.galley.size() / 2.0)
                    }
                    _ => None,
                })
                .expect("bouton de résolution introuvable");
            for pressed in [true, false] {
                image(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
            assert_eq!(
                action.map(|a| match a {
                    Choix::Central(id) => ("central", id),
                    Choix::Local(id) => ("local", id),
                }),
                attendu
            );
        }
    }
}
