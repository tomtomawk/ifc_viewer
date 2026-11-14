//! Unité d'affichage des longueurs (les valeurs du modèle sont en millimètres).

use serde::{Deserialize, Serialize};
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
    /// Valeur tapée dans l'unité d'affichage, convertie en millimètres.
    pub fn vers_mm(self, valeur: f64) -> f64 {
        valeur * self.facteur()
    }
    pub fn afficher(self, mm: f64) -> String {
        format!(
            "{}{}",
            noyau::nombres::format_real(mm / self.facteur()),
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
