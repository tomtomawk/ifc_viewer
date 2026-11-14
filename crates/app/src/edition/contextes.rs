//! Gestion des phases, sous-projets et variantes et de leurs affectations.
use crate::modele::creation::ListeCreation;
use crate::modele::{Projet, PHASE_EXISTANT};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::contextes::FiltresContextes;
    use crate::modele::{ContexteElement, creation::ParametresMur};

    #[test]
    fn renommage_et_suppression_reaffectent_toutes_les_references() {
        let mut p = Projet::nouveau("Gestion");
        p.phases.push("Finition".into());
        p.sous_projets.push("Structure".into());
        let mur = p.ajouter_mur([0.,0.], [4000.,0.], &ParametresMur::default()).unwrap();
        p.murs[0].phase = Some("Projet".into());
        p.murs[0].sous_projet = Some("A".into());
        p.contextes.insert("autre".into(), ContexteElement { phase: Some("Projet".into()), sous_projet: Some("A".into()), variante: None });
        p.contextes_vues.insert("vue".into(), FiltresContextes { phase: Some("Projet".into()), ..Default::default() });
        p.demolitions.insert(mur.clone(), "Finition".into());
        p.renommer_contexte(ListeCreation::Phase, "Projet", "Travaux", "").unwrap();
        assert_eq!(p.murs[0].phase.as_deref(), Some("Travaux"));
        assert_eq!(p.contextes["autre"].phase.as_deref(), Some("Travaux"));
        assert_eq!(p.contextes_vues["vue"].phase.as_deref(), Some("Travaux"));
        p.renommer_contexte(ListeCreation::Phase, "Finition", "Démolition", "").unwrap();
        assert_eq!(p.demolitions[&mur], "Démolition");
        let avant = p.json().unwrap();
        assert!(p.deplacer_phase("Démolition", -1).is_err());
        assert_eq!(p.json().unwrap(), avant);
        p.supprimer_contexte(ListeCreation::SousProjet, "A", Some("Structure")).unwrap();
        assert_eq!(p.murs[0].sous_projet.as_deref(), Some("Structure"));
        assert_eq!(p.contextes["autre"].sous_projet.as_deref(), Some("Structure"));
        assert!(p.renommer_contexte(ListeCreation::Phase, PHASE_EXISTANT, "Ancien", "").is_err());
    }

    #[test]
    fn variantes_et_filtres_preservent_les_elements_communs() {
        let mut p = Projet::nouveau("Variantes");
        p.phases.push("Démolition".into());
        let a = p.ajouter_choix_creation(ListeCreation::Variante, "A", "Façade").unwrap();
        let b = p.ajouter_choix_creation(ListeCreation::Variante, "B", "Façade").unwrap();
        let c = p.ajouter_choix_creation(ListeCreation::Variante, "C", "Toiture").unwrap();
        let mur = p.ajouter_mur([0.,0.], [4000.,0.], &ParametresMur::default()).unwrap();
        p.murs[0].variante = Some(b.clone());
        p.contextes_vues.insert("vue".into(), FiltresContextes { variante: Some(b.clone()), ..Default::default() });
        p.murs[0].phase = Some("Projet".into());
        p.ouvertures.push(crate::modele::Ouverture { id: "porte".into(), nom: "Porte".into(), genre: crate::modele::GenreOuverture::Libre, mur: mur.clone(), type_ouverture: None, abscisse: 1000.0, largeur: 600.0, hauteur: 2000.0, allege: 0.0, representation: None, valeurs: Default::default(), materiaux: Default::default(), retournee: false, inversee: false, origine: None });
        p.demolitions.insert(mur.clone(), "Démolition".into());
        let mut filtre = FiltresContextes { phase: Some("Projet".into()), sous_projet: None, variante: Some(a.clone()) };
        assert!(p.elements_masques_contextes(&filtre).contains(&mur));
        assert!(p.elements_masques_contextes(&filtre).contains("porte"));
        // sans choix dans le jeu Façade, seule la variante retenue (A, la première) est visible
        filtre.variante = Some(c.clone());
        assert!(p.elements_masques_contextes(&filtre).contains(&mur));
        assert!(p.elements_masques_contextes(&Default::default()).contains(&mur));
        assert_eq!(p.variante_retenue("Façade").unwrap().id, a);
        p.retenir_variante(&b).unwrap();
        assert_eq!(p.variante_retenue("Façade").unwrap().id, b);
        assert!(!p.elements_masques_contextes(&filtre).contains(&mur));
        assert!(!p.elements_masques_contextes(&Default::default()).contains("porte"));
        let relu: Projet = serde_json::from_str(&p.json().unwrap()).unwrap();
        assert_eq!(relu.variante_retenue("Façade").unwrap().id, b);
        // le choix de la vue prime sur la variante retenue
        let vue = FiltresContextes { variante: Some(a.clone()), ..Default::default() };
        assert!(p.elements_masques_contextes(&FiltresContextes::default().sur(&vue)).contains(&mur));
        assert!(!p.elements_masques_contextes(&FiltresContextes { variante: Some(b.clone()), ..Default::default() }.sur(&vue)).contains(&mur));
        filtre.phase = Some("Démolition".into());
        assert!(p.elements_masques_contextes(&filtre).contains(&mur));
        // adopter A : le mur de la variante B est supprimé, celui de A rejoint le modèle principal
        let mut q = p.clone();
        q.demolitions.clear();
        let garde = q.ajouter_mur([0.,3000.], [4000.,3000.], &ParametresMur::default()).unwrap();
        q.murs.iter_mut().find(|m| m.id == garde).unwrap().variante = Some(a.clone());
        assert_eq!(q.elements_variantes_ecartees(&a), vec![mur.clone()]);
        assert_eq!(q.adopter_variante(&a).unwrap(), 1);
        assert!(q.murs.iter().all(|m| m.id != mur) && q.ouvertures.is_empty());
        assert_eq!(q.murs.iter().find(|m| m.id == garde).unwrap().variante, None);
        assert!(q.variantes.iter().all(|v| v.jeu != "Façade") && q.variantes.iter().any(|v| v.id == c));
        assert_eq!(q.contextes_vues["vue"].variante, None);
        assert!(p.supprimer_contexte(ListeCreation::Variante, &b, Some(&c)).is_err());
        p.supprimer_contexte(ListeCreation::Variante, &b, Some(&a)).unwrap();
        assert_eq!(p.murs[0].variante.as_deref(), Some(a.as_str()));
        assert_eq!(p.contextes_vues["vue"].variante.as_deref(), Some(a.as_str()));
        p.renommer_jeu_variantes("Façade", "Enveloppe").unwrap();
        assert_eq!(p.variantes.iter().find(|v| v.id == a).unwrap().jeu, "Enveloppe");
        assert!(p.renommer_jeu_variantes("Enveloppe", "Toiture").is_err());
    }
}

