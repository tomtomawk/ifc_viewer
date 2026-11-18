//! Éléments 2D des vues d'une famille : une seule liste, à coordonnées fixes ou
//! ancrés aux plans de référence. Un élément ancré porte un `PointEsquisse` (plan et
//! décalage sur chaque axe de la vue) par point de sa forme ; à la résolution, ces
//! points sont recalculés depuis la position des plans, et la forme suit les
//! paramètres (sens d'ouverture, débattement, tracés qui suivent largeur et hauteur).
use super::{Affichage, FamilleModelee, NiveauDetail, PointEsquisse, Resolution, TypeVue, VisibiliteElement, VueEsquisse};
use crate::documentation::{AnnotationForme, RemplissageContour};
use serde::{Deserialize, Serialize};

/// Élément graphique associé à une vue de la famille ; les textes et leurs
/// dimensions utilisent les mêmes formes que les annotations du projet.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Element2DFamille {
    pub vue: VueEsquisse,
    /// Forme ; pour un élément ancré, ses points sont la position à la dernière
    /// résolution et sont recalculés depuis `ancrage`.
    pub forme: AnnotationForme,
    /// Hauteur de texte sur le papier en millimètres.
    pub hauteur: f64,
    /// Ancrage des points de la forme aux plans de référence, dans l'ordre de
    /// `points_ancrables_mut` ; vide : coordonnées fixes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ancrage: Vec<PointEsquisse>,
    /// Paramètre oui/non qui commande l'affichage (None : toujours visible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<String>,
    /// Visibilité par niveau de détail et par type de vue (absente : partout).
    #[serde(default, skip_serializing_if = "VisibiliteElement::partout")]
    pub visibilite: VisibiliteElement,
}

impl Element2DFamille {
    /// Élément à coordonnées fixes.
    pub fn fixe(vue: VueEsquisse, forme: AnnotationForme, hauteur: f64) -> Self {
        Self { vue, forme, hauteur, ancrage: Vec::new(), visible: None, visibilite: VisibiliteElement::default() }
    }

    /// Élément ancré : un sommet par point ancrable de la forme ; les coordonnées de
    /// la forme sont recalculées à la résolution.
    pub fn ancre(vue: VueEsquisse, mut forme: AnnotationForme, ancrage: Vec<PointEsquisse>) -> Self {
        // coordonnées provisoires : décalages seuls, remplacées à la résolution
        if let Some(points) = points_ancrables_mut(&mut forme) {
            for (p, a) in points.into_iter().zip(&ancrage) { *p = [a.h.decalage, a.v.decalage]; }
        }
        Self { vue, forme, hauteur: 2.5, ancrage, visible: None, visibilite: VisibiliteElement::default() }
    }

    /// Polyligne ancrée (ouverte ou fermée).
    pub fn polyligne(vue: VueEsquisse, ancrage: Vec<PointEsquisse>, fermee: bool) -> Self {
        let points = vec![[0.0; 2]; ancrage.len()];
        Self::ancre(vue, AnnotationForme::Polyligne { points, fermee, courbures: Vec::new(), remplissage: None, traits: Vec::new() }, ancrage)
    }

    /// Arc ancré par trois points (départ, passage, fin).
    pub fn arc(vue: VueEsquisse, ancrage: Vec<PointEsquisse>) -> Self {
        Self::ancre(vue, AnnotationForme::Arc { points: [[0.0; 2]; 3] }, ancrage)
    }

    pub fn est_ancre(&self) -> bool {
        !self.ancrage.is_empty()
    }

    /// Polyligne fermée : peut servir de section de profil en plan.
    pub fn contour_ferme(&self) -> bool {
        matches!(&self.forme, AnnotationForme::Polyligne { fermee: true, .. })
    }

    /// Remplissage d'une polyligne fermée.
    pub fn remplissage_mut(&mut self) -> Option<&mut Option<RemplissageContour>> {
        match &mut self.forme {
            AnnotationForme::Polyligne { remplissage, .. } => Some(remplissage),
            _ => None,
        }
    }
}

/// Points d'une forme qui peuvent être ancrés aux plans de référence, dans l'ordre de
/// l'ancrage (cercle : son centre, le rayon reste fixe) ; None : forme non ancrable.
pub fn points_ancrables_mut(forme: &mut AnnotationForme) -> Option<Vec<&mut [f64; 2]>> {
    Some(match forme {
        AnnotationForme::Texte { point, .. } | AnnotationForme::Etiquette { point, .. } => vec![point],
        AnnotationForme::Ligne { debut, fin } => vec![debut, fin],
        AnnotationForme::Polyligne { points, .. } | AnnotationForme::Spline { points, .. } | AnnotationForme::Zone { contour: points } => points.iter_mut().collect(),
        AnnotationForme::Arc { points } => points.iter_mut().collect(),
        AnnotationForme::Cercle { centre, .. } => vec![centre],
        _ => return None,
    })
}

