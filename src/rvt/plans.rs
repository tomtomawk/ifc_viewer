//! Plans de référence, plans de construction et apparences de rendu (LISEZMOI-RVT.md du
//! dossier Decodage RVT, § 7 « Plans et apparences »).
//!
//! Plan de référence (`RefPlane`, dérivé de `DatumPlane`) : nom `m_text`, plan
//! `m_pSurface` (`m_origin`, `m_xVec`, `m_yVec`, enveloppe `m_Envelope.m_corners` en
//! coordonnées du plan) ; direction = `m_xVec`, normale = `m_xVec` × `m_yVec` (opposée
//! si `m_orientFlag` vaut 0), extrémité
//! à bulle = origine + x·u max + y·v min, extrémité libre = origine + x·u min + y·v min
//! (comme `BubbleEnd`, `FreeEnd` de l'API ; les champs `m_bubbleEnd`, `m_freeEnd` en
//! diffèrent) ; « Est une référence » `m_refName` (code de l'API), « Définit l'origine »
//! `m_definesOrigin`.
//!
//! Plan de construction (`SketchPlane`) : repère `m_oTrf` (lignes `m_3x3`, origine
//! `m_or` ; `m_flipZ` : normale inversée), plan support `m_oPlaneRef.m_datumPlaneId`
//! (niveau, plan de référence, grille, vue) ou face d'un élément
//! (`m_oPlaneRef.m_geomRef.m_elemId` : forme, mur, sol, instance), qui lui donne son nom
//! (celui du type pour un élément), « <non associé> » sinon.
//!
//! Apparence (`AppearanceAssetElem`) : `m_pAppearanceAsset` → `AppearanceAsset`
//! (`m_Name`, `m_Asset` : schéma `m_sName`, propriétés `m_aAProperties` nommées
//! `m_sName`, valeur `m_value`, assets connectés `m_aConnected`) ; schéma générique :
//! couleur `generic_diffuse` (`APropertyDouble4`), texture `unifiedbitmap_Bitmap` de
//! l'asset connecté. Un matériau désigne la sienne par `m_appearanceAssetId`.

use super::champs::{Obj, Record, Val};
use super::geometrie::FT;
use glam::{DMat3, DVec3};

pub struct PlanReference {
    pub id: i64,
    pub nom: String,
    /// Extrémités (mm), direction et normale.
    pub bulle: DVec3,
    pub libre: DVec3,
    pub direction: DVec3,
    pub normale: DVec3,
    /// « Est une référence » (`m_refName`, code de l'API : 12 pas une référence, 14
    /// référence moins importante, 1 centre gauche/droite…).
    pub reference: i64,
    pub definit_origine: bool,
    /// Vue propriétaire (−1 : plan du modèle).
    pub vue: i64,
}

pub struct PlanConstruction {
    pub id: i64,
    /// Plan support (`m_datumPlaneId`), −1 : aucun.
    pub support: i64,
    /// Origine (mm) et axes (colonnes, comme `GetPlane` : X, Y, normale).
    pub origine: DVec3,
    pub axes: DMat3,
}

