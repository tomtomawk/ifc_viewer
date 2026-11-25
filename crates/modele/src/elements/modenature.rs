//! Profilés de mur (modénature) : un profil balayé le long d'une face d'un mur, en
//! relief (corniche, bandeau, soubassement, larmier) ou en creux (engravure, retiré du
//! mur), comme « Balayage » et « Profil en creux » de Revit.
//!
//! Section : profil du projet (`profils`), ou rectangle `largeur` × `hauteur` du type.
//! Repère de la section : `x` perpendiculaire à la face (vers l'extérieur du mur pour un
//! relief, vers l'intérieur du mur pour un creux), `y` vers le haut ; origine sur la face
//! (décalée de `decalage` vers l'extérieur du mur), à la hauteur de l'occurrence.
use crate::murs::assemblage::decaler_axe;
use crate::volumes::solides::{repere, Tri};
use crate::{Axe, Mur, Projet};
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TypeModenature {
    pub id: String,
    pub nom: String,
    /// Creux : retiré du mur ; sinon relief, élément à part.
    #[serde(default)]
    pub creux: bool,
    /// Profil du projet ; None : rectangle `largeur` × `hauteur`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profil: Option<String>,
    /// Saillie (relief) ou profondeur (creux) du rectangle, mm.
    pub largeur: f64,
    pub hauteur: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
    /// Décalage de la section depuis la face, vers l'extérieur du mur (mm).
    #[serde(default)]
    pub decalage: f64,
    /// Retrait aux deux extrémités (mm).
    #[serde(default)]
    pub retrait: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Modenature {
    pub id: String,
    pub nom: String,
    /// Mur indépendant ou assemblage hôte.
    pub mur: String,
    pub type_modenature: String,
    /// Face intérieure (dernière couche) ; sinon face extérieure (première couche).
    #[serde(default)]
    pub interieur: bool,
    /// Hauteur de l'origine de la section depuis la base (ou depuis le haut).
    pub hauteur: f64,
    #[serde(default)]
    pub depuis_haut: bool,
    /// Début et fin le long de la face depuis son origine (mm) ; None : bout du mur.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debut: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fin: Option<f64>,
}

impl TypeModenature {
    /// Section dans son repère (x, y), mm.
    pub fn section(&self, p: &Projet) -> Result<Vec<[f64; 2]>, String> {
        let points = match &self.profil {
            Some(id) => p.profils.iter().find(|x| &x.id == id).ok_or_else(|| format!("{} : profil absent.", self.nom))?.points()?,
            None => {
                let (l, h) = (self.largeur, self.hauteur);
                if !(l > 0.0 && h > 0.0 && l.is_finite() && h.is_finite()) { return Err(format!("{} : dimensions non valides.", self.nom)); }
                vec![[0.0, 0.0], [l, 0.0], [l, h], [0.0, h]]
            }
        };
        crate::famille::profil::contour_simple(&points)?;
        Ok(points)
    }
}

impl Projet {
    pub fn type_modenature(&self, id: &str) -> Option<&TypeModenature> {
        self.types_modenatures.iter().find(|t| t.id == id)
    }

    /// Couche portant la face de la modénature (première ou dernière couche de l'hôte).
    fn couche_face(&self, m: &Modenature) -> Result<&Mur, String> {
        let id = match self.assemblage(&m.mur) {
            Some(a) => if m.interieur { a.couches.last() } else { a.couches.first() }.ok_or("Assemblage vide.")?.as_str(),
            None => m.mur.as_str(),
        };
        self.murs.iter().find(|x| x.id == id).ok_or_else(|| "Mur hôte du profilé absent.".into())
    }

    /// Ligne de la face (axe décalé de la demi-épaisseur de sa couche), sens de la
    /// normale vers l'extérieur du mur (+1 : gauche de l'axe), base et haut de la couche.
    pub fn face_modenature(&self, m: &Modenature) -> Result<(Axe, f64, f64, f64), String> {
        let c = self.couche_face(m)?;
        let e = self.type_mur(&c.type_mur).ok_or("Type de mur introuvable.")?.epaisseur();
        let signe = if m.interieur { -1.0 } else { 1.0 };
        let (bas, haut) = self.bas_haut(c).ok_or("Contrainte de hauteur non valide.")?;
        Ok((decaler_axe(&c.axe, signe * e / 2.0), signe, bas, haut))
    }
}

type Booleen = fn(&[Tri], &[Tri], noyau::csg::Op, f64) -> Option<Vec<Tri>>;

/// Aire signée.
fn aire(c: &[[f64; 2]]) -> f64 {
    (0..c.len()).map(|i| { let (a, b) = (c[i], c[(i + 1) % c.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() / 2.0
}

/// Solide fermé de la modénature (relief à ajouter ou creux à retirer du mur).
pub fn solide(p: &Projet, m: &Modenature) -> Result<Vec<Tri>, String> {
    let t = p.type_modenature(&m.type_modenature).ok_or("Type de profilé absent.")?;
    let mut section = t.section(p)?;
    if aire(&section) < 0.0 { section.reverse(); }
    let (axe, signe, bas, haut) = p.face_modenature(m)?;
    let longueur = axe.longueur();
    let s0 = m.debut.unwrap_or(0.0).max(0.0) + t.retrait;
    let s1 = m.fin.unwrap_or(longueur).min(longueur) - t.retrait;
    if s1 - s0 <= 1.0 || !s1.is_finite() || !s0.is_finite() { return Err(format!("{} : longueur nulle après retraits.", m.nom)); }
    let z = if m.depuis_haut { haut - m.hauteur } else { bas + m.hauteur };
    // creux : vers l'intérieur du mur ; ses sommets sur la face sortent d'1 mm pour une
    // découpe franche
    let dx = if t.creux { -1.0 } else { 1.0 };
    let xmin = section.iter().map(|v| v[0]).fold(f64::INFINITY, f64::min);
    let sortie = |x: f64| if t.creux && x <= xmin + 1e-9 { 1.0 } else { 0.0 };
    let n = match axe {
        Axe::Droit { .. } => 1,
        Axe::Arc { .. } => (axe_angle(&axe) / (5f64.to_radians())).ceil().max(1.0) as usize,
    };
    let sections: Vec<Vec<DVec3>> = (0..=n)
        .map(|k| {
            let s = s0 + (s1 - s0) * k as f64 / n as f64;
            let (q, u) = repere(&axe, s);
            let nrm = DVec2::new(-u.y, u.x) * signe;
            section.iter().map(|v| { let h = q + nrm * (dx * v[0] + t.decalage + sortie(v[0])); DVec3::new(h.x, h.y, z + v[1]) }).collect()
        })
        .collect();
    let mut tris = Vec::new();
    let c = section.len();
    for k in 0..n {
        let (a, b) = (&sections[k], &sections[k + 1]);
        for i in 0..c {
            let j = (i + 1) % c;
            tris.push([a[i], a[j], b[j]]);
            tris.push([a[i], b[j], b[i]]);
        }
    }
    let coords: Vec<f64> = section.iter().flatten().copied().collect();
    let indices = noyau::geometrie::earcut(&coords, &[], 2).ok_or("Section du profilé non triangulable.")?;
    for t in indices.as_chunks::<3>().0 {
        tris.push([sections[0][t[0]], sections[0][t[2]], sections[0][t[1]]]);
        tris.push([sections[n][t[0]], sections[n][t[1]], sections[n][t[2]]]);
    }
    // normales vers l'extérieur du solide, quel que soit le sens du repère
    let volume: f64 = tris.iter().map(|t| t[0].dot(t[1].cross(t[2]))).sum();
    if volume < 0.0 { for t in &mut tris { t.swap(1, 2); } }
    Ok(tris)
}

fn axe_angle(axe: &Axe) -> f64 {
    let Axe::Arc { debut, centre, .. } = *axe else { return 0.0 };
    axe.longueur() / (debut[0] - centre[0]).hypot(debut[1] - centre[1]).max(1e-9)
}

/// Retire du volume d'un mur les creux de son hôte (mur indépendant ou assemblage).
pub(crate) fn creuser(p: &Projet, mur: &Mur, mut tris: Vec<Tri>, booleen: Booleen) -> Result<Vec<Tri>, String> {
    let hote = mur.assemblage.as_deref().unwrap_or(&mur.id);
    for m in p.modenatures.iter().filter(|m| m.mur == hote && p.type_modenature(&m.type_modenature).is_some_and(|t| t.creux)) {
        if tris.is_empty() { break; }
        tris = booleen(&tris, &solide(p, m)?, noyau::csg::Op::Difference, 1e-5).ok_or("Profilé en creux trop complexe.")?;
    }
    Ok(tris)
}

/// Contrôle des profilés et de leurs types.
pub fn verifier(p: &Projet) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for t in &p.types_modenatures {
        if t.id.is_empty() || !ids.insert(t.id.as_str()) { return Err("Type de profilé : identifiant vide ou dupliqué.".into()); }
        if ![t.decalage, t.retrait].iter().all(|v| v.is_finite()) || t.retrait < 0.0 { return Err(format!("{} : décalage ou retrait non valide.", t.nom)); }
        t.section(p)?;
    }
    for m in &p.modenatures {
        if m.id.is_empty() || !ids.insert(m.id.as_str()) { return Err("Profilé : identifiant vide ou dupliqué.".into()); }
        let hote = p.murs.iter().any(|x| x.id == m.mur && x.assemblage.is_none()) || p.assemblage(&m.mur).is_some();
        if !hote { return Err(format!("{} : mur hôte absent.", m.nom)); }
        if !m.hauteur.is_finite() || m.debut.iter().chain(&m.fin).any(|v| !v.is_finite()) { return Err(format!("{} : position non valide.", m.nom)); }
        solide(p, m)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests_modenature.rs"]
mod tests;
