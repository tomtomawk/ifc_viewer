//! Primitives 2D vectorielles : couche commune entre ce qui produit un dessin (coupe du
//! modèle, arêtes vues, éléments 2D de Revit) et ce qui le consomme (affichage par le GPU,
//! export SVG). Sur le modèle de la liste d'affichage de CADCraft : chaque primitive porte
//! sa forme, son style (couleur, épaisseur en mm du papier), son rôle dans le dessin et
//! l'élément dont elle vient.
//!
//! Les points sont en 3D (mètres), dans le plan du dessin ; l'export les projette sur ses
//! axes `droite` et `haut`.

use crate::step::Id;
use glam::DVec3;
use std::collections::HashMap;
use std::fmt::Write;

/// Forme d'une primitive.
#[derive(Clone, Debug)]
pub enum Forme {
    /// Suite de points, fermée ou non.
    Polyligne { points: Vec<DVec3>, fermee: bool },
    /// Segments indépendants (hachures).
    Segments(Vec<[DVec3; 2]>),
    /// Surface pleine : contours extérieurs et trous (règle pair-impair).
    Surface(Vec<Vec<DVec3>>),
    /// Surface déjà triangulée (glyphes des textes, traits des cotes et des cartouches).
    Triangles(Vec<[DVec3; 3]>),
}

/// Rôle d'une primitive dans le dessin : ordre de tracé et épaisseur par défaut.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Remplissage des parties coupées.
    Poche,
    /// Hachures (des parties coupées, des régions remplies).
    Hachure,
    /// Arêtes vues au-delà du plan du dessin.
    Vu,
    /// Contour des parties coupées.
    Coupe,
    /// Traits, textes, cotes et symboles des vues.
    Annotation,
}

impl Role {
    /// Ordre de tracé (du fond vers le dessus).
    pub fn rang(self) -> u8 {
        match self {
            Role::Poche => 0,
            Role::Hachure => 1,
            Role::Vu => 2,
            Role::Coupe => 3,
            Role::Annotation => 4,
        }
    }

