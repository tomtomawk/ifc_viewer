//! Gabarit de projet, comme un gabarit Revit : matériaux, types de murs, d'éléments
//! horizontaux et de vues proposés à la création. Le gabarit par défaut est embarqué
//! (`gabarits/defaut.gabarit.json`) ; un autre fichier peut être choisi dans
//! Début > Options.

use crate::modele::{Couche, Materiau, MotifCoupe, SorteDalle};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, RwLock};

pub const FORMAT: &str = "ifc_viewer.gabarit";
const DEFAUT: &str = include_str!("../gabarits/defaut.gabarit.json");

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Gabarit {
    pub format: String,
    pub version: u32,
    pub nom: String,
    /// Niveaux d'un nouveau projet.
    #[serde(default)]
    pub niveaux: Vec<NiveauGabarit>,
    /// Matériaux reconnus par un mot de leur nom (premier qui correspond).
    #[serde(default)]
    pub materiaux: Vec<FamilleMateriau>,
    /// Types de murs proposés en plus de ceux du projet.
    #[serde(default)]
    pub types_murs: Vec<TypeGabarit>,
    #[serde(default)]
    pub types_surfaces: Vec<TypeSurface>,
    #[serde(default)]
    pub types_vues: Vec<TypeVue>,
}

/// Niveau d'un nouveau projet : nom et altitude (mm).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NiveauGabarit {
    pub nom: String,
    pub altitude: f64,
}

/// Couleurs et motifs (surface, coupe) des matériaux dont le nom contient un des mots.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FamilleMateriau {
    pub mots: Vec<String>,
    /// Couleur de surface (fond).
    pub couleur: [u8; 3],
    /// Motif de coupe (premier plan).
    #[serde(default)]
    pub motif: MotifCoupe,
    #[serde(default)]
    pub coupe_couleur: Option<[u8; 3]>,
    #[serde(default)]
    pub coupe_fond: Option<[u8; 3]>,
    #[serde(default)]
    pub coupe_fond_motif: Option<MotifCoupe>,
    #[serde(default)]
    pub surface_fond_motif: Option<MotifCoupe>,
    #[serde(default)]
    pub surface_motif: Option<MotifCoupe>,
    #[serde(default)]
    pub surface_couleur: Option<[u8; 3]>,
}

/// Type composé : couches (murs : de l'extérieur vers l'intérieur).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypeGabarit {
    pub nom: String,
    pub couches: Vec<Couche>,
}

/// Type d'élément horizontal : couches du haut vers le bas.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypeSurface {
    pub sorte: SorteDalle,
    pub nom: String,
    pub couches: Vec<Couche>,
}

/// Type de coupe ou d'élévation : échelle et profondeur (mm) par défaut.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypeVue {
    pub elevation: bool,
    pub nom: String,
    #[serde(default)]
    pub aide: String,
    pub echelle: f64,
    pub profondeur: f64,
}

impl Gabarit {
    pub fn defaut() -> Gabarit {
        Gabarit::lire(DEFAUT).expect("gabarit par défaut valide")
    }

    pub fn lire(texte: &str) -> Result<Gabarit, String> {
        let g: Gabarit = serde_json::from_str(texte).map_err(|e| format!("Gabarit illisible : {e}"))?;
        if g.format != FORMAT {
            return Err(format!("Ce fichier n'est pas un gabarit ({FORMAT})."));
        }
        let couches_valides = |cs: &[Couche]| !cs.is_empty() && cs.iter().all(|c| c.epaisseur > 0.0 && c.epaisseur.is_finite());
        if let Some(t) = g.types_murs.iter().find(|t| !couches_valides(&t.couches)) {
            return Err(format!("Gabarit : le type de mur « {} » doit avoir des couches d'épaisseur positive.", t.nom));
        }
        if let Some(t) = g.types_surfaces.iter().find(|t| !couches_valides(&t.couches)) {
            return Err(format!("Gabarit : le type « {} » doit avoir des couches d'épaisseur positive.", t.nom));
        }
        if let Some(t) = g.types_vues.iter().find(|t| !(t.echelle > 0.0 && t.profondeur > 0.0)) {
            return Err(format!("Gabarit : le type de vue « {} » doit avoir une échelle et une profondeur positives.", t.nom));
        }
        if !g.types_vues.iter().any(|t| !t.elevation) || !g.types_vues.iter().any(|t| t.elevation) {
            return Err("Gabarit : il faut au moins un type de coupe et un type d'élévation.".into());
        }
        Ok(g)
    }

