//! Terrains multicouches adaptatifs (n° 14).
//!
//! La surface supérieure suit les points topographiques (et les fouilles des
//! terre-pleins). La face inférieure est un plan horizontal continu, à la profondeur
//! nominale du type sous le point le plus bas du terrain naturel. Si un terre-plein
//! creuse plus bas, la profondeur effective de l'instance augmente d'elle-même pour
//! garder au moins `GARDE_SOUS_FOUILLE` sous le fond de la fouille.
//!
//! Couches du type, de haut en bas : une couche fixe garde son épaisseur sous la surface
//! (bornée par le fond) ; la couche variable (une au plus ; à défaut, la dernière) comble
//! jusqu'au fond plat. Un type sans couches est une seule couche variable de son matériau.
use crate::elements::site::{Terrain, TypeTerrain};
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Épaisseur de terrain gardée sous le fond d'une fouille (mm).
pub const GARDE_SOUS_FOUILLE: f64 = 1000.0;

/// Couche d'un type de terrain.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CoucheTerrain {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
    /// Épaisseur (mm) ; ignorée pour la couche variable.
    pub epaisseur: f64,
    /// Couche variable : comble jusqu'au fond plat du terrain.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub variable: bool,
}

/// Couches effectives d'un type : (matériau, profondeur du haut, profondeur du bas sous
/// la surface ; None : jusqu'au fond).
pub fn couches(t: &TypeTerrain) -> Result<Vec<(Option<String>, f64, Option<f64>)>, String> {
    if t.couches.is_empty() {
        return Ok(vec![(t.materiau.clone(), 0.0, None)]);
    }
    let variables = t.couches.iter().filter(|c| c.variable).count();
    if variables > 1 {
        return Err(format!("{} : une seule couche variable par type de terrain.", t.nom));
    }
    let variable = t.couches.iter().position(|c| c.variable).unwrap_or(t.couches.len() - 1);
    let mut v = Vec::new();
    let mut d = 0.0;
    for (i, c) in t.couches.iter().enumerate() {
        if i == variable {
            v.push((c.materiau.clone(), d, None));
            // les couches fixes sous la couche variable reposent sur le fond : elles sont
            // comptées depuis le fond, au-dessus de lui
            break;
        }
        if !(c.epaisseur > 0.0 && c.epaisseur.is_finite()) {
            return Err(format!("{} : épaisseur de couche non valide.", t.nom));
        }
        v.push((c.materiau.clone(), d, Some(d + c.epaisseur)));
        d += c.epaisseur;
    }
    Ok(v)
}

/// Épaisseur des couches fixes posées sur le fond (sous la couche variable).
fn couches_du_fond(t: &TypeTerrain) -> Vec<(Option<String>, f64)> {
    let Some(i) = t.couches.iter().position(|c| c.variable).or(t.couches.len().checked_sub(1)) else { return Vec::new() };
    t.couches[i + 1..].iter().map(|c| (c.materiau.clone(), c.epaisseur)).collect()
}

/// Cote du fond plat (repère du projet) et profondeur effective, depuis la surface
/// naturelle (`naturel`, triangles) et la surface creusée (`dessus`).
pub fn fond(naturel: &[[DVec3; 3]], dessus: &[[DVec3; 3]], profondeur: f64) -> (f64, f64) {
    let zmin = |t: &[[DVec3; 3]]| t.iter().flatten().map(|p| p.z).fold(f64::INFINITY, f64::min);
    let (z_naturel, z_creuse) = (zmin(naturel), zmin(dessus));
    let mut z_fond = z_naturel - profondeur;
    if z_creuse < z_naturel - 1e-6 {
        z_fond = z_fond.min(z_creuse - GARDE_SOUS_FOUILLE);
    }
    (z_fond, z_naturel - z_fond)
}

/// Résultat : solides par couche (matériau, triangles), profondeurs nominale et effective.
pub struct CouchesTerrain {
    pub parts: Vec<(Option<String>, Vec<[DVec3; 3]>)>,
    pub nominale: f64,
    pub effective: f64,
}

