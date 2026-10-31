//! Géométrie des terrains et des limites de propriété (coordonnées en millimètres).
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Terrain {
    pub id: String,
    pub nom: String,
    /// Sommets du périmètre, dans l'ordre, avec leur altitude absolue.
    pub contour: Vec<[f64; 3]>,
    /// Points cotés strictement à l'intérieur du périmètre.
    #[serde(default)]
    pub points: Vec<[f64; 3]>,
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
        super::edition::contour_valide(&self.contour)?;
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
        let valide = super::edition::contour_valide(&contour)?;
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
        Ok(faces.into_iter().map(|t| t.map(|i| sommets[i])).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn triangulation_utilise_les_altitudes_et_refuse_un_point_exterieur() {
        let mut t = Terrain { id: "t".into(), nom: "Terrain".into(), contour: vec![[0.,0.,0.], [1000.,0.,100.], [1000.,1000.,200.], [0.,1000.,0.]], points: vec![[200.,300.,450.]] };
        let tris = t.triangles().unwrap();
        assert_eq!(tris.len(), 4);
        assert!(tris.iter().flatten().any(|p| p.z == 450.));
        t.points.push([2000., 2000., 0.]);
        assert!(t.triangles().is_err());
    }

    #[test]
    fn creation_site_sauvegarde_export_et_suppression() {
        use crate::modele::{commandes::Commande, Projet};
        let p = Projet::nouveau("Site");
        let terrain = Terrain { id: String::new(), nom: "Terrain".into(), contour: vec![[0.,0.,0.], [1000.,0.,100.], [1000.,1000.,200.], [0.,1000.,0.]], points: vec![[500.,500.,450.]] };
        let (p, t) = p.executer(Commande::Terrain { terrain }).unwrap();
        let limite = LimitePropriete { id: String::new(), nom: "Parcelle".into(), altitude: 0., contour: vec![[0.,0.], [1000.,0.], [1000.,1000.], [0.,1000.]] };
        let (p, l) = p.executer(Commande::LimitePropriete { limite }).unwrap();
        let json = p.json().unwrap();
        assert!(json.contains("limites_propriete") && json.contains("terrains"));
        let ifc = crate::modele::export_ifc::exporter(&p).unwrap();
        assert!(ifc.contains("IFCGEOGRAPHICELEMENT(") && ifc.contains("IFCPOLYLINE("));
        let fichier = crate::step::StepFile::parse(ifc.as_bytes()).unwrap();
        let ifc_modele = crate::model::IfcModel::new(fichier);
        let terrain_ifc = ifc_modele.products.iter().copied().find(|&id| ifc_modele.ty(id) == "IFCGEOGRAPHICELEMENT").unwrap();
        assert!(crate::geom::product_geometry(&ifc_modele, &crate::geom::GeomStats::default(), terrain_ifc).is_some());
        let scene = crate::scene::depuis_natif(&p, std::path::Path::new("site.json")).unwrap();
        assert!(scene.objects.iter().any(|o| o.id == crate::scene::id_natif("terrain", &t[0])));
        assert!(scene.objects.iter().any(|o| o.id == crate::scene::id_natif("limite_propriete", &l[0])));
        let (copie, ids) = p.executer(Commande::Copier { ids: vec![t[0].clone(), l[0].clone()], delta: [3000., 0., 0.] }).unwrap();
        assert_eq!(copie.terrains.len(), 2);
        assert_eq!(copie.limites_propriete.len(), 2);
        assert_eq!(ids.len(), 2);
        assert!(copie.terrains.iter().any(|x| x.contour[0][0] == 3000.));
        let (p, _) = p.executer(Commande::Supprimer { ids: vec![t[0].clone(), l[0].clone()] }).unwrap();
        assert!(p.terrains.is_empty() && p.limites_propriete.is_empty());
    }
}

