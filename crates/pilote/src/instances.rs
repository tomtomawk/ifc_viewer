//! Registre local des instances pilotables : un fichier JSON par instance dans
//! `%LOCALAPPDATA%\BIMedit\instances` (Windows) ou un dossier propre à l'utilisateur
//! ailleurs (`dossier`), lisible par lui seul (port, jeton, fichier ouvert). Le pont MCP le lit pour
//! lister les instances et en choisir une ; une instance lancée par l'utilisateur est donc
//! joignable comme une instance lancée par le pont.
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

/// Dossier du registre. Windows : `%LOCALAPPDATA%\BIMedit\instances`. Ailleurs, jamais le
/// dossier temporaire commun (les jetons y seraient lisibles par les autres comptes) :
/// `$XDG_RUNTIME_DIR/bimedit/instances` (propre à l'utilisateur), sinon
/// `$XDG_STATE_HOME/bimedit/instances`, sinon `~/.local/state/bimedit/instances`.
pub fn dossier() -> PathBuf {
    dossier_depuis(|nom| std::env::var_os(nom))
}

/// `dossier` d'après les variables d'environnement données (testable).
fn dossier_depuis(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> PathBuf {
    let variable = |nom: &str| env(nom).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(base) = variable("LOCALAPPDATA") {
        return base.join("BIMedit").join("instances");
    }
    let base = variable("XDG_RUNTIME_DIR")
        .or_else(|| variable("XDG_STATE_HOME"))
        .or_else(|| variable("HOME").map(|h| h.join(".local").join("state")))
        .unwrap_or_else(|| std::env::temp_dir().join(format!("bimedit-{}", utilisateur())));
    base.join("bimedit").join("instances")
}

/// Nom du compte, pour un dernier recours sans dossier personnel.
fn utilisateur() -> String {
    std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "inconnu".into())
}

/// Crée le dossier du registre, réservé à l'utilisateur hors Windows (0700).
fn creer_dossier() -> std::io::Result<()> {
    let d = dossier();
    std::fs::create_dir_all(&d)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn fichier_de(pid: u32) -> PathBuf {
    dossier().join(format!("{pid}.json"))
}

pub fn nouveau_jeton() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Instance de ce processus, une fois inscrite.
static COURANTE: Mutex<Option<Instance>> = Mutex::new(None);

fn ecrire(i: &Instance) -> Result<(), String> {
    let chemin = fichier_de(i.pid);
    let message = |e: std::io::Error| format!("Registre des instances ({}) : {e}", chemin.display());
    creer_dossier().map_err(message)?;
    // écriture atomique : le pont ne lit jamais un fichier à moitié écrit
    let temporaire = chemin.with_extension("tmp");
    let json = serde_json::to_vec_pretty(i).map_err(|e| e.to_string())?;
    std::fs::write(&temporaire, json).map_err(message)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporaire, std::fs::Permissions::from_mode(0o600)).map_err(message)?;
    }
    std::fs::rename(&temporaire, &chemin).map_err(message)
}

/// Inscrit l'instance (mode normal seulement). Un échec est rendu : l'instance reste
/// pilotable par son port, mais le pont MCP ne la trouvera pas.
pub fn inscrire(port: u16, jeton: &str) -> Result<(), String> {
    let i = Instance {
        pid: std::process::id(),
        port,
        jeton: jeton.into(),
        debut: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        exe: std::env::current_exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default(),
        fichier: None,
        partage: None,
    };
    let r = ecrire(&i);
    *COURANTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(i);
    r
}

/// Met à jour le fichier ouvert et le partage ; n'écrit que s'ils ont changé.
pub fn publier(fichier: Option<String>, partage: Option<String>) {
    let mut c = COURANTE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(i) = c.as_mut() else { return };
    if i.fichier == fichier && i.partage == partage { return; }
    i.fichier = fichier;
    i.partage = partage;
    if let Err(e) = ecrire(i) {
        eprintln!("{e}");
    }
}

/// Pid et port de cette instance, si elle est inscrite.
pub fn courante() -> Option<(u32, u16)> {
    COURANTE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|i| (i.pid, i.port))
}

/// Retire l'inscription de ce processus (fermeture).
pub fn retirer() {
    if let Some(i) = COURANTE.lock().unwrap_or_else(|e| e.into_inner()).take() {
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

    /// Hors Windows, le registre (jetons) n'est jamais dans le dossier temporaire commun.
    #[test]
    fn dossier_du_registre_propre_a_l_utilisateur() {
        let env = |vars: &'static [(&'static str, &'static str)]| move |nom: &str| vars.iter().find(|(n, _)| *n == nom).map(|(_, v)| std::ffi::OsString::from(v));
        assert_eq!(dossier_depuis(env(&[("LOCALAPPDATA", "C:/L"), ("HOME", "/h")])), PathBuf::from("C:/L/BIMedit/instances"));
        assert_eq!(dossier_depuis(env(&[("XDG_RUNTIME_DIR", "/run/user/1000"), ("HOME", "/h")])), PathBuf::from("/run/user/1000/bimedit/instances"));
        assert_eq!(dossier_depuis(env(&[("HOME", "/h")])), PathBuf::from("/h/.local/state/bimedit/instances"));
        assert_eq!(dossier_depuis(env(&[("LOCALAPPDATA", ""), ("HOME", "/h")])), PathBuf::from("/h/.local/state/bimedit/instances"));
        assert_ne!(dossier_depuis(env(&[])).parent(), Some(std::env::temp_dir().join("BIMedit").as_path()));
    }

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