impl Projet {
    /// Variante affichée par défaut dans le jeu `jeu`.
    pub fn variante_retenue(&self, jeu: &str) -> Option<&crate::modele::Variante> {
        let mut du_jeu = self.variantes.iter().filter(|v| v.jeu == jeu);
        let premiere = du_jeu.next()?;
        if premiere.retenue { Some(premiere) } else { du_jeu.find(|v| v.retenue).or(Some(premiere)) }
    }

    /// Retient une variante : elle remplace les autres de son jeu dans les vues sans choix.
    pub fn retenir_variante(&mut self, id: &str) -> Result<(), String> {
        let jeu = self.variantes.iter().find(|v| v.id == id).ok_or("Variante introuvable.")?.jeu.clone();
        for v in self.variantes.iter_mut().filter(|v| v.jeu == jeu) { v.retenue = v.id == id; }
        Ok(())
    }

    /// Éléments des variantes du même jeu que `id`, autres qu'elle : ceux qu'adopter
    /// `id` supprimerait (un assemblage compte pour ses couches).
    pub fn elements_variantes_ecartees(&self, id: &str) -> Vec<String> {
        let Some(jeu) = self.variantes.iter().find(|v| v.id == id).map(|v| v.jeu.as_str()) else { return Vec::new() };
        let autres: std::collections::HashSet<&str> = self.variantes.iter().filter(|v| v.jeu == jeu && v.id != id).map(|v| v.id.as_str()).collect();
        let ecartee = |v: &Option<String>| v.as_deref().is_some_and(|v| autres.contains(v));
        let connus = self.identifiants();
        let mut ids: Vec<String> = Vec::new();
        for m in self.murs.iter().filter(|m| ecartee(&m.variante)) {
            let cle = m.assemblage.as_ref().filter(|a| self.assemblage(a).is_some()).unwrap_or(&m.id);
            if !ids.contains(cle) { ids.push(cle.clone()); }
        }
        ids.extend(self.dalles.iter().filter(|d| ecartee(&d.variante)).map(|d| d.id.clone()));
        for (cle, c) in &self.contextes {
            if ecartee(&c.variante) && connus.contains(cle) && !ids.contains(cle) && !self.murs.iter().any(|m| &m.id == cle) { ids.push(cle.clone()); }
        }
        ids
    }

