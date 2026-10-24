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

use super::champs::{self, Obj, Record, Val};
use super::coordonnees::{self, Coordonnees};
use super::geometrie::{self, FT, Maillage};
use super::objets::{document_de, elem_table, episodes, inflate, records, sections_documents, unpad};
use super::parametres::{self, Brut, Definition};
use super::schema::Schema;
use super::feuilles;
use super::vues::{self, Cadre, Contenu, Element2D, Motif, TypeFenetre, TypeRegion, Vue};
use super::Cfb;
use crate::geom::{Ligne, Part, ProductGeom, Rgba, class_color};
use crate::model::IfcModel;
use crate::scene::{self, NavFamille, NavVue, Navigateur, Scene};
use crate::step::{Entity, Id, IdMap, StepFile, Value, format_real};
use crate::trace2d::{self, Primitive, Role, Style};
use glam::{DMat3, DVec3};
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
    /// Esquisse (dérivées de `Sketch`) : ses lignes ne sont pas tracées dans la vue.
    Esquisse,
}

impl Sorte {
    /// Éléments dont on lit le complément 0x67 (géométrie).
    fn a_geometrie(self) -> bool {
        matches!(self, Sorte::Mur | Sorte::Dalle | Sorte::Toposolide | Sorte::SurfaceTopo | Sorte::Toit | Sorte::Volee | Sorte::GardeCorps | Sorte::Instance | Sorte::Piece | Sorte::Forme | Sorte::Region)
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

/// Lit le schéma, la table des éléments et les partitions, puis les éléments et leur
/// géométrie.
pub fn lire(path: &Path, progress: &dyn Fn(String)) -> Result<Maquette, String> {
    let mut cfb = Cfb::open(path)?;
    // document de famille : flux PartAtom (un projet ou un gabarit a ProjectInformation)
    let est_famille = cfb.entries().iter().any(|(p, dir, _)| !dir && p == "/PartAtom");
    let raw = cfb.read("Formats/Latest")?.ok_or("flux Formats/Latest absent")?;
    let sch = Schema::parse(&inflate(&unpad(&raw)).map_err(|e| format!("Formats/Latest : {e}"))?)?;
    let ids8 = sch.ids8();
    let table = elem_table(&cfb.read("Global/ElemTable")?.ok_or("flux Global/ElemTable absent")?)?;
    // épisodes de création : UniqueId et IfcGUID (règle vérifiée sur les formats 2024 et
    // suivants, ElemTable de 40 octets)
    let episodes = if ids8 { cfb.read("Global/History")?.and_then(|h| episodes(&h).ok()).unwrap_or_default() } else { Vec::new() };
    let sortes: HashMap<u16, Sorte> = sch.classes.iter().filter_map(|c| Some((c.id, Sorte::de(&sch, c.id, &c.name)?))).collect();

    let mut parts: Vec<(u64, String)> = cfb
        .entries()
        .into_iter()
        .filter(|(p, dir, _)| !dir && p.starts_with("/Partitions/"))
        .filter_map(|(p, _, _)| Some((p.rsplit('/').next()?.parse().ok()?, p)))
        .collect();
    parts.sort();

    // types (Symbol) : leur GElement porte la géométrie des GInstance (familles, marches
    // d'escalier assemblé `StairsTriserSymbol`…)
    let classes_types: std::collections::HashSet<u16> = sch.classes.iter().filter(|c| sch.inherits(c.id, "Symbol")).map(|c| c.id).collect();
    let mut types_famille: std::collections::HashSet<i64> = Default::default();
    let mut objets: HashMap<i64, (Sorte, Vec<u8>)> = HashMap::new();
    // textes, étiquettes et polices des documents de famille embarqués (hors ElemTable)
    let marqueurs = sch.by_name("ContentMarker").zip(sch.by_name("ContentKey")).map(|(a, b)| (a.id, b.id));
    let classes_embarquees: std::collections::HashSet<u16> = ["TextNote", "TagNote", "FontElem", "ParamElemFamily", "FilledRegion", "FilledRegionAttributes", "FillPatternElem", "GStyleElem", "TextNoteAttributes", "TagNoteAttributes"].iter().filter_map(|n| sch.by_name(n)).map(|c| c.id).collect();
    let mut embarques: HashMap<i64, ([u8; 16], Vec<u8>)> = HashMap::new();
    let classe_region = sch.by_name("FilledRegion").map(|c| c.id);
    let mut regions_embarquees: std::collections::HashSet<i64> = Default::default();
    let mut complements: HashMap<i64, Vec<u8>> = HashMap::new();
    let mut geometries_types: HashMap<i64, Vec<u8>> = HashMap::new();
    for (k, (_, p)) in parts.iter().enumerate() {
        progress(format!("Partition {} / {} ({p})…", k + 1, parts.len()));
        let raw = cfb.read(p)?.ok_or_else(|| format!("flux {p} illisible"))?;
        let (sec, debuts) = sections_documents(&raw, marqueurs).map_err(|e| format!("{p} : {e}"))?;
        let vide = Vec::new();
        let donnees = sec.get(&0x66).unwrap_or(&vide);
        let debuts = debuts.get(&0x66).map(Vec::as_slice).unwrap_or_default();
        for (eid, body) in records(donnees, true, ids8).map_err(|e| format!("{p}, section 0x66 : {e}"))? {
            if !table.contains_key(&eid) {
                if let (Some(c), Some(doc)) = (champs::class_id(body), document_de(debuts, body.as_ptr() as usize - donnees.as_ptr() as usize)) {
                    if classes_embarquees.contains(&c) {
                        embarques.insert(eid, (doc, body.to_vec()));
                        if Some(c) == classe_region {
                            regions_embarquees.insert(eid);
                        }
                    }
                }
                continue;
            }
            let Some(c) = champs::class_id(body) else { continue };
            if classes_types.contains(&c) {
                types_famille.insert(eid);
            }
            if let Some(&s) = sortes.get(&c).filter(|s| est_famille || !s.de_famille()) {
                objets.insert(eid, (s, body.to_vec()));
            }
        }
        for (eid, body) in records(sec.get(&0x67).unwrap_or(&vide), true, ids8).map_err(|e| format!("{p}, section 0x67 : {e}"))? {
            if objets.get(&eid).is_some_and(|o| o.0.a_geometrie()) {
                complements.insert(eid, body.to_vec());
            } else if types_famille.contains(&eid) {
                geometries_types.insert(eid, body.to_vec());
            } else if regions_embarquees.contains(&eid) {
                complements.insert(eid, body.to_vec());
            }
        }
    }

    // Les niveaux d'abord (altitudes), puis chaque objet est lu et réduit aussitôt à ses
    // champs utiles : garder tous les enregistrements lus coûtait 6 Go à Saint-Malo.
    progress(format!("Lecture de {} objets…", objets.len()));
    let mut ids: Vec<i64> = objets.keys().copied().collect();
    ids.sort_unstable();
    let lire_objet = |eid: &i64| -> Option<Result<(Sorte, Record), (i64, String)>> {
        let (s, body) = &objets[eid];
        match champs::read(&sch, body) {
            Ok(r) if place(&r.root) => Some(Ok((*s, r))),
            Ok(_) => None,
            Err(e) => Some(Err((*eid, e))),
        }
    };
    let mut illisibles = Vec::new();
    let mut niveaux = Vec::new();
    let mut parametres: HashMap<i64, Vec<(i64, Brut)>> = HashMap::new();
    for eid in ids.iter().filter(|e| objets[e].0 == Sorte::Niveau) {
        match lire_objet(eid) {
            Some(Ok((_, r))) => {
                niveaux.extend(niveau(*eid, &r));
                parametres.insert(*eid, parametres::valeurs(&r));
            }
            Some(Err(e)) => illisibles.push(e),
            None => {}
        }
    }
    niveaux.sort_by(|a, b| a.altitude.total_cmp(&b.altitude).then(a.id.cmp(&b.id)));
    let altitude: HashMap<i64, f64> = niveaux.iter().map(|n| (n.id, n.altitude)).collect();
    let herite = |c: &str, base: &str| sch.by_name(c).is_some_and(|c| sch.inherits(c.id, base));
    let extraits: Vec<Result<(i64, Extrait, Vec<(i64, Brut)>), (i64, String)>> = ids
        .par_iter()
        .filter(|e| objets[e].0 != Sorte::Niveau)
        .filter_map(|eid| Some(lire_objet(eid)?.map(|(s, r)| (*eid, extraire(*eid, s, &r, &altitude, &herite), parametres::valeurs(&r)))))
        .collect();
    drop(objets);

    let (mut types, mut familles, mut materiaux) = (HashMap::new(), HashMap::new(), HashMap::new());
    let (mut murs, mut dalles, mut toits, mut escaliers, mut grilles, mut pieces) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut toposolides, mut surfaces_topo) = (Vec::new(), Vec::new());
    let (mut volees, mut instances, mut instances_de_vue, mut garde_corps) = (HashMap::new(), Vec::new(), 0, Vec::new());
    let (mut formes, mut categories) = (Vec::new(), HashMap::new());
    let (mut vues, mut cadrages, mut elements_2d) = (Vec::new(), HashMap::new(), Vec::new());
    let (mut motifs, mut types_regions, mut couleurs_categories) = (HashMap::new(), HashMap::new(), HashMap::new());
    let (mut definitions, mut phases) = (HashMap::new(), HashMap::new());
    let mut coord = Coordonnees::default();
    let (mut ordre_phases, mut filtres_phase) = (Vec::new(), HashMap::new());
    let mut unites = parametres::Unites::default();
    let (mut instances_vue, mut polices, mut types_fenetres, mut revisions, mut info_projet) = (Vec::new(), HashMap::new(), HashMap::new(), HashMap::new(), -1);
    let (mut types_cotes, mut format_longueurs) = (HashMap::new(), None);
    let mut lignes_esquisse: std::collections::HashSet<i64> = Default::default();
    for x in extraits {
        let x = x.map(|(eid, x, vals)| {
            if !vals.is_empty() {
                parametres.insert(eid, vals);
            }
            x
        });
        match x {
            Err(e) => illisibles.push(e),
            Ok(Extrait::Rien) => {}
            Ok(Extrait::Type(id, t)) => drop(types.insert(id, t)),
            Ok(Extrait::Famille(id, f)) => drop(familles.insert(id, f)),
            Ok(Extrait::Materiau(id, m)) => drop(materiaux.insert(id, m)),
            Ok(Extrait::Mur(m)) => murs.push(m),
            Ok(Extrait::Dalle(d)) => dalles.push(d),
            Ok(Extrait::Toposolide(d)) => toposolides.push(d),
            Ok(Extrait::SurfaceTopo(x)) => surfaces_topo.push(x),
            Ok(Extrait::Toit(t)) => toits.push(t),
            Ok(Extrait::Escalier(e)) => escaliers.push(e),
            Ok(Extrait::Volee(id, esc)) => drop(volees.insert(id, esc)),
            Ok(Extrait::GardeCorps(g)) => garde_corps.push(g),
            Ok(Extrait::Grille(g)) => grilles.push(g),
            Ok(Extrait::Instance(i, h)) => instances.push((i, h)),
            Ok(Extrait::InstanceDeVue(i)) => {
                instances_de_vue += 1;
                instances_vue.push(i);
            }
            Ok(Extrait::Piece(p)) => pieces.push(p),
            Ok(Extrait::Forme(f)) => formes.push(f),
            Ok(Extrait::Categorie(id, nom)) => drop(categories.insert(id, nom)),
            Ok(Extrait::Vue(v)) => vues.push(v),
            Ok(Extrait::Cadrage(v, c, k)) => drop(cadrages.insert(v, (c, k))),
            Ok(Extrait::Element2D(e)) => elements_2d.push(e),
            Ok(Extrait::Motif(id, x)) => drop(motifs.insert(id, x)),
            Ok(Extrait::TypeRegion(id, x)) => drop(types_regions.insert(id, x)),
            Ok(Extrait::Style(cat, c)) => drop(couleurs_categories.insert(cat, c)),
            Ok(Extrait::Parametre(id, d)) => drop(definitions.insert(id, d)),
            Ok(Extrait::Phase(id, nom)) => drop(phases.insert(id, nom)),
            Ok(Extrait::OrdrePhases(l)) => ordre_phases = l,
            Ok(Extrait::FiltrePhase(id, nom, p)) => drop(filtres_phase.insert(id, (nom, p))),
            Ok(Extrait::Coordonnee(x)) => match x {
                coordonnees::Piece::Emplacement(e) => coord.emplacements.push(e),
                coordonnees::Piece::Lieu(id, l) => drop(coord.lieux.insert(id, l)),
                coordonnees::Piece::Point(true, p) => coord.point_topo = Some(p),
                coordonnees::Piece::Point(false, p) => coord.point_base = Some(p),
                coordonnees::Piece::Actif(a, p) => (coord.actif, coord.projet) = (a, p),
            },
            Ok(Extrait::Police(t, x)) => drop(polices.insert(t, x)),
            Ok(Extrait::TypeFenetre(id, x)) => drop(types_fenetres.insert(id, x)),
            Ok(Extrait::Revision(id, x)) => drop(revisions.insert(id, x)),
            Ok(Extrait::InfoProjet(id)) => info_projet = id,
            Ok(Extrait::TypeCote(id, x)) => drop(types_cotes.insert(id, x)),
            Ok(Extrait::Unites(f, u)) => {
                format_longueurs = f;
                unites = u;
            }
            Ok(Extrait::Esquisse(l)) => lignes_esquisse.extend(l),
        }
    }
    let famille_document = if est_famille { familles.iter().filter(|(_, f): &(&i64, &Famille)| f.parent == -1).map(|(&id, _)| id).min() } else { None };
    // vues et éléments 2D du document : ceux des familles imbriquées rangées en ligne
    // dans un .rfa (symboles d'annotation) sont écartés
    let du_document = |f: i64| f == -1 || familles.get(&f).is_some_and(|x: &Famille| x.parent == -1);
    vues.retain(|v: &Vue| du_document(v.famille));
    for v in &mut vues {
        if let Some((c, k)) = cadrages.remove(&v.id) {
            v.cadrage = Some(c);
            v.cadre = k;
        }
    }
    vues.sort_by_key(|v| v.id);
    let ids_vues: std::collections::HashSet<i64> = vues.iter().map(|v| v.id).collect();
    // les lignes d'une esquisse (contour d'une région remplie…) ne sont pas dans la vue
    // (absentes du collecteur de la vue dans l'API)
    elements_2d.retain(|e: &Element2D| du_document(e.famille) && ids_vues.contains(&e.vue) && !(e.classe == "CurveElem" && lignes_esquisse.contains(&e.id)));
    elements_2d.sort_by_key(|e| e.id);
    // formes affichées : hors combinaisons (le solide est celui de la combinaison) et
    // hors familles imbriquées (symboles d'annotation)
    let membres: std::collections::HashSet<i64> = formes.iter().flat_map(|f: &Forme| f.membres.iter().copied()).collect();
    formes.retain(|f| !membres.contains(&f.id) && (f.famille == -1 || familles.get(&f.famille).is_some_and(|x: &Famille| x.parent == -1)));
    // volées et paliers rattachés par m_stairsId ; point des instances hébergées par un mur
    for e in &mut escaliers {
        let mut parties: BTreeSet<i64> = e.parties.iter().copied().collect();
        parties.extend(volees.iter().filter(|&(_, &s)| s == e.id).map(|(&v, _)| v));
        parties.retain(|p| volees.contains_key(p));
        e.parties = parties.into_iter().collect();
    }
    let traces: HashMap<i64, Trace> = murs.iter().filter_map(|m| Some((m.id, m.trace?))).collect();
    let instances: Vec<Instance> = instances
        .into_iter()
        .map(|(mut i, hote)| {
            if let (Some(t), Some((h, elevation))) = (traces.get(&i.hote), hote) {
                let q = t.point(h);
                i.point = DVec3::new(q.x, q.y, altitude.get(&i.niveau).copied().unwrap_or(0.0) + elevation);
            }
            i
        })
        .collect();

    // textes, étiquettes et polices des documents de famille embarqués
    let mut textes_familles: HashMap<[u8; 16], Vec<Element2D>> = HashMap::new();
    let mut params_embarques: HashMap<i64, String> = HashMap::new();
    let mut categories_textes: HashMap<i64, i64> = HashMap::new();
    let mut ids_embarques: Vec<&i64> = embarques.keys().collect();
    ids_embarques.sort_unstable();
    for eid in ids_embarques {
        let (doc, body) = &embarques[eid];
        let Ok(r) = champs::read(&sch, body) else { continue };
        if r.root.class_name() == "FontElem" {
            if let Extrait::Police(t, x) = extraire(*eid, Sorte::Police, &r, &altitude, &herite) {
                polices.insert(t, x);
            }
        } else if r.root.class_name() == "ParamElemFamily" {
            if let Some(d) = parametres::definition(&r, &herite) {
                params_embarques.insert(*eid, d.nom);
            }
        } else if matches!(r.root.class_name(), "TextNoteAttributes" | "TagNoteAttributes") {
            if let Some(a) = r.root.get("m_lineAndTextAttr").and_then(Val::obj) {
                categories_textes.insert(*eid, id_of(a, "m_categoryId"));
            }
        } else if r.root.class_name() == "FilledRegionAttributes" {
            types_regions.extend(vues::type_region(&r).map(|x| (*eid, x)));
        } else if r.root.class_name() == "FillPatternElem" {
            motifs.extend(vues::motif(&r).map(|x| (*eid, x)));
        } else if r.root.class_name() == "GStyleElem" {
            if let Extrait::Style(cat, c) = extraire(*eid, Sorte::Style, &r, &altitude, &herite) {
                couleurs_categories.insert(cat, c);
            }
        } else if let Some(e) = vues::element(*eid, &r) {
            textes_familles.entry(*doc).or_default().push(e);
        }
    }
    drop(embarques);
    // dessin 2D des types placés dans une vue et des familles de titre de vue
    let mut types_2d: Vec<i64> = instances_vue.iter().map(|i: &InstanceVue| i.type_id).chain(types_fenetres.values().map(|t: &TypeFenetre| t.famille_titre)).collect();
    types_2d.sort_unstable();
    types_2d.dedup();
    let bib_2d = geometrie::Bibliotheque::dessins(&sch, &geometries_types);
    let dessins: HashMap<i64, geometrie::Dessin> = types_2d.par_iter().filter_map(|&t| Some((t, bib_2d.dessin(t)?))).collect();
    drop(bib_2d);

    // Géométrie, en parallèle.
    let mut a_mailler: Vec<i64> = murs.iter().map(|m| m.id).chain(dalles.iter().map(|d| d.id)).chain(toits.iter().map(|t| t.id)).collect();
    a_mailler.extend(toposolides.iter().map(|t| t.id).chain(surfaces_topo.iter().map(|x| x.id)));
    a_mailler.extend(escaliers.iter().flat_map(|e| e.parties.iter().copied()));
    a_mailler.extend(garde_corps.iter().map(|g| g.id));
    a_mailler.extend(instances.iter().map(|i| i.id).chain(pieces.iter().map(|p| p.id)));
    a_mailler.extend(formes.iter().map(|f| f.id));
    a_mailler.extend(elements_2d.iter().filter(|e| matches!(e.contenu, Contenu::Region { .. })).map(|e| e.id));
    let regions_familles: Vec<i64> = textes_familles.values().flatten().filter(|e| matches!(e.contenu, Contenu::Region { .. })).map(|e| e.id).collect();
    a_mailler.extend(&regions_familles);
    progress(format!("Géométrie de {} éléments…", a_mailler.len()));
    progress(format!("Géométrie de {} types ({} Mo)…", geometries_types.len(), geometries_types.values().map(Vec::len).sum::<usize>() >> 20));
    let bib = geometrie::Bibliotheque::new(&sch, &geometries_types);
    let mut regions: std::collections::HashSet<i64> = elements_2d.iter().filter(|e| matches!(e.contenu, Contenu::Region { .. })).map(|e| e.id).collect();
    regions.extend(regions_familles);
    // une région remplie est une face ouverte (feuille), pas un solide
    let mut geoms: HashMap<i64, Geometrie> = a_mailler.par_iter().map(|&eid| (eid, maillage(&sch, &complements, &bib, eid, regions.contains(&eid)))).collect();
    let mut prendre = |eid: i64| geoms.remove(&eid).unwrap_or_else(|| Err("non maillé".into()));

    let mut m = Maquette {
        niveaux,
        types,
        familles,
        materiaux,
        murs,
        dalles,
        toposolides,
        surfaces_topo,
        toits,
        escaliers,
        garde_corps,
        grilles,
        instances,
        pieces,
        formes,
        categories,
        vues,
        elements_2d,
        motifs,
        types_regions,
        couleurs_categories,
        definitions,
        parametres,
        phases,
        unites,
        categorie_de: HashMap::new(),
        type_de_element: HashMap::new(),
        ordre_phases,
        filtres_phase,
        coordonnees: {
            coord.emplacements.sort_by_key(|e| e.id);
            coord
        },
        famille_document,
        episodes,
        episode_de: if ids8 { table } else { HashMap::new() },
        instances_de_vue,
        instances_vue,
        dessins,
        textes_familles,
        polices,
        params_embarques,
        categories_textes,
        types_fenetres,
        revisions,
        info_projet,
        types_cotes,
        format_longueurs,
        lignes_esquisse,
        illisibles,
    };
    for x in &mut m.murs {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.dalles {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.toposolides {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.surfaces_topo {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.toits {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.instances {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.garde_corps {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.pieces {
        x.geometrie = prendre(x.id);
        // surface et périmètre calculés par Revit (ROOM_AREA, ROOM_PERIMETER) : face
        // inférieure du volume de la pièce
        if let Ok(g) = &x.geometrie {
            let (aire, perimetre) = pied_de_piece(g);
            let v = m.parametres.entry(x.id).or_default();
            v.retain(|(p, _)| *p != -1_006_902 && *p != -1_006_917);
            v.push((-1_006_902, Brut::Reel(aire / (FT * FT))));
            v.push((-1_006_917, Brut::Reel(perimetre / FT)));
        }
    }
    for x in &mut m.formes {
        x.geometrie = prendre(x.id);
    }
    for x in m.elements_2d.iter_mut().chain(m.textes_familles.values_mut().flatten()).filter(|e| matches!(e.contenu, Contenu::Region { .. })) {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.escaliers {
        let mut tout: Option<Maillage> = None;
        let mut erreur = String::from("aucune volée ni palier");
        for &p in &x.parties {
            match prendre(p) {
                Ok(g) => match &mut tout {
                    Some(t) => t.ajouter(g),
                    None => tout = Some(g),
                },
                Err(e) => erreur = format!("partie {p} : {e}"),
            }
        }
        x.geometrie = tout.ok_or(erreur);
    }
    m.indexer();
    Ok(m)
}

/// Maillage du complément 0x67 d'un élément, géométrie de ses familles comprise ;
/// `feuilles` : garder aussi les faces ouvertes (régions remplies).
fn maillage(sch: &Schema, complements: &HashMap<i64, Vec<u8>>, bib: &geometrie::Bibliotheque, eid: i64, feuilles: bool) -> Geometrie {
    let b = complements.get(&eid).ok_or_else(|| format!("élément {eid} sans enregistrement 0x67"))?;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| champs::read(sch, b).and_then(|rec| geometrie::maillage(&rec, feuilles, bib))))
        .unwrap_or_else(|_| Err("erreur interne".into()))
        .and_then(|g| if g.faces.is_empty() { Err("aucun solide fermé dans le GElement".into()) } else { Ok(g) })
}

/// Surface (mm²) et périmètre (mm) d'une pièce : triangles horizontaux tournés vers le
/// bas au plus bas du volume, et longueur de leurs arêtes non partagées.
fn pied_de_piece(g: &Maillage) -> (f64, f64) {
    let zmin = g.sommets.iter().map(|p| p.z).fold(f64::MAX, f64::min);
    let mut aire = 0.0;
    let mut aretes: HashMap<(usize, usize), usize> = HashMap::new();
    for f in &g.faces {
        for t in &f.triangles {
            let [a, b, c] = t.map(|i| g.sommets[i]);
            let n = (b - a).cross(c - a);
            if n.z < 0.0 && n.z.abs() > 0.999 * n.length() && (a.z - zmin).abs() < 1.0 {
                aire += n.length() / 2.0;
                for (i, j) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    *aretes.entry((i.min(j), i.max(j))).or_default() += 1;
                }
            }
        }
    }
    let perimetre = aretes.iter().filter(|e| *e.1 == 1).map(|e| g.sommets[e.0.0].distance(g.sommets[e.0.1])).sum();
    (aire, perimetre)
}

/// Élément placé : hors d'un groupe non placé.
fn place(o: &Obj) -> bool {
    o.get("m_unplacedOwnerId").and_then(Val::element_id).is_none_or(|g| g == -1)
}

fn id_of(o: &Obj, name: &str) -> i64 {
    o.get(name).and_then(Val::element_id).unwrap_or(-1)
}

fn reel(o: &Obj, name: &str) -> Option<f64> {
    o.get(name).and_then(Val::real)
}

fn vec3(v: &Val) -> Option<DVec3> {
    match v.reals().as_deref() {
        Some(&[x, y, z]) => Some(DVec3::new(x, y, z)),
        _ => None,
    }
}

/// Valeurs des jeux de paramètres double, entier, chaîne et ElementId, par id de
/// paramètre.
fn params<'r, 's>(r: &'r Record<'s>, o: &Obj<'s>) -> HashMap<i64, &'r Val<'s>> {
    let mut out = HashMap::new();
    for k in ["m_pParamValueSetDouble", "m_pParamValueSetInt", "m_pParamValueSetAString", "m_pParamValueSetElementId"] {
        let Some(ps) = r.field_body(o, k) else { continue };
        for x in ps.get("m_paramSet").map(Val::list).unwrap_or_default() {
            if let Some((id, v)) = x.obj().and_then(|x| Some((x.get("m_paramId").and_then(Val::element_id)?, x.get("m_value")?))) {
                out.insert(id, v);
            }
        }
    }
    out
}

fn reel_p(p: &HashMap<i64, &Val>, k: i64) -> f64 {
    p.get(&k).and_then(|v| v.real()).unwrap_or(0.0)
}

fn texte_p(p: &HashMap<i64, &Val>, k: i64) -> String {
    p.get(&k).and_then(|v| v.str()).unwrap_or_default().to_string()
}

/// Cellules de données de l'élément (`Element.m_cellList`).
fn cellules<'r, 's>(r: &'r Record<'s>, o: &Obj<'s>) -> Vec<&'r Obj<'s>> {
    let Some(cl) = r.field_body(o, "m_cellList") else { return Vec::new() };
    cl.get("m_cells").map(Val::list).unwrap_or_default().iter().filter_map(|c| r.deref(c).map(|b| &b.obj)).collect()
}

impl Trace {
    fn lire(c: &Obj) -> Option<Trace> {
        let v = |name| c.get(name).and_then(vec3);
        match c.class_name() {
            "GLine" => Some(Trace::Ligne { origine: v("m_origin")? * FT, direction: v("m_dirVec")? * FT }),
            "GArc" => Some(Trace::Arc { centre: v("m_center")? * FT, rayon: reel(c, "m_radius")? * FT, x: v("m_xVec")?, y: v("m_yVec")? }),
            _ => None,
        }
    }

    /// Point de paramètre t, en mm.
    pub fn point(&self, t: f64) -> DVec3 {
        match *self {
            Trace::Ligne { origine, direction } => origine + t * direction,
            Trace::Arc { centre, rayon, x, y } => centre + rayon * (t.cos() * x + t.sin() * y),
        }
    }
}

fn mur(id: i64, r: &Record, altitude: &HashMap<i64, f64>) -> Mur {
    let o = &r.root;
    let p = params(r, o);
    let mut m = Mur {
        id,
        classe: o.class_name().to_string(),
        type_id: id_of(o, "m_WallAttributesId"),
        niveau: id_of(o, "m_assocLevelId"),
        niveau_haut: id_of(o, "m_upToLevelId"),
        hauteur: reel_p(&p, WALL_USER_HEIGHT_PARAM) * FT,
        decalage_bas: reel_p(&p, WALL_BASE_OFFSET) * FT,
        decalage_haut: reel_p(&p, WALL_TOP_OFFSET) * FT,
        trace: r.field_body(o, "m_pCurveDriver").and_then(|d| r.field_body(d, "m_pCrv")).and_then(Trace::lire),
        geometrie: Err(String::new()),
    };
    if let (Some(h), Some(b)) = (altitude.get(&m.niveau_haut), altitude.get(&m.niveau)) {
        m.hauteur = h + m.decalage_haut - b - m.decalage_bas;
    }
    m
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

fn niveau(id: i64, r: &Record) -> Option<Niveau> {
    let z = r.field_body(&r.root, "m_pSurface")?.get("m_origin")?.reals()?.get(2).copied()?;
    Some(Niveau { id, nom: r.root.get("m_text")?.str()?.to_string(), altitude: z * FT })
}

fn extraire(eid: i64, s: Sorte, r: &Record, altitude: &HashMap<i64, f64>, herite: &dyn Fn(&str, &str) -> bool) -> Extrait {
    let o = &r.root;
    let ou_rien = |x: Option<Extrait>| x.unwrap_or(Extrait::Rien);
    match s {
        Sorte::Niveau => Extrait::Rien,
        Sorte::Type => ou_rien((|| {
            let nom = r.field_body(o, "m_symbolInfo")?.get("m_name")?.str()?.to_string();
            let epaisseur = r.field_body(o, "m_pCompoundStructure").map(|cs| {
                let couches = cs.get("m_layers").map(Val::list).unwrap_or_default();
                couches.iter().filter_map(|c| c.obj()?.get("m_layerWidth")?.real()).sum::<f64>() * FT
            });
            let mut valeurs = HashMap::new();
            if let Some(pp) = r.field_body(o, "m_pParams") {
                for x in pp.get("m_params").map(Val::list).unwrap_or_default() {
                    if let Some((id, v)) = x.obj().and_then(|x| Some((x.get("m_paramId").and_then(Val::element_id)?, x.get("m_value").and_then(Val::real)?))) {
                        valeurs.insert(id, v * FT);
                    }
                }
            }
            let (largeur, hauteur) = (valeurs.get(&FAMILY_WIDTH_PARAM).copied(), valeurs.get(&FAMILY_HEIGHT_PARAM).copied());
            let categorie_texte = o.get("m_lineAndTextAttr").and_then(Val::obj).map(|a| id_of(a, "m_categoryId")).unwrap_or(-1);
            Some(Extrait::Type(eid, TypeElem { nom, classe: o.class_name().to_string(), epaisseur, famille: id_of(o, "m_familyId"), largeur, hauteur, categorie_texte }))
        })()),
        Sorte::Famille => ou_rien((|| {
            let occurrence = r.field_body(o, "m_familyParams").map(|fp| {
                let liste = fp.get("m_params").map(Val::list).unwrap_or_default();
                liste.iter().filter_map(Val::obj).filter(|x| x.get("m_instance").and_then(Val::int) == Some(1)).filter_map(|x| x.get("m_paramId").and_then(Val::element_id)).collect()
            });
            let formules = r
                .field_body(o, "m_familyParams")
                .map(|fp| {
                    let liste = fp.get("m_params").map(Val::list).unwrap_or_default();
                    liste
                        .iter()
                        .filter_map(Val::obj)
                        .filter_map(|x| {
                            let ex = x.get("m_oExpression")?;
                            Some((x.get("m_paramId").and_then(Val::element_id)?, super::formules::lire(r, r.deref(ex).map(|b| &b.obj).or(ex.obj())?)?))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let document = r.field_body(o, "m_oFamDoc").and_then(|d| d.get("m_contentDocGUID")?.obj()?.get("m_guid")).and_then(|g| match g {
                Val::Guid(g) => Some(*g),
                _ => None,
            });
            let f = Famille { nom: o.get("m_name")?.str()?.to_string(), categorie: id_of(o, "m_categoryId"), parent: id_of(o, "m_famId"), occurrence: occurrence.unwrap_or_default(), document, formules };
            Some(Extrait::Famille(eid, f))
        })()),
        Sorte::Materiau => ou_rien((|| {
            let m = r.field_body(o, "m_pMaterial")?;
            // COLORREF : rouge dans l'octet de poids faible
            let c = m.get("m_color").and_then(Val::int).unwrap_or(0x7F7F7F);
            let couleur = [c as u8, (c >> 8) as u8, (c >> 16) as u8];
            let motif = |id: &str, c: &str| {
                let c = m.get(c).and_then(Val::int).unwrap_or(0);
                (id_of(m, id), [c as u8, (c >> 8) as u8, (c >> 16) as u8])
            };
            let motifs = [
                motif("m_cutPatternId", "m_cutPatternColor"),
                motif("m_cutBackgroundPatternId", "m_cutBackgroundPatternColor"),
                motif("m_surfacePatternId", "m_surfacePatternColor"),
                motif("m_surfaceBackgroundPatternId", "m_surfaceBackgroundPatternColor"),
            ];
            Some(Extrait::Materiau(eid, Materiau { nom: m.get("m_name")?.str()?.to_string(), couleur, transparence: m.get("m_transparency").and_then(Val::real).unwrap_or(0.0), motifs }))
        })()),
        Sorte::Mur => Extrait::Mur(mur(eid, r, altitude)),
        Sorte::Dalle => {
            let (champ_type, k) = if o.class_name() == "Ceiling" { ("m_ceilingAttributesId", CEILING_HEIGHTABOVELEVEL_PARAM) } else { ("m_floorAttributesId", FLOOR_HEIGHTABOVELEVEL_PARAM) };
            let p = params(r, o);
            Extrait::Dalle(Dalle { id: eid, classe: o.class_name().to_string(), type_id: id_of(o, champ_type), niveau: id_of(o, "m_assocLevelId"), decalage: reel_p(&p, k) * FT, geometrie: Err(String::new()) })
        }
        Sorte::Toposolide => {
            let p = params(r, o);
            Extrait::Toposolide(Dalle { id: eid, classe: o.class_name().to_string(), type_id: id_of(o, "m_TypeId"), niveau: id_of(o, "m_assocLevelId"), decalage: reel_p(&p, TOPOSOLID_HEIGHTABOVELEVEL_PARAM) * FT, geometrie: Err(String::new()) })
        }
        Sorte::SurfaceTopo => {
            // points saisis : sommets fixes du premier état de la triangulation
            // (`m_drivers[0].m_pMementos[0]`) ; l'état suivant d'une surface hôte ajoute
            // les intersections avec le contour de ses sous-régions
            let points = (|| {
                let d = r.deref(o.get("m_drivers")?.list().first()?)?;
                let m = r.deref(d.obj.get("m_pMementos")?.list().first()?)?;
                Some(m.obj.get("m_fixedVerts")?.list().len())
            })()
            .unwrap_or(0);
            Extrait::SurfaceTopo(SurfaceTopo { id: eid, hote: id_of(o, "m_hostId"), points, geometrie: Err(String::new()) })
        }
        Sorte::Toit => {
            let p = params(r, o);
            Extrait::Toit(Toit { id: eid, classe: o.class_name().to_string(), type_id: id_of(o, "m_roofAttributesId"), niveau: id_of(o, "m_assocLevelId"), decalage: reel_p(&p, ROOF_LEVEL_OFFSET_PARAM) * FT, geometrie: Err(String::new()) })
        }
        Sorte::Escalier => Extrait::Escalier(escalier(eid, r, altitude)),
        Sorte::Volee => Extrait::Volee(eid, id_of(o, "m_stairsId")),
        Sorte::GardeCorps => Extrait::GardeCorps(GardeCorps { id: eid, type_id: id_of(o, "m_stairsRailingAttrId"), niveau: id_of(o, "m_assocLevelId"), geometrie: Err(String::new()) }),
        Sorte::Grille => ou_rien(grille(eid, r).map(Extrait::Grille)),
        Sorte::Instance if id_of(o, "m_ownerDBViewId") != -1 => ou_rien((|| {
            let t = r.field_body(o, "m_pInstanceInfo")?.get("m_Trf")?.obj()?;
            let l = t.get("m_3x3")?.list();
            let ligne = |i: usize| l.get(i).and_then(vec3);
            // la colonne j de m_3x3 est l'axe j (geometrie::Trf)
            let axes = DMat3::from_cols(ligne(0)?, ligne(1)?, ligne(2)?).transpose();
            Some(Extrait::InstanceDeVue(InstanceVue { id: eid, vue: id_of(o, "m_ownerDBViewId"), type_id: id_of(o, "m_masterSymbolId"), origine: t.get("m_or").and_then(vec3)? * FT, axes }))
        })()),
        Sorte::Instance => {
            let (i, h) = instance(eid, r);
            Extrait::Instance(i, h)
        }
        Sorte::Piece => Extrait::Piece(piece(eid, r, altitude)),
        Sorte::Forme => ou_rien(forme(eid, r).map(Extrait::Forme)),
        Sorte::Categorie => ou_rien((|| Some(Extrait::Categorie(eid, r.field_body(o, "m_pCategory")?.get("m_name")?.str()?.to_string())))()),
        Sorte::Vue => ou_rien(vues::vue(eid, r).map(Extrait::Vue)),
        Sorte::Viewer => ou_rien(vues::cadrage(r).map(|(v, o, a, b)| Extrait::Cadrage(v, (o, a, b), vues::cadre(r).map(|c| c.1)))),
        Sorte::Annotation | Sorte::Region => ou_rien(vues::element(eid, r).map(Extrait::Element2D)),
        Sorte::Motif => ou_rien(vues::motif(r).map(|x| Extrait::Motif(eid, x))),
        Sorte::TypeRegion => ou_rien(vues::type_region(r).map(|x| Extrait::TypeRegion(eid, x))),
        Sorte::Style => ou_rien((|| {
            // style de projection (type 1) : couleur COLORREF, rouge dans l'octet faible
            if o.get("m_gstyleType").and_then(Val::int) != Some(1) {
                return None;
            }
            let c = r.field_body(o, "m_pGStyle")?.get("m_color")?.int()?;
            Some(Extrait::Style(id_of(o, "m_categoryId"), [c as u8, (c >> 8) as u8, (c >> 16) as u8]))
        })()),
        Sorte::Parametre => ou_rien(parametres::definition(r, herite).map(|d| Extrait::Parametre(eid, d))),
        Sorte::Phase if o.class_name() == "AllProjectPhases" => Extrait::OrdrePhases(o.get("m_phaseIds").map(Val::list).unwrap_or_default().iter().filter_map(Val::element_id).collect()),
        Sorte::Phase if o.class_name() == "PhaseFilterElem" => {
            let mut pres = [0u8; 7];
            for (k, v) in o.get("m_phaseStatusPresentation").map(Val::list).unwrap_or_default().iter().take(7).enumerate() {
                pres[k] = v.int().unwrap_or(0) as u8;
            }
            Extrait::FiltrePhase(eid, o.get("m_name").and_then(Val::str).unwrap_or_default().to_string(), pres)
        }
        Sorte::Phase => ou_rien(o.get("m_name").and_then(Val::str).map(|n| Extrait::Phase(eid, n.to_string()))),
        Sorte::Coordonnee => ou_rien(coordonnees::lire(eid, r).map(Extrait::Coordonnee)),
        Sorte::Police => ou_rien((|| {
            let f = r.field_body(o, "m_pFont")?;
            Some(Extrait::Police(id_of(o, "m_ownerId"), Police { nom: f.get("m_name")?.str()?.to_string(), taille: f.get("m_size")?.real()? * FT }))
        })()),
        Sorte::TypeFenetre => ou_rien((|| {
            let nom = r.field_body(o, "m_symbolInfo")?.get("m_name")?.str()?.to_string();
            let oui = |k| o.get(k).and_then(Val::int) == Some(1);
            Some(Extrait::TypeFenetre(eid, TypeFenetre { nom, titre: oui("m_showLabel"), famille_titre: id_of(o, "m_familyTagId"), ligne: oui("m_showExtensionLine") }))
        })()),
        Sorte::Revision => {
            let t = |k| o.get(k).and_then(Val::str).unwrap_or_default().to_string();
            let sequence = o.get("m_sequenceNumber").and_then(Val::int).unwrap_or(0);
            Extrait::Revision(eid, Revision { description: t("m_description"), date: t("m_date"), numero: t("m_numberStr"), sequence, remise_a: t("m_issuedTo"), remise_par: t("m_issuedBy") })
        }
        Sorte::InfoProjet => Extrait::InfoProjet(eid),
        Sorte::TypeCote => ou_rien(vues::type_cote(r).map(|t| Extrait::TypeCote(eid, t))),
        Sorte::Unites => Extrait::Unites(vues::format_projet(r), parametres::Unites::lire(r).unwrap_or_default()),
        Sorte::Esquisse => {
            let d = r.field_body(o, "m_elemIdsPairSet").and_then(|p| p.get("m_data")).map(Val::list).unwrap_or_default();
            Extrait::Esquisse(d.iter().filter_map(|x| x.obj().map(|x| id_of(x, "m_elementId"))).collect())
        }
    }
}

/// Forme pleine ou combinaison ; None pour un vide (`GenSweep.m_cutting` = 1).
fn forme(id: i64, r: &Record) -> Option<Forme> {
    let o = &r.root;
    if o.get("m_cutting").and_then(Val::int) == Some(1) {
        return None;
    }
    let mut membres = Vec::new();
    for x in o.get("m_svMap").map(Val::list).unwrap_or_default() {
        let Some(x) = x.obj() else { continue };
        membres.extend(x.get("first").and_then(Val::element_id));
        let vides = x.get("second").and_then(Val::obj).and_then(|s| s.get("m_set")).map(Val::list).unwrap_or_default();
        membres.extend(vides.iter().filter_map(Val::element_id));
    }
    Some(Forme {
        id,
        classe: o.class_name().to_string(),
        sous_categorie: id_of(o, "m_categoryId"),
        materiau: id_of(o, "m_materialId"),
        famille: id_of(o, "m_famId"),
        niveau: id_of(o, "m_assocLevelId"),
        membres,
        geometrie: Err(String::new()),
    })
}

/// Volées et paliers listés par l'escalier (`m_runAndLandings` de ses cellules,
/// `m_runsAndLandings`) ; `lire` y ajoute les parties dont `m_stairsId` le désigne.
fn escalier(id: i64, r: &Record, altitude: &HashMap<i64, f64>) -> Escalier {
    let o = &r.root;
    let mut parties = BTreeSet::new();
    for c in cellules(r, o).into_iter().chain([o]) {
        for k in ["m_runAndLandings", "m_runsAndLandings"] {
            parties.extend(c.get(k).map(Val::list).unwrap_or_default().iter().filter_map(Val::element_id));
        }
    }
    let (bas, haut) = (id_of(o, "m_baseLevelId"), id_of(o, "m_topLevelId"));
    let hauteur = match (altitude.get(&bas), altitude.get(&haut)) {
        (Some(b), Some(h)) => h + reel(o, "m_topLevelOffset").unwrap_or(0.0) * FT - b - reel(o, "m_baseLevelOffset").unwrap_or(0.0) * FT,
        _ => reel(o, "m_stairsHeight").unwrap_or(0.0) * FT,
    };
    Escalier {
        id,
        type_id: id_of(o, "m_typeId"),
        niveau_bas: bas,
        niveau_haut: haut,
        contremarches: o.get("m_actualNumberOfRisers").and_then(Val::int).unwrap_or(0),
        hauteur_contremarche: reel(o, "m_actualRiserHeight").unwrap_or(0.0) * FT,
        giron: reel(o, "m_actualTreadDepth").unwrap_or(0.0) * FT,
        hauteur,
        parties: parties.into_iter().collect(),
        geometrie: Err(String::new()),
    }
}

fn grille(id: i64, r: &Record) -> Option<Grille> {
    let o = &r.root;
    let s = r.field_body(o, "m_pSurface")?;
    let env = s.get("m_Envelope")?.obj()?.get("m_corners")?.list();
    let (u0, u1) = (env.first()?.reals()?.first().copied()?, env.get(1)?.reals()?.first().copied()?);
    let debut = geometrie::point(r, s, u0, 0.0).ok()? * FT;
    let fin = geometrie::point(r, s, u1, 0.0).ok()? * FT;
    let arc = if s.class_name() == "CylSurf" { Some((s.get("m_center").and_then(vec3)? * FT, reel(s, "m_radius")? * FT)) } else { None };
    Some(Grille { id, nom: o.get("m_text")?.str()?.to_string(), debut, fin, arc })
}

/// Instance, son point valant `m_instOrigin` ; pour une instance hébergée, aussi
/// (`m_hostParam`, `m_elevation` en mm), le point étant replacé par `lire` sur la ligne
/// du mur hôte.
fn instance(id: i64, r: &Record) -> (Instance, Option<(f64, f64)>) {
    let o = &r.root;
    let p = params(r, o);
    let mut repere = texte_p(&p, ALL_MODEL_MARK);
    if repere.is_empty() {
        // les portes rangent leur repère dans FamInstSpec.m_doorNumber
        repere = r.field_body(o, "m_oFamInstSpec").and_then(|s| s.get("m_doorNumber")?.str()).unwrap_or_default().to_string();
    }
    let i = Instance {
        id,
        type_id: id_of(o, "m_masterSymbolId"),
        hote: id_of(o, "m_hostId"),
        niveau: id_of(o, "m_assocLevelId"),
        point: o.get("m_instOrigin").and_then(vec3).unwrap_or_default() * FT,
        repere,
        geometrie: Err(String::new()),
    };
    (i, reel(o, "m_hostParam").map(|h| (h, reel(o, "m_elevation").unwrap_or(0.0) * FT)))
}

fn piece(id: i64, r: &Record, altitude: &HashMap<i64, f64>) -> Piece {
    let o = &r.root;
    let p = params(r, o);
    let niveau = id_of(o, "m_assocLevelId");
    // m_point : (x, y) en pieds, à l'altitude du niveau
    let point = match o.get("m_point").and_then(Val::reals).as_deref() {
        Some(&[x, y]) => Some(DVec3::new(x * FT, y * FT, altitude.get(&niveau).copied().unwrap_or(0.0))),
        _ => None,
    };
    let genre = if id_of(o, "m_areaSchemeId") != -1 {
        "surface"
    } else if id_of(o, "m_zoneSchemeId") != -1 {
        "zone"
    } else {
        "piece"
    };
    let place = o.get("m_bIsLocationless").and_then(Val::int) != Some(1);
    Piece {
        id,
        genre,
        nom: texte_p(&p, ROOM_NAME),
        numero: texte_p(&p, ROOM_NUMBER),
        service: texte_p(&p, ROOM_DEPARTMENT),
        niveau,
        point: point.filter(|_| place),
        geometrie: Err(String::new()),
    }
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

    /// Type d'un élément lu.
    pub fn type_de(&self, element: i64) -> Option<i64> {
        self.type_de_element.get(&element).copied()
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

/// GlobalId synthétique de 22 caractères, pour les entités qui ne sont pas des éléments
/// Revit (jeux de propriétés, relations…) ou un élément sans IfcGUID connu.
fn gid(prefix: &str, id: i64) -> Value {
    Value::Str(format!("{prefix}{:0>width$}", id, width = 22 - prefix.len()))
}

fn mm(v: f64) -> Value {
    Value::Str(format!("{} mm", format_real((v * 10.0).round() / 10.0)))
}

fn point_mm(p: DVec3) -> Value {
    Value::Str(format!("{} ; {} ; {} mm", format_real((p.x * 10.0).round() / 10.0), format_real((p.y * 10.0).round() / 10.0), format_real((p.z * 10.0).round() / 10.0)))
}

/// Couleur d'un matériau dans la scène (transparence plafonnée pour que les faces
/// « vides » restent visibles).
fn couleur(m: &Materiau) -> Rgba {
    let [r, g, b] = m.couleur.map(|c| c as f32 / 255.0);
    [r, g, b, (1.0 - m.transparence as f32).clamp(0.1, 1.0)]
}

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

impl Builder {
    fn add(&mut self, ty: &str, args: Vec<Value>) -> Id {
        let id = self.entities.len() as Id + 1;
        self.entities.insert(id, Entity { ty: Arc::from(ty), args });
        id
    }

    fn pset(&mut self, prefix: &str, owner: i64, name: &str, props: Vec<(&str, Value)>) -> Id {
        let refs = props.into_iter().map(|(n, v)| Value::Ref(self.add("IFCPROPERTYSINGLEVALUE", vec![Value::Str(n.into()), Value::Null, v]))).collect();
        self.add("IFCPROPERTYSET", vec![gid(prefix, owner), Value::Null, Value::Str(name.into()), Value::Null, Value::List(refs)])
    }

    /// Un jeu de propriétés par groupe de paramètres de Revit (« Contraintes », « Cotes »…).
    fn psets_parametres(&mut self, m: &Maquette, element: i64) -> Vec<Id> {
        let mut out = Vec::new();
        let aff = m.parametres_affiches(element);
        let mut k = 0;
        for groupe in aff.chunk_by(|a, b| a.groupe == b.groupe) {
            let props: Vec<(&str, Value)> = groupe.iter().map(|a| (a.nom.as_str(), Value::Str(a.valeur.clone()))).collect();
            out.push(self.pset(&format!("RVTG{k:02}"), element, groupe[0].groupe, props));
            k += 1;
        }
        out
    }

    /// Rattache des jeux de propriétés à une entité.
    fn definir(&mut self, prefix: &str, element: i64, entite: Id, psets: Vec<Id>) {
        for (k, p) in psets.into_iter().enumerate() {
            self.add("IFCRELDEFINESBYPROPERTIES", vec![gid(&format!("{prefix}{k:02}"), element), Value::Null, Value::Null, Value::Null, Value::List(vec![Value::Ref(entite)]), Value::Ref(p)]);
        }
    }
}

/// Tracé d'une région remplie : arrière-plan uni s'il y en a un, puis premier plan uni
/// (face colorée) ou hachures (traits de 0,13 mm sur le papier, découpés sur la face) ;
/// région sans type connu : face grise translucide.
fn region(m: &Maquette, type_id: i64, g: &Maillage, v: &Vue, element: Option<Id>) -> Vec<Primitive> {
    let face: Vec<[DVec3; 3]> = g.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| g.sommets[i]))).collect();
    let en_m = |tris: &[[DVec3; 3]]| tris.iter().map(|t| t.map(|p| p / 1000.0)).collect::<Vec<_>>();
    let plein = |couleur: [u8; 4]| Primitive { forme: trace2d::Forme::Triangles(en_m(&face)), style: Style { couleur, epaisseur: 0.0 }, role: Role::Annotation, element };
    let Some(t) = m.types_regions.get(&type_id) else { return vec![plein([115, 115, 115, 153])] };
    let avant = m.couleurs_categories.get(&t.categorie).copied().unwrap_or([0, 0, 0]);
    let avant = [avant[0], avant[1], avant[2], 255];
    let normale = v.droite.cross(v.haut).normalize_or_zero();
    let mut prims = Vec::new();
    match m.motifs.get(&t.arriere) {
        Some(x) if x.grilles.is_empty() => prims.push(plein([t.couleur_arriere[0], t.couleur_arriere[1], t.couleur_arriere[2], 255])),
        _ => prims.push(plein([217, 217, 217, 64])),
    }
    match m.motifs.get(&t.avant) {
        Some(x) if x.grilles.is_empty() => prims.push(plein(avant)),
        Some(x) => {
            // traits légèrement décollés de la face, vers l'observateur
            let ecart = normale * (0.05 * v.echelle);
            let s: Vec<[DVec3; 2]> = vues::hachures(&face, x, v.echelle, v.droite, v.haut, 20_000).into_iter().map(|s| s.map(|p| (p + ecart) / 1000.0)).collect();
            if !s.is_empty() {
                prims.push(Primitive { forme: trace2d::Forme::Segments(s), style: Style { couleur: avant, epaisseur: 0.13 }, role: Role::Hachure, element });
            }
        }
        None => {}
    }
    prims
}

/// Couleur d'une partie (0..1) en octets.
fn octets(c: Rgba) -> [u8; 4] {
    c.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Primitives déjà triangulées d'un élément (glyphes, traits des cotes et des cartouches).
fn primitives_parts(parts: Vec<Part>, element: Option<Id>) -> Vec<Primitive> {
    parts.into_iter().map(|p| Primitive { forme: trace2d::Forme::Triangles(p.tris), style: Style { couleur: octets(p.color), epaisseur: 0.0 }, role: Role::Annotation, element }).collect()
}

/// Parties à afficher (triangles par couleur) des primitives d'une vue de normale `normale`
/// à l'échelle `echelle`.
fn parts_primitives(prims: &[Primitive], normale: DVec3, echelle: f64) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    for p in prims {
        let m = p.mailler(normale, echelle);
        // traits fins : rubans de 0,1 mm (le tampon des traits d'un pixel n'est pas
        // celui des objets 2D)
        let mut tris = m.triangles;
        if !m.traits.is_empty() {
            let fin = Primitive { forme: trace2d::Forme::Segments(m.traits), style: Style { couleur: p.style.couleur, epaisseur: 0.1 }, ..p.clone() };
            tris.extend(fin.mailler(normale, echelle).triangles);
        }
        if tris.is_empty() {
            continue;
        }
        let color = p.style.couleur.map(|c| c as f32 / 255.0);
        match parts.last_mut() {
            Some(q) if q.color == color => q.tris.extend(tris),
            _ => parts.push(Part { color, tris }),
        }
    }
    parts
}

/// Propriétés « Coordonnées » du projet, comme la boîte « Emplacement » et les points de
/// base de Revit : emplacement actif, position partagée du point de base du projet et
/// angle par rapport au nord géographique, point topographique, lieu, autres
/// emplacements.
fn coordonnees_affichees(m: &Maquette) -> Vec<(String, String)> {
    let c = &m.coordonnees;
    let Some(e) = c.actif() else { return Vec::new() };
    let metres = |v: f64| format!("{} m", format_real(v.round() / 1000.0));
    let degres = |a: f64| format!("{}°", format_real((a.to_degrees() * 100.0).round() / 100.0));
    let mut out = vec![("Emplacement actif".to_string(), e.nom.clone())];
    let base = c.point_base.unwrap_or_default();
    let s = e.vers_partage(base);
    out.push(("Point de base du projet : E/O".into(), metres(s.x)));
    out.push(("Point de base du projet : N/S".into(), metres(s.y)));
    out.push(("Point de base du projet : Élév.".into(), metres(s.z)));
    // paramètre « Angle par rapport au nord géographique » du point de base
    out.push(("Angle par rapport au nord géographique".into(), degres((-e.angle()).rem_euclid(std::f64::consts::TAU))));
    if base.length() > 1e-6 {
        out.push(("Point de base du projet (repère interne)".into(), format!("{} ; {} ; {} mm", format_real(base.x.round()), format_real(base.y.round()), format_real(base.z.round()))));
    }
    if let Some(t) = c.point_topo {
        out.push(("Point topographique (repère interne)".into(), format!("{} ; {} ; {} mm", format_real(t.x.round()), format_real(t.y.round()), format_real(t.z.round()))));
    }
    if let Some(l) = c.lieux.get(&e.lieu) {
        if !l.nom.is_empty() {
            out.push(("Lieu".into(), l.nom.clone()));
        }
        out.push(("Latitude".into(), format!("{:.6}°", l.latitude)));
        out.push(("Longitude".into(), format!("{:.6}°", l.longitude)));
        if !l.systeme.is_empty() {
            let code = l.epsg.map(|c| format!(" (EPSG {c})")).unwrap_or_default();
            out.push(("Système de coordonnées".into(), format!("{}{code}", l.systeme)));
        }
        out.push(("Fuseau horaire".into(), format!("UTC{}{}", if l.fuseau < 0.0 { "−" } else { "+" }, format_real(l.fuseau.abs()))));
        if !l.station.is_empty() {
            out.push(("Station météo".into(), l.station.clone()));
        }
    }
    for o in c.nommes().filter(|o| o.id != e.id) {
        let p = o.vers_partage(DVec3::ZERO);
        out.push((format!("Emplacement « {} »", o.nom), format!("E/O {}, N/S {}, Élév. {}, {}", metres(p.x), metres(p.y), metres(p.z), degres(o.angle()))));
    }
    out
}

/// Lit le fichier et construit la scène du viewer. Erreur si aucun élément n'a pu être
/// maillé.
pub fn load_scene(path: &Path, titre: &str, format: &str, progress: &dyn Fn(String)) -> Result<Scene, String> {
    let t0 = std::time::Instant::now();
    let m = lire(path, progress)?;
    let ko = |v: &mut dyn Iterator<Item = &Geometrie>| v.filter(|g| g.is_err()).count();
    let murs_ko = ko(&mut m.murs.iter().map(|x| &x.geometrie));
    let mailles = m.murs.len() - murs_ko
        + m.dalles.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.toposolides.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.surfaces_topo.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.toits.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.escaliers.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.instances.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.formes.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.elements_2d.iter().filter(|x| !matches!(x.contenu, Contenu::FenetreDeVue { .. })).count();
    if mailles == 0 {
        return Err(match m.murs.len() {
            0 => "aucun mur, sol, toit, escalier, famille, forme ni élément 2D dans le fichier".into(),
            n => format!("aucun des {n} murs n'a pu être maillé"),
        });
    }
    progress("Assemblage des tampons…".into());
    let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut s = scene(&m, titre, file_name, format);
    let pieces: Vec<&Piece> = m.pieces.iter().filter(|p| p.genre == "piece" && p.point.is_some()).collect();
    let toutes = m.murs.iter().map(|x| &x.geometrie).chain(m.dalles.iter().map(|x| &x.geometrie)).chain(m.toits.iter().map(|x| &x.geometrie));
    let toutes = toutes.chain(m.toposolides.iter().map(|x| &x.geometrie)).chain(m.surfaces_topo.iter().map(|x| &x.geometrie));
    let toutes = toutes.chain(m.escaliers.iter().map(|x| &x.geometrie)).chain(m.instances.iter().map(|x| &x.geometrie)).chain(m.garde_corps.iter().map(|x| &x.geometrie));
    let toutes = toutes.chain(m.formes.iter().map(|x| &x.geometrie));
    let solides_ko: usize = toutes.filter_map(|g| g.as_ref().ok()).map(|g| g.echecs.len()).sum();
    for (nom, n) in [
        ("Murs Revit non maillés", murs_ko),
        ("Sols et plafonds Revit non maillés", ko(&mut m.dalles.iter().map(|x| &x.geometrie))),
        ("Toposolides Revit non maillés", ko(&mut m.toposolides.iter().map(|x| &x.geometrie))),
        ("Surfaces topographiques Revit non maillées", ko(&mut m.surfaces_topo.iter().map(|x| &x.geometrie))),
        ("Toits Revit non maillés", ko(&mut m.toits.iter().map(|x| &x.geometrie))),
        ("Escaliers Revit non maillés", ko(&mut m.escaliers.iter().map(|x| &x.geometrie))),
        ("Garde-corps Revit non maillés", ko(&mut m.garde_corps.iter().map(|x| &x.geometrie))),
        ("Pièces Revit sans volume", ko(&mut pieces.iter().map(|x| &x.geometrie))),
        ("Familles Revit sans géométrie 3D", ko(&mut m.instances.iter().map(|x| &x.geometrie))),
        ("Formes de famille sans solide (entièrement évidées)", ko(&mut m.formes.iter().map(|x| &x.geometrie))),
        ("Solides Revit non décodés dans des éléments affichés", solides_ko),
        ("Objets Revit illisibles", m.illisibles.len()),
    ] {
        if n > 0 {
            s.unsupported.push((nom.into(), n));
        }
    }
    s.load_ms = t0.elapsed().as_millis();
    Ok(s)
}

/// Produits de la scène : murs, sols, plafonds, toits, escaliers, garde-corps, instances
/// de familles, formes d'un document de famille, pièces placées et grilles.
fn produits(m: &Maquette) -> Vec<Produit<'_>> {
    let nom_niveau: HashMap<i64, &str> = m.niveaux.iter().map(|l| (l.id, l.nom.as_str())).collect();
    let niveau = |l: i64| nom_niveau.get(&l).map(|s| s.to_string()).unwrap_or_else(|| l.to_string());
    let nom_type = |t: i64| m.types.get(&t).map(|t| t.nom.clone()).unwrap_or_else(|| format!("Type {t}"));
    let mut out = Vec::new();
    for x in &m.murs {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: "IFCWALL",
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau,
            props: vec![
                ("Classe Revit", Value::Str(x.classe.clone())),
                ("Type", Value::Str(format!("{nt} [{}]", x.type_id))),
                ("Contrainte inférieure", Value::Str(niveau(x.niveau))),
                ("Contrainte supérieure", Value::Str(if x.niveau_haut == -1 { "Non contraint".into() } else { niveau(x.niveau_haut) })),
                ("Hauteur", mm(x.hauteur)),
                ("Décalage inférieur", mm(x.decalage_bas)),
                ("Décalage supérieur", mm(x.decalage_haut)),
            ],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.toposolides {
        let nt = nom_type(x.type_id);
        let mut props = vec![("Classe Revit", Value::Str(x.classe.clone())), ("Type", Value::Str(format!("{nt} [{}]", x.type_id)))];
        if x.niveau != -1 {
            props.push(("Niveau", Value::Str(niveau(x.niveau))));
            props.push(("Hauteur par rapport au niveau", mm(x.decalage)));
        }
        out.push(Produit { id: x.id, classe: "IFCGEOGRAPHICELEMENT", nom: format!("{nt} [{}]", x.id), type_id: x.type_id, nom_type: nt.clone(), niveau: x.niveau, props, geometrie: &x.geometrie });
    }
    for x in &m.surfaces_topo {
        let (nom, mut props) = if x.hote == -1 { ("Surface topographique", vec![]) } else { ("Sous-région", vec![("Surface hôte", Value::Str(x.hote.to_string()))]) };
        props.insert(0, ("Classe Revit", Value::Str("SiteSurface".into())));
        props.push(("Points", Value::Str(x.points.to_string())));
        out.push(Produit { id: x.id, classe: "IFCGEOGRAPHICELEMENT", nom: format!("{nom} [{}]", x.id), type_id: -1, nom_type: nom.into(), niveau: -1, props, geometrie: &x.geometrie });
    }
    for x in &m.dalles {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: if x.classe == "Ceiling" { "IFCCOVERING" } else { "IFCSLAB" },
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau,
            props: vec![
                ("Classe Revit", Value::Str(x.classe.clone())),
                ("Type", Value::Str(format!("{nt} [{}]", x.type_id))),
                ("Niveau", Value::Str(niveau(x.niveau))),
                ("Hauteur par rapport au niveau", mm(x.decalage)),
            ],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.toits {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: "IFCROOF",
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau,
            props: vec![
                ("Classe Revit", Value::Str(x.classe.clone())),
                ("Type", Value::Str(format!("{nt} [{}]", x.type_id))),
                ("Niveau de base", Value::Str(niveau(x.niveau))),
                ("Décalage par rapport au niveau", mm(x.decalage)),
            ],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.escaliers {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: "IFCSTAIR",
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau_bas,
            props: vec![
                ("Type", Value::Str(format!("{nt} [{}]", x.type_id))),
                ("Niveau de base", Value::Str(niveau(x.niveau_bas))),
                ("Niveau supérieur", Value::Str(niveau(x.niveau_haut))),
                ("Hauteur", mm(x.hauteur)),
                ("Nombre de contremarches", Value::Str(x.contremarches.to_string())),
                ("Hauteur de contremarche", mm(x.hauteur_contremarche)),
                ("Giron", mm(x.giron)),
                ("Volées et paliers (ElementId)", Value::Str(x.parties.iter().map(i64::to_string).collect::<Vec<_>>().join(", "))),
            ],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.garde_corps {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: "IFCRAILING",
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau,
            props: vec![("Type", Value::Str(format!("{nt} [{}]", x.type_id))), ("Niveau", Value::Str(niveau(x.niveau)))],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.instances {
        let (cat, classe) = m.categorie(x);
        let nt = m.nom_type_instance(x);
        let mut props = vec![
            ("Catégorie", Value::Str(cat)),
            ("Famille et type", Value::Str(format!("{nt} [{}]", x.type_id))),
            ("Niveau", Value::Str(niveau(x.niveau))),
            ("Point d'insertion", point_mm(x.point)),
        ];
        if let Some(t) = m.types.get(&x.type_id) {
            // cotes du panneau (paramètres de type, ou d'occurrence si la famille les
            // déclare ainsi), pas l'encombrement de la famille
            let occ = m.familles.get(&t.famille).map(|f| &f.occurrence);
            let cote = |p: i64, du_type: Option<f64>| {
                if !occ.is_some_and(|o| o.contains(&p)) {
                    return du_type;
                }
                let b = m.parametres.get(&x.id)?.iter().find(|(q, _)| *q == p)?.1.selon(parametres::Stockage::Reel);
                if let Brut::Reel(v) = b { Some(v * FT) } else { None }
            };
            if let Some(l) = cote(FAMILY_WIDTH_PARAM, t.largeur).filter(|&l| l > 0.0) {
                props.push(("Largeur (panneau)", mm(l)));
            }
            if let Some(h) = cote(FAMILY_HEIGHT_PARAM, t.hauteur).filter(|&h| h > 0.0) {
                props.push(("Hauteur (panneau)", mm(h)));
            }
        }
        if x.hote != -1 {
            props.push(("Hôte (ElementId)", Value::Str(x.hote.to_string())));
        }
        if !x.repere.is_empty() {
            props.push(("Repère", Value::Str(x.repere.clone())));
        }
        out.push(Produit { id: x.id, classe, nom: format!("{nt} [{}]", x.id), type_id: x.type_id, nom_type: nt, niveau: x.niveau, props, geometrie: &x.geometrie });
    }
    for x in &m.formes {
        let (cat, classe) = m.categorie_famille(x.famille);
        let mut props = vec![("Classe Revit", Value::Str(x.classe.clone())), ("Catégorie", Value::Str(cat))];
        if x.sous_categorie != -1 {
            props.push(("Sous-catégorie", Value::Str(m.nom_categorie(x.sous_categorie))));
        }
        if x.materiau != -1 {
            let nom = m.materiaux.get(&x.materiau).map(|x| x.nom.clone()).unwrap_or_else(|| x.materiau.to_string());
            props.push(("Matériau", Value::Str(nom)));
        }
        if !x.membres.is_empty() {
            props.push(("Formes combinées (ElementId)", Value::Str(x.membres.iter().map(i64::to_string).collect::<Vec<_>>().join(", "))));
        }
        out.push(Produit { id: x.id, classe, nom: format!("{} [{}]", nom_forme(&x.classe), x.id), type_id: -1, nom_type: String::new(), niveau: x.niveau, props, geometrie: &x.geometrie });
    }
    for x in m.pieces.iter().filter(|p| p.genre == "piece" && p.point.is_some()) {
        let mut props = vec![("Nom", Value::Str(x.nom.clone())), ("Numéro", Value::Str(x.numero.clone())), ("Niveau", Value::Str(niveau(x.niveau)))];
        if !x.service.is_empty() {
            props.push(("Service", Value::Str(x.service.clone())));
        }
        out.push(Produit { id: x.id, classe: "IFCSPACE", nom: format!("{} {}", x.numero, x.nom), type_id: -1, nom_type: String::new(), niveau: x.niveau, props, geometrie: &x.geometrie });
    }
    out
}

/// Scène du viewer : un modèle IFC synthétique (projet, niveaux, éléments, types,
/// propriétés) pour l'arborescence et les propriétés, et les maillages colorés par
/// matériau.
pub fn scene(m: &Maquette, titre: &str, file_name: String, format: &str) -> Scene {
    let mut b = Builder { entities: IdMap::default() };
    let mut element_de: HashMap<Id, i64> = HashMap::new();
    let n = Value::Null;
    // GlobalId d'un élément Revit : son IfcGUID, comme à l'export IFC de Revit
    let guid = |prefix: &str, id: i64| m.guid_ifc(id).map(Value::Str).unwrap_or_else(|| gid(prefix, id));
    let projet = b.add("IFCPROJECT", vec![gid("RVTPROJET", 0), n.clone(), Value::Str(titre.into())]);
    let coord = coordonnees_affichees(m);
    if !coord.is_empty() {
        let ps = b.pset("RVTCOORD", 0, "Coordonnées", coord.iter().map(|(k, v)| (k.as_str(), Value::Str(v.clone()))).collect());
        b.definir("RVTDEFC", 0, projet, vec![ps]);
    }
    // document de famille : paramètres de la famille (valeurs du type courant)
    if let Some(f) = m.famille_document {
        let ps = b.psets_parametres(m, f);
        b.definir("RVTDEFF", f, projet, ps);
    }
    let mut etage: HashMap<i64, Id> = HashMap::new();
    for l in &m.niveaux {
        let args = vec![guid("RVTNIV", l.id), n.clone(), Value::Str(l.nom.clone()), n.clone(), n.clone(), n.clone(), n.clone(), n.clone(), Value::Enum("ELEMENT".into()), Value::Real(l.altitude / 1000.0)];
        let e = b.add("IFCBUILDINGSTOREY", args);
        let ps = b.psets_parametres(m, l.id);
        b.definir("RVTDEFN", l.id, e, ps);
        etage.insert(l.id, e);
    }
    let storeys: Vec<Value> = m.niveaux.iter().map(|l| Value::Ref(etage[&l.id])).collect();
    b.add("IFCRELAGGREGATES", vec![gid("RVTAGG", 0), n.clone(), n.clone(), n.clone(), Value::Ref(projet), Value::List(storeys)]);
    let shape = b.add("IFCPRODUCTDEFINITIONSHAPE", vec![n.clone(), n.clone(), Value::List(vec![])]);

    let mut types: HashMap<(i64, &str), Id> = HashMap::new();
    let mut contenus: BTreeMap<Id, Vec<Value>> = BTreeMap::new();
    let mut geoms = Vec::new();
    // éléments qui portent chaque matériau (navigateur : onglet Matériaux)
    let mut usages: HashMap<i64, Vec<Id>> = HashMap::new();
    // traits des motifs de surface : plafond (2 sommets de 24 octets par trait)
    let (mut traits_motifs, mut motifs_omis, mut motifs_dessin) = (0usize, 0usize, 0usize);
    for p in produits(m) {
        let id = b.add(p.classe, vec![guid("RVT", p.id), n.clone(), Value::Str(p.nom.clone()), n.clone(), Value::Str(p.nom_type.clone()), n.clone(), Value::Ref(shape), Value::Str(p.id.to_string())]);
        element_de.insert(id, p.id);
        let mut props = vec![("ElementId", Value::Str(p.id.to_string()))];
        if let Some(u) = m.unique_id(p.id) {
            props.push(("UniqueId", Value::Str(u)));
        }
        props.extend(p.props);
        match p.geometrie {
            Ok(g) => {
                let mut mats: Vec<&Materiau> = g.faces.iter().filter_map(|f| m.materiaux.get(&f.materiau)).collect();
                mats.sort_unstable_by(|a, b| a.nom.cmp(&b.nom));
                mats.dedup_by(|a, b| a.nom == b.nom);
                props.push(("Matériaux des faces", Value::Str(mats.iter().map(|x| x.nom.as_str()).collect::<Vec<_>>().join(", "))));
                // motifs de coupe et de surface (premier plan) de chaque matériau
                let nom_motif = |id: i64| m.motifs.get(&id).map(|x| x.nom.as_str()).unwrap_or("aucun");
                let motifs: Vec<String> = mats.iter().filter(|x| x.motifs[0].0 != -1 || x.motifs[2].0 != -1).map(|x| format!("{} : coupe {}, surface {}", x.nom, nom_motif(x.motifs[0].0), nom_motif(x.motifs[2].0))).collect();
                if !motifs.is_empty() {
                    props.push(("Motifs des matériaux", Value::Str(motifs.join(" ; "))));
                }
            }
            Err(e) => props.push(("Géométrie", Value::Str(e.clone()))),
        }
        let pset = b.pset("RVTPSET", p.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", p.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        let ps = b.psets_parametres(m, p.id);
        b.definir("RVTDEFP", p.id, id, ps);
        if let Ok(g) = p.geometrie {
            let c = g.controle();
            let qv = b.add("IFCQUANTITYVOLUME", vec![Value::Str("Volume".into()), n.clone(), n.clone(), Value::Real(c.volume_m3)]);
            let qa = b.add("IFCQUANTITYAREA", vec![Value::Str("Aire des faces".into()), n.clone(), n.clone(), Value::Real(c.aire_m2)]);
            let qto = b.add("IFCELEMENTQUANTITY", vec![gid("RVTQTO", p.id), n.clone(), Value::Str("Géométrie Revit (GElement)".into()), n.clone(), n.clone(), Value::List(vec![Value::Ref(qv), Value::Ref(qa)])]);
            b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEFQ", p.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(qto)]);
            // une pièce garde la couleur des espaces ; les autres prennent celle de leurs matériaux
            let defaut = class_color(p.classe);
            let mut par_mat: BTreeMap<i64, Vec<[DVec3; 3]>> = BTreeMap::new();
            for f in &g.faces {
                par_mat.entry(f.materiau).or_default().extend(f.triangles.iter().map(|t| t.map(|i| g.sommets[i] / 1000.0)));
            }
            for &k in par_mat.keys() {
                usages.entry(k).or_default().push(id);
            }
            let parts = par_mat
                .into_iter()
                .map(|(k, tris)| Part { color: if p.classe == "IFCSPACE" { defaut } else { m.materiaux.get(&k).map(couleur).unwrap_or(defaut) }, tris })
                .collect();
            geoms.push(ProductGeom { id, parts, lignes: Vec::new() });
            // motifs de surface des matériaux (motifs de modèle), calés face par face
            let mut par_couleur: BTreeMap<[u8; 3], Vec<[DVec3; 2]>> = BTreeMap::new();
            for f in &g.faces {
                let Some(cal) = f.motif else { continue };
                match m.motifs.get(&cal.motif) {
                    Some(x) if x.modele && !x.grilles.is_empty() => {
                        if traits_motifs >= MAX_TRAITS_MOTIFS {
                            motifs_omis += 1;
                            continue;
                        }
                        let tris: Vec<[DVec3; 3]> = f.triangles.iter().map(|t| t.map(|i| g.sommets[i])).collect();
                        let s = vues::motif_sur_face(&tris, &cal, x, 1.0, MAX_TRAITS_MOTIFS - traits_motifs);
                        traits_motifs += s.len();
                        par_couleur.entry(cal.couleur).or_default().extend(s);
                    }
                    Some(x) if !x.modele && !x.grilles.is_empty() => motifs_dessin += 1,
                    _ => {}
                }
            }
            if !par_couleur.is_empty() {
                let f = b.add("IFCSURFACEFEATURE", vec![gid("RVTMOTIF", p.id), n.clone(), Value::Str(format!("Motifs de surface [{}]", p.id)), n.clone(), n.clone(), n.clone(), Value::Ref(shape)]);
                b.add("IFCRELAGGREGATES", vec![gid("RVTAGGM", p.id), n.clone(), n.clone(), n.clone(), Value::Ref(id), Value::List(vec![Value::Ref(f)])]);
                let lignes = par_couleur.into_iter().map(|(c, segs)| Ligne { color: [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0], segs }).collect();
                geoms.push(ProductGeom { id: f, parts: Vec::new(), lignes });
            }
        }
        if p.type_id != -1 {
            let classe_type: &str = p.classe;
            let t = match types.get(&(p.type_id, classe_type)) {
                Some(&t) => t,
                None => {
                    let mut tp = vec![("ElementId", Value::Str(p.type_id.to_string()))];
                    if let Some(e) = m.types.get(&p.type_id).and_then(|t| t.epaisseur) {
                        tp.push(("Épaisseur", mm(e)));
                    }
                    if let Some(u) = m.unique_id(p.type_id) {
                        tp.push(("UniqueId", Value::Str(u)));
                    }
                    let pset = b.pset("RVTTPSET", p.type_id, "Revit", tp);
                    let mut psets = vec![Value::Ref(pset)];
                    psets.extend(b.psets_parametres(m, p.type_id).into_iter().map(Value::Ref));
                    let t = b.add(&format!("{}TYPE", p.classe), vec![guid("RVTTYPE", p.type_id), n.clone(), Value::Str(p.nom_type.clone()), n.clone(), n.clone(), Value::List(psets)]);
                    types.insert((p.type_id, classe_type), t);
                    t
                }
            };
            b.add("IFCRELDEFINESBYTYPE", vec![gid("RVTRELT", p.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(t)]);
        }
        contenus.entry(etage.get(&p.niveau).copied().unwrap_or(projet)).or_default().push(Value::Ref(id));
    }
    for g in &m.grilles {
        let mut props = vec![("ElementId", Value::Str(g.id.to_string())), ("Début", point_mm(g.debut)), ("Fin", point_mm(g.fin))];
        match g.arc {
            Some((c, r)) => {
                props.push(("Centre", point_mm(c)));
                props.push(("Rayon", mm(r)));
            }
            None => props.push(("Longueur", mm(g.debut.distance(g.fin)))),
        }
        let id = b.add("IFCGRID", vec![guid("RVT", g.id), n.clone(), Value::Str(g.nom.clone()), n.clone(), n.clone(), n.clone(), Value::Ref(shape)]);
        let pset = b.pset("RVTPSET", g.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", g.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        contenus.entry(projet).or_default().push(Value::Ref(id));
    }
    // Vues : un nœud par vue (dans le niveau d'un plan, sinon dans le projet), ses
    // éléments 2D en enfants, tracés dans le plan de la vue (masqués par défaut).
    // éléments 2D par vue, sauf ceux que Revit n'y montre pas (masqués, cotes dont un
    // élément coté n'est pas visible dans la vue)
    let boites = feuilles::boites(m);
    let par_id: HashMap<i64, &Vue> = m.vues.iter().map(|v| (v.id, v)).collect();
    let mut par_vue: BTreeMap<i64, Vec<&Element2D>> = BTreeMap::new();
    for e in m.elements_2d.iter().filter(|e| par_id.get(&e.vue).is_some_and(|v| feuilles::visible_dans_vue(m, e, v, &boites))) {
        par_vue.entry(e.vue).or_default().push(e);
    }
    let nom_vue: HashMap<i64, &str> = m.vues.iter().map(|v| (v.id, v.nom.as_str())).collect();
    let mut entite_vue: HashMap<i64, Id> = HashMap::new();
    // éléments 2D de chaque vue en primitives (export)
    let mut primitives_vues: HashMap<i64, Vec<Primitive>> = HashMap::new();
    for v in m.vues.iter().filter(|v| !v.gabarit && v.genre != "Arborescence") {
        let mut props = vec![
            ("ElementId", Value::Str(v.id.to_string())),
            ("Classe Revit", Value::Str(v.classe.clone())),
            ("Genre", Value::Str(v.genre.into())),
            ("Échelle", Value::Str(format!("1:{}", v.echelle))),
            ("Orientation", Value::Str((if v.nord_geographique { "Nord géographique" } else { "Nord du projet" }).into())),
            ("Plage de vue", Value::Str(texte_plage(m, v))),
            ("Origine", point_mm(v.origine)),
            ("Direction de vue", Value::Str(format!("{:.4} ; {:.4} ; {:.4}", v.direction.x, v.direction.y, v.direction.z))),
            ("Droite, haut", Value::Str(format!("{:.4} ; {:.4} ; {:.4} / {:.4} ; {:.4} ; {:.4}", v.droite.x, v.droite.y, v.droite.z, v.haut.x, v.haut.y, v.haut.z))),
        ];
        if let Some(niv) = m.niveaux.iter().find(|l| l.id == v.niveau) {
            props.push(("Niveau", Value::Str(niv.nom.clone())));
        }
        if let Some((_, _, l)) = v.cadrage {
            props.push(("Cadrage", Value::Str(format!("{} × {} × {} mm", format_real(l[0].round()), format_real(l[1].round()), format_real(l[2].round())))));
        }
        let elems = par_vue.get(&v.id).map(Vec::as_slice).unwrap_or_default();
        props.push(("Éléments 2D", Value::Str(elems.len().to_string())));
        let nom = if v.nom.is_empty() { format!("{} [{}]", v.genre, v.id) } else { format!("{} : {}", v.genre, v.nom) };
        let id_vue = b.add("IFCANNOTATION", vec![guid("RVTVUE", v.id), n.clone(), Value::Str(nom), n.clone(), Value::Str(v.genre.into()), n.clone(), Value::Ref(shape)]);
        entite_vue.insert(v.id, id_vue);
        let pset = b.pset("RVTPSETV", v.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEFV", v.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id_vue)]), Value::Ref(pset)]);
        let ps = b.psets_parametres(m, v.id);
        b.definir("RVTDEFW", v.id, id_vue, ps);
        contenus.entry(etage.get(&v.niveau).copied().unwrap_or(projet)).or_default().push(Value::Ref(id_vue));
        let normale = if v.direction.length() > 0.5 { v.direction.normalize() } else { DVec3::Z };
        let mut enfants = Vec::new();
        for e in elems {
            let (genre, couleur, mut props): (&str, Rgba, Vec<(&str, Value)>) = match &e.contenu {
                Contenu::Trait(p) => ("Trait", [0.12, 0.12, 0.12, 1.0], vec![("Longueur", mm(p.windows(2).map(|w| w[0].distance(w[1])).sum()))]),
                Contenu::Texte { texte, point, .. } => ("Texte", [0.80, 0.35, 0.10, 0.45], vec![("Texte", Value::Str(texte.clone())), ("Point", point_mm(*point))]),
                Contenu::Cote { segments, .. } => {
                    let valeurs = segments.iter().map(|s| format!("{} mm", format_real((s.0 * 10.0).round() / 10.0))).collect::<Vec<_>>().join(" + ");
                    ("Cote", [0.10, 0.35, 0.80, 1.0], vec![("Valeur", Value::Str(valeurs))])
                }
                Contenu::Region { type_id } => {
                    let t = m.types_regions.get(type_id);
                    let nom_motif = |id: i64| m.motifs.get(&id).map(|x| x.nom.clone()).unwrap_or_else(|| "aucun".into());
                    let mut p = vec![("Type", Value::Str(t.map(|t| t.nom.clone()).unwrap_or_else(|| type_id.to_string())))];
                    if let Some(t) = t {
                        p.push(("Motif de premier plan", Value::Str(nom_motif(t.avant))));
                        p.push(("Motif d'arrière-plan", Value::Str(nom_motif(t.arriere))));
                    }
                    ("Région remplie", [0.45, 0.45, 0.45, 0.6], p)
                }
                Contenu::Etiquette { params, hauteur, .. } => {
                    let exemple = params.iter().map(|p| format!("{}{}{}", p.prefixe, p.exemple, p.suffixe)).collect::<Vec<_>>().join(" ");
                    ("Étiquette", [0.80, 0.35, 0.10, 0.45], vec![("Texte d'exemple", Value::Str(exemple)), ("Hauteur du cadre", mm(*hauteur))])
                }
                Contenu::FenetreDeVue { vue, echelle, decalage, .. } => {
                    let nv = nom_vue.get(vue).map(|s| s.to_string()).unwrap_or_else(|| vue.to_string());
                    ("Fenêtre de vue", [0.5, 0.5, 0.5, 1.0], vec![("Vue placée", Value::Str(nv)), ("Échelle", Value::Str(format!("1:{}", (1.0 / echelle).round()))), ("Décalage", point_mm(*decalage))])
                }
            };
            props.splice(0..0, [("ElementId", Value::Str(e.id.to_string())), ("Classe Revit", Value::Str(e.classe.clone())), ("Vue", Value::Str(v.nom.clone()))]);
            let titre = match &e.contenu {
                Contenu::Texte { texte, .. } => format!("{genre} « {} » [{}]", texte.lines().next().unwrap_or_default(), e.id),
                _ => format!("{genre} [{}]", e.id),
            };
            let id = b.add("IFCANNOTATION", vec![guid("RVT2D", e.id), n.clone(), Value::Str(titre), n.clone(), Value::Str(genre.into()), n.clone(), Value::Ref(shape)]);
            let pset = b.pset("RVTPSET2D", e.id, "Revit", props);
            b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF2D", e.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
            let ps = b.psets_parametres(m, e.id);
            b.definir("RVTDEF2P", e.id, id, ps);
            let mut prims = Vec::new();
            // note de texte : ses glyphes (le cadre seul si le texte est vide) ; cote :
            // ligne, lignes d'attache, marques et valeurs comme dans Revit
            if let Some((color, t)) = feuilles::note(m, e, v.echelle) {
                prims.extend(primitives_parts(vec![Part { color, tris: t.into_iter().map(|t| t.map(|p| p / 1000.0)).collect() }], Some(id)));
            } else if let Some(p) = feuilles::cote(m, e, normale, v.droite, v.echelle) {
                prims.extend(primitives_parts(p, Some(id)));
            } else {
                prims.extend(vues::primitives(e, normale, v.echelle, octets(couleur), Some(id)));
            }
            if let (Contenu::Region { type_id }, Ok(g)) = (&e.contenu, &e.geometrie) {
                prims.extend(region(m, *type_id, g, v, Some(id)));
            }
            let parts = parts_primitives(&prims, normale, v.echelle);
            if !parts.is_empty() {
                geoms.push(ProductGeom { id, parts, lignes: Vec::new() });
            }
            primitives_vues.entry(v.id).or_default().extend(prims);
            enfants.push(Value::Ref(id));
        }
        // instances propres à la vue (cartouches, symboles, composants de détail) et titres
        // des fenêtres de vue d'une feuille
        for o in feuilles::objets_vue(m, v) {
            let id = b.add("IFCANNOTATION", vec![guid("RVT2D", o.id), n.clone(), Value::Str(o.titre), n.clone(), Value::Str(o.genre.into()), n.clone(), Value::Ref(shape)]);
            let classe = if o.genre == "Titre de vue" { "Viewport" } else { "FamilyInstance" };
            let props = vec![("ElementId", Value::Str(o.id.to_string())), ("Classe Revit", Value::Str(classe.into())), ("Genre", Value::Str(o.genre.into())), ("Vue", Value::Str(v.nom.clone()))];
            let pset = b.pset("RVTPSET2I", o.id, "Revit", props);
            b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF2I", o.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
            let ps = b.psets_parametres(m, o.id);
            b.definir("RVTDEF2J", o.id, id, ps);
            primitives_vues.entry(v.id).or_default().extend(primitives_parts(o.parts.clone(), Some(id)));
            geoms.push(ProductGeom { id, parts: o.parts, lignes: Vec::new() });
            enfants.push(Value::Ref(id));
        }
        if !enfants.is_empty() {
            b.add("IFCRELAGGREGATES", vec![gid("RVTAGGV", v.id), n.clone(), n.clone(), n.clone(), Value::Ref(id_vue), Value::List(enfants)]);
        }
    }
    for (k, (s, items)) in contenus.into_iter().enumerate() {
        b.add("IFCRELCONTAINEDINSPATIALSTRUCTURE", vec![gid("RVTCONT", k as i64), n.clone(), n.clone(), n.clone(), Value::List(items), Value::Ref(s)]);
    }
    let entite_type: HashMap<i64, Id> = types.iter().map(|(&(t, _), &e)| (t, e)).collect();
    let mut nav = navigateur(m, &entite_vue, &entite_type, &etage);
    nav.materiaux = m
        .materiaux
        .iter()
        .map(|(k, x)| scene::NavMateriau { nom: x.nom.clone(), couleur: x.couleur, transparence: x.transparence, elements: usages.remove(k).unwrap_or_default() })
        .collect();
    nav.materiaux.sort_by_key(|a| a.nom.to_lowercase());
    for v in &mut nav.vues {
        v.primitives = primitives_vues.remove(&v.revit_id).unwrap_or_default();
    }
    let model = IfcModel::new(StepFile { schema: format!("REVIT {format}"), entities: b.entities });
    let mut s = scene::assemble(model, geoms, file_name);
    let mut nav = nav;
    for (&ent, &el) in &element_de {
        let ph = |k| id_parametre(m, el, k);
        if let Some(c) = ph(-1_012_100) {
            nav.phases_objets.insert(ent, (c, ph(-1_012_101).unwrap_or(-1)));
        }
    }
    s.navigateur = Some(nav);
    // coordonnées partagées du point cliqué (document placé)
    if let Some(e) = m.coordonnees.actif().filter(|_| m.coordonnees.place()) {
        let col = |j: usize| DVec3::new(e.lignes[0][j], e.lignes[1][j], e.lignes[2][j]);
        s.partage = Some(scene::ReperePartage { nom: e.nom.clone(), origine: e.origine / 1000.0, colonnes: [col(0), col(1), col(2)] });
    }
    for (nom, k) in [
        ("Faces à motif de surface de dessin (taille d'écran dans Revit), non tracé en 3D", motifs_dessin),
        ("Faces à motif de surface non tracé (limite de traits atteinte)", motifs_omis),
    ] {
        if k > 0 {
            s.unsupported.push((nom.into(), k));
        }
    }
    s
}

/// Famille système (catégorie, famille) d'une classe de type, comme dans le navigateur
/// de Revit ; None pour les types qu'il ne montre pas sous « Familles ».
fn famille_systeme(classe: &str) -> Option<(&'static str, &'static str)> {
    Some(match classe {
        "BasicWallType" => ("Murs", "Mur de base"),
        "NewCurtainWallType" | "CurtainWallType" => ("Murs", "Mur-rideau"),
        "StackedWallType" => ("Murs", "Mur empilé"),
        "FloorAttributes" => ("Sols", "Sol"),
        "CeilingAttributes" => ("Plafonds", "Plafond composé"),
        "RoofAttributes" => ("Toits", "Toit de base"),
        "StairsType" => ("Escaliers", "Escalier assemblé"),
        "StairsRunType" => ("Escaliers", "Volée"),
        "StairsLandingType" => ("Escaliers", "Palier"),
        "BaseRailingSym" | "StairsRailingAttr" => ("Garde-corps", "Garde-corps"),
        "ToposolidType" => ("Toposolides", "Toposolide"),
        "GridAttributes" => ("Quadrillages", "Quadrillage"),
        "LevelAttributes" => ("Niveaux", "Niveau"),
        "TextNoteAttributes" => ("Annotations", "Note textuelle"),
        "FilledRegionAttributes" => ("Annotations", "Zone remplie"),
        _ => return None,
    })
}

/// Arborescence du projet : vues (hors gabarits), feuilles et vues qu'elles portent,
/// familles chargées par catégorie puis familles système, groupes.
/// Plage de vue d'un plan en clair : « Haut : Niveau 1 + 2300 mm ; Plan de coupe : … ».
fn texte_plage(m: &Maquette, v: &Vue) -> String {
    let Some(p) = v.plage_vue else { return String::new() };
    let niveau = |n: i64| match n {
        -3 => "Niveau associé".to_string(),
        -4 => "Niveau au-dessus".to_string(),
        -2 => "Niveau en dessous".to_string(),
        -1 => "Illimité".to_string(),
        n => m.niveaux.iter().find(|x| x.id == n).map(|x| x.nom.clone()).unwrap_or_else(|| format!("#{n}")),
    };
    ["Haut", "Plan de coupe", "Bas", "Profondeur de vue"]
        .iter()
        .zip(p)
        .map(|(nom, (n, d))| if n == -1 { format!("{nom} : Illimité") } else { format!("{nom} : {} {:+} mm", niveau(n), format_real((d * 10.0).round() / 10.0).parse::<f64>().unwrap_or(d)) })
        .collect::<Vec<_>>()
        .join(" ; ")
}

/// Valeur ElementId d'un paramètre d'un élément (jeux de paramètres).
fn id_parametre(m: &Maquette, element: i64, param: i64) -> Option<i64> {
    m.parametres.get(&element)?.iter().find(|(p, _)| *p == param).and_then(|(_, b)| if let Brut::Id(i) = b.selon(parametres::Stockage::Id) { Some(i) } else { None })
}

fn navigateur(m: &Maquette, entite_vue: &HashMap<i64, Id>, entite_type: &HashMap<i64, Id>, etage: &HashMap<i64, Id>) -> Navigateur {
    let mut placees: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut fenetres: HashMap<i64, Vec<scene::NavFenetre>> = HashMap::new();
    let par_id: HashMap<i64, &Vue> = m.vues.iter().map(|v| (v.id, v)).collect();
    for e in &m.elements_2d {
        if let Contenu::FenetreDeVue { vue, decalage, echelle, .. } = &e.contenu {
            placees.entry(e.vue).or_default().push(*vue);
            if let Some(v) = par_id.get(vue) {
                let mut f = feuilles::fenetre(*decalage, *echelle, v);
                f.masques.extend(feuilles::masques_par_categorie(m, v));
                fenetres.entry(e.vue).or_default().push(f);
            }
        }
    }
    let nom_type = |t: i64| m.types.get(&t).map(|x| x.nom.clone()).unwrap_or_default();
    let vues = m
        .vues
        .iter()
        .filter(|v| !v.gabarit && v.genre != "Arborescence")
        .map(|v| NavVue {
            revit_id: v.id,
            phase: id_parametre(m, v.id, -1_012_102).unwrap_or(0),
            filtre_phase: id_parametre(m, v.id, -1_012_103).unwrap_or(-1),
            coupe: v.plage_vue.map(|p| {
                // plan de coupe ramené au niveau de la vue
                let alt = |n: i64| m.niveaux.iter().find(|x| x.id == n).map(|x| x.altitude);
                let (n, d) = p[1];
                let ecart = if n == -3 || n == v.niveau { 0.0 } else { alt(n).zip(alt(v.niveau)).map_or(0.0, |(a, b)| a - b) };
                (ecart + d) / 1000.0
            }),
            entite: entite_vue.get(&v.id).copied(),
            genre: v.genre.to_string(),
            type_vue: nom_type(v.type_vue),
            nom: v.nom.clone(),
            echelle: v.echelle,
            numero: v.numero.clone(),
            haut: v.haut,
            direction: v.direction,
            etage: etage.get(&v.niveau).copied(),
            placees: placees.remove(&v.id).unwrap_or_default(),
            fenetres: fenetres.remove(&v.id).unwrap_or_default(),
            primitives: Vec::new(),
            tableau: v.nomenclature.as_ref().map(|d| {
                let t = super::nomenclatures::calculer(m, d);
                (t.titres, t.lignes, t.non_calculees, super::nomenclatures::texte_tris(d))
            }),
        })
        .collect();
    // familles chargées (catégorie, famille, types), puis familles système
    let mut par_famille: BTreeMap<(String, String), Vec<(String, Option<Id>)>> = BTreeMap::new();
    for (&id, f) in m.familles.iter().filter(|(_, f)| f.parent == -1) {
        let cle = (m.nom_categorie(f.categorie), f.nom.clone());
        let mut ts: Vec<(String, Option<Id>)> = m.types.iter().filter(|(_, t)| t.famille == id).map(|(&tid, t)| (t.nom.clone(), entite_type.get(&tid).copied())).collect();
        ts.sort();
        par_famille.entry(cle).or_default().extend(ts);
    }
    for (&tid, t) in &m.types {
        if let Some((cat, fam)) = famille_systeme(&t.classe) {
            par_famille.entry((cat.to_string(), fam.to_string())).or_default().push((t.nom.clone(), entite_type.get(&tid).copied()));
        }
    }
    let familles = par_famille
        .into_iter()
        .map(|((categorie, nom), mut types)| {
            types.sort();
            types.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
            NavFamille { categorie, nom, types }
        })
        .collect();
    let mut groupes: Vec<String> = m.types.values().filter(|t| t.classe == "ElementGroupType").map(|t| t.nom.clone()).collect();
    groupes.sort();
    let filtres_phase = m.filtres_phase.iter().map(|(&id, (_, p))| (id, *p)).collect();
    Navigateur { vues, familles, groupes, materiaux: Vec::new(), phases: m.ordre_phases.clone(), filtres_phase, phases_objets: Default::default() }
}

/// Nombre maximal de traits de motifs de surface dans une scène.
const MAX_TRAITS_MOTIFS: usize = 1_500_000;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Fichier d'essai hors dépôt, dans le dossier du décodage (`essais\` : essais créés
    /// par les campagnes ; `fichiers\` : projets et familles de référence ; voir son
    /// SOURCES.md) ; dossier remplaçable par la variable d'environnement RVT_ESSAIS.
    /// Absent : le test est sauté.
    fn essai(dossier: &str, nom: &str) -> Option<PathBuf> {
        let base = std::env::var("RVT_ESSAIS").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(r"C:\Users\theba\Projets code\Decodage RVT"));
        let p = base.join(dossier).join(nom);
        if p.exists() {
            Some(p)
        } else {
            eprintln!("fichier d'essai absent, test sauté : {}", p.display());
            None
        }
    }

    fn proche(v: f64, att: f64, tol: f64, quoi: &str) {
        assert!((v - att).abs() <= tol, "{quoi} : {v} au lieu de {att}");
    }

    /// Essai E04 : mur de 12 345,6 × 200 × 2 718,2 mm, valeurs validées contre Revit 2026.
    #[test]
    fn e04_volume_et_boite() {
        let Some(p) = essai(r"essais\2026", "E04_mur.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.murs.len(), 1);
        let mur = &m.murs[0];
        assert_eq!((mur.id, mur.classe.as_str(), mur.niveau, mur.type_id), (2797, "SWall", 1959, 1990));
        assert!((mur.hauteur - 2718.2).abs() < 0.05, "hauteur {}", mur.hauteur);
        let g = mur.geometrie.as_ref().unwrap();
        let c = g.controle();
        assert_eq!((g.sommets.len(), g.faces.len(), c.triangles, c.non_appariees), (8, 6, 12, 0));
        assert!((c.volume_m3 - 6.711562).abs() < 1e-6, "volume {}", c.volume_m3);
        assert!((c.aire_m2 - 73.1411).abs() < 1e-4, "aire {}", c.aire_m2);
        for (v, att) in [(c.min, [0.0, -100.0, 0.0]), (c.max, [12_345.6, 100.0, 2_718.2])] {
            for k in 0..3 {
                assert!((v[k] - att[k]).abs() < 1e-3, "boîte {v:?} au lieu de {att:?}");
            }
        }
        let t = &m.types[&1990];
        assert!((t.epaisseur.unwrap() - 200.0).abs() < 1e-6);
    }

    /// Projet1.rvt (format 2026) : 59 murs validés contre Revit 2026 (volume, aire et
    /// faces) ; sols, escaliers, pièces, instances et matériaux comparés à l'API de
    /// Revit 2026 (script RiveTT rvt-reference-elements, 21/10/2026).
    #[test]
    fn projet1() {
        let Some(p) = essai("fichiers", "Projet1.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.murs.len(), 59);
        assert_eq!(m.niveaux.len(), 3);
        let (mut vol, mut tris) = (0.0, 0);
        for mur in &m.murs {
            let c = mur.geometrie.as_ref().unwrap_or_else(|e| panic!("mur {} : {e}", mur.id)).controle();
            assert_eq!(c.non_appariees, 0, "mur {} non fermé", mur.id);
            vol += c.volume_m3;
            tris += c.triangles;
        }
        assert!((vol - 257.974221).abs() < 1e-5, "volume total {vol}");
        assert_eq!(tris, 3768);

        // Matériaux : nom, couleur, transparence (Revit : Verre 92-99-177, 70 %).
        let verre = &m.materiaux[&33];
        assert_eq!((verre.nom.as_str(), verre.couleur), ("Verre", [92, 99, 177]));
        proche(verre.transparence * 100.0, 70.0, 1e-4, "transparence");
        assert_eq!(m.materiaux[&31].couleur, [142, 140, 122]);
        assert_eq!(m.materiaux.len(), 18);

        // Sols : type, niveau, décalage, volume du solide.
        let att = [(256950, 1910, -50.0, 1.8), (257366, 1910, 0.0, 1.875), (257376, 1910, 0.0, 2.25), (257386, 2049, 0.0, 1.875), (257396, 2049, 0.0, 2.25), (257406, 2049, 0.0, 60.0)];
        assert_eq!(m.dalles.len(), att.len());
        for (d, (id, niv, dec, v)) in m.dalles.iter().zip(att) {
            assert_eq!((d.id, d.niveau, m.types[&d.type_id].nom.as_str()), (id, niv, "Dalle en béton - 250 mm"));
            proche(d.decalage, dec, 1e-6, "décalage");
            proche(d.geometrie.as_ref().unwrap().controle().volume_m3, v, 1e-5, "volume de dalle");
        }

        // Escaliers : niveaux, contremarches, giron, volées, volume.
        assert_eq!(m.escaliers.len(), 2);
        for (e, (id, bas, haut, volee)) in m.escaliers.iter().zip([(257153, 608, 1910, 257154), (257309, 1910, 2049, 257310)]) {
            assert_eq!((e.id, e.niveau_bas, e.niveau_haut, e.contremarches, e.parties.as_slice()), (id, bas, haut, 16, &[volee][..]));
            assert_eq!(m.types[&e.type_id].nom, "Monobloc");
            proche(e.hauteur_contremarche, 187.5, 1e-6, "contremarche");
            proche(e.giron, 230.0, 1e-6, "giron");
            proche(e.hauteur, 3000.0, 1e-6, "hauteur");
            proche(e.geometrie.as_ref().unwrap().controle().volume_m3, 0.807594, 1e-5, "volume d'escalier");
        }

        // Pièces : 39, nom, numéro et volume (Revit : 22,468454 m³ pour « Local vélos »).
        let pieces: Vec<&Piece> = m.pieces.iter().filter(|p| p.genre == "piece").collect();
        assert_eq!(pieces.len(), 39);
        let velos = pieces.iter().find(|p| p.id == 257034).unwrap();
        assert_eq!((velos.nom.as_str(), velos.numero.as_str()), ("Local vélos", "14"));
        assert!(pieces.iter().all(|p| p.point.is_some()));
        proche(velos.geometrie.as_ref().unwrap().controle().volume_m3, 22.468454, 1e-5, "volume de pièce");

        // Instances : catégories, famille et type, hôte, point, repère.
        let par_cat = |c: &str| m.instances.iter().filter(|i| m.categorie(i).0 == c).count();
        assert_eq!((par_cat("Portes"), par_cat("Fenêtres"), par_cat("Barreaux")), (43, 26, 33));
        let porte = m.instances.iter().find(|i| i.id == 257051).unwrap();
        assert_eq!(m.nom_type_instance(porte), "Int. Simple : PP (0.83m x 2.04m)");
        // cotes du panneau : paramètres de type Largeur et Hauteur (Revit : 830 × 2 040)
        let t = &m.types[&porte.type_id];
        assert_eq!((t.largeur.map(f64::round), t.hauteur.map(f64::round)), (Some(830.0), Some(2040.0)));
        assert_eq!((porte.hote, porte.niveau, porte.repere.as_str()), (257007, 608, "P4"));
        assert!(porte.point.distance(DVec3::new(1500.0, 0.0, 0.0)) < 1e-3, "point {:?}", porte.point);
        let fenetre = m.instances.iter().find(|i| i.id == 257071).unwrap();
        assert_eq!(fenetre.repere, "F1");
        assert!(fenetre.point.distance(DVec3::new(2200.0, 0.0, 1500.0)) < 1e-3, "point {:?}", fenetre.point);

        // Géométrie des familles (Revit, détail fin) : porte 0,14565 m³, fenêtre
        // 0,022169 m³ (ouverture invisible écartée), 102 instances 4,453885 m³.
        let vol = |i: &Instance| i.geometrie.as_ref().unwrap_or_else(|e| panic!("instance {} : {e}", i.id)).controle().volume_m3;
        proche(vol(porte), 0.14565, 1e-5, "volume de porte");
        proche(vol(fenetre), 0.022169, 1e-5, "volume de fenêtre");
        let total: f64 = m.instances.iter().map(vol).sum();
        proche(total, 4.453885, 1e-3, "volume des instances");

        // Garde-corps : 11, volumes identiques à Revit (main courante et barreaux).
        assert_eq!(m.garde_corps.len(), 11);
        let gc = m.garde_corps.iter().find(|g| g.id == 256986).unwrap();
        assert_eq!(m.types[&gc.type_id].nom, "Type Bateau");
        proche(gc.geometrie.as_ref().unwrap().controle().volume_m3, 0.098736, 1e-5, "volume de garde-corps");
        let total: f64 = m.garde_corps.iter().map(|g| g.geometrie.as_ref().unwrap().controle().volume_m3).sum();
        proche(total, 0.821724, 1e-5, "volume des garde-corps");

        let s = scene(&m, "Projet1", "Projet1.rvt".into(), "2026");
        assert!(s.objects.len() > 59);
    }

    /// Kerreut 2025.rvt (format 2025, corpus de l'agence, hors dépôt) : toits par tracé
    /// comparés à l'API de Revit 2026 (fichier mis à niveau à l'ouverture).
    #[test]
    fn kerreut_2025_toits() {
        let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let att = [
            (9793689, 512913, 0.0, 7.483604),
            (9814775, 512913, 0.0, 6.997656),
            (9815700, 512913, -500.0, 1.095285),
            (9899736, 512913, -100.0, 0.329248),
            (9911706, 512914, 0.0, 9.267006),
            (9912297, 608, 2765.69, 7.62112),
            (9955293, 512913, -357.22, 1.918785),
            (10089207, 608, 3100.0, 2.403819),
            (10235462, 512914, 700.88, 3.064929),
            (10235825, 512914, 880.88, 8.257448),
        ];
        assert_eq!(m.toits.len(), att.len());
        for (t, (id, niv, dec, v)) in m.toits.iter().zip(att) {
            assert_eq!((t.id, t.niveau, t.classe.as_str()), (id, niv, "ProfileRoof"));
            proche(t.decalage, dec, 0.01, "décalage de toit");
            proche(t.geometrie.as_ref().unwrap().controle().volume_m3, v, 1e-5, "volume de toit");
        }
        assert_eq!(m.types[&m.toits[0].type_id].nom, "ARC_Toit_Couverture(2)_Chevron(6)");
        // porte « PP (1.03m x 2.04m) » : Largeur 1 030, Hauteur 2 040 dans Revit (absente de
        // la table de types de la famille, présente dans FamilySymbol.m_pParams)
        let porte = m.instances.iter().find(|i| i.id == 9911769).unwrap();
        let t = &m.types[&porte.type_id];
        assert_eq!((t.nom.as_str(), t.largeur.map(f64::round), t.hauteur.map(f64::round)), ("PP (1.03m x 2.04m)", Some(1030.0), Some(2040.0)));
        let e = m.escaliers.iter().find(|e| e.id == 9911889).unwrap();
        assert_eq!((e.niveau_bas, e.niveau_haut, e.contremarches, e.parties.as_slice()), (512913, 512914, 15, &[9911890][..]));
        proche(e.hauteur_contremarche, 180.0, 1e-6, "contremarche");
        proche(e.giron, 240.0, 1e-6, "giron");
        proche(e.hauteur, 2700.0, 1e-6, "hauteur");
        // escalier assemblé : marches dans le type `StairsTriserSymbol` (GInstance)
        proche(e.geometrie.as_ref().unwrap().controle().volume_m3, 0.195419, 1e-5, "volume d'escalier");

        // Familles : 342 instances sur 346 maillées (les 4 autres n'ont pas de solide dans
        // Revit) ; arbre aux surfaces de révolution, 6,566407 m³ dans Revit (−51 % sans
        // points intérieurs).
        assert_eq!(m.instances.iter().filter(|i| i.geometrie.is_ok()).count(), 342);
        let arbre = m.instances.iter().find(|i| i.id == 9692487).unwrap();
        let v = arbre.geometrie.as_ref().unwrap().controle().volume_m3;
        assert!((v - 6.566407).abs() / 6.566407 < 0.01, "volume de l'arbre {v}");
    }

    /// Essai E12 : trois grilles créées dans Revit 2026 (A droite, B2 oblique, Courbe en
    /// arc de rayon 4 000 mm).
    #[test]
    fn e12_grilles() {
        let Some(p) = essai(r"essais\2026", "E12_grilles.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.grilles.len(), 3);
        let g = |nom: &str| m.grilles.iter().find(|g| g.nom == nom).unwrap();
        let pt = |x, y| DVec3::new(x, y, 0.0);
        for (nom, id, debut, fin) in [("A", 2797, pt(0.0, 0.0), pt(12_345.6, 0.0)), ("B2", 2798, pt(1000.0, -2000.0), pt(4000.0, 7000.0)), ("Courbe", 2799, pt(9000.0, 3000.0), pt(5000.0, 7000.0))] {
            let x = g(nom);
            assert_eq!(x.id, id);
            assert!(x.debut.distance(debut) < 1e-6 && x.fin.distance(fin) < 1e-6, "{nom} : {:?} → {:?}", x.debut, x.fin);
        }
        let (c, r) = g("Courbe").arc.unwrap();
        assert!(c.distance(pt(5000.0, 3000.0)) < 1e-6 && (r - 4000.0).abs() < 1e-6);
        assert!(g("A").arc.is_none());
    }

    /// Famille de fenêtre `FEN_2PF.rfa` : formes affichées comme dans Revit 2026 (pleines
    /// hors combinaison, combinaisons ; membres et vides écartés), volumes de
    /// `get_Geometry` relevés par le script RiveTT `rvt-reference-formes-famille`.
    #[test]
    fn fenetre_rfa_formes() {
        let Some(p) = essai("fichiers", "FEN_2PF.rfa") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let ids: BTreeSet<i64> = m.formes.iter().map(|f| f.id).collect();
        let attendus = [9900043, 9900072, 9900099, 9900103, 9900142, 9900181, 9900224, 9900236, 9900237, 9900258, 9900428, 9900429, 9900436];
        assert_eq!(ids, attendus.into_iter().collect());
        let vol = |id: i64| m.formes.iter().find(|f| f.id == id).unwrap().geometrie.as_ref().map(|g| g.controle().volume_m3);
        for (id, v) in [(9900043, 0.0094), (9900099, 0.01264), (9900103, 0.00264), (9900224, 0.006528), (9900236, 0.004373), (9900428, 0.00788), (9900258, 0.00972)] {
            proche(vol(id).unwrap(), v, 2e-6, &format!("volume de la forme {id}"));
        }
        // combinaison entièrement évidée : aucun solide, comme dans Revit
        assert!(vol(9900072).is_err());
        // coffre de volet roulant (face cylindrique concave à 24 facettes) : 0,040615 m³ dans Revit
        assert!((vol(9900142).unwrap() - 0.040615).abs() / 0.040615 < 0.005);
        let total: f64 = m.formes.iter().filter_map(|f| f.geometrie.as_ref().ok()).map(|g| g.controle().volume_m3).sum();
        assert!((total - 0.11791).abs() / 0.11791 < 0.002, "volume total {total}");
        assert_eq!(m.categorie_famille(m.formes[0].famille).1, "IFCWINDOW");
        assert_eq!(m.nom_categorie(9880506), "VR");
        assert_eq!(m.murs.len(), 1);
    }

    /// Essai E13 : vues et éléments 2D créés dans Revit 2026 (script RiveTT
    /// `rvt-essai-e13-vues2d`), valeurs relevées par `rvt-reference-vues-2d`.
    #[test]
    fn e13_vues_2d() {
        let Some(p) = essai(r"essais\2026\C14", "E13_vues2d.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.vues.len(), 19);
        let vue = |id: i64| m.vues.iter().find(|v| v.id == id).unwrap();
        let coupe = vue(258483);
        assert_eq!((coupe.nom.as_str(), coupe.genre, coupe.echelle), ("ZZ_COUPE", "Coupe", 100.0));
        assert!(coupe.origine.distance(DVec3::new(3500.0, -2000.0, -150.0)) < 1e-6);
        assert!(coupe.direction.distance(DVec3::X) < 1e-9);
        let (_, _, l) = coupe.cadrage.unwrap();
        assert!((l[0] - 4000.0).abs() < 1e-6 && (l[1] - 3000.0).abs() < 1e-6 && (l[2] - 1000.0).abs() < 1e-6);
        assert_eq!((vue(609).genre, vue(609).niveau), ("Plan d'étage", 608));
        assert_eq!((vue(258471).genre, vue(258471).echelle), ("Vue de dessin", 10.0));
        assert_eq!(vue(258491).genre, "Feuille");
        let el = |id: i64| &m.elements_2d.iter().find(|e| e.id == id).unwrap().contenu;
        let pt = |x, y| DVec3::new(x, y, 0.0);
        match el(258460) {
            Contenu::Trait(p) => assert!(p[0].distance(pt(1000.0, -1000.0)) < 1e-6 && p[1].distance(pt(3456.7, -1000.0)) < 1e-6),
            _ => panic!("ligne"),
        }
        match el(258461) {
            Contenu::Trait(p) => assert!(p[0].distance(pt(4765.4, -2000.0)) < 1e-6 && p.last().unwrap().distance(pt(3234.6, -2000.0)) < 1e-6 && p.len() > 30),
            _ => panic!("arc"),
        }
        match el(258462) {
            Contenu::Texte { texte, point, .. } => assert!(texte == "ZZ_TEXTE_PLAN" && point.distance(pt(1234.5, -2345.6)) < 1e-6),
            _ => panic!("texte"),
        }
        match el(258463) {
            Contenu::Cote { segments, origine, .. } => assert!(segments.len() == 1 && (segments[0].0 - 2456.7).abs() < 1e-6 && origine.distance(pt(1000.0, -1500.0)) < 1e-6),
            _ => panic!("cote"),
        }
        match el(258496) {
            Contenu::FenetreDeVue { vue, echelle, .. } => assert!(*vue == 609 && (*echelle - 0.01).abs() < 1e-12),
            _ => panic!("fenêtre de vue"),
        }
        // région remplie de 2 000 × 1 000 mm : face plane de son GElement
        let r = m.elements_2d.iter().find(|e| e.id == 258466).unwrap();
        let g = r.geometrie.as_ref().unwrap();
        let aire: f64 = g.faces.iter().flat_map(|f| &f.triangles).map(|t| (g.sommets[t[1]] - g.sommets[t[0]]).cross(g.sommets[t[2]] - g.sommets[t[0]]).length() / 2.0).sum();
        proche(aire, 2_000_000.0, 1e-3, "aire de la région");
        let mut classes: Vec<&str> = m.elements_2d.iter().map(|e| e.classe.as_str()).collect();
        classes.sort_unstable();
        classes.dedup_by(|a, b| a == b);
        // les 4 lignes de l'esquisse de la région ne sont pas dans la vue (collecteur de la
        // vue dans l'API, script `rvt-elements-propres-e13`)
        assert_eq!(m.elements_2d.len(), 10, "{classes:?}");
        assert!([258467, 258468, 258469, 258470].iter().all(|id| !m.elements_2d.iter().any(|e| e.id == *id)));
        // scène : un objet tracé par élément 2D visible (sauf la fenêtre de vue), dans le plan
        // de la vue, plus le titre de la fenêtre de vue sur la feuille ; les deux cotes
        // automatiques de l'esquisse, absentes de la vue, ne sont pas tracées
        let s = scene(&m, "E13", "E13_vues2d.rvt".into(), "2026");
        let annot: Vec<_> = s.objects.iter().filter(|o| &*o.class == "IFCANNOTATION").collect();
        assert_eq!(annot.len(), 8, "objets 2D tracés");
        assert!(annot.iter().all(|o| o.opaque.count + o.transparent.count > 0));
    }

    /// Vues et éléments 2D de `FEN_2PF.rfa` : 8 vues, 26 cotes et 4 textes comme dans
    /// Revit (ceux des familles imbriquées écartés), textes lus dans leur RTF.
    #[test]
    fn fenetre_rfa_vues() {
        let Some(p) = essai("fichiers", "FEN_2PF.rfa") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.vues.len(), 8);
        let cotes: HashMap<i64, Vec<f64>> = m
            .elements_2d
            .iter()
            .filter_map(|e| match &e.contenu {
                Contenu::Cote { segments, .. } => Some((e.id, segments.iter().map(|s| s.0).collect())),
                _ => None,
            })
            .collect();
        assert_eq!(cotes.len(), 26);
        for (id, v) in [(9899425, vec![1200.0]), (9899435, vec![2040.0]), (9899422, vec![600.0, 600.0]), (9900193, vec![1020.0, 1020.0])] {
            assert!(cotes[&id].iter().zip(&v).all(|(a, b)| (a - b).abs() < 1e-6) && cotes[&id].len() == v.len(), "cote {id} : {:?}", cotes[&id]);
        }
        let textes: Vec<&str> = m.elements_2d.iter().filter_map(|e| if let Contenu::Texte { texte, .. } = &e.contenu { Some(texte.as_str()) } else { None }).collect();
        assert_eq!(textes, ["Extérieur", "Intérieur", "Extérieur", "Extérieur"]);
    }

    /// Hachures de `Kerreut 2025.rvt` : 219 motifs et types de régions identiques à Revit
    /// (script RiveTT `rvt-reference-hachures`) ; 93 régions : 20 unies, 70 hachurées et
    /// 3 sans type, comme dans l'API ; motifs de coupe et de surface des matériaux
    /// (`rvt-reference-motifs-materiaux`).
    #[test]
    fn kerreut_hachures() {
        let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.motifs.len(), 219);
        let (mut unies, mut hachurees, mut sans_type, mut traits) = (0, 0, 0, 0);
        for e in &m.elements_2d {
            let Contenu::Region { type_id } = &e.contenu else { continue };
            let Some(t) = m.types_regions.get(type_id) else {
                sans_type += 1;
                continue;
            };
            match m.motifs.get(&t.avant) {
                Some(x) if !x.grilles.is_empty() => {
                    let g = e.geometrie.as_ref().unwrap();
                    let v = m.vues.iter().find(|v| v.id == e.vue).unwrap();
                    let face: Vec<[DVec3; 3]> = g.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| g.sommets[i]))).collect();
                    traits += vues::hachures(&face, x, v.echelle, v.droite, v.haut, 20_000).len();
                    hachurees += 1;
                }
                _ => unies += 1,
            }
        }
        assert_eq!((unies, hachurees, sans_type), (20, 70, 3));
        assert!(traits > 1000, "{traits} traits");
        // motif 11 « Diagonale haut » : 45°, 3,175 mm (1/8 po) entre les traits, sur le papier
        let d = &m.motifs[&11];
        assert_eq!(d.nom, "Diagonale haut");
        assert!(!d.modele && d.grilles.len() == 1 && (d.grilles[0].angle.to_degrees() - 45.0).abs() < 1e-9 && (d.grilles[0].decalage - 3.175).abs() < 1e-9);
        // ARC_MAT_PIERRE : coupe 3 245 428, surface 963 733 (couleur 3C3C3C)
        let pierre = &m.materiaux[&9241118];
        assert_eq!((pierre.motifs[0].0, pierre.motifs[2].0, pierre.motifs[2].1), (3245428, 963733, [0x3c, 0x3c, 0x3c]));
    }

    /// Motifs de surface de `Kerreut 2025.rvt` : faces planes calées par leur
    /// `FillPatternPlacer` ; sur un mur en « Blocs 200x400 » (motif 819), les joints de
    /// la grille à 0° sont horizontaux et espacés de 200 mm.
    #[test]
    fn kerreut_motifs_surface() {
        let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let blocs = &m.motifs[&819];
        assert!(blocs.modele && blocs.grilles.len() == 2);
        let (mut faces, mut horizontaux, mut verticaux, mut autres) = (0, 0, 0, 0);
        let mut hauteurs: Vec<f64> = Vec::new();
        for g in m.murs.iter().filter_map(|x| x.geometrie.as_ref().ok()) {
            for f in g.faces.iter().filter(|f| f.motif.is_some_and(|c| c.motif == 819)) {
                let tris: Vec<[DVec3; 3]> = f.triangles.iter().map(|t| t.map(|i| g.sommets[i])).collect();
                let n = (tris[0][1] - tris[0][0]).cross(tris[0][2] - tris[0][0]).normalize();
                if n.z.abs() > 1e-6 {
                    continue; // face non verticale
                }
                faces += 1;
                for [a, b] in vues::motif_sur_face(&tris, &f.motif.unwrap(), blocs, 0.0, 100_000) {
                    let d = (b - a).normalize();
                    if d.z.abs() < 1e-6 {
                        horizontaux += 1;
                        hauteurs.push(a.z * 1000.0);
                    } else if d.z.abs() > 1.0 - 1e-6 {
                        verticaux += 1;
                    } else {
                        autres += 1;
                    }
                }
            }
        }
        assert!(faces > 0 && horizontaux > 0 && verticaux > 0 && autres == 0, "{faces} faces, {horizontaux} / {verticaux} / {autres}");
        // joints horizontaux : altitudes à un multiple de 200 mm près d'un même décalage
        let r0 = hauteurs[0].rem_euclid(200.0);
        let hors = hauteurs.iter().filter(|h| ((*h - r0).rem_euclid(200.0) + 1e-3) % 200.0 > 2e-3).count();
        assert!(hors * 10 < hauteurs.len(), "{hors} joints hors de la trame sur {}", hauteurs.len());
    }

    /// Arborescence du projet comme dans Revit : `FEN_2PF.rfa` (plans, plafond, vue 3D,
    /// élévations « Élévation 1 », familles) et `Projet1.rvt` (feuilles numérotées avec
    /// leurs vues placées, nomenclatures).
    #[test]
    fn navigateur_projet() {
        if let Some(p) = essai("fichiers", "FEN_2PF.rfa") {
            let m = lire(&p, &|_| {}).unwrap();
            let nav = scene(&m, "FEN_2PF", "FEN_2PF.rfa".into(), "2026").navigateur.unwrap();
            let mut vues: Vec<(String, String, String)> = nav.vues.iter().map(|v| (v.genre.clone(), v.type_vue.clone(), v.nom.clone())).collect();
            vues.sort();
            let attendu = [
                ("Plan d'étage", "Plan d'étage", "Niveau de réf."),
                ("Plan de plafond", "Plan de plafond", "Niveau de réf."),
                ("Vue 3D", "Vue 3D", "Vue 1"),
                ("Élévation", "Elévation 1", "Droite"),
                ("Élévation", "Elévation 1", "Extérieur"),
                ("Élévation", "Elévation 1", "Gauche"),
                ("Élévation", "Elévation 1", "Intérieur"),
            ];
            assert_eq!(vues, attendu.map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())));
            assert!(nav.vues.iter().all(|v| v.entite.is_some()));
            assert!(nav.familles.iter().any(|f| f.categorie == "Murs" && f.nom == "Mur de base"));
        }
        let Some(p) = essai("fichiers", "Projet1.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let nav = scene(&m, "Projet1", "Projet1.rvt".into(), "2026").navigateur.unwrap();
        let f = nav.vues.iter().find(|v| v.numero == "A101").unwrap();
        assert_eq!((f.genre.as_str(), f.nom.as_str()), ("Feuille", "RDC - Plan et cotations"));
        let placee = nav.vues.iter().find(|v| v.revit_id == f.placees[0]).unwrap();
        assert_eq!(placee.nom, "RDC - Présentation");
        assert_eq!(nav.vues.iter().filter(|v| v.genre == "Nomenclature").count(), 8);
        assert_eq!(nav.vues.iter().filter(|v| v.genre == "Feuille").count(), 5);
        let portes = nav.familles.iter().filter(|f| f.categorie == "Portes").count();
        assert!(portes > 0);
    }

    /// Essai C14 F04 : la fenêtre chargée dans le projet vide et placée deux fois dans un
    /// mur ; volume de chaque instance 0,110441 m³ dans Revit (coffre : −0,35 %).
    #[test]
    fn c14_fenetres_placees() {
        let Some(p) = essai(r"essais\2026\C14", "F04_deux.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert!(m.formes.is_empty(), "les formes des familles chargées ne sont pas des éléments du projet");
        assert_eq!(m.instances.len(), 2);
        for i in &m.instances {
            let v = i.geometrie.as_ref().unwrap().controle().volume_m3;
            assert!((v - 0.110441).abs() / 0.110441 < 0.002, "instance {} : {v}", i.id);
            assert_eq!(m.categorie(i).1, "IFCWINDOW");
        }
        let i = m.instances.iter().find(|i| i.id == 258460).unwrap();
        assert!(i.point.distance(DVec3::new(2345.6, 0.0, 912.3)) < 1e-6, "{:?}", i.point);
        proche(m.murs[0].geometrie.as_ref().unwrap().controle().volume_m3, 2.968736, 1e-6, "volume du mur percé");
    }

    /// Toposolides de Saint-Malo : 40 terrains, bordures et revêtements, volumes de
    /// `get_Geometry` relevés par le script RiveTT `rvt-reference-toposolides`.
    #[test]
    fn saint_malo_toposolides() {
        let Some(p) = essai("fichiers", "Saint-Malo_avenue aristide briand_46.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.toposolides.len(), 40);
        assert!(m.toposolides.iter().all(|t| t.geometrie.is_ok()));
        let t = |id: i64| m.toposolides.iter().find(|t| t.id == id).unwrap();
        let vol = |id: i64| t(id).geometrie.as_ref().unwrap().controle().volume_m3;
        // terrain « Contexte 300 » (1 133 faces) : 44 893,385 424 m³ dans Revit
        assert!((vol(10126070) - 44893.385424).abs() / 44893.385424 < 1e-6, "{}", vol(10126070));
        assert_eq!(t(10126070).geometrie.as_ref().unwrap().faces.len(), 1133);
        for (id, v) in [(10477748, 20.05992), (11226899, 3.011512), (11020967, 121.657792), (11167178, 0.799032), (11013412, 0.025601)] {
            assert!((vol(id) - v).abs() / v < 1e-4, "toposolide {id} : {} au lieu de {v}", vol(id));
        }
        // niveau et décalage (TOPOSOLID_HEIGHTABOVELEVEL_PARAM) : −100 mm sur le niveau 512 913
        assert_eq!((t(11226899).niveau, t(11226899).type_id), (512913, 10049174));
        proche(t(11226899).decalage, -100.0, 1e-6, "décalage");
        assert_eq!(m.types[&10049174].nom, "Terre20");
    }

    /// Essai E15 : surface topographique de 42 points et sous-région rectangulaire
    /// créées dans Revit 2026 (script RiveTT `rvt-essai-e15-surface-topo`) ; points,
    /// sommets, triangles, aire et boîte du maillage de `get_Geometry` relevés par
    /// `rvt-reference-toposurfaces`.
    #[test]
    fn e15_surface_topo() {
        let Some(p) = essai(r"essais\2026", "E15_surface_topo.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.surfaces_topo.len(), 2);
        for (id, hote, points, sommets, triangles, aire, min, max) in [
            (2797, -1, 42, 58, 84, 829.698725, [0.0, 0.0, -3.308721], [120.406824, 84.645669, 6.806726]),
            (2840, 2797, 24, 24, 26, 120.73165, [26.246719, 19.685039, 1.491963], [65.616798, 52.493439, 5.053267]),
        ] {
            let s = m.surfaces_topo.iter().find(|s| s.id == id).unwrap();
            assert_eq!((s.hote, s.points), (hote, points), "{id}");
            let g = s.geometrie.as_ref().unwrap();
            let c = g.controle();
            assert_eq!((g.sommets.len(), c.triangles), (sommets, triangles), "{id}");
            proche(c.aire_m2, aire, 1e-6, "aire");
            for k in 0..3 {
                proche(c.min[k], min[k] * FT, 0.01, "boîte min");
                proche(c.max[k], max[k] * FT, 0.01, "boîte max");
            }
        }
    }

    /// Feuilles de Kerreut : fenêtres de vue, cartouches et étiquettes, comparés à l'API
    /// de Revit 2026 (scripts RiveTT `rvt-reference-feuilles`, `rvt-compare-feuilles`) :
    /// `GetBoxCenter`, `GetBoxOutline`, `GetTransform` des cartouches.
    #[test]
    fn kerreut_feuilles() {
        let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let vue = |id: i64| m.vues.iter().find(|v| v.id == id).unwrap();
        let fen = |id: i64| match &m.elements_2d.iter().find(|e| e.id == id).unwrap().contenu {
            Contenu::FenetreDeVue { vue: v, decalage, echelle, .. } => (*v, *decalage, *echelle),
            _ => panic!("{id} n'est pas une fenêtre de vue"),
        };
        // centre et boîte (mm) : décalage + contour du cadrage, élargi de 0,01 pied
        for (id, centre, boite) in [
            (9985483, [-134.509, 231.005], [-266.514, 168.778, -2.504, 293.231]),
            (10258761, [-310.344, 212.743], [-415.665, 138.341, -205.024, 287.146]),
            (10226514, [-354.368, 194.746], [-414.919, 165.809, -293.816, 223.682]),
        ] {
            let (v, d, e) = fen(id);
            let f = feuilles::fenetre(d, e, vue(v));
            let c = f.cadre.unwrap().map(|x| x * 1000.0);
            proche((c[0] + c[2]) / 2.0, centre[0], 0.002, "centre x");
            proche((c[1] + c[3]) / 2.0, centre[1], 0.002, "centre y");
            let marge = 0.01 * FT;
            for (k, b) in boite.iter().enumerate() {
                proche(c[k] + if k < 2 { -marge } else { marge }, *b, 0.002, "boîte");
            }
            // l'origine de la vue tombe sur le décalage
            let o = f.vers_feuille.transform_point3(vue(v).origine / 1000.0) * 1000.0;
            proche(o.x, d.x, 1e-6, "origine x");
            proche(o.y, d.y, 1e-6, "origine y");
        }
        // cartouche réduit de PRO 05 : repère, traits du type, étiquettes remplies
        let i = m.instances_vue.iter().find(|i| i.id == 10168802).unwrap();
        assert_eq!((i.vue, i.type_id), (10168795, 9791176));
        proche(i.origine.x, -1190.793, 0.001, "origine du cartouche");
        assert_eq!(m.dessins[&9791176].traits.len(), 23);
        assert_eq!(m.dessins[&8966185].traits.len(), 7);
        let doc = m.familles[&m.types[&9791176].famille].document.unwrap();
        assert_eq!(m.textes_familles[&doc].iter().filter(|e| matches!(e.contenu, Contenu::Etiquette { .. })).count(), 7);
        assert_eq!(feuilles::police(&m, 9791121).0, "Arial");
        proche(feuilles::police(&m, 9791121).1, 2.6, 1e-9, "taille de police");
        let f = vue(10168795);
        let val = |p: i64| feuilles::valeur_cartouche(&m, f, 10168802, p).unwrap_or_default();
        assert_eq!(val(-1006319), "Nicolas et Isabelle MUSSET");
        assert_eq!(val(-1007401), "PRO 05");
        assert_eq!(val(-1007400), "Plan RDC");
        assert_eq!(val(-1007402), "1 : 50");
        assert_eq!(val(-1007414), "DOSSIER PRE-PRO");
        assert_eq!(val(-1007415), "23/03/2024");
        assert_eq!(val(-1006320), "PCMI");
        // 66 instances sur les feuilles, toutes dessinées
        let feuilles: std::collections::HashSet<i64> = m.vues.iter().filter(|v| v.genre == "Feuille").map(|v| v.id).collect();
        let sur_feuilles: Vec<&InstanceVue> = m.instances_vue.iter().filter(|i| feuilles.contains(&i.vue)).collect();
        assert_eq!(sur_feuilles.len(), 66);
        assert!(sur_feuilles.iter().all(|i| m.dessins.contains_key(&i.type_id)));
    }

    /// Feuilles de `Projet1.rvt` (sans cartouche) et d'E13 : boîtes des fenêtres de vue
    /// comparées à `GetBoxOutline` de Revit 2026 (`rvt-compare-feuilles`) ; titre de la
    /// fenêtre de vue d'E13 (« Niveau 0 », « 1 : 100 », numéro de détail).
    #[test]
    fn projet1_e13_feuilles() {
        let marge = 0.01 * FT;
        if let Some(p) = essai("fichiers", "Projet1.rvt") {
            let m = lire(&p, &|_| {}).unwrap();
            // plans cadrés : boîte exacte ; élévation : centre exact, boîte de Revit élargie
            // de 25,598 mm à gauche et à droite par les niveaux qui sortent du cadrage
            for (id, boite) in [(258041, [-118.048, -78.048, 118.048, 78.048]), (258042, [-118.048, -85.548, 118.048, 85.548]), (258010, [-273.118, -158.048, 273.118, 158.048])] {
                let e = m.elements_2d.iter().find(|e| e.id == id).unwrap();
                let Contenu::FenetreDeVue { vue, decalage, echelle, .. } = &e.contenu else { panic!() };
                let v = m.vues.iter().find(|v| v.id == *vue).unwrap();
                let c = feuilles::fenetre(*decalage, *echelle, v).cadre.unwrap().map(|x| x * 1000.0);
                for (k, b) in boite.iter().enumerate() {
                    proche(c[k] + if k < 2 { -marge } else { marge }, *b, 0.002, "boîte");
                }
            }
        }
        let Some(p) = essai(r"essais\2026\C14", "E13_vues2d.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let e = m.elements_2d.iter().find(|e| e.id == 258496).unwrap();
        let Contenu::FenetreDeVue { vue, decalage, echelle, titre, longueur_titre, type_id } = &e.contenu else { panic!() };
        let v = m.vues.iter().find(|v| v.id == *vue).unwrap();
        // vue non cadrée : pas de découpe ; l'origine de la vue tombe sur le décalage
        let f = feuilles::fenetre(*decalage, *echelle, v);
        assert!(f.cadre.is_none());
        proche(decalage.x, 370.0, 1e-9, "décalage");
        // titre : m_labelOrigin = coin de la boîte + LabelOffset de l'API (370 ; 252,774)
        proche(titre.x, 370.0, 1e-3, "titre x");
        proche(titre.y, 252.774, 1e-3, "titre y");
        proche(*longueur_titre, 60.0, 1e-6, "ligne de titre");
        assert!(m.types_fenetres[type_id].titre);
        assert_eq!(feuilles::valeur_titre(&m, e, v, -1_005_203).as_deref(), Some("Niveau 0"));
        assert_eq!(feuilles::valeur_titre(&m, e, v, -1_005_204).as_deref(), Some("1 : 100"));
        let feuille = m.vues.iter().find(|x| x.id == 258491).unwrap();
        let titres: Vec<_> = feuilles::objets_vue(&m, feuille).into_iter().filter(|o| o.genre == "Titre de vue").collect();
        assert_eq!(titres.len(), 1);
        assert!(titres[0].parts.len() >= 3, "régions, traits et textes du titre");
    }

    /// Titres des fenêtres de vue de Kerreut (feuille PCMI3) : « Titre sur la feuille »
    /// à la place du nom, numéro de détail masqué par le paramètre « Numero » du type,
    /// comme dans l'export de la feuille par Revit.
    #[test]
    fn kerreut_titres_de_vue() {
        let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let e = m.elements_2d.iter().find(|e| e.id == 9985364).unwrap();
        let v = m.vues.iter().find(|v| v.id == 9985266).unwrap();
        assert_eq!((v.nom.as_str(), v.titre_feuille.as_str()), ("PC A", "COUPE AA"));
        assert_eq!(feuilles::valeur_titre(&m, e, v, -1_005_203).as_deref(), Some("COUPE AA"));
        let t = &m.types_fenetres[&9347557];
        assert!(t.titre && !t.ligne && t.famille_titre == 9347548);
        // étiquettes du titre : nom et échelle visibles, numéro de détail masqué
        let doc = m.familles[&m.types[&9347548].famille].document.unwrap();
        let etiquettes: Vec<&Element2D> = m.textes_familles[&doc].iter().filter(|x| matches!(x.contenu, Contenu::Etiquette { .. })).collect();
        assert_eq!(etiquettes.len(), 3);
        let numero = etiquettes.iter().find(|x| x.id == 9347483).unwrap();
        assert_eq!(m.params_embarques[&numero.visible_si], "Numero");
        // cadrage d'annotation actif sur A 50e, inactif sur PC A (paramètre de l'API)
        assert!(m.vues.iter().find(|v| v.id == 10168711).unwrap().cadre.unwrap().annotation_active);
        assert!(!v.cadre.unwrap().annotation_active);
    }

    /// Cotes de Kerreut comme dans Revit 2026 : visibles seulement si chaque élément coté
    /// est dans la vue (collecteur de la vue, script `rvt-visibilite-elements-vue`),
    /// lignes d'esquisse écartées, catégories masquées (`GetCategoryHidden`, script
    /// `rvt-categories-masquees`), valeurs au format du type (centimètres) ou remplacées.
    #[test]
    fn kerreut_cotes() {
        let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let boites = feuilles::boites(&m);
        let vue = |id: i64| m.vues.iter().find(|v| v.id == id).unwrap();
        let el = |id: i64| m.elements_2d.iter().find(|e| e.id == id).unwrap();
        let visible = |e: i64, v: i64| feuilles::visible_dans_vue(&m, el(e), vue(v), &boites);
        // « RDC 50e » : 10168246 visible ; 10168254 cote un mur hors cadrage
        assert!(visible(10168246, 10168217));
        assert!(!visible(10168254, 10168217));
        // « PC Est » : cotes accrochées à des lignes d'esquisse, absentes de la vue
        assert!(m.lignes_esquisse.contains(&9985379));
        assert!(!m.elements_2d.iter().any(|e| e.id == 9985379));
        assert!(!visible(10085862, 9985368) && !visible(10104460, 9985368));
        // catégories masquées : les 10 de l'API (niveaux compris) sont dans la liste lue
        let api = [-2009645, -2008186, -2008185, -2003400, -2001115, -2001113, -2000535, -2000269, -2000240, -2000200];
        assert!(api.iter().all(|c| vue(9985368).categories_masquees.contains(c)));
        assert_eq!(vue(9985368).masques, vec![9722834, 9916200, 9988805]);
        // type de cote « Cotes_2.25 mm_Noir_M 1 » : centimètres à l'unité, gras
        let t = &m.types_cotes[&9706670];
        assert_eq!(t.format.as_ref().map(|f| (f.0.as_str(), f.1)), Some(("autodesk.unit.unit:centimeters-1.0.1", 1.0)));
        assert!(t.gras && !t.attache_fixe);
        proche(t.decalage_texte, 1.5, 1e-9, "décalage du texte");
        let Contenu::Cote { segments, textes, .. } = &el(10168246).contenu else { panic!() };
        assert_eq!(vues::texte_longueur(segments[0].0, t.format.as_ref()), "321");
        // valeurs remplacées de la coupe « PC A » : « GARAGE », « MAISON »
        let Contenu::Cote { textes: tx, .. } = &el(10020238).contenu else { panic!() };
        assert_eq!(tx.iter().map(|t| t.remplacement.as_str()).collect::<Vec<_>>(), ["GARAGE", "MAISON"]);
        assert!(textes.iter().all(|t| t.remplacement.is_empty()));
    }

    /// Format des longueurs : unité et précision du type de cote ou du projet.
    #[test]
    fn format_longueurs() {
        let cm = ("autodesk.unit.unit:centimeters-1.0.1".to_string(), 1.0);
        assert_eq!(vues::texte_longueur(5650.0, Some(&cm)), "565");
        let m = ("autodesk.unit.unit:meters-1.0.0".to_string(), 0.01);
        assert_eq!(vues::texte_longueur(2456.7, Some(&m)), "2.46");
        assert_eq!(vues::texte_longueur(2456.7, None), "2.46");
        let mm = ("autodesk.unit.unit:millimeters-1.0.0".to_string(), 0.5);
        assert_eq!(vues::texte_longueur(12.3, Some(&mm)), "12.5");
    }

    /// Paramètres de `FEN_2PF.rfa` : valeurs du type courant (`Family.m_familyParams`)
    /// contre FamilyManager dans Revit 2026, type courant « 120x204ht 16 » (script RiveTT
    /// `rvt-reference-parametres-famille`).
    #[test]
    fn fenetre_rfa_parametres() {
        let Some(p) = essai("fichiers", "FEN_2PF.rfa") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.famille_document, Some(9_899_388));
        let aff = m.parametres_affiches(9_899_388);
        let val = |nom: &str| aff.iter().find(|a| a.nom == nom).map(|a| (a.groupe, a.valeur.as_str()));
        assert_eq!(val("Distance résa"), Some(("Autre", "110")), "formule if(…, 110 mm, 10 mm), unités de la famille : mm sans symbole");
        assert_eq!(val("Nu pose"), Some(("Cotes", "160")));
        assert_eq!(val("Seuil PMR"), Some(("Cotes", "Oui")));
        assert_eq!(val("Allège supérieure à 12cm (equerre)"), Some(("Autre", "Non")), "formule not(…)");
        assert_eq!(val("Hauteur"), Some(("Cotes", "2040")));
        assert_eq!(val("Largeur"), Some(("Cotes", "1200")));
        assert_eq!(val("Description"), Some(("Données d'identification", "PF + FIXE")));
        assert_eq!(val("Commentaires du type"), Some(("Données d'identification", "M02")));
        let d = &m.definitions[&9_900_475];
        assert_eq!((d.nom.as_str(), d.genre.as_str(), d.occurrence), ("Distance résa", "famille", Some(false)));
        assert_eq!(m.unique_id(9_899_388).as_deref(), Some("79dc52d9-c475-4841-a47e-5c94d4e238e2-00970d7c"));
    }

    /// Paramètres et IfcGUID de `Kerreut 2025.rvt` contre l'API de Revit 2026 (script
    /// RiveTT `rvt-reference-parametres-projet`) : porte coulissante 9911752, dont la
    /// largeur est un paramètre d'occurrence de sa famille, son type 8677076 et un
    /// paramètre partagé.
    #[test]
    fn kerreut_parametres() {
        let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let val = |id: i64, nom: &str| m.parametres_affiches(id).into_iter().find(|a| a.nom == nom).map(|a| a.valeur);
        assert_eq!(val(9_911_752, "Largeur").as_deref(), Some("1.30"));
        assert_eq!(val(9_911_752, "Identifiant").as_deref(), Some("2275"));
        assert_eq!(val(9_911_752, "Niveau").as_deref(), Some("RDC"));
        assert_eq!(val(9_911_752, "Famille et type").as_deref(), Some("ARC_PTE_Placard coulissante_multi-panneaux: Largeur 1 m mini_largeur panneaux max 90 cm"));
        // le type ne montre pas la largeur, paramètre d'occurrence de sa famille
        assert_eq!(val(8_677_076, "Largeur"), None);
        assert_eq!(val(8_677_076, "Hauteur").as_deref(), Some("2.10"));
        assert_eq!(val(8_677_076, "Identifiant du type").as_deref(), Some("PL03"));
        assert_eq!(val(8_677_076, "Opération").as_deref(), Some("SlidingToRight"));
        assert_eq!(m.guid_ifc(9_911_752).as_deref(), Some("3Sv4yF$LD1uBYRSvD2Wzs7"));
        assert_eq!(m.guid_ifc(8_677_076).as_deref(), Some("2VbQHXdHTBoAiSW3rsPCNH"));
        let d = &m.definitions[&208_407];
        assert_eq!((d.nom.as_str(), d.genre.as_str(), d.guid.as_deref()), ("Adresse du Client", "partagé", Some("f25fdde5-ae9e-422f-87c4-1d9e5fc71836")));
    }

    /// Coordonnées contre l'API de Revit 2026 (script RiveTT `rvt-reference-coordonnees`,
    /// `Decodage RVT/references/coordonnees.json`) : essai E16 (`rvt-essai-e16-coordonnees` :
    /// emplacement actif placé, second emplacement, point de base déplacé, lieu), Kerreut
    /// (rotation et altitude réelles) et Saint-Malo.
    #[test]
    fn coordonnees() {
        let pied = |v: f64| v * FT;
        for (fichier, nom, emplacements, position, angle, base, base_partage, lieu) in [
            (
                essai(r"essais\2026", "E16_coordonnees.rvt"),
                "Site par défaut",
                2,
                [1_152_344.379_921_258_4, 22_273_662.549_212_597, 149.862_204_724_409_45],
                0.409_384_429_347_789_9,
                [16.404_199_475_065_617, 22.965_879_265_091_864, 3.280_839_895_013_123_5],
                [1_152_350.287_129_944, 22_273_690.146_934_54, 153.143_044_619_422_57],
                (47.2184, -1.5536, "ZZ_LIEU_NANTES"),
            ),
            (essai("fichiers", "Kerreut 2025.rvt"), "Interne", 1, [0.0, 0.0, 226.049_868_766_404_02], 0.729_962_381_603_939_6, [0.0; 3], [0.0, 0.0, 226.049_868_766_404_02], (48.113_518, -1.669_897, "48.1135177612305,-1.66989731788635")),
            (essai("fichiers", "Saint-Malo_avenue aristide briand_46.rvt"), "Interne", 1, [0.0, 0.0, 25.426_509_186_351_172], -0.031_576_952_957_789_395, [0.0; 3], [0.0, 0.0, 25.426_509_186_351_172], (48.113_518, -1.669_897, "48.1135177612305,-1.66989731788635")),
        ] {
            let Some(p) = fichier else { continue };
            let m = lire(&p, &|_| {}).unwrap();
            let c = &m.coordonnees;
            let e = c.actif().unwrap();
            assert_eq!((e.nom.as_str(), c.nommes().count()), (nom, emplacements), "{}", p.display());
            let o = c.partage(DVec3::ZERO).unwrap();
            for k in 0..3 {
                proche(o[k], pied(position[k]), 1e-3, "origine interne en coordonnées partagées");
                proche(c.point_base.unwrap()[k], pied(base[k]), 1e-6, "point de base");
                proche(c.partage(c.point_base.unwrap()).unwrap()[k], pied(base_partage[k]), 1e-3, "point de base partagé");
            }
            proche(e.angle(), angle, 1e-12, "angle par rapport au nord géographique");
            let l = &c.lieux[&e.lieu];
            proche(l.latitude, lieu.0, 1e-6, "latitude");
            proche(l.longitude, lieu.1, 1e-6, "longitude");
            assert_eq!(l.nom, lieu.2);
        }
    }

    /// Essai E17 (`rvt-essai-e17-scg-nord`, depuis E16) : système EPSG:2154 et un plan
    /// orienté au nord géographique ; valeurs de Revit 2026 (`rvt-reference-scg-nord`).
    #[test]
    fn e17_systeme_et_nord() {
        let Some(p) = essai(r"essais\2026", "E17_scg_nord.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let c = &m.coordonnees;
        let l = &c.lieux[&c.actif().unwrap().lieu];
        assert_eq!((l.systeme.as_str(), l.epsg), ("Lambert93", Some(2154)));
        proche(l.latitude, 0.839_648_926_828_889_7_f64.to_degrees(), 1e-9, "latitude");
        proche(l.longitude, (-0.029_473_898_594_148_437_f64).to_degrees(), 1e-9, "longitude");
        assert_eq!(l.nom, "48.1083397799843,-1.6887299952413");
        let nord = |id: i64| m.vues.iter().find(|v| v.id == id).unwrap().nord_geographique;
        assert!(nord(1979) && !nord(1960) && !nord(1970));
    }

    /// Phases de Saint-Malo contre l'API de Revit 2026 (`rvt-reference-phases`) : ordre,
    /// filtre « Conservé + Projet », élément créé en Existant et démoli en Démolition.
    #[test]
    fn phases_saint_malo() {
        let Some(p) = essai("fichiers", "Saint-Malo_avenue aristide briand_46.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        assert_eq!(m.ordre_phases, [3, 354_698, 0]);
        assert_eq!(m.phases[&0], "Projet");
        assert_eq!(m.filtres_phase[&751], ("Conservé + Projet".to_string(), [0, 0, 1, 0, 1, 0, 0]));
        let ph = |k| id_parametre(&m, 10_126_105, k);
        assert_eq!((ph(-1_012_100), ph(-1_012_101)), (Some(3), Some(354_698)));
        let etats: Vec<usize> = m.ordre_phases.iter().map(|&q| scene::etat_de_phase(&m.ordre_phases, 3, 354_698, q)).collect();
        assert_eq!(etats, [4, 3, 1]);
    }

    /// Plages de vue contre l'API de Revit 2026 (`rvt-reference-plages-vue`) : plan
    /// SP/ST_RDC de Kerreut (niveau RDC) et gabarit au niveau associé (−3).
    #[test]
    fn plages_de_vue_kerreut() {
        let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let v = m.vues.iter().find(|v| v.id == 2_519_142).unwrap();
        let attendu = [(608, 7.545_931_758_530_182), (608, 3.937_007_874_015_747_3), (608, 0.0), (608, 0.0)];
        for (lu, (n, d)) in v.plage_vue.unwrap().iter().zip(attendu) {
            assert_eq!(lu.0, n);
            proche(lu.1, d * FT, 1e-6, "décalage");
        }
        assert_eq!(texte_plage(&m, v), "Haut : RDC +2300 mm ; Plan de coupe : RDC +1200 mm ; Bas : RDC +0 mm ; Profondeur de vue : RDC +0 mm");
        let g = m.vues.iter().find(|v| v.id == 6_965_030).unwrap();
        assert!(g.plage_vue.unwrap().iter().all(|x| x.0 == -3));
    }

    /// Nomenclatures de Projet1 contre le tableau affiché par Revit 2026
    /// (`rvt-reference-nomenclatures`) : définition (catégorie, détail, champs, filtre,
    /// tri) et contenu de la nomenclature détaillée des portes (43 lignes).
    #[test]
    fn nomenclatures_projet1() {
        let Some(p) = essai("fichiers", "Projet1.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let def = |id: i64| m.vues.iter().find(|v| v.id == id).and_then(|v| v.nomenclature.clone()).unwrap();
        let portes = def(76_991);
        assert_eq!((portes.categorie, portes.detail, portes.champs.len()), (-2_000_023, true, 8));
        let f = &portes.filtres[0];
        assert_eq!((f.champ, f.genre, &f.valeur), (7, 1, &parametres::Brut::Entier(0)));
        assert!(portes.tris[0].entete && portes.tris[0].pied && portes.tris[0].ligne_vide);
        let t = super::super::nomenclatures::calculer(&m, &def(257_959));
        assert_eq!(t.titres, ["Identifiant", "Famille et type", "Niveau", "Largeur", "Hauteur"]);
        assert_eq!(t.lignes.len(), 43);
        assert_eq!(t.lignes[1], ["P4", "Int. Simple: PP (0.83m x 2.04m)", "Niveau 0", "0.83", "2.04"]);
    }

    /// Essai E18 (`rvt-essai-e18-formules`) : une formule par fonction, évaluée avec les
    /// valeurs du type courant, contre la valeur que donne FamilyManager dans Revit 2026
    /// (`Decodage RVT/references/formules_e18.json`).
    #[test]
    fn formules_e18() {
        use super::super::formules::{de_brut, evaluer, Expr, Valeur};
        use super::super::parametres::Stockage;
        let Some(p) = essai(r"essais\2026", "E18_formules.rfa") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let (fid, f) = m.familles.iter().find(|(_, f)| !f.formules.is_empty()).unwrap();
        let vals = &m.parametres[fid];
        let st = |p: i64| m.definitions.get(&p).map(|d| d.stockage).unwrap_or(Stockage::Aucun);
        let v = |p: i64| vals.iter().find(|(q, _)| *q == p).map(|(_, b)| de_brut(b, st(p)));
        let var = |e: &Expr| match e {
            Expr::Param(q) => v(*q),
            _ => None,
        };
        let calcul = |id: i64| f.formules.iter().find(|(p, _)| *p == id).and_then(|(_, ex)| evaluer(ex, &var));
        let revit = [
            (2942, 0.5), (2943, 0.866_025_403_784_438_7), (2944, 0.577_350_269_189_625_7), (2945, 0.523_598_775_598_298_9), (2946, 1.047_197_551_196_597_9),
            (2947, 0.785_398_163_397_448_3), (2948, 9.974_182_454_814_718), (2949, 0.361_727_836_017_592_84), (2951, 1.516_575_088_810_31), (2952, 2.7),
            (2953, std::f64::consts::PI), (2954, 2.0), (2955, 3.0), (2956, 2.0), (2957, -3.0), (2958, -2.0), (2959, -3.0), (2960, 3.0), (2961, 5.29),
            (2962, -2.3), (2964, -0.4), (2965, 1.15), (2966, 1.0), (2967, 0.0), (2968, 1.0), (2969, 0.0), (2970, 1.0), (2971, 1.0), (2976, 20.0),
        ];
        for (id, attendu) in revit {
            let x = calcul(id).and_then(|x| x.nombre()).unwrap_or(f64::NAN);
            proche(x, attendu, 1e-9, &format!("formule {id}"));
        }
        assert_eq!(calcul(2975), Some(Valeur::Texte("grand".into())));
        assert_eq!(f.formules.len(), 30);
    }

    /// Champ pourcentage et champ calculé de Saint-Malo contre le tableau de Revit 2026
    /// (`rvt-reference-nomenclatures`) : « Part de SP » = « % de SHAB » × 5 000 m², le
    /// pourcentage étant la part de la surface de la pièce dans le total général.
    #[test]
    fn nomenclature_formule_saint_malo() {
        let Some(p) = essai("fichiers", "Saint-Malo_avenue aristide briand_46.rvt") else { return };
        let m = lire(&p, &|_| {}).unwrap();
        let d = m.vues.iter().find(|v| v.id == 9_190_140).and_then(|v| v.nomenclature.clone()).unwrap();
        let t = super::super::nomenclatures::calculer(&m, &d);
        let ligne = |n: &str| t.lignes.iter().find(|l| l[0] == n).cloned().unwrap();
        assert_eq!(ligne("A110"), ["A110", "T1", "", "20.5 m²", "1%", "42.3 m²"]);
        assert_eq!(ligne("A119"), ["A119", "T1", "", "33.4 m²", "1%", "68.9 m²"]);
        assert_eq!(&ligne("Total général")[4], "100%");
    }

    /// Formules des familles évaluées avec les valeurs de chaque type, contre la valeur
    /// calculée que Revit a enregistrée dans le type (une formule entière est arrondie).
    #[test]
    fn formules_familles() {
        use super::super::formules::{de_brut, evaluer, Expr};
        use super::super::parametres::Stockage;
        for (fichier, minimum) in [("Kerreut 2025.rvt", 161), ("Saint-Malo_avenue aristide briand_46.rvt", 411)] {
            let Some(p) = essai("fichiers", fichier) else { continue };
            let m = lire(&p, &|_| {}).unwrap();
            let st = |p: i64| m.definitions.get(&p).map(|d| d.stockage).or_else(|| parametres::integres().get(&p).map(|i| i.stockage)).unwrap_or(Stockage::Aucun);
            let (mut justes, mut sans, mut faux) = (0, 0, Vec::new());
            for (t, ty) in &m.types {
                let (Some(f), Some(vals)) = (m.familles.get(&ty.famille), m.parametres.get(t)) else { continue };
                let v = |p: i64| vals.iter().find(|(q, _)| *q == p).map(|(_, b)| de_brut(b, st(p)));
                for (p, ex) in &f.formules {
                    let Some(attendu) = v(*p).and_then(|x| x.nombre()) else { continue };
                    let var = |e: &Expr| match e {
                        Expr::Param(q) => v(*q),
                        _ => None,
                    };
                    match evaluer(ex, &var).and_then(|x| x.nombre()) {
                        Some(x) => {
                            let x = if st(*p) == Stockage::Entier { x.round() } else { x };
                            if (x - attendu).abs() <= 1e-6 * attendu.abs().max(1.0) {
                                justes += 1
                            } else {
                                faux.push((*t, *p, x, attendu))
                            }
                        }
                        None => sans += 1,
                    }
                }
            }
            eprintln!("{fichier} : {justes} justes, {sans} sans valeur");
            assert!(faux.is_empty() && justes >= minimum, "{fichier} : {justes} justes, faux {faux:?}");
        }
    }
}