    pub fn charger(chemin: &Path) -> Result<Gabarit, String> {
        let texte = std::fs::read_to_string(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        Gabarit::lire(&texte)
    }

    /// Types d'une sorte d'élément horizontal, dans l'ordre du gabarit.
    pub fn surfaces(&self, sorte: SorteDalle) -> Vec<&TypeSurface> {
        self.types_surfaces.iter().filter(|t| t.sorte == sorte).collect()
    }

    /// Matériau d'après son nom ; à défaut, couleur stable tirée du nom.
    pub fn materiau(&self, nom: &str) -> Materiau {
        let n = nom.to_lowercase();
        match self.materiaux.iter().find(|f| f.mots.iter().any(|m| n.contains(&m.to_lowercase()))) {
            Some(f) => Materiau {
                nom: nom.into(),
                couleur: f.couleur,
                motif: f.motif,
                coupe_couleur: f.coupe_couleur,
                coupe_fond: f.coupe_fond,
                coupe_fond_motif: f.coupe_fond_motif,
                surface_fond_motif: f.surface_fond_motif,
                surface_motif: f.surface_motif,
                surface_couleur: f.surface_couleur,
            },
            None => {
                // couleur assez claire pour les hachures
                let h = crate::modele::identifiant("materiau", nom);
                let v = u32::from_str_radix(&h[..6], 16).unwrap_or(0);
                let couleur = [150 + (v & 0x5f) as u8, 150 + ((v >> 8) & 0x5f) as u8, 150 + ((v >> 16) & 0x5f) as u8];
                Materiau { nom: nom.into(), couleur, motif: MotifCoupe::Diagonales, ..Default::default() }
            }
        }
    }
}

static ACTIF: RwLock<Option<Arc<Gabarit>>> = RwLock::new(None);

/// Gabarit en vigueur (par défaut tant qu'aucun autre n'a été choisi).
pub fn actif() -> Arc<Gabarit> {
    if let Some(g) = ACTIF.read().ok().and_then(|g| g.clone()) {
        return g;
    }
    let g = Arc::new(Gabarit::defaut());
    if let Ok(mut a) = ACTIF.write() {
        a.get_or_insert_with(|| g.clone());
    }
    g
}

pub fn definir(g: Gabarit) {
    if let Ok(mut a) = ACTIF.write() {
        *a = Some(Arc::new(g));
    }
}

/// Texte du gabarit par défaut (base d'un gabarit d'agence).
pub fn texte_defaut() -> &'static str {
    DEFAUT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gabarit_par_defaut_complet() {
        let g = Gabarit::defaut();
        for s in SorteDalle::TOUTES {
            assert!(!g.surfaces(s).is_empty(), "{s:?}");
        }
        assert!(!g.types_murs.is_empty());
        assert!(!g.niveaux.is_empty());
        assert_eq!(g.materiau("Laine minérale").motif, MotifCoupe::Croisees);
        assert_eq!(g.materiau("Béton armé").couleur, [176, 176, 170]);
    }

    #[test]
    fn gabarit_refuse_un_autre_format() {
        assert!(Gabarit::lire(r#"{"format":"x","version":1,"nom":"x"}"#).is_err());
        let sans_vues = DEFAUT.replace("\"elevation\": true", "\"elevation\": false");
        assert!(Gabarit::lire(&sans_vues).is_err());
    }
}
