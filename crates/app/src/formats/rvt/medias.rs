//! Images, imports et liens (LISEZMOI-RVT.md du dossier Decodage RVT, § 7 « Images
//! et liens »).
//!
//! Image : données `ImageHolder.m_pRasterImage` → `ARasterImage` (`m_compressedImage` :
//! le fichier image complet, JPEG ou PNG, en octets signés ; `m_width`, `m_height` en
//! pixels, `m_dpiX`), type `ImageSymbol` (`m_idImageHolder`, `m_widthInFeet`,
//! `m_heightInFeet`, `m_widthInPixels`, `m_heightInPixels`, `m_dpi`), occurrence
//! `ImageInstance` (vue `m_ownerDBViewId`, `m_pImageInfo` : type `m_symbolId`, centre
//! `m_Trf.m_or`, échelles `m_dWidthScale`, `m_dHeightScale` de la taille du type).
//! Vignette d'une apparence de rendu : `AppearanceAssetElem.m_pThumbnail`, même
//! `ARasterImage` (PNG).
//!
//! Import CAO : type `MasterImportSymbol` (nom du fichier `m_symbolInfo.m_name`, taille
//! `m_size` en octets, unité `m_unitTypeId`, catégorie `m_categoryId`), occurrence
//! `ImportInstance` (type `m_pInstanceInfo.m_symbolId`, repère `m_Trf`, vue
//! `m_ownerDBViewId`, −1 : dans le modèle).
//!
//! Lien Revit : type `RvtLinkSymbol` (nom, chemin
//! `m_otherDocModelLocation.m_lastKnownModelPath.m_path`, `m_relative`), occurrence
//! `RvtLinkInstance` (`m_name`, type et repère `m_pInstanceInfo`).

use super::champs::{Obj, Record, Val};
use super::geometrie::FT;
use glam::{DMat3, DVec3};

/// Fichier image enregistré (`ARasterImage`).
pub struct Raster {
    /// « jpg », « png », « bmp » ou « ? ».
    pub format: &'static str,
    pub donnees: Vec<u8>,
    pub largeur_px: i64,
    pub hauteur_px: i64,
}

pub struct TypeImage {
    pub id: i64,
    pub nom: String,
    /// Famille propriétaire (`m_famId`), −1 : image du projet.
    pub famille: i64,
    /// Fichier d'origine (référence externe, `ImageType.Path`).
    pub chemin: Option<String>,
    pub donnees: i64,
    /// Taille (mm), pixels, résolution.
    pub largeur: f64,
    pub hauteur: f64,
    pub largeur_px: i64,
    pub hauteur_px: i64,
    pub dpi: f64,
}

pub struct InstanceImage {
    pub id: i64,
    pub vue: i64,
    pub type_id: i64,
    /// Centre (mm) et échelles de la taille du type.
    pub centre: DVec3,
    pub echelle: (f64, f64),
}

pub struct TypeImport {
    pub id: i64,
    pub nom: String,
    /// Chemin du fichier (`m_pImportData.m_fullName`, relatif s'il l'est dans Revit).
    pub chemin: String,
    /// Lié (référence externe au fichier) ou importé.
    pub lie: bool,
    pub taille: i64,
    pub unite: String,
    pub categorie: i64,
}

pub struct TypeLien {
    pub id: i64,
    pub nom: String,
    pub chemin: String,
    pub relatif: bool,
}

/// Occurrence d'un import CAO ou d'un lien Revit.
pub struct InstanceLien {
    pub id: i64,
    pub nom: String,
    pub type_id: i64,
    pub vue: i64,
    pub origine: DVec3,
    pub axes: DMat3,
}

pub enum Media {
    Raster(i64, Raster),
    TypeImage(TypeImage),
    InstanceImage(InstanceImage),
    TypeImport(TypeImport),
    InstanceImport(InstanceLien),
    TypeLien(TypeLien),
    InstanceLien(InstanceLien),
}

fn corps<'r, 's>(r: &'r Record<'s>, v: &'r Val<'s>) -> Option<&'r Obj<'s>> {
    r.deref(v).map(|b| &b.obj).or(v.obj())
}

fn vec3(v: Option<&Val>) -> Option<DVec3> {
    match v?.reals().as_deref() {
        Some(&[x, y, z]) => Some(DVec3::new(x, y, z)),
        _ => None,
    }
}

fn id(o: &Obj, k: &str) -> i64 {
    o.get(k).and_then(Val::element_id).unwrap_or(-1)
}

fn reel(o: &Obj, k: &str) -> f64 {
    o.get(k).and_then(Val::real).unwrap_or(0.0)
}

fn entier(o: &Obj, k: &str) -> i64 {
    o.get(k).and_then(Val::int).unwrap_or(0)
}

