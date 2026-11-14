//! Éléments lus directement dans les partitions d'un RVT, et leur traduction en scène du
//! viewer : murs, sols, toits, escaliers, grilles, matériaux, familles, vues, paramètres…
//!
//! Les objets courants sont ceux des ids de `Global/ElemTable`, une partition plus
//! récente remplaçant une plus ancienne. Comme les collecteurs de Revit, les éléments
//! d'un groupe non placé (`Element.m_unplacedOwnerId` ≠ −1) sont ignorés.
//!
//! La géométrie de tous les éléments est le solide calculé par Revit (`GElement`,
//! section 0x67), maillé par [`geometrie::maillage`] ; la couleur d'une face est celle
//! de son matériau (`GFace.m_renderStyleId` = id d'un `MaterialElem`). Les familles
//! (`GInstance`) prennent la géométrie de leur type, placée par leur repère et filtrée
//! comme dans une vue 3D en détail fin ([`geometrie::Bibliotheque`]).

mod lecture;
pub use lecture::*;
mod vers_scene;
pub use vers_scene::*;
#[cfg(test)]
mod tests;

use super::champs::{self, Obj, Record, Val};
use super::coordonnees::{self, Coordonnees};
use super::geometrie::{self, FT, Maillage};
use super::objets::{document_de, elem_table, episodes, inflate, records, sections_documents, unpad};
use super::parametres::{self, Brut, Definition};
use super::schema::Schema;
use super::feuilles;
use super::vues::{self, Cadre, Contenu, Element2D, TypeFenetre, TypeRegion, Vue};
use super::Cfb;
use crate::formats::ifc::geom::{Ligne, Part, ProductGeom, Rgba, class_color};
use ifc_base::semantique::IfcModel;
use scene::{NavFamille, NavVue, Navigateur, Scene};
use ifc_base::step::{Entity, Id, IdMap, StepFile, Value, format_real};
use noyau::trace2d::{self, Primitive, Role, Style};
use noyau::motif::Motif;
use glam::{DMat3, DVec2, DVec3};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

// Ids de paramètres (BuiltInParameter), relevés dans l'API de Revit 2026.
const WALL_USER_HEIGHT_PARAM: i64 = -1_001_105;
const WALL_BASE_OFFSET: i64 = -1_001_108;
const WALL_TOP_OFFSET: i64 = -1_001_109;
const FLOOR_HEIGHTABOVELEVEL_PARAM: i64 = -1_001_951;
const CEILING_HEIGHTABOVELEVEL_PARAM: i64 = -1_002_300;
const ROOF_LEVEL_OFFSET_PARAM: i64 = -1_001_701;
const TOPOSOLID_HEIGHTABOVELEVEL_PARAM: i64 = -1_155_255;
const ALL_MODEL_MARK: i64 = -1_001_203;
const FAMILY_WIDTH_PARAM: i64 = -1_001_301;
const FAMILY_HEIGHT_PARAM: i64 = -1_001_300;
const ROOM_NAME: i64 = -1_006_900;
const ROOM_NUMBER: i64 = -1_006_901;
const ROOM_DEPARTMENT: i64 = -1_006_907;

/// Catégories de familles (BuiltInCategory, noms de l'interface française de Revit) et
/// classe IFC de leurs instances dans la scène.
const CATEGORIES: &[(i64, &str, &str)] = &[
    (-2_000_023, "Portes", "IFCDOOR"),
    (-2_000_014, "Fenêtres", "IFCWINDOW"),
    (-2_000_080, "Mobilier", "IFCFURNITURE"),
    (-2_000_100, "Poteaux", "IFCCOLUMN"),
    (-2_001_320, "Ossature", "IFCBEAM"),
    (-2_000_127, "Barreaux", "IFCRAILING"),
    (-2_000_170, "Panneaux de murs-rideaux", "IFCPLATE"),
    (-2_001_160, "Appareils sanitaires", "IFCSANITARYTERMINAL"),
    (-2_000_151, "Modèles génériques", "IFCBUILDINGELEMENTPROXY"),
    (-2_001_350, "Equipement spécialisé", "IFCBUILDINGELEMENTPROXY"),
    (-2_001_360, "Plantes", "IFCBUILDINGELEMENTPROXY"),
    (-2_001_180, "Parking", "IFCBUILDINGELEMENTPROXY"),
    (-2_001_040, "Tableaux/Baies", "IFCBUILDINGELEMENTPROXY"),
];

/// Sous-catégories prédéfinies des formes de famille (BuiltInCategory), noms relevés
/// dans Revit 2026 sur `FEN_2PF.rfa` ; les sous-catégories créées par l'utilisateur
/// sont des `CategoryElem` du document.
const SOUS_CATEGORIES: &[(i64, &str)] = &[(-2_000_016, "Verre"), (-2_000_018, "Cadre/Meneau"), (-2_000_020, "Appui/Linteau")];

/// Nom français d'une classe de forme de famille.
fn nom_forme(classe: &str) -> &str {
    match classe {
        "ExtrusionElem" => "Extrusion",
        "BlendElem" => "Raccord",
        "RevolutionElem" => "Révolution",
        "SweepElem" => "Balayage",
        "SweptBlendElem" => "Raccord balayé",
        "FamilyGeomCombination" => "Combinaison",
        "FreeFormElement" => "Forme libre",
        "FormElem" => "Forme",
        c => c,
    }
}

/// Maillage du `GElement`, en mm, ou raison de son absence.
pub type Geometrie = Result<Maillage, String>;

pub struct Niveau {
    pub id: i64,
    pub nom: String,
    /// Altitude en mm.
    pub altitude: f64,
    /// Type de niveau (`m_attrId`, `LevelAttributes`).
    pub type_id: i64,
}

/// Type d'élément (tout `Symbol` : types de murs, de sols, de toits, d'escaliers,
/// types de familles…).
pub struct TypeElem {
    pub nom: String,
    /// Classe Revit du type (`BasicWallType`, `FloorAttributes`, `FamilySymbol`…).
    pub classe: String,
    /// Somme des couches de la structure composée, en mm (types de murs, sols et toits ;
    /// absente pour les murs-rideaux et les murs empilés).
    pub epaisseur: Option<f64>,
    /// Couches dans l'ordre de la structure composée Revit (extérieur → intérieur
    /// pour les murs, dessus → dessous pour les sols). None : lecture incomplète.
    pub couches: Option<Vec<CoucheType>>,
    pub couches_variables: bool,
    /// Famille d'un type de famille (`FamilySymbol.m_familyId`), −1 sinon.
    pub famille: i64,
    /// Largeur et hauteur en mm (paramètres de type `FAMILY_WIDTH_PARAM` et
    /// `FAMILY_HEIGHT_PARAM`, lus dans `FamilySymbol.m_pParams`) : pour une porte ou une
    /// fenêtre, les cotes du panneau, pas l'encombrement de la famille (huisserie).
    pub largeur: Option<f64>,
    pub hauteur: Option<f64>,
    /// Sous-catégorie d'un type de texte ou d'étiquette (`m_lineAndTextAttr.m_categoryId`),
    /// qui porte sa couleur ; −1 sinon.
    pub categorie_texte: i64,
    /// Type de groupe : origine d'insertion (`ElementGroupType.m_insertionOrigin`, mm).
    pub origine_insertion: Option<DVec3>,
}

