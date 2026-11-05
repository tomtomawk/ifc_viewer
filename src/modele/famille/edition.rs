//! Propagation métier d'une famille : résultat complet transporté par la transaction.
use crate::modele::*;
use super::modelee::{FichierFamille, GenreParametre, Portee};

impl Projet {
    pub fn avec_famille(
        &self,
        cible: Option<&str>,
        ouverture: Option<&str>,
        f: &FichierFamille,
        epaisseur: f64,
        partages: BTreeMap<String, f64>,
    ) -> Result<(Projet, String), String> {
        if f.nom.trim().is_empty() {
            return Err("Nommer le type".into());
        }
        f.famille.valider(f.largeur, f.hauteur, epaisseur)?;
        let mut q = self.clone();
        let id = if let Some(id) = cible.filter(|id| q.types_ouvertures.iter().any(|t| t.id == *id))
        {
            let t = q.types_ouvertures.iter_mut().find(|t| t.id == id).unwrap();
            (t.nom, t.largeur, t.hauteur, t.modele, t.genre) = (
                f.nom.trim().into(),
                f.largeur,
                f.hauteur,
                Some(f.famille.clone()),
                f.famille.genre,
            );
            t.famille = None;
            id.to_string()
        } else {
            let id = collaboration::uuid();
            let base = f.nom.trim().to_string();
            let mut nom = base.clone();
            let mut n = 2;
            while q.types_ouvertures.iter().any(|t| t.nom == nom) {
                nom = format!("{base} ({n})");
                n += 1;
            }
            q.types_ouvertures.push(TypeOuverture {
                id: id.clone(),
                nom,
                genre: f.famille.genre,
                largeur: f.largeur,
                hauteur: f.hauteur,
                famille: None,
                modele: Some(f.famille.clone()),
                origine: None,
            });
            id
        };
        // Sémantique existante : toutes les ouvertures de ce type prennent les cotes
        // du type et filtrent leurs paramètres ; ActualiserType reste une autre action.
        for o in &mut q.ouvertures {
            if ouverture == Some(o.id.as_str()) {
                o.type_ouverture = Some(id.clone());
                o.representation = None;
            }
            if o.type_ouverture.as_ref() == Some(&id) {
                (o.largeur, o.hauteur, o.genre) = (f.largeur, f.hauteur, f.famille.genre);
                o.valeurs.retain(|k, _| {
                    f.famille
                        .parametres
                        .iter()
                        .any(|p| &p.nom == k && p.portee == Portee::Occurrence)
                });
                o.materiaux.retain(|k, _| {
                    f.famille.parametres.iter().any(|p| {
                        &p.nom == k
                            && p.portee == Portee::Occurrence
                            && p.genre == GenreParametre::Materiau
                    })
                });
            }
        }
        q.parametres_partages = partages;
        q.recaler_menuiseries(self)?;
        q.verifier()?;
        Ok((q, id))
    }
}
