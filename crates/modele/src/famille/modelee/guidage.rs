//! Formes entre sections : raccordement et balayage sur chemin (segments et arcs par
//! trois points, ouvert ou fermé). La géométrie est dans `balayage` (chemin, sections
//! en onglet) et `raccord` (correspondance de profils de nombres de sommets différents).
use super::PointEsquisse;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "genre", rename_all = "snake_case")]
pub enum FormeGuidee {
    ExtrusionChemin {
        chemin: Vec<[f64; 3]>,
        /// Indices des points de passage d'arcs : le point `i` et ses voisins forment
        /// un arc par trois points.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        arcs: Vec<usize>,
        /// Chemin fermé : un dernier tronçon relie le dernier point au premier.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        ferme: bool,
    },
    Raccordement {
        profil_fin: Vec<[f64; 2]>,
        hauteur: f64,
        /// Profil d'arrivée verrouillé sur les plans (paramètres et formules) ; il
        /// remplace alors `profil_fin`, recalculé à la résolution.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ancrage_fin: Option<Vec<PointEsquisse>>,
    },
    RaccordementChemin {
        profil_fin: Vec<[f64; 2]>,
        chemin: Vec<[f64; 3]>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        arcs: Vec<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ancrage_fin: Option<Vec<PointEsquisse>>,
    },
}

/// Chemin, arcs et fermeture modifiables d'une forme par chemin.
pub type TraceMut<'a> = (&'a mut Vec<[f64; 3]>, &'a mut Vec<usize>, Option<&'a mut bool>);