#[derive(Clone, Debug)]
pub struct CoucheType {
    pub epaisseur: f64,
    /// ElementId du matériau, −1 : par catégorie.
    pub materiau: i64,
}

fn couches_structure(cs: &Obj<'_>) -> Option<Vec<CoucheType>> {
    let Val::List(liste) = cs.get("m_layers")? else { return None };
    if liste.is_empty() { return None; }
    liste.iter().map(|v| {
        let c = v.obj()?;
        let epaisseur = c.get("m_layerWidth")?.real()? * FT;
        if !epaisseur.is_finite() || epaisseur < 0.0 { return None; }
        Some(CoucheType { epaisseur, materiau: c.get("m_materialId")?.element_id()? })
    }).collect()
}

pub struct Famille {
    pub nom: String,
    /// Catégorie (BuiltInCategory).
    pub categorie: i64,
    /// Famille hôte d'une famille imbriquée (`Element.m_famId`), −1 pour la famille d'un
    /// document `.rfa` ou une famille chargée dans un projet.
    pub parent: i64,
    /// Paramètres d'occurrence de la famille (`Family.m_familyParams`, `m_instance` = 1),
    /// intégrés compris : leur valeur est portée par chaque instance, pas par le type.
    pub occurrence: std::collections::HashSet<i64>,
    /// Document embarqué d'une famille chargée (`m_oFamDoc.m_contentDocGUID`).
    pub document: Option<[u8; 16]>,
    /// Paramètres à formule (`NamedParam.m_oExpression`) : Revit enregistre leur valeur
    /// calculée dans chaque type.
    pub formules: Vec<(i64, super::formules::Expr)>,
}

/// Instance de famille propre à une vue (cartouche ou symbole d'une feuille, composant
/// de détail, annotation) : type, vue, repère `m_pInstanceInfo.m_Trf` (comme
/// `GetTransform` de l'API : origine en mm, axes en colonnes, échelle de la vue comprise
/// pour une annotation).
pub struct InstanceVue {
    pub id: i64,
    pub vue: i64,
    pub type_id: i64,
    pub origine: DVec3,
    pub axes: DMat3,
}

/// Révision du projet (`ProjectRevision`).
pub struct Revision {
    pub description: String,
    pub date: String,
    pub numero: String,
    pub sequence: i64,
    pub remise_a: String,
    pub remise_par: String,
}

/// Police d'un type de texte ou d'étiquette (`FontElem.m_pFont`, propriétaire
/// `m_ownerId`) : nom et taille (mm, hauteur des capitales dans Revit).
pub struct Police {
    pub nom: String,
    pub taille: f64,
}

/// Forme d'un document de famille (`.rfa`) : extrusion, raccord, révolution, balayage…
/// (classes dérivées de `GenSweep`, pleines), ou combinaison de formes
/// (`FamilyGeomCombination`) dont le `GElement` porte le solide résultant. Dans Revit,
/// une forme membre d'une combinaison n'a pas de géométrie propre et un vide n'en a
/// pas non plus : ni l'une ni l'autre n'est gardée.
pub struct Forme {
    pub id: i64,
    pub classe: String,
    /// Sous-catégorie (`GenSweep.m_categoryId` : BuiltInCategory ou id d'un
    /// `CategoryElem`), −1 pour une combinaison.
    pub sous_categorie: i64,
    /// Matériau imposé (`GenSweep.m_materialId`), −1 : par catégorie.
    pub materiau: i64,
    /// Famille propriétaire (`Element.m_famId`).
    pub famille: i64,
    pub niveau: i64,
    /// Combinaison : forme pleine puis vides (`GeomCombination.m_svMap`).
    pub membres: Vec<i64>,
    pub geometrie: Geometrie,
}

pub struct Materiau {
    pub nom: String,
    /// Couleur d'ombrage (rouge, vert, bleu).
    pub couleur: [u8; 3],
    /// Apparence de rendu (`m_appearanceAssetId`, `AppearanceAssetElem`), −1 : aucune.
    pub apparence: i64,
    /// 0 opaque, 1 transparent (Revit affiche ×100).
    pub transparence: f64,
    /// Motifs de coupe et de surface, premier plan puis arrière-plan (ids de
    /// `FillPatternElem`, −1 : aucun), avec leurs couleurs.
    pub motifs: [(i64, [u8; 3]); 4],
}

/// Courbe de positionnement (`GLine`, `GArc`) en mm, dans l'espace de paramètres de
/// Revit (celui de `m_endParams` et de `InsertableInst.m_hostParam`).
#[derive(Clone, Copy, Debug)]
pub enum Trace {
    Ligne { origine: DVec3, direction: DVec3 },
    Arc { centre: DVec3, rayon: f64, x: DVec3, y: DVec3 },
}

pub struct Mur {
    pub id: i64,
    /// `SWall` (droit) ou `ArcWall` (courbe).
    pub classe: String,
    pub type_id: i64,
    pub niveau: i64,
    /// −1 : hauteur non contrainte.
    pub niveau_haut: i64,
    /// Hauteur en mm : celle que donne Revit, calculée depuis les niveaux si le mur est
    /// contraint en haut.
    pub hauteur: f64,
    pub decalage_bas: f64,
    pub decalage_haut: f64,
    pub trace: Option<Trace>,
    pub geometrie: Geometrie,
}

/// Sol (`Floor`), plafond (`Ceiling`) ou toposolide (`Toposolid`, terrain depuis 2024).
pub struct Dalle {
    pub id: i64,
    pub classe: String,
    pub type_id: i64,
    pub niveau: i64,
    /// Hauteur par rapport au niveau, en mm.
    pub decalage: f64,
    pub geometrie: Geometrie,
}

/// Surface topographique (`SiteSurface`, `TopographySurface` de l'API : le terrain
/// d'avant les toposolides de 2024) ou sous-région (`m_hostId`, −1 sans hôte) ;
/// géométrie : maillage à facettes (`GPolyMesh`) du `GElement`.
pub struct SurfaceTopo {
    pub id: i64,
    pub hote: i64,
    /// Points saisis (`GetPoints` de l'API).
    pub points: usize,
    pub geometrie: Geometrie,
}

/// Toit (classes dérivées de `RoofBase` : `ProfileRoof` pour un toit par tracé…).
pub struct Toit {
    pub id: i64,
    pub classe: String,
    pub type_id: i64,
    pub niveau: i64,
    /// Décalage par rapport au niveau, en mm.
    pub decalage: f64,
    pub geometrie: Geometrie,
}

pub struct Escalier {
    pub id: i64,
    pub type_id: i64,
    pub niveau_bas: i64,
    pub niveau_haut: i64,
    pub contremarches: i64,
    /// Hauteur de contremarche et giron réels, en mm.
    pub hauteur_contremarche: f64,
    pub giron: f64,
    /// Hauteur totale en mm, calculée depuis les niveaux et leurs décalages.
    pub hauteur: f64,
    /// Volées et paliers (`StairsRunAndLanding`).
    pub parties: Vec<i64>,
    /// Solides des volées et paliers réunis.
    pub geometrie: Geometrie,
}

