//! Éléments lus directement dans les partitions d'un RVT, et leur traduction en scène du
//! viewer (portage de `champs.murs`, `elements.*` et `geometrie.maillage` de
//! `tools/rvt-research/`, plus toits, escaliers, grilles et matériaux).
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
use super::geometrie::{self, FT, Maillage};
use super::objets::{elem_table, inflate, records, sections, unpad};
use super::schema::Schema;
use super::vues::{self, Contenu, Element2D, Motif, TypeRegion, Vue};
use super::Cfb;
use crate::geom::{Ligne, Part, ProductGeom, Rgba, class_color};
use crate::model::IfcModel;
use crate::scene::{self, Scene};
use crate::step::{Entity, Id, IdMap, StepFile, Value, format_real};
use glam::DVec3;
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
}

pub struct Famille {
    pub nom: String,
    /// Catégorie (BuiltInCategory).
    pub categorie: i64,
    /// Famille hôte d'une famille imbriquée (`Element.m_famId`), −1 pour la famille d'un
    /// document `.rfa` ou une famille chargée dans un projet.
    pub parent: i64,
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
    /// Instances propres à une vue, ignorées.
    pub instances_de_vue: usize,
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
}

impl Sorte {
    /// Éléments dont on lit le complément 0x67 (géométrie).
    fn a_geometrie(self) -> bool {
        matches!(self, Sorte::Mur | Sorte::Dalle | Sorte::Toposolide | Sorte::Toit | Sorte::Volee | Sorte::GardeCorps | Sorte::Instance | Sorte::Piece | Sorte::Forme | Sorte::Region)
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
            "MaterialElem" => Sorte::Materiau,
            "StairsElement" => Sorte::Escalier,
            "Grid" => Sorte::Grille,
            "RoomElem" => Sorte::Piece,
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
            // marqueurs de vue (Section, Elevation, Viewer3d…) : cadrage de leur vue
            _ if h("Viewer") => Sorte::Viewer,
            "FilledRegion" => Sorte::Region,
            "FillPatternElem" => Sorte::Motif,
            "GStyleElem" => Sorte::Style,
            "CurveElem" | "TextNote" | "LinearDimString" | "RadialDim" | "AngularDim" | "ArcLengthDim" | "Viewport" => Sorte::Annotation,
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
    let mut complements: HashMap<i64, Vec<u8>> = HashMap::new();
    let mut geometries_types: HashMap<i64, Vec<u8>> = HashMap::new();
    for (k, (_, p)) in parts.iter().enumerate() {
        progress(format!("Partition {} / {} ({p})…", k + 1, parts.len()));
        let raw = cfb.read(p)?.ok_or_else(|| format!("flux {p} illisible"))?;
        let sec = sections(&raw).map_err(|e| format!("{p} : {e}"))?;
        let vide = Vec::new();
        for (eid, body) in records(sec.get(&0x66).unwrap_or(&vide), true, ids8).map_err(|e| format!("{p}, section 0x66 : {e}"))? {
            let Some(c) = champs::class_id(body).filter(|_| table.contains(&eid)) else { continue };
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
    for eid in ids.iter().filter(|e| objets[e].0 == Sorte::Niveau) {
        match lire_objet(eid) {
            Some(Ok((_, r))) => niveaux.extend(niveau(*eid, &r)),
            Some(Err(e)) => illisibles.push(e),
            None => {}
        }
    }
    niveaux.sort_by(|a, b| a.altitude.total_cmp(&b.altitude).then(a.id.cmp(&b.id)));
    let altitude: HashMap<i64, f64> = niveaux.iter().map(|n| (n.id, n.altitude)).collect();
    let extraits: Vec<Result<Extrait, (i64, String)>> = ids
        .par_iter()
        .filter(|e| objets[e].0 != Sorte::Niveau)
        .filter_map(|eid| Some(lire_objet(eid)?.map(|(s, r)| extraire(*eid, s, &r, &altitude))))
        .collect();
    drop(objets);

    let (mut types, mut familles, mut materiaux) = (HashMap::new(), HashMap::new(), HashMap::new());
    let (mut murs, mut dalles, mut toits, mut escaliers, mut grilles, mut pieces) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut toposolides = Vec::new();
    let (mut volees, mut instances, mut instances_de_vue, mut garde_corps) = (HashMap::new(), Vec::new(), 0, Vec::new());
    let (mut formes, mut categories) = (Vec::new(), HashMap::new());
    let (mut vues, mut cadrages, mut elements_2d) = (Vec::new(), HashMap::new(), Vec::new());
    let (mut motifs, mut types_regions, mut couleurs_categories) = (HashMap::new(), HashMap::new(), HashMap::new());
    for x in extraits {
        match x {
            Err(e) => illisibles.push(e),
            Ok(Extrait::Rien) => {}
            Ok(Extrait::Type(id, t)) => drop(types.insert(id, t)),
            Ok(Extrait::Famille(id, f)) => drop(familles.insert(id, f)),
            Ok(Extrait::Materiau(id, m)) => drop(materiaux.insert(id, m)),
            Ok(Extrait::Mur(m)) => murs.push(m),
            Ok(Extrait::Dalle(d)) => dalles.push(d),
            Ok(Extrait::Toposolide(d)) => toposolides.push(d),
            Ok(Extrait::Toit(t)) => toits.push(t),
            Ok(Extrait::Escalier(e)) => escaliers.push(e),
            Ok(Extrait::Volee(id, esc)) => drop(volees.insert(id, esc)),
            Ok(Extrait::GardeCorps(g)) => garde_corps.push(g),
            Ok(Extrait::Grille(g)) => grilles.push(g),
            Ok(Extrait::Instance(i, h)) => instances.push((i, h)),
            Ok(Extrait::InstanceDeVue) => instances_de_vue += 1,
            Ok(Extrait::Piece(p)) => pieces.push(p),
            Ok(Extrait::Forme(f)) => formes.push(f),
            Ok(Extrait::Categorie(id, nom)) => drop(categories.insert(id, nom)),
            Ok(Extrait::Vue(v)) => vues.push(v),
            Ok(Extrait::Cadrage(v, c)) => drop(cadrages.insert(v, c)),
            Ok(Extrait::Element2D(e)) => elements_2d.push(e),
            Ok(Extrait::Motif(id, x)) => drop(motifs.insert(id, x)),
            Ok(Extrait::TypeRegion(id, x)) => drop(types_regions.insert(id, x)),
            Ok(Extrait::Style(cat, c)) => drop(couleurs_categories.insert(cat, c)),
        }
    }
    // vues et éléments 2D du document : ceux des familles imbriquées rangées en ligne
    // dans un .rfa (symboles d'annotation) sont écartés
    let du_document = |f: i64| f == -1 || familles.get(&f).is_some_and(|x: &Famille| x.parent == -1);
    vues.retain(|v: &Vue| du_document(v.famille));
    for v in &mut vues {
        v.cadrage = cadrages.remove(&v.id);
    }
    vues.sort_by_key(|v| v.id);
    let ids_vues: std::collections::HashSet<i64> = vues.iter().map(|v| v.id).collect();
    elements_2d.retain(|e: &Element2D| du_document(e.famille) && ids_vues.contains(&e.vue));
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

    // Géométrie, en parallèle.
    let mut a_mailler: Vec<i64> = murs.iter().map(|m| m.id).chain(dalles.iter().map(|d| d.id)).chain(toits.iter().map(|t| t.id)).collect();
    a_mailler.extend(toposolides.iter().map(|t| t.id));
    a_mailler.extend(escaliers.iter().flat_map(|e| e.parties.iter().copied()));
    a_mailler.extend(garde_corps.iter().map(|g| g.id));
    a_mailler.extend(instances.iter().map(|i| i.id).chain(pieces.iter().map(|p| p.id)));
    a_mailler.extend(formes.iter().map(|f| f.id));
    a_mailler.extend(elements_2d.iter().filter(|e| matches!(e.contenu, Contenu::Region { .. })).map(|e| e.id));
    progress(format!("Géométrie de {} éléments…", a_mailler.len()));
    progress(format!("Géométrie de {} types ({} Mo)…", geometries_types.len(), geometries_types.values().map(Vec::len).sum::<usize>() >> 20));
    let bib = geometrie::Bibliotheque::new(&sch, &geometries_types);
    let regions: std::collections::HashSet<i64> = elements_2d.iter().filter(|e| matches!(e.contenu, Contenu::Region { .. })).map(|e| e.id).collect();
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
        instances_de_vue,
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
    }
    for x in &mut m.formes {
        x.geometrie = prendre(x.id);
    }
    for x in m.elements_2d.iter_mut().filter(|e| matches!(e.contenu, Contenu::Region { .. })) {
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
    Toit(Toit),
    Escalier(Escalier),
    /// Volée ou palier et son escalier (`m_stairsId`).
    Volee(i64, i64),
    GardeCorps(GardeCorps),
    Grille(Grille),
    /// Instance et, si elle est hébergée, (`m_hostParam`, `m_elevation` en mm) pour
    /// placer son point sur la ligne de l'hôte.
    Instance(Instance, Option<(f64, f64)>),
    InstanceDeVue,
    Piece(Piece),
    Forme(Forme),
    Categorie(i64, String),
    Vue(Vue),
    /// Cadrage (origine, axes, longueurs) de la vue d'un `Viewer`.
    Cadrage(i64, (DVec3, [DVec3; 3], [f64; 3])),
    Element2D(Element2D),
    Motif(i64, Motif),
    TypeRegion(i64, TypeRegion),
    /// Couleur de projection (catégorie, rouge vert bleu).
    Style(i64, [u8; 3]),
}

fn niveau(id: i64, r: &Record) -> Option<Niveau> {
    let z = r.field_body(&r.root, "m_pSurface")?.get("m_origin")?.reals()?.get(2).copied()?;
    Some(Niveau { id, nom: r.root.get("m_text")?.str()?.to_string(), altitude: z * FT })
}

fn extraire(eid: i64, s: Sorte, r: &Record, altitude: &HashMap<i64, f64>) -> Extrait {
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
            Some(Extrait::Type(eid, TypeElem { nom, epaisseur, famille: id_of(o, "m_familyId"), largeur, hauteur }))
        })()),
        Sorte::Famille => ou_rien((|| Some(Extrait::Famille(eid, Famille { nom: o.get("m_name")?.str()?.to_string(), categorie: id_of(o, "m_categoryId"), parent: id_of(o, "m_famId") })))()),
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
        Sorte::Toit => {
            let p = params(r, o);
            Extrait::Toit(Toit { id: eid, classe: o.class_name().to_string(), type_id: id_of(o, "m_roofAttributesId"), niveau: id_of(o, "m_assocLevelId"), decalage: reel_p(&p, ROOF_LEVEL_OFFSET_PARAM) * FT, geometrie: Err(String::new()) })
        }
        Sorte::Escalier => Extrait::Escalier(escalier(eid, r, altitude)),
        Sorte::Volee => Extrait::Volee(eid, id_of(o, "m_stairsId")),
        Sorte::GardeCorps => Extrait::GardeCorps(GardeCorps { id: eid, type_id: id_of(o, "m_stairsRailingAttrId"), niveau: id_of(o, "m_assocLevelId"), geometrie: Err(String::new()) }),
        Sorte::Grille => ou_rien(grille(eid, r).map(Extrait::Grille)),
        Sorte::Instance if id_of(o, "m_ownerDBViewId") != -1 => Extrait::InstanceDeVue,
        Sorte::Instance => {
            let (i, h) = instance(eid, r);
            Extrait::Instance(i, h)
        }
        Sorte::Piece => Extrait::Piece(piece(eid, r, altitude)),
        Sorte::Forme => ou_rien(forme(eid, r).map(Extrait::Forme)),
        Sorte::Categorie => ou_rien((|| Some(Extrait::Categorie(eid, r.field_body(o, "m_pCategory")?.get("m_name")?.str()?.to_string())))()),
        Sorte::Vue => ou_rien(vues::vue(eid, r).map(Extrait::Vue)),
        Sorte::Viewer => ou_rien(vues::cadrage(r).map(|(v, o, a, b)| Extrait::Cadrage(v, (o, a, b)))),
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

