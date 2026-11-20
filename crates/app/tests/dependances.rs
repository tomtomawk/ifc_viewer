//! Garde-fou de l'architecture : sens des dépendances entre les crates du workspace.
//!
//! Lit les `Cargo.toml` de `crates/*` (parsing simple, sans dépendance externe) et vérifie :
//! 1. chaque dépendance interne (`path = "../…"`) est dans `AUTORISEES` ; les
//!    `[dev-dependencies]` sont contrôlées à part (`AUTORISEES` + `DEV_SEULEMENT`) ;
//! 2. il n'y a pas de cycle, et toute dépendance va vers une crate située avant dans `ORDRE` ;
//! 3. `eframe`/`egui`/`wgpu` ne sont déclarées que dans `CRATES_AVEC_EGUI`.
//!
//! `AUTORISEES` reproduit le tableau du §2 de `docs/ARCHITECTURE.md` : modifier les deux ensemble.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Ordre des crates : une crate n'importe que des crates situées avant elle.
const ORDRE: [&str; 15] = [
    "noyau",
    "modele",
    "ifc_base",
    "scene",
    "formats",
    "maillage",
    "commandes",
    "moteur2d",
    "moteur3d",
    "echanges",
    "collaboration",
    "pilote",
    "config",
    "interface",
    "app",
];

/// Dépendances internes autorisées (production et développement), §2 de ARCHITECTURE.md.
/// Resserrées aux dépendances réellement utilisées : en ajouter une se décide ici et au §2.
const AUTORISEES: [(&str, &[&str]); 15] = [
    ("noyau", &[]),
    ("modele", &["noyau"]),
    ("ifc_base", &["noyau"]),
    ("scene", &["noyau", "modele", "ifc_base"]),
    ("formats", &["noyau", "ifc_base", "scene"]),
    ("maillage", &["noyau", "modele", "ifc_base", "scene", "formats"]),
    ("commandes", &["noyau", "modele"]),
    ("moteur2d", &["noyau", "modele", "scene", "maillage"]),
    ("moteur3d", &["noyau", "modele", "scene"]),
    ("echanges", &["noyau", "modele", "ifc_base", "formats"]),
    ("collaboration", &["noyau", "modele", "maillage", "commandes", "echanges"]),
    ("pilote", &["noyau", "modele", "maillage", "commandes", "moteur2d", "echanges", "collaboration"]),
    ("config", &["noyau", "modele", "echanges"]),
    (
        "interface",
        &["noyau", "modele", "ifc_base", "scene", "formats", "maillage", "commandes", "moteur2d", "moteur3d", "echanges", "collaboration", "pilote", "config"],
    ),
    ("app", &["noyau", "modele", "ifc_base", "formats", "echanges", "collaboration", "pilote", "config", "interface"]),
];

/// Autorisations supplémentaires, réservées aux `[dev-dependencies]` (tests d'intégration).
const DEV_SEULEMENT: [(&str, &[&str]); 4] = [
    ("commandes", &["maillage"]),
    ("moteur2d", &["commandes"]),
    ("moteur3d", &["maillage"]),
    ("echanges", &["commandes", "maillage", "moteur2d"]),
];