    /// Adopte une variante : ses éléments rejoignent le modèle principal, ceux des autres
    /// variantes du jeu sont supprimés, et le jeu disparaît. Renvoie le nombre d'éléments
    /// supprimés.
    pub fn adopter_variante(&mut self, id: &str) -> Result<usize, String> {
        let jeu = self.variantes.iter().find(|v| v.id == id).ok_or("Variante introuvable.")?.jeu.clone();
        let ids = self.elements_variantes_ecartees(id);
        let nombre = ids.len();
        let mut copie = self.clone();
        if !ids.is_empty() {
            copie = copie.executer(crate::edition::commandes::Commande::Supprimer { ids })?.0;
        }
        for v in self.variantes.iter().filter(|v| v.jeu == jeu) {
            copie.remplacer_contexte(ListeCreation::Variante, &v.id, None);
        }
        copie.variantes.retain(|v| v.jeu != jeu);
        *self = copie;
        Ok(nombre)
    }

    /// Nombre d'affectations directes et de démolitions utilisant une entrée.
    pub fn utilisations_contexte(&self, liste: ListeCreation, cle: &str) -> usize {
        let choix = |phase: &Option<String>, sous_projet: &Option<String>, variante: &Option<String>| match liste {
            ListeCreation::Phase => phase.as_deref() == Some(cle),
            ListeCreation::SousProjet => sous_projet.as_deref() == Some(cle),
            ListeCreation::Variante => variante.as_deref() == Some(cle),
        };
        self.murs.iter().filter(|m| choix(&m.phase, &m.sous_projet, &m.variante)).count()
            + self.dalles.iter().filter(|d| choix(&d.phase, &d.sous_projet, &d.variante)).count()
            + self.contextes.values().filter(|c| choix(&c.phase, &c.sous_projet, &c.variante)).count()
            + self.contextes_vues.values().filter(|c| choix(&c.phase, &c.sous_projet, &c.variante)).count()
            + self.gabarits_vues.iter().filter(|g| choix(&g.phase, &None, &g.variante)).count()
            + if liste == ListeCreation::Phase { self.demolitions.values().filter(|v| v.as_str() == cle).count() } else { 0 }
    }

    fn remplacer_contexte(&mut self, liste: ListeCreation, ancien: &str, nouveau: Option<&str>) {
        let remplacer = |v: &mut Option<String>| { if v.as_deref() == Some(ancien) { *v = nouveau.map(str::to_string); } };
        for m in &mut self.murs { match liste { ListeCreation::Phase => remplacer(&mut m.phase), ListeCreation::SousProjet => remplacer(&mut m.sous_projet), ListeCreation::Variante => remplacer(&mut m.variante) } }
        for d in &mut self.dalles { match liste { ListeCreation::Phase => remplacer(&mut d.phase), ListeCreation::SousProjet => remplacer(&mut d.sous_projet), ListeCreation::Variante => remplacer(&mut d.variante) } }
        for c in self.contextes.values_mut() { match liste { ListeCreation::Phase => remplacer(&mut c.phase), ListeCreation::SousProjet => remplacer(&mut c.sous_projet), ListeCreation::Variante => remplacer(&mut c.variante) } }
        for c in self.contextes_vues.values_mut() { match liste { ListeCreation::Phase => remplacer(&mut c.phase), ListeCreation::SousProjet => remplacer(&mut c.sous_projet), ListeCreation::Variante => remplacer(&mut c.variante) } }
        for g in &mut self.gabarits_vues { match liste { ListeCreation::Phase => remplacer(&mut g.phase), ListeCreation::Variante => remplacer(&mut g.variante), ListeCreation::SousProjet => {} } }
        if liste == ListeCreation::Phase {
            if let Some(nouveau) = nouveau { for phase in self.demolitions.values_mut().filter(|p| p.as_str() == ancien) { *phase = nouveau.into(); } }
            else { self.demolitions.retain(|_, p| p != ancien); }
        }
    }

    /// Les démolitions ne peuvent précéder la création, même après réorganisation.
    fn verifier_chronologie_contextes(&self) -> Result<(), String> {
        for (id, demolition) in &self.demolitions {
            let d = self.phases.iter().position(|p| p == demolition).ok_or("Phase de démolition introuvable.")?;
            if let Some(creation) = self.phase_element(id) && let Some(c) = self.phases.iter().position(|p| p == &creation) && c > d {
                return Err(format!("La phase de démolition « {demolition} » précéderait la création « {creation} »."));
            }
        }
        Ok(())
    }

