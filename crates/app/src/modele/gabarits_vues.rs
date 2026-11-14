//! Gabarits de vue partagés par les vues natives.
use crate::modele::Projet;
use serde::{Deserialize, Serialize};

/// Paramètres pilotés par un gabarit. Le cadrage, l'orientation et les objets masqués
/// restent propres à chaque vue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GabaritVue {
    pub id: String,
    pub nom: String,
    pub echelle: f64,
    pub poche: String,
    pub phase: Option<String>,
    pub variante: Option<String>,
}

impl Projet {
    pub fn gabarit_vue(&self, id_vue: &str) -> Option<&GabaritVue> {
        let id = self.gabarits_vue_affectes.get(id_vue)?;
        self.gabarits_vues.iter().find(|g| &g.id == id)
    }

    pub(crate) fn valider_gabarit_vue(&self, g: &GabaritVue) -> Result<(), String> {
        if g.nom.trim().is_empty() { return Err("Donnez un nom au gabarit de vue.".into()); }
        if self.gabarits_vues.iter().any(|v| v.id != g.id && v.nom.eq_ignore_ascii_case(g.nom.trim())) { return Err("Un gabarit de vue porte déjà ce nom.".into()); }
        if !(g.echelle.is_finite() && (1.0..=10000.0).contains(&g.echelle)) { return Err("L'échelle doit être comprise entre 1 et 10 000.".into()); }
        if !matches!(g.poche.as_str(), "hachure" | "plein" | "vide" | "materiau") { return Err("Poché de coupe non valide.".into()); }
        if g.phase.as_ref().is_some_and(|p| !self.phases.contains(p)) { return Err("Phase du gabarit introuvable.".into()); }
        if g.variante.as_ref().is_some_and(|id| !self.variantes.iter().any(|v| &v.id == id)) { return Err("Variante du gabarit introuvable.".into()); }
        Ok(())
    }

    fn appliquer_gabarit_vue(&mut self, id_vue: &str, g: &GabaritVue) -> Result<(), String> {
        if id_vue == "3d" {
            self.echelle_vue_3d = Some(g.echelle);
            self.poche_vue_3d = Some(g.poche.clone());
        } else {
            let vue = self.vues.iter_mut().find(|v| v.id == id_vue).ok_or("Vue introuvable.")?;
            vue.echelle = g.echelle;
            vue.poche_vue = Some(g.poche.clone());
        }
        let contexte = self.contextes_vues.entry(id_vue.into()).or_default();
        contexte.phase = g.phase.clone();
        contexte.variante = g.variante.clone();
        Ok(())
    }

    /// Crée ou modifie un gabarit et actualise toutes ses vues en une opération.
    pub fn enregistrer_gabarit_vue(&mut self, mut g: GabaritVue) -> Result<String, String> {
        g.nom = g.nom.trim().into();
        if g.id.is_empty() { g.id = noyau::uuid::uuid(); }
        self.valider_gabarit_vue(&g)?;
        let mut copie = self.clone();
        if let Some(existant) = copie.gabarits_vues.iter_mut().find(|x| x.id == g.id) { *existant = g.clone(); }
        else { copie.gabarits_vues.push(g.clone()); }
        let vues: Vec<String> = copie.gabarits_vue_affectes.iter().filter(|(_, id)| *id == &g.id).map(|(vue, _)| vue.clone()).collect();
        for vue in vues { copie.appliquer_gabarit_vue(&vue, &g)?; }
        *self = copie;
        Ok(g.id)
    }

    /// Retirer un gabarit garde les dernières valeurs appliquées à la vue.
    pub fn affecter_gabarit_vue(&mut self, vue: &str, gabarit: Option<&str>) -> Result<(), String> {
        if vue != "3d" && !self.vues.iter().any(|v| v.id == vue) { return Err("Vue introuvable.".into()); }
        let mut copie = self.clone();
        if let Some(id) = gabarit {
            let g = copie.gabarits_vues.iter().find(|g| g.id == id).cloned().ok_or("Gabarit introuvable.")?;
            copie.valider_gabarit_vue(&g)?;
            copie.appliquer_gabarit_vue(vue, &g)?;
            copie.gabarits_vue_affectes.insert(vue.into(), id.into());
        } else { copie.gabarits_vue_affectes.remove(vue); }
        *self = copie; Ok(())
    }