/// Références à des fichiers externes d'un élément (`Element.m_cellList` →
/// `ExternalResourceReferenceCell.m_externalResourceReferences` → `ExternalResourceRef` :
/// `m_referenceInformation` « Path », sinon `m_displayPath`).
pub fn references_externes(r: &Record, o: &Obj) -> Vec<String> {
    let Some(cells) = o.get("m_cellList").and_then(|v| corps(r, v)).and_then(|c| c.get("m_cells")) else { return Vec::new() };
    let mut out = Vec::new();
    for c in cells.list().iter().filter_map(|v| corps(r, v)).filter(|c| c.class_name() == "ExternalResourceReferenceCell") {
        // références directes, puis développées (`m_externalResourceReferencesExpanded` :
        // paires type → `ExternalRefIdMap.m_refs` → paires clé → `ExternalResourceRef`)
        let mut refs: Vec<&Obj> = c.get("m_externalResourceReferences").map(Val::list).unwrap_or_default().iter().filter_map(|v| corps(r, v)).filter_map(|p| p.get("second").and_then(|v| corps(r, v))).collect();
        for p in c.get("m_externalResourceReferencesExpanded").map(Val::list).unwrap_or_default().iter().filter_map(|v| corps(r, v)) {
            let m = p.get("second").and_then(|v| corps(r, v)).and_then(|m| m.get("m_refs")).map(Val::list).unwrap_or_default();
            refs.extend(m.iter().filter_map(|v| corps(r, v)).filter_map(|q| q.get("second").and_then(|v| corps(r, v))));
        }
        for rf in refs {
            let info = rf.get("m_referenceInformation").map(Val::list).unwrap_or_default();
            let chemin = info
                .iter()
                .filter_map(|v| corps(r, v))
                .find(|i| i.get("first").and_then(Val::str) == Some("Path"))
                .and_then(|i| i.get("second").and_then(Val::str))
                .or_else(|| rf.get("m_displayPath").and_then(Val::str));
            if let Some(ch) = chemin.filter(|s| !s.is_empty()) {
                out.push(ch.to_string());
            }
        }
    }
    out
}

/// Nom d'un type (`Symbol.m_symbolInfo.m_name`).
fn nom_symbole(r: &Record, o: &Obj) -> String {
    o.get("m_symbolInfo").and_then(|v| corps(r, v)).and_then(|s| s.get("m_name")).and_then(Val::str).unwrap_or_default().to_string()
}

/// Lit un `ARasterImage`.
pub fn raster(o: &Obj) -> Option<Raster> {
    let donnees: Vec<u8> = o.get("m_compressedImage")?.list().iter().map(|v| v.int().unwrap_or(0) as u8).collect();
    if donnees.is_empty() {
        return None;
    }
    let format = match donnees.as_slice() {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, 0xFF, ..] => "jpg",
        [b'B', b'M', ..] => "bmp",
        _ => "?",
    };
    Some(Raster { format, donnees, largeur_px: entier(o, "m_width"), hauteur_px: entier(o, "m_height") })
}

/// Repère `m_pInstanceInfo.m_Trf` (lignes `m_3x3`, origine `m_or`) et type `m_symbolId`.
fn repere(r: &Record, o: &Obj) -> Option<(DVec3, DMat3, i64)> {
    let info = o.get("m_pInstanceInfo").and_then(|v| corps(r, v))?;
    let t = info.get("m_Trf").and_then(|v| corps(r, v))?;
    let l = t.get("m_3x3")?.list();
    let ligne = |i: usize| vec3(l.get(i));
    Some((vec3(t.get("m_or"))? * FT, DMat3::from_cols(ligne(0)?, ligne(1)?, ligne(2)?).transpose(), id(info, "m_symbolId")))
}

pub fn lire(eid: i64, r: &Record) -> Option<Media> {
    let o = &r.root;
    Some(match o.class_name() {
        "ImageHolder" => Media::Raster(eid, raster(r.field_body(o, "m_pRasterImage")?)?),
        "ImageSymbol" => Media::TypeImage(TypeImage {
            id: eid,
            nom: nom_symbole(r, o),
            famille: id(o, "m_famId"),
            chemin: references_externes(r, o).into_iter().next(),
            donnees: id(o, "m_idImageHolder"),
            largeur: reel(o, "m_widthInFeet") * FT,
            hauteur: reel(o, "m_heightInFeet") * FT,
            largeur_px: entier(o, "m_widthInPixels"),
            hauteur_px: entier(o, "m_heightInPixels"),
            dpi: reel(o, "m_dpi"),
        }),
        "ImageInstance" => {
            let info = r.field_body(o, "m_pImageInfo")?;
            let t = info.get("m_Trf").and_then(|v| corps(r, v))?;
            Media::InstanceImage(InstanceImage {
                id: eid,
                vue: id(o, "m_ownerDBViewId"),
                type_id: id(info, "m_symbolId"),
                centre: vec3(t.get("m_or"))? * FT,
                echelle: (reel(info, "m_dWidthScale"), reel(info, "m_dHeightScale")),
            })
        }
        "MasterImportSymbol" => Media::TypeImport(TypeImport {
            id: eid,
            nom: nom_symbole(r, o),
            chemin: r.field_body(o, "m_pImportData").and_then(|d| d.get("m_fullName")).and_then(Val::str).unwrap_or_default().to_string(),
            lie: !references_externes(r, o).is_empty(),
            taille: entier(o, "m_size"),
            unite: o.get("m_unitTypeId").and_then(Val::obj).and_then(|u| u.get("m_typeId")).and_then(Val::str).unwrap_or_default().to_string(),
            categorie: id(o, "m_categoryId"),
        }),
        "ImportInstance" => {
            let (origine, axes, type_id) = repere(r, o)?;
            Media::InstanceImport(InstanceLien { id: eid, nom: String::new(), type_id, vue: id(o, "m_ownerDBViewId"), origine, axes })
        }
        "RvtLinkSymbol" => {
            let lieu = o.get("m_otherDocModelLocation").and_then(|v| corps(r, v));
            let chemin = lieu.and_then(|l| l.get("m_lastKnownModelPath")).and_then(|v| corps(r, v)).and_then(|p| p.get("m_path")).and_then(Val::str).unwrap_or_default().to_string();
            Media::TypeLien(TypeLien { id: eid, nom: nom_symbole(r, o), chemin, relatif: entier(o, "m_relative") == 1 })
        }
        "RvtLinkInstance" => {
            let (origine, axes, type_id) = repere(r, o)?;
            Media::InstanceLien(InstanceLien { id: eid, nom: o.get("m_name").and_then(Val::str).unwrap_or_default().to_string(), type_id, vue: id(o, "m_ownerDBViewId"), origine, axes })
        }
        _ => return None,
    })
}