    /// « Famille : type » d'une instance.
    pub fn nom_type_instance(&self, inst: &Instance) -> String {
        let t = self.types.get(&inst.type_id);
        let fam = t.and_then(|t| self.familles.get(&t.famille)).map(|f| f.nom.as_str()).unwrap_or("?");
        format!("{fam} : {}", t.map(|t| t.nom.as_str()).unwrap_or("?"))
    }
}

// ------------------------------------------------------------------ Scène

/// GlobalId synthétique de 22 caractères (les UniqueId de Revit ne sont pas décodés).
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
}

/// Tracé d'une région remplie : arrière-plan uni s'il y en a un, puis premier plan uni
/// (face colorée) ou hachures (traits de 0,13 mm sur le papier, découpés sur la face) ;
/// région sans type connu : face grise translucide.
fn region(m: &Maquette, type_id: i64, g: &Maillage, v: &Vue) -> Vec<Part> {
    let face: Vec<[DVec3; 3]> = g.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| g.sommets[i]))).collect();
    let en_m = |tris: &[[DVec3; 3]]| tris.iter().map(|t| t.map(|p| p / 1000.0)).collect::<Vec<_>>();
    let Some(t) = m.types_regions.get(&type_id) else { return vec![Part { color: [0.45, 0.45, 0.45, 0.6], tris: en_m(&face) }] };
    let rgb = |c: [u8; 3], a: f32| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, a];
    let avant = m.couleurs_categories.get(&t.categorie).copied().unwrap_or([0, 0, 0]);
    let normale = v.droite.cross(v.haut).normalize_or_zero();
    let mut parts = Vec::new();
    match m.motifs.get(&t.arriere) {
        Some(x) if x.grilles.is_empty() => parts.push(Part { color: rgb(t.couleur_arriere, 1.0), tris: en_m(&face) }),
        _ => parts.push(Part { color: [0.85, 0.85, 0.85, 0.25], tris: en_m(&face) }),
    }
    match m.motifs.get(&t.avant) {
        Some(x) if x.grilles.is_empty() => parts.push(Part { color: rgb(avant, 1.0), tris: en_m(&face) }),
        Some(x) => {
            // traits légèrement décollés de la face, vers l'observateur
            let ecart = normale * (0.05 * v.echelle);
            let mut tris = Vec::new();
            for [a, b] in vues::hachures(&face, x, v.echelle, v.droite, v.haut, 20_000) {
                vues::ruban(&[a + ecart, b + ecart], normale, 0.13 * v.echelle, &mut tris);
            }
            if !tris.is_empty() {
                parts.push(Part { color: rgb(avant, 1.0), tris });
            }
        }
        None => {}
    }
    parts
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
    let toutes = toutes.chain(m.toposolides.iter().map(|x| &x.geometrie));
    let toutes = toutes.chain(m.escaliers.iter().map(|x| &x.geometrie)).chain(m.instances.iter().map(|x| &x.geometrie)).chain(m.garde_corps.iter().map(|x| &x.geometrie));
    let toutes = toutes.chain(m.formes.iter().map(|x| &x.geometrie));
    let solides_ko: usize = toutes.filter_map(|g| g.as_ref().ok()).map(|g| g.echecs.len()).sum();
    for (nom, n) in [
        ("Murs Revit non maillés", murs_ko),
        ("Sols et plafonds Revit non maillés", ko(&mut m.dalles.iter().map(|x| &x.geometrie))),
        ("Toposolides Revit non maillés", ko(&mut m.toposolides.iter().map(|x| &x.geometrie))),
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
            // cotes du panneau (paramètres de type), pas l'encombrement de la famille
            if let Some(l) = t.largeur.filter(|&l| l > 0.0) {
                props.push(("Largeur (panneau)", mm(l)));
            }
            if let Some(h) = t.hauteur.filter(|&h| h > 0.0) {
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
    let n = Value::Null;
    let projet = b.add("IFCPROJECT", vec![gid("RVTPROJET", 0), n.clone(), Value::Str(titre.into())]);
    let mut etage: HashMap<i64, Id> = HashMap::new();
    for l in &m.niveaux {
        let args = vec![gid("RVTNIV", l.id), n.clone(), Value::Str(l.nom.clone()), n.clone(), n.clone(), n.clone(), n.clone(), n.clone(), Value::Enum("ELEMENT".into()), Value::Real(l.altitude / 1000.0)];
        etage.insert(l.id, b.add("IFCBUILDINGSTOREY", args));
    }
    let storeys: Vec<Value> = m.niveaux.iter().map(|l| Value::Ref(etage[&l.id])).collect();
    b.add("IFCRELAGGREGATES", vec![gid("RVTAGG", 0), n.clone(), n.clone(), n.clone(), Value::Ref(projet), Value::List(storeys)]);
    let shape = b.add("IFCPRODUCTDEFINITIONSHAPE", vec![n.clone(), n.clone(), Value::List(vec![])]);

    let mut types: HashMap<(i64, &str), Id> = HashMap::new();
    let mut contenus: BTreeMap<Id, Vec<Value>> = BTreeMap::new();
    let mut geoms = Vec::new();
    // traits des motifs de surface : plafond (2 sommets de 24 octets par trait)
    let (mut traits_motifs, mut motifs_omis, mut motifs_dessin) = (0usize, 0usize, 0usize);
    for p in produits(m) {
        let id = b.add(p.classe, vec![gid("RVT", p.id), n.clone(), Value::Str(p.nom.clone()), n.clone(), Value::Str(p.nom_type.clone()), n.clone(), Value::Ref(shape), Value::Str(p.id.to_string())]);
        let mut props = vec![("ElementId", Value::Str(p.id.to_string()))];
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
                    let pset = b.pset("RVTTPSET", p.type_id, "Revit", tp);
                    let t = b.add(&format!("{}TYPE", p.classe), vec![gid("RVTTYPE", p.type_id), n.clone(), Value::Str(p.nom_type.clone()), n.clone(), n.clone(), Value::List(vec![Value::Ref(pset)])]);
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
        let id = b.add("IFCGRID", vec![gid("RVT", g.id), n.clone(), Value::Str(g.nom.clone()), n.clone(), n.clone(), n.clone(), Value::Ref(shape)]);
        let pset = b.pset("RVTPSET", g.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", g.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        contenus.entry(projet).or_default().push(Value::Ref(id));
    }
    // Vues : un nœud par vue (dans le niveau d'un plan, sinon dans le projet), ses
    // éléments 2D en enfants, tracés dans le plan de la vue (masqués par défaut).
    let mut par_vue: BTreeMap<i64, Vec<&Element2D>> = BTreeMap::new();
    for e in &m.elements_2d {
        par_vue.entry(e.vue).or_default().push(e);
    }
    let nom_vue: HashMap<i64, &str> = m.vues.iter().map(|v| (v.id, v.nom.as_str())).collect();
    for v in m.vues.iter().filter(|v| !v.gabarit && v.genre != "Arborescence") {
        let mut props = vec![
            ("ElementId", Value::Str(v.id.to_string())),
            ("Classe Revit", Value::Str(v.classe.clone())),
            ("Genre", Value::Str(v.genre.into())),
            ("Échelle", Value::Str(format!("1:{}", v.echelle))),
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
        let id_vue = b.add("IFCANNOTATION", vec![gid("RVTVUE", v.id), n.clone(), Value::Str(nom), n.clone(), Value::Str(v.genre.into()), n.clone(), Value::Ref(shape)]);
        let pset = b.pset("RVTPSETV", v.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEFV", v.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id_vue)]), Value::Ref(pset)]);
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
                Contenu::FenetreDeVue { vue, echelle, decalage } => {
                    let nv = nom_vue.get(vue).map(|s| s.to_string()).unwrap_or_else(|| vue.to_string());
                    ("Fenêtre de vue", [0.5, 0.5, 0.5, 1.0], vec![("Vue placée", Value::Str(nv)), ("Échelle", Value::Str(format!("1:{}", (1.0 / echelle).round()))), ("Décalage", point_mm(*decalage))])
                }
            };
            props.splice(0..0, [("ElementId", Value::Str(e.id.to_string())), ("Classe Revit", Value::Str(e.classe.clone())), ("Vue", Value::Str(v.nom.clone()))]);
            let titre = match &e.contenu {
                Contenu::Texte { texte, .. } => format!("{genre} « {} » [{}]", texte.lines().next().unwrap_or_default(), e.id),
                _ => format!("{genre} [{}]", e.id),
            };
            let id = b.add("IFCANNOTATION", vec![gid("RVT2D", e.id), n.clone(), Value::Str(titre), n.clone(), Value::Str(genre.into()), n.clone(), Value::Ref(shape)]);
            let pset = b.pset("RVTPSET2D", e.id, "Revit", props);
            b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF2D", e.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
            let tris = vues::trace(e, normale, v.echelle);
            let mut parts = Vec::new();
            if !tris.is_empty() {
                parts.push(Part { color: couleur, tris });
            }
            if let (Contenu::Region { type_id }, Ok(g)) = (&e.contenu, &e.geometrie) {
                parts.extend(region(m, *type_id, g, v));
            }
            if !parts.is_empty() {
                geoms.push(ProductGeom { id, parts, lignes: Vec::new() });
            }
            enfants.push(Value::Ref(id));
        }
        if !enfants.is_empty() {
            b.add("IFCRELAGGREGATES", vec![gid("RVTAGGV", v.id), n.clone(), n.clone(), n.clone(), Value::Ref(id_vue), Value::List(enfants)]);
        }
    }
    for (k, (s, items)) in contenus.into_iter().enumerate() {
        b.add("IFCRELCONTAINEDINSPATIALSTRUCTURE", vec![gid("RVTCONT", k as i64), n.clone(), n.clone(), n.clone(), Value::List(items), Value::Ref(s)]);
    }
    let model = IfcModel::new(StepFile { schema: format!("REVIT {format}"), entities: b.entities });
    let mut s = scene::assemble(model, geoms, file_name);
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

/// Nombre maximal de traits de motifs de surface dans une scène.
const MAX_TRAITS_MOTIFS: usize = 1_500_000;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Fichier d'essai hors dépôt (docs/PROTOCOLE-RVT.md) ; dossier remplaçable par la
    /// variable d'environnement RVT_ESSAIS. Absent : le test est sauté.
    fn essai(dossier: &str, nom: &str) -> Option<PathBuf> {
        let base = std::env::var("RVT_ESSAIS").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(r"C:\Users\theba\Desktop\Revit"));
        let p = base.join(dossier).join(nom);
        if p.exists() {
            Some(p)
        } else {
            eprintln!("fichier d'essai absent, test sauté : {}", p.display());
            None
        }
    }

    /// Projet du corpus de l'agence (hors dépôt) ; dossier remplaçable par la variable
    /// d'environnement RVT_CORPUS. Absent : le test est sauté.
    fn corpus(chemin: &str) -> Option<PathBuf> {
        let base = std::env::var("RVT_CORPUS").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(r"C:\Users\theba\Documents\Thomas\ARCHI PERSO"));
        let p = base.join(chemin);
        if p.exists() {
            Some(p)
        } else {
            eprintln!("fichier du corpus absent, test sauté : {}", p.display());
            None
        }
    }

    fn proche(v: f64, att: f64, tol: f64, quoi: &str) {
        assert!((v - att).abs() <= tol, "{quoi} : {v} au lieu de {att}");
    }

    /// Essai E04 : mur de 12 345,6 × 200 × 2 718,2 mm, valeurs validées contre Revit 2026.
    #[test]
    fn e04_volume_et_boite() {
        let Some(p) = essai(r"Essais-format\2026", "E04_mur.rvt") else { return };
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
        let Some(p) = essai("Archives", "Projet1.rvt") else { return };
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
        let Some(p) = corpus(r"PERROS-GUIREC - rue de Kerreut\01-TRAVAIL\Kerreut 2025.rvt") else { return };
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
        let Some(p) = essai(r"Essais-format\2026", "E12_grilles.rvt") else { return };
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
        let Some(p) = essai("", "FEN_2PF.rfa") else { return };
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
        let Some(p) = essai(r"Essais-format\2026\C14", "E13_vues2d.rvt") else { return };
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
        assert_eq!(m.elements_2d.len(), 14, "{classes:?}");
        // scène : un objet tracé par élément 2D (sauf la fenêtre de vue), dans le plan de la vue
        let s = scene(&m, "E13", "E13_vues2d.rvt".into(), "2026");
        let annot: Vec<_> = s.objects.iter().filter(|o| &*o.class == "IFCANNOTATION").collect();
        assert_eq!(annot.len(), 13, "objets 2D tracés");
        assert!(annot.iter().all(|o| o.opaque.count + o.transparent.count > 0));
    }

    /// Vues et éléments 2D de `FEN_2PF.rfa` : 8 vues, 26 cotes et 4 textes comme dans
    /// Revit (ceux des familles imbriquées écartés), textes lus dans leur RTF.
    #[test]
    fn fenetre_rfa_vues() {
        let Some(p) = essai("", "FEN_2PF.rfa") else { return };
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
        let Some(p) = corpus(r"PERROS-GUIREC - rue de Kerreut\01-TRAVAIL\Kerreut 2025.rvt") else { return };
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
        let Some(p) = corpus(r"PERROS-GUIREC - rue de Kerreut\01-TRAVAIL\Kerreut 2025.rvt") else { return };
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

    /// Essai C14 F04 : la fenêtre chargée dans le projet vide et placée deux fois dans un
    /// mur ; volume de chaque instance 0,110441 m³ dans Revit (coffre : −0,35 %).
    #[test]
    fn c14_fenetres_placees() {
        let Some(p) = essai(r"Essais-format\2026\C14", "F04_deux.rvt") else { return };
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
        let Some(p) = essai("", "Saint-Malo_avenue aristide briand_46.rvt") else { return };
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
}




