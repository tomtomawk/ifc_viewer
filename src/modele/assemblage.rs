//! Assemblages de murs : un mur composé dont chaque couche est un mur indépendant.
//! Les couches vont de l'extérieur vers l'intérieur ; l'extérieur est à gauche de l'axe
//! parcouru du début vers la fin (normale (−dy, dx)), comme un mur Revit tracé dans le
//! sens horaire.
use super::*;

/// Matériaux reconnus par leur nom : couleur et motif de coupe par défaut.
pub fn materiau_par_defaut(nom: &str) -> Materiau {
    let n = nom.to_lowercase();
    let (couleur, motif) = if ["isol", "laine", "polystyr", "pse", "pui", "fibre"].iter().any(|k| n.contains(k)) {
        ([236, 214, 120], MotifCoupe::Croisees)
    } else if ["plâtre", "platre", "placo", "enduit", "peinture", "finition"].iter().any(|k| n.contains(k)) {
        ([238, 236, 230], MotifCoupe::Vide)
    } else if ["brique", "terre cuite"].iter().any(|k| n.contains(k)) {
        ([196, 120, 92], MotifCoupe::Diagonales)
    } else if ["bois", "ossature", "osb", "clt"].iter().any(|k| n.contains(k)) {
        ([196, 160, 110], MotifCoupe::Diagonales)
    } else if ["béton", "beton", "parpaing", "bloc", "agglo", "maçon", "macon"].iter().any(|k| n.contains(k)) {
        ([176, 176, 170], MotifCoupe::Diagonales)
    } else if ["air", "lame"].iter().any(|k| n.contains(k)) {
        ([250, 250, 250], MotifCoupe::Vide)
    } else {
        // couleur stable tirée du nom, assez claire pour les hachures
        let h = identifiant("materiau", nom);
        let v = u32::from_str_radix(&h[..6], 16).unwrap_or(0);
        ([150 + (v & 0x5f) as u8, 150 + ((v >> 8) & 0x5f) as u8, 150 + ((v >> 16) & 0x5f) as u8], MotifCoupe::Diagonales)
    };
    Materiau { nom: nom.into(), couleur, motif }
}

/// Implantation d'un mur composé : ligne tracée par rapport à ses couches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reference {
    /// Milieu de l'épaisseur totale.
    Axe,
    FaceExterieure,
    FaceInterieure,
    /// Milieu de la première couche de structure (sinon milieu de l'épaisseur).
    AxePorteur,
}

impl TypeCompose {
    /// Distance, vers l'extérieur, de la ligne de référence au milieu de l'épaisseur.
    pub fn decalage_reference(&self, r: Reference) -> f64 {
        let e = self.epaisseur();
        match r {
            Reference::Axe => 0.0,
            Reference::FaceExterieure => e / 2.0,
            Reference::FaceInterieure => -e / 2.0,
            Reference::AxePorteur => {
                let mut depuis_ext = 0.0;
                for c in &self.couches {
                    if c.fonction == FonctionCouche::Structure {
                        return e / 2.0 - (depuis_ext + c.epaisseur / 2.0);
                    }
                    depuis_ext += c.epaisseur;
                }
                0.0
            }
        }
    }

    /// Décalage du milieu de chaque couche par rapport au milieu de l'épaisseur, vers
    /// l'extérieur (mm).
    pub fn decalages_couches(&self) -> Vec<f64> {
        let e = self.epaisseur();
        let mut depuis_ext = 0.0;
        self.couches
            .iter()
            .map(|c| {
                let d = e / 2.0 - depuis_ext - c.epaisseur / 2.0;
                depuis_ext += c.epaisseur;
                d
            })
            .collect()
    }
}

/// Normale unitaire vers l'extérieur (gauche) d'un axe droit ; None pour un arc ou un
/// axe nul.
pub fn normale_exterieure(axe: &Axe) -> Option<[f64; 2]> {
    let Axe::Droit { debut, fin } = *axe else { return None };
    let (dx, dy) = (fin[0] - debut[0], fin[1] - debut[1]);
    let l = dx.hypot(dy);
    (l > 1e-9).then(|| [-dy / l, dx / l])
}

/// Axe décalé de `d` le long de la normale (droit) ou du rayon (arc ; extérieur à gauche).
pub fn decaler_axe(axe: &Axe, d: f64) -> Axe {
    match *axe {
        Axe::Droit { debut, fin } => {
            let n = normale_exterieure(axe).unwrap_or([0.0, 0.0]);
            let m = |p: [f64; 2]| [p[0] + n[0] * d, p[1] + n[1] * d];
            Axe::Droit { debut: m(debut), fin: m(fin) }
        }
        Axe::Arc { debut, fin, centre, direct } => {
            // parcours direct : la gauche est vers le centre
            let r = (debut[0] - centre[0]).hypot(debut[1] - centre[1]).max(1e-9);
            let k = if direct { (r - d) / r } else { (r + d) / r };
            let m = |p: [f64; 2]| [centre[0] + (p[0] - centre[0]) * k, centre[1] + (p[1] - centre[1]) * k];
            Axe::Arc { debut: m(debut), fin: m(fin), centre, direct }
        }
    }
}

