//! Annotations dessinées sur une feuille : textes et dessin 2D en mm du papier.
use super::*;

/// Hauteur des textes posés sur une feuille, par défaut (mm du papier).
pub const HAUTEUR_TEXTE_FEUILLE: f64 = 3.5;

fn hauteur_texte_feuille() -> f64 {
    HAUTEUR_TEXTE_FEUILLE
}

/// Cartouche posé sur une feuille : famille (nom ; vide : cartouche standard) et coin
/// bas gauche (mm du papier).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CartouchePlace {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub famille: String,
    pub position: [f64; 2],
}

/// Annotation d'une feuille : forme en mm du papier (origine en bas à gauche) ; textes,
/// lignes, polylignes, splines, cercles, arcs et zones. Les champs `{projet}`,
/// `{feuille}` et `{numero}` des textes sont remplis.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnnotationFeuille {
    /// Hauteur des textes (mm du papier).
    #[serde(default = "hauteur_texte_feuille")]
    pub hauteur: f64,
    #[serde(flatten)]
    pub forme: AnnotationForme,
}

impl AnnotationFeuille {
    /// Points de la forme (mm du papier), pour la déplacer ; vide pour une forme qui ne
    /// se pose pas sur une feuille (cotes, étiquettes).
    pub fn points_mut(&mut self) -> Vec<&mut [f64; 2]> {
        match &mut self.forme {
            AnnotationForme::Texte { point, .. } => vec![point],
            AnnotationForme::Ligne { debut, fin } => vec![debut, fin],
            AnnotationForme::Zone { contour: points } | AnnotationForme::Polyligne { points, .. } | AnnotationForme::Spline { points, .. } => points.iter_mut().collect(),
            AnnotationForme::Cercle { centre, .. } => vec![centre],
            AnnotationForme::Arc { points } => points.iter_mut().collect(),
            _ => Vec::new(),
        }
    }

    /// Déplace l'annotation de `d` (mm du papier).
    pub fn translater(&mut self, d: [f64; 2]) {
        for q in self.points_mut() {
            q[0] += d[0];
            q[1] += d[1];
        }
    }

    /// Forme admise sur une feuille.
    pub fn admise(&self) -> bool {
        matches!(self.forme, AnnotationForme::Texte { .. } | AnnotationForme::Ligne { .. } | AnnotationForme::Zone { .. } | AnnotationForme::Polyligne { .. } | AnnotationForme::Spline { .. } | AnnotationForme::Cercle { .. } | AnnotationForme::Arc { .. })
    }

    /// Tracé de la forme en polylignes (mm du papier) ; le cadre du texte pour un texte.
    pub fn polylignes(&self, f: &Feuille, p: &Projet) -> Vec<Vec<DVec2>> {
        match &self.forme {
            AnnotationForme::Texte { .. } => self.cadre_texte(f, p).map(|[x0, y0, x1, y1]| vec![vec![DVec2::new(x0, y0), DVec2::new(x1, y0), DVec2::new(x1, y1), DVec2::new(x0, y1), DVec2::new(x0, y0)]]).unwrap_or_default(),
            AnnotationForme::Zone { contour } => vec![contour.iter().chain(contour.first()).map(|q| DVec2::from_array(*q)).collect()],
            forme => forme.forme_2d().map(|g| g.polylignes()).unwrap_or_default(),
        }
    }

    /// Cadre d'un texte [x0, y0, x1, y1] (mm du papier), contenu rempli.
    pub fn cadre_texte(&self, f: &Feuille, p: &Projet) -> Option<[f64; 4]> {
        let AnnotationForme::Texte { point, texte } = &self.forme else { return None };
        let l = noyau::texte::largeur(&contenu_texte_feuille(p, f, texte), "Arial", self.hauteur, false).max(self.hauteur);
        Some([point[0], point[1], point[0] + l, point[1] + self.hauteur])
    }

    /// Distance (mm du papier) du point `q` à l'annotation : 0 dans le cadre d'un texte.
    pub fn distance(&self, f: &Feuille, p: &Projet, q: [f64; 2]) -> f64 {
        let q = DVec2::from_array(q);
        if let Some([x0, y0, x1, y1]) = self.cadre_texte(f, p) {
            let d = DVec2::new((x0 - q.x).max(q.x - x1).max(0.0), (y0 - q.y).max(q.y - y1).max(0.0));
            return d.length();
        }
        self.polylignes(f, p).iter().flat_map(|l| l.windows(2)).map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let t = if ab.length_squared() > 0.0 { ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            (a + ab * t).distance(q)
        }).fold(f64::INFINITY, f64::min)
    }

    /// Étendue [x0, y0, x1, y1] (mm du papier).
    pub fn etendue(&self, f: &Feuille, p: &Projet) -> Option<[f64; 4]> {
        let pts: Vec<DVec2> = self.polylignes(f, p).into_iter().flatten().collect();
        let first = pts.first()?;
        Some(pts.iter().fold([first.x, first.y, first.x, first.y], |e, q| [e[0].min(q.x), e[1].min(q.y), e[2].max(q.x), e[3].max(q.y)]))
    }
}

/// Texte d'une annotation de feuille, champs remplis.
pub fn contenu_texte_feuille(p: &Projet, f: &Feuille, texte: &str) -> String {
    remplir_champs(texte, &[("{projet}", &p.nom), ("{feuille}", &f.nom), ("{numero}", &f.numero)])
}

/// Primitives d'une annotation de feuille, en mètres du papier.
pub fn primitives_annotation_feuille(p: &Projet, f: &Feuille, a: &AnnotationFeuille) -> Vec<Primitive> {
    match &a.forme {
        AnnotationForme::Texte { point, texte: s } => {
            let contenu = contenu_texte_feuille(p, f, s);
            if contenu.is_empty() { return Vec::new(); }
            vec![texte(&contenu, pt_papier(point[0], point[1]), DVec3::X, DVec3::Y, a.hauteur / 1000.0, [20, 20, 20, 255])]
        }
        _ if !a.admise() => Vec::new(),
        forme => forme_cartouche(p, forme),
    }
}
