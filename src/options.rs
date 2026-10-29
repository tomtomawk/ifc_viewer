//! Options de l'application, enregistrées dans le dossier de configuration de l'utilisateur.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Unite {
    Millimetre,
    #[default]
    Centimetre,
    Metre,
}
impl Unite {
    pub fn facteur(self) -> f64 {
        match self {
            Self::Millimetre => 1.0,
            Self::Centimetre => 10.0,
            Self::Metre => 1000.0,
        }
    }
    pub fn suffixe(self) -> &'static str {
        match self {
            Self::Millimetre => " mm",
            Self::Centimetre => " cm",
            Self::Metre => " m",
        }
    }
    pub fn afficher(self, mm: f64) -> String {
        format!(
            "{}{}",
            crate::step::format_real(mm / self.facteur()),
            self.suffixe()
        )
    }
}
static UNITE: AtomicU8 = AtomicU8::new(1);
pub fn definir_unite(u: Unite) {
    UNITE.store(
        match u {
            Unite::Millimetre => 0,
            Unite::Centimetre => 1,
            Unite::Metre => 2,
        },
        Ordering::Relaxed,
    );
}
pub fn unite() -> Unite {
    match UNITE.load(Ordering::Relaxed) {
        0 => Unite::Millimetre,
        2 => Unite::Metre,
        _ => Unite::Centimetre,
    }
}
pub trait Dimensions<'a> {
    fn dimensions(self) -> eframe::egui::DragValue<'a>;
    fn dimensions_metres(self) -> eframe::egui::DragValue<'a>;
}
impl<'a> Dimensions<'a> for eframe::egui::DragValue<'a> {
    fn dimensions(self) -> eframe::egui::DragValue<'a> {
        let u = unite();
        let facteur = u.facteur();
        self.custom_formatter(move |v, _| crate::step::format_real(v / facteur))
            .custom_parser(move |s| {
                s.trim()
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .map(|v| v * facteur)
            })
            .suffix(u.suffixe())
    }
    fn dimensions_metres(self) -> eframe::egui::DragValue<'a> {
        let u = unite();
        let facteur = u.facteur();
        self.custom_formatter(move |v, _| crate::step::format_real(v * 1000.0 / facteur))
            .custom_parser(move |s| {
                s.trim()
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .map(|v| v * facteur / 1000.0)
            })
            .suffix(u.suffixe())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Options {
    /// Arborescence spatiale, classes et attributs IFC pour les modèles Revit et natifs.
    pub navigation_ifc: bool,
    pub unite: Unite,
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
    #[test]
    fn centimetrage_par_defaut_et_conversion() {
        assert_eq!(Options::default().unite, Unite::Centimetre);
        assert_eq!(Unite::Centimetre.afficher(2800.0), "280 cm");
        assert_eq!(Unite::Metre.afficher(2800.0), "2.8 m");
        assert_eq!(Unite::Millimetre.afficher(2800.0), "2800 mm");
    }
}
