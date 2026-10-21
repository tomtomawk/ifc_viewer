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
//! de son matériau (`GFace.m_renderStyleId` = id d'un `MaterialElem`). La géométrie
//! des familles (`GInstance`) n'est pas décodée : une instance n'a de maillage que si
//! elle porte son propre solide.

use super::champs::{self, Obj, Record, Val};
use super::geometrie::{self, FT, Maillage};
use super::objets::{elem_table, inflate, records, sections, unpad};
use super::schema::Schema;
use super::Cfb;
use crate::geom::{Part, ProductGeom, Rgba, class_color};
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
const ALL_MODEL_MARK: i64 = -1_001_203;
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
}

pub struct Famille {
    pub nom: String,
    /// Catégorie (BuiltInCategory).
    pub categorie: i64,
}

pub struct Materiau {
    pub nom: String,
    /// Couleur d'ombrage (rouge, vert, bleu).
    pub couleur: [u8; 3],
    /// 0 opaque, 1 transparent (Revit affiche ×100).
    pub transparence: f64,
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

/// Sol (`Floor`) ou plafond (`Ceiling`).
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
    pub toits: Vec<Toit>,
    pub escaliers: Vec<Escalier>,
    pub grilles: Vec<Grille>,
    pub instances: Vec<Instance>,
    pub pieces: Vec<Piece>,
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
    Toit,
    Escalier,
    /// Volée ou palier d'escalier.
    Volee,
    Grille,
    Instance,
    Piece,
}

impl Sorte {
    /// Éléments dont on lit le complément 0x67 (géométrie).
    fn a_geometrie(self) -> bool {
        matches!(self, Sorte::Mur | Sorte::Dalle | Sorte::Toit | Sorte::Volee | Sorte::Instance | Sorte::Piece)
    }

    fn de(sch: &Schema, id: u16, name: &str) -> Option<Sorte> {
        let h = |base| sch.inherits(id, base);
        Some(match name {
            "SWall" | "ArcWall" => Sorte::Mur,
            "MaterialElem" => Sorte::Materiau,
            "StairsElement" => Sorte::Escalier,
            "Grid" => Sorte::Grille,
            "RoomElem" => Sorte::Piece,
            _ if h("Level") => Sorte::Niveau,
            _ if h("Symbol") => Sorte::Type,
            _ if h("Family") => Sorte::Famille,
            _ if h("Floor") || h("Ceiling") => Sorte::Dalle,
            _ if h("RoofBase") => Sorte::Toit,
            _ if h("StairsRunAndLanding") => Sorte::Volee,
            _ if h("FamilyInstance") => Sorte::Instance,
            _ => return None,
        })
    }
}

