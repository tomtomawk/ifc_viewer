//! Gabarit de projet, comme un gabarit Revit : matériaux, types de murs, d'éléments
//! horizontaux et de vues proposés à la création. Le gabarit par défaut est embarqué
//! (`gabarits/defaut.gabarit.json`) ; un autre fichier peut être choisi dans
//! Début > Options.

use crate::{Couche, DefinitionMotif, Materiau, MotifCoupe, SorteDalle};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, RwLock};

pub const FORMAT: &str = "bimedit.gabarit";
const DEFAUT: &str = include_str!("../../../gabarits/defaut.gabarit.json");

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Gabarit {
    pub format: String,
    pub version: u32,
    pub nom: String,
    /// Niveaux d'un nouveau projet.
    #[serde(default)]
    pub niveaux: Vec<NiveauGabarit>,
    /// Épaisseurs des plumes 1 à 12 (mm du papier).
    #[serde(default)]
    pub plumes: Vec<f64>,
    /// Motifs de hachures proposés (en plus des motifs de base).
    #[serde(default)]
    pub motifs: Vec<DefinitionMotif>,
    /// Matériaux reconnus par un mot de leur nom (premier qui correspond).
    #[serde(default)]
    pub materiaux: Vec<FamilleMateriau>,
    /// Styles d'objets d'un nouveau projet, par catégorie d'élément.
    #[serde(default)]
    pub styles_categories: std::collections::BTreeMap<crate::CategorieMateriau, crate::StyleCategorie>,
    /// Types de murs proposés en plus de ceux du projet.
    #[serde(default)]
    pub types_murs: Vec<TypeGabarit>,
    #[serde(default)]
    pub types_surfaces: Vec<TypeSurface>,
    #[serde(default)]
    pub types_vues: Vec<TypeVue>,
    /// Types de trait du dessin 2D d'un nouveau projet.
    #[serde(default)]
    pub types_traits: Vec<crate::documentation::TypeTrait>,
    /// Types d'ouvertures chargés dans un nouveau projet, avec leur famille modélisée.
    #[serde(default)]
    pub types_ouvertures: Vec<TypeOuvertureGabarit>,
    /// Profils paramétriques chargés dans les nouveaux projets.
    #[serde(default)]
    pub profils: Vec<crate::famille::profil::Profil>,
    /// Types de poteaux et de poutres d'un nouveau projet ; `profil` : nom d'un profil
    /// du gabarit (rectangle sans profil).
    #[serde(default)]
    pub types_poteaux: Vec<crate::elements::poutre::TypeSection>,
    #[serde(default)]
    pub types_poutres: Vec<crate::elements::poutre::TypeSection>,
    /// Types de terrain (épaisseur, matériau) proposés au sélecteur des terrains.
    #[serde(default)]
    pub types_terrains: Vec<crate::elements::site::TypeTerrain>,
}

/// Type d'ouverture du gabarit : nom, cotes et famille.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypeOuvertureGabarit {
    pub nom: String,
    pub largeur: f64,
    pub hauteur: f64,
    pub famille: crate::famille::modelee::FamilleModelee,
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
    /// Apparence (style Réaliste) : brins d'herbe, textures (chemins `gabarit:textures/…` :
    /// textures livrées avec le logiciel), découpe du feuillage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apparence: Option<crate::Apparence>,
}

/// Préfixe des textures livrées avec le gabarit (dossier `gabarits/textures`).
pub const PREFIXE_TEXTURE: &str = "gabarit:";

/// Type composé : couches (murs : de l'extérieur vers l'intérieur).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypeGabarit {
    pub nom: String,
    pub couches: Vec<Couche>,
    /// Catégorie d'un type de mur ; absente : selon le nombre de couches.
    pub categorie: Option<crate::CategorieMur>,
}

