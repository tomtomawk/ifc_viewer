//! Assemblages de murs : un mur composé dont chaque couche est un mur indépendant.
//! Les couches vont de l'extérieur vers l'intérieur ; l'extérieur est à gauche de l'axe
//! parcouru du début vers la fin (normale (−dy, dx)), comme un mur Revit tracé dans le
//! sens horaire.
use super::*;

/// Matériaux reconnus par leur nom (familles du gabarit) : couleur et motif de coupe.
pub fn materiau_par_defaut(nom: &str) -> Materiau {
    crate::gabarit::actif().materiau(nom)
}

/// Implantation d'un mur composé : ligne tracée par rapport à ses couches.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
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
    /// Ligne d'implantation (justification) choisie pour un mur ou un assemblage droit :
    /// extrémités (mm) et distance, vers l'extérieur, qui la sépare de l'axe médian.
    /// Sans implantation enregistrée, c'est l'axe médian.
    pub fn ligne_justification(&self, id: &str) -> Option<([f64; 2], [f64; 2], f64)> {
        let (premier, type_mur, rang) = match self.assemblage(id) {
            Some(a) => (*self.couches_assemblage(id).first()?, self.type_mur(&a.type_mur)?, 0),
            None => {
                let m = self.murs.iter().find(|m| m.id == id)?;
                (m, self.type_mur(&m.type_mur)?, 0)
            }
        };
        let Axe::Droit { debut, fin } = premier.axe else { return None };
        let n = normale_exterieure(&premier.axe)?;
        // couche d'un assemblage : son axe est décalé du milieu de l'épaisseur totale
        let depart = if self.assemblage(id).is_some() { type_mur.decalages_couches().get(rang).copied().unwrap_or(0.0) } else { 0.0 };
        let reference = self.implantations.get(id).copied().unwrap_or(Reference::Axe);
        let d = type_mur.decalage_reference(reference);
        let k = d - depart;
        let m = |p: [f64; 2]| [p[0] + n[0] * k, p[1] + n[1] * k];
        Some((m(debut), m(fin), d))
    }

    /// Type monocouche d'une couche (même matériau, épaisseur et fonction), créé s'il
    /// n'existe pas.
    pub fn type_couche(&mut self, c: &Couche) -> String {
        if let Some(t) = self.types_murs.iter().find(|t| t.couches.len() == 1 && t.couches[0] == *c) {
            return t.id.clone();
        }
        let mut cle = format!("{}|{}|{:?}", c.materiau.as_deref().unwrap_or(""), c.epaisseur, c.fonction);
        // plume choisie : type distinct (sans elle, la clé des projets existants ne change pas)
        if let Some(n) = c.plume {
            cle.push_str(&format!("|plume {n}"));
        }
        let id = identifiant("type-couche", &cle);
        let nom = format!("{} · {} mm", c.materiau.as_deref().unwrap_or("Couche"), crate::step::format_real(c.epaisseur));
        self.types_murs.push(TypeCompose { id: id.clone(), nom, couches: vec![c.clone()], categorie: None, couches_detaillees: true, origine: None });
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
        if self.murs.iter().find(|m|m.id==mur).and_then(|m|m.assemblage.as_ref()).and_then(|a|self.assemblage(a)).is_some_and(|a|a.couches.len()==1 && self.ouvertures.iter().any(|o|o.mur==a.id)) {
            return Err("La dernière couche héberge des ouvertures : conservez l'assemblage ou supprimez les ouvertures.".into());
        }
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
        let c = |m: &str, e: f64, f| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f, plume: None };
        TypeCompose {
            id: "t".into(),
            nom: "Mur isolé".into(),
            couches: vec![c("Enduit", 20.0, FonctionCouche::Finition), c("Béton", 200.0, FonctionCouche::Structure), c("Laine de verre", 100.0, FonctionCouche::Isolation)],
            categorie: None, couches_detaillees: true,
            origine: None,
        }
    }

    #[test]
    fn extremite_d_assemblage_deplacee() {
        let mut p = Projet::nouveau("A");
        p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur::default()).unwrap();
        p.types_murs.push(type_trois_couches());
        let id = p.murs[0].id.clone();
        p.murs[0].type_mur = "t".into();
        p.eclater_mur(&id).unwrap();
        let (a, b, _) = p.axe_element(&id).unwrap();
        // axe moyen des couches : y = (150 + 40 − 110) / 3
        assert!((a[1] - 80.0 / 3.0).abs() < 1e-6 && (b[0] - 5000.0).abs() < 1e-6);
        // fin tournée de 90° : les couches gardent leur écart à l'axe
        p.deplacer_extremite(&id, true, [a[0], a[1] + 4000.0]).unwrap();
        let x: Vec<f64> = p.couches_assemblage(&id).iter().map(|m| match m.axe { Axe::Droit { fin, .. } => fin[0], _ => f64::NAN }).collect();
        let ecarts: Vec<f64> = x.iter().map(|x| (x - a[0]).round()).collect();
        assert_eq!(ecarts, [-(150.0 - 80.0 / 3.0f64).round(), -(40.0 - 80.0 / 3.0f64).round(), (110.0 + 80.0 / 3.0f64).round()]);
        assert!(p.deplacer_extremite(&id, true, a).is_err());
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
        assert_eq!(p.materiau("Laine de verre").unwrap().motif, MotifCoupe::Isolation);
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
