//! Familles des symboles d'annotation (têtes de niveau et de quadrillage, repères de
//! coupe et d'élévation) : ouvertes dans l'éditeur de familles, puis enregistrées dans
//! le projet et choisies pour leur catégorie.
use crate::*;
use modele::documentation::{FamilleSymbole, GenreSymbole};

impl Application {
    /// Ouvre la famille de symbole `id` du projet ; vide : une copie de la famille
    /// utilisée pour `genre` (standard si aucune), enregistrée comme nouvelle famille.
    pub(crate) fn editer_famille_symbole(&mut self, genre: GenreSymbole, id: &str) {
        let projet_vide = modele::Projet::nouveau("Famille");
        let p = self.natif.as_ref().map(|(p, _)| p).unwrap_or(&projet_vide);
        if let Some(f) = p.familles_symboles.iter().find(|f| f.id == id).cloned() {
            self.ouvrir_famille_document_projet(*f.document_famille, Some(f.id));
            return;
        }
        let f = p.famille_symbole(genre);
        let base = if f.id.is_empty() { genre.libelle().to_string() } else { format!("{} (copie)", f.nom) };
        let nom = (1..).map(|k| if k == 1 { base.clone() } else { format!("{base} {k}") }).find(|n| !p.familles_symboles.iter().any(|x| &x.nom == n)).unwrap_or_default();
        let mut document = *f.document_famille;
        document.nom = nom;
        self.ouvrir_famille_document_projet(document, None);
    }

    /// Enregistre une famille de symbole (nouvelle si son identifiant est vide) ;
    /// `choisir` : elle devient celle de sa catégorie dans le projet. Rend son
    /// identifiant.
    pub(crate) fn enregistrer_famille_symbole(&mut self, mut famille: FamilleSymbole, choisir: bool) -> Result<String, String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let genre = famille.genre().ok_or("Catégorie de famille de symbole non valide.")?;
        let nom = famille.nom.trim().to_string();
        if nom.is_empty() || GenreSymbole::TOUS.iter().any(|g| g.nom_standard() == nom) || p.familles_symboles.iter().any(|f| f.nom == nom && f.id != famille.id) {
            return Err("Choisir un nom de famille non vide et distinct des familles existantes.".into());
        }
        famille.nom = nom;
        let mut q = p.clone();
        if famille.id.is_empty() {
            famille.id = noyau::uuid::uuid();
        }
        let id = famille.id.clone();
        match q.familles_symboles.iter_mut().find(|f| f.id == id) {
            Some(f) => *f = famille,
            None => q.familles_symboles.push(famille),
        }
        if choisir {
            *q.symboles.choix_mut(genre) = Some(id.clone());
        }
        self.appliquer_projet(q)?;
        self.coupe_dirty = true;
        self.status = format!("{} enregistré{} — Ctrl+Z pour annuler.", genre.libelle(), if choisir { " et utilisé dans le projet" } else { "" });
        Ok(id)
    }

    /// Repère du type de vue de rang `k` : famille `id`, ou celui du projet (None).
    pub(crate) fn choisir_repere_type_vue(&mut self, k: usize, id: Option<String>) -> Result<(), String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let mut q = p.clone();
        q.types_vues.get_mut(k).ok_or("Type de vue absent.")?.repere = id;
        self.appliquer_projet(q)?;
        self.coupe_dirty = true;
        self.status = "Repère du type de vue modifié — Ctrl+Z pour annuler.".into();
        Ok(())
    }

    /// Choisit la famille `id` (vide : la standard) pour les symboles `genre`.
    pub(crate) fn choisir_famille_symbole(&mut self, genre: GenreSymbole, id: &str) -> Result<(), String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let mut q = p.clone();
        *q.symboles.choix_mut(genre) = (!id.is_empty()).then(|| id.to_string());
        self.appliquer_projet(q)?;
        self.coupe_dirty = true;
        Ok(())
    }
}