pub struct Apparence {
    pub nom: String,
    /// Couleur diffuse (rouge, vert, bleu, alpha ; 0 à 1), schéma générique.
    pub diffuse: Option<[f64; 4]>,
    /// Texture de la couleur diffuse (chemins séparés par « | »).
    pub texture: Option<String>,
    /// Vignette de rendu (`m_pThumbnail`, PNG).
    pub vignette: Option<super::medias::Raster>,
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

pub fn plan_reference(id: i64, r: &Record) -> Option<PlanReference> {
    let o = &r.root;
    let s = r.field_body(o, "m_pSurface")?;
    let (org, x, y) = (vec3(s.get("m_origin"))?, vec3(s.get("m_xVec"))?, vec3(s.get("m_yVec"))?);
    let coins = s.get("m_Envelope").and_then(|e| corps(r, e)).and_then(|e| e.get("m_corners"))?.list();
    let uv = |k: usize| coins.get(k).and_then(Val::reals).filter(|c| c.len() == 2);
    let (a, b) = (uv(0)?, uv(1)?);
    let p = |u: f64, v: f64| (org + x * u + y * v) * FT;
    Some(PlanReference {
        id,
        // sans nom : « Plan de référence », comme dans Revit
        nom: o.get("m_text").and_then(Val::str).filter(|s| !s.is_empty()).unwrap_or("Plan de référence").to_string(),
        bulle: p(b[0], a[1]),
        libre: p(a[0], a[1]),
        direction: x,
        // orientation de la surface (`m_orientFlag` 0) : normale opposée
        normale: if s.get("m_orientFlag").and_then(Val::int) == Some(1) { x.cross(y) } else { -x.cross(y) },
        reference: o.get("m_refName").and_then(Val::int).unwrap_or(-1),
        definit_origine: o.get("m_definesOrigin").and_then(Val::int) == Some(1),
        vue: o.get("m_ownerDBViewId").and_then(Val::element_id).unwrap_or(-1),
    })
}

pub fn plan_construction(id: i64, r: &Record) -> Option<PlanConstruction> {
    let o = &r.root;
    let t = o.get("m_oTrf").and_then(|v| corps(r, v))?;
    let l = t.get("m_3x3")?.list();
    let ligne = |i: usize| vec3(l.get(i));
    let mut axes = DMat3::from_cols(ligne(0)?, ligne(1)?, ligne(2)?).transpose();
    // plan retourné (`m_flipZ`) : normale inversée, comme `GetPlane`
    if o.get("m_flipZ").and_then(Val::int) == Some(1) {
        axes.z_axis = -axes.z_axis;
    }
    // plan support : plan de référence, niveau, grille ou vue (`m_datumPlaneId`), sinon
    // élément dont une face porte le plan (`m_geomRef.m_elemId`)
    let pr = o.get("m_oPlaneRef").and_then(|v| corps(r, v));
    let datum = pr.and_then(|p| p.get("m_datumPlaneId")).and_then(Val::element_id).filter(|&i| i != -1);
    let face = pr.and_then(|p| p.get("m_geomRef")).and_then(|v| corps(r, v)).and_then(|g| g.get("m_elemId")).and_then(Val::element_id).filter(|&i| i != -1);
    let support = datum.or(face).unwrap_or(-1);
    Some(PlanConstruction { id, support, origine: vec3(t.get("m_or"))? * FT, axes })
}

/// Propriété nommée d'un asset.
fn propriete<'r, 's>(r: &'r Record<'s>, asset: &'r Obj<'s>, nom: &str) -> Option<&'r Obj<'s>> {
    asset.get("m_aAProperties")?.list().iter().filter_map(|v| corps(r, v)).find(|p| p.get("m_sName").and_then(Val::str) == Some(nom))
}

pub fn apparence(r: &Record) -> Option<Apparence> {
    let a = r.field_body(&r.root, "m_pAppearanceAsset")?;
    let asset = a.get("m_Asset").and_then(|v| corps(r, v));
    let diffuse = asset.and_then(|s| propriete(r, s, "generic_diffuse"));
    let texture = diffuse
        .and_then(|d| d.get("m_aConnected")?.list().iter().find_map(|v| corps(r, v)))
        .and_then(|c| propriete(r, c, "unifiedbitmap_Bitmap"))
        .and_then(|b| b.get("m_value").and_then(Val::str))
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Some(Apparence {
        nom: a.get("m_Name").and_then(Val::str).unwrap_or_default().to_string(),
        diffuse: diffuse.and_then(|d| d.get("m_value")).and_then(Val::reals).and_then(|v| v.try_into().ok()),
        texture,
        vignette: r.field_body(&r.root, "m_pThumbnail").and_then(super::medias::raster),
    })
}
