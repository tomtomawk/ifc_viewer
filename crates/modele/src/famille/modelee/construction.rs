//! Cycle de vie des plans de construction : renommage, suppression refusée tant que des
//! formes y sont esquissées, détachement en repère fixe équivalent.
//!
//! Les formes désignent leur plan par son identité : un renommage ne les touche pas, et
//! un plan qui suit un plan de référence (`reference`) entraîne ses formes quand ce plan
//! se déplace (`FamilleModelee::repere_construction`).
use super::{FamilleModelee, PlanConstruction, Resolution};

impl FamilleModelee {
    /// Noms des formes esquissées sur le plan de construction `id`.
    pub fn formes_sur_construction(&self, id: &str) -> Vec<String> {
        self.extrusions.iter().filter(|e| e.construction.as_deref() == Some(id)).map(|e| e.nom.clone()).collect()
    }

    fn indice_construction(&self, id: &str) -> Result<usize, String> {
        self.plans_construction.iter().position(|p| p.id == id).ok_or_else(|| "Plan de construction introuvable.".to_owned())
    }

    /// Renomme un plan de construction ; le nom reste unique parmi les plans de construction.
    pub fn renommer_plan_construction(&mut self, id: &str, nom: &str) -> Result<(), String> {
        let nom = nom.trim();
        let k = self.indice_construction(id)?;
        if nom.is_empty() || self.plans_construction.iter().any(|p| p.id != id && p.nom == nom) {
            return Err(format!("Nom de plan de construction vide ou déjà pris : « {nom} »."));
        }
        self.plans_construction[k].nom = nom.to_owned();
        Ok(())
    }

    /// Supprime un plan de construction ; refusé, avec la liste des formes, s'il porte
    /// encore des formes (les détacher, ou supprimer ces formes d'abord).
    pub fn supprimer_plan_construction(&mut self, id: &str) -> Result<PlanConstruction, String> {
        let k = self.indice_construction(id)?;
        let formes = self.formes_sur_construction(id);
        if !formes.is_empty() {
            return Err(format!("« {} » porte {} forme(s) : {}. Détacher le plan ou supprimer ces formes d'abord.", self.plans_construction[k].nom, formes.len(), formes.join(", ")));
        }
        Ok(self.plans_construction.remove(k))
    }

    /// Détache un plan de construction de son plan de référence : il devient un repère
    /// fixe à sa position courante (résolue par `r`), si bien que ses formes ne bougent
    /// pas et que le plan de référence peut être supprimé. Un plan déjà fixe sans forme
    /// est supprimé ; avec des formes, il est déjà détaché. Rend le nom final du plan, ou
    /// `None` s'il a été supprimé.
    pub fn detacher_plan_construction(&mut self, id: &str, r: &Resolution) -> Result<Option<String>, String> {
        let k = self.indice_construction(id)?;
        if self.plans_construction[k].reference.is_none() {
            if self.formes_sur_construction(id).is_empty() {
                self.plans_construction.remove(k);
                return Ok(None);
            }
            return Err(format!("« {} » est déjà un repère fixe.", self.plans_construction[k].nom));
        }
        let mut fixe = self.repere_construction(id, r).ok_or("Plan de référence du plan de construction introuvable.")?;
        fixe.reference = None;
        let base = format!("{} (fixe)", fixe.nom);
        fixe.nom = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base} {n}") })
            .find(|nom| self.plans_construction.iter().all(|p| p.id == id || &p.nom != nom)).expect("nom libre");
        fixe.valider()?;
        let nom = fixe.nom.clone();
        self.plans_construction[k] = fixe;
        Ok(Some(nom))
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    const DIMENSIONS: (f64, f64, f64) = (1200.0, 1350.0, 300.0);

