//! Révolution d'un profil fermé autour d'un axe dans son plan d'esquisse.
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Revolution {
    pub axe: [[f64; 2]; 2],
    pub debut: f64,
    pub angle: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests_maillage::verifier_fermeture;

    #[test]
    fn revolutions_fermees_partielles_et_profils_sur_axe() {
        for rayon_interieur in [0.0, 30.0] {
            for angle in [90.0,180.0,360.0] {
                for cote in [-1.0,1.0] {
                    let mut contour=vec![[rayon_interieur*cote,0.0],[100.0*cote,0.0],[100.0*cote,200.0],[rayon_interieur*cote,200.0]];
                    for _ in 0..2 {
                        let revolution=Revolution {axe:[[0.0,0.0],[0.0,100.0]],debut:25.0,angle};
                        let tris=revolution.triangles(&contour);
                        verifier_fermeture(&tris);
                        let volume:f64=tris.iter().map(|t| t[0].dot(t[1].cross(t[2]))/6.0).sum();
                        let attendu=std::f64::consts::PI*(10000.0-rayon_interieur*rayon_interieur)*200.0*angle/360.0;
                        assert!((volume-attendu).abs()/attendu < 0.002, "{volume} / {attendu}");
                        contour.reverse();
                    }
                }
            }
        }
    }

    #[test]
    fn revolution_axe_oblique_et_rechargement() {
        let s=std::f64::consts::FRAC_1_SQRT_2;
        let tourner=|p:[f64;2]| [10.0+(p[0]-p[1])*s,20.0+(p[0]+p[1])*s];
        let r=Revolution {axe:[tourner([0.0,0.0]),tourner([0.0,100.0])],debut:-45.0,angle:270.0};
        let contour=[[30.0,0.0],[100.0,0.0],[100.0,200.0],[30.0,200.0]].map(tourner);
        verifier_fermeture(&r.triangles(&contour));
        assert_eq!(r,serde_json::from_str::<Revolution>(&serde_json::to_string(&r).unwrap()).unwrap());
    }

    #[test]
    fn revolution_refuse_axe_profil_et_angles_invalides() {
        let contour=[[10.0,0.0],[100.0,0.0],[100.0,100.0],[10.0,100.0]];
        for angle in [0.0,-10.0,361.0,f64::NAN] {
            let r=Revolution {axe:[[0.0,0.0],[0.0,100.0]],debut:0.0,angle};
            assert!(r.valider(&contour).is_err()); assert!(r.triangles(&contour).is_empty());
        }
        assert!(Revolution::nouvelle([[0.0,0.0];2]).valider(&contour).is_err());
        assert!(Revolution::nouvelle([[50.0,0.0],[50.0,100.0]]).valider(&contour).is_err());
    }
}

impl Revolution {
    pub fn nouvelle(axe: [[f64; 2]; 2]) -> Self { Self { axe, debut: 0.0, angle: 360.0 } }

    pub fn valider(&self, contour: &[[f64; 2]]) -> Result<(), String> {
        let a = DVec2::from_array(self.axe[0]);
        let d = DVec2::from_array(self.axe[1]) - a;
        if !a.is_finite() || !d.is_finite() || d.length() < 0.1 {
            return Err("Révolution : choisir deux points distincts pour l'axe.".into());
        }
        if !self.debut.is_finite() || !self.angle.is_finite() || self.angle <= 0.0 || self.angle > 360.0 {
            return Err("Révolution : l'angle doit être supérieur à 0° et inférieur ou égal à 360°.".into());
        }
        if contour.iter().flatten().any(|x| !x.is_finite()) { return Err("Révolution : coordonnées non finies.".into()); }
        super::super::profil::contour_simple(contour)?;
        let d = d.normalize();
        let rayons: Vec<_> = contour.iter().map(|p| d.perp_dot(DVec2::from_array(*p)-a)).collect();
        if rayons.iter().any(|r| *r < -1e-7) && rayons.iter().any(|r| *r > 1e-7) {
            return Err("Révolution : le profil doit rester d'un seul côté de l'axe.".into());
        }
        if rayons.iter().all(|r| r.abs() < 0.1) { return Err("Révolution : profil confondu avec l'axe.".into()); }
        Ok(())
    }

    pub fn triangles(&self, contour: &[[f64; 2]]) -> Vec<[DVec3; 3]> {
        if self.valider(contour).is_err() { return Vec::new(); }
        let origine = DVec3::new(self.axe[0][0], self.axe[0][1], 0.0);
        let axe = (DVec3::new(self.axe[1][0], self.axe[1][1], 0.0)-origine).normalize();
        let ferme = (self.angle - 360.0).abs() < 1e-9;
        let pas = (self.angle / 5.0).ceil().max(1.0) as usize;
        let point = |i: usize, j: usize| {
            let j = if ferme && j == pas { 0 } else { j };
            let angle = (self.debut.rem_euclid(360.0) + self.angle * j as f64 / pas as f64).to_radians();
            let p = DVec3::new(contour[i][0], contour[i][1], 0.0)-origine;
            origine + p * angle.cos() + axe.cross(p) * angle.sin() + axe * axe.dot(p) * (1.0-angle.cos())
        };
        let n = contour.len();
        let area: f64 = (0..n).map(|i| contour[i][0]*contour[(i+1)%n][1]-contour[(i+1)%n][0]*contour[i][1]).sum();
        let mut triangles = Vec::new();
        for j in 0..pas {
            for i in 0..n {
                let suivant = (i+1)%n;
                for mut t in [[point(i,j),point(suivant,j),point(suivant,j+1)], [point(i,j),point(suivant,j+1),point(i,j+1)]] {
                    if area < 0.0 { t.swap(1,2); }
                    if (t[1]-t[0]).cross(t[2]-t[0]).length_squared() > 1e-16 { triangles.push(t); }
                }
            }
        }
        if !ferme {
            let coords: Vec<_> = contour.iter().flatten().copied().collect();
            let Ok(indices) = earcutr::earcut(&coords, &[], 2) else { return Vec::new(); };
            for t in indices.chunks_exact(3) {
                let mut ids = [t[0],t[1],t[2]];
                let a = DVec2::from_array(contour[ids[0]]);
                let b = DVec2::from_array(contour[ids[1]]);
                let c = DVec2::from_array(contour[ids[2]]);
                if (b-a).perp_dot(c-a) < 0.0 { ids.swap(1,2); }
                triangles.push([point(ids[2],0),point(ids[1],0),point(ids[0],0)]);
                triangles.push([point(ids[0],pas),point(ids[1],pas),point(ids[2],pas)]);
            }
        }
        // La convention d'orientation dépend du côté de l'axe où se trouve le profil.
        let volume: f64 = triangles.iter().map(|t| (t[0]-origine).dot((t[1]-origine).cross(t[2]-origine))).sum();
        if volume < 0.0 { for t in &mut triangles { t.swap(1,2); } }
        triangles
    }
}