impl CouchesTerrain {
    pub fn tous(&self) -> Vec<[DVec3; 3]> {
        self.parts.iter().flat_map(|(_, t)| t.iter().copied()).collect()
    }
}

/// Solides des couches d'un terrain dont la surface (creusée ou non) est `dessus`
/// (triangles orientés vers le haut, repère du projet), `parois` les marches des
/// fouilles sans talus (ajoutées à la couche de surface), `contour` son périmètre au
/// niveau de la surface.
pub fn construire(t: &TypeTerrain, naturel: &[[DVec3; 3]], dessus: &[[DVec3; 3]], parois: Vec<[DVec3; 3]>, contour: &[DVec3]) -> Result<CouchesTerrain, String> {
    let nominale = t.epaisseur;
    if !(nominale > 0.0 && nominale.is_finite()) {
        return Err("L'épaisseur du terrain doit être positive.".into());
    }
    let (z_fond, effective) = fond(naturel, dessus, nominale);
    let du_fond = couches_du_fond(t);
    let sur_fond: f64 = du_fond.iter().map(|c| c.1).sum();
    let mut parts = Vec::new();
    // surfaces décalées sous le dessus, bornées par le haut des couches du fond
    let plancher = z_fond + sur_fond;
    let a_cote = |p: DVec3, d: Option<f64>| DVec3::new(p.x, p.y, d.map_or(plancher, |d| (p.z - d).max(plancher)));
    for (k, (materiau, haut, bas)) in couches(t)?.into_iter().enumerate() {
        let mut tris = Vec::new();
        for f in dessus {
            let h = f.map(|p| a_cote(p, Some(haut)));
            let b = f.map(|p| a_cote(p, bas));
            // couche réduite à rien sous ce triangle (bornée par le plancher) : rien
            if h.iter().zip(&b).all(|(h, b)| (h.z - b.z).abs() < 1e-6) { continue; }
            tris.push(h);
            tris.push([b[0], b[2], b[1]]);
        }
        let n = contour.len();
        for i in 0..n {
            let (a, c) = (contour[i], contour[(i + 1) % n]);
            let (ah, ch, ab, cb) = (a_cote(a, Some(haut)), a_cote(c, Some(haut)), a_cote(a, bas), a_cote(c, bas));
            if (ah.z - ab.z).abs() > 1e-6 || (ch.z - cb.z).abs() > 1e-6 {
                tris.push([ah, ab, cb]);
                tris.push([ah, cb, ch]);
            }
        }
        if k == 0 { tris.extend(parois.iter().copied()); }
        parts.push((materiau, tris));
    }
    // couches posées sur le fond : dalles horizontales sous l'emprise
    let mut z = plancher;
    for (materiau, e) in du_fond {
        let (haut, bas) = (z, z - e);
        let mut tris = Vec::new();
        for f in dessus {
            tris.push(f.map(|p| DVec3::new(p.x, p.y, haut)));
            let b = f.map(|p| DVec3::new(p.x, p.y, bas));
            tris.push([b[0], b[2], b[1]]);
        }
        let n = contour.len();
        for i in 0..n {
            let (a, c) = (contour[i], contour[(i + 1) % n]);
            let a_z = |p: DVec3, z: f64| DVec3::new(p.x, p.y, z);
            let (ah, ch, ab, cb) = (a_z(a, haut), a_z(c, haut), a_z(a, bas), a_z(c, bas));
            tris.push([ah, ab, cb]);
            tris.push([ah, cb, ch]);
        }
        parts.push((materiau, tris));
        z = bas;
    }
    Ok(CouchesTerrain { parts, nominale, effective })
}

