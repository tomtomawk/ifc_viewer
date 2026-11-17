//! Poutres : type (famille de profil ou section rectangulaire, dimensions, matériau),
//! pose de l'instance (justification horizontale, arase de référence) et solide.
//!
//! Trois niveaux, comme les murs : la famille de profil (`Projet::profils`) donne une
//! forme de section paramétrée ; le type de poutre choisit un profil (ou une section
//! rectangulaire), ses dimensions et son matériau ; l'instance (`Barre`) porte son axe,
//! son niveau, sa justification et son arase.
use crate::{Barre, GenreBarre, Projet};
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// Type de poutre. Sans profil : section rectangulaire `largeur` × `hauteur` (mm). Avec
/// profil : contour de la famille de profil, mis à l'échelle de `largeur` × `hauteur`
/// (encombrement de la section).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TypePoutre {
    pub id: String,
    pub nom: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profil: Option<String>,
    pub largeur: f64,
    pub hauteur: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
}

/// Position de la poutre par rapport à la ligne tracée, dans le sens du tracé.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Justification {
    /// La ligne tracée est la face gauche ; la poutre est à droite.
    Gauche,
    #[default]
    Axe,
    /// La ligne tracée est la face droite ; la poutre est à gauche.
    Droite,
}

impl Justification {
    pub const TOUTES: [Justification; 3] = [Justification::Gauche, Justification::Axe, Justification::Droite];

    pub fn libelle(self) -> &'static str {
        match self {
            Justification::Gauche => "Gauche",
            Justification::Axe => "Axe",
            Justification::Droite => "Droite",
        }
    }

    /// Justification suivante (Ctrl pendant le tracé).
    pub fn suivante(self) -> Self {
        match self {
            Justification::Gauche => Justification::Axe,
            Justification::Axe => Justification::Droite,
            Justification::Droite => Justification::Gauche,
        }
    }

    /// Décalage du milieu de la section depuis la ligne tracée, le long de la normale
    /// gauche (mm).
    pub fn decalage(self, largeur: f64) -> f64 {
        match self {
            Justification::Gauche => -largeur / 2.0,
            Justification::Axe => 0.0,
            Justification::Droite => largeur / 2.0,
        }
    }

    fn est_axe(&self) -> bool {
        *self == Justification::Axe
    }
}

/// Hauteur de référence d'une poutre : la cote Z de l'axe (`Barre::debut[2]`, relative au
/// niveau) désigne son arase supérieure, son axe ou son arase inférieure.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Arase {
    Superieure,
    #[default]
    Axe,
    Inferieure,
}

impl Arase {
    pub const TOUTES: [Arase; 3] = [Arase::Superieure, Arase::Axe, Arase::Inferieure];

    pub fn libelle(self) -> &'static str {
        match self {
            Arase::Superieure => "Arase supérieure",
            Arase::Axe => "Axe",
            Arase::Inferieure => "Arase inférieure",
        }
    }

    /// Bas de la section depuis la cote de référence, pour une hauteur `h`.
    pub fn bas(self, h: f64) -> f64 {
        match self {
            Arase::Superieure => -h,
            Arase::Axe => -h / 2.0,
            Arase::Inferieure => 0.0,
        }
    }

    fn est_axe(&self) -> bool {
        *self == Arase::Axe
    }
}

/// Pose d'une poutre (champs à plat dans le JSON de la barre).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct PosePoutre {
    /// Type de poutre ; None : section rectangulaire `largeur` × `hauteur` de la barre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_poutre: Option<String>,
    #[serde(default, skip_serializing_if = "Justification::est_axe")]
    pub justification: Justification,
    #[serde(default, skip_serializing_if = "Arase::est_axe")]
    pub arase: Arase,
}

impl Projet {
    pub fn type_poutre(&self, id: &str) -> Option<&TypePoutre> {
        self.types_poutres.iter().find(|t| t.id == id)
    }
}

