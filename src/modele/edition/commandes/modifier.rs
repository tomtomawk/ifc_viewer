//! Commandes de modification composées : symétrie, échelle, réseau, décaler, scinder.
//! Elles réutilisent Copier, Déplacer et la transformation affine du plan.
use super::transformations::Affine;
use super::*;

impl Projet {
    pub(super) fn symetrie(&mut self, ids: Vec<String>, a: [f64; 2], b: [f64; 2]) -> Result<Vec<String>, String> {
        let f = Affine::symetrie(a, b).ok_or("Symétrie : l'axe doit avoir deux points distincts.")?;
        let set = self.dependances_avec(&ids, true)?;
        self.transformer_affine(&set, f, 0.0)?;
        Ok(ids)
    }

    pub(super) fn echelle(&mut self, ids: Vec<String>, centre: [f64; 2], facteur: f64) -> Result<Vec<String>, String> {
        if !(facteur.is_finite() && facteur > 1e-6) {
            return Err("Échelle : le facteur doit être positif.".into());
        }
        let set = self.dependances_avec(&ids, true)?;
        self.transformer_affine(&set, Affine::echelle(centre, facteur), 0.0)?;
        Ok(ids)
    }

    /// Réseau : `nombre` éléments au total (original compris) ; la copie k est tournée de
    /// k·`angle` degrés autour de `centre` puis déplacée de k·`delta`.
    pub(super) fn reseau(&mut self, ids: Vec<String>, nombre: usize, delta: [f64; 3], centre: [f64; 2], angle: f64) -> Result<Vec<String>, String> {
        if !(2..=500).contains(&nombre) {
            return Err("Réseau : de 2 à 500 éléments.".into());
        }
        if delta.iter().all(|v| v.abs() < 1e-9) && angle.abs() < 1e-9 {
            return Err("Réseau : le pas est nul.".into());
        }
        let mut toutes = Vec::new();
        for k in 1..nombre {
            let copies = self.executer_interne(Commande::Copier { ids: ids.clone(), delta: [0.0; 3] })?;
            let set = self.dependances_avec(&copies, true)?;
            let k = k as f64;
            self.transformer(&set, [delta[0] * k, delta[1] * k, delta[2] * k], centre, angle * k)?;
            toutes.extend(copies);
        }
        Ok(toutes)
    }

    /// Couches d'un mur (lui-même) ou d'un mur composé, dans l'ordre.
    fn couches_de(&self, id: &str) -> Vec<String> {
        match self.assemblage(id) {
            Some(a) => a.couches.clone(),
            None => vec![id.to_string()],
        }
    }

    /// Décaler : mur droit (ou composé) ou ligne d'annotation, de `distance` mm vers la
    /// gauche de son sens (négatif : vers la droite) ; `copie` garde l'original.
    pub(super) fn decaler(&mut self, id: String, distance: f64, copie: bool) -> Result<Vec<String>, String> {
        if !distance.is_finite() || distance.abs() < 1e-6 {
            return Err("Décaler : distance nulle.".into());
        }
        let (a, b) = if let Some((a, b, _)) = self.axe_element(&id) {
            if self.couches_de(&id).iter().any(|c| self.murs.iter().any(|m| &m.id == c && matches!(m.axe, Axe::Arc { .. }))) {
                return Err("Décaler : mur droit seulement (un mur courbe se modifie par son arc).".into());
            }
            (a, b)
        } else if let Some(x) = self.annotations.iter().find(|x| x.id == id) {
            match x.forme {
                documentation::AnnotationForme::Ligne { debut, fin } => (debut, fin),
                _ => return Err("Décaler : ligne d'annotation seulement.".into()),
            }
        } else {
            return Err("Décaler : choisir un mur droit ou une ligne.".into());
        };
        let u = (DVec2::from_array(b) - DVec2::from_array(a)).try_normalize().ok_or("Décaler : élément de longueur nulle.")?;
        let v = u.perp() * distance;
        let delta = [v.x, v.y, 0.0];
        let ids = vec![id];
        if copie {
            self.executer_interne(Commande::Copier { ids, delta })
        } else {
            self.executer_interne(Commande::Deplacer { ids: ids.clone(), delta })?;
            Ok(ids)
        }
    }

