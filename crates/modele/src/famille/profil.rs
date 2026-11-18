//! Familles de profils plans définis par des paramètres et un contour fermé.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profil {
    /// Famille complète ; la section provient de son contour fermé en plan
    /// (`section_profil` le désigne quand il y en a plusieurs).
    pub document_famille: Box<super::modelee::FichierFamille>,
    #[serde(default)] pub id: String,
    pub nom: String,
}

impl Profil {
    /// Profil dont la famille porte un seul contour fermé fixe en plan.
    pub fn depuis_contour(nom: &str, points: Vec<[f64; 2]>) -> Self {
        use super::modelee::{CategorieFamille, Element2DFamille, FamilleModelee, FichierFamille, VueEsquisse, FORMAT_FAMILLE};
        use crate::documentation::AnnotationForme;
        let mut famille = FamilleModelee::vide(CategorieFamille::Profil);
        famille.elements_2d.push(Element2DFamille::fixe(VueEsquisse::Plan, AnnotationForme::Polyligne { points, fermee: true, courbures: vec![], remplissage: None, traits: vec![] }, 2.5));
        Self { document_famille: Box::new(FichierFamille { format: FORMAT_FAMILLE.into(), nom: nom.into(), largeur: 100.0, hauteur: 100.0, profondeur: 100.0, famille }), id: String::new(), nom: nom.into() }
    }

    pub fn points(&self) -> Result<Vec<[f64; 2]>, String> {
        use super::modelee::VueEsquisse;
        use crate::documentation::AnnotationForme;
        if self.nom.trim().is_empty() { return Err("Donnez un nom au profil.".into()); }
        let document = &self.document_famille;
        let r = document.famille.resoudre(document.largeur, document.hauteur, document.profondeur);
        if let Some(erreur) = r.erreurs.first() { return Err(erreur.clone()); }
        if let Some(section) = document.famille.section_profil {
            return document.famille.section_resolue(&r, section);
        }
        let elements = document.famille.elements_2d_resolus(&r, VueEsquisse::Plan);
        let mut contours = elements.into_iter().filter_map(|e| match e.forme {
            AnnotationForme::Polyligne { points, fermee: true, .. } => Some(points),
            _ => None,
        });
        let points = contours.next().ok_or("Un profil nécessite une polyligne fermée en plan.")?;
        if contours.next().is_some() { return Err("Plusieurs contours fermés : choisissez la section dans les propriétés.".into()); }
        contour_simple(&points)?;
        Ok(points)
    }
    pub fn valider(&self) -> Result<(), String> { self.points().map(|_| ()) }
}

