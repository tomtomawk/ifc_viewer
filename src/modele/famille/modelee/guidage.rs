//! Formes entre sections : raccordement et balayage sur chemin polygonal.
use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "genre", rename_all = "snake_case")]
pub enum FormeGuidee {
    ExtrusionChemin { chemin: Vec<[f64; 3]> },
    Raccordement { profil_fin: Vec<[f64; 2]>, hauteur: f64 },
    RaccordementChemin { profil_fin: Vec<[f64; 2]>, chemin: Vec<[f64; 3]> },
}

impl FormeGuidee {
    pub fn nom(&self) -> &'static str {
        match self {
            Self::ExtrusionChemin { .. } => "Extrusion par chemin",
            Self::Raccordement { .. } => "Raccordement",
            Self::RaccordementChemin { .. } => "Raccordement par chemin",
        }
    }

    pub fn chemin(&self) -> Vec<[f64; 3]> {
        match self {
            Self::Raccordement { hauteur, .. } => vec![[0.0; 3], [0.0, 0.0, *hauteur]],
            Self::ExtrusionChemin { chemin } | Self::RaccordementChemin { chemin, .. } => chemin.clone(),
        }
    }

    pub fn profil_fin(&self) -> Option<&[[f64; 2]]> {
        match self {
            Self::ExtrusionChemin { .. } => None,
            Self::Raccordement { profil_fin, .. } | Self::RaccordementChemin { profil_fin, .. } => Some(profil_fin),
        }
    }

    pub fn valider(&self, profil: &[[f64; 2]]) -> Result<(), String> {
        let simple = |p: &[[f64; 2]]| {
            if p.iter().flatten().any(|x| !x.is_finite()) { return Err("Profil : coordonnées non finies.".into()); }
            super::super::profil::contour_simple(p)
        };
        simple(profil)?;
        let fin = self.profil_fin().unwrap_or(profil);
        simple(fin)?;
        if fin.len() != profil.len() {
            return Err("Les deux profils doivent avoir le même nombre de sommets, dans le même ordre de correspondance.".into());
        }
        if aire(profil) * aire(fin) <= 0.0 {
            return Err("Les deux profils doivent être dessinés dans le même sens.".into());
        }
        // Empêche les effondrements et croisements intermédiaires usuels.
        for i in 1..20 {
            let section = interpoler(profil, fin, i as f64 / 20.0);
            simple(&section).map_err(|e| format!("Raccordement : section intermédiaire invalide ({e})"))?;
            if aire(&section) * aire(profil) <= 0.0 { return Err("Le raccordement inverse une section intermédiaire.".into()); }
        }
        let chemin = self.chemin();
        if chemin.len() < 2 { return Err("Le chemin doit contenir au moins deux points.".into()); }
        if chemin.iter().flatten().any(|x| !x.is_finite()) { return Err("Chemin : coordonnées non finies.".into()); }
        let mut precedent: Option<DVec3> = None;
        for segment in chemin.windows(2) {
            let d = DVec3::from_array(segment[1]) - DVec3::from_array(segment[0]);
            if !d.length().is_finite() || d.length() < 0.1 { return Err("Le chemin contient un segment nul ou trop court.".into()); }
            let direction = d.normalize();
            if precedent.is_some_and(|p| p.dot(direction) < -0.9999) {
                return Err("Le chemin ne peut pas faire demi-tour sur lui-même.".into());
            }
            precedent = Some(direction);
        }
        if DVec3::from_array(chemin[0]).distance(DVec3::from_array(*chemin.last().unwrap())) < 0.1 {
            return Err("Utiliser un chemin ouvert ; les chemins fermés ne sont pas encore pris en charge.".into());
        }
        Ok(())
    }

    pub fn triangles(&self, profil: &[[f64; 2]]) -> Vec<[DVec3; 3]> {
        if self.valider(profil).is_err() { return Vec::new(); }
        let fin = self.profil_fin().unwrap_or(profil);
        let chemin: Vec<_> = self.chemin().into_iter().map(DVec3::from_array).collect();
        let directions: Vec<_> = chemin.windows(2).map(|p| (p[1]-p[0]).normalize()).collect();
        let longueurs: Vec<_> = chemin.windows(2).map(|p| p[1].distance(p[0])).collect();
        let total: f64 = longueurs.iter().sum();
        let mut sections = Vec::new();
        let mut distance = 0.0;
        let mut normale = DVec3::Z;
        let mut horizontal = DVec3::X;
        for (i, origine) in chemin.iter().enumerate() {
            if i > 0 { distance += longueurs[i-1]; }
            let tangente = if i == 0 { directions[0] }
                else if i == chemin.len()-1 { directions[i-1] }
                else { (directions[i-1]+directions[i]).normalize() };
            // Transport du repère, sans réinitialiser l'orientation à chaque segment.
            horizontal = DQuat::from_rotation_arc(normale, tangente) * horizontal;
            horizontal = (horizontal - tangente * horizontal.dot(tangente)).normalize();
            let vertical = tangente.cross(horizontal);
            normale = tangente;
            let contour = interpoler(profil, fin, distance / total);
            sections.push(contour.iter().map(|p| *origine + horizontal*p[0] + vertical*p[1]).collect::<Vec<_>>());
        }
        let mut tris = Vec::new();
        let n = profil.len();
        for paire in sections.windows(2) {
            for i in 0..n {
                let j = (i+1)%n;
                for mut t in [[paire[0][i],paire[0][j],paire[1][j]], [paire[0][i],paire[1][j],paire[1][i]]] {
                    if aire(profil) < 0.0 { t.swap(1,2); }
                    tris.push(t);
                }
            }
        }
        for (contour, section, inverse) in [(profil, &sections[0], true), (fin, sections.last().unwrap(), false)] {
            let coords: Vec<_> = contour.iter().flatten().copied().collect();
            let Ok(indices) = earcutr::earcut(&coords, &[], 2) else { return Vec::new(); };
            for ids in indices.chunks_exact(3) {
                let (a,b,c) = (contour[ids[0]],contour[ids[1]],contour[ids[2]]);
                let sens = (b[0]-a[0])*(c[1]-a[1]) - (b[1]-a[1])*(c[0]-a[0]);
                let mut t = [section[ids[0]],section[ids[1]],section[ids[2]]];
                if (sens < 0.0) != inverse { t.swap(1,2); }
                tris.push(t);
            }
        }
        if tris.iter().any(|t| t.iter().any(|p| !p.is_finite()) || (t[1]-t[0]).cross(t[2]-t[0]).length_squared()<1e-16) { return Vec::new(); }
        let origine = chemin[0];
        let volume: f64 = tris.iter().map(|t| (t[0]-origine).dot((t[1]-origine).cross(t[2]-origine))).sum();
        if volume.abs() < 1e-9 { return Vec::new(); }
        if volume < 0.0 { for t in &mut tris { t.swap(1,2); } }
        tris
    }
}

