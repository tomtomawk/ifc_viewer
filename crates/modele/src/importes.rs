//! Données d'un élément importé lues dans la source : jeux de propriétés et de quantités,
//! matériau, couleur. Champs facultatifs, à plat dans le JSON de l'élément
//! (`importes[]`) ; un élément sans données les omet.
use serde::{Deserialize, Serialize};

/// Données lues d'un élément importé (lecture seule dans l'interface, réécrites à
/// l'export IFC).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct DonneesImportees {
    /// Jeux de propriétés et de quantités, dans l'ordre de la source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proprietes: Vec<JeuProprietes>,
    /// Nom du matériau (IFC : matériaux associés, joints par « | »).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
    /// Couleur sRGB et opacité (0 à 255) de l'élément dans la source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub couleur: Option<[u8; 4]>,
}

/// Jeu de propriétés (IFC : `IfcPropertySet`) ou de quantités (`IfcElementQuantity`).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct JeuProprietes {
    pub nom: String,
    /// Jeu de quantités : chaque valeur est un nombre positif ou nul, avec son unité.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quantites: bool,
    pub proprietes: Vec<ProprieteImportee>,
}

/// Propriété lue : valeur en texte, unité facultative (m, m², m³, kg… : unités SI de
/// base, les valeurs de la source sont converties à la lecture).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ProprieteImportee {
    pub nom: String,
    pub valeur: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unite: Option<String>,
    /// Type booléen de la source (IFC : `IfcBoolean`, `IfcLogical`) ; `valeur` vaut alors
    /// « Vrai », « Faux » (ou « Inconnu » pour un logique). Omis pour un texte ou un nombre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logique: Option<TypeLogique>,
}

/// Type d'une valeur booléenne importée.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TypeLogique {
    /// Vrai ou faux (IFC : `IfcBoolean`).
    Booleen,
    /// Vrai, faux ou inconnu (IFC : `IfcLogical`).
    Logique,
}

impl TypeLogique {
    /// Valeur STEP (`T`, `F`, `U`) d'un texte « Vrai », « Faux », « Inconnu » admis par ce type.
    pub fn step(self, valeur: &str) -> Option<char> {
        match (self, valeur) {
            (_, "Vrai") => Some('T'),
            (_, "Faux") => Some('F'),
            (TypeLogique::Logique, "Inconnu") => Some('U'),
            _ => None,
        }
    }
}

impl ProprieteImportee {
    /// Valeur numérique, si le texte est un nombre décimal (« 2.5 », « -3 », « 1e-3 »).
    /// Les mots `inf`, `NaN`… acceptés par `f64::from_str` ne sont pas des nombres ici.
    pub fn nombre(&self) -> Option<f64> {
        let t = self.valeur.trim();
        let chiffres = t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'));
        if !chiffres || !t.chars().any(|c| c.is_ascii_digit()) {
            return None;
        }
        t.parse::<f64>().ok()
    }

    /// Valeur suivie de son unité, pour l'affichage.
    pub fn libelle(&self) -> String {
        match &self.unite {
            Some(u) => format!("{} {u}", self.valeur),
            None => self.valeur.clone(),
        }
    }
}

impl DonneesImportees {
    /// Rien n'a été lu.
    pub fn est_vide(&self) -> bool {
        self.proprietes.is_empty() && self.materiau.is_none() && self.couleur.is_none()
    }

    /// Nombre total de propriétés, tous jeux confondus.
    pub fn nombre_proprietes(&self) -> usize {
        self.proprietes.iter().map(|j| j.proprietes.len()).sum()
    }

