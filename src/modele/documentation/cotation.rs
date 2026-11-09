//! Méthode de cotation commune au projet (vues), au cartouche et à l'éditeur de
//! familles : outils, nombre de points, consignes, construction de la cote depuis les
//! points cliqués et géométrie de rendu. Chaque éditeur ne fait que recueillir les
//! points (avec ses accrochages) et convertir la géométrie dans son repère.
use super::*;

/// Outil de cote.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GenreCote {
    /// Longueur : deux points, position de la ligne de cote.
    Lineaire,
    /// Angle : sommet, un point sur chaque branche, position de l'arc.
    #[serde(alias = "angle")]
    Angulaire,
    /// Diamètre : centre, point du cercle.
    Diametre,
    /// Altimétrie : point coté, position du texte.
    Altimetrie,
}

impl GenreCote {
    pub fn nom(self) -> &'static str {
        match self { Self::Lineaire => "Longueur", Self::Angulaire => "Angle", Self::Diametre => "Diamètre", Self::Altimetrie => "Altimétrie" }
    }

    /// Nombre de points à cliquer (le dernier place la cote).
    pub fn points(self) -> usize {
        match self { Self::Lineaire => 3, Self::Angulaire => 4, Self::Diametre | Self::Altimetrie => 2 }
    }

    /// Consigne de l'étape `k` (points déjà donnés).
    pub fn invite(self, k: usize) -> &'static str {
        match self {
            Self::Lineaire => ["Premier point à coter", "Second point à coter", "Position de la ligne de cote"][k.min(2)],
            Self::Angulaire => ["Sommet de l'angle", "Point sur la première branche", "Point sur la seconde branche", "Position de l'arc de cote"][k.min(3)],
            Self::Diametre => ["Centre du cercle", "Point du cercle"][k.min(1)],
            Self::Altimetrie => ["Point dont l'altitude est cotée", "Position du texte"][k.min(1)],
        }
    }

    /// Cote construite depuis ses points (`points()` points, repère de la vue, mm) ;
    /// altimétrie : référence (niveau sous le point) et décimales. None : points
    /// confondus ou alignés.
    pub fn forme(self, pts: &[DVec2], relative_niveau: bool, decimales: u8) -> Option<AnnotationForme> {
        let a = |p: DVec2| p.to_array();
        Some(match (self, pts) {
            (Self::Lineaire, [p, q, pos]) => {
                let d = (*q - *p).try_normalize()?;
                AnnotationForme::Cote { debut: PointCote::Fixe { point: a(*p) }, fin: PointCote::Fixe { point: a(*q) }, decalage: (*pos - *p).dot(d.perp()) }
            }
            (Self::Angulaire, [s, p, q, pos]) => {
                angle_branches(*s, *p, *q)?;
                AnnotationForme::CoteAngulaire { sommet: a(*s), debut: a(*p), fin: a(*q), rayon: pos.distance(*s).max(1e-3) }
            }
            (Self::Diametre, [c, p]) if c.distance(*p) > 1e-6 => AnnotationForme::CoteDiametre { centre: a(*c), point: a(*p) },
            (Self::Altimetrie, [p, r]) => AnnotationForme::Altimetrie { point: a(*p), repere: a(*r), relative_niveau, decimales },
            _ => return None,
        })
    }

    /// Points de l'aperçu : ceux donnés et le curseur ; un angle dont l'arc n'est pas
    /// encore placé prend un rayon provisoire (moitié de la branche la plus courte).
    pub fn points_apercu(self, donnes: &[DVec2], curseur: DVec2) -> Option<Vec<DVec2>> {
        let mut pts = donnes.to_vec();
        pts.push(curseur);
        if self == Self::Angulaire && pts.len() == 3 {
            let r = pts[1].distance(pts[0]).min(pts[2].distance(pts[0])) * 0.5;
            pts.push(pts[0] + DVec2::X * r);
        }
        (pts.len() == self.points()).then_some(pts)
    }

    /// Genre d'une cote existante.
    pub fn de_forme(forme: &AnnotationForme) -> Option<GenreCote> {
        Some(match forme {
            AnnotationForme::Cote { .. } => Self::Lineaire,
            AnnotationForme::CoteAngulaire { .. } => Self::Angulaire,
            AnnotationForme::CoteDiametre { .. } => Self::Diametre,
            AnnotationForme::Altimetrie { .. } => Self::Altimetrie,
            _ => return None,
        })
    }
}

/// Habillage d'une cote dans son repère (mm du repère) : longueur des traits obliques,
/// hauteur du texte, écriture des longueurs et altitude d'un point.
pub struct StyleCote<'a> {
    pub oblique: f64,
    pub taille: f64,
    pub longueur: &'a dyn Fn(f64) -> String,
    pub altitude: &'a dyn Fn([f64; 2], bool) -> f64,
}

/// Géométrie d'une cote, quel qu'en soit le genre : traits, et textes (milieu de la
/// ligne de base, direction, contenu). Cote linéaire : points déjà résolus
/// (`PointCote::Fixe`), sinon `debut`/`fin` sont à fournir par l'appelant.
pub fn geometrie(forme: &AnnotationForme, extremites: Option<(DVec2, DVec2)>, style: &StyleCote) -> Option<(Vec<[DVec2; 2]>, Vec<(DVec2, DVec2, String)>)> {
    let x = |q: &[f64; 2]| DVec2::from_array(*q);
    let marge = style.taille;
    let g = match forme {
        AnnotationForme::Cote { debut, fin, decalage } => {
            let (a, b) = match (debut, fin, extremites) {
                (_, _, Some(e)) => e,
                (PointCote::Fixe { point: a }, PointCote::Fixe { point: b }, None) => (x(a), x(b)),
                _ => return None,
            };
            let g = geometrie_cote(&[a, b], *decalage, style.oblique);
            let d = (b - a).normalize_or(DVec2::X);
            let dir = if d.x < -1e-9 || (d.x.abs() < 1e-9 && d.y < 0.0) { -d } else { d };
            let textes = g.textes.into_iter().map(|(p, v)| (p + dir.perp() * marge * 0.3, dir, (style.longueur)(v))).collect();
            return Some((g.traits, textes));
        }
        AnnotationForme::CoteAngulaire { sommet, debut, fin, rayon } => geometrie_angle(x(sommet), x(debut), x(fin), *rayon, style.oblique, marge)?,
        AnnotationForme::CoteDiametre { centre, point } => {
            let mut g = geometrie_diametre(x(centre), x(point), style.oblique, marge)?;
            g.texte.2 = format!("Ø {}", (style.longueur)(2.0 * x(centre).distance(x(point))));
            g
        }
        AnnotationForme::Altimetrie { point, repere, relative_niveau, decimales } => {
            geometrie_altimetrie(x(point), x(repere), (style.altitude)(*point, *relative_niveau), *decimales, style.taille)
        }
        _ => return None,
    };
    Some((g.traits, vec![g.texte]))
}
