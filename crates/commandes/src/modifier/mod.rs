//! Modifier : déplacer, tourner, aligner, ajuster, attacher, supprimer ; copies dans
//! `copier`, commandes composées (symétrie, échelle, réseau, décaler, scinder, démolir,
//! reproduire) dans `composees`, transformation affine dans `transformations`.
pub mod alignement;
pub mod composees;
pub mod copier;
pub mod presse_papiers;
mod scinder_surface;
pub mod supprimer;
pub mod transformations;
use crate::socle::interne::*;

/// Exécution des commandes de modification.
pub(crate) trait ExecutionModifier {
    fn executer_modifier(&mut self, c: Commande) -> Result<Vec<String>, String>;
}

impl ExecutionModifier for Projet {
    fn executer_modifier(&mut self, c: Commande) -> Result<Vec<String>, String> {
        match c {
            Commande::Attacher { ids, toiture } => {
                for id in &ids {
                    if !self.murs.iter().any(|m| m.id == *id) && self.assemblage(id).is_none() {
                        return Err("Sélectionnez des murs ou assemblages.".into());
                    }
                    if let Some(t) = &toiture {
                        self.attaches_toitures.entry(id.clone()).or_default().insert(t.clone());
                    } else {
                        self.attaches_toitures.remove(id);
                    }
                }
                Ok(ids)
            }
            Commande::PrioriteJonction { id, priorite } => {
                if !self.murs.iter().any(|m| m.id == id) {
                    return Err("Sélectionnez une couche ou un mur indépendant.".into());
                }
                self.priorites_jonction.insert(id.clone(), priorite);
                Ok(vec![id])
            }
            Commande::Aligner { ids, reference } => {
                let (a, b, _) = self
                    .axe_element(&reference)
                    .ok_or("Référence : mur droit requis.")?;
                let a = DVec2::from_array(a);
                let u = (DVec2::from_array(b) - a).normalize();
                for id in &ids {
                    if *id == reference {
                        continue;
                    }
                    let (x, y, _) = self
                        .axe_element(id)
                        .ok_or("Alignement : mur droit requis.")?;
                    let x = DVec2::from_array(x);
                    let v = (DVec2::from_array(y) - x).normalize();
                    let angle = v.perp_dot(u).atan2(v.dot(u)).to_degrees();
                    let set = self.dependances(std::slice::from_ref(id))?;
                    self.transformer(&set, [0.0; 3], x.to_array(), angle)?;
                    let d = u.perp() * (a - x).dot(u.perp());
                    self.transformer(&set, [d.x, d.y, 0.0], [0.0; 2], 0.0)?;
                }
                Ok(ids)
            }
            Commande::InverserMur { id } => {
                self.inverser_mur(&id)?;
                Ok(vec![id])
            }
            Commande::ModifierTrajectoireMur{id,points}=>{
                self.modifier_trajectoire_courbe(&id,points)?;
                Ok(vec![id])
            }
            Commande::AjusterAngle { id, reference, fin, fin_reference } => {
                if id == reference { return Err("Choisissez un mur différent de la limite.".into()); }
                let (ra, rb) = self.ligne_ajustable(&reference).ok_or("Référence droite requise.")?;
                let (ia, ib) = self.ligne_ajustable(&id).ok_or("Mur droit ou poutre requis.")?;
                let point = self.cible_justification(&id, [ra, rb], fin)?;
                self.cible_justification(&reference, [ia, ib], fin_reference)?;
                let poutre = |p: &Projet, x: &str| p.barres.iter().any(|b| b.id == x);
                if !poutre(self, &id) && !poutre(self, &reference) {
                    let z1 = self.axe_element(&id).ok_or("Mur absent")?.2;
                    let z2 = self.axe_element(&reference).ok_or("Référence absente")?.2;
                    if (z1-z2).abs()>1e-6 { return Err("Les murs doivent être au même niveau pour former un angle.".into()); }
                }
                self.deplacer_extremite_ajustable(&id, fin, point)?;
                self.deplacer_extremite_ajustable(&reference, fin_reference, point)?;
                Ok(vec![id, reference])
            }
            Commande::Ajuster { id, reference, fin, ligne } => {
                if id == reference && ligne.is_none() { return Err("Choisissez un mur différent de la limite.".into()); }
                let ligne = match ligne {
                    Some(l) => l,
                    None => { let (a, b) = self.ligne_ajustable(&reference).ok_or("Référence droite requise.")?; [a, b] }
                };
                let point = self.cible_justification(&id, ligne, fin)?;
                self.deplacer_extremite_ajustable(&id, fin, point)?;
                Ok(vec![id])
            }
            Commande::Deplacer { ids, delta } => {
                let set = self.dependances_avec(&ids, true)?;
                self.transformer(&set, delta, [0.0; 2], 0.0)?;
                Ok(ids)
            }
            Commande::Tourner { ids, centre, angle } => {
                let set = self.dependances_avec(&ids, true)?;
                self.transformer(&set, [0.0; 3], centre, angle)?;
                Ok(ids)
            }
            Commande::Symetrie { ids, a, b } => self.symetrie(ids, a, b),
            Commande::Echelle { ids, centre, facteur } => self.echelle(ids, centre, facteur),
            Commande::Reseau { ids, nombre, delta, centre, angle } => self.reseau(ids, nombre, delta, centre, angle),
            Commande::Decaler { id, distance, copie } => self.decaler(id, distance, copie),
            Commande::ScinderSurface { id, trace } => scinder_surface::executer(self, &id, &trace),
            Commande::Scinder { id, point, ecart } => self.scinder(id, point, ecart),
            Commande::Demolir { ids, phase } => self.demolir(ids, phase),
            Commande::Reproduire { source, cibles } => self.reproduire(source, cibles),
            Commande::ChangerHote { id, mur, point } => self.changer_hote(id, mur, point),
            Commande::InverserOuverture { id, paumelles } => self.inverser_ouverture(id, paumelles),
            Commande::Supprimer { ids } => {
                let set = self.dependances(&ids)?;
                for n in &self.niveaux {
                    if set.contains(&n.id)
                        && (self.murs.iter().any(|m| {
                            m.niveau == n.id
                                || matches!(&m.haut,Haut::Niveau {niveau,..} if niveau==&n.id)
                        }) || self.dalles.iter().any(|d| d.niveau == n.id)
                            || self.barres.iter().any(|b| b.niveau == n.id)
                            || self.pieces.iter().any(|b| b.niveau == n.id)
                            || self.composants.iter().any(|b| b.niveau == n.id)
                            || self.escaliers.iter().any(|e| e.niveau == n.id || e.niveau_haut.as_ref() == Some(&n.id))
                            || self.garde_corps.iter().any(|g| g.niveau == n.id)
                            || self
                                .importes
                                .iter()
                                .any(|i| i.niveau.as_ref() == Some(&n.id))
                            || self.vues.iter().any(
                                |v| matches!(&v.genre,GenreVue::Plan {niveau,bas_niveau,..} if niveau==&n.id || bas_niveau==&n.id) || v.limite_haute_niveau.as_ref()==Some(&n.id),
                            ))
                    {
                        return Err("Ce niveau est encore utilisé.".into());
                    }
                }
                supprimer::detacher_annotations(self, &set);
                self.murs.retain(|x| !set.contains(&x.id));
                self.assemblages.retain(|x| !set.contains(&x.id));
                self.ouvertures.retain(|x| !set.contains(&x.id));
                self.dalles.retain(|x| !set.contains(&x.id));
                self.barres.retain(|x| !set.contains(&x.id));
                self.pieces.retain(|x| !set.contains(&x.id));
                self.composants.retain(|x| !set.contains(&x.id));
                self.escaliers.retain(|x| !set.contains(&x.id));
                self.garde_corps.retain(|x| !set.contains(&x.id));
                self.importes.retain(|x| !set.contains(&x.id));
                self.liens.retain(|x| !set.contains(&x.id));
                self.terrains.retain(|x| !set.contains(&x.id));
                self.limites_propriete.retain(|x| !set.contains(&x.id));
                self.niveaux.retain(|x| !set.contains(&x.id));
                self.vues.retain(|x| !set.contains(&x.id));
                for vue in &mut self.vues { vue.elements_masques.retain(|id| !set.contains(id)); }
                self.attaches_toitures
                    .retain(|id, toits| {
                        toits.retain(|toit| !set.contains(toit));
                        !set.contains(id) && !toits.is_empty()
                    });
                self.priorites_jonction.retain(|id, _| !set.contains(id));
                self.implantations.retain(|id, _| !set.contains(id));
                self.placements_surface.retain(|id, _| !set.contains(id));
                self.contextes.retain(|id, _| !set.contains(id));
                self.annotations
                    .retain(|x| !set.contains(&x.id) && !set.contains(&x.vue));
                self.quadrillages.retain(|x| !set.contains(&x.id));
                self.feuilles.retain(|x| !set.contains(&x.id));
                self.nomenclatures.retain(|x| !set.contains(&x.id));
                for f in &mut self.feuilles {
                    f.vues.retain(|v| !set.contains(&v.vue));
                    f.nomenclatures.retain(|n| !set.contains(&n.nomenclature));
                }
                Ok(Vec::new())
            }
            _ => unreachable!("commande hors du métier : la répartition est faite par socle/execution.rs"),
        }
    }
}