/// Grille (quadrillage) : trace au sol de son plan (`DatumPlane.m_pSurface`, `Plane`
/// pour une grille droite, `CylSurf` pour une grille courbe) entre les bornes en u de
/// son enveloppe.
pub struct Grille {
    pub id: i64,
    pub nom: String,
    /// Extrémités en mm.
    pub debut: DVec3,
    pub fin: DVec3,
    /// Centre et rayon (mm) d'une grille courbe.
    pub arc: Option<(DVec3, f64)>,
}

/// Instance de famille placée dans le modèle (les éléments propres à une vue,
/// composants de détail, cartouches…, sont ignorés).
pub struct Instance {
    pub id: i64,
    /// Type de famille (`m_masterSymbolId`).
    pub type_id: i64,
    pub hote: i64,
    pub niveau: i64,
    /// Point d'insertion en mm (comme `LocationPoint` dans l'API) : pour une instance
    /// hébergée par un mur, point de la ligne du mur au paramètre `m_hostParam`, à
    /// l'altitude du niveau plus `m_elevation` ; sinon `m_instOrigin`.
    pub point: DVec3,
    /// Repère (paramètre ALL_MODEL_MARK, ou numéro de porte).
    pub repere: String,
    /// Axes X et Y du repère de l'instance (`m_pInstanceInfo.m_Trf`, en colonnes,
    /// identiques à `GetTransform`).
    pub axes: Option<DMat3>,
    /// Pièces de départ et d'arrivée inversées (`m_flippedFromToRoom`).
    pub pieces_inversees: bool,
    /// Main retournée (`m_flippedX`, HandFlipped de l'API).
    pub main_retournee: bool,
    pub geometrie: Geometrie,
}

/// Garde-corps (classes dérivées de `BaseRailing`) : main courante et barreaux, dans
/// son `GElement` et les types qu'il place (`GInstance`).
pub struct GardeCorps {
    pub id: i64,
    /// Type de garde-corps (`m_stairsRailingAttrId`).
    pub type_id: i64,
    pub niveau: i64,
    pub geometrie: Geometrie,
}

/// Pièce, surface ou zone (`RoomElem`).
pub struct Piece {
    pub id: i64,
    /// "piece", "surface" (`m_areaSchemeId`) ou "zone" (`m_zoneSchemeId`).
    pub genre: &'static str,
    pub nom: String,
    pub numero: String,
    pub service: String,
    /// Phase de la pièce (`m_phaseId` ; `m_createdPhaseId` vaut −1).
    pub phase: i64,
    /// Schéma d'une zone de surface (`m_areaSchemeId`), −1 pour une pièce.
    pub schema: i64,
    /// Circuit de la pièce dans la topologie de son niveau (`m_cachedCircuitId`).
    pub circuit: i64,
    pub niveau: i64,
    /// Point de placement en mm (x, y de `m_point`, z à l'altitude du niveau), None si
    /// la pièce n'est pas placée.
    pub point: Option<DVec3>,
    /// Volume de la pièce.
    pub geometrie: Geometrie,
}

