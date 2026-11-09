//! Familles de profils plans définis par des paramètres et un contour fermé.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParametreProfil { pub nom: String, pub formule: String }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointProfil { pub x: String, pub y: String }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profil {
    #[serde(default)] pub id: String,
    pub nom: String,
    pub parametres: Vec<ParametreProfil>,
    pub contour: Vec<PointProfil>,
}

impl Profil {
    pub fn points(&self) -> Result<Vec<[f64; 2]>, String> {
        if self.nom.trim().is_empty() { return Err("Donnez un nom au profil.".into()); }
        let mut noms = HashSet::new();
        for p in &self.parametres {
            if p.nom.is_empty() || !p.nom.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || p.nom.chars().next().is_some_and(|c| c.is_ascii_digit()) || !noms.insert(p.nom.to_lowercase()) {
                return Err(format!("Nom de paramètre invalide ou dupliqué : {}.", p.nom));
            }
        }
        let mut vars = BTreeMap::new();
        let mut attente: Vec<_> = self.parametres.iter().collect();
        while !attente.is_empty() {
            let avant = attente.len();
            attente.retain(|p| match super::modelee::evaluer(&p.formule, &vars) {
                Ok(v) if v.is_finite() => { vars.insert(p.nom.clone(), v); false },
                _ => true,
            });
            if attente.len() == avant { return Err(format!("Formule invalide ou référence circulaire : {}.", attente[0].nom)); }
        }
        let points: Vec<[f64; 2]> = self.contour.iter().map(|p| {
            let x = super::modelee::evaluer(&p.x, &vars)?;
            let y = super::modelee::evaluer(&p.y, &vars)?;
            if !x.is_finite() || !y.is_finite() { return Err("Coordonnée non finie.".into()); }
            Ok([x, y])
        }).collect::<Result<_, String>>()?;
        if points.len() < 3 { return Err("Le contour doit compter au moins trois sommets.".into()); }
        let n = points.len();
        let croix = |a: [f64;2], b: [f64;2], c: [f64;2]| (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);
        let mut aire = 0.0;
        for i in 0..n {
            let a = points[i]; let b = points[(i+1)%n];
            if (a[0]-b[0]).hypot(a[1]-b[1]) < 1e-7 { return Err("Deux sommets consécutifs sont confondus.".into()); }
            aire += a[0]*b[1]-b[0]*a[1];
            for j in i+1..n {
                if j == i+1 || (i == 0 && j == n-1) { continue; }
                let c = points[j]; let d = points[(j+1)%n];
                let (ab_c, ab_d, cd_a, cd_b) = (croix(a,b,c), croix(a,b,d), croix(c,d,a), croix(c,d,b));
                let sur_segment = |a: [f64;2], b: [f64;2], p: [f64;2]| p[0] >= a[0].min(b[0])-1e-7 && p[0] <= a[0].max(b[0])+1e-7 && p[1] >= a[1].min(b[1])-1e-7 && p[1] <= a[1].max(b[1])+1e-7;
                if (ab_c * ab_d < 0.0 && cd_a * cd_b < 0.0)
                    || (ab_c.abs() < 1e-7 && sur_segment(a,b,c)) || (ab_d.abs() < 1e-7 && sur_segment(a,b,d))
                    || (cd_a.abs() < 1e-7 && sur_segment(c,d,a)) || (cd_b.abs() < 1e-7 && sur_segment(c,d,b)) {
                    return Err("Le contour se croise ou se touche.".into());
                }
            }
        }
        if aire.abs() < 1e-7 { return Err("Le contour doit avoir une surface non nulle.".into()); }
        Ok(points)
    }
    pub fn valider(&self) -> Result<(), String> { self.points().map(|_| ()) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FichierProfil { pub format: String, pub profil: Profil }
impl FichierProfil {
    pub fn lire(s: &str) -> Result<Profil, String> {
        let f: Self = serde_json::from_str(s).map_err(|e| format!("Fichier de profil illisible : {e}"))?;
        if f.format != "bimedit.profil" { return Err("Ce fichier n'est pas un profil BIMedit.".into()); }
        f.profil.valider()?;
        Ok(f.profil)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn profils_du_gabarit_varient_et_survivent_au_rechargement() {
        let g = crate::config::gabarit::Gabarit::defaut();
        assert_eq!(g.profils.len(), 4);
        let mut p = crate::modele::Projet::depuis_gabarit("Profils", &g);
        p.verifier().unwrap();
        for profil in &mut p.profils {
            let avant = profil.points().unwrap();
            profil.parametres[0].formule = "250".into();
            assert_ne!(avant, profil.points().unwrap());
        }
        let relu = crate::modele::Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(p.profils, relu.profils);
        let fichier = super::FichierProfil { format: "bimedit.profil".into(), profil: p.profils[0].clone() };
        assert_eq!(super::FichierProfil::lire(&serde_json::to_string(&fichier).unwrap()).unwrap(), p.profils[0]);
        let partage = crate::modele::collaboration::projet(&crate::modele::collaboration::objets(&p).unwrap()).unwrap();
        assert_eq!(p.profils, partage.profils);
    }
    #[test]
    fn refuse_formule_cyclique_et_contour_invalide() {
        let mut p = crate::config::gabarit::Gabarit::defaut().profils[0].clone();
        p.parametres[0].formule = p.parametres[0].nom.clone();
        assert!(p.valider().is_err());
        p.parametres[0].formule = "100".into();
        p.contour.truncate(2);
        assert!(p.valider().is_err());
    }
}
