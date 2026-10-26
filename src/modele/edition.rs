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
    pub niveau_haut: Option<String>,
    pub decalage_haut: f64,
    pub phase: Option<String>,
    pub sous_projet: Option<String>,
}

impl Default for ParametresMur {
    fn default() -> Self {
        Self { hauteur: 2800.0, epaisseur: 300.0, implantation: Implantation::Axe, niveau: None, decalage: 0.0, niveau_haut: None, decalage_haut: 0.0, phase: None, sous_projet: None }
    }
}

impl ParametresMur {
    /// Altitudes résolues pour la validation, le schéma et l'aperçu 3D (mm).
    pub fn bornes(&self, projet: &Projet) -> Result<(f64, f64), String> {
        let base = match &self.niveau {
            Some(id) => projet.niveau(id).ok_or("Limite basse introuvable.")?.altitude,
            None => projet.niveaux.first().map_or(0.0, |n| n.altitude),
        };
        let bas = base + self.decalage;
        let haut = match &self.niveau_haut {
            Some(id) => projet.niveau(id).ok_or("Limite haute introuvable.")?.altitude + self.decalage_haut,
            None => bas + self.hauteur + self.decalage_haut,
        };
        if ![base, bas, haut, self.decalage, self.decalage_haut].iter().all(|x| x.is_finite()) || haut <= bas {
            return Err("La limite haute du mur doit être au-dessus de sa limite basse ; les valeurs doivent être finies.".into());
        }
        Ok((bas, haut))
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
        let (bas, haut) = p.bornes(self)?;
        for (valeur, choix, nom) in [(&p.phase, &self.phases, "Phase"), (&p.sous_projet, &self.sous_projets, "Sous-projet")] {
            if let Some(v) = valeur {
                if v.trim().is_empty() || (!choix.is_empty() && !choix.contains(v)) {
                    return Err(format!("{nom} introuvable."));
                }
            }
        }
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
        if self.phases.is_empty() { self.phases.push(p.phase.clone().unwrap_or_else(|| "Projet".into())); }
        if self.sous_projets.is_empty() { self.sous_projets.push(p.sous_projet.clone().unwrap_or_else(|| "Sous-projet 1".into())); }
        let contrainte = match &p.niveau_haut {
            Some(niveau) => Haut::Niveau { niveau: niveau.clone(), decalage: p.decalage_haut },
            None => Haut::Libre { hauteur: haut - bas },
        };
        self.murs.push(Mur { phase: p.phase.clone().or_else(|| self.phases.last().cloned()), sous_projet: p.sous_projet.clone().or_else(|| self.sous_projets.first().cloned()), id: id.clone(), nom: format!("Mur {k}"), type_mur, niveau, decalage_bas: p.decalage,
            haut: contrainte, axe: Axe::Droit { debut: deplacer(debut), fin: deplacer(fin) }, origine: None });
        self.arrondir();
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mur_contraintes_phases_et_sous_projets_persistes() {
        let mut p = Projet::nouveau("Contraintes");
        p.niveaux = vec![
            Niveau { id: "bas".into(), nom: "RDC".into(), altitude: 1000., origine: None },
            Niveau { id: "haut".into(), nom: "Étage".into(), altitude: 4200., origine: None },
        ];
        p.phases = vec!["Existant".into(), "Projet".into()];
        p.sous_projets = vec!["Structure".into(), "Enveloppe".into()];
        let mut options = ParametresMur { niveau: Some("bas".into()), niveau_haut: Some("haut".into()),
            decalage: 150., decalage_haut: -200., phase: Some("Projet".into()), sous_projet: Some("Enveloppe".into()), ..Default::default() };
        assert_eq!(options.bornes(&p).unwrap(), (1150., 4000.));
        p.ajouter_mur([0.,0.], [1000.,0.], &options).unwrap();
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        let mur = &relu.murs[0];
        assert_eq!(relu.bas_haut(mur), Some((1150.,4000.)));
        assert_eq!(mur.haut, Haut::Niveau { niveau: "haut".into(), decalage: -200. });
        assert_eq!(mur.phase.as_deref(), Some("Projet"));
        assert_eq!(mur.sous_projet.as_deref(), Some("Enveloppe"));
        assert_eq!(relu.sous_projets, p.sous_projets);
        let scene = crate::scene::depuis_natif(&relu, std::path::Path::new("test.json")).unwrap();
        assert!(((scene.max.z-scene.min.z) as f64 - 2.85).abs() < 1e-5);
        let avant = p.json().unwrap();
        for (niveau, decalage) in [("absent", 0.), ("bas", 0.), ("haut", -4000.), ("haut", f64::INFINITY)] {
            options.niveau_haut = Some(niveau.into()); options.decalage_haut = decalage;
            assert!(p.ajouter_mur([0.,0.], [1000.,0.], &options).is_err());
            assert_eq!(p.json().unwrap(), avant);
        }
        options.niveau_haut = None; options.decalage_haut = 100.;
        assert_eq!(options.bornes(&p).unwrap(), (1150.,4050.));
        options.phase = Some("Absente".into());
        assert!(p.ajouter_mur([0.,0.], [1000.,0.], &options).is_err());
        assert_eq!(p.json().unwrap(), avant);
    }

    #[test]
    fn anciens_modeles_sans_phase_ni_sous_projet_restent_lisibles() {
        let mut p = Projet::nouveau("Ancien");
        p.ajouter_mur([0.,0.], [1000.,0.], &ParametresMur::default()).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&p.json().unwrap()).unwrap();
        let objet = json.as_object_mut().unwrap();
        objet.remove("phases"); objet.remove("sous_projets");
        let mur = objet["murs"][0].as_object_mut().unwrap();
        mur.remove("phase"); mur.remove("sous_projet");
        let relu = Projet::lire_json(&json.to_string()).unwrap();
        assert!(relu.phases.is_empty());
        assert!(relu.murs[0].phase.is_none());
        assert_eq!(relu.bas_haut(&relu.murs[0]), Some((0.,2800.)));
    }

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
