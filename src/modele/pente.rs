//! Éléments horizontaux à arêtes en pente, comme une toiture par tracé de Revit : chaque
//! arête qui définit une pente porte un plan qui monte vers l'intérieur du contour ; la
//! surface est le plus bas de ces plans (croupes et noues à leurs intersections). Sans
//! arête en pente, l'élément est plan. Coordonnées en mm.

use super::{geometrie, Dalle};
use glam::{DVec2, DVec3};

/// Plan d'une arête : hauteur `gradient · p + origine` au-dessus de la référence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub gradient: DVec2,
    pub origine: f64,
    /// 1 / cos(pente) : une épaisseur normale au plan vaut `secante` en vertical.
    pub secante: f64,
}

impl Plan {
    pub fn hauteur(&self, p: DVec2) -> f64 {
        self.gradient.dot(p) + self.origine
    }

    /// Plan décalé de l'épaisseur `e` (normale au plan, vers le haut si positive).
    fn decale(&self, e: f64) -> Plan {
        Plan { origine: self.origine + e * self.secante, ..*self }
    }
}

const PLAT: Plan = Plan { gradient: DVec2::ZERO, origine: 0.0, secante: 1.0 };

fn interieur(contours: &[Vec<[f64; 2]>], p: DVec2) -> bool {
    contours.iter().filter(|c| geometrie::dedans(p.to_array(), c)).count() % 2 == 1
}

/// Arêtes du contour orientées matière à gauche : (début, fin, contour, rang).
fn aretes(contours: &[Vec<[f64; 2]>]) -> Vec<(DVec2, DVec2, usize, usize)> {
    let mut v = Vec::new();
    for (c, contour) in contours.iter().enumerate() {
        let n = contour.len();
        for k in 0..n {
            let (a, b) = (DVec2::from_array(contour[k]), DVec2::from_array(contour[(k + 1) % n]));
            let t = b - a;
            if t.length() < 1e-6 {
                continue;
            }
            let gauche = DVec2::new(-t.y, t.x).normalize();
            let pas = (t.length() * 0.01).min(1.0);
            if interieur(contours, (a + b) * 0.5 + gauche * pas) { v.push((a, b, c, k)) } else { v.push((b, a, c, k)) }
        }
    }
    v
}

/// Direction (unitaire) vers l'intérieur du contour depuis le milieu de l'arête `k` du
/// contour `c`.
pub fn vers_interieur(contours: &[Vec<[f64; 2]>], c: usize, k: usize) -> Option<DVec2> {
    aretes(contours).into_iter().find(|x| x.2 == c && x.3 == k).map(|(a, b, _, _)| {
        let t = (b - a).normalize();
        DVec2::new(-t.y, t.x)
    })
}

/// Plans des arêtes en pente (pente entre 0 et 89° exclus) ; un plan horizontal sans
/// arête en pente.
pub fn plans(d: &Dalle) -> Vec<Plan> {
    let mut v: Vec<Plan> = Vec::new();
    for (a, b, c, k) in aretes(&d.contours) {
        let Some(deg) = d.pente(c, k).filter(|x| *x > 0.0 && *x < 89.0) else { continue };
        let t = (b - a).normalize();
        let vers_interieur = DVec2::new(-t.y, t.x);
        let r = deg.to_radians();
        let g = vers_interieur * r.tan();
        let plan = Plan { gradient: g, origine: -g.dot(a), secante: 1.0 / r.cos() };
        // deux arêtes alignées de même pente portent le même plan
        if !v.iter().any(|q| (q.gradient - plan.gradient).length() < 1e-9 && (q.origine - plan.origine).abs() < 1e-6) {
            v.push(plan);
        }
    }
    if v.is_empty() {
        v.push(PLAT);
    }
    v
}

/// Hauteur de la surface décalée de `e` au point `p`.
pub fn hauteur(plans: &[Plan], e: f64, p: DVec2) -> f64 {
    plans.iter().map(|q| q.decale(e).hauteur(p)).fold(f64::INFINITY, f64::min)
}

/// Garde la partie du polygone convexe où `f(p) <= 0` (f affine).
fn decouper(poly: &[DVec2], f: impl Fn(DVec2) -> f64) -> Vec<DVec2> {
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (fa, fb) = (f(a), f(b));
        if fa <= 0.0 {
            out.push(a);
        }
        if (fa < 0.0 && fb > 0.0) || (fa > 0.0 && fb < 0.0) {
            out.push(a + (b - a) * (fa / (fa - fb)));
        }
    }
    out
}