impl Projet {
    /// Type monocouche d'une couche (même matériau, épaisseur et fonction), créé s'il
    /// n'existe pas.
    pub fn type_couche(&mut self, c: &Couche) -> String {
        if let Some(t) = self.types_murs.iter().find(|t| t.couches.len() == 1 && t.couches[0] == *c) {
            return t.id.clone();
        }
        let cle = format!("{}|{}|{:?}", c.materiau.as_deref().unwrap_or(""), c.epaisseur, c.fonction);
        let id = identifiant("type-couche", &cle);
        let nom = format!("{} · {} mm", c.materiau.as_deref().unwrap_or("Couche"), crate::step::format_real(c.epaisseur));
        self.types_murs.push(TypeCompose { id: id.clone(), nom, couches: vec![c.clone()], couches_detaillees: true, origine: None });
        id
    }

    /// Matériaux des couches ajoutés à la liste s'ils manquent (couleur par défaut).
    pub fn completer_materiaux(&mut self) {
        let noms: Vec<String> = self.types_murs.iter().chain(&self.types_dalles).flat_map(|t| t.couches.iter().filter_map(|c| c.materiau.clone())).collect();
        for n in noms {
            if self.materiau(&n).is_none() {
                self.materiaux.push(materiau_par_defaut(&n));
            }
        }
    }

    /// Remplace un mur à type multicouche par un assemblage : un mur par couche, de même
    /// niveau, contraintes, phase, sous-projet et variante. L'assemblage reprend
    /// l'identifiant du mur (ses ouvertures traversent toutes les couches). Retourne
    /// false si le mur n'est pas composé.
    pub fn eclater_mur(&mut self, id: &str) -> Result<bool, String> {
        let i = self.murs.iter().position(|m| m.id == id).ok_or("Mur introuvable.")?;
        let m = self.murs[i].clone();
        let t = self.type_mur(&m.type_mur).ok_or("Type de mur introuvable.")?.clone();
        if t.couches.len() < 2 || m.assemblage.is_some() {
            return Ok(false);
        }
        if t.couches.iter().any(|c| !(c.epaisseur > 0.0 && c.epaisseur.is_finite())) {
            return Err(format!("{} : couche d'épaisseur non valide.", t.nom));
        }
        let mut couches = Vec::new();
        let mut nouveaux = Vec::new();
        for (k, (c, d)) in t.couches.iter().zip(t.decalages_couches()).enumerate() {
            let type_mur = self.type_couche(c);
            let cid = identifiant("couche", &format!("{}:{k}", m.id));
            couches.push(cid.clone());
            nouveaux.push(Mur {
                id: cid,
                nom: format!("{} · {}", m.nom, c.materiau.as_deref().map_or_else(|| format!("couche {}", k + 1), str::to_string)),
                type_mur,
                assemblage: Some(m.id.clone()),
                axe: decaler_axe(&m.axe, d),
                origine: None,
                ..m.clone()
            });
        }
        self.murs.splice(i..=i, nouveaux);
        self.assemblages.push(Assemblage { id: m.id.clone(), nom: m.nom.clone(), type_mur: t.id.clone(), couches, origine: m.origine.clone() });
        self.completer_materiaux();
        self.arrondir();
        Ok(true)
    }

    /// Sort une couche de son assemblage : elle reste un mur indépendant. Un assemblage
    /// sans couche disparaît.
    pub fn dissocier(&mut self, mur: &str) -> Result<(), String> {
        let m = self.murs.iter_mut().find(|m| m.id == mur).ok_or("Mur introuvable.")?;
        let Some(a) = m.assemblage.take() else { return Ok(()) };
        if let Some(asm) = self.assemblages.iter_mut().find(|x| x.id == a) {
            asm.couches.retain(|c| c != mur);
        }
        self.assemblages.retain(|x| !x.couches.is_empty() || self.ouvertures.iter().any(|o| o.mur == x.id));
        Ok(())
    }