pub struct Maquette {
    pub niveaux: Vec<Niveau>,
    pub types: HashMap<i64, TypeElem>,
    pub familles: HashMap<i64, Famille>,
    pub materiaux: HashMap<i64, Materiau>,
    pub murs: Vec<Mur>,
    pub dalles: Vec<Dalle>,
    /// Toposolides (terrain) : type `m_TypeId`, niveau (souvent −1 : toposolide issu
    /// d'une subdivision ou d'un site sans niveau), décalage
    /// `TOPOSOLID_HEIGHTABOVELEVEL_PARAM`.
    pub toposolides: Vec<Dalle>,
    /// Surfaces topographiques et sous-régions (`SiteSurface`, fichiers d'avant 2024).
    pub surfaces_topo: Vec<SurfaceTopo>,
    pub toits: Vec<Toit>,
    pub escaliers: Vec<Escalier>,
    pub garde_corps: Vec<GardeCorps>,
    pub grilles: Vec<Grille>,
    pub instances: Vec<Instance>,
    pub pieces: Vec<Piece>,
    /// Emprises des pièces de la dernière phase, pour situer un point.
    pub emprises: Vec<Emprise>,
    /// Plans de référence et plans de construction (`plans`).
    pub plans_reference: Vec<super::plans::PlanReference>,
    pub plans_construction: Vec<super::plans::PlanConstruction>,
    /// Apparences de rendu, par id.
    pub apparences: HashMap<i64, super::plans::Apparence>,
    /// Images : fichiers (par id de `ImageHolder`), types, occurrences (`medias`).
    pub rasters: HashMap<i64, super::medias::Raster>,
    pub types_images: Vec<super::medias::TypeImage>,
    pub images: Vec<super::medias::InstanceImage>,
    /// Imports CAO et liens Revit : types et occurrences.
    pub types_imports: Vec<super::medias::TypeImport>,
    pub imports: Vec<super::medias::InstanceLien>,
    pub types_liens: Vec<super::medias::TypeLien>,
    pub liens: Vec<super::medias::InstanceLien>,
    /// Objets annexes (`annexes`).
    pub groupes: Vec<super::annexes::Groupe>,
    /// Groupe de chaque élément membre lu.
    pub groupe_de: HashMap<i64, i64>,
    /// Options et jeux de variantes, par id ; option de chaque élément lu qui en a une.
    pub variantes: HashMap<i64, super::annexes::Variante>,
    pub variante_de: HashMap<i64, i64>,
    /// Liaisons des paramètres de projet et partagés : catégories, occurrence (sinon type).
    pub liaisons: HashMap<i64, (Vec<i64>, bool)>,
    pub legendes: Vec<super::annexes::ComposantLegende>,
    pub contraintes: Vec<super::annexes::Contrainte>,
    pub motifs_ligne: HashMap<i64, super::annexes::MotifLigne>,
    pub epaisseurs: Vec<super::annexes::TableEpaisseurs>,
    pub tables_cles: Vec<super::annexes::TableCles>,
    pub objets_nommes: Vec<super::annexes::ObjetNomme>,
    /// Formes et combinaisons d'un document de famille.
    pub formes: Vec<Forme>,
    /// Noms des sous-catégories définies dans le document (`CategoryElem`), lus pour
    /// un document de famille.
    pub categories: HashMap<i64, String>,
    /// Vues du document (hors familles imbriquées).
    pub vues: Vec<Vue>,
    /// Éléments 2D propres à une vue : traits, textes, cotes, régions, fenêtres de vue.
    pub elements_2d: Vec<Element2D>,
    /// Motifs de remplissage (hachures), par id de `FillPatternElem`.
    pub motifs: HashMap<i64, Motif>,
    /// Types de régions remplies (`FilledRegionAttributes`).
    pub types_regions: HashMap<i64, TypeRegion>,
    /// Couleur de projection de chaque catégorie ou sous-catégorie (`GStyleElem` de
    /// type 1 → `GStyle.m_color`).
    pub couleurs_categories: HashMap<i64, [u8; 3]>,
    /// Définitions des paramètres utilisateur (`ParamElem`), par id.
    pub definitions: HashMap<i64, Definition>,
    /// Valeurs des paramètres de chaque élément lu (jeux, listes et champs appris).
    pub parametres: HashMap<i64, Vec<(i64, Brut)>>,
    /// Noms des phases (`ProjectPhase`).
    pub phases: HashMap<i64, String>,
    /// Unités du projet (`UnitsElem`).
    pub unites: parametres::Unites,
    /// Catégorie (BuiltInCategory) et type de chaque élément lu, pour les nomenclatures.
    pub categorie_de: HashMap<i64, i64>,
    pub type_de_element: HashMap<i64, i64>,
    /// Phases dans l'ordre (`AllProjectPhases.m_phaseIds`).
    pub ordre_phases: Vec<i64>,
    /// Filtres de phase (`PhaseFilterElem`) : nom et présentation par état
    /// (`m_phaseStatusPresentation`, indice = `ElementOnPhaseStatus`).
    pub filtres_phase: HashMap<i64, (String, [u8; 7])>,
    /// Emplacements de projet, points de base et lieu géographique.
    pub coordonnees: Coordonnees,
    /// Famille d'un document `.rfa` (`Family` sans famille hôte), dont
    /// `m_familyParams` porte les valeurs du type courant.
    pub famille_document: Option<i64>,
    /// GUID des épisodes (`Global/History`) et index de l'épisode de création de chaque
    /// élément (`Global/ElemTable`, formats 2024 et suivants) : UniqueId et IfcGUID.
    pub episodes: Vec<[u8; 16]>,
    pub episode_de: HashMap<i64, u32>,
    /// Instances propres à une vue (cartouches, symboles, composants de détail).
    pub instances_de_vue: usize,
    pub instances_vue: Vec<InstanceVue>,
    /// Dessin 2D (traits, régions) des types placés dans une vue et des titres de vue.
    pub dessins: HashMap<i64, geometrie::Dessin>,
    /// Textes, étiquettes et régions remplies des documents de famille embarqués, par
    /// document.
    pub textes_familles: HashMap<[u8; 16], Vec<Element2D>>,
    /// Polices des types de texte et d'étiquette, par type.
    pub polices: HashMap<i64, Police>,
    /// Noms des paramètres des documents de famille embarqués (`ParamElemFamily`), qui
    /// commandent la visibilité de leurs textes.
    pub params_embarques: HashMap<i64, String>,
    /// Sous-catégorie (couleur) des types de texte et d'étiquette des documents de
    /// famille embarqués.
    pub categories_textes: HashMap<i64, i64>,
    /// Types de fenêtres de vue (`ViewportAttributes`).
    pub types_fenetres: HashMap<i64, TypeFenetre>,
    pub revisions: HashMap<i64, Revision>,
    /// Informations du projet (`ProjectInfo`) : leurs paramètres sont dans `parametres`.
    pub info_projet: i64,
    /// Types de cotes (`DimensionStyle`).
    pub types_cotes: HashMap<i64, vues::TypeCote>,
    /// Format des longueurs du projet (unité, précision), None : inconnu.
    pub format_longueurs: Option<(String, f64)>,
    /// Lignes d'esquisse (non tracées ; une cote qui s'y accroche est masquée).
    pub lignes_esquisse: std::collections::HashSet<i64>,
    /// Objets retenus mais illisibles (données d'add-ins…).
    pub illisibles: Vec<(i64, String)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Sorte {
    Mur,
    Niveau,
    Type,
    Famille,
    Materiau,
    Dalle,
    /// Toposolide (terrain).
    Toposolide,
    /// Surface topographique ou sous-région (`SiteSurface`).
    SurfaceTopo,
    Toit,
    Escalier,
    /// Volée ou palier d'escalier.
    Volee,
    GardeCorps,
    Grille,
    Instance,
    Piece,
    /// Forme ou combinaison d'un document de famille.
    Forme,
    /// Sous-catégorie (`CategoryElem`), pour un document de famille.
    Categorie,
    /// Vue (classes dérivées de `DBView`).
    Vue,
    /// Cadrage d'une vue (`Viewer`).
    Viewer,
    /// Élément 2D d'une vue (trait, texte, cote, fenêtre de vue).
    Annotation,
    /// Région remplie (2D, avec géométrie).
    Region,
    /// Motif de remplissage (`FillPatternElem`).
    Motif,
    /// Type de région remplie (`FilledRegionAttributes`).
    TypeRegion,
    /// Style graphique d'une catégorie (`GStyleElem`).
    Style,
    /// Définition de paramètre utilisateur (dérivées de `ParamElem`).
    Parametre,
    /// Phase (`ProjectPhase`).
    Phase,
    /// Coordonnées : emplacement (`GeoLocation`), lieu (`GeoSite`), point de base
    /// (`BasePoint`), emplacement actif (`ActiveGeoLocationTrackingElement`).
    Coordonnee,
    /// Police d'un type de texte (`FontElem`).
    Police,
    /// Type de fenêtre de vue (`ViewportAttributes`).
    TypeFenetre,
    /// Révision (`ProjectRevision`).
    Revision,
    /// Informations du projet (`ProjectInfo`).
    InfoProjet,
    /// Type de cote (`DimensionStyle`).
    TypeCote,
    /// Unités du projet (`UnitsElem`).
    Unites,
    /// Plan de référence (`RefPlane`), plan de construction (`SketchPlane`), apparence
    /// de rendu (`AppearanceAssetElem`).
    PlanReference,
    PlanConstruction,
    Apparence,
    /// Images, imports CAO, liens Revit (`medias`).
    Media,
    /// Objets annexes (`annexes` : groupes, légendes, contraintes, motifs de ligne…).
    Annexe,
    /// Profilé de mur (`WallSweep`) : géométrie propre, lu comme une forme.
    Profile,
    /// Topologie de plan d'un niveau (`LevelRoomPlan`).
    Topologie,
    /// Esquisse (dérivées de `Sketch`) : ses lignes ne sont pas tracées dans la vue.
    Esquisse,
}

impl Sorte {
    /// Éléments dont on lit le complément 0x67 (géométrie).
    fn a_geometrie(self) -> bool {
        matches!(self, Sorte::Mur | Sorte::Dalle | Sorte::Toposolide | Sorte::SurfaceTopo | Sorte::Toit | Sorte::Volee | Sorte::GardeCorps | Sorte::Instance | Sorte::Piece | Sorte::Forme | Sorte::Region | Sorte::Profile)
    }

    /// Sortes lues seulement dans un document de famille : dans un projet, les formes
    /// des familles in situ sont déjà dans la géométrie de leur instance.
    fn de_famille(self) -> bool {
        matches!(self, Sorte::Forme | Sorte::Categorie)
    }

