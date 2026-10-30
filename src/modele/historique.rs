//! Transactions atomiques par document. Les instantanés sont bornés en mémoire.
use super::Projet;
use std::collections::VecDeque;

struct Entree {
    json: String,
    libelle: String,
}

#[derive(Default)]
pub struct Historique {
    avant: VecDeque<Entree>,
    apres: Vec<Entree>,
    sauvegarde: Option<String>,
    geste: bool,
    debut_geste: Option<Entree>,
}

/// Décrit les collections modifiées, sans prétendre connaître une commande lorsque
/// plusieurs collections changent en même temps.
fn libelle(avant: &str, apres: &str) -> String {
    let (Ok(a), Ok(b)) = (
        serde_json::from_str::<serde_json::Value>(avant),
        serde_json::from_str::<serde_json::Value>(apres),
    ) else {
        return "Modifier le projet".into();
    };
    let categories = [
        ("niveaux", "niveau"),
        ("murs", "mur"),
        ("assemblages", "assemblage"),
        ("dalles", "élément horizontal"),
        ("barres", "structure"),
        ("escaliers", "escalier"),
        ("garde_corps", "garde-corps"),
        ("ouvertures", "ouverture"),
        ("importes", "élément importé"),
        ("vues", "vue"),
        ("quadrillages", "quadrillage"),
        ("annotations", "annotation"),
        ("feuilles", "feuille"),
        ("nomenclatures", "nomenclature"),
        ("types_murs", "type de mur"),
        ("types_dalles", "type de dalle"),
        ("materiaux", "matériau"),
    ];
    let mut changements = Vec::new();
    for (cle, nom) in categories {
        let va = a.get(cle).and_then(|v| v.as_array());
        let vb = b.get(cle).and_then(|v| v.as_array());
        let na = va.map_or(0, Vec::len);
        let nb = vb.map_or(0, Vec::len);
        if na != nb || (na > 0 && va != vb) {
            changements.push((nom, na, nb, va, vb));
        }
    }
    match changements.as_slice() {
        [(nom, na, nb, va, vb)] => {
            let action = if nb > na {
                "Ajouter"
            } else if nb < na {
                "Supprimer"
            } else {
                "Modifier"
            };
            let (source, autre) = if nb < na { (va, vb) } else { (vb, va) };
            let noms: Vec<&str> = source
                .into_iter()
                .flat_map(|v| v.iter())
                .filter(|objet| {
                    let Some(id) = objet.get("id") else {
                        return false;
                    };
                    let precedent = autre.and_then(|v| v.iter().find(|x| x.get("id") == Some(id)));
                    if na == nb {
                        precedent.is_some_and(|p| p != *objet)
                    } else {
                        precedent.is_none()
                    }
                })
                .filter_map(|objet| {
                    objet
                        .get("nom")
                        .and_then(|n| n.as_str())
                        .filter(|n| !n.trim().is_empty())
                })
                .collect();
            if let [objet] = noms.as_slice() {
                format!("{action} {nom} « {objet} »")
            } else {
                format!("{action} {nom}")
            }
        }
        _ => "Modifier le projet".into(),
    }
}