/// Dimensions (largeur, hauteur) et contour de la section d'une poutre (mm, x vers la
/// gauche du tracé, y vers le haut ; encombrement centré sur l'origine).
pub fn section(p: &Projet, b: &Barre) -> Result<(f64, f64, Vec<[f64; 2]>), String> {
    let t = match &b.pose.type_poutre {
        Some(id) => Some(p.type_poutre(id).ok_or_else(|| format!("{} : type de poutre absent.", b.nom))?),
        None => None,
    };
    let (l, h) = t.map_or((b.largeur, b.hauteur), |t| (t.largeur, t.hauteur));
    if !(l.is_finite() && h.is_finite() && l > 0.0 && h > 0.0) {
        return Err(format!("{} : dimensions de section non valides.", b.nom));
    }
    let rectangle = || vec![[-l / 2.0, -h / 2.0], [l / 2.0, -h / 2.0], [l / 2.0, h / 2.0], [-l / 2.0, h / 2.0]];
    let contour = match t.and_then(|t| t.profil.as_ref()) {
        None => rectangle(),
        Some(id) => {
            let points = p.profils.iter().find(|x| &x.id == id).ok_or_else(|| format!("{} : profil absent.", b.nom))?.points()?;
            let (mn, mx) = points.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(a, b), q| ([a[0].min(q[0]), a[1].min(q[1])], [b[0].max(q[0]), b[1].max(q[1])]));
            let (dl, dh) = (mx[0] - mn[0], mx[1] - mn[1]);
            if !(dl > 1e-9 && dh > 1e-9) {
                return Err(format!("{} : profil sans surface.", b.nom));
            }
            points.iter().map(|q| [(q[0] - mn[0]) / dl * l - l / 2.0, (q[1] - mn[1]) / dh * h - h / 2.0]).collect()
        }
    };
    Ok((l, h, contour))
}

/// Ligne de la face gauche, de l'axe et de la face droite d'une poutre posée sur le
/// segment a → b : milieu de la section en plan (mm), compte tenu de la justification.
pub fn axe_reel(a: DVec2, b: DVec2, largeur: f64, justification: Justification) -> Option<(DVec2, DVec2)> {
    let n = (b - a).try_normalize()?.perp();
    let d = n * justification.decalage(largeur);
    Some((a + d, b + d))
}

