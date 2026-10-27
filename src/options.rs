//! Options de l'application, enregistrées dans le dossier de configuration de l'utilisateur.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Options {
    /// Arborescence spatiale, classes et attributs IFC pour les modèles Revit et natifs.
    pub navigation_ifc: bool,
    /// Gabarit choisi (None : gabarit par défaut embarqué).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gabarit: Option<PathBuf>,
}

/// `%APPDATA%\ifc_viewer\options.json`, `$XDG_CONFIG_HOME/ifc_viewer/options.json` ou `~/.config/ifc_viewer/options.json`.
fn chemin() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("ifc_viewer").join("options.json"))
}

impl Options {
    pub fn charger() -> Self {
        chemin()
            .and_then(|c| std::fs::read_to_string(c).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn enregistrer(&self) {
        let Some(c) = chemin() else { return };
        if let Some(d) = c.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(c, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_ifc_desactivee_par_defaut() {
        assert!(!Options::default().navigation_ifc);
        let o: Options = serde_json::from_str("{}").unwrap();
        assert_eq!(o, Options::default());
    }
}