/// Couches d'un terrain sans fouille : la surface est celle des points topographiques.
pub fn couches_terrain_naturel(terrain: &Terrain, t: &TypeTerrain, ngf: f64) -> Result<CouchesTerrain, String> {
    let decalage = DVec3::new(0.0, 0.0, -ngf);
    let dessus: Vec<[DVec3; 3]> = terrain.triangles()?.into_iter().map(|f| f.map(|p| p + decalage)).collect();
    let contour: Vec<DVec3> = terrain.contour.iter().map(|p| DVec3::from_array(*p) + decalage).collect();
    construire(t, &dessus, &dessus, Vec::new(), &contour)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumes::geometrie::volume;

    fn terrain() -> Terrain {
        // 10 × 10 m, en pente de 0 à -2 m, un point haut à +1 m
        Terrain { id: "t".into(), nom: "TN".into(), contour: vec![[0., 0., 0.], [10_000., 0., -2000.], [10_000., 10_000., -2000.], [0., 10_000., 0.]], points: vec![[5000., 5000., 1000.]], type_terrain: None }
    }

    fn type_multi() -> TypeTerrain {
        TypeTerrain { id: "m".into(), nom: "Multicouche".into(), epaisseur: 3000.0, materiau: None, couches: vec![
            CoucheTerrain { materiau: Some("Terre végétale".into()), epaisseur: 300.0, variable: false },
            CoucheTerrain { materiau: Some("Remblai".into()), epaisseur: 0.0, variable: true },
            CoucheTerrain { materiau: Some("Roche".into()), epaisseur: 500.0, variable: false },
        ] }
    }

    /// Fond plat à la profondeur nominale sous le point le plus bas ; couche fixe de
    /// 30 cm sous toute la surface ; couche de roche de 50 cm sur le fond ; volume total
    /// égal au plein entre la surface et le fond.
    #[test]
    fn fond_plat_et_couches() {
        let c = couches_terrain_naturel(&terrain(), &type_multi(), 0.0).unwrap();
        assert_eq!(c.parts.len(), 3);
        assert!((c.effective - 3000.0).abs() < 1e-6 && (c.nominale - 3000.0).abs() < 1e-6);
        let tous = c.tous();
        let zmin = tous.iter().flatten().map(|p| p.z).fold(f64::INFINITY, f64::min);
        assert!((zmin + 5000.0).abs() < 1e-6, "fond à -2 m - 3 m : {zmin}");
        // face inférieure continue : tous les sommets les plus bas au même Z
        let roche = &c.parts[2].1;
        assert!(roche.iter().flatten().all(|p| (p.z + 5000.0).abs() < 1e-6 || (p.z + 4500.0).abs() < 1e-6));
        let vegetale = volume(&c.parts[0].1);
        assert!((vegetale - 1e8 * 300.0).abs() / (1e8 * 300.0) < 1e-6, "{vegetale}");
        let total: f64 = c.parts.iter().map(|(_, t)| volume(t)).sum();
        let plein = volume(&couches_terrain_naturel(&terrain(), &TypeTerrain { couches: Vec::new(), ..type_multi() }, 0.0).unwrap().tous());
        assert!((total - plein).abs() / plein < 1e-6, "{total} {plein}");
    }

    /// Fouille plus basse que le fond nominal : la profondeur effective augmente pour
    /// garder 1 m sous le fond de fouille ; la nominale ne change pas.
    #[test]
    fn fouille_profonde_approfondit_le_fond() {
        let t = terrain();
        let naturel: Vec<[DVec3; 3]> = t.triangles().unwrap();
        let mut dessus = naturel.clone();
        for f in &mut dessus { for p in f.iter_mut() { if p.z > 500.0 { p.z = -6000.0; } } }
        let (z_fond, effective) = fond(&naturel, &dessus, 3000.0);
        assert!((z_fond + 7000.0).abs() < 1e-6 && (effective - 5000.0).abs() < 1e-6);
        let (z_fond, effective) = fond(&naturel, &naturel, 3000.0);
        assert!((z_fond + 5000.0).abs() < 1e-6 && (effective - 3000.0).abs() < 1e-6);
    }

    #[test]
    fn deux_couches_variables_refusees() {
        let mut t = type_multi();
        t.couches[0].variable = true;
        assert!(couches(&t).is_err());
    }
}