    fn de(sch: &Schema, id: u16, name: &str) -> Option<Sorte> {
        let h = |base| sch.inherits(id, base);
        Some(match name {
            "SWall" | "ArcWall" => Sorte::Mur,
            // avant Symbol et Instance, dont GeoSite et GeoLocation dérivent
            "GeoLocation" | "GeoSite" | "BasePoint" | "ActiveGeoLocationTrackingElement" => Sorte::Coordonnee,
            "MaterialElem" => Sorte::Materiau,
            "StairsElement" => Sorte::Escalier,
            "Grid" => Sorte::Grille,
            "RoomElem" => Sorte::Piece,
            "SiteSurface" => Sorte::SurfaceTopo,
            "FontElem" => Sorte::Police,
            "ViewportAttributes" => Sorte::TypeFenetre,
            "ProjectRevision" => Sorte::Revision,
            "ProjectInfo" => Sorte::InfoProjet,
            "DimensionStyle" => Sorte::TypeCote,
            "UnitsElem" => Sorte::Unites,
            "LevelRoomPlan" => Sorte::Topologie,
            "RefPlane" => Sorte::PlanReference,
            "SketchPlane" => Sorte::PlanConstruction,
            "AppearanceAssetElem" => Sorte::Apparence,
            "ImageHolder" | "ImageSymbol" | "ImageInstance" | "MasterImportSymbol" | "ImportInstance" | "RvtLinkSymbol" | "RvtLinkInstance" => Sorte::Media,
            "WallSweep" => Sorte::Profile,
            // profil de plan de référence (plan d'esquisse d'un profil) : lu comme un plan
            "ProfileRefPlane" => Sorte::PlanReference,
            c if super::annexes::CLASSES.contains(&c) => Sorte::Annexe,

            // type de région remplie : un Symbol, lu à part (motifs et couleurs)
            "FilledRegionAttributes" => Sorte::TypeRegion,
            _ if h("Level") => Sorte::Niveau,
            _ if h("BaseRailing") => Sorte::GardeCorps,
            _ if h("Symbol") => Sorte::Type,
            _ if h("Family") => Sorte::Famille,
            _ if h("Floor") || h("Ceiling") => Sorte::Dalle,
            // dérive de CeilingAndFloor, pas de Floor : géométrie B-rep comme un sol
            _ if h("Toposolid") => Sorte::Toposolide,
            _ if h("RoofBase") => Sorte::Toit,
            _ if h("StairsRunAndLanding") => Sorte::Volee,
            _ if h("FamilyInstance") => Sorte::Instance,
            "CategoryElem" => Sorte::Categorie,
            // esquisses (`VarSketch`…) : leurs lignes ne sont pas tracées
            _ if h("Sketch") => Sorte::Esquisse,
            // marqueurs de vue (Section, Elevation, Viewer3d…) : cadrage de leur vue
            _ if h("Viewer") => Sorte::Viewer,
            "FilledRegion" => Sorte::Region,
            "FillPatternElem" => Sorte::Motif,
            "GStyleElem" => Sorte::Style,
            "ProjectPhase" | "AllProjectPhases" | "PhaseFilterElem" => Sorte::Phase,
            _ if h("ParamElem") => Sorte::Parametre,
            "CurveElem" | "TextNote" | "TagNote" | "LinearDimString" | "RadialDim" | "AngularDim" | "ArcLengthDim" | "Viewport" => Sorte::Annotation,
            _ if h("DBView") => Sorte::Vue,
            _ if h("GenSweep") || h("FamilyGeomCombination") => Sorte::Forme,
            _ => return None,
        })
    }
}

/// Mesure du décodage d'un fichier (`bimedit --decodage`) : part des flux OLE lus par
/// le lecteur (octets), des enregistrements d'éléments (section 0x66) lus en entier par
/// le schéma, de ceux dont la classe est exploitée (`Sorte`), et des compléments de
/// géométrie (section 0x67) lus en entier ; en nombre et en octets.
pub struct Decodage {
    pub flux: (u64, u64),
    /// Flux non lus : chemin, taille.
    pub non_lus: Vec<(String, u64)>,
    pub elements: (usize, usize, usize),
    pub octets_elements: (u64, u64, u64),
    /// Octets des classes interprétées par leur seul nom (`annexes::NOMMEES_SEULEMENT`).
    pub octets_nommes: u64,
    pub geometries: (usize, usize),
    pub octets_geometries: (u64, u64),
    /// Classes lues mais non interprétées : nom, nombre, octets (les plus lourdes d'abord).
    pub non_interpretees: Vec<(String, usize, u64)>,
}

/// Flux globaux, chacun un objet sérialisé (préfixe de 8 octets, 24 pour `Contents`,
/// puis gzip ; objet suivi d'un u32 0) : `Global/Latest` (`ADocument` : table des
/// documents embarqués, applications, versions de Revit qui l'ont enregistré…),
/// `Global/PartitionTable` (sous-projets), `Global/DocumentIncrementTable` (historique
/// des enregistrements : utilisateur, épisodes, nombre d'éléments), `Contents`
/// (`DocumentStorageIndexImpl` : version du logiciel, révisions des flux, emplacement
/// central). `Global/ContentDocuments` : pour chaque document de famille embarqué,
/// `ContentMarker`, u32 n, son `ADocument` de n octets, u32 n ; puis un marqueur vide
/// et u32 0.
pub const FLUX_GLOBAUX: &[&str] = &["/Global/Latest", "/Global/PartitionTable", "/Global/DocumentIncrementTable", "/Contents", "/Global/ContentDocuments"];

/// Objets d'un flux global (racines lues en entier).
pub fn flux_global<'s>(sch: &'s Schema, nom: &str, raw: &[u8]) -> Result<Vec<Record<'s>>, String> {
    let b = unpad(raw);
    let h = b.windows(3).position(|w| w == [0x1f, 0x8b, 0x08]).ok_or("pas de gzip")?;
    let d = inflate(&b[h..])?;
    let u32_a = |o: usize| d.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize);
    if nom != "/Global/ContentDocuments" {
        if d.len() < 4 || u32_a(d.len() - 4) != Some(0) {
            return Err("u32 0 final absent".into());
        }
        return Ok(vec![champs::read(sch, &d[..d.len() - 4])?]);
    }
    let mut out = Vec::new();
    let mut o = 0;
    loop {
        let (marqueur, fin) = champs::read_prefix(sch, &d[o..])?;
        if marqueur.root.class_name() != "ContentMarker" {
            return Err(format!("{} au lieu de ContentMarker à {o}", marqueur.root.class_name()));
        }
        let o2 = o + fin;
        if o2 + 4 == d.len() && u32_a(o2) == Some(0) {
            return Ok(out);
        }
        let n = u32_a(o2).ok_or("longueur absente")?;
        let doc = champs::read(sch, d.get(o2 + 4..o2 + 4 + n).ok_or("document tronqué")?)?;
        if u32_a(o2 + 4 + n) != Some(n) {
            return Err(format!("longueur non répétée à {}", o2 + 4 + n));
        }
        out.push(doc);
        o = o2 + 8 + n;
    }
}

/// Informations du document tirées des flux globaux.
#[derive(Debug, Default)]
pub struct InfosDocument {
    /// Version du logiciel du dernier enregistrement (`Contents.m_softwareVersion`).
    pub logiciel: String,
    /// Versions de Revit qui ont enregistré le fichier (`ADocument.m_storedByRevitBuild`).
    pub versions: Vec<String>,
    /// Sous-projets (`PartitionTable.m_set[].m_name`).
    pub sous_projets: Vec<String>,
    /// Enregistrements : utilisateur, nombre d'éléments (`DocumentIncrementTable.m_increments`).
    pub enregistrements: Vec<(String, i64)>,
    /// Documents de famille embarqués : famille propriétaire (`ADocument.m_ownerFamilyId`)
    /// et nombre d'éléments de leur table.
    pub documents: Vec<(i64, usize)>,
}