/// Morceaux plans de la surface décalée de `e` : (plan, polygone convexe orienté dans le
/// sens trigonométrique).
pub fn morceaux(contours: &[Vec<[f64; 2]>], plans: &[Plan], e: f64) -> Vec<(Plan, Vec<DVec2>)> {
    let Some(&o) = contours.first().and_then(|c| c.first()) else { return Vec::new() };
    let o = DVec2::from_array(o);
    // triangulation en coordonnées locales (f32) : précision gardée loin de l'origine
    let locaux: Vec<Vec<[f32; 2]>> = contours.iter().map(|c| c.iter().map(|p| [(p[0] - o.x) as f32, (p[1] - o.y) as f32]).collect()).collect();
    let decales: Vec<Plan> = plans.iter().map(|q| q.decale(e)).collect();
    let mut v = Vec::new();
    for t in crate::texte::trianguler(locaux) {
        let mut t = t.map(|p| o + DVec2::new(p[0] as f64, p[1] as f64));
        if (t[1] - t[0]).perp_dot(t[2] - t[0]) < 0.0 {
            t.swap(1, 2);
        }
        for (i, q) in decales.iter().enumerate() {
            let mut poly = t.to_vec();
            for (j, r) in decales.iter().enumerate() {
                if i == j || poly.len() < 3 {
                    continue;
                }
                poly = decouper(&poly, |p| q.hauteur(p) - r.hauteur(p));
            }
            if poly.len() >= 3 && aire(&poly) > 1e-3 {
                v.push((*q, poly));
            }
        }
    }
    v
}

fn aire(poly: &[DVec2]) -> f64 {
    (0..poly.len()).map(|i| poly[i].perp_dot(poly[(i + 1) % poly.len()])).sum::<f64>() / 2.0
}

/// Intégrale de la hauteur de la surface décalée de `e` sur le contour (mm³).
fn integrale(contours: &[Vec<[f64; 2]>], plans: &[Plan], e: f64) -> f64 {
    morceaux(contours, plans, e)
        .iter()
        .map(|(q, poly)| {
            let a = aire(poly);
            // centre de gravité du polygone
            let c = (0..poly.len()).fold(DVec2::ZERO, |s, i| {
                let (p, n) = (poly[i], poly[(i + 1) % poly.len()]);
                s + (p + n) * p.perp_dot(n)
            }) / (6.0 * a);
            a * q.hauteur(c)
        })
        .sum()
}

/// Volume entre les surfaces décalées de `e0` et `e1` (mm³).
pub fn volume(contours: &[Vec<[f64; 2]>], plans: &[Plan], e0: f64, e1: f64) -> f64 {
    integrale(contours, plans, e1) - integrale(contours, plans, e0)
}

/// Triangles (m) du volume compris entre les surfaces décalées de `e0` < `e1`, la
/// référence étant à l'altitude `z0` (mm) ; normales vers l'extérieur.
pub fn couche(contours: &[Vec<[f64; 2]>], plans: &[Plan], z0: f64, e0: f64, e1: f64) -> Vec<[DVec3; 3]> {
    let p3 = |p: DVec2, z: f64| DVec3::new(p.x, p.y, z0 + z) * 0.001;
    let mut tris = Vec::new();
    for (q, poly) in morceaux(contours, plans, e1) {
        for k in 1..poly.len() - 1 {
            let [a, b, c] = [poly[0], poly[k], poly[k + 1]];
            tris.push([p3(a, q.hauteur(a)), p3(b, q.hauteur(b)), p3(c, q.hauteur(c))]);
        }
    }
    for (q, poly) in morceaux(contours, plans, e0) {
        for k in 1..poly.len() - 1 {
            let [a, b, c] = [poly[0], poly[k], poly[k + 1]];
            tris.push([p3(a, q.hauteur(a)), p3(c, q.hauteur(c)), p3(b, q.hauteur(b))]);
        }
    }
    if tris.is_empty() {
        return tris;
    }
    // faces latérales : hauteurs affines par morceaux le long de chaque arête
    let (bas, haut): (Vec<Plan>, Vec<Plan>) = (plans.iter().map(|q| q.decale(e0)).collect(), plans.iter().map(|q| q.decale(e1)).collect());
    for (a, b, _, _) in aretes(contours) {
        let d = b - a;
        let mut ts = vec![0.0, 1.0];
        for jeu in [&bas, &haut] {
            for i in 0..jeu.len() {
                for j in i + 1..jeu.len() {
                    // hauteurs égales des plans i et j le long de l'arête
                    let (fa, fb) = (jeu[i].hauteur(a) - jeu[j].hauteur(a), jeu[i].hauteur(b) - jeu[j].hauteur(b));
                    if (fa - fb).abs() > 1e-12 {
                        let t = fa / (fa - fb);
                        if t > 1e-9 && t < 1.0 - 1e-9 {
                            ts.push(t);
                        }
                    }
                }
            }
        }
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
        let z = |jeu: &[Plan], p: DVec2| jeu.iter().map(|q| q.hauteur(p)).fold(f64::INFINITY, f64::min);
        for w in ts.windows(2) {
            let (p, n) = (a + d * w[0], a + d * w[1]);
            // matière à gauche : la face regarde à droite (vers l'extérieur)
            let (pb, nb, ph, nh) = (p3(p, z(&bas, p)), p3(n, z(&bas, n)), p3(p, z(&haut, p)), p3(n, z(&haut, n)));
            tris.push([pb, nb, nh]);
            tris.push([pb, nh, ph]);
        }
    }
    tris
}

