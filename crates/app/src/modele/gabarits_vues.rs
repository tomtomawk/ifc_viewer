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