    /// Scinder un mur droit (ou composé) au point projeté de `point` ; `ecart` (mm)
    /// laisse un vide entre les deux parties. Les ouvertures suivent leur partie.
    pub(super) fn scinder(&mut self, id: String, point: [f64; 2], ecart: f64) -> Result<Vec<String>, String> {
        if !(ecart.is_finite() && ecart >= 0.0) {
            return Err("Scinder : écart négatif.".into());
        }
        let couches = self.couches_de(&id);
        if couches.is_empty() || !couches.iter().all(|c| self.murs.iter().any(|m| &m.id == c && matches!(m.axe, Axe::Droit { .. }))) {
            return Err("Scinder : mur droit seulement.".into());
        }
        let (axe, _, _) = solides::hote(self, &id)?;
        let longueur = axe.longueur();
        let s = axe.abscisse(point);
        if s - ecart / 2.0 < 1.0 || s + ecart / 2.0 > longueur - 1.0 {
            return Err("Scinder : le point de coupure doit être à l'intérieur du mur.".into());
        }
        let (debut_coupe, fin_coupe) = (s - ecart / 2.0, s + ecart / 2.0);
        let hote_de = |p: &Projet, o: &Ouverture, couches: &[String], id: &str| o.mur == id || couches.contains(&o.mur) && p.assemblage(id).is_some();
        for o in self.ouvertures.iter().filter(|o| hote_de(self, o, &couches, &id)) {
            if o.abscisse - o.largeur / 2.0 < fin_coupe && o.abscisse + o.largeur / 2.0 > debut_coupe {
                return Err(format!("Scinder : l'ouverture « {} » traverse le point de coupure.", o.nom));
            }
        }
        let copie = self.executer_interne(Commande::Copier { ids: vec![id.clone()], delta: [0.0; 3] })?.remove(0);
        let couches_copie = self.couches_de(&copie);
        let p = DVec2::from_array(point);
        for (c, c2) in couches.iter().zip(&couches_copie) {
            let Some(Axe::Droit { debut, fin }) = self.murs.iter().find(|m| &m.id == c).map(|m| m.axe.clone()) else { continue };
            let (a, b) = (DVec2::from_array(debut), DVec2::from_array(fin));
            let v = (b - a).normalize();
            let q = a + v * (p - a).dot(v);
            let (q1, q2) = ((q - v * ecart / 2.0).to_array(), (q + v * ecart / 2.0).to_array());
            if let Some(m) = self.murs.iter_mut().find(|m| &m.id == c) {
                m.axe = Axe::Droit { debut, fin: q1 };
            }
            if let Some(m) = self.murs.iter_mut().find(|m| &m.id == c2) {
                m.axe = Axe::Droit { debut: q2, fin };
            }
        }
        // ouvertures : chacune reste dans la partie qui la contient
        let mut a_supprimer = Vec::new();
        for o in &mut self.ouvertures {
            let original = o.mur == id || couches.contains(&o.mur) && o.mur != copie;
            let copiee = o.mur == copie || couches_copie.contains(&o.mur);
            if copiee {
                if o.abscisse < s {
                    a_supprimer.push(o.id.clone());
                } else {
                    o.abscisse -= fin_coupe;
                }
            } else if original && o.abscisse > s {
                a_supprimer.push(o.id.clone());
            }
        }
        if !a_supprimer.is_empty() {
            self.executer_interne(Commande::Supprimer { ids: a_supprimer })?;
        }
        Ok(vec![id, copie])
    }
}

#[cfg(test)]
mod tests {
    use crate::modele::edition::commandes::Commande;
    use crate::modele::edition::ParametresMur;
    use crate::modele::{Axe, Projet};

    fn mur(p: &mut Projet, a: [f64; 2], b: [f64; 2]) -> String {
        p.ajouter_mur(a, b, &ParametresMur::default()).unwrap();
        p.murs.last().unwrap().id.clone()
    }
    fn axe(p: &Projet, id: &str) -> ([f64; 2], [f64; 2]) {
        match p.murs.iter().find(|m| m.id == id).unwrap().axe {
            Axe::Droit { debut, fin } => (debut, fin),
            _ => unreachable!(),
        }
    }
    fn proche(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
    }

    #[test]
    fn symetrie_retourne_le_sens_du_mur_et_garde_la_longueur() {
        let mut p = Projet::nouveau("S");
        let id = mur(&mut p, [1000.0, 0.0], [5000.0, 0.0]);
        let (q, _) = p.executer(Commande::Symetrie { ids: vec![id.clone()], a: [0.0, 0.0], b: [0.0, 1.0] }).unwrap();
        let (d, f) = axe(&q, &id);
        // image de (1000,0)→(5000,0) : (-1000,0)→(-5000,0), sens inversé
        assert!(proche(d, [-5000.0, 0.0]) && proche(f, [-1000.0, 0.0]), "{d:?} {f:?}");
    }

