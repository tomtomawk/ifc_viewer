//! Filtres de phase, de sous-projet et de variante d'une vue, et éléments qu'ils masquent.
use crate::modele::Projet;

/// Filtres temporaires de la vue, indépendants des affectations enregistrées.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FiltresContextes {
    pub phase: Option<String>,
    pub sous_projet: Option<String>,
    pub variante: Option<String>,
}

impl FiltresContextes {
    /// Filtre résultant : les choix de `self` priment, `base` complète le reste.
    pub fn sur(&self, base: &FiltresContextes) -> FiltresContextes {
        FiltresContextes {
            phase: self.phase.clone().or_else(|| base.phase.clone()),
            sous_projet: self.sous_projet.clone().or_else(|| base.sous_projet.clone()),
            variante: self.variante.clone().or_else(|| base.variante.clone()),
        }
    }
}

impl Projet {
    /// Calcule les éléments masqués en une passe, sans recherche linéaire par objet.
    pub fn elements_masques_contextes(&self, filtre: &FiltresContextes) -> std::collections::HashSet<String> {
        use std::collections::{HashMap, HashSet};
        type Champs<'a> = (Option<&'a str>, Option<&'a str>, Option<&'a str>);
        let mut champs: HashMap<&str, Champs<'_>> = self.contextes.iter().map(|(id, c)| (id.as_str(), (c.phase.as_deref(), c.sous_projet.as_deref(), c.variante.as_deref()))).collect();
        for m in &self.murs {
            let ancien = champs.get(m.id.as_str()).copied().unwrap_or((None,None,None));
            champs.insert(&m.id, (m.phase.as_deref().or(ancien.0), m.sous_projet.as_deref().or(ancien.1), m.variante.as_deref().or(ancien.2)));
        }
        for d in &self.dalles {
            let ancien = champs.get(d.id.as_str()).copied().unwrap_or((None,None,None));
            champs.insert(&d.id, (d.phase.as_deref().or(ancien.0), d.sous_projet.as_deref().or(ancien.1), d.variante.as_deref().or(ancien.2)));
        }
        for a in &self.assemblages {
            if let Some(c) = a.couches.first().and_then(|id| champs.get(id.as_str())).copied() { champs.insert(&a.id, c); }
        }
        for o in &self.ouvertures {
            if let Some(c) = champs.get(o.mur.as_str()).copied() { champs.insert(&o.id, c); }
        }
        let parents: HashMap<&str, &str> = self.murs.iter().filter_map(|m| m.assemblage.as_deref().map(|a| (m.id.as_str(), a)))
            .chain(self.ouvertures.iter().map(|o| (o.id.as_str(), o.mur.as_str()))).collect();
        let rangs: HashMap<&str, usize> = self.phases.iter().enumerate().map(|(i, p)| (p.as_str(), i)).collect();
        let active = filtre.phase.as_deref().and_then(|p| rangs.get(p)).copied();
        let jeux: HashMap<&str, &str> = self.variantes.iter().map(|v| (v.id.as_str(), v.jeu.as_str())).collect();
        // une seule variante visible par jeu : celle du filtre, sinon la retenue
        let mut affichees: HashMap<&str, &str> = HashMap::new();
        for v in &self.variantes {
            let courante = affichees.entry(v.jeu.as_str()).or_insert(v.id.as_str());
            if v.retenue && !self.variantes.iter().any(|x| x.id == *courante && x.retenue) { *courante = v.id.as_str(); }
        }
        if let Some((id, jeu)) = filtre.variante.as_deref().and_then(|id| jeux.get_key_value(id)) { affichees.insert(*jeu, *id); }
        let mut masques = HashSet::new();
        for (id, (phase, sous_projet, option)) in champs {
            let demolition = self.demolitions.get(id).or_else(|| parents.get(id).and_then(|p| self.demolitions.get(*p)))
                .or_else(|| parents.get(id).and_then(|p| parents.get(p)).and_then(|p| self.demolitions.get(*p)));
            let hors_phase = active.is_some_and(|a| phase.and_then(|p| rangs.get(p)).is_some_and(|&c| c > a) || demolition.and_then(|d| rangs.get(d.as_str())).is_some_and(|&d| d <= a));
            let hors_sous_projet = filtre.sous_projet.as_deref().zip(sous_projet).is_some_and(|(a,b)| a != b);
            let hors_variante = option.is_some_and(|b| jeux.get(b).and_then(|jeu| affichees.get(jeu)).is_some_and(|a| *a != b));
            if hors_phase || hors_sous_projet || hors_variante { masques.insert(id.to_string()); }
        }
        masques
    }
}
