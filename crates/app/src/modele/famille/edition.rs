//! Propagation métier d'une famille : résultat complet transporté par la transaction.
use crate::modele::*;
use super::modelee::{FichierFamille, GenreParametre, Portee};

impl Projet {
    /// Copie d'un type d'ouverture dans la même famille, sous un nom libre dérivé de `base`.
    pub fn avec_copie_type_ouverture(&self, id: &str, base: &str) -> Result<(Projet, String), String> {
        let mut q = self.clone();
        let source = q.types_ouvertures.iter().find(|t| t.id == id).cloned().ok_or("Type d'ouverture absent.")?;
        let mut t = source.clone();
        t.id = noyau::uuid::uuid();
        t.famille_nom = Some(source.nom_famille().to_string());
        t.origine = None;
        t.nom = q.nom_type_libre(base, None);
        // la source rejoint explicitement la famille, même si son nom de type change ensuite
        if let Some(s) = q.types_ouvertures.iter_mut().find(|x| x.id == id) {
            s.famille_nom = t.famille_nom.clone();
        }
        let nouvel = t.id.clone();
        q.types_ouvertures.push(t);
        q.verifier()?;
        Ok((q, nouvel))
    }

    /// Valeurs qui surchargent les formules d'une ouverture : paramètres partagés du
    /// projet, puis valeurs d'occurrence propres à l'ouverture.
    pub fn surcharges(&self, o: &Ouverture) -> BTreeMap<String, f64> {
        let mut s = self.parametres_partages.clone();
        s.extend(o.valeurs.iter().map(|(k, v)| (k.clone(), *v)));
        s
    }

    /// Nom libre pour un type d'ouverture (`base`, puis « base (2) »…), hors `sauf`.
    fn nom_type_libre(&self, base: &str, sauf: Option<&str>) -> String {
        let pris = |n: &str| self.types_ouvertures.iter().any(|t| t.nom == n && Some(t.id.as_str()) != sauf);
        let mut nom = base.to_string();
        let mut n = 2;
        while pris(&nom) {
            nom = format!("{base} ({n})");
            n += 1;
        }
        nom
    }

    /// `partages` : paramètres partagés modifiés dans l'éditeur, fusionnés dans la table
    /// du projet (les autres valeurs du projet restent).
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
            let nom = f.nom.trim().to_string();
            if q.nom_type_libre(&nom, Some(id)) != nom {
                return Err(format!("Le nom « {nom} » est déjà pris par un autre type."));
            }
            let t = q.types_ouvertures.iter_mut().find(|t| t.id == id).unwrap();
            (t.nom, t.largeur, t.hauteur, t.modele, t.genre) = (
                nom,
                f.largeur,
                f.hauteur,
                Some(f.famille.clone()),
                f.famille.genre,
            );
            t.famille = None;
            id.to_string()
        } else {
            let id = noyau::uuid::uuid();
            let nom = q.nom_type_libre(f.nom.trim(), None);
            q.types_ouvertures.push(TypeOuverture {
                id: id.clone(),
                nom,
                genre: f.famille.genre,
                largeur: f.largeur,
                hauteur: f.hauteur,
                famille: None,
                modele: Some(f.famille.clone()),
                origine: None,
                famille_nom: None,
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
        q.parametres_partages.extend(partages);
        // la famille doit tenir dans chaque mur qui porte une ouverture du type
        let mut epaisseurs: Vec<f64> = q.ouvertures.iter()
            .filter(|o| o.type_ouverture.as_ref() == Some(&id))
            .filter_map(|o| crate::modele::volumes::solides::hote(&q, &o.mur).ok().map(|h| h.1))
            .collect();
        epaisseurs.sort_by(f64::total_cmp);
        epaisseurs.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        for e in epaisseurs.into_iter().filter(|e| (e - epaisseur).abs() >= 0.5) {
            f.famille.valider(f.largeur, f.hauteur, e).map_err(|x| format!("Mur de {e:.0} mm : {x}"))?;
        }
        q.recaler_menuiseries(self)?;
        q.verifier()?;
        Ok((q, id))
    }
}