fn aire(p: &[[f64; 2]]) -> f64 {
    (0..p.len()).map(|i| p[i][0]*p[(i+1)%p.len()][1]-p[(i+1)%p.len()][0]*p[i][1]).sum()
}

fn interpoler(a: &[[f64; 2]], b: &[[f64; 2]], t: f64) -> Vec<[f64; 2]> {
    a.iter().zip(b).map(|(a,b)| [a[0]*(1.0-t)+b[0]*t,a[1]*(1.0-t)+b[1]*t]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests_maillage::{verifier_fermeture, volume};

    fn profil() -> Vec<[f64;2]> { vec![[-100.0,-50.0],[100.0,-50.0],[100.0,50.0],[-100.0,50.0]] }

    #[test]
    fn guidages_droits_volumes_et_orientation() {
        for inverse in [false,true] {
            let mut p=profil();
            if inverse { p.reverse(); }
            let fin: Vec<_>=p.iter().map(|q| [q[0]/2.0,q[1]/2.0]).collect();
            let chemin=vec![[0.0;3],[0.0,0.0,1000.0]];
            for (g,attendu) in [
                (FormeGuidee::ExtrusionChemin {chemin:chemin.clone()},20_000_000.0),
                (FormeGuidee::Raccordement {profil_fin:fin.clone(),hauteur:1000.0},35_000_000.0/3.0),
                (FormeGuidee::RaccordementChemin {profil_fin:fin,chemin},35_000_000.0/3.0),
            ] {
                g.valider(&p).unwrap();
                let tris=g.triangles(&p);
                verifier_fermeture(&tris);
                assert!((volume(&tris)-attendu).abs()/attendu<1e-8);
                assert_eq!(g,serde_json::from_str::<FormeGuidee>(&serde_json::to_string(&g).unwrap()).unwrap());
            }
        }
    }

    #[test]
    fn guidages_chemin_spatial_et_sections_intermediaires() {
        let p=profil();
        let chemin=vec![[0.0;3],[0.0,0.0,1000.0],[500.0,0.0,2000.0],[500.0,500.0,3000.0]];
        for g in [FormeGuidee::ExtrusionChemin {chemin:chemin.clone()}, FormeGuidee::RaccordementChemin {profil_fin:p.iter().map(|q| [q[0]*0.5,q[1]*0.8]).collect(),chemin}] {
            let tris=g.triangles(&p);
            verifier_fermeture(&tris);
            assert!(volume(&tris)>0.0);
        }
    }

    #[test]
    fn guidages_refusent_entrees_invalides() {
        for chemin in [vec![],vec![[0.0;3]],vec![[0.0;3],[0.0;3]],vec![[0.0;3],[0.0,0.0,100.0],[0.0,0.0,50.0]],vec![[0.0;3],[f64::NAN,0.0,10.0]]] {
            let g=FormeGuidee::ExtrusionChemin {chemin};
            assert!(g.valider(&profil()).is_err()); assert!(g.triangles(&profil()).is_empty());
        }
        let mut fin=profil(); fin.pop();
        assert!(FormeGuidee::Raccordement {profil_fin:fin,hauteur:100.0}.valider(&profil()).is_err());
        let mut fin=profil(); fin.reverse();
        assert!(FormeGuidee::Raccordement {profil_fin:fin,hauteur:100.0}.valider(&profil()).is_err());
        assert!(FormeGuidee::Raccordement {profil_fin:profil(),hauteur:0.0}.valider(&profil()).is_err());
    }

    #[test]
    fn guidage_famille_rechargee_et_repere_oblique() {
        use super::super::{FamilleModelee,Extrusion,PlanConstruction,VueEsquisse,Face,PointEsquisse,Materiau};
        let mut famille=FamilleModelee::gabarit_fenetre();
        famille.volumes.clear(); famille.extrusions.clear();
        let plan=PlanConstruction::deux_points("Oblique".into(),VueEsquisse::Plan,[0.0,0.0],[100.0,100.0]).unwrap();
        let face=|v| Face {plan:String::new(),decalage:v};
        famille.extrusions.push(Extrusion {
            guidage:Some(FormeGuidee::RaccordementChemin {profil_fin:profil().iter().map(|p| [p[0]*0.5,p[1]*0.5]).collect(),chemin:vec![[0.0;3],[0.0,0.0,1000.0]]}),
            revolution:None,construction:Some(plan.clone()),nom:"Guidage test".into(),vide:true,materiau:Materiau::Menuiserie,vue:VueEsquisse::Plan,
            profil:profil().iter().map(|p| PointEsquisse {h:face(p[0]),v:face(p[1])}).collect(),
            profondeur:[face(0.0),face(1000.0)],visible:None,materiau_parametre:None,
        });
        famille.plans_construction.push(plan);
        famille.valider(1200.0,1350.0,300.0).unwrap();
        let relue:FamilleModelee=serde_json::from_str(&serde_json::to_string(&famille).unwrap()).unwrap();
        assert_eq!(famille,relue);
        let solides=relue.solides(&relue.resoudre(1200.0,1350.0,300.0));
        assert!(solides[0].vide);
        verifier_fermeture(&solides[0].tris);
        assert!((volume(&solides[0].tris)-35_000_000.0/3.0).abs()<0.001);
    }
}