impl FamilleModelee {
    /// Élément résolu : points ancrés recalculés depuis les plans ; ancrage retiré.
    pub fn element_resolu(&self, r: &Resolution, e: &Element2DFamille) -> Element2DFamille {
        let mut forme = e.forme.clone();
        if e.est_ancre() && let Some(points) = points_ancrables_mut(&mut forme) {
            for (p, a) in points.into_iter().zip(&e.ancrage) {
                let (h, v) = self.point_esquisse(r, a);
                *p = [h, v];
            }
        }
        Element2DFamille { ancrage: Vec::new(), forme, ..e.clone() }
    }

    /// Contenu graphique d'une vue, dans son repère local, points ancrés résolus.
    pub fn elements_2d_resolus(&self, r: &Resolution, vue: VueEsquisse) -> Vec<Element2DFamille> {
        self.elements_2d.iter().filter(|e| e.vue == vue).map(|e| self.element_resolu(r, e)).collect()
    }

    /// Éléments de la vue `vue` montrés dans une vue du projet de niveau de détail
    /// `detail` (type de vue tiré de `vue`, paramètre « Visible si » appliqué), résolus.
    pub fn elements_2d_affiches(&self, r: &Resolution, vue: VueEsquisse, detail: NiveauDetail) -> Vec<Element2DFamille> {
        let a = Affichage { detail, vue: TypeVue::depuis_esquisse(vue) };
        self.elements_2d_resolus(r, vue).into_iter().filter(|e| e.visibilite.montre(a) && self.visible(r, &e.visible)).collect()
    }

    /// Traits affichés de la vue `vue` en polylignes (h, v) ; arcs et courbes discrétisés.
    pub fn lignes_2d_resolues(&self, r: &Resolution, vue: VueEsquisse, detail: NiveauDetail) -> Vec<Vec<(f64, f64)>> {
        self.elements_2d_affiches(r, vue, detail).into_iter()
            .filter_map(|e| e.forme.forme_2d())
            .flat_map(|f| f.polylignes())
            .map(|ligne| ligne.into_iter().map(|p| (p.x, p.y)).collect())
            .collect()
    }

    /// Régions 2D remplies de la vue `vue` : contour fermé développé (h, v) et remplissage.
    pub fn regions_resolues(&self, r: &Resolution, vue: VueEsquisse, detail: NiveauDetail) -> Vec<(Vec<(f64, f64)>, RemplissageContour)> {
        self.elements_2d_affiches(r, vue, detail).into_iter().filter_map(|e| {
            let AnnotationForme::Polyligne { fermee: true, remplissage: Some(m), .. } = &e.forme else { return None };
            let contour = e.forme.contour_developpe()?;
            Some((contour.into_iter().map(|p| (p[0], p[1])).collect(), m.clone()))
        }).collect()
    }

    /// Textes de la vue `vue` résolus : position (h, v), contenu, hauteur papier.
    pub fn textes_resolus(&self, r: &Resolution, vue: VueEsquisse, detail: NiveauDetail) -> Vec<((f64, f64), String, f64)> {
        self.elements_2d_affiches(r, vue, detail).into_iter().filter_map(|e| match e.forme {
            AnnotationForme::Texte { point, texte } => Some(((point[0], point[1]), texte, e.hauteur)),
            _ => None,
        }).collect()
    }

    /// Résout le contour de l'élément 2D désigné comme section du profil.
    pub fn section_resolue(&self, r: &Resolution, section: usize) -> Result<Vec<[f64; 2]>, String> {
        let e = self.elements_2d.get(section).ok_or("Section de profil introuvable.")?;
        let points = match self.element_resolu(r, e).forme {
            AnnotationForme::Polyligne { points, fermee: true, .. } if e.vue == VueEsquisse::Plan => points,
            _ => return Err("La section doit être une polyligne fermée en plan.".into()),
        };
        super::super::profil::contour_simple(&points)?;
        Ok(points)
    }

