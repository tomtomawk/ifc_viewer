//! Commandes d'édition sérialisables pour l'interface et le pilote MCP.
use modele::*;
use modele::elements::site;
use serde::Deserialize;
#[derive(Deserialize, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Commande {
    Terrain { terrain: site::Terrain },
    LimitePropriete { limite: site::LimitePropriete },
    /// Lien IFC : création (identifiant vide) ou remplacement (position, visibilité,
    /// chemin). Sa suppression passe par `Supprimer`.
    Lien { lien: liens::LienIfc },
    /// Fusionne un projet importé (IFC converti) dans le projet ouvert : niveaux
    /// rapprochés, types, matériaux et profils identiques réutilisés, identifiants
    /// renouvelés, éléments décalés de `decalage` (mm, origine sur origine par défaut).
    Fusionner {
        projet: Box<Projet>,
        #[serde(default)]
        decalage: [f64; 3],
    },
    /// Coordonnées de l'origine du projet : E/O, N/S, cote NGF (mm).
    CoordonneesOrigine { coordonnees: [f64; 3] },
    Barre {
        barre: Barre,
    },
    /// Crée (identifiant vide : numéro suivant, contour détecté depuis les murs ; refusée
    /// si l'espace n'est pas fermé) ou modifie une pièce.
    Piece {
        piece: Piece,
    },
    /// Crée (identifiant vide ; type vide : type par défaut) ou modifie un composant.
    Composant {
        composant: Composant,
    },
    /// Crée (identifiant vide) ou remplace un type de composant.
    TypeComposant {
        type_composant: TypeComposant,
    },
    /// Crée (identifiant vide) ou remplace un escalier.
    Escalier {
        escalier: Escalier,
    },
    /// Crée (identifiant vide) ou remplace un garde-corps.
    GardeCorps {
        garde_corps: GardeCorps,
    },
    ModifierVue {
        vue: Vue,
    },
    ModifierQuadrillage {
        quadrillage: documentation::Quadrillage,
    },
    Niveau {
        nom: String,
        altitude: f64,
    },
    Quadrillage {
        nom: String,
        debut: [f64; 2],
        fin: [f64; 2],
    },
    Annoter {
        annotation: documentation::Annotation,
    },
    Feuille {
        feuille: documentation::Feuille,
    },
    Nomenclature {
        nomenclature: documentation::Nomenclature,
    },
    ModifierNiveau {
        id: String,
        nom: String,
        altitude: f64,
    },
    Ouverture {
        mur: String,
        genre: GenreOuverture,
        type_ouverture: Option<String>,
        abscisse: f64,
        largeur: f64,
        hauteur: f64,
        allege: f64,
    },
    Deplacer {
        ids: Vec<String>,
        delta: [f64; 3],
    },
    CopierSurNiveau { ids:Vec<String>, niveau:String },
    Copier {
        ids: Vec<String>,
        delta: [f64; 3],
    },
    Tourner {
        ids: Vec<String>,
        centre: [f64; 2],
        angle: f64,
    },
    Aligner {
        ids: Vec<String>,
        reference: String,
    },
    /// Prolonge (ou raccourcit) la ligne de justification du mur `id` jusqu'à `ligne`
    /// (une arête du mur `reference`) ; sans `ligne`, jusqu'à la ligne de justification
    /// de `reference`.
    Ajuster {
        id: String,
        reference: String,
        fin: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ligne: Option<[[f64; 2]; 2]>,
    },
    /// Raccorde les lignes de justification des deux murs à leur intersection, en une
    /// seule transaction.
    AjusterAngle { id: String, reference: String, fin: bool, fin_reference: bool },
    ModifierTrajectoireMur {id:String,points:[[f64;2];3]},
    InverserMur { id: String },
    ActualiserType {
        assemblage: String,
    },
    Attacher {
        ids: Vec<String>,
        toiture: Option<String>,
    },
    /// Profil d'élévation d'un mur indépendant ou d'un assemblage (une couche désigne
    /// son assemblage) ; None : rectangle d'origine. Le mur quitte ses toitures.
    ProfilMur {
        id: String,
        profil: Option<modele::murs::profil::ProfilMur>,
    },
    PrioriteJonction {
        id: String,
        priorite: i32,
    },
    Supprimer {
        ids: Vec<String>,
    },
    /// Symétrie par rapport à la droite (a, b) ; la face extérieure des murs reste dehors.
    Symetrie {
        ids: Vec<String>,
        a: [f64; 2],
        b: [f64; 2],
    },
    /// Mise à l'échelle autour de `centre` (annotations, contours, terrains).
    Echelle {
        ids: Vec<String>,
        centre: [f64; 2],
        facteur: f64,
    },
    /// Réseau de `nombre` éléments (original compris) : copie k tournée de k·angle autour
    /// de `centre` puis déplacée de k·delta.
    Reseau {
        ids: Vec<String>,
        nombre: usize,
        delta: [f64; 3],
        centre: [f64; 2],
        angle: f64,
    },
    /// Décale un mur droit ou une ligne de `distance` (positive : à gauche du sens).
    Decaler {
        id: String,
        distance: f64,
        copie: bool,
    },
    /// Scinde un mur droit au point projeté, avec un écart éventuel.
    Scinder {
        id: String,
        point: [f64; 2],
        ecart: f64,
    },
    /// Découpe une surface le long de la polyligne dessinée en plan (mm).
    ScinderSurface { id: String, trace: Vec<[f64; 2]> },
    /// Démolit les éléments dans `phase` (absente : annule la démolition).
    Demolir {
        ids: Vec<String>,
        phase: Option<String>,
    },
    /// Reproduit le type et les paramètres de `source` sur `cibles` (même catégorie).
    Reproduire {
        source: String,
        cibles: Vec<String>,
    },
    /// Place l'ouverture `id` sur le mur `mur`, centrée sur la projection de `point`.
    ChangerHote {
        id: String,
        mur: String,
        point: [f64; 2],
    },
    /// Inverse le côté d'ouverture d'une menuiserie, ou le sens de ses paumelles.
    InverserOuverture {
        id: String,
        paumelles: bool,
    },
    /// Verrouille (`verrou` vrai) ou déverrouille les éléments.
    Verrouiller {
        ids: Vec<String>,
        verrou: bool,
    },
    Contour {
        id: String,
        contours: Vec<Vec<[f64; 2]>>,
    },
    Tremie {
        id: String,
        contour: Vec<[f64; 2]>,
    },
}