impl FormeGuidee {
    pub fn nom(&self) -> &'static str {
        match self {
            Self::ExtrusionChemin { .. } => "Extrusion par chemin",
            Self::Raccordement { .. } => "Raccordement",
            Self::RaccordementChemin { .. } => "Raccordement par chemin",
        }
    }

    /// Extrusion par chemin droit (premier point à l'origine de la section).
    pub fn extrusion(chemin: Vec<[f64; 3]>) -> Self { Self::ExtrusionChemin { chemin, arcs: Vec::new(), ferme: false } }
    pub fn raccordement(profil_fin: Vec<[f64; 2]>, hauteur: f64) -> Self { Self::Raccordement { profil_fin, hauteur, ancrage_fin: None } }
    pub fn raccordement_chemin(profil_fin: Vec<[f64; 2]>, chemin: Vec<[f64; 3]>) -> Self {
        Self::RaccordementChemin { profil_fin, chemin, arcs: Vec::new(), ancrage_fin: None }
    }

    /// Chemin discrétisé (arcs en cordes de 5° au plus) ; fermé : sans répéter le départ.
    pub fn points_chemin(chemin: &[[f64; 3]], arcs: &[usize], ferme: bool) -> Result<Vec<[f64; 3]>, String> {
        Ok(super::balayage::discretiser(chemin, arcs, ferme)?.into_iter().map(|p| p.to_array()).collect())
    }

    pub fn chemin(&self) -> Vec<[f64; 3]> {
        match self {
            Self::Raccordement { hauteur, .. } => vec![[0.0; 3], [0.0, 0.0, *hauteur]],
            Self::ExtrusionChemin { chemin, .. } | Self::RaccordementChemin { chemin, .. } => chemin.clone(),
        }
    }

    /// Points de passage des arcs du chemin.
    pub fn arcs(&self) -> &[usize] {
        match self {
            Self::ExtrusionChemin { arcs, .. } | Self::RaccordementChemin { arcs, .. } => arcs,
            Self::Raccordement { .. } => &[],
        }
    }

    pub fn ferme(&self) -> bool { matches!(self, Self::ExtrusionChemin { ferme: true, .. }) }

    /// Chemin, arcs et fermeture modifiables (formes par chemin seulement).
    pub fn trace_mut(&mut self) -> Option<TraceMut<'_>> {
        match self {
            Self::ExtrusionChemin { chemin, arcs, ferme } => Some((chemin, arcs, Some(ferme))),
            Self::RaccordementChemin { chemin, arcs, .. } => Some((chemin, arcs, None)),
            Self::Raccordement { .. } => None,
        }
    }

    pub fn profil_fin(&self) -> Option<&[[f64; 2]]> {
        match self {
            Self::ExtrusionChemin { .. } => None,
            Self::Raccordement { profil_fin, .. } | Self::RaccordementChemin { profil_fin, .. } => Some(profil_fin),
        }
    }

    pub fn ancrage_fin(&self) -> Option<&[PointEsquisse]> {
        match self {
            Self::ExtrusionChemin { .. } => None,
            Self::Raccordement { ancrage_fin, .. } | Self::RaccordementChemin { ancrage_fin, .. } => ancrage_fin.as_deref(),
        }
    }

    pub fn ancrage_fin_mut(&mut self) -> Option<&mut Option<Vec<PointEsquisse>>> {
        match self {
            Self::ExtrusionChemin { .. } => None,
            Self::Raccordement { ancrage_fin, .. } | Self::RaccordementChemin { ancrage_fin, .. } => Some(ancrage_fin),
        }
    }

    /// Forme dont le profil d'arrivée ancré est recalculé par `point` (résolution des
    /// plans de la famille) ; sans ancrage, la forme telle quelle.
    pub fn resolue(&self, point: impl Fn(&PointEsquisse) -> (f64, f64)) -> FormeGuidee {
        let mut forme = self.clone();
        let calcule: Option<Vec<[f64; 2]>> = self.ancrage_fin().map(|a| a.iter().map(|p| { let (h, v) = point(p); [h, v] }).collect());
        if let (Some(fin), Self::Raccordement { profil_fin, .. } | Self::RaccordementChemin { profil_fin, .. }) = (calcule, &mut forme) {
            *profil_fin = fin;
        }
        forme
    }

    pub fn valider(&self, profil: &[[f64; 2]]) -> Result<(), String> {
        self.construire(profil).map(|_| ())
    }

    pub fn triangles(&self, profil: &[[f64; 2]]) -> Vec<[glam::DVec3; 3]> {
        self.construire(profil).unwrap_or_default()
    }

    fn construire(&self, profil: &[[f64; 2]]) -> Result<Vec<[glam::DVec3; 3]>, String> {
        let simple = |p: &[[f64; 2]]| {
            if p.iter().flatten().any(|x| !x.is_finite()) { return Err("Profil : coordonnées non finies.".to_string()); }
            super::super::profil::contour_simple(p)
        };
        simple(profil)?;
        let fin = self.profil_fin().unwrap_or(profil);
        simple(fin)?;
        if let Some(a) = self.ancrage_fin() && a.len() < 3 {
            return Err("Le profil d'arrivée ancré doit compter au moins trois sommets.".into());
        }
        if super::raccord::aire(profil) * super::raccord::aire(fin) <= 0.0 {
            return Err("Les deux profils doivent être dessinés dans le même sens.".into());
        }
        let (depart, arrivee) = super::raccord::correspondance(profil, fin);
        // Empêche les effondrements et croisements intermédiaires usuels.
        for i in 1..20 {
            let section = super::raccord::interpoler(&depart, &arrivee, i as f64 / 20.0);
            simple(&section).map_err(|e| format!("Raccordement : section intermédiaire invalide ({e})"))?;
            if super::raccord::aire(&section) * super::raccord::aire(profil) <= 0.0 { return Err("Le raccordement inverse une section intermédiaire.".into()); }
        }
        let chemin = self.chemin();
        if chemin.iter().flatten().any(|x| !x.is_finite()) { return Err("Chemin : coordonnées non finies.".into()); }
        let ferme = self.ferme();
        let points = super::balayage::discretiser(&chemin, self.arcs(), ferme)?;
        super::balayage::balayer(&points, ferme, &depart, &arrivee)
    }
}

#[cfg(test)]
#[path = "tests_guidage.rs"]
mod tests;