    /// Épaisseur à l'export d'un trait fin (épaisseur nulle), mm du papier.
    pub fn trait_fin(self) -> f64 {
        match self {
            Role::Hachure => 0.09,
            Role::Vu => 0.13,
            _ => 0.18,
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Role::Poche => "poche",
            Role::Hachure => "hachure",
            Role::Vu => "vu",
            Role::Coupe => "coupe",
            Role::Annotation => "annotation",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub couleur: [u8; 4],
    /// Épaisseur des traits, mm du papier ; 0 : trait fin (un pixel à l'écran).
    pub epaisseur: f64,
}

#[derive(Clone, Debug)]
pub struct Primitive {
    pub forme: Forme,
    pub style: Style,
    pub role: Role,
    /// Élément du modèle dont vient la primitive (sélection, regroupement à l'export).
    pub element: Option<Id>,
}

/// Maillage d'une primitive pour le GPU : triangles et traits d'un pixel.
#[derive(Default)]
pub struct Maillage {
    pub triangles: Vec<[DVec3; 3]>,
    pub traits: Vec<[DVec3; 2]>,
}

impl Primitive {
    /// Copie déplacée de `d`.
    pub fn translater(&self, d: DVec3) -> Primitive {
        let forme = match &self.forme {
            Forme::Polyligne { points, fermee } => Forme::Polyligne { points: points.iter().map(|p| *p + d).collect(), fermee: *fermee },
            Forme::Segments(s) => Forme::Segments(s.iter().map(|s| s.map(|p| p + d)).collect()),
            Forme::Surface(c) => Forme::Surface(c.iter().map(|c| c.iter().map(|p| *p + d).collect()).collect()),
            Forme::Triangles(t) => Forme::Triangles(t.iter().map(|t| t.map(|p| p + d)).collect()),
        };
        Primitive { forme, ..self.clone() }
    }

    /// Triangles et traits de la primitive dans le plan de normale `normale`, à l'échelle
    /// `echelle` (dénominateur) : traits épais en rubans, prolongés d'une demi-largeur à
    /// chaque bout pour fermer les angles.
    pub fn mailler(&self, normale: DVec3, echelle: f64) -> Maillage {
        self.mailler_raccords(normale, echelle, None)
    }

    /// Comme `mailler`, les bouts des traits épais prolongés selon les traits qu'ils
    /// rejoignent (`raccords`, construits à la même échelle).
    pub fn mailler_raccords(&self, normale: DVec3, echelle: f64, raccords: Option<&Raccords>) -> Maillage {
        let mut m = Maillage::default();
        let largeur = self.style.epaisseur * echelle / 1000.0;
        let trait_ = |a: DVec3, b: DVec3, m: &mut Maillage| {
            if largeur <= 0.0 {
                m.traits.push([a, b]);
            } else {
                let h = largeur / 2.0;
                let (ea, eb) = raccords.map_or((h, h), |r| (r.prolongement(a, b, h), r.prolongement(b, a, h)));
                ruban(a, b, normale, largeur, ea, eb, &mut m.triangles);
            }
        };
        match &self.forme {
            Forme::Polyligne { points, fermee } => {
                for w in points.windows(2) {
                    trait_(w[0], w[1], &mut m);
                }
                if *fermee && points.len() > 2 {
                    trait_(points[points.len() - 1], points[0], &mut m);
                }
            }
            Forme::Segments(s) => {
                for [a, b] in s {
                    trait_(*a, *b, &mut m);
                }
            }
            Forme::Surface(contours) => m.triangles = trianguler(contours, normale),
            Forme::Triangles(t) => m.triangles = t.clone(),
        }
        m
    }
}

/// Rectangle de largeur `l` le long de [a, b] dans le plan de normale `n`, prolongé de
/// `ea` au début et `eb` à la fin.
fn ruban(a: DVec3, b: DVec3, n: DVec3, l: f64, ea: f64, eb: f64, out: &mut Vec<[DVec3; 3]>) {
    let d = b - a;
    let cote = n.cross(d);
    if d.length() < 1e-12 || cote.length() < 1e-12 {
        return;
    }
    let u = d.normalize();
    let c = cote.normalize() * (l / 2.0);
    let (p0, p1, p2, p3) = (a - u * ea - c, b + u * eb - c, b + u * eb + c, a - u * ea + c);
    out.push([p0, p1, p2]);
    out.push([p0, p2, p3]);
}

/// Raccords des traits épais d'un dessin. Un trait prolongé de sa propre demi-largeur
/// (bout carré) dépasse d'un trait plus fin qu'il rejoint : chaque bout est prolongé de
/// la demi-largeur du trait qu'il rejoint (le plus épais s'il y en a plusieurs), si
/// bien que deux traits d'épaisseurs différentes se raccordent sans débord.
pub struct Raccords {
    cellule: f64,
    grille: HashMap<[i64; 3], Vec<usize>>,
    /// Segments épais et leur demi-largeur (m).
    segments: Vec<([DVec3; 2], f64)>,
}

impl Raccords {
    pub fn new(prims: &[Primitive], echelle: f64) -> Raccords {
        let mut segments = Vec::new();
        for p in prims.iter().filter(|p| p.style.epaisseur > 0.0) {
            let h = p.style.epaisseur * echelle / 2000.0;
            match &p.forme {
                Forme::Polyligne { points, fermee } => {
                    segments.extend(points.windows(2).map(|w| ([w[0], w[1]], h)));
                    if *fermee && points.len() > 2 {
                        segments.push(([points[points.len() - 1], points[0]], h));
                    }
                }
                Forme::Segments(s) => segments.extend(s.iter().map(|s| (*s, h))),
                _ => {}
            }
        }
        let cellule = 0.5;
        let mut grille: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
        let case = |p: DVec3| (p / cellule).floor().as_i64vec3().to_array();
        for (k, ([a, b], _)) in segments.iter().enumerate() {
            // cases traversées (pas d'une demi-case) et leurs voisines
            let n = ((*b - *a).length() / (cellule * 0.5)).ceil().min(100_000.0) as usize + 1;
            let mut vues = std::collections::HashSet::new();
            for i in 0..=n {
                let c = case(a.lerp(*b, i as f64 / n as f64));
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            vues.insert([c[0] + dx, c[1] + dy, c[2] + dz]);
                        }
                    }
                }
            }
            for c in vues {
                grille.entry(c).or_default().push(k);
            }
        }
        Raccords { cellule, grille, segments }
    }

    /// Prolongement du bout `p` du trait [p, q] de demi-largeur `h` : demi-largeur du plus
    /// épais des traits qu'il rejoint (bout ou milieu) ; 0 dans le prolongement d'un trait
    /// aligné ; `h` s'il ne rejoint rien.
    pub fn prolongement(&self, p: DVec3, q: DVec3, h: f64) -> f64 {
        let Some(ks) = self.grille.get(&(p / self.cellule).floor().as_i64vec3().to_array()) else { return h };
        let d = (q - p).normalize_or_zero();
        let tol = 1e-5;
        let (mut ext, mut aligne) = (None::<f64>, false);
        for &k in ks {
            let ([a, b], h2) = self.segments[k];
            let ab = b - a;
            let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            if (a + ab * t).distance(p) > tol {
                continue;
            }
            let e = ab.normalize_or_zero();
            if d.cross(e).length() < 0.05 {
                // même direction : le trait lui-même, un doublon ou un trait aligné ; ce
                // dernier ne compte que s'il continue au-delà du bout
                let au_dela = [a, b].iter().any(|x| (*x - p).dot(d) < -tol);
                aligne |= au_dela;
                continue;
            }
            ext = Some(ext.map_or(h2, |x: f64| x.max(h2)));
        }
        ext.unwrap_or(if aligne { 0.0 } else { h })
    }
}

