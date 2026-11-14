//! Géométrie des terrains et des limites de propriété (coordonnées en millimètres).
//! Les altitudes d'un terrain sont des cotes NGF : le terrain suit l'altitude NGF de
//! l'origine du projet (`Projet::coordonnees_origine`), qui remonte ou descend le
//! projet par rapport à lui.
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Terrain {
    pub id: String,
    pub nom: String,
    /// Sommets du périmètre, dans l'ordre, avec leur cote NGF.
    pub contour: Vec<[f64; 3]>,
    /// Points cotés (cote NGF) strictement à l'intérieur du périmètre.
    #[serde(default)]
    pub points: Vec<[f64; 3]>,
    /// Type (épaisseur, matériau) ; None : le premier type du projet, sinon le type par
    /// défaut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_terrain: Option<String>,
}

/// Type de terrain : épaisseur sous la surface (mm) et matériau.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TypeTerrain {
    pub id: String,
    pub nom: String,
    pub epaisseur: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
}

impl TypeTerrain {
    /// Type des terrains d'un projet sans type de terrain.
    pub fn defaut() -> TypeTerrain {
        TypeTerrain { id: "terrain-naturel".into(), nom: "Terrain naturel".into(), epaisseur: 1000.0, materiau: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LimitePropriete {
    pub id: String,
    pub nom: String,
    pub altitude: f64,
    pub contour: Vec<[f64; 2]>,
}

impl LimitePropriete {
    pub fn verifier(&self) -> Result<(), String> {
        if !self.altitude.is_finite() { return Err("Altitude de limite non valide.".into()); }
        crate::modele::creation::contour_valide(&self.contour)?;
        Ok(())
    }
}

impl Terrain {
    /// Triangule le contour puis insère chaque point dans la face qui le contient.
    pub fn triangles(&self) -> Result<Vec<[DVec3; 3]>, String> {
        if self.contour.iter().chain(&self.points).flatten().any(|v| !v.is_finite()) {
            return Err("Les coordonnées du terrain doivent être finies.".into());
        }
        let contour: Vec<[f64; 2]> = self.contour.iter().map(|p| [p[0], p[1]]).collect();
        let valide = crate::modele::creation::contour_valide(&contour)?;
        if valide.len() != contour.len() { return Err("Le contour du terrain comporte des points répétés.".into()); }
        let coords: Vec<f64> = contour.iter().flatten().copied().collect();
        let indices = earcutr::earcut(&coords, &[], 2).map_err(|_| "Triangulation du terrain impossible.")?;
        if indices.is_empty() || indices.len() % 3 != 0 { return Err("Triangulation du terrain impossible.".into()); }
        let mut sommets: Vec<DVec3> = self.contour.iter().copied().map(DVec3::from_array).collect();
        let mut faces: Vec<[usize; 3]> = indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
        for &point in &self.points {
            let p = DVec2::new(point[0], point[1]);
            if sommets.iter().any(|v| v.truncate().distance(p) < 1e-6) { return Err("Deux points du terrain ont la même position.".into()); }
            let mut face = None;
            let mut arete: Option<(usize, usize)> = None;
            for (i, t) in faces.iter().enumerate() {
                let [a, b, c] = t.map(|k| sommets[k].truncate());
                let s = [(b-a).perp_dot(p-a), (c-b).perp_dot(p-b), (a-c).perp_dot(p-c)];
                let eps = 1e-8 * (b-a).length().max((c-b).length()).max((a-c).length()).max(1.0);
                if s.iter().all(|x| *x > eps) || s.iter().all(|x| *x < -eps) { face = Some(i); break; }
                if s.iter().all(|x| *x >= -eps) || s.iter().all(|x| *x <= eps) {
                    if let Some(k) = s.iter().position(|x| x.abs() <= eps) { arete = Some((t[k], t[(k + 1) % 3])); }
                }
            }
            let n = sommets.len();
            sommets.push(DVec3::from_array(point));
            if let Some(i) = face {
                let [a, b, c] = faces.swap_remove(i);
                faces.extend([[a,b,n], [b,c,n], [c,a,n]]);
            } else if let Some((a, b)) = arete {
                let voisines: Vec<_> = faces.iter().enumerate().filter(|(_, t)| t.contains(&a) && t.contains(&b)).map(|(i, _)| i).collect();
                if voisines.len() != 2 { return Err("Le point coté doit être strictement à l'intérieur du terrain.".into()); }
                for i in voisines.into_iter().rev() {
                    let t = faces.swap_remove(i);
                    let k = (0..3).find(|&k| (t[k] == a && t[(k+1)%3] == b) || (t[k] == b && t[(k+1)%3] == a)).unwrap();
                    let (x, y, z) = (t[k], t[(k+1)%3], t[(k+2)%3]);
                    faces.extend([[x,n,z], [n,y,z]]);
                }
            } else { return Err("Le point coté doit être strictement à l'intérieur du terrain.".into()); }
        }
        let plan: Vec<DVec2> = sommets.iter().map(|v| v.truncate()).collect();
        delaunay(&plan, &mut faces);
        Ok(faces.into_iter().map(|t| t.map(|i| sommets[i])).collect())
    }

    /// Solide du terrain dans le repère du projet : surface (cotes NGF moins `ngf`,
    /// cote NGF de l'origine), même surface `epaisseur` plus bas, côtés le long du
    /// contour.
    pub fn solide(&self, ngf: f64, epaisseur: f64) -> Result<Vec<[DVec3; 3]>, String> {
        if !(epaisseur > 0.0 && epaisseur.is_finite()) { return Err("L'épaisseur du terrain doit être positive.".into()); }
        let decalage = DVec3::new(0.0, 0.0, -ngf);
        let bas = DVec3::new(0.0, 0.0, -epaisseur);
        let dessus: Vec<[DVec3; 3]> = self.triangles()?.into_iter().map(|t| t.map(|p| p + decalage)).collect();
        let mut tris = dessus.clone();
        tris.extend(dessus.iter().map(|t| [t[0] + bas, t[2] + bas, t[1] + bas]));
        let n = self.contour.len();
        for i in 0..n {
            let (a, b) = (DVec3::from_array(self.contour[i]) + decalage, DVec3::from_array(self.contour[(i + 1) % n]) + decalage);
            tris.push([a, a + bas, b + bas]);
            tris.push([a, b + bas, b]);
        }
        Ok(tris)
    }
}

/// Triangulation de Delaunay contrainte par le contour : une arête intérieure est
/// retournée tant que le sommet opposé d'un triangle voisin tombe dans le cercle
/// circonscrit de l'autre (quadrilatère convexe seulement). Plus de triangles
/// effilés : la surface ne se plisse plus entre deux points proches en plan.
fn delaunay(s: &[DVec2], faces: &mut [[usize; 3]]) {
    let orient = |a: DVec2, b: DVec2, c: DVec2| (b - a).perp_dot(c - a);
    for f in faces.iter_mut() {
        if orient(s[f[0]], s[f[1]], s[f[2]]) < 0.0 { f.swap(1, 2); }
    }
    // cercle circonscrit de (a, b, c), sens direct : d dedans si le déterminant est positif
    let dans_cercle = |a: DVec2, b: DVec2, c: DVec2, d: DVec2| {
        let (a, b, c) = (a - d, b - d, c - d);
        let det = a.length_squared() * b.perp_dot(c) - b.length_squared() * a.perp_dot(c) + c.length_squared() * a.perp_dot(b);
        let echelle = a.length_squared().max(b.length_squared()).max(c.length_squared());
        det > 1e-9 * echelle * echelle
    };
    for _ in 0..faces.len() * 4 + 8 {
        let mut aretes: std::collections::HashMap<(usize, usize), Vec<usize>> = std::collections::HashMap::new();
        for (i, f) in faces.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (f[k], f[(k + 1) % 3]);
                aretes.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        let mut touchees = vec![false; faces.len()];
        let mut retournees = 0;
        for ((a, b), voisines) in aretes {
            let [i, j] = voisines[..] else { continue };
            if touchees[i] || touchees[j] { continue; }
            let oppose = |f: [usize; 3]| f.into_iter().find(|&x| x != a && x != b).unwrap();
            let (c, d) = (oppose(faces[i]), oppose(faces[j]));
            let t = faces[i];
            if !dans_cercle(s[t[0]], s[t[1]], s[t[2]], s[d]) { continue; }
            // la nouvelle diagonale c-d doit couper a-b : quadrilatère convexe
            if orient(s[c], s[d], s[a]) * orient(s[c], s[d], s[b]) >= 0.0 || orient(s[a], s[b], s[c]) * orient(s[a], s[b], s[d]) >= 0.0 { continue; }
            let direct = |f: [usize; 3]| if orient(s[f[0]], s[f[1]], s[f[2]]) < 0.0 { [f[0], f[2], f[1]] } else { f };
            faces[i] = direct([c, d, a]);
            faces[j] = direct([d, c, b]);
            touchees[i] = true;
            touchees[j] = true;
            retournees += 1;
        }
        if retournees == 0 { break; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delaunay_supprime_les_triangles_effiles() {
        // contour carré, point très proche d'un côté : l'insertion seule laisse des
        // triangles effilés, la triangulation de Delaunay les retourne
        let t = Terrain { id: "t".into(), nom: "T".into(), contour: vec![[0.,0.,0.], [10_000.,0.,0.], [10_000.,10_000.,0.], [0.,10_000.,0.]], points: vec![[5000.,200.,1000.], [5000.,5000.,500.]], type_terrain: None };
        let tris = t.triangles().unwrap();
        let aire: f64 = tris.iter().map(|t| (t[1] - t[0]).truncate().perp_dot((t[2] - t[0]).truncate()).abs() * 0.5).sum();
        assert!((aire - 1e8).abs() < 1.0);
        // plus petit angle des triangles : pas de triangle quasi plat
        let angle_min = tris.iter().flat_map(|t| (0..3).map(move |k| {
            let (a, b, c) = (t[k].truncate(), t[(k + 1) % 3].truncate(), t[(k + 2) % 3].truncate());
            (b - a).angle_to(c - a).abs()
        })).fold(f64::MAX, f64::min);
        assert!(angle_min > 0.03, "{angle_min}");
    }
    #[test]
    fn triangulation_utilise_les_altitudes_et_refuse_un_point_exterieur() {
        let mut t = Terrain { id: "t".into(), nom: "Terrain".into(), contour: vec![[0.,0.,0.], [1000.,0.,100.], [1000.,1000.,200.], [0.,1000.,0.]], points: vec![[200.,300.,450.]], type_terrain: None };
        let tris = t.triangles().unwrap();
        assert_eq!(tris.len(), 4);
        assert!(tris.iter().flatten().any(|p| p.z == 450.));
        t.points.push([2000., 2000., 0.]);
        assert!(t.triangles().is_err());
    }
}