pub fn infos_document(path: &Path) -> Result<InfosDocument, String> {
    let mut cfb = Cfb::open(path)?;
    let raw = cfb.read("Formats/Latest")?.ok_or("flux Formats/Latest absent")?;
    let sch = Schema::parse(&inflate(&unpad(&raw)).map_err(|e| format!("Formats/Latest : {e}"))?)?;
    let mut infos = InfosDocument::default();
    let lire = |cfb: &mut Cfb, f: &str| -> Result<Vec<Record<'_>>, String> {
        match cfb.read(&f[1..])? {
            Some(raw) => flux_global(&sch, f, &raw),
            None => Ok(Vec::new()),
        }
    };
    let texte = |o: &Obj, k: &str| o.get(k).and_then(Val::str).unwrap_or_default().to_string();
    for r in lire(&mut cfb, "/Contents")? {
        infos.logiciel = texte(&r.root, "m_softwareVersion");
    }
    for r in lire(&mut cfb, "/Global/Latest")? {
        infos.versions = r.root.get("m_storedByRevitBuild").map(Val::list).unwrap_or_default().iter().filter_map(|v| Some(texte(v.obj()?, "m_str"))).collect();
    }
    for r in lire(&mut cfb, "/Global/PartitionTable")? {
        let set = r.root.get("m_set").map(Val::list).unwrap_or_default();
        infos.sous_projets = set.iter().filter_map(|v| r.deref(v).map(|b| &b.obj).or(v.obj())).map(|o| texte(o, "m_name")).collect();
    }
    for r in lire(&mut cfb, "/Global/DocumentIncrementTable")? {
        let l = r.root.get("m_increments").map(Val::list).unwrap_or_default();
        infos.enregistrements = l.iter().filter_map(|v| r.deref(v).map(|b| &b.obj).or(v.obj())).map(|o| (texte(o, "m_userName"), o.get("m_totalElements").and_then(Val::int).unwrap_or(0))).collect();
    }
    for r in lire(&mut cfb, "/Global/ContentDocuments")? {
        let n = r.field_body(&r.root, "m_elemTable").or(r.root.get("m_elemTable").and_then(Val::obj)).and_then(|t| t.get("m_elemArr")).map(|a| a.list().len()).unwrap_or(0);
        infos.documents.push((r.root.get("m_ownerFamilyId").and_then(Val::element_id).unwrap_or(-1), n));
    }
    Ok(infos)
}

pub fn decodage(path: &Path) -> Result<Decodage, String> {
    let mut cfb = Cfb::open(path)?;
    const LUS: &[&str] = &["/Formats/Latest", "/Global/ElemTable", "/Global/History", "/BasicFileInfo", "/ProjectInformation", "/PartAtom", "/TransmissionData", "/RevitPreview4.0"];
    let entrees = cfb.entries();
    let total: u64 = entrees.iter().filter(|e| !e.1).map(|e| e.2).sum();
    let raw = cfb.read("Formats/Latest")?.ok_or("flux Formats/Latest absent")?;
    let sch = Schema::parse(&inflate(&unpad(&raw)).map_err(|e| format!("Formats/Latest : {e}"))?)?;
    // flux globaux comptés lus s'ils se lisent en entier
    let mut globaux_lus = Vec::new();
    for f in FLUX_GLOBAUX {
        if let Some(raw) = cfb.read(&f[1..])? {
            if flux_global(&sch, f, &raw).is_ok() {
                globaux_lus.push(*f);
            }
        }
    }
    let lu = |p: &str| LUS.contains(&p) || globaux_lus.contains(&p) || p.starts_with("/Partitions/");
    let lus: u64 = entrees.iter().filter(|e| !e.1 && lu(&e.0)).map(|e| e.2).sum();
    let ids8 = sch.ids8();
    let sortes: HashMap<u16, Sorte> = sch.classes.iter().filter_map(|c| Some((c.id, Sorte::de(&sch, c.id, &c.name)?))).collect();
    let marqueurs = sch.by_name("ContentMarker").zip(sch.by_name("ContentKey")).map(|(a, b)| (a.id, b.id));
    let non_lus = entrees.iter().filter(|e| !e.1 && !lu(&e.0)).map(|e| (e.0.clone(), e.2)).collect();
    let mut d = Decodage { flux: (lus, total), non_lus, elements: (0, 0, 0), octets_elements: (0, 0, 0), geometries: (0, 0), octets_geometries: (0, 0), non_interpretees: Vec::new(), octets_nommes: 0 };
    let nommees: std::collections::HashSet<u16> = super::annexes::NOMMEES_SEULEMENT.iter().filter_map(|n| sch.by_name(n)).map(|c| c.id).collect();
    let mut autres: HashMap<u16, (usize, u64)> = HashMap::new();
    let parts: Vec<String> = entrees.iter().filter(|e| !e.1 && e.0.starts_with("/Partitions/")).map(|e| e.0.clone()).collect();
    for p in parts {
        let raw = cfb.read(&p)?.ok_or_else(|| format!("flux {p} illisible"))?;
        let (sec, _) = sections_documents(&raw, marqueurs).map_err(|e| format!("{p} : {e}"))?;
        let vide = Vec::new();
        for (_, body) in records(sec.get(&0x66).unwrap_or(&vide), true, ids8).map_err(|e| format!("{p} : {e}"))? {
            let n = body.len() as u64;
            d.elements.0 += 1;
            d.octets_elements.0 += n;
            if champs::read(&sch, body).is_ok() {
                d.elements.1 += 1;
                d.octets_elements.1 += n;
                match champs::class_id(body) {
                    Some(c) if sortes.contains_key(&c) => {
                        d.elements.2 += 1;
                        d.octets_elements.2 += n;
                        if nommees.contains(&c) {
                            d.octets_nommes += n;
                        }
                    }
                    Some(c) => {
                        let e = autres.entry(c).or_default();
                        e.0 += 1;
                        e.1 += n;
                    }
                    None => {}
                }
            }
        }
        for (_, body) in records(sec.get(&0x67).unwrap_or(&vide), true, ids8).map_err(|e| format!("{p} : {e}"))? {
            let n = body.len() as u64;
            d.geometries.0 += 1;
            d.octets_geometries.0 += n;
            if champs::read(&sch, body).is_ok() {
                d.geometries.1 += 1;
                d.octets_geometries.1 += n;
            }
        }
    }
    let mut l: Vec<(String, usize, u64)> = autres.into_iter().map(|(c, (n, o))| (sch.classes.iter().find(|k| k.id == c).map(|k| k.name.clone()).unwrap_or_default(), n, o)).collect();
    l.sort_by(|a, b| b.2.cmp(&a.2));
    d.non_interpretees = l;
    Ok(d)
}

/// Emprise d'une pièce : triangles de son pied (x, y en mm) et étendue en hauteur.
pub struct Emprise {
    pub piece: i64,
    pub triangles: Vec<[DVec2; 3]>,
    pub zmin: f64,
    pub zmax: f64,
}