    /// Rattache un mur indépendant à un assemblage, à sa place de l'extérieur vers
    /// l'intérieur (position de son axe le long de la normale de l'assemblage).
    pub fn associer(&mut self, mur: &str, assemblage: &str) -> Result<(), String> {
        let m = self.murs.iter().find(|m| m.id == mur).ok_or("Mur introuvable.")?;
        if m.assemblage.is_some() {
            return Err("Ce mur appartient déjà à un assemblage ; dissociez-le d'abord.".into());
        }
        let asm = self.assemblage(assemblage).ok_or("Assemblage introuvable.")?;
        let reference = asm.couches.iter().find_map(|c| self.murs.iter().find(|m| &m.id == c)).ok_or("Assemblage vide.")?;
        let n = normale_exterieure(&reference.axe).ok_or("Seuls les assemblages droits acceptent une couche.")?;
        let position = |axe: &Axe| match axe {
            Axe::Droit { debut, fin } => ((debut[0] + fin[0]) / 2.0) * n[0] + ((debut[1] + fin[1]) / 2.0) * n[1],
            Axe::Arc { .. } => f64::NAN,
        };
        let p = position(&m.axe);
        if !p.is_finite() {
            return Err("Seul un mur droit peut rejoindre un assemblage.".into());
        }
        // de l'extérieur (position la plus grande) vers l'intérieur
        let rang = asm.couches.iter().take_while(|c| self.murs.iter().find(|m| &m.id == *c).is_some_and(|c| position(&c.axe) > p)).count();
        let (mur, assemblage) = (mur.to_string(), assemblage.to_string());
        self.assemblages.iter_mut().find(|a| a.id == assemblage).unwrap().couches.insert(rang, mur.clone());
        self.murs.iter_mut().find(|m| m.id == mur).unwrap().assemblage = Some(assemblage);
        Ok(())
    }

    /// Murs (couches) d'un assemblage, de l'extérieur vers l'intérieur.
    pub fn couches_assemblage(&self, id: &str) -> Vec<&Mur> {
        self.assemblage(id).map_or_else(Vec::new, |a| a.couches.iter().filter_map(|c| self.murs.iter().find(|m| &m.id == c)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::edition::ParametresMur;

    fn type_trois_couches() -> TypeCompose {
        let c = |m: &str, e: f64, f| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f };
        TypeCompose {
            id: "t".into(),
            nom: "Mur isolé".into(),
            couches: vec![c("Enduit", 20.0, FonctionCouche::Finition), c("Béton", 200.0, FonctionCouche::Structure), c("Laine de verre", 100.0, FonctionCouche::Isolation)],
            couches_detaillees: true,
            origine: None,
        }
    }

    #[test]
    fn eclater_dissocier_associer() {
        let mut p = Projet::nouveau("A");
        p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur::default()).unwrap();
        p.types_murs.push(type_trois_couches());
        let id = p.murs[0].id.clone();
        p.murs[0].type_mur = "t".into();
        assert!(p.eclater_mur(&id).unwrap());
        assert_eq!(p.murs.len(), 3);
        let a = &p.assemblages[0];
        assert_eq!(a.id, id);
        // milieux des couches : extérieur (gauche, +y) 150, 40, −110
        let y: Vec<f64> = p.couches_assemblage(&id).iter().map(|m| match m.axe { Axe::Droit { debut, .. } => debut[1], _ => f64::NAN }).collect();
        assert_eq!(y, [150.0, 40.0, -110.0]);
        let e: Vec<f64> = p.couches_assemblage(&id).iter().map(|m| p.type_mur(&m.type_mur).unwrap().epaisseur()).collect();
        assert_eq!(e, [20.0, 200.0, 100.0]);
        // volume conservé
        let v: f64 = p.murs.iter().map(|m| p.volume_mur(m).unwrap()).sum();
        assert!((v - 5000.0 * 2800.0 * 320.0).abs() < 1.0);
        assert_eq!(p.materiaux.len(), 3);
        assert_eq!(p.materiau("Laine de verre").unwrap().motif, MotifCoupe::Croisees);
        // la couche d'isolant devient indépendante, puis revient à sa place
        let isolant = p.couches_assemblage(&id)[2].id.clone();
        p.dissocier(&isolant).unwrap();
        assert_eq!(p.assemblage(&id).unwrap().couches.len(), 2);
        assert!(p.murs.iter().find(|m| m.id == isolant).unwrap().assemblage.is_none());
        p.associer(&isolant, &id).unwrap();
        assert_eq!(p.assemblage(&id).unwrap().couches[2], isolant);
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(relu.assemblages[0].couches.len(), 3);
        assert_eq!(relu.murs[0].assemblage.as_deref(), Some(id.as_str()));
        assert_eq!(relu.types_murs.iter().find(|t| t.id == "t").unwrap().couches[1].fonction, FonctionCouche::Structure);
    }

    #[test]
    fn references_d_implantation() {
        let t = type_trois_couches();
        assert_eq!(t.decalage_reference(Reference::Axe), 0.0);
        assert_eq!(t.decalage_reference(Reference::FaceExterieure), 160.0);
        assert_eq!(t.decalage_reference(Reference::FaceInterieure), -160.0);
        // milieu du béton : 20 + 100 = 120 mm depuis l'extérieur, soit 40 au-dessus du milieu
        assert_eq!(t.decalage_reference(Reference::AxePorteur), 40.0);
        let arc = Axe::Arc { debut: [1000.0, 0.0], fin: [0.0, 1000.0], centre: [0.0, 0.0], direct: true };
        assert!(matches!(decaler_axe(&arc, 100.0), Axe::Arc { debut, .. } if (debut[0] - 900.0).abs() < 1e-9));
    }
}
