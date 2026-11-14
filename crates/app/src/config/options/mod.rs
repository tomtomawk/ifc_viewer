//! Options de l'application, enregistrées dans le dossier de configuration de l'utilisateur.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub mod raccourcis;

// Unité d'affichage : définie dans le modèle (les cotes et nomenclatures la lisent) ;
// ré-exportée ici, où l'interface la règle et la lit (84 références).
pub use modele::unite::{Unite, definir_unite, unite};

pub trait Dimensions<'a> {
    fn dimensions(self) -> eframe::egui::DragValue<'a>;
    fn dimensions_metres(self) -> eframe::egui::DragValue<'a>;
}
impl<'a> Dimensions<'a> for eframe::egui::DragValue<'a> {
    fn dimensions(self) -> eframe::egui::DragValue<'a> {
        let u = unite();
        let facteur = u.facteur();
        self.custom_formatter(move |v, _| crate::formats::ifc::step::format_real(v / facteur))
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
        self.custom_formatter(move |v, _| crate::formats::ifc::step::format_real(v * 1000.0 / facteur))
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all="snake_case")]
pub enum ThemeInterface { Clair, Sombre, #[default] Systeme }
impl ThemeInterface {
    /// Cycle du bouton de la barre haute, comme photocraft : Système → Clair → Sombre → Système.
    pub fn suivant(self)->Self {match self {Self::Systeme=>Self::Clair,Self::Clair=>Self::Sombre,Self::Sombre=>Self::Systeme}}
    pub fn nom(self)->&'static str {match self {Self::Clair=>"Clair",Self::Sombre=>"Sombre",Self::Systeme=>"Système"}}
    pub const TOUS:[Self;3]=[Self::Systeme,Self::Clair,Self::Sombre];
    pub fn appliquer(self,ctx:&eframe::egui::Context) {
        ctx.set_theme(match self {
            Self::Clair=>eframe::egui::ThemePreference::Light,
            Self::Sombre=>eframe::egui::ThemePreference::Dark,
            Self::Systeme=>eframe::egui::ThemePreference::System,
        });
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Options {
    pub theme_interface:ThemeInterface,
    pub delai_accrochage:f64,
    pub raccourcis:std::collections::BTreeMap<String,String>,
    /// Arborescence spatiale, classes et attributs IFC pour les modèles Revit et natifs.
    pub navigation_ifc: bool,
    pub unite: Unite,
    /// Gabarit choisi (None : gabarit par défaut embarqué).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gabarit: Option<PathBuf>,
}
impl Default for Options {
    fn default()->Self {Self{theme_interface:ThemeInterface::default(),navigation_ifc:false,unite:Unite::default(),gabarit:None,delai_accrochage:0.8,raccourcis:Default::default()}}
}

/// `%APPDATA%\bimedit\options.json`, `$XDG_CONFIG_HOME/bimedit/options.json` ou `~/.config/bimedit/options.json`.
fn chemin() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("bimedit").join("options.json"))
}

impl Options {
    pub fn charger() -> Self {
        let mut o:Self=chemin()
            .and_then(|c| std::fs::read_to_string(c).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if o.verifier().is_err() {o.raccourcis.clear();o.delai_accrochage=0.8;}
        o
    }

    pub fn enregistrer(&self) {
        let _=self.sauver();
    }
    pub fn sauver(&self)->Result<(),String> {
        self.verifier()?;
        let c=chemin().ok_or("Dossier de configuration introuvable")?;
        if let Some(d) = c.parent() {
            std::fs::create_dir_all(d).map_err(|e|e.to_string())?;
        }
        let t=serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?;
        crate::echanges::enregistrement::ecrire_atomique(&c,&t)
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
    fn migration_et_persistance_des_preferences_personnelles() {
        let mut o:Options=serde_json::from_str(r#"{"navigation_ifc":true,"unite":"metre"}"#).unwrap();
        assert_eq!(o.delai_accrochage,0.8);
        assert_eq!(o.raccourci(&raccourcis::COMMANDES[0]),"Ctrl+N");
        o.raccourcis.insert("mur".into(),"MW".into());
        o.verifier().unwrap();
        let r:Options=serde_json::from_str(&serde_json::to_string(&o).unwrap()).unwrap();
        assert_eq!(r,o);
        o.delai_accrochage=f64::NAN;assert!(o.verifier().is_err());
    }
    #[test]
    fn centimetrage_par_defaut_et_conversion() {
        assert_eq!(Options::default().unite, Unite::Centimetre);
        assert_eq!(Unite::Centimetre.afficher(2800.0), "280 cm");
        assert_eq!(Unite::Metre.afficher(2800.0), "2.8 m");
        assert_eq!(Unite::Millimetre.afficher(2800.0), "2800 mm");
    }
}