/// Objet lu, réduit aux champs utiles.
enum Extrait {
    Rien,
    Type(i64, TypeElem),
    Famille(i64, Famille),
    Materiau(i64, Materiau),
    Mur(Mur),
    Dalle(Dalle),
    Toposolide(Dalle),
    SurfaceTopo(SurfaceTopo),
    Toit(Toit),
    Escalier(Escalier),
    /// Volée ou palier et son escalier (`m_stairsId`).
    Volee(i64, i64),
    GardeCorps(GardeCorps),
    Grille(Grille),
    /// Instance et, si elle est hébergée, (`m_hostParam`, `m_elevation` en mm) pour
    /// placer son point sur la ligne de l'hôte.
    Instance(Instance, Option<(f64, f64)>),
    InstanceDeVue(InstanceVue),
    Piece(Piece),
    Topologie(super::zones::Topologie),
    PlanReference(super::plans::PlanReference),
    PlanConstruction(super::plans::PlanConstruction),
    Apparence(i64, super::plans::Apparence),
    Media(super::medias::Media),
    Annexe(super::annexes::Annexe),
    /// Ligne de zone : schéma, niveau, extrémités (x, y en mm).
    LigneZone(i64, i64, DVec2, DVec2),
    Forme(Forme),
    Categorie(i64, String),
    Vue(Vue),
    /// Cadrage (origine, axes, longueurs ; cadrage complet) de la vue d'un `Viewer`.
    Cadrage(i64, (DVec3, [DVec3; 3], [f64; 3]), Option<Cadre>),
    Element2D(Element2D),
    Motif(i64, Motif),
    TypeRegion(i64, TypeRegion),
    /// Couleur de projection (catégorie, rouge vert bleu).
    Style(i64, [u8; 3]),
    Parametre(i64, Definition),
    Phase(i64, String),
    OrdrePhases(Vec<i64>),
    /// Filtre de phase : nom, présentation par état.
    FiltrePhase(i64, String, [u8; 7]),
    Coordonnee(coordonnees::Piece),
    /// Police et type de texte propriétaire.
    Police(i64, Police),
    TypeFenetre(i64, TypeFenetre),
    Revision(i64, Revision),
    InfoProjet(i64),
    TypeCote(i64, vues::TypeCote),
    /// Format des longueurs (unité, précision) et unités du projet.
    Unites(Option<(String, f64)>, parametres::Unites),
    /// Lignes d'une esquisse (`m_elemIdsPairSet.m_data[].m_elementId`).
    Esquisse(Vec<i64>),
}

impl Maquette {
    /// Nom et classe IFC de la catégorie d'une instance (par sa famille).
    pub fn categorie(&self, inst: &Instance) -> (String, &'static str) {
        self.categorie_famille(self.types.get(&inst.type_id).map(|t| t.famille).unwrap_or(-1))
    }

    /// Nom et classe IFC de la catégorie d'une famille.
    pub fn categorie_famille(&self, famille: i64) -> (String, &'static str) {
        let cat = self.familles.get(&famille).map(|f| f.categorie).unwrap_or(-1);
        match CATEGORIES.iter().find(|c| c.0 == cat) {
            Some(&(_, nom, classe)) => (nom.to_string(), classe),
            None => (format!("Catégorie {cat}"), "IFCBUILDINGELEMENTPROXY"),
        }
    }

    /// Nom d'une catégorie ou sous-catégorie : celle du document, prédéfinie, ou son id.
    pub fn nom_categorie(&self, id: i64) -> String {
        if let Some(n) = self.categories.get(&id) {
            return n.clone();
        }
        let predefinie = SOUS_CATEGORIES.iter().map(|&(i, n)| (i, n)).chain(CATEGORIES.iter().map(|&(i, n, _)| (i, n))).find(|&(i, _)| i == id);
        predefinie.map(|(_, n)| n.to_string()).unwrap_or_else(|| format!("Catégorie {id}"))
    }

    /// Catégorie et type de chaque élément lu (murs, sols, plafonds, toits, escaliers,
    /// garde-corps, instances, pièces, surfaces, niveaux, vues, feuilles, grilles).
    fn indexer(&mut self) {
        let (mut c, mut t) = (HashMap::new(), HashMap::new());
        for x in &self.murs {
            c.insert(x.id, -2_000_011);
            t.insert(x.id, x.type_id);
        }
        for x in &self.dalles {
            c.insert(x.id, if x.classe == "Ceiling" { -2_000_038 } else { -2_000_032 });
            t.insert(x.id, x.type_id);
        }
        for x in &self.toits {
            c.insert(x.id, -2_000_035);
            t.insert(x.id, x.type_id);
        }
        for x in &self.escaliers {
            c.insert(x.id, -2_000_120);
            t.insert(x.id, x.type_id);
        }
        for x in &self.garde_corps {
            c.insert(x.id, -2_000_126);
            t.insert(x.id, x.type_id);
        }
        for x in &self.instances {
            let f = self.types.get(&x.type_id).map(|ty| ty.famille).unwrap_or(-1);
            c.insert(x.id, self.familles.get(&f).map(|f| f.categorie).unwrap_or(-1));
            t.insert(x.id, x.type_id);
        }
        for x in &self.pieces {
            match x.genre {
                "piece" => drop(c.insert(x.id, -2_000_160)),
                "surface" => drop(c.insert(x.id, -2_003_200)),
                _ => {}
            }
        }
        for x in &self.niveaux {
            c.insert(x.id, -2_000_240);
            t.insert(x.id, x.type_id);
        }
        for x in self.vues.iter().filter(|v| !v.gabarit) {
            c.insert(x.id, if x.genre == "Feuille" { -2_003_100 } else { -2_000_279 });
        }
        for x in &self.grilles {
            c.insert(x.id, -2_000_220);
        }
        self.categorie_de = c;
        self.type_de_element = t;
    }

    /// Éléments lus d'une catégorie.
    pub fn elements_de_categorie(&self, categorie: i64) -> Vec<i64> {
        self.categorie_de.iter().filter(|&(_, &c)| c == categorie).map(|(&e, _)| e).collect()
    }

    /// Nom d'un plan de construction (`SketchPlane.Name` de l'API) : celui de son plan
    /// support (niveau, plan de référence, grille), « <non associé> » sans support.
    pub fn nom_plan_construction(&self, p: &super::plans::PlanConstruction) -> String {
        if p.support == -1 {
            return "<non associé>".into();
        }
        // formes d'une famille : nom de leur genre dans Revit
        let forme = |id: i64| {
            self.formes.iter().find(|f| f.id == id).and_then(|f| match f.classe.as_str() {
                "ExtrusionElem" => Some("Extrusion"),
                "FamilyGeomCombination" => Some("Géométrie solide attachée"),
                _ => None,
            })
        };
        self.niveaux
            .iter()
            .find(|n| n.id == p.support)
            .map(|n| n.nom.clone())
            .or_else(|| self.plans_reference.iter().find(|r| r.id == p.support).map(|r| r.nom.clone()))
            .or_else(|| self.grilles.iter().find(|g| g.id == p.support).map(|g| g.nom.clone()))
            .or_else(|| self.vues.iter().find(|v| v.id == p.support).map(|v| v.nom.clone()))
            .or_else(|| forme(p.support).map(str::to_string))
            // face d'un mur, d'un sol, d'une instance : nom de son type
            .or_else(|| self.type_de(p.support).and_then(|t| self.types.get(&t)).map(|t| t.nom.clone()))
            .or_else(|| self.nom_element(-1, p.support))
            .unwrap_or_else(|| "<non associé>".into())
    }

