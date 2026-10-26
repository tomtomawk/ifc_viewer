//! Création paramétrique d'éléments natifs. Les coordonnées sont en millimètres.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Implantation { Axe, Exterieure, Interieure }

#[derive(Clone)]
pub struct ParametresMur {
    pub hauteur: f64,
    pub epaisseur: f64,
    pub implantation: Implantation,
    pub niveau: Option<String>,
    pub decalage: f64,
}

impl Default for ParametresMur {
    fn default() -> Self {
        Self { hauteur: 2800.0, epaisseur: 300.0, implantation: Implantation::Axe, niveau: None, decalage: 0.0 }
    }
}

impl Projet {
    /// Valide entièrement la commande avant d'ajouter le niveau, le type et le mur.
    pub fn ajouter_mur(&mut self, debut: [f64; 2], fin: [f64; 2], p: &ParametresMur) -> Result<String, String> {
        if !debut.iter().chain(&fin).all(|x| x.is_finite()) || ![p.hauteur, p.epaisseur, p.decalage].iter().all(|x| x.is_finite()) {
            return Err("Les coordonnées et dimensions du mur doivent être finies.".into());
        }
        let longueur = (fin[0] - debut[0]).hypot(fin[1] - debut[1]);
        if !longueur.is_finite() || longueur < 1.0 { return Err("Le mur doit avoir une longueur finie d’au moins 1 mm.".into()); }
        if p.hauteur <= 0.0 || p.epaisseur <= 0.0 { return Err("La hauteur et l’épaisseur doivent être positives.".into()); }
        let niveau = match &p.niveau {
            Some(id) => self.niveau(id).ok_or("Niveau de base introuvable.")?.id.clone(),
            None => self.niveaux.first().map(|n| n.id.clone()).unwrap_or_else(|| identifiant("niveau", "niveau-0")),
        };
        let existant = self.types_murs.iter().find(|t| (t.epaisseur() - p.epaisseur).abs() < 1e-6 && t.couches.len() == 1).map(|t| t.id.clone());
        let type_mur = existant.clone().unwrap_or_else(|| {
            let mut k = self.types_murs.len() + 1;
            loop { let id = identifiant("type-mur", &format!("natif-{k}")); if self.types_murs.iter().all(|t| t.id != id) { break id; } k += 1; }
        });
        let mut k = self.murs.len() + 1;
        let id = loop { let id = identifiant("mur", &format!("natif-{k}")); if self.murs.iter().all(|m| m.id != id) { break id; } k += 1; };
        let signe = match p.implantation { Implantation::Axe => 0.0, Implantation::Exterieure => -0.5, Implantation::Interieure => 0.5 };
        let n = [-(fin[1] - debut[1]) / longueur, (fin[0] - debut[0]) / longueur];
        let deplacer = |q: [f64; 2]| [q[0] + n[0] * p.epaisseur * signe, q[1] + n[1] * p.epaisseur * signe];
        if self.niveaux.is_empty() {
            self.niveaux.push(Niveau { id: niveau.clone(), nom: "Niveau 0".into(), altitude: 0.0, origine: None });
        }
        if existant.is_none() {
            self.types_murs.push(TypeCompose { id: type_mur.clone(), nom: format!("Mur · {} mm", p.epaisseur), couches: vec![Couche { materiau: None, epaisseur: p.epaisseur }], couches_detaillees: true, origine: None });
        }
        self.murs.push(Mur { id: id.clone(), nom: format!("Mur {k}"), type_mur, niveau, decalage_bas: p.decalage,
            haut: Haut::Libre { hauteur: p.hauteur }, axe: Axe::Droit { debut: deplacer(debut), fin: deplacer(fin) }, origine: None });
        self.arrondir();
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mur_creation_enregistrement_relecture() {
        let mut p = Projet::nouveau("Essai");
        let id = p.ajouter_mur([0.,0.], [5000.,0.], &ParametresMur::default()).unwrap();
        p.ajouter_mur([5000.,0.], [5000.,3000.], &ParametresMur::default()).unwrap();
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(relu.murs[0].id, id);
        assert_eq!(relu.niveaux.len(), 1);
        assert_eq!(relu.types_murs.len(), 1);
        assert_eq!(relu.murs.len(), 2);
        assert!((relu.volume_mur(&relu.murs[0]).unwrap() - 4_200_000_000.0).abs() < 1.0);
        let s = crate::scene::depuis_natif(&relu, std::path::Path::new("essai.modele.json")).unwrap();
        assert_eq!(s.objects.len(), 2);
        assert_eq!(s.triangles, 24);
        let volume: f64 = s.objects.iter().map(|o| s.vertices[o.opaque.start as usize..(o.opaque.start+o.opaque.count) as usize].chunks_exact(3).map(|t| {
            let p = [0,1,2].map(|k| glam::Vec3::from(t[k].pos).as_dvec3());
            p[0].dot(p[1].cross(p[2])) / 6.0
        }).sum::<f64>()).sum();
        assert!((volume - 6.72).abs() < 1e-5, "Volume du maillage : {volume}");
    }
    #[test]
    fn mur_implantations_et_erreurs_sans_mutation() {
        for (implantation, y) in [(Implantation::Axe,0.), (Implantation::Exterieure,-150.), (Implantation::Interieure,150.)] {
            let mut p = Projet::nouveau("Essai");
            let mut options = ParametresMur { implantation, ..Default::default() };
            p.ajouter_mur([0.,0.],[1000.,0.],&options).unwrap();
            assert!(matches!(p.murs[0].axe, Axe::Droit { debut, .. } if debut[1] == y));
            let avant = p.json().unwrap();
            assert!(p.ajouter_mur([0.,0.],[0.,0.],&options).is_err());
            options.hauteur = -1.;
            assert!(p.ajouter_mur([0.,0.],[1000.,0.],&options).is_err());
            assert_eq!(p.json().unwrap(), avant);
        }
    }
}