impl Historique {
    pub fn enregistrer(&mut self, p: &Projet) -> Result<(), String> {
        self.geste(false);
        self.sauvegarde = Some(p.json()?);
        Ok(())
    }
    pub fn modifie(&self, p: &Projet) -> bool {
        p.json().ok().as_ref() != self.sauvegarde.as_ref()
    }
    #[cfg(test)]
    pub fn peut_annuler(&self) -> bool {
        !self.avant.is_empty()
    }
    #[cfg(test)]
    pub fn peut_retablir(&self) -> bool {
        !self.apres.is_empty()
    }
    pub fn libelles(&self, retablir: bool) -> Vec<String> {
        if retablir {
            self.apres.iter().rev().map(|e| e.libelle.clone()).collect()
        } else {
            self.avant.iter().rev().map(|e| e.libelle.clone()).collect()
        }
    }
    pub fn geste(&mut self, actif: bool) {
        if !actif && let Some(s) = self.debut_geste.take() {
            self.avant.push_back(s);
            self.borner();
        }
        self.geste = actif;
    }
    fn borner(&mut self) {
        while self.avant.len() > 100
            || (self.avant.len() > 1
                && self.avant.iter().map(|e| e.json.len()).sum::<usize>() > 64 * 1024 * 1024)
        {
            self.avant.pop_front();
        }
    }
    pub fn pousser(&mut self, avant: &Projet, apres: &Projet) -> Result<(), String> {
        let a = avant.json()?;
        let b = apres.json()?;
        if a == b {
            return Ok(());
        }
        if !self.geste || self.debut_geste.is_none() {
            let entree = Entree {
                libelle: libelle(&a, &b),
                json: a,
            };
            if self.geste {
                self.debut_geste = Some(entree);
            } else {
                self.avant.push_back(entree);
            }
        }
        self.apres.clear();
        self.borner();
        Ok(())
    }
    /// Le contrôleur valide et reconstruit la cible avant de déplacer les piles.
    pub fn cible_n(&self, retablir: bool, nombre: usize) -> Result<Projet, String> {
        if nombre == 0 {
            return Err("Nombre de modifications invalide.".into());
        }
        let s = if retablir {
            self.apres.iter().rev().nth(nombre - 1)
        } else {
            self.avant.iter().rev().nth(nombre - 1)
        }
        .ok_or("Historique insuffisant.")?;
        Projet::lire_json(&s.json)
    }
    pub fn confirmer_n(
        &mut self,
        actuel: &Projet,
        retablir: bool,
        nombre: usize,
    ) -> Result<(), String> {
        if nombre == 0
            || nombre
                > if retablir {
                    self.apres.len()
                } else {
                    self.avant.len()
                }
        {
            return Err("Historique insuffisant.".into());
        }
        let mut courant = actuel.json()?;
        for _ in 0..nombre {
            let entree = if retablir {
                self.apres.pop().unwrap()
            } else {
                self.avant.pop_back().unwrap()
            };
            let inverse = Entree {
                json: courant,
                libelle: entree.libelle,
            };
            courant = entree.json;
            if retablir {
                self.avant.push_back(inverse);
            } else {
                self.apres.push(inverse);
            }
        }
        self.borner();
        Ok(())
    }
    #[cfg(test)]
    pub fn cible(&self, retablir: bool) -> Result<Projet, String> {
        self.cible_n(retablir, 1)
    }
    #[cfg(test)]
    pub fn confirmer(&mut self, actuel: &Projet, retablir: bool) -> Result<(), String> {
        self.confirmer_n(actuel, retablir, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sauvegarde_annulation_branche_et_documents_isoles() {
        let a = Projet::nouveau("a");
        let b = Projet::nouveau("b");
        let c = Projet::nouveau("c");
        let mut h = Historique::default();
        let mut autre = Historique::default();
        h.enregistrer(&a).unwrap();
        autre.enregistrer(&c).unwrap();
        h.pousser(&a, &b).unwrap();
        assert!(h.modifie(&b));
        let retour = h.cible(false).unwrap();
        h.confirmer(&b, false).unwrap();
        assert!(!h.modifie(&retour));
        assert!(h.peut_retablir());
        assert!(!autre.peut_annuler());
        h.pousser(&retour, &c).unwrap();
        assert!(!h.peut_retablir());
    }
    #[test]
    fn plusieurs_etapes_conservent_ordre_et_annulent_atomiquement() {
        let a = Projet::nouveau("a");
        let b = Projet::nouveau("b");
        let c = Projet::nouveau("c");
        let d = Projet::nouveau("d");
        let mut h = Historique::default();
        h.enregistrer(&a).unwrap();
        h.pousser(&a, &b).unwrap();
        h.pousser(&b, &c).unwrap();
        h.pousser(&c, &d).unwrap();
        assert!(h.cible_n(false, 4).is_err());
        assert!(h.confirmer_n(&d, false, 4).is_err());
        assert_eq!(h.libelles(false).len(), 3);
        let cible = h.cible_n(false, 2).unwrap();
        assert_eq!(cible.nom, "b");
        h.confirmer_n(&d, false, 2).unwrap();
        assert_eq!(h.cible_n(true, 1).unwrap().nom, "c");
        assert_eq!(h.cible_n(true, 2).unwrap().nom, "d");
        h.confirmer_n(&cible, true, 2).unwrap();
        assert_eq!(h.cible_n(false, 2).unwrap().nom, "b");
    }
    #[test]
    fn les_libelles_identifient_les_elements_et_suivent_le_retablissement() {
        let a = Projet::nouveau("test");
        let mut b = a.clone();
        b.niveaux.push(super::super::Niveau {
            id: "n1".into(),
            nom: "Étage".into(),
            altitude: 3000.0,
            origine: None,
        });
        let mut h = Historique::default();
        h.pousser(&a, &b).unwrap();
        assert_eq!(h.libelles(false), ["Ajouter niveau « Étage »"]);
        h.confirmer_n(&b, false, 1).unwrap();
        assert_eq!(h.libelles(true), ["Ajouter niveau « Étage »"]);
    }
}