/// Seules crates qui peuvent déclarer `eframe`, `egui` ou `wgpu`.
const CRATES_AVEC_EGUI: [&str; 5] = ["moteur3d", "pilote", "config", "interface", "app"];
const EXTERNES_INTERDITS_AILLEURS: [&str; 4] = ["eframe", "egui", "egui_wgpu", "wgpu"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Genre {
    Production,
    Dev,
    Build,
}

struct Dep {
    /// Nom de la clé dans le `Cargo.toml`.
    cle: String,
    /// Crate interne visée (`path = "../x"`), sinon `None`.
    interne: Option<String>,
    genre: Genre,
}

fn racine() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Genre de dépendance d'un en-tête de section TOML, `None` si ce n'en est pas une.
fn genre_de_section(entete: &str) -> Option<Genre> {
    let e = entete.trim().trim_start_matches('[').trim_end_matches(']');
    if e.ends_with("dev-dependencies") {
        Some(Genre::Dev)
    } else if e.ends_with("build-dependencies") {
        Some(Genre::Build)
    } else if e == "dependencies" || e.ends_with(".dependencies") {
        Some(Genre::Production)
    } else {
        None
    }
}

fn lire_dependances(manifeste: &str) -> Vec<Dep> {
    let mut genre = None;
    let mut deps = Vec::new();
    for ligne in manifeste.lines() {
        let ligne = ligne.split('#').next().unwrap_or("").trim();
        if ligne.is_empty() {
            continue;
        }
        if ligne.starts_with('[') {
            assert!(!ligne.starts_with("[dependencies.") && !ligne.starts_with("[dev-dependencies."), "tableau `[dependencies.x]` non géré par le garde-fou : {ligne}");
            genre = genre_de_section(ligne);
            continue;
        }
        let Some(genre) = genre else { continue };
        let Some((cle, valeur)) = ligne.split_once('=') else { continue };
        let cle = cle.trim().split('.').next().unwrap_or("").trim().to_string();
        let interne = valeur.find("path").and_then(|i| {
            let reste = &valeur[i..];
            let debut = reste.find("\"../")? + 4;
            let fin = reste[debut..].find('"')? + debut;
            Some(reste[debut..fin].trim_end_matches('/').to_string())
        });
        deps.push(Dep { cle, interne, genre });
    }
    deps
}

/// Crates du workspace : nom du dossier -> dépendances lues.
fn lire_workspace() -> Result<BTreeMap<String, Vec<Dep>>, String> {
    let dossier = racine().join("crates");
    let mut crates = BTreeMap::new();
    for entree in std::fs::read_dir(&dossier).map_err(|e| format!("dossier crates/ : {e}"))? {
        let chemin = entree.map_err(|e| format!("dossier crates/ : {e}"))?.path();
        let manifeste = chemin.join("Cargo.toml");
        if !manifeste.is_file() {
            continue;
        }
        let nom = chemin.file_name().ok_or_else(|| format!("{} : nom de dossier absent", chemin.display()))?.to_string_lossy().into_owned();
        let texte = std::fs::read_to_string(&manifeste).map_err(|e| format!("{} : {e}", manifeste.display()))?;
        crates.insert(nom, lire_dependances(&texte));
    }
    Ok(crates)
}

fn table(t: &[(&'static str, &'static [&'static str])]) -> BTreeMap<&'static str, &'static [&'static str]> {
    t.iter().copied().collect()
}

#[test]
fn les_crates_du_workspace_sont_celles_du_garde_fou() {
    // une crate ajoutée doit être inscrite dans ORDRE et AUTORISEES (et au §2 d'ARCHITECTURE.md)
    let reelles: BTreeSet<String> = lire_workspace().unwrap_or_else(|e| panic!("{e}")).into_keys().collect();
    let connues: BTreeSet<String> = ORDRE.iter().map(|s| s.to_string()).collect();
    assert_eq!(reelles, connues, "crates/* et ORDRE diffèrent : mettre à jour tests/dependances.rs et docs/ARCHITECTURE.md §2");
    let autorisees: BTreeSet<&str> = AUTORISEES.iter().map(|(n, _)| *n).collect();
    assert_eq!(autorisees, ORDRE.iter().copied().collect::<BTreeSet<_>>(), "AUTORISEES et ORDRE diffèrent");
}

#[test]
fn dependances_internes_autorisees() {
    let autorisees = table(&AUTORISEES);
    let dev = table(&DEV_SEULEMENT);
    let mut erreurs = Vec::new();
    for (nom, deps) in lire_workspace().unwrap_or_else(|e| panic!("{e}")) {
        let permises = autorisees.get(nom.as_str()).copied().unwrap_or(&[]);
        let permises_dev = dev.get(nom.as_str()).copied().unwrap_or(&[]);
        for d in deps {
            let Some(cible) = d.interne else { continue };
            let ok = match d.genre {
                Genre::Production | Genre::Build => permises.contains(&cible.as_str()),
                Genre::Dev => permises.contains(&cible.as_str()) || permises_dev.contains(&cible.as_str()),
            };
            if !ok {
                erreurs.push(format!("{nom} -> {cible} ({:?}) interdite : voir docs/ARCHITECTURE.md §2", d.genre));
            }
            if d.cle != cible {
                erreurs.push(format!("{nom} : la clé `{}` ne porte pas le nom de la crate `{cible}`", d.cle));
            }
        }
    }
    assert!(erreurs.is_empty(), "dépendances internes non autorisées :\n{}", erreurs.join("\n"));
}

#[test]
fn aucun_cycle_et_ordre_respecte() {
    let rang: BTreeMap<&str, usize> = ORDRE.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let crates = lire_workspace().unwrap_or_else(|e| panic!("{e}"));
    let mut aretes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut erreurs = Vec::new();
    for (nom, deps) in &crates {
        for d in deps {
            let Some(cible) = &d.interne else { continue };
            aretes.entry(nom.clone()).or_default().push(cible.clone());
            match (rang.get(nom.as_str()), rang.get(cible.as_str())) {
                (Some(a), Some(b)) if b < a => {}
                _ => erreurs.push(format!("{nom} -> {cible} : la cible doit être située avant dans l'ordre")),
            }
        }
    }
    // recherche de cycle (parcours en profondeur) : indépendante de ORDRE
    fn visiter(n: &str, aretes: &BTreeMap<String, Vec<String>>, pile: &mut Vec<String>, finis: &mut BTreeSet<String>, erreurs: &mut Vec<String>) {
        if finis.contains(n) {
            return;
        }
        if let Some(i) = pile.iter().position(|p| p == n) {
            erreurs.push(format!("cycle : {} -> {n}", pile[i..].join(" -> ")));
            return;
        }
        pile.push(n.to_string());
        for s in aretes.get(n).into_iter().flatten() {
            visiter(s, aretes, pile, finis, erreurs);
        }
        pile.pop();
        finis.insert(n.to_string());
    }
    let mut finis = BTreeSet::new();
    for n in crates.keys() {
        visiter(n, &aretes, &mut Vec::new(), &mut finis, &mut erreurs);
    }
    assert!(erreurs.is_empty(), "graphe de dépendances invalide :\n{}", erreurs.join("\n"));
}

#[test]
fn egui_et_wgpu_seulement_dans_les_crates_autorisees() {
    let mut erreurs = Vec::new();
    for (nom, deps) in lire_workspace().unwrap_or_else(|e| panic!("{e}")) {
        if CRATES_AVEC_EGUI.contains(&nom.as_str()) {
            continue;
        }
        for d in deps {
            if EXTERNES_INTERDITS_AILLEURS.contains(&d.cle.as_str()) {
                erreurs.push(format!("{nom} déclare `{}` ({:?}) : réservé à {}", d.cle, d.genre, CRATES_AVEC_EGUI.join(", ")));
            }
        }
    }
    assert!(erreurs.is_empty(), "dépendances d'interface hors des crates autorisées :\n{}", erreurs.join("\n"));
}
