//! Registre local des viewers pilotables : un fichier JSON par instance dans
//! `%LOCALAPPDATA%\BIMedit\instances` (port, jeton, fichier ouvert). Le pont MCP le lit pour
//! lister les instances et en choisir une ; un viewer lancé par l'utilisateur est donc
//! joignable comme un viewer lancé par le pont.
//!
//! Mode isolé (`BIMEDIT_PORT` défini, tests et CI) : port fixe, aucune inscription, jeton
//! seulement si `BIMEDIT_JETON` est fourni.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub pid: u32,
    pub port: u16,
    pub jeton: String,
    /// Démarrage, en secondes depuis 1970.
    pub debut: u64,
    pub exe: String,
    /// Projet natif ouvert (chemin complet), à défaut nom du fichier affiché.
    #[serde(default)]
    pub fichier: Option<String>,
    /// Fichier partagé (`.bimpartage`) de la session collaborative.
    #[serde(default)]
    pub partage: Option<String>,
}

/// Port fixe demandé (mode isolé).
pub fn port_isole() -> Option<u16> {
    std::env::var("BIMEDIT_PORT").ok().and_then(|p| p.parse().ok())
}

/// Jeton imposé par l'environnement (mode isolé).
pub fn jeton_impose() -> Option<String> {
    std::env::var("BIMEDIT_JETON").ok().filter(|j| !j.is_empty())
}

pub fn dossier() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("BIMedit").join("instances")
}

fn fichier_de(pid: u32) -> PathBuf {
    dossier().join(format!("{pid}.json"))
}

pub fn nouveau_jeton() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Instance de ce processus, une fois inscrite.
static COURANTE: Mutex<Option<Instance>> = Mutex::new(None);

fn ecrire(i: &Instance) {
    let chemin = fichier_de(i.pid);
    if std::fs::create_dir_all(dossier()).is_err() { return; }
    // écriture atomique : le pont ne lit jamais un fichier à moitié écrit
    let temporaire = chemin.with_extension("tmp");
    if let Ok(json) = serde_json::to_vec_pretty(i)
        && std::fs::write(&temporaire, json).is_ok()
    {
        let _ = std::fs::rename(&temporaire, &chemin);
    }
}

/// Inscrit le viewer (mode normal seulement).
pub fn inscrire(port: u16, jeton: &str) {
    let i = Instance {
        pid: std::process::id(),
        port,
        jeton: jeton.into(),
        debut: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        exe: std::env::current_exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default(),
        fichier: None,
        partage: None,
    };
    ecrire(&i);
    *COURANTE.lock().unwrap() = Some(i);
}

/// Met à jour le fichier ouvert et le partage ; n'écrit que s'ils ont changé.
pub fn publier(fichier: Option<String>, partage: Option<String>) {
    let mut c = COURANTE.lock().unwrap();
    let Some(i) = c.as_mut() else { return };
    if i.fichier == fichier && i.partage == partage { return; }
    i.fichier = fichier;
    i.partage = partage;
    ecrire(i);
}

/// Pid et port de ce viewer, s'il est inscrit.
pub fn courante() -> Option<(u32, u16)> {
    COURANTE.lock().unwrap().as_ref().map(|i| (i.pid, i.port))
}

/// Retire l'inscription de ce processus (fermeture).
pub fn retirer() {
    if let Some(i) = COURANTE.lock().unwrap().take() {
        let _ = std::fs::remove_file(fichier_de(i.pid));
    }
}

/// Inscriptions présentes (y compris d'instances arrêtées : le pont vérifie chacune).
pub fn lire() -> Vec<Instance> {
    let Ok(entrees) = std::fs::read_dir(dossier()) else { return Vec::new() };
    let mut liste: Vec<Instance> = entrees
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    liste.sort_by_key(|i| i.debut);
    liste
}

/// Supprime l'inscription d'une instance qui ne répond plus.
pub fn oublier(pid: u32) {
    let _ = std::fs::remove_file(fichier_de(pid));
}

/// Retire le préfixe `@jeton ` d'une ligne et vérifie le jeton attendu.
pub fn authentifier<'a>(ligne: &'a str, attendu: Option<&str>) -> Result<&'a str, &'static str> {
    let (fourni, reste) = match ligne.strip_prefix('@') {
        Some(r) => r.split_once(' ').map_or((Some(r), ""), |(j, l)| (Some(j), l)),
        None => (None, ligne),
    };
    match (attendu, fourni) {
        (None, _) => Ok(reste),
        (Some(a), Some(f)) if egal_temps_constant(a.as_bytes(), f.as_bytes()) => Ok(reste),
        _ => Err("jeton du pilote absent ou invalide"),
    }
}

fn egal_temps_constant(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jeton_exige_et_retire() {
        assert_eq!(authentifier("@abc etat", Some("abc")), Ok("etat"));
        assert_eq!(authentifier("@abc ouvrir C:/a b.json", Some("abc")), Ok("ouvrir C:/a b.json"));
        assert!(authentifier("etat", Some("abc")).is_err());
        assert!(authentifier("@abd etat", Some("abc")).is_err());
        assert!(authentifier("@ab etat", Some("abc")).is_err());
        // mode isolé sans jeton : lignes brutes acceptées, préfixe toléré
        assert_eq!(authentifier("etat", None), Ok("etat"));
        assert_eq!(authentifier("@x etat", None), Ok("etat"));
    }

    #[test]
    fn jetons_distincts_et_longs() {
        let (a, b) = (nouveau_jeton(), nouveau_jeton());
        assert_ne!(a, b);
        assert_eq!(a.len(), 32);
    }
}
