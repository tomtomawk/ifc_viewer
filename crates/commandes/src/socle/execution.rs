//! Exécution des commandes : copie de travail, verrous, validation, et répartition de
//! chaque commande vers son métier (`modeliser`, `modifier`, `annoter`, `documenter`).
use super::interne::*;
use crate::annoter::ExecutionAnnoter;
use crate::documenter::ExecutionDocumenter;
use crate::modeliser::ExecutionModeliser;
use crate::modifier::ExecutionModifier;
use crate::modifier::copier::ExecutionCopier;

pub(crate) fn identite(_p: &Projet, prefixe: &str) -> String {
    format!("{prefixe}-{}", noyau::uuid::uuid())
}

/// Exécution des commandes d'édition sur un projet.
pub trait Execution {
    fn actualiser_types_murs_modifies(&mut self, avant: &Projet) -> Result<(), String>;
    fn identifiants(&self) -> HashSet<String>;
    fn executer(&self, c: Commande) -> Result<(Projet, Vec<String>), String>;
    fn dependances(&self, ids: &[String]) -> Result<HashSet<String>, String>;
}

/// Étapes internes de l'exécution d'une commande.
pub(crate) trait ExecutionInterne {
    fn dependances_avec(&self, ids: &[String], couche_seule: bool) -> Result<HashSet<String>, String>;
    fn executer_interne(&mut self, c: Commande) -> Result<Vec<String>, String>;
}

impl Execution for Projet {
    /// Répercute une modification de composition sur tous les murs du type, en gardant
    /// leur ligne d'implantation et l'identité de chaque assemblage. L'opération entière
    /// est annulée si une des reconstructions échoue.
    fn actualiser_types_murs_modifies(&mut self, avant: &Projet) -> Result<(), String> {
        let types: std::collections::HashSet<String> = self.types_murs.iter().filter(|t| {
            avant.type_mur(&t.id).is_some_and(|ancien| ancien.couches != t.couches)
        }).map(|t| t.id.clone()).collect();
        if types.is_empty() { return Ok(()); }
        let mut travail = self.clone();
        let assemblages: Vec<String> = travail.assemblages.iter().filter(|a| types.contains(&a.type_mur) && avant.assemblage(&a.id).is_some_and(|ancien| ancien.type_mur == a.type_mur)).map(|a| a.id.clone()).collect();
        for id in assemblages {
            let (axe, _) = avant.trajectoire_justification(&id).ok_or("Ligne d'implantation du mur introuvable.")?;
            let assemblage = travail.assemblage(&id).ok_or("Assemblage absent.")?;
            let type_mur = travail.type_mur(&assemblage.type_mur).ok_or("Type absent.")?;
            let reference = travail.implantations.get(&id).copied().unwrap_or(modele::murs::assemblage::Reference::Axe);
            let decalage = type_mur.decalages_couches().first().copied().unwrap_or(0.0) - type_mur.decalage_reference(reference);
            let premiere = assemblage.couches.first().cloned().ok_or("Assemblage vide.")?;
            travail.murs.iter_mut().find(|m| m.id == premiere).ok_or("Couche introuvable.")?.axe = assemblage::decaler_axe(&axe, decalage);
            travail.executer_interne(Commande::ActualiserType { assemblage: id })?;
        }
        let murs: Vec<String> = travail.murs.iter().filter(|m| m.assemblage.is_none() && types.contains(&m.type_mur) && avant.murs.iter().any(|a| a.id == m.id && a.type_mur == m.type_mur)).map(|m| m.id.clone()).collect();
        for id in murs {
            let (axe, _) = avant.trajectoire_justification(&id).ok_or("Ligne d'implantation du mur introuvable.")?;
            let mur = travail.murs.iter().find(|m| m.id == id).ok_or("Mur introuvable.")?;
            let type_mur = travail.type_mur(&mur.type_mur).ok_or("Type absent.")?;
            let reference = travail.implantations.get(&id).copied().unwrap_or(modele::murs::assemblage::Reference::Axe);
            let axe = assemblage::decaler_axe(&axe, -type_mur.decalage_reference(reference));
            travail.murs.iter_mut().find(|m| m.id == id).unwrap().axe = axe;
            travail.eclater_mur(&id)?;
        }
        *self = travail;
        Ok(())
    }
    fn identifiants(&self) -> HashSet<String> {
        self.niveaux
            .iter()
            .map(|x| x.id.clone())
            .chain(self.murs.iter().map(|x| x.id.clone()))
            .chain(self.assemblages.iter().map(|x| x.id.clone()))
            .chain(self.dalles.iter().map(|x| x.id.clone()))
            .chain(self.terrains.iter().map(|x| x.id.clone()))
            .chain(self.limites_propriete.iter().map(|x| x.id.clone()))
            .chain(self.barres.iter().map(|x| x.id.clone()))
            .chain(self.pieces.iter().map(|x| x.id.clone()))
            .chain(self.composants.iter().map(|x| x.id.clone()))
            .chain(self.arbres.iter().map(|x| x.id.clone()))
            .chain(self.escaliers.iter().map(|x| x.id.clone()))
            .chain(self.garde_corps.iter().map(|x| x.id.clone()))
            .chain(self.ouvertures.iter().map(|x| x.id.clone()))
            .chain(self.importes.iter().map(|x| x.id.clone()))
            .chain(self.liens.iter().map(|x| x.id.clone()))
            .chain(self.vues.iter().map(|x| x.id.clone()))
            .chain(self.quadrillages.iter().map(|x| x.id.clone()))
            .chain(self.annotations.iter().map(|x| x.id.clone()))
            .chain(self.feuilles.iter().map(|x| x.id.clone()))
            .chain(self.nomenclatures.iter().map(|x| x.id.clone()))
            .collect()
    }
    /// Copie de travail : une erreur ne laisse jamais de modification partielle.
    fn executer(&self, c: Commande) -> Result<(Projet, Vec<String>), String> {
        let mut p = self.clone();
        let c = self.filtrer_verrouilles(c)?;
        let ids = p.executer_interne(c)?;
        super::verrous::verifier(self, &p)?;
        p.detacher_couches_disjointes(self);
        p.recaler_menuiseries(self)?;
        p.actualiser_pieces();
        p.verifier()?;
        Ok((p, ids))
    }
    fn dependances(&self, ids: &[String]) -> Result<HashSet<String>, String> {
        self.dependances_avec(ids, false)
    }
}