    #[test]
    fn echelle_refusee_pour_les_murs() {
        let mut p = Projet::nouveau("E");
        let id = mur(&mut p, [0.0, 0.0], [4000.0, 0.0]);
        assert!(p.executer(Commande::Echelle { ids: vec![id], centre: [0.0, 0.0], facteur: 2.0 }).is_err());
    }

    #[test]
    fn echelle_d_une_ligne_d_annotation() {
        use crate::modele::documentation::{Annotation, AnnotationForme};
        let mut p = Projet::nouveau("E");
        mur(&mut p, [0.0, 0.0], [1000.0, 0.0]);
        let niveau = p.niveaux[0].id.clone();
        let vue = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
        let annotation = Annotation { id: String::new(), vue, hauteur: 2.5, forme: AnnotationForme::Ligne { debut: [1000.0, 0.0], fin: [3000.0, 0.0] } };
        let (p, ids) = p.executer(Commande::Annoter { annotation }).unwrap();
        let (q, _) = p.executer(Commande::Echelle { ids: ids.clone(), centre: [0.0, 0.0], facteur: 2.0 }).unwrap();
        let AnnotationForme::Ligne { debut, fin } = q.annotations.iter().find(|a| a.id == ids[0]).unwrap().forme else { unreachable!() };
        assert!(proche(debut, [2000.0, 0.0]) && proche(fin, [6000.0, 0.0]));
    }

    #[test]
    fn reseau_lineaire_et_polaire() {
        let mut p = Projet::nouveau("R");
        let id = mur(&mut p, [0.0, 0.0], [1000.0, 0.0]);
        let (q, ids) = p.executer(Commande::Reseau { ids: vec![id.clone()], nombre: 4, delta: [0.0, 2000.0, 0.0], centre: [0.0, 0.0], angle: 0.0 }).unwrap();
        assert_eq!(ids.len(), 3);
        assert!(proche(axe(&q, &ids[2]).0, [0.0, 6000.0]));
        let (q, ids) = p.executer(Commande::Reseau { ids: vec![id], nombre: 4, delta: [0.0; 3], centre: [0.0, 0.0], angle: 90.0 }).unwrap();
        assert!(proche(axe(&q, &ids[0]).1, [0.0, 1000.0]));
        assert!(proche(axe(&q, &ids[1]).1, [-1000.0, 0.0]));
    }

    #[test]
    fn decaler_vers_la_gauche_avec_ou_sans_copie() {
        let mut p = Projet::nouveau("D");
        let id = mur(&mut p, [0.0, 0.0], [4000.0, 0.0]);
        let (q, _) = p.executer(Commande::Decaler { id: id.clone(), distance: 500.0, copie: false }).unwrap();
        assert!(proche(axe(&q, &id).0, [0.0, 500.0]));
        let (q, ids) = p.executer(Commande::Decaler { id: id.clone(), distance: -300.0, copie: true }).unwrap();
        assert_eq!(q.murs.len(), 2);
        assert!(proche(axe(&q, &id).0, [0.0, 0.0]));
        assert!(proche(axe(&q, &ids[0]).0, [0.0, -300.0]));
    }

    #[test]
    fn scinder_avec_ecart_et_ouvertures_reparties() {
        let mut p = Projet::nouveau("C");
        let id = mur(&mut p, [0.0, 0.0], [6000.0, 0.0]);
        for abscisse in [1000.0, 5000.0] {
            p = p.executer(Commande::Ouverture { mur: id.clone(), genre: crate::modele::GenreOuverture::Porte, type_ouverture: None, abscisse, largeur: 600.0, hauteur: 2000.0, allege: 0.0 }).unwrap().0;
        }
        let (q, ids) = p.executer(Commande::Scinder { id: id.clone(), point: [3000.0, 0.0], ecart: 200.0 }).unwrap();
        assert_eq!(ids.len(), 2);
        assert!(proche(axe(&q, &ids[0]).1, [2900.0, 0.0]));
        assert!(proche(axe(&q, &ids[1]).0, [3100.0, 0.0]) && proche(axe(&q, &ids[1]).1, [6000.0, 0.0]));
        let sur = |m: &str| q.ouvertures.iter().filter(|o| o.mur == m).map(|o| o.abscisse).collect::<Vec<_>>();
        assert_eq!(sur(&ids[0]), vec![1000.0]);
        assert_eq!(sur(&ids[1]).len(), 1);
        assert!((sur(&ids[1])[0] - 1900.0).abs() < 1e-6);
        // ouverture à cheval : refus
        assert!(p.executer(Commande::Scinder { id, point: [1000.0, 0.0], ecart: 0.0 }).is_err());
    }
}