/// Lit le schéma, la table des éléments et les partitions, puis les éléments et leur
/// géométrie.
pub fn lire(path: &Path, progress: &dyn Fn(String)) -> Result<Maquette, String> {
    let mut cfb = Cfb::open(path)?;
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

    let mut objets: HashMap<i64, (Sorte, Vec<u8>)> = HashMap::new();
    let mut complements: HashMap<i64, Vec<u8>> = HashMap::new();
    for (k, (_, p)) in parts.iter().enumerate() {
        progress(format!("Partition {} / {} ({p})…", k + 1, parts.len()));
        let raw = cfb.read(p)?.ok_or_else(|| format!("flux {p} illisible"))?;
        let sec = sections(&raw).map_err(|e| format!("{p} : {e}"))?;
        let vide = Vec::new();
        for (eid, body) in records(sec.get(&0x66).unwrap_or(&vide), true, ids8).map_err(|e| format!("{p}, section 0x66 : {e}"))? {
            if let Some(&s) = champs::class_id(body).and_then(|c| sortes.get(&c)).filter(|_| table.contains(&eid)) {
                objets.insert(eid, (s, body.to_vec()));
            }
        }
        for (eid, body) in records(sec.get(&0x67).unwrap_or(&vide), true, ids8).map_err(|e| format!("{p}, section 0x67 : {e}"))? {
            if objets.get(&eid).is_some_and(|o| o.0.a_geometrie()) {
                complements.insert(eid, body.to_vec());
            }
        }
    }

    progress(format!("Lecture de {} objets…", objets.len()));
    let mut ids: Vec<i64> = objets.keys().copied().collect();
    ids.sort_unstable();
    let lectures: Vec<(i64, Sorte, Result<Record, String>)> = ids
        .par_iter()
        .map(|eid| {
            let (s, body) = &objets[eid];
            (*eid, *s, champs::read(&sch, body))
        })
        .collect();
    let mut illisibles = Vec::new();
    let mut lus: Vec<(i64, Sorte, Record)> = Vec::new();
    for (eid, s, r) in lectures {
        match r {
            Ok(r) if place(&r.root) => lus.push((eid, s, r)),
            Ok(_) => {}
            Err(e) => illisibles.push((eid, e)),
        }
    }
    let de = |s: Sorte| lus.iter().filter(move |x| x.1 == s).map(|(eid, _, r)| (*eid, r));

    let mut niveaux: Vec<Niveau> = de(Sorte::Niveau)
        .filter_map(|(eid, r)| {
            let z = r.field_body(&r.root, "m_pSurface")?.get("m_origin")?.reals()?.get(2).copied()?;
            Some(Niveau { id: eid, nom: r.root.get("m_text")?.str()?.to_string(), altitude: z * FT })
        })
        .collect();
    niveaux.sort_by(|a, b| a.altitude.total_cmp(&b.altitude).then(a.id.cmp(&b.id)));
    let altitude: HashMap<i64, f64> = niveaux.iter().map(|n| (n.id, n.altitude)).collect();

    let types = de(Sorte::Type)
        .filter_map(|(eid, r)| {
            let nom = r.field_body(&r.root, "m_symbolInfo")?.get("m_name")?.str()?.to_string();
            let epaisseur = r.field_body(&r.root, "m_pCompoundStructure").map(|cs| {
                let couches = cs.get("m_layers").map(Val::list).unwrap_or_default();
                couches.iter().filter_map(|c| c.obj()?.get("m_layerWidth")?.real()).sum::<f64>() * FT
            });
            Some((eid, TypeElem { nom, epaisseur, famille: id_of(&r.root, "m_familyId") }))
        })
        .collect();

    let familles = de(Sorte::Famille)
        .filter_map(|(eid, r)| Some((eid, Famille { nom: r.root.get("m_name")?.str()?.to_string(), categorie: id_of(&r.root, "m_categoryId") })))
        .collect();

    let materiaux = de(Sorte::Materiau)
        .filter_map(|(eid, r)| {
            let m = r.field_body(&r.root, "m_pMaterial")?;
            // COLORREF : rouge dans l'octet de poids faible
            let c = m.get("m_color").and_then(Val::int).unwrap_or(0x7F7F7F);
            let couleur = [c as u8, (c >> 8) as u8, (c >> 16) as u8];
            Some((eid, Materiau { nom: m.get("m_name")?.str()?.to_string(), couleur, transparence: m.get("m_transparency").and_then(Val::real).unwrap_or(0.0) }))
        })
        .collect();

    let murs: Vec<Mur> = de(Sorte::Mur).map(|(eid, r)| mur(eid, r, &altitude)).collect();
    let traces: HashMap<i64, Trace> = murs.iter().filter_map(|m| Some((m.id, m.trace?))).collect();

    let dalles: Vec<Dalle> = de(Sorte::Dalle)
        .map(|(eid, r)| {
            let o = &r.root;
            let (champ_type, k) = if o.class_name() == "Ceiling" { ("m_ceilingAttributesId", CEILING_HEIGHTABOVELEVEL_PARAM) } else { ("m_floorAttributesId", FLOOR_HEIGHTABOVELEVEL_PARAM) };
            let p = params(r, o);
            Dalle { id: eid, classe: o.class_name().to_string(), type_id: id_of(o, champ_type), niveau: id_of(o, "m_assocLevelId"), decalage: reel_p(&p, k) * FT, geometrie: Err(String::new()) }
        })
        .collect();

    let toits: Vec<Toit> = de(Sorte::Toit)
        .map(|(eid, r)| {
            let o = &r.root;
            let p = params(r, o);
            Toit { id: eid, classe: o.class_name().to_string(), type_id: id_of(o, "m_roofAttributesId"), niveau: id_of(o, "m_assocLevelId"), decalage: reel_p(&p, ROOF_LEVEL_OFFSET_PARAM) * FT, geometrie: Err(String::new()) }
        })
        .collect();

    let volees: HashMap<i64, i64> = de(Sorte::Volee).map(|(eid, r)| (eid, id_of(&r.root, "m_stairsId"))).collect();
    let escaliers: Vec<Escalier> = de(Sorte::Escalier).map(|(eid, r)| escalier(eid, r, &altitude, &volees)).collect();

    let grilles: Vec<Grille> = de(Sorte::Grille).filter_map(|(eid, r)| grille(eid, r)).collect();

    let mut instances_de_vue = 0;
    let mut instances = Vec::new();
    for (eid, r) in de(Sorte::Instance) {
        if id_of(&r.root, "m_ownerDBViewId") != -1 {
            instances_de_vue += 1;
        } else {
            instances.push(instance(eid, r, &altitude, &traces));
        }
    }

    let pieces: Vec<Piece> = de(Sorte::Piece).map(|(eid, r)| piece(eid, r, &altitude)).collect();

    // Géométrie, en parallèle.
    let mut a_mailler: Vec<i64> = murs.iter().map(|m| m.id).chain(dalles.iter().map(|d| d.id)).chain(toits.iter().map(|t| t.id)).collect();
    a_mailler.extend(escaliers.iter().flat_map(|e| e.parties.iter().copied()));
    a_mailler.extend(instances.iter().map(|i| i.id).chain(pieces.iter().map(|p| p.id)));
    progress(format!("Géométrie de {} éléments…", a_mailler.len()));
    let mut geoms: HashMap<i64, Geometrie> = a_mailler.par_iter().map(|&eid| (eid, maillage(&sch, &complements, eid))).collect();
    let mut prendre = |eid: i64| geoms.remove(&eid).unwrap_or_else(|| Err("non maillé".into()));

    let mut m = Maquette {
        niveaux,
        types,
        familles,
        materiaux,
        murs,
        dalles,
        toits,
        escaliers,
        grilles,
        instances,
        pieces,
        instances_de_vue,
        illisibles,
    };
    for x in &mut m.murs {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.dalles {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.toits {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.instances {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.pieces {
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

/// Maillage du complément 0x67 d'un élément.
fn maillage(sch: &Schema, complements: &HashMap<i64, Vec<u8>>, eid: i64) -> Geometrie {
    let b = complements.get(&eid).ok_or_else(|| format!("élément {eid} sans enregistrement 0x67"))?;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| champs::read(sch, b).and_then(|rec| geometrie::maillage(&rec, false))))
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
            if let Some(x) = x.obj() {
                if let (Some(id), Some(v)) = (x.get("m_paramId").and_then(Val::element_id), x.get("m_value")) {
                    out.insert(id, v);
                }
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

/// Volées et paliers : listes `m_runAndLandings` des cellules de l'escalier et
/// `m_runsAndLandings`, plus les parties dont `m_stairsId` désigne l'escalier.
fn escalier(id: i64, r: &Record, altitude: &HashMap<i64, f64>, volees: &HashMap<i64, i64>) -> Escalier {
    let o = &r.root;
    let mut parties = BTreeSet::new();
    for c in cellules(r, o).into_iter().chain([o]) {
        for k in ["m_runAndLandings", "m_runsAndLandings"] {
            parties.extend(c.get(k).map(Val::list).unwrap_or_default().iter().filter_map(Val::element_id));
        }
    }
    parties.extend(volees.iter().filter(|&(_, &s)| s == id).map(|(&v, _)| v));
    parties.retain(|p| volees.contains_key(p));
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

fn instance(id: i64, r: &Record, altitude: &HashMap<i64, f64>, traces: &HashMap<i64, Trace>) -> Instance {
    let o = &r.root;
    let p = params(r, o);
    let (hote, niveau) = (id_of(o, "m_hostId"), id_of(o, "m_assocLevelId"));
    let point = match (traces.get(&hote), reel(o, "m_hostParam")) {
        (Some(t), Some(h)) => {
            let q = t.point(h);
            DVec3::new(q.x, q.y, altitude.get(&niveau).copied().unwrap_or(0.0) + reel(o, "m_elevation").unwrap_or(0.0) * FT)
        }
        _ => o.get("m_instOrigin").and_then(vec3).unwrap_or_default() * FT,
    };
    let mut repere = texte_p(&p, ALL_MODEL_MARK);
    if repere.is_empty() {
        // les portes rangent leur repère dans FamInstSpec.m_doorNumber
        repere = r.field_body(o, "m_oFamInstSpec").and_then(|s| s.get("m_doorNumber")?.str()).unwrap_or_default().to_string();
    }
    Instance { id, type_id: id_of(o, "m_masterSymbolId"), hote, niveau, point, repere, geometrie: Err(String::new()) }
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
        let cat = self.types.get(&inst.type_id).and_then(|t| self.familles.get(&t.famille)).map(|f| f.categorie).unwrap_or(-1);
        match CATEGORIES.iter().find(|c| c.0 == cat) {
            Some(&(_, nom, classe)) => (nom.to_string(), classe),
            None => (format!("Catégorie {cat}"), "IFCBUILDINGELEMENTPROXY"),
        }
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

/// Lit le fichier et construit la scène du viewer. Erreur si aucun élément n'a pu être
/// maillé.
pub fn load_scene(path: &Path, titre: &str, format: &str, progress: &dyn Fn(String)) -> Result<Scene, String> {
    let t0 = std::time::Instant::now();
    let m = lire(path, progress)?;
    let ko = |v: &mut dyn Iterator<Item = &Geometrie>| v.filter(|g| g.is_err()).count();
    let murs_ko = ko(&mut m.murs.iter().map(|x| &x.geometrie));
    let mailles = m.murs.len() - murs_ko
        + m.dalles.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.toits.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.escaliers.iter().filter(|x| x.geometrie.is_ok()).count();
    if mailles == 0 {
        return Err(match m.murs.len() {
            0 => "aucun mur, sol, toit ou escalier dans le fichier".into(),
            n => format!("aucun des {n} murs n'a pu être maillé"),
        });
    }
    progress("Assemblage des tampons…".into());
    let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut s = scene(&m, titre, file_name, format);
    let pieces: Vec<&Piece> = m.pieces.iter().filter(|p| p.genre == "piece" && p.point.is_some()).collect();
    for (nom, n) in [
        ("Murs Revit non maillés", murs_ko),
        ("Sols et plafonds Revit non maillés", ko(&mut m.dalles.iter().map(|x| &x.geometrie))),
        ("Toits Revit non maillés", ko(&mut m.toits.iter().map(|x| &x.geometrie))),
        ("Escaliers Revit non maillés", ko(&mut m.escaliers.iter().map(|x| &x.geometrie))),
        ("Pièces Revit sans volume", ko(&mut pieces.iter().map(|x| &x.geometrie))),
        ("Familles Revit sans géométrie décodée (GInstance)", ko(&mut m.instances.iter().map(|x| &x.geometrie))),
        ("Objets Revit illisibles", m.illisibles.len()),
    ] {
        if n > 0 {
            s.unsupported.push((nom.into(), n));
        }
    }
    s.load_ms = t0.elapsed().as_millis();
    Ok(s)
}

/// Produits de la scène : murs, sols, plafonds, toits, escaliers, instances de
/// familles, pièces placées et grilles.
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
    for x in &m.instances {
        let (cat, classe) = m.categorie(x);
        let nt = m.nom_type_instance(x);
        let mut props = vec![
            ("Catégorie", Value::Str(cat)),
            ("Famille et type", Value::Str(format!("{nt} [{}]", x.type_id))),
            ("Niveau", Value::Str(niveau(x.niveau))),
            ("Point d'insertion", point_mm(x.point)),
        ];
        if x.hote != -1 {
            props.push(("Hôte (ElementId)", Value::Str(x.hote.to_string())));
        }
        if !x.repere.is_empty() {
            props.push(("Repère", Value::Str(x.repere.clone())));
        }
        out.push(Produit { id: x.id, classe, nom: format!("{nt} [{}]", x.id), type_id: x.type_id, nom_type: nt, niveau: x.niveau, props, geometrie: &x.geometrie });
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
    for p in produits(m) {
        let id = b.add(p.classe, vec![gid("RVT", p.id), n.clone(), Value::Str(p.nom.clone()), n.clone(), Value::Str(p.nom_type.clone()), n.clone(), Value::Ref(shape), Value::Str(p.id.to_string())]);
        let mut props = vec![("ElementId", Value::Str(p.id.to_string()))];
        props.extend(p.props);
        match p.geometrie {
            Ok(g) => {
                let mut mats: Vec<&str> = g.faces.iter().filter_map(|f| m.materiaux.get(&f.materiau)).map(|x| x.nom.as_str()).collect();
                mats.sort_unstable();
                mats.dedup();
                props.push(("Matériaux des faces", Value::Str(mats.join(", "))));
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
            geoms.push(ProductGeom { id, parts });
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
    for (k, (s, items)) in contenus.into_iter().enumerate() {
        b.add("IFCRELCONTAINEDINSPATIALSTRUCTURE", vec![gid("RVTCONT", k as i64), n.clone(), n.clone(), n.clone(), Value::List(items), Value::Ref(s)]);
    }
    let model = IfcModel::new(StepFile { schema: format!("REVIT {format}"), entities: b.entities });
    scene::assemble(model, geoms, file_name)
}

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
        assert_eq!((porte.hote, porte.niveau, porte.repere.as_str()), (257007, 608, "P4"));
        assert!(porte.point.distance(DVec3::new(1500.0, 0.0, 0.0)) < 1e-3, "point {:?}", porte.point);
        let fenetre = m.instances.iter().find(|i| i.id == 257071).unwrap();
        assert_eq!(fenetre.repere, "F1");
        assert!(fenetre.point.distance(DVec3::new(2200.0, 0.0, 1500.0)) < 1e-3, "point {:?}", fenetre.point);

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
        let e = m.escaliers.iter().find(|e| e.id == 9911889).unwrap();
        assert_eq!((e.niveau_bas, e.niveau_haut, e.contremarches, e.parties.as_slice()), (512913, 512914, 15, &[9911890][..]));
        proche(e.hauteur_contremarche, 180.0, 1e-6, "contremarche");
        proche(e.giron, 240.0, 1e-6, "giron");
        proche(e.hauteur, 2700.0, 1e-6, "hauteur");
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
}