impl TypeGabarit {
    pub fn categorie_mur(&self) -> crate::CategorieMur {
        self.categorie.unwrap_or(if self.couches.len() > 1 { crate::CategorieMur::Compose } else { crate::CategorieMur::Mur })
    }
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
    /// Famille du repère des vues de ce type dans les plans (identifiant d'une famille
    /// `repere_coupe` ou `repere_elevation` du projet) ; absente : celle du projet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repere: Option<String>,
}

impl TypeVue {
    /// Types de vues du code (projet sans types de vues) : une coupe et une élévation.
    pub fn par_defaut() -> Vec<TypeVue> {
        vec![
            TypeVue { elevation: false, nom: "Coupe".into(), aide: "Coupe générale".into(), echelle: 100.0, profondeur: 15_000.0, repere: None },
            TypeVue { elevation: true, nom: "Élévation".into(), aide: "Façade".into(), echelle: 100.0, profondeur: 30_000.0, repere: None },
        ]
    }
}

impl crate::Projet {
    /// Types de coupe et d'élévation du projet, sinon ceux du code.
    pub fn types_de_vues(&self) -> Vec<TypeVue> {
        if self.types_vues.is_empty() { TypeVue::par_defaut() } else { self.types_vues.clone() }
    }
}

impl Gabarit {
    #[allow(clippy::expect_used, reason = "gabarit embarqué à la compilation, relu par le test gabarit_par_defaut_complet")]
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
        for profil in &g.profils { profil.valider()?; }
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
        materiau_d_apres(&self.materiaux, nom)
    }
}