    /// Les vues détachées conservent leurs réglages courants.
    pub fn supprimer_gabarit_vue(&mut self, id: &str) -> Result<(), String> {
        if !self.gabarits_vues.iter().any(|g| g.id == id) { return Err("Gabarit introuvable.".into()); }
        self.gabarits_vues.retain(|g| g.id != id);
        self.gabarits_vue_affectes.retain(|_, g| g != id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::creation::ListeCreation;

    #[test]
    fn gabarit_partage_actualise_detache_et_se_recharge() {
        let mut p = Projet::depuis_gabarit("Gabarits", &crate::modele::gabarit::Gabarit::defaut());
        let niveau = p.niveaux[0].id.clone();
        let a = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
        let b = p.ajouter_vue_plan(&niveau, 1300.0).unwrap();
        let c = p.ajouter_vue_plan(&niveau, 1400.0).unwrap();
        let variante = p.ajouter_choix_creation(ListeCreation::Variante, "Option A", "Façade").unwrap();
        let g = GabaritVue { id: String::new(), nom: "Plan projet".into(), echelle: 50.0, poche: "plein".into(), phase: Some("Projet".into()), variante: Some(variante.clone()) };
        let id = p.enregistrer_gabarit_vue(g).unwrap();
        p.affecter_gabarit_vue(&a, Some(&id)).unwrap();
        p.affecter_gabarit_vue(&b, Some(&id)).unwrap();
        p.affecter_gabarit_vue("3d", Some(&id)).unwrap();
        assert_eq!(p.vues.iter().find(|v| v.id == a).unwrap().echelle, 50.0);
        assert_eq!(p.contextes_vues[&b].variante.as_deref(), Some(variante.as_str()));
        assert_eq!(p.contextes_vues["3d"].phase.as_deref(), Some("Projet"));
        assert_eq!(p.echelle_vue_3d, Some(50.0));
        let mut g = p.gabarits_vues[0].clone();
        g.echelle = 200.0; g.poche = "vide".into(); g.phase = Some("Existant".into());
        p.enregistrer_gabarit_vue(g.clone()).unwrap();
        for id_vue in [&a, &b] {
            let vue = p.vues.iter().find(|v| &v.id == id_vue).unwrap();
            assert_eq!(vue.echelle, 200.0);
            assert_eq!(vue.poche_vue.as_deref(), Some("vide"));
            assert_eq!(p.contextes_vues[id_vue].phase.as_deref(), Some("Existant"));
        }
        assert_eq!(p.vues.iter().find(|v| v.id == c).unwrap().echelle, 100.0);
        assert_eq!(p.echelle_vue_3d, Some(200.0));
        p.affecter_gabarit_vue(&a, None).unwrap();
        g.echelle = 75.0;
        p.enregistrer_gabarit_vue(g).unwrap();
        assert_eq!(p.vues.iter().find(|v| v.id == a).unwrap().echelle, 200.0);
        assert_eq!(p.vues.iter().find(|v| v.id == b).unwrap().echelle, 75.0);
        p.verifier().unwrap();
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(relu.gabarit_vue(&b).unwrap().echelle, 75.0);
        assert!(relu.gabarit_vue(&a).is_none());
        use crate::modele::collaboration;
        let objets = collaboration::objets(&p).unwrap();
        let partage = collaboration::projet(&objets).unwrap();
        assert_eq!(partage.gabarit_vue(&b).unwrap(), p.gabarit_vue(&b).unwrap());
        assert_eq!(partage.contextes_vues[&b], p.contextes_vues[&b]);
        partage.verifier().unwrap();
        p.supprimer_gabarit_vue(&id).unwrap();
        assert!(p.gabarit_vue(&b).is_none());
        assert_eq!(p.vues.iter().find(|v| v.id == b).unwrap().echelle, 75.0);
        assert_eq!(p.echelle_vue_3d, Some(75.0));
        p.verifier().unwrap();
    }

    #[test]
    fn invalides_et_modifications_de_phases_restent_coherentes() {
        let mut p = Projet::depuis_gabarit("Gabarits", &crate::modele::gabarit::Gabarit::defaut());
        let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0).unwrap();
        let v = p.ajouter_choix_creation(ListeCreation::Variante, "A", "Jeu").unwrap();
        let id = p.enregistrer_gabarit_vue(GabaritVue { id: String::new(), nom: "Test".into(), echelle: 100.0, poche: "hachure".into(), phase: Some("Projet".into()), variante: Some(v.clone()) }).unwrap();
        p.affecter_gabarit_vue(&vue, Some(&id)).unwrap();
        p.renommer_contexte(ListeCreation::Phase, "Projet", "Travaux", "").unwrap();
        assert_eq!(p.gabarit_vue(&vue).unwrap().phase.as_deref(), Some("Travaux"));
        assert_eq!(p.contextes_vues[&vue].phase.as_deref(), Some("Travaux"));
        p.supprimer_contexte(ListeCreation::Variante, &v, None).unwrap();
        assert_eq!(p.gabarit_vue(&vue).unwrap().variante, None);
        assert_eq!(p.contextes_vues[&vue].variante, None);
        let avant = p.json().unwrap();
        let mut invalide = p.gabarit_vue(&vue).unwrap().clone();
        invalide.echelle = -1.0;
        assert!(p.enregistrer_gabarit_vue(invalide).is_err());
        assert!(p.affecter_gabarit_vue("vue absente", Some(&id)).is_err());
        assert_eq!(p.json().unwrap(), avant);
    }
}