/// Repère orthonormé (x, y) d'un plan de normale `n`.
fn axes(n: DVec3) -> (DVec3, DVec3) {
    let r = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let x = (r - n * r.dot(n)).normalize();
    (x, n.cross(x))
}

/// Triangulation de contours plans (pair-impair) dans le plan de normale `n`.
fn trianguler(contours: &[Vec<DVec3>], n: DVec3) -> Vec<[DVec3; 3]> {
    let Some(&p0) = contours.iter().flatten().next() else { return Vec::new() };
    let (x, y) = axes(n);
    let plans: Vec<Vec<[f32; 2]>> = contours.iter().map(|c| c.iter().map(|p| [(*p - p0).dot(x) as f32, (*p - p0).dot(y) as f32]).collect()).collect();
    let profondeur = |p: [f32; 2]| p0 + x * p[0] as f64 + y * p[1] as f64;
    crate::texte::trianguler(plans).into_iter().map(|t| t.map(profondeur)).collect()
}

/// Dessin : primitives d'un plan de vue, orienté par `droite` et `haut` (vecteurs
/// unitaires), à l'échelle `echelle`.
pub struct Dessin {
    pub droite: DVec3,
    pub haut: DVec3,
    pub echelle: f64,
    pub primitives: Vec<Primitive>,
}

impl Dessin {
    /// Coordonnées sur le papier (mm, y vers le bas comme en SVG) d'un point.
    fn papier(&self, p: DVec3) -> [f64; 2] {
        let k = 1000.0 / self.echelle;
        // + 0.0 : pas de « -0.000 » dans le fichier
        [p.dot(self.droite) * k + 0.0, -p.dot(self.haut) * k + 0.0]
    }