    /// Contrôle des éléments 2D : hauteur, forme cohérente, ancrage sur des plans des
    /// axes de la vue, section de profil.
    pub(super) fn valider_elements_2d(&self) -> Result<(), String> {
        if let Some(i) = self.section_profil
            && !self.elements_2d.get(i).is_some_and(|e| e.vue == VueEsquisse::Plan && e.contour_ferme()) {
            return Err("Section de profil absente ou non fermée en plan.".into());
        }
        for (i, e) in self.elements_2d.iter().enumerate() {
            let n = i + 1;
            if !e.hauteur.is_finite() || e.hauteur <= 0.0 { return Err(format!("Élément 2D {n} : hauteur invalide.")); }
            if let AnnotationForme::Polyligne { points, fermee, remplissage, .. } = &e.forme
                && (points.len() < 2 || (*fermee && points.len() < 3) || (remplissage.is_some() && !fermee)) {
                return Err(format!("Élément 2D {n} : polyligne de moins de deux sommets, ou remplie sans être fermée."));
            }
            if !e.est_ancre() { continue; }
            let mut forme = e.forme.clone();
            let Some(points) = points_ancrables_mut(&mut forme) else { return Err(format!("Élément 2D {n} : forme non ancrable.")) };
            if points.len() != e.ancrage.len() { return Err(format!("Élément 2D {n} : un ancrage par point de la forme attendu.")); }
            let (ah, av, _) = e.vue.axes();
            let sur = |nom: &str, axe| self.plan(nom).is_some_and(|p| p.axe == axe);
            if !e.ancrage.iter().all(|a| sur(&a.h.plan, ah) && sur(&a.v.plan, av) && a.h.decalage.is_finite() && a.v.decalage.is_finite()) {
                return Err(format!("Élément 2D {n} : points sur des plans absents."));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{FichierFamille, FORMAT_FAMILLE};
    use super::*;

    fn porte() -> FichierFamille {
        FichierFamille { format: FORMAT_FAMILLE.into(), nom: "Porte".into(), largeur: 900.0, hauteur: 2150.0, profondeur: 200.0, famille: FamilleModelee::gabarit_porte() }
    }

    #[test]
    fn element_ancre_ecriture_relecture() {
        let f = porte();
        let json = f.json().unwrap();
        assert!(json.contains("\"ancrage\"") && !json.contains("\"symboles\""));
        assert_eq!(FichierFamille::lire(&json).unwrap(), f);
    }

    #[test]
    fn element_ancre_suit_les_parametres() {
        let f = FamilleModelee::gabarit_porte();
        for largeur in [900.0, 1200.0] {
            let r = f.resoudre(largeur, 2150.0, 200.0);
            let e = f.elements_2d_resolus(&r, VueEsquisse::Plan);
            let AnnotationForme::Polyligne { points, .. } = &e[0].forme else { panic!() };
            assert!((points[0][0] - r.positions["Dormant gauche"]).abs() < 1e-9, "{points:?}");
        }
    }

    #[test]
    fn elements_2d_filtres_par_detail_et_parametre() {
        let mut f = FamilleModelee::gabarit_porte();
        let r = f.resoudre(900.0, 2150.0, 200.0);
        let n = f.lignes_2d_resolues(&r, VueEsquisse::Plan, NiveauDetail::Faible).len();
        f.elements_2d[0].visibilite.faible = false;
        assert!(f.lignes_2d_resolues(&r, VueEsquisse::Plan, NiveauDetail::Faible).len() < n);
        assert_eq!(f.lignes_2d_resolues(&r, VueEsquisse::Plan, NiveauDetail::Eleve).len(), n);
        f.elements_2d[0].visibilite.faible = true;
        f.elements_2d[0].visibilite.plan = false;
        assert!(f.lignes_2d_resolues(&r, VueEsquisse::Plan, NiveauDetail::Moyen).len() < n);
        f.elements_2d[0].visibilite.plan = true;
        f.parametres.push(super::super::Parametre { genre: super::super::GenreParametre::OuiNon, ..super::super::Parametre::nouveau("Montrer", "0") });
        f.elements_2d[0].visible = Some("Montrer".into());
        let r = f.resoudre(900.0, 2150.0, 200.0);
        assert!(f.lignes_2d_resolues(&r, VueEsquisse::Plan, NiveauDetail::Moyen).len() < n);
        assert_eq!(f.elements_2d_resolus(&r, VueEsquisse::Plan).len(), f.elements_2d.iter().filter(|e| e.vue == VueEsquisse::Plan).count());
    }

    #[test]
    fn ancrage_invalide_refuse() {
        let mut f = porte();
        f.famille.elements_2d[0].ancrage.pop();
        assert!(FichierFamille::lire(&f.json().unwrap()).is_err());
        let mut f = porte();
        f.famille.elements_2d[0].ancrage[0].h.plan = "Absent".into();
        assert!(FichierFamille::lire(&f.json().unwrap()).is_err());
        let mut f = porte();
        f.famille.elements_2d[0].forme = AnnotationForme::Cote { debut: crate::documentation::PointCote::Fixe { point: [0.0; 2] }, fin: crate::documentation::PointCote::Fixe { point: [1.0, 0.0] }, decalage: 0.0 };
        assert!(FichierFamille::lire(&f.json().unwrap()).is_err());
        // ancien format : `symboles` n'est plus lu
        let mut ancien = serde_json::to_value(porte()).unwrap();
        ancien["famille"]["symboles"] = serde_json::json!([]);
        assert!(FichierFamille::lire(&ancien.to_string()).is_err());
    }
}