    pub fn renommer_contexte(&mut self, liste: ListeCreation, cle: &str, nom: &str, jeu: &str) -> Result<(), String> {
        let nom = nom.trim(); let jeu = jeu.trim();
        if nom.is_empty() { return Err("Saisissez un nom.".into()); }
        let mut copie = self.clone();
        match liste {
            ListeCreation::Variante => {
                if jeu.is_empty() { return Err("Saisissez le jeu de variantes.".into()); }
                if copie.variantes.iter().any(|v| v.id != cle && v.jeu.eq_ignore_ascii_case(jeu) && v.nom.eq_ignore_ascii_case(nom)) { return Err("Cette variante existe déjà dans ce jeu.".into()); }
                let v = copie.variantes.iter_mut().find(|v| v.id == cle).ok_or("Variante introuvable.")?;
                v.nom = nom.into(); v.jeu = jeu.into();
            }
            _ => {
                if liste == ListeCreation::Phase && cle == PHASE_EXISTANT && nom != cle { return Err("La phase Existant est réservée aux éléments existants.".into()); }
                let choix = if liste == ListeCreation::Phase { &mut copie.phases } else { &mut copie.sous_projets };
                if choix.iter().any(|v| v != cle && v.eq_ignore_ascii_case(nom)) { return Err("Ce nom existe déjà.".into()); }
                let entree = choix.iter_mut().find(|v| v.as_str() == cle).ok_or("Entrée introuvable.")?;
                *entree = nom.into();
                copie.remplacer_contexte(liste, cle, Some(nom));
            }
        }
        copie.verifier_chronologie_contextes()?;
        *self = copie; Ok(())
    }

    pub fn renommer_jeu_variantes(&mut self, ancien: &str, nouveau: &str) -> Result<(), String> {
        let nouveau = nouveau.trim();
        if nouveau.is_empty() { return Err("Saisissez un nom de jeu.".into()); }
        if !self.variantes.iter().any(|v| v.jeu == ancien) { return Err("Jeu introuvable.".into()); }
        if self.variantes.iter().any(|v| v.jeu != ancien && v.jeu.eq_ignore_ascii_case(nouveau)) {
            return Err("Un jeu porte déjà ce nom.".into());
        }
        for v in self.variantes.iter_mut().filter(|v| v.jeu == ancien) { v.jeu = nouveau.into(); }
        Ok(())
    }

    pub fn supprimer_contexte(&mut self, liste: ListeCreation, cle: &str, remplacement: Option<&str>) -> Result<(), String> {
        if liste == ListeCreation::Phase && cle == PHASE_EXISTANT { return Err("La phase Existant ne peut pas être supprimée.".into()); }
        let mut copie = self.clone();
        let valide = match liste {
            ListeCreation::Phase => copie.phases.iter().any(|v| v == cle) && copie.phases.len() > 1 && remplacement.is_some_and(|r| r != cle && copie.phases.iter().any(|v| v == r)),
            ListeCreation::SousProjet => copie.sous_projets.iter().any(|v| v == cle) && copie.sous_projets.len() > 1 && remplacement.is_some_and(|r| r != cle && copie.sous_projets.iter().any(|v| v == r)),
            ListeCreation::Variante => copie.variantes.iter().find(|v| v.id == cle).is_some_and(|source| remplacement.is_none_or(|r| r != cle && copie.variantes.iter().any(|v| v.id == r && v.jeu == source.jeu))),
        };
        if !valide { return Err("Choisissez une entrée de remplacement valide ; au moins une phase et un sous-projet sont nécessaires.".into()); }
        copie.remplacer_contexte(liste, cle, remplacement);
        match liste { ListeCreation::Phase => copie.phases.retain(|v| v != cle), ListeCreation::SousProjet => copie.sous_projets.retain(|v| v != cle), ListeCreation::Variante => copie.variantes.retain(|v| v.id != cle) }
        copie.verifier_chronologie_contextes()?;
        *self = copie; Ok(())
    }

    pub fn deplacer_phase(&mut self, phase: &str, delta: isize) -> Result<(), String> {
        let mut copie = self.clone();
        let i = copie.phases.iter().position(|p| p == phase).ok_or("Phase introuvable.")?;
        let j = i.checked_add_signed(delta).filter(|&j| j < copie.phases.len()).ok_or("La phase est déjà en bout de liste.")?;
        if copie.phases[i] == PHASE_EXISTANT || copie.phases[j] == PHASE_EXISTANT { return Err("La phase Existant reste en première position.".into()); }
        copie.phases.swap(i, j);
        copie.verifier_chronologie_contextes()?;
        *self = copie; Ok(())
    }
}