impl ExecutionInterne for Projet {
    /// `couche_seule` : une couche d'assemblage peut être transformée seule (déplacer,
    /// tourner) ; elle se détache ensuite si elle quitte le mur composé.
    fn dependances_avec(&self, ids: &[String], couche_seule: bool) -> Result<HashSet<String>, String> {
        let connus = self.identifiants();
        if ids.is_empty() || ids.iter().any(|id| !connus.contains(id)) {
            return Err("Sélection vide ou identifiant absent.".into());
        }
        let mut out: HashSet<String> = ids.iter().cloned().collect();
        for a in &self.assemblages {
            if out.contains(&a.id) {
                out.extend(a.couches.clone());
            }
        }
        for m in &self.murs {
            if !couche_seule && out.contains(&m.id) && m.assemblage.as_ref().is_some_and(|a| !out.contains(a)) {
                return Err("Sélectionnez l'assemblage entier ou dissociez la couche avant cette opération.".into());
            }
        }
        for o in &self.ouvertures {
            if out.contains(&o.mur) {
                out.insert(o.id.clone());
            }
        }
        // garde-corps hébergés : suivent leur escalier
        for g in &self.garde_corps {
            if g.escalier.as_ref().is_some_and(|e| out.contains(e)) {
                out.insert(g.id.clone());
            }
        }
        Ok(out)
    }
    fn executer_interne(&mut self, c: Commande) -> Result<Vec<String>, String> {
        match c {
            Commande::Lien { .. }
            | Commande::Terrain { .. }
            | Commande::LimitePropriete { .. }
            | Commande::CoordonneesOrigine { .. }
            | Commande::Escalier { .. }
            | Commande::GardeCorps { .. }
            | Commande::Piece { .. }
            | Commande::TypeComposant { .. }
            | Commande::Arbre { .. }
            | Commande::Composant { .. }
            | Commande::Barre { .. }
            | Commande::Ouverture { .. }
            | Commande::ActualiserType { .. }
            | Commande::Contour { .. }
            | Commande::Tremie { .. } => self.executer_modeliser(c),
            Commande::Attacher { .. }
            | Commande::PrioriteJonction { .. }
            | Commande::Aligner { .. }
            | Commande::InverserMur { .. }
            | Commande::ModifierTrajectoireMur { .. }
            | Commande::AjusterAngle { .. }
            | Commande::Ajuster { .. }
            | Commande::Deplacer { .. }
            | Commande::Tourner { .. }
            | Commande::Symetrie { .. }
            | Commande::Echelle { .. }
            | Commande::Reseau { .. }
            | Commande::Decaler { .. }
            | Commande::Scinder { .. }
            | Commande::Demolir { .. }
            | Commande::Reproduire { .. }
            | Commande::ChangerHote { .. }
            | Commande::InverserOuverture { .. }
            | Commande::Supprimer { .. } => self.executer_modifier(c),
            Commande::CopierSurNiveau { .. }
            | Commande::Copier { .. } => self.executer_copier(c),
            Commande::ModifierQuadrillage { .. }
            | Commande::Quadrillage { .. }
            | Commande::Annoter { .. }
            | Commande::Niveau { .. }
            | Commande::ModifierNiveau { .. } => self.executer_annoter(c),
            Commande::ModifierVue { .. }
            | Commande::Feuille { .. }
            | Commande::Nomenclature { .. } => self.executer_documenter(c),
            Commande::Verrouiller { ids, verrou } => {
                let connus = self.identifiants();
                for id in &ids {
                    if !connus.contains(id) {
                        return Err(format!("Élément introuvable : {id}"));
                    }
                    if verrou {
                        self.verrous.insert(id.clone(), true);
                    } else {
                        self.verrous.remove(id);
                    }
                }
                Ok(ids)
            }
        }
    }
}