/// Solide d'une poutre (mm, repère du projet) : section extrudée le long de son axe,
/// justifiée en plan et calée sur son arase.
pub fn solide(p: &Projet, b: &Barre) -> Result<Vec<[DVec3; 3]>, String> {
    debug_assert_eq!(b.genre, GenreBarre::Poutre);
    let z = p.niveau(&b.niveau).ok_or("Niveau de la structure absent.")?.altitude;
    if b.debut.iter().chain(&b.fin).any(|v| !v.is_finite()) {
        return Err("Dimensions de structure non valides.".into());
    }
    if (b.fin[2] - b.debut[2]).abs() > 1e-6 {
        return Err("La poutre doit être horizontale et avoir une longueur positive.".into());
    }
    let (a, f) = (DVec2::new(b.debut[0], b.debut[1]), DVec2::new(b.fin[0], b.fin[1]));
    let (largeur, h, contour) = section(p, b)?;
    let (a, f) = axe_reel(a, f, largeur, b.pose.justification).ok_or("La poutre doit être horizontale et avoir une longueur positive.")?;
    let dir = (f - a).normalize();
    let longueur = a.distance(f);
    // repère de la section : x vers la gauche du tracé, y vers le haut, z le long de l'axe
    let (ux, uy, uz) = (dir.perp().extend(0.0), DVec3::Z, dir.extend(0.0));
    let origine = a.extend(z + b.debut[2] + b.pose.arase.bas(h) + h / 2.0);
    let mut contour = contour;
    // prisme extrudé selon +z local : contour dans le sens direct vu depuis +z
    let aire: f64 = (0..contour.len()).map(|i| { let (p, q) = (contour[i], contour[(i + 1) % contour.len()]); p[0] * q[1] - q[0] * p[1] }).sum();
    if aire < 0.0 { contour.reverse(); }
    Ok(crate::volumes::solides::prisme(&contour, 0.0, longueur).into_iter()
        .map(|t| t.map(|q| origine + ux * q.x + uy * q.y + uz * q.z))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometrie::volume;

    fn projet() -> Projet {
        let mut p = Projet::depuis_gabarit("Poutres", &crate::gabarit::Gabarit::defaut());
        p.types_poutres.push(TypePoutre { id: "t".into(), nom: "Poutre BA 20×50".into(), profil: None, largeur: 200.0, hauteur: 500.0, materiau: Some("Béton armé".into()) });
        p
    }

    fn poutre(p: &Projet, pose: PosePoutre, z: f64) -> Barre {
        Barre { id: "b".into(), nom: "P1".into(), genre: GenreBarre::Poutre, niveau: p.niveaux[0].id.clone(), debut: [0.0, 0.0, z], fin: [4000.0, 0.0, z], largeur: 100.0, hauteur: 100.0, rotation: 0.0, pose, origine: None }
    }

    fn etendue(t: &[[DVec3; 3]]) -> (DVec3, DVec3) {
        t.iter().flatten().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), q| (a.min(*q), b.max(*q)))
    }

    /// N° 10, 7, 8 : la section vient du type ; justification gauche / droite décalent
    /// la poutre d'une demi-largeur de part et d'autre de la ligne ; l'arase supérieure
    /// met le dessus à la cote, l'inférieure le dessous.
    #[test]
    fn type_justification_et_arase() {
        let p = projet();
        let pose = |justification, arase| PosePoutre { type_poutre: Some("t".into()), justification, arase };
        let b = poutre(&p, pose(Justification::Axe, Arase::Axe), 2500.0);
        let t = solide(&p, &b).unwrap();
        assert!((volume(&t) - 4000.0 * 200.0 * 500.0).abs() < 1.0);
        let (mn, mx) = etendue(&t);
        assert!((mn.y + 100.0).abs() < 1e-6 && (mx.y - 100.0).abs() < 1e-6);
        assert!((mn.z - 2250.0).abs() < 1e-6 && (mx.z - 2750.0).abs() < 1e-6);
        // tracé vers +x : la gauche est +y
        let (mn, mx) = etendue(&solide(&p, &poutre(&p, pose(Justification::Gauche, Arase::Superieure), 2500.0)).unwrap());
        assert!((mn.y + 200.0).abs() < 1e-6 && mx.y.abs() < 1e-6, "gauche : la poutre est à droite de la ligne");
        assert!((mx.z - 2500.0).abs() < 1e-6 && (mn.z - 2000.0).abs() < 1e-6);
        let (mn, mx) = etendue(&solide(&p, &poutre(&p, pose(Justification::Droite, Arase::Inferieure), 2500.0)).unwrap());
        assert!(mn.y.abs() < 1e-6 && (mx.y - 200.0).abs() < 1e-6);
        assert!((mn.z - 2500.0).abs() < 1e-6 && (mx.z - 3000.0).abs() < 1e-6);
    }

    /// Profil de famille : contour mis à l'encombrement du type ; volume positif.
    #[test]
    fn profil_de_famille_a_l_echelle_du_type() {
        let mut p = projet();
        let profil = p.profils.first().cloned().or_else(|| crate::gabarit::Gabarit::defaut().profils.first().cloned()).unwrap();
        if p.profils.is_empty() { p.profils.push(profil.clone()); }
        p.types_poutres.push(TypePoutre { id: "pf".into(), nom: "Profil".into(), profil: Some(profil.id.clone()), largeur: 150.0, hauteur: 300.0, materiau: None });
        let b = poutre(&p, PosePoutre { type_poutre: Some("pf".into()), ..Default::default() }, 2500.0);
        let t = solide(&p, &b).unwrap();
        let (mn, mx) = etendue(&t);
        assert!(((mx.y - mn.y) - 150.0).abs() < 1e-6 && ((mx.z - mn.z) - 300.0).abs() < 1e-6);
        assert!(volume(&t) > 0.0);
    }

    /// Le JSON d'une poutre sans pose particulière ne change pas ; la pose s'écrit à plat.
    #[test]
    fn pose_a_plat_dans_le_json() {
        let p = projet();
        let b = poutre(&p, PosePoutre::default(), 0.0);
        let j = serde_json::to_value(&b).unwrap();
        assert!(j.get("justification").is_none() && j.get("type_poutre").is_none());
        let b = poutre(&p, PosePoutre { type_poutre: Some("t".into()), justification: Justification::Gauche, arase: Arase::Superieure }, 0.0);
        let j = serde_json::to_value(&b).unwrap();
        assert_eq!(j["justification"], "gauche");
        assert_eq!(j["arase"], "superieure");
        let relu: Barre = serde_json::from_value(j).unwrap();
        assert_eq!(relu.pose, b.pose);
    }
}