    /// Document SVG, unités en mm du papier : primitives regroupées par rôle (un groupe
    /// par rôle, dans l'ordre de tracé), marge de 10 mm autour du dessin.
    pub fn svg(&self, titre: &str) -> String {
        let (mut mn, mut mx) = ([f64::MAX; 2], [f64::MIN; 2]);
        let mut etendre = |p: [f64; 2]| {
            mn = [mn[0].min(p[0]), mn[1].min(p[1])];
            mx = [mx[0].max(p[0]), mx[1].max(p[1])];
        };
        for pr in &self.primitives {
            match &pr.forme {
                Forme::Polyligne { points, .. } => points.iter().for_each(|p| etendre(self.papier(*p))),
                Forme::Segments(s) => s.iter().flatten().for_each(|p| etendre(self.papier(*p))),
                Forme::Surface(c) => c.iter().flatten().for_each(|p| etendre(self.papier(*p))),
                Forme::Triangles(t) => t.iter().flatten().for_each(|p| etendre(self.papier(*p))),
            }
        }
        if mn[0] > mx[0] {
            (mn, mx) = ([0.0; 2], [1.0; 2]);
        }
        let marge = 10.0;
        let (x0, y0, l, h) = (mn[0] - marge, mn[1] - marge, mx[0] - mn[0] + 2.0 * marge, mx[1] - mn[1] + 2.0 * marge);
        let mut s = String::new();
        let _ = writeln!(s, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        let _ = writeln!(
            s,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{l:.2}mm" height="{h:.2}mm" viewBox="{x0:.3} {y0:.3} {l:.3} {h:.3}" stroke-linecap="square" stroke-linejoin="miter">"#
        );
        let _ = writeln!(s, "<title>{}</title>", echapper(titre));
        let _ = writeln!(s, r#"<rect x="{x0:.3}" y="{y0:.3}" width="{l:.3}" height="{h:.3}" fill="white"/>"#);
        let raccords = Raccords::new(&self.primitives, self.echelle);
        let mut ordre: Vec<&Primitive> = self.primitives.iter().collect();
        ordre.sort_by_key(|p| p.role.rang());
        let mut role_ouvert = None;
        for pr in ordre {
            if role_ouvert != Some(pr.role) {
                if role_ouvert.is_some() {
                    s.push_str("</g>\n");
                }
                let _ = writeln!(s, r#"<g id="{}">"#, pr.role.nom());
                role_ouvert = Some(pr.role);
            }
            self.svg_primitive(pr, &raccords, &mut s);
        }
        if role_ouvert.is_some() {
            s.push_str("</g>\n");
        }
        s.push_str("</svg>\n");
        s
    }

    fn svg_primitive(&self, pr: &Primitive, raccords: &Raccords, s: &mut String) {
        let c = pr.style.couleur;
        let couleur = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        let opacite = if c[3] < 255 { format!(r#" opacity="{:.3}""#, c[3] as f64 / 255.0) } else { String::new() };
        let epaisseur = if pr.style.epaisseur > 0.0 { pr.style.epaisseur } else { pr.role.trait_fin() };
        let element = pr.element.map(|e| format!(r#" data-element="{e}""#)).unwrap_or_default();
        let pts = |ps: &[DVec3]| ps.iter().map(|p| self.papier(*p)).map(|[x, y]| format!("{x:.3},{y:.3}")).collect::<Vec<_>>().join(" ");
        // traits épais : segments à bouts francs, prolongés selon les traits rejoints
        let segments: Option<Vec<[DVec3; 2]>> = match &pr.forme {
            _ if pr.style.epaisseur <= 0.0 => None,
            Forme::Polyligne { points, fermee } => {
                let mut v: Vec<[DVec3; 2]> = points.windows(2).map(|w| [w[0], w[1]]).collect();
                if *fermee && points.len() > 2 {
                    v.push([points[points.len() - 1], points[0]]);
                }
                Some(v)
            }
            Forme::Segments(seg) => Some(seg.clone()),
            _ => None,
        };
        if let Some(seg) = segments {
            let h = epaisseur * self.echelle / 2000.0;
            let mut d = String::new();
            for [a, b] in seg {
                let u = (b - a).normalize_or_zero();
                let (a2, b2) = (a - u * raccords.prolongement(a, b, h), b + u * raccords.prolongement(b, a, h));
                let ([xa, ya], [xb, yb]) = (self.papier(a2), self.papier(b2));
                let _ = write!(d, "M{xa:.3} {ya:.3}L{xb:.3} {yb:.3}");
            }
            let _ = writeln!(s, r#"<path d="{d}" fill="none" stroke="{couleur}" stroke-width="{epaisseur:.3}" stroke-linecap="butt"{opacite}{element}/>"#);
            return;
        }
        match &pr.forme {
            Forme::Polyligne { points, fermee } => {
                let balise = if *fermee { "polygon" } else { "polyline" };
                let _ = writeln!(s, r#"<{balise} points="{}" fill="none" stroke="{couleur}" stroke-width="{epaisseur:.3}"{opacite}{element}/>"#, pts(points));
            }
            Forme::Segments(seg) => {
                let mut d = String::new();
                for [a, b] in seg {
                    let ([xa, ya], [xb, yb]) = (self.papier(*a), self.papier(*b));
                    let _ = write!(d, "M{xa:.3} {ya:.3}L{xb:.3} {yb:.3}");
                }
                let _ = writeln!(s, r#"<path d="{d}" fill="none" stroke="{couleur}" stroke-width="{epaisseur:.3}"{opacite}{element}/>"#);
            }
            Forme::Surface(contours) => {
                let mut d = String::new();
                for c in contours {
                    for (k, p) in c.iter().enumerate() {
                        let [x, y] = self.papier(*p);
                        let _ = write!(d, "{}{x:.3} {y:.3}", if k == 0 { "M" } else { "L" });
                    }
                    d.push('Z');
                }
                let _ = writeln!(s, r#"<path d="{d}" fill="{couleur}" fill-rule="evenodd" stroke="none"{opacite}{element}/>"#);
            }
            Forme::Triangles(t) => {
                // un chemin par primitive ; contour de la couleur du fond pour masquer les
                // jointures entre triangles
                let mut d = String::new();
                for tri in t {
                    let [[xa, ya], [xb, yb], [xc, yc]] = tri.map(|p| self.papier(p));
                    let _ = write!(d, "M{xa:.3} {ya:.3}L{xb:.3} {yb:.3}L{xc:.3} {yc:.3}Z");
                }
                let _ = writeln!(s, r#"<path d="{d}" fill="{couleur}" stroke="{couleur}" stroke-width="0.02"{opacite}{element}/>"#);
            }
        }
    }
}

fn echapper(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trait_(points: Vec<DVec3>, epaisseur: f64, role: Role) -> Primitive {
        Primitive { forme: Forme::Polyligne { points, fermee: false }, style: Style { couleur: [0, 0, 0, 255], epaisseur }, role, element: Some(7) }
    }

    /// Trait de 1 m, 0,5 mm au 1:100 : ruban de 5 cm, prolongé de 2,5 cm à chaque bout.
    #[test]
    fn ruban_a_l_echelle() {
        let p = trait_(vec![DVec3::ZERO, DVec3::X], 0.5, Role::Coupe);
        let m = p.mailler(DVec3::Z, 100.0);
        assert_eq!(m.triangles.len(), 2);
        let aire: f64 = m.triangles.iter().map(|t| (t[1] - t[0]).cross(t[2] - t[0]).length() / 2.0).sum();
        assert!((aire - 1.05 * 0.05).abs() < 1e-9, "{aire}");
        assert!(trait_(vec![DVec3::ZERO, DVec3::X], 0.0, Role::Vu).mailler(DVec3::Z, 100.0).traits.len() == 1);
    }

    /// Angle entre un trait de 0,5 mm et un de 0,18 mm au 1:100 : chaque bout prolongé de
    /// la demi-largeur de l'autre, pas de la sienne ; bout libre : la sienne.
    #[test]
    fn raccord_d_epaisseurs_differentes() {
        let fort = trait_(vec![DVec3::ZERO, DVec3::X], 0.5, Role::Coupe);
        let fin = trait_(vec![DVec3::X, DVec3::new(1.0, 1.0, 0.0)], 0.18, Role::Coupe);
        let r = Raccords::new(&[fort, fin], 100.0);
        assert!((r.prolongement(DVec3::X, DVec3::ZERO, 0.025) - 0.009).abs() < 1e-12);
        assert!((r.prolongement(DVec3::X, DVec3::new(1.0, 1.0, 0.0), 0.009) - 0.025).abs() < 1e-12);
        assert!((r.prolongement(DVec3::ZERO, DVec3::X, 0.025) - 0.025).abs() < 1e-12);
        // trait aligné qui continue : aucun prolongement
        let suite = trait_(vec![DVec3::X, DVec3::new(2.0, 0.0, 0.0)], 0.18, Role::Coupe);
        let r = Raccords::new(&[trait_(vec![DVec3::ZERO, DVec3::X], 0.5, Role::Coupe), suite], 100.0);
        assert_eq!(r.prolongement(DVec3::X, DVec3::ZERO, 0.025), 0.0);
    }

    /// Carré de 2 m percé d'un carré de 1 m : 3 m² de triangles.
    #[test]
    fn surface_trouee() {
        let carre = |c: f64| vec![DVec3::new(-c, -c, 3.0), DVec3::new(c, -c, 3.0), DVec3::new(c, c, 3.0), DVec3::new(-c, c, 3.0)];
        let p = Primitive { forme: Forme::Surface(vec![carre(1.0), carre(0.5)]), style: Style { couleur: [200; 4], epaisseur: 0.0 }, role: Role::Poche, element: None };
        let t = p.mailler(DVec3::Z, 100.0).triangles;
        let aire: f64 = t.iter().map(|t| (t[1] - t[0]).cross(t[2] - t[0]).length() / 2.0).sum();
        assert!((aire - 3.0).abs() < 1e-5, "{aire}");
        assert!(t.iter().flatten().all(|p| (p.z - 3.0).abs() < 1e-9));
    }

    /// SVG : coordonnées en mm du papier, y vers le bas, groupes par rôle dans l'ordre.
    #[test]
    fn export_svg() {
        let d = Dessin {
            droite: DVec3::X,
            haut: DVec3::Y,
            echelle: 50.0,
            primitives: vec![trait_(vec![DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0)], 0.35, Role::Coupe), trait_(vec![DVec3::ZERO, DVec3::X], 0.0, Role::Vu)],
        };
        let s = d.svg("Plan <RDC>");
        // trait épais : segment à bouts francs prolongé d'une demi-largeur (bout libre)
        assert!(s.contains("M-0.124 0.124L20.124 -20.124") && s.contains(r#"stroke-linecap="butt""#), "{s}");
        assert!(s.contains(r#"stroke-width="0.350""#) && s.contains(r#"stroke-width="0.130""#));
        assert!(s.find(r#"<g id="vu">"#).unwrap() < s.find(r#"<g id="coupe">"#).unwrap());
        assert!(s.contains("Plan &lt;RDC&gt;") && s.contains(r#"data-element="7""#));
    }
}