/// Plans 3D (m, repère du fichier ; dedans si `n · x + w >= 0`) bornant la couche entre
/// les décalages `e0` et `e1`, un jeu par plan de la surface du dessus.
pub fn bandes(plans: &[Plan], z0: f64, e0: f64, e1: f64) -> Vec<Vec<glam::Vec4>> {
    let plan4 = |n: DVec3, w: f64| {
        let l = n.length();
        glam::Vec4::new((n.x / l) as f32, (n.y / l) as f32, (n.z / l) as f32, (w / l) as f32)
    };
    let haut: Vec<Plan> = plans.iter().map(|q| q.decale(e1)).collect();
    let mut v = Vec::new();
    'plans: for (i, q) in plans.iter().enumerate() {
        let (h, b) = (q.decale(e1), q.decale(e0));
        // z (m) sous le dessus : g·x − z + (origine + z0)/1000 >= 0
        let mut jeu = vec![
            plan4(DVec3::new(h.gradient.x, h.gradient.y, -1.0), (h.origine + z0) * 0.001),
            plan4(DVec3::new(-b.gradient.x, -b.gradient.y, 1.0), -(b.origine + z0) * 0.001),
        ];
        for (j, r) in haut.iter().enumerate() {
            if i == j {
                continue;
            }
            // région où le plan i est le plus bas : r − h >= 0
            let g = r.gradient - h.gradient;
            let w = (r.origine - h.origine) * 0.001;
            if g.length() < 1e-12 {
                if w < 0.0 { continue 'plans; }
                continue;
            }
            jeu.push(plan4(DVec3::new(g.x, g.y, 0.0), w));
        }
        v.push(jeu);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::{Dalle, SorteDalle};

    fn toiture(pentes: Vec<Option<f64>>) -> Dalle {
        Dalle {
            variante: None, phase: None, sous_projet: None,
            id: "t".into(), nom: "T".into(), type_dalle: "x".into(), niveau: "n".into(),
            sorte: SorteDalle::Toiture, decalage: 0.0,
            contours: vec![vec![[0.0, 0.0], [10_000.0, 0.0], [10_000.0, 6_000.0], [0.0, 6_000.0]]],
            pentes: vec![pentes], origine: None,
        }
    }

    #[test]
    fn deux_versants_faitage_au_milieu() {
        // pentes sur les deux longs pans : pignons aux extrémités
        let d = toiture(vec![Some(45.0), None, Some(45.0), None]);
        let p = plans(&d);
        assert_eq!(p.len(), 2);
        assert!((hauteur(&p, 0.0, DVec2::new(5_000.0, 3_000.0)) - 3_000.0).abs() < 1e-6);
        assert!(hauteur(&p, 0.0, DVec2::new(5_000.0, 0.0)).abs() < 1e-6);
        // volume d'une couche de 200 mm normale aux versants : aire × 200 / cos 45°
        let v = volume(&d.contours, &p, 0.0, 200.0);
        assert!((v - 60e6 * 200.0 * 2f64.sqrt()).abs() < 1.0, "{v}");
        let tris = couche(&d.contours, &p, 0.0, 0.0, 200.0);
        let mm: Vec<[DVec3; 3]> = tris.iter().map(|t| t.map(|x| x * 1000.0)).collect();
        assert!((geometrie::volume(&mm) - v).abs() < 10.0, "{} {v}", geometrie::volume(&mm));
    }

    #[test]
    fn croupes_et_plat() {
        let d = toiture(vec![Some(30.0); 4]);
        let p = plans(&d);
        assert_eq!(p.len(), 4);
        // faîtage à mi-largeur : 3 m × tan 30°
        let h = hauteur(&p, 0.0, DVec2::new(5_000.0, 3_000.0));
        assert!((h - 3_000.0 * 30f64.to_radians().tan()).abs() < 1e-6);
        let tris = couche(&d.contours, &p, 0.0, 0.0, 300.0);
        let mm: Vec<[DVec3; 3]> = tris.iter().map(|t| t.map(|x| x * 1000.0)).collect();
        assert!((geometrie::volume(&mm) - volume(&d.contours, &p, 0.0, 300.0)).abs() < 100.0);
        let plat = toiture(vec![]);
        assert_eq!(plans(&plat), vec![PLAT]);
        assert!((volume(&plat.contours, &plans(&plat), -200.0, 0.0) - 60e6 * 200.0).abs() < 1.0);
    }
}