    /// Variante d'un élément : (jeu, option, principale), None dans le modèle principal.
    pub fn variante(&self, element: i64) -> Option<(&str, &str, bool)> {
        let o = self.variantes.get(self.variante_de.get(&element)?)?;
        let jeu = self.variantes.get(&o.lien);
        Some((jeu.map(|j| j.nom.as_str()).unwrap_or(""), o.nom.as_str(), jeu.is_some_and(|j| j.lien == o.id)))
    }

    /// Point d'un groupe (LocationPoint de l'API, mm) : origine de son repère plus
    /// l'origine d'insertion de son type, tournée par le repère ; groupe de modèle
    /// (rattaché à un niveau) : cote = altitude du niveau + décalage (`m_offsetFromLevel`).
    pub fn point_groupe(&self, g: &super::annexes::Groupe) -> DVec3 {
        let io = self.types.get(&g.type_id).and_then(|t| t.origine_insertion).unwrap_or(DVec3::ZERO);
        let p = g.origine + g.axes * io;
        match self.niveaux.iter().find(|l| l.id == g.niveau) {
            Some(l) => DVec3::new(p.x, p.y, l.altitude + g.decalage),
            None => p,
        }
    }

    /// Type d'un élément lu.
    pub fn type_de(&self, element: i64) -> Option<i64> {
        self.type_de_element.get(&element).copied()
    }

    /// Pièces de départ et d'arrivée d'une porte ou fenêtre (FromRoom, ToRoom de l'API,
    /// dernière phase ; −1 : aucune). Revit ne les enregistre pas : pièces contenant les
    /// points situés de part et d'autre du mur hôte, à 100 mm de ses faces et 300 mm
    /// au-dessus du point d'insertion, le long de l'axe Y du repère ; l'arrivée est du
    /// côté +Y, sauf si la main est retournée (`m_flippedX`) ou exclusif les pièces sont
    /// inversées (`m_flippedFromToRoom`). Sans mur hôte, aucune pièce.
    pub fn pieces_de_porte(&self, element: i64) -> (i64, i64) {
        let Some(i) = self.instances.iter().find(|i| i.id == element) else { return (-1, -1) };
        let Some(mur) = self.murs.iter().find(|w| w.id == i.hote) else { return (-1, -1) };
        let Some(axes) = i.axes else { return (-1, -1) };
        let ep = self.types.get(&mur.type_id).and_then(|t| t.epaisseur).unwrap_or(0.0);
        let cote = |s: f64| {
            let p = i.point + axes.y_axis * s * (ep / 2.0 + 100.0) + DVec3::Z * 300.0;
            self.emprises.iter().find(|e| e.contient(p)).map(|e| e.piece).unwrap_or(-1)
        };
        let (plus, moins) = (cote(1.0), cote(-1.0));
        if i.main_retournee != i.pieces_inversees { (plus, moins) } else { (moins, plus) }
    }

    /// IfcGUID d'un élément du document (None hors ElemTable ou format antérieur à 2024).
    pub fn guid_ifc(&self, id: i64) -> Option<String> {
        Some(parametres::ifc_guid(self.episodes.get(*self.episode_de.get(&id)? as usize)?, id))
    }

    /// UniqueId de Revit d'un élément.
    pub fn unique_id(&self, id: i64) -> Option<String> {
        Some(parametres::unique_id(self.episodes.get(*self.episode_de.get(&id)? as usize)?, id))
    }

    /// Nom de l'élément désigné par la valeur ElementId d'un paramètre : type (« Famille :
    /// type » pour FAMILY_AND_TYPE, famille seule pour FAMILY), niveau, matériau, phase,
    /// vue, famille.
    pub fn nom_element(&self, param: i64, id: i64) -> Option<String> {
        const FAMILLE_ET_TYPE: [i64; 2] = [-1_002_052, -1_002_001];
        const FAMILLE: [i64; 2] = [-1_002_051, -1_002_002];
        if let Some(t) = self.types.get(&id) {
            // famille chargée, sinon famille système (« Mur de base », « Mur-rideau »…)
            let fam = self.familles.get(&t.famille).map(|f| f.nom.as_str()).or_else(|| famille_systeme(&t.classe).map(|x| x.1));
            return Some(match fam {
                Some(f) if FAMILLE.contains(&param) => f.to_string(),
                // écriture de Revit : « Famille: Type »
                Some(f) if FAMILLE_ET_TYPE.contains(&param) => format!("{f}: {}", t.nom),
                _ => t.nom.clone(),
            });
        }
        if let Some(n) = self.niveaux.iter().find(|n| n.id == id) {
            return Some(n.nom.clone());
        }
        let nom = self.materiaux.get(&id).map(|x| x.nom.clone()).or_else(|| self.phases.get(&id).cloned()).or_else(|| self.filtres_phase.get(&id).map(|f| f.0.clone()));
        nom.or_else(|| self.vues.iter().find(|v| v.id == id).map(|v| v.nom.clone())).or_else(|| self.familles.get(&id).map(|f| f.nom.clone()))
    }

    /// Paramètres affichables d'un élément, par groupe de Revit. Type de famille : sans
    /// les paramètres d'occurrence de sa famille (Revit les montre sur les instances) ;
    /// famille d'un document `.rfa` : « (occurrence) » après le nom de ces paramètres.
    pub fn parametres_affiches(&self, id: i64) -> Vec<parametres::Affiche> {
        let Some(v) = self.parametres.get(&id) else { return Vec::new() };
        let occ = self.types.get(&id).and_then(|t| self.familles.get(&t.famille)).map(|f| &f.occurrence);
        let v: Vec<(i64, Brut)> = v.iter().filter(|(p, _)| occ.is_none_or(|o| !o.contains(p))).cloned().collect();
        let mut out = parametres::affiches(&v, &self.definitions, &self.unites, &|p, e| self.nom_element(p, e));
        if let Some(f) = self.familles.get(&id).filter(|_| self.famille_document == Some(id)) {
            let noms: std::collections::HashSet<String> = v.iter().filter(|(p, _)| f.occurrence.contains(p)).filter_map(|(p, _)| self.definitions.get(p).map(|d| d.nom.clone()).or_else(|| parametres::integres().get(p).map(|i| i.libelle.to_string()))).collect();
            for a in out.iter_mut().filter(|a| noms.contains(&a.nom)) {
                a.nom.push_str(" (occurrence)");
            }
        }
        out
    }

    /// « Famille : type » d'une instance.
    pub fn nom_type_instance(&self, inst: &Instance) -> String {
        let t = self.types.get(&inst.type_id);
        let fam = t.and_then(|t| self.familles.get(&t.famille)).map(|f| f.nom.as_str()).unwrap_or("?");
        format!("{fam} : {}", t.map(|t| t.nom.as_str()).unwrap_or("?"))
    }
}

// ------------------------------------------------------------------ Scène

/// Élément de la scène, quelle que soit sa catégorie.
struct Produit<'a> {
    id: i64,
    classe: &'static str,
    nom: String,
    type_id: i64,
    nom_type: String,
    niveau: i64,
    props: Vec<(&'static str, Value)>,
    geometrie: &'a Geometrie,
}

struct Builder {
    entities: IdMap<Entity>,
}

/// Nombre maximal de traits de motifs de surface dans une scène.
const MAX_TRAITS_MOTIFS: usize = 1_500_000;