    /// Noms non vides ; noms de jeux uniques ; noms uniques dans un jeu ; unité non vide
    /// si présente ; nombres finis ; quantités numériques positives ou nulles ; matériau
    /// non vide.
    pub fn verifier(&self) -> Result<(), String> {
        if self.materiau.as_ref().is_some_and(|m| m.trim().is_empty()) {
            return Err("matériau importé sans nom.".into());
        }
        let mut jeux = std::collections::HashSet::new();
        for j in &self.proprietes {
            if j.nom.trim().is_empty() {
                return Err("jeu de propriétés sans nom.".into());
            }
            if !jeux.insert(j.nom.as_str()) {
                return Err(format!("jeu de propriétés en double : {}.", j.nom));
            }
            let mut noms = std::collections::HashSet::new();
            for p in &j.proprietes {
                if p.nom.trim().is_empty() {
                    return Err(format!("{} : propriété sans nom.", j.nom));
                }
                if !noms.insert(p.nom.as_str()) {
                    return Err(format!("{} : propriété en double : {}.", j.nom, p.nom));
                }
                if p.unite.as_ref().is_some_and(|u| u.trim().is_empty()) {
                    return Err(format!("{}.{} : unité vide.", j.nom, p.nom));
                }
                if p.logique.is_some_and(|t| j.quantites || t.step(&p.valeur).is_none()) {
                    return Err(format!("{}.{} : valeur booléenne non valide.", j.nom, p.nom));
                }
                match p.nombre() {
                    Some(v) if !v.is_finite() => return Err(format!("{}.{} : valeur non finie.", j.nom, p.nom)),
                    Some(v) if j.quantites && v < 0.0 => return Err(format!("{}.{} : quantité négative.", j.nom, p.nom)),
                    None if j.quantites => return Err(format!("{}.{} : quantité non numérique.", j.nom, p.nom)),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ElementImporte, Maillage, Projet};

    fn element(donnees: DonneesImportees) -> ElementImporte {
        let maillage = Maillage { sommets: vec![[0.0; 3], [1000.0, 0.0, 0.0], [0.0, 1000.0, 0.0], [0.0, 0.0, 1000.0]], triangles: vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]] };
        ElementImporte { id: "importe-1".into(), nom: "Bloc".into(), categorie: "IfcBuildingElementProxy".into(), niveau: None, maillage, raison: "essai".into(), origine: None, donnees }
    }

    fn donnees() -> DonneesImportees {
        let p = |n: &str, v: &str, u: Option<&str>| ProprieteImportee { nom: n.into(), valeur: v.into(), unite: u.map(str::to_string), logique: None };
        DonneesImportees {
            proprietes: vec![
                JeuProprietes { nom: "Pset_Essai".into(), quantites: false, proprietes: vec![p("Reference", "B-01", None), p("Masse", "12.5", Some("kg")), ProprieteImportee { logique: Some(TypeLogique::Booleen), ..p("IsExternal", "Vrai", None) }] },
                JeuProprietes { nom: "Qto_Essai".into(), quantites: true, proprietes: vec![p("NetVolume", "0.1667", Some("m³"))] },
            ],
            materiau: Some("Béton".into()),
            couleur: Some([204, 25, 25, 255]),
        }
    }

    /// Écriture puis relecture : les données reviennent identiques ; un élément sans
    /// données n'écrit aucun des trois champs, et un fichier qui ne les a pas se lit.
    #[test]
    fn ecriture_relecture() {
        let mut p = Projet::nouveau("Essai");
        p.importes.push(element(donnees()));
        p.verifier().unwrap();
        let json = p.json().unwrap();
        assert!(json.contains("\"proprietes\"") && json.contains("\"couleur\"") && json.contains("\"quantites\":true"));
        let relu = Projet::lire_json(&json).unwrap();
        assert_eq!(relu.importes[0].donnees, donnees());
        assert_eq!(relu.importes[0].donnees.nombre_proprietes(), 4);
        assert!(json.contains("\"logique\":\"booleen\""));
        let mut sans = Projet::nouveau("Essai");
        sans.importes.push(element(DonneesImportees::default()));
        let json = sans.json().unwrap();
        assert!(!json.contains("\"proprietes\"") && !json.contains("\"materiau\"") && !json.contains("\"couleur\""));
        assert!(Projet::lire_json(&json).unwrap().importes[0].donnees.est_vide());
    }

    /// Rejets : noms vides ou en double, valeur non finie, quantité non numérique ou
    /// négative, unité ou matériau vides.
    #[test]
    fn rejet() {
        /// Altération des données qui doit les faire rejeter.
        type Alteration = Box<dyn Fn(&mut DonneesImportees)>;
        let cas: Vec<(&str, Alteration)> = vec![
            ("jeu sans nom", Box::new(|d| d.proprietes[0].nom = " ".into())),
            ("jeu en double", Box::new(|d| d.proprietes[1].nom = "Pset_Essai".into())),
            ("propriété sans nom", Box::new(|d| d.proprietes[0].proprietes[0].nom.clear())),
            ("propriété en double", Box::new(|d| d.proprietes[0].proprietes[1].nom = "Reference".into())),
            ("valeur non finie", Box::new(|d| d.proprietes[0].proprietes[1].valeur = "1e999".into())),
            ("quantité non numérique", Box::new(|d| d.proprietes[1].proprietes[0].valeur = "beaucoup".into())),
            ("quantité négative", Box::new(|d| d.proprietes[1].proprietes[0].valeur = "-1".into())),
            ("unité vide", Box::new(|d| d.proprietes[0].proprietes[1].unite = Some(String::new()))),
            ("booléen non valide", Box::new(|d| d.proprietes[0].proprietes[2].valeur = "Inconnu".into())),
            ("logique sur un texte", Box::new(|d| d.proprietes[0].proprietes[0].logique = Some(TypeLogique::Logique))),
            ("matériau vide", Box::new(|d| d.materiau = Some(String::new()))),
        ];
        for (nom, modifier) in cas {
            let mut d = donnees();
            modifier(&mut d);
            assert!(d.verifier().is_err(), "{nom}");
            let mut p = Projet::nouveau("Essai");
            p.importes.push(element(d));
            assert!(p.verifier().is_err(), "{nom}");
        }
        // « NaN » et « inf » restent du texte (pas des nombres)
        let p = ProprieteImportee { nom: "x".into(), valeur: "NaN".into(), unite: None, logique: None };
        assert_eq!(p.nombre(), None);
    }
}
