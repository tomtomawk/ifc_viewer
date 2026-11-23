//! Copier et copier sur un niveau : duplication des éléments, de leurs dépendances et
//! de leurs relations.
use crate::socle::interne::*;

/// Exécution des commandes de copie.
pub(crate) trait ExecutionCopier {
    fn executer_copier(&mut self, c: Commande) -> Result<Vec<String>, String>;
}

impl ExecutionCopier for Projet {
    fn executer_copier(&mut self, c: Commande) -> Result<Vec<String>, String> {
        match c {
            Commande::CopierSurNiveau {ids,niveau} => {
                let cible=self.niveau(&niveau).ok_or("Niveau de destination introuvable")?.altitude;
                for id in &ids {
                    let admis=self.murs.iter().any(|x|&x.id==id) || self.assemblage(id).is_some()
                        || self.dalles.iter().any(|x|&x.id==id) || self.barres.iter().any(|x|&x.id==id)
                        || self.composants.iter().any(|x|&x.id==id) || self.pieces.iter().any(|x|&x.id==id)
                        || self.escaliers.iter().any(|x|&x.id==id) || self.garde_corps.iter().any(|x|&x.id==id)
                        || self.importes.iter().any(|x|&x.id==id && x.niveau.is_some());
                    if !admis {return Err("La sélection contient un élément qui ne peut pas être copié sur un niveau ; les ouvertures suivent leur mur hôte.".into());}
                }
                let niveaux=self.niveaux.clone();
                let altitude=|id:&str|niveaux.iter().find(|n|n.id==id).map(|n|n.altitude).ok_or("Niveau d'origine introuvable".to_string());
                let copies=self.executer_interne(Commande::Copier{ids,delta:[0.;3]})?;
                let ensemble=self.dependances(&copies)?;
                for m in self.murs.iter_mut().filter(|m|ensemble.contains(&m.id)) {
                    let origine=altitude(&m.niveau)?;
                    if let Haut::Niveau{niveau:haut,decalage}=&mut m.haut {
                        let ancien=altitude(haut)?;let voulu=cible+(ancien-origine);
                        if let Some(n)=niveaux.iter().find(|n|(n.altitude-voulu).abs()<0.001) {*haut=n.id.clone();}
                        else {*decalage+=cible-origine;}
                    }
                    m.niveau=niveau.clone();
                }
                for d in self.dalles.iter_mut().filter(|d|ensemble.contains(&d.id)) {d.niveau=niveau.clone();}
                for b in self.barres.iter_mut().filter(|b|ensemble.contains(&b.id)) {b.niveau=niveau.clone();}
                for b in self.composants.iter_mut().filter(|b|ensemble.contains(&b.id)) {b.niveau=niveau.clone();}
                for b in self.pieces.iter_mut().filter(|b|ensemble.contains(&b.id)) {b.niveau=niveau.clone();}
                for e in self.escaliers.iter_mut().filter(|e|ensemble.contains(&e.id)) {
                    let origine=altitude(&e.niveau)?;
                    if let Some(haut)=&e.niveau_haut {
                        let ancien=altitude(haut)?;let voulu=cible+(ancien-origine);
                        e.niveau_haut=niveaux.iter().find(|n|(n.altitude-voulu).abs()<0.001).map(|n|n.id.clone());
                        if e.niveau_haut.is_none() {e.hauteur=ancien-origine-e.decalage_bas;}
                    }
                    e.niveau=niveau.clone();
                }
                for g in self.garde_corps.iter_mut().filter(|g|ensemble.contains(&g.id)) {g.niveau=niveau.clone();}
                for e in self.importes.iter_mut().filter(|e|ensemble.contains(&e.id)) {
                    let ancien=e.niveau.as_deref().ok_or("Un élément importé n'a pas de niveau")?;
                    let dz=cible-altitude(ancien)?;
                    for p in &mut e.maillage.sommets {p[2]+=dz;}
                    e.niveau=Some(niveau.clone());
                }
                self.arrondir();
                Ok(copies)
            }
            Commande::Copier { ids, delta } => {
                let set = self.dependances(&ids)?;
                if self.niveaux.iter().any(|n| set.contains(&n.id))
                    || self
                        .vues
                        .iter()
                        .any(|n| set.contains(&n.id) && matches!(n.genre, GenreVue::Plan { .. }))
                    || self.feuilles.iter().any(|n| set.contains(&n.id))
                    || self.nomenclatures.iter().any(|n| set.contains(&n.id))
                {
                    return Err("Copie réservée aux éléments du modèle.".into());
                }
                if self
                    .ouvertures
                    .iter()
                    .any(|o| set.contains(&o.id) && !set.contains(&o.mur))
                {
                    return Err(
                        "Copiez le mur hôte avec son ouverture, ou placez une nouvelle ouverture."
                            .into(),
                    );
                }
                let mut map = HashMap::new();
                let mut connus = self.identifiants();
                let mut ordre: Vec<_> = set.iter().collect();
                ordre.sort();
                for old in ordre {
                    let mut n = 1u64;
                    let id = loop {
                        let candidat = format!("copie-{n}");
                        if !connus.contains(&candidat) {
                            break candidat;
                        }
                        n += 1;
                    };
                    connus.insert(id.clone());
                    map.insert(old.clone(), id);
                }
                let mut murs = Vec::new();
                for x in &self.murs {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.origine = None;
                        x.assemblage = x.assemblage.as_ref().map(|a| map[a].clone());
                        murs.push(x);
                    }
                }
                let mut assems = Vec::new();
                for x in &self.assemblages {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.origine = None;
                        x.couches = x.couches.iter().map(|id| map[id].clone()).collect();
                        assems.push(x);
                    }
                }
                let mut baies = Vec::new();
                for x in &self.ouvertures {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.mur = map[&x.mur].clone();
                        x.origine = None;
                        baies.push(x);
                    }
                }
                let mut dalles = Vec::new();
                for x in &self.dalles {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.origine = None;
                        dalles.push(x);
                    }
                }
                let mut imports = Vec::new();
                for x in &self.importes {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.origine = None;
                        imports.push(x);
                    }
                }
                let barres: Vec<_> = self
                    .barres
                    .iter()
                    .filter(|b| set.contains(&b.id))
                    .map(|b| {
                        let mut b = b.clone();
                        b.id = map[&b.id].clone();
                        b.origine = None;
                        b
                    })
                    .collect();
                let vues: Vec<_> = self
                    .vues
                    .iter()
                    .filter(|v| set.contains(&v.id))
                    .map(|v| {
                        let mut v = v.clone();
                        v.id = map[&v.id].clone();
                        v.nom = format!("{} (copie)", v.nom);
                        v
                    })
                    .collect();
                self.vues.extend(vues);
                let annotations: Vec<_> = self
                    .annotations
                    .iter()
                    .filter(|a| map.contains_key(&a.vue) && !set.contains(&a.id))
                    .cloned()
                    .collect();
                for mut annotation in annotations {
                    annotation.id = identite(self, "annotation");
                    annotation.vue = map[&annotation.vue].clone();
                    remapper_annotation(&mut annotation, &map);
                    self.annotations.push(annotation);
                }
                let annotations: Vec<_> = self
                    .annotations
                    .iter()
                    .filter(|a| set.contains(&a.id))
                    .cloned()
                    .collect();
                for mut annotation in annotations {
                    annotation.id = map[&annotation.id].clone();
                    if let Some(id) = map.get(&annotation.vue) {
                        annotation.vue = id.clone();
                    }
                    remapper_annotation(&mut annotation, &map);
                    self.annotations.push(annotation);
                }
                let quadrillages: Vec<_> = self
                    .quadrillages
                    .iter()
                    .filter(|g| set.contains(&g.id))
                    .map(|g| {
                        let mut g = g.clone();
                        g.id = map[&g.id].clone();
                        g.nom = format!("{} (copie)", g.nom);
                        g
                    })
                    .collect();
                self.quadrillages.extend(quadrillages);
                let contextes: Vec<_> = self
                    .contextes
                    .iter()
                    .filter_map(|(id, c)| map.get(id).map(|nouveau| (nouveau.clone(), c.clone())))
                    .collect();
                self.contextes.extend(contextes);
                self.barres.extend(barres);
                let pieces: Vec<_> = self.pieces.iter().filter(|x| set.contains(&x.id)).map(|x| Piece { id: map[&x.id].clone(), origine: None, ..x.clone() }).collect();
                for mut x in pieces {
                    x.numero = self.numero_piece_suivant();
                    self.pieces.push(x);
                }
                let composants: Vec<_> = self.composants.iter().filter(|x| set.contains(&x.id)).map(|x| Composant { id: map[&x.id].clone(), ..x.clone() }).collect();
                self.composants.extend(composants);
                let escaliers: Vec<_> = self.escaliers.iter().filter(|e| set.contains(&e.id)).map(|e| Escalier { id: map[&e.id].clone(), ..e.clone() }).collect();
                let garde_corps: Vec<_> = self
                    .garde_corps
                    .iter()
                    .filter(|g| set.contains(&g.id))
                    .map(|g| GardeCorps { id: map[&g.id].clone(), escalier: g.escalier.as_ref().map(|e| map.get(e).unwrap_or(e).clone()), ..g.clone() })
                    .collect();
                self.escaliers.extend(escaliers);
                self.garde_corps.extend(garde_corps);
                self.murs.extend(murs);
                self.assemblages.extend(assems);
                self.ouvertures.extend(baies);
                self.dalles.extend(dalles);
                let terrains: Vec<_> = self.terrains.iter().filter(|t| set.contains(&t.id)).map(|t| { let mut t = t.clone(); t.id = map[&t.id].clone(); t }).collect();
                let limites: Vec<_> = self.limites_propriete.iter().filter(|l| set.contains(&l.id)).map(|l| { let mut l = l.clone(); l.id = map[&l.id].clone(); l }).collect();
                self.terrains.extend(terrains);
                self.limites_propriete.extend(limites);
                self.importes.extend(imports);
                let attaches: Vec<_> = self
                    .attaches_toitures
                    .iter()
                    .filter(|(id, _)| map.contains_key(*id))
                    .map(|(id, toits)| {
                        let toits = toits.iter().map(|t| map.get(t).unwrap_or(t).clone()).collect();
                        (map[id].clone(), toits)
                    })
                    .collect();
                self.attaches_toitures.extend(attaches);
                let priorites: Vec<_> = self
                    .priorites_jonction
                    .iter()
                    .filter(|(id, _)| map.contains_key(*id))
                    .map(|(id, n)| (map[id].clone(), *n))
                    .collect();
                self.priorites_jonction.extend(priorites);
                let implantations: Vec<_> = self.implantations.iter().filter(|(id, _)| map.contains_key(*id)).map(|(id, r)| (map[id].clone(), *r)).collect();
                self.implantations.extend(implantations);
                let placements: Vec<_> = map.iter().filter_map(|(a, b)| {
                    Some((b.clone(), self.placements_surface.get(a)?.clone()))
                }).collect();
                self.placements_surface.extend(placements);
                self.transformer(&map.values().cloned().collect(), delta, [0.0; 2], 0.0)?;
                Ok(ids.iter().map(|id| map[id].clone()).collect())
            }
            _ => unreachable!("commande hors du métier : la répartition est faite par socle/execution.rs"),
        }
    }
}
fn remapper_annotation(annotation: &mut documentation::Annotation, ids: &HashMap<String, String>) {
    use documentation::{AnnotationForme as F, PointCote};
    match &mut annotation.forme {
        F::Etiquette { element, .. } => {
            if let Some(id) = ids.get(element) {
                *element = id.clone();
            }
        }
        F::Cote { debut, fin, .. } => {
            for p in [debut, fin] {
                if let PointCote::Mur { id, .. } | PointCote::Arete { id, .. } = p
                    && let Some(nouveau) = ids.get(id) {
                        *id = nouveau.clone();
                    }
            }
        }
        F::CoteAngulaire { aretes: Some(aretes), .. } => {
            for a in aretes {
                if let Some(nouveau) = ids.get(&a.id) {
                    a.id = nouveau.clone();
                }
            }
        }
        _ => {}
    }
}