/// Matériau nommé `nom` d'après la première famille dont un mot figure dans le nom ;
/// aucune : couleur claire tirée du nom, hachures diagonales (défaut du code).
pub fn materiau_d_apres(familles: &[FamilleMateriau], nom: &str) -> Materiau {
    {
        let n = nom.to_lowercase();
        match familles.iter().find(|f| f.mots.iter().any(|m| n.contains(&m.to_lowercase()))) {
            Some(f) => Materiau {
                id: noyau::uuid::uuid(),
                nom: nom.into(),
                couleur: f.couleur,
                motif: f.motif.clone(),
                coupe_couleur: f.coupe_couleur,
                coupe_fond: f.coupe_fond,
                coupe_fond_motif: f.coupe_fond_motif.clone(),
                surface_fond_motif: f.surface_fond_motif.clone(),
                surface_motif: f.surface_motif.clone(),
                surface_couleur: f.surface_couleur,
                apparence: f.apparence.clone().unwrap_or_default(),
            },
            None => {
                // couleur assez claire pour les hachures
                let h = crate::identifiant("materiau", nom);
                let v = u32::from_str_radix(&h[..6], 16).unwrap_or(0);
                let couleur = [150 + (v & 0x5f) as u8, 150 + ((v >> 8) & 0x5f) as u8, 150 + ((v >> 16) & 0x5f) as u8];
                Materiau { id: noyau::uuid::uuid(), nom: nom.into(), couleur, motif: MotifCoupe::Diagonales, ..Default::default() }
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

/// Familles d'ouvertures livrées dans le gabarit par défaut (source de sa section
/// `types_ouvertures`).
#[cfg_attr(not(test), allow(dead_code))] // source du gabarit, lue par les tests
pub fn familles_livrees() -> Vec<TypeOuvertureGabarit> {
    use crate::famille::modelee::FamilleModelee;
    vec![
        TypeOuvertureGabarit { nom: "Fenêtre OF 2 vantaux".into(), largeur: 1200.0, hauteur: 1350.0, famille: FamilleModelee::fenetre_of_2_vantaux() },
        TypeOuvertureGabarit { nom: "Porte intérieure 1 vantail".into(), largeur: 900.0, hauteur: 2150.0, famille: FamilleModelee::porte_1_vantail() },
        TypeOuvertureGabarit { nom: "Porte-fenêtre 2 vantaux, tableaux et seuil".into(), largeur: 1400.0, hauteur: 2150.0, famille: FamilleModelee::porte_fenetre_tableaux_seuil() },
    ]
}

/// Texte du gabarit par défaut (base d'un gabarit d'agence).
pub fn texte_defaut() -> &'static str {
    DEFAUT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn herbe_et_feuillage_du_gabarit() {
        let g = Gabarit::defaut();
        // terrain naturel et pelouse : brins d'herbe ; feuillage découpé ; textures livrées
        assert!(g.materiau("Terre naturelle").apparence.herbe > 0.0 && g.materiau("Pelouse").apparence.herbe > 0.0);
        let f = g.materiau("Feuillage");
        assert!(f.apparence.textures.contains_key(&crate::CarteTexture::Decoupe));
        assert!(f.apparence.textures.values().chain(g.materiau("Écorce").apparence.textures.values()).all(|t| t.chemin.starts_with(PREFIXE_TEXTURE)));
        assert_eq!(g.materiau("Béton").apparence.herbe, 0.0);
    }

    #[test]
    fn gabarit_par_defaut_complet() {
        let g = Gabarit::defaut();
        for s in SorteDalle::TOUTES {
            assert!(!g.surfaces(s).is_empty(), "{s:?}");
        }
        assert!(!g.types_murs.is_empty());
        // Au 1:100, 13 mm de placo donnent 0,13 mm sur papier : les contours
        // de finition et de structure doivent laisser un espace entre eux.
        assert!((g.plumes[1] + g.plumes[4]) / 2.0 < 13.0 / 100.0);
        // types de trait du gabarit : ceux du code, valides
        assert_eq!(g.types_traits, crate::documentation::types_traits_standard());
        g.types_traits.iter().try_for_each(|t| t.valider()).unwrap();
        assert!(!g.niveaux.is_empty());
        assert_eq!(g.materiau("Laine minérale").motif, MotifCoupe::Isolation);
        assert!(g.motifs.iter().any(|m| m.nom == "Béton"));
        assert_eq!(g.materiau("Béton armé").couleur, [176, 176, 176]);
        assert_eq!(g.materiau("Béton armé").motif, MotifCoupe::Uni);
    }

    #[test]
    fn gabarit_refuse_un_autre_format() {
        assert!(Gabarit::lire(r#"{"format":"x","version":1,"nom":"x"}"#).is_err());
        let sans_vues = DEFAUT.replace("\"elevation\": true", "\"elevation\": false");
        assert!(Gabarit::lire(&sans_vues).is_err());
    }

    /// G1 : la création copie du gabarit plumes, motifs, familles de matériaux et types
    /// de vues ; un projet qui ne les a pas prend les défauts du code, jamais le gabarit.
    #[test]
    fn projet_porte_ses_donnees_sans_relire_le_gabarit() {
        let g = Gabarit::defaut();
        let p = crate::Projet::depuis_gabarit("G1", &g);
        assert_eq!(p.plumes, g.plumes);
        assert_eq!(p.motifs.len(), g.motifs.len());
        assert_eq!(p.familles_materiaux.len(), g.materiaux.len());
        assert_eq!(p.types_de_vues().len(), g.types_vues.len());
        let beton = g.materiaux.iter().flat_map(|f| f.mots.iter()).next().expect("une famille de matériau").clone();
        assert_eq!(p.materiau_par_defaut(&beton).couleur, g.materiau(&beton).couleur);
        // projet sans ces données : défauts du code
        let vide = crate::Projet::nouveau("vide");
        assert_eq!(vide.epaisseurs_plumes(), crate::PLUMES.to_vec());
        assert_eq!(vide.types_de_vues().len(), 2);
        assert_eq!(vide.materiau_par_defaut(&beton).motif, crate::MotifCoupe::Diagonales);
        let motif_gabarit = g.motifs.iter().find(|m| !crate::DefinitionMotif::base().iter().any(|b| b.nom == m.nom)).expect("un motif propre au gabarit");
        assert!(vide.definition_motif(&crate::MotifCoupe::hachures(&motif_gabarit.nom)).is_none());
        assert!(p.definition_motif(&crate::MotifCoupe::hachures(&motif_gabarit.nom)).is_some());
    }
}