    /// Fenêtre avec un plan libre « Ref » (axe Z) et un plan de construction qui le suit,
    /// portant une extrusion.
    fn famille() -> (FamilleModelee, String) {
        let mut f = FamilleModelee::gabarit_fenetre();
        f.plans.push(PlanRef { nom: "Ref".into(), axe: AxePlan::Z, position: 400.0, fixe: false, hote: None });
        let plan = PlanConstruction { id: "c1".into(), reference: Some("Ref".into()), nom: "Référence : Ref".into(), origine: [0.0; 3], horizontal: [1.0, 0.0, 0.0], vertical: [0.0, 1.0, 0.0] };
        let face = |x| Face { plan: String::new(), decalage: x };
        f.extrusions.push(FormeProfil { visibilite: Default::default(),
            operation: OperationForme::Extrusion, construction: Some(plan.id.clone()), nom: "Tablette".into(), vide: false,
            materiau: Materiau::Menuiserie, vue: VueEsquisse::Plan,
            profil: [[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [0.0, 50.0]].into_iter().map(|q| PointEsquisse { h: face(q[0]), v: face(q[1]) }).collect(),
            profondeur: [face(0.0), face(20.0)], visible: None, materiau_parametre: None,
        });
        f.plans_construction.push(plan);
        (f, "c1".into())
    }

    fn origine(f: &FamilleModelee, id: &str) -> [f64; 3] {
        let (l, h, e) = DIMENSIONS;
        f.repere_construction(id, &f.resoudre(l, h, e)).unwrap().origine
    }

    #[test]
    fn suppression_refusee_avec_la_liste_des_formes() {
        let (mut f, id) = famille();
        let e = f.supprimer_plan_construction(&id).unwrap_err();
        assert!(e.contains("Tablette") && e.contains("1 forme"), "{e}");
        assert_eq!(f.plans_construction.len(), 1);
        f.extrusions.clear();
        assert_eq!(f.supprimer_plan_construction(&id).unwrap().id, id);
        assert!(f.plans_construction.is_empty());
        assert!(f.supprimer_plan_construction(&id).is_err());
    }

    #[test]
    fn renommage_et_deplacement_suivis_par_les_formes() {
        let (mut f, id) = famille();
        f.renommer_plan_construction(&id, "Tablette basse").unwrap();
        assert_eq!(f.extrusions.last().unwrap().construction.as_deref(), Some(id.as_str()));
        assert!(f.renommer_plan_construction(&id, "  ").is_err());
        let (l, h, e) = DIMENSIONS;
        f.valider(l, h, e).unwrap();
        // le plan de référence se déplace : le repère (donc la forme) le suit
        assert_eq!(origine(&f, &id)[2], 400.0);
        f.plans.iter_mut().find(|p| p.nom == "Ref").unwrap().position = 650.0;
        assert_eq!(origine(&f, &id)[2], 650.0);
    }

    #[test]
    fn detachement_garde_la_position_et_libere_le_plan_de_reference() {
        let (mut f, id) = famille();
        let (l, h, e) = DIMENSIONS;
        let avant = f.solides(&f.resoudre(l, h, e));
        let nom = f.detacher_plan_construction(&id, &f.resoudre(l, h, e)).unwrap();
        assert_eq!(nom.as_deref(), Some("Référence : Ref (fixe)"));
        let plan = &f.plans_construction[0];
        assert_eq!((plan.reference.as_ref(), plan.origine[2]), (None, 400.0));
        f.valider(l, h, e).unwrap();
        let tris = |s: &[_]| s.iter().map(|x: &SolideFamille| x.tris.clone()).collect::<Vec<_>>();
        assert_eq!(tris(&f.solides(&f.resoudre(l, h, e))), tris(&avant));
        // le plan de référence ne porte plus rien : il peut bouger sans entraîner la forme
        f.plans.iter_mut().find(|p| p.nom == "Ref").unwrap().position = 900.0;
        assert_eq!(origine(&f, &id)[2], 400.0);
        // déjà fixe : refus tant que des formes y sont ; sans forme, le plan disparaît
        assert!(f.detacher_plan_construction(&id, &f.resoudre(l, h, e)).is_err());
        f.extrusions.clear();
        assert_eq!(f.detacher_plan_construction(&id, &f.resoudre(l, h, e)).unwrap(), None);
        assert!(f.plans_construction.is_empty());
    }
}
