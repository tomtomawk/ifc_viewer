//! Familles des feuilles : cartouches et étiquettes de pièce ouverts dans l'éditeur
//! de familles, puis enregistrés dans le projet.
use crate::*;
use modele::documentation as doc;

impl Application {
    /// Ouvre la famille du cartouche `rang` de la feuille `id` ; elle lui reste
    /// affectée à l'enregistrement (même renommée ou enregistrée sous un autre nom).
    pub(crate) fn editer_cartouche_feuille(&mut self, id: &str, rang: usize) {
        let Some((p, _)) = &self.natif else { return };
        let Some(c) = p.feuilles.iter().find(|f| f.id == id).and_then(|f| f.cartouches.get(rang)) else { return };
        let source = Some(doc::famille_cartouche(p, &c.famille).cloned().unwrap_or_default());
        self.editer_cartouche(source, Some(id.to_string()));
        if let Some(ed) = self.famille.as_mut() { ed.cible_cartouche = Some(rang); }
    }

    pub(crate) fn editer_cartouche_catalogue(&mut self, id: &str) {
        let Some((p, _)) = &self.natif else { return };
        let source = p.familles_cartouches.iter().find(|f| f.id == id).cloned();
        self.editer_cartouche(source, None);
    }

    pub(crate) fn editer_cartouche(&mut self, source: Option<doc::FamilleCartouche>, feuille: Option<String>) {
        let noms: Vec<String> = self.natif.as_ref().map(|(p, _)| p.familles_cartouches.iter().map(|f| f.nom.clone()).collect()).unwrap_or_default();
        match source {
            Some(c) => self.ouvrir_famille_document_projet(*c.document_famille, Some(c.id)),
            None => {
                let mut document = *doc::FamilleCartouche::default().document_famille;
                document.nom = (1..).map(|n| if n == 1 { "Cartouche personnalisé".into() } else { format!("Cartouche personnalisé {n}") })
                    .find(|nom| !noms.contains(nom)).unwrap_or_default();
                self.ouvrir_famille_document_projet(document, None);
            }
        }
        if let Some(ed) = self.famille.as_mut() { ed.cible_feuille = feuille; }
    }
    /// Ouvre l'éditeur d'une famille d'étiquette de pièce : `famille` (Some("") : la
    /// standard ; None : nouvelle, copie de celle de la pièce), affectée à `piece`.
    pub(crate) fn editer_famille_etiquette(&mut self, famille: Option<String>, piece: Option<String>) {
        let projet_vide = modele::Projet::nouveau("Famille");
        let p = self.natif.as_ref().map(|(p, _)| p).unwrap_or(&projet_vide);
        let existante = famille.as_ref().and_then(|id| p.familles_etiquettes.iter().find(|f| &f.id == id)).cloned();
        if let Some(f) = existante {
            self.ouvrir_famille_document_projet(*f.document_famille, Some(f.id));
        } else {
            // la standard ou une copie de celle de la pièce : nouvelle famille à l'enregistrement
            let actuelle = piece.as_ref().and_then(|id| p.pieces.iter().find(|x| &x.id == id)).map(|x| p.famille_etiquette(x));
            let f = if famille.is_none() { actuelle.unwrap_or_else(doc::FamilleEtiquette::standard) } else { doc::FamilleEtiquette::standard() };
            let base = if famille.is_none() { format!("{} (copie)", f.nom) } else { "Étiquette de pièce".to_string() };
            let nom = (1..).map(|k| if k == 1 { base.clone() } else { format!("{base} {k}") }).find(|n| !p.familles_etiquettes.iter().any(|x| &x.nom == n)).unwrap_or_default();
            let mut document = *f.document_famille;
            document.nom = nom;
            self.ouvrir_famille_document_projet(document, None);
        }
        if let Some(ed) = self.famille.as_mut() { ed.cible_piece = piece; }
    }


    /// Enregistre une famille d'étiquette (nouvelle si son identifiant est vide) et
    /// l'affecte à la pièce `piece` ; rend son identifiant.
    pub(crate) fn enregistrer_famille_etiquette(&mut self, mut famille: doc::FamilleEtiquette, piece: Option<String>, original: &str) -> Result<String, String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let nom = famille.nom.trim().to_string();
        if nom.is_empty() || nom == "Étiquette standard" || p.familles_etiquettes.iter().any(|f| f.nom == nom && f.id != original) {
            return Err("Choisir un nom de famille non vide et distinct des familles existantes.".into());
        }
        famille.nom = nom;
        let mut q = p.clone();
        if famille.id.is_empty() {
            famille.id = (1..).map(|k| format!("etiquette-piece-{k}")).find(|i| !q.familles_etiquettes.iter().any(|f| &f.id == i)).unwrap_or_default();
        }
        let id = famille.id.clone();
        match q.familles_etiquettes.iter_mut().find(|f| f.id == id) { Some(f) => *f = famille, None => q.familles_etiquettes.push(famille) }
        if let Some(x) = piece.and_then(|i| q.pieces.iter_mut().find(|x| x.id == i)) { x.etiquette = id.clone(); }
        self.appliquer_projet(q)?;
        if piece_outil_suit(&self.bim.etiquette_piece, original) { self.bim.etiquette_piece = id.clone(); }
        self.coupe_dirty = true;
        self.status = "Famille d'étiquette enregistrée — Ctrl+Z pour annuler.".into();
        Ok(id)
    }

    /// Enregistre une famille de cartouche ; un renommage suit sur les cartouches posés,
    /// et le cartouche `cible` (feuille, rang) prend cette famille.
    pub(crate) fn enregistrer_cartouche(&mut self, famille: &doc::FamilleCartouche, cible: Option<(&str, usize)>, original: &str) -> Result<(), String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let nom = &famille.nom;
        if nom.is_empty() || (nom == doc::NOM_CARTOUCHE_STANDARD && original != doc::NOM_CARTOUCHE_STANDARD)
            || (original == doc::NOM_CARTOUCHE_STANDARD && nom != doc::NOM_CARTOUCHE_STANDARD)
            || p.familles_cartouches.iter().any(|f| &f.nom == nom && f.id != famille.id) {
            return Err("Choisir un nom de famille non vide et distinct des familles existantes.".into());
        }
        let mut q = p.clone();
        if original.is_empty() || (original == doc::NOM_CARTOUCHE_STANDARD && !q.familles_cartouches.iter().any(|f| f.nom == original)) {
            q.familles_cartouches.push(famille.clone());
        } else {
            let f = q.familles_cartouches.iter_mut().find(|f| f.id == famille.id).ok_or("Famille de cartouche absente.")?;
            let ancien = std::mem::replace(f, famille.clone()).nom;
            if ancien != *nom {
                for c in q.feuilles.iter_mut().flat_map(|f| f.cartouches.iter_mut()).filter(|c| c.famille == ancien) {
                    c.famille = nom.clone();
                }
            }
        }
        if let Some((id, rang)) = cible
            && let Some(c) = q.feuilles.iter_mut().find(|f| f.id == id).and_then(|f| f.cartouches.get_mut(rang)) {
                c.famille = if nom == doc::NOM_CARTOUCHE_STANDARD { String::new() } else { nom.clone() };
            }
        self.appliquer_projet(q)
    }
}

/// L'outil Pièce suit la famille enregistrée s'il utilisait celle qu'on vient d'éditer.
fn piece_outil_suit(outil: &str, original: &str) -> bool {
    !original.is_empty() && outil == original
}