/// Contrôle d'un contour fermé : au moins trois sommets distincts, sans croisement ni
/// contact entre côtés non voisins, d'aire non nulle.
pub fn contour_simple(points: &[[f64; 2]]) -> Result<(), String> {
    if points.len() < 3 { return Err("Le contour doit compter au moins trois sommets.".into()); }
    let n = points.len();
    let croix = |a: [f64;2], b: [f64;2], c: [f64;2]| (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);
    let mut aire = 0.0;
    for i in 0..n {
        let a = points[i]; let b = points[(i+1)%n];
        if (a[0]-b[0]).hypot(a[1]-b[1]) < 1e-7 { return Err("Deux sommets consécutifs sont confondus.".into()); }
        aire += a[0]*b[1]-b[0]*a[1];
        for j in i+1..n {
            if j == i+1 || (i == 0 && j == n-1) { continue; }
            let c = points[j]; let d = points[(j+1)%n];
            let (ab_c, ab_d, cd_a, cd_b) = (croix(a,b,c), croix(a,b,d), croix(c,d,a), croix(c,d,b));
            let sur_segment = |a: [f64;2], b: [f64;2], p: [f64;2]| p[0] >= a[0].min(b[0])-1e-7 && p[0] <= a[0].max(b[0])+1e-7 && p[1] >= a[1].min(b[1])-1e-7 && p[1] <= a[1].max(b[1])+1e-7;
            if (ab_c * ab_d < 0.0 && cd_a * cd_b < 0.0)
                || (ab_c.abs() < 1e-7 && sur_segment(a,b,c)) || (ab_d.abs() < 1e-7 && sur_segment(a,b,d))
                || (cd_a.abs() < 1e-7 && sur_segment(c,d,a)) || (cd_b.abs() < 1e-7 && sur_segment(c,d,b)) {
                return Err("Le contour se croise ou se touche.".into());
            }
        }
    }
    if aire.abs() < 1e-7 { return Err("Le contour doit avoir une surface non nulle.".into()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn section_explicite_permet_plusieurs_contours_et_refuse_les_references_invalides() {
        use crate::documentation::AnnotationForme;
        use crate::famille::modelee::{Element2DFamille, VueEsquisse, FichierFamille};
        let mut profil = crate::gabarit::Gabarit::defaut().profils[0].clone();
        let attendu = profil.points().unwrap();
        let famille = &mut profil.document_famille.famille;
        let autre = vec![[0.0, 0.0], [20.0, 0.0], [20.0, 10.0], [0.0, 10.0]];
        famille.elements_2d.push(Element2DFamille::fixe(VueEsquisse::Plan, AnnotationForme::Polyligne { points: autre.clone(), fermee: true, courbures: vec![], remplissage: None, traits: vec![] }, 2.5));
        assert!(profil.points().is_err());
        profil.document_famille.famille.section_profil = Some(0);
        assert_eq!(profil.points().unwrap(), attendu);
        profil.document_famille.famille.section_profil = Some(1);
        assert_eq!(profil.points().unwrap(), autre);
        let doc = &*profil.document_famille;
        let json = doc.json().unwrap();
        assert!(json.contains("\"section_profil\": 1"));
        assert_eq!(FichierFamille::lire(&json).unwrap(), *doc);
        profil.document_famille.famille.section_profil = Some(99);
        assert!(profil.points().is_err());
        let doc = &*profil.document_famille;
        assert!(FichierFamille::lire(&doc.json().unwrap()).is_err());
        let famille = &mut profil.document_famille.famille;
        famille.section_profil = Some(1);
        if let AnnotationForme::Polyligne { points, .. } = &mut famille.elements_2d[1].forme { points.truncate(2); }
        let doc = &*profil.document_famille;
        assert!(FichierFamille::lire(&doc.json().unwrap()).is_err());
    }

    #[test]
    fn refuse_formule_cyclique_et_contour_invalide() {
        let mut p = crate::gabarit::Gabarit::defaut().profils[0].clone();
        let famille = &mut p.document_famille.famille;
        famille.parametres[0].formule = famille.parametres[0].nom.clone();
        assert!(p.valider().is_err());
        let famille = &mut p.document_famille.famille;
        famille.parametres[0].formule = "100".into();
        famille.elements_2d[0].ancrage.truncate(2);
        assert!(p.valider().is_err());
    }

    #[test]
    fn familles_documentaires_exigent_leur_document() {
        use crate::documentation::{FamilleCartouche, FamilleEtiquette};
        let profil = super::Profil::depuis_contour("Triangle", vec![[0.0, 0.0], [60.0, 0.0], [0.0, 30.0]]);
        let relu: super::Profil = serde_json::from_str(&serde_json::to_string(&profil).unwrap()).unwrap();
        assert_eq!(relu.points().unwrap(), vec![[0.0, 0.0], [60.0, 0.0], [0.0, 30.0]]);
        let ancien = r#"{"id":"p","nom":"P","parametres":[],"contour":[{"x":"0","y":"0"},{"x":"1","y":"0"},{"x":"0","y":"1"}]}"#;
        assert!(serde_json::from_str::<super::Profil>(ancien).is_err());
        let cartouche = FamilleCartouche::default();
        assert_eq!(serde_json::from_str::<FamilleCartouche>(&serde_json::to_string(&cartouche).unwrap()).unwrap(), cartouche);
        assert!(serde_json::from_str::<FamilleCartouche>(r#"{"id":"c","nom":"C","largeur":10,"hauteur":10,"traits":[],"textes":[]}"#).is_err());
        assert!(serde_json::from_str::<FamilleEtiquette>(r#"{"id":"e","nom":"E","textes":[]}"#).is_err());
    }
}
