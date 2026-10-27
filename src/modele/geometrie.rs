//! Lecture de paramètres dans une géométrie (maillages en mm) : volume, prisme vertical
//! (dalles), étendue le long d'un axe (murs), arc passant par des points.

use glam::{DVec2, DVec3};

/// Volume d'un maillage fermé, orienté vers l'extérieur (mm³ ; valeur absolue).
pub fn volume(tris: &[[DVec3; 3]]) -> f64 {
    // sommes rapportées à un point du maillage : moins d'erreurs d'arrondi loin de l'origine
    let Some(o) = tris.first().map(|t| t[0]) else { return 0.0 };
    tris.iter().map(|t| (t[0] - o).dot((t[1] - o).cross(t[2] - o))).sum::<f64>().abs() / 6.0
}

/// Aire de contours plans, règle pair-impair (un contour contenu dans un nombre impair
/// d'autres est un trou).
pub fn aire_pair_impair(contours: &[Vec<[f64; 2]>]) -> f64 {
    let aire = |c: &[[f64; 2]]| (0..c.len()).map(|i| {
        let (a, b) = (c[i], c[(i + 1) % c.len()]);
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f64>() / 2.0;
    contours
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let Some(&p) = c.first() else { return 0.0 };
            let profondeur = contours.iter().enumerate().filter(|&(j, d)| j != i && dedans(p, d)).count();
            if profondeur % 2 == 0 { aire(c).abs() } else { -aire(c).abs() }
        })
        .sum()
}

pub fn dedans(p: [f64; 2], c: &[[f64; 2]]) -> bool {
    let mut d = false;
    for i in 0..c.len() {
        let (a, b) = (c[i], c[(i + 1) % c.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            d = !d;
        }
    }
    d
}

/// Prisme vertical : contours du dessus (plan, mm) et altitudes du bas et du haut.
#[derive(Debug)]
pub struct Prisme {
    pub contours: Vec<Vec<[f64; 2]>>,
    pub bas: f64,
    pub haut: f64,
}

/// Le maillage est-il un prisme vertical (faces horizontales en haut et en bas, faces
/// latérales verticales) ? Tolérances : 10⁻⁴ sur les normales, 0,5 mm sur les altitudes,
/// 0,5 % sur les aires.
pub fn prisme_vertical(tris: &[[DVec3; 3]]) -> Option<Prisme> {
    let (mut haut, mut bas) = (f64::MIN, f64::MAX);
    let (mut aire_haut, mut aire_bas) = (0.0, 0.0);
    let (mut dessus, mut dessous) = (Vec::new(), Vec::new());
    for t in tris {
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        let a = n.length() / 2.0;
        if a < 1e-6 {
            continue;
        }
        let n = n.normalize();
        if n.z.abs() < 1e-4 {
            continue;
        }
        if n.z.abs() < 1.0 - 1e-4 {
            return None;
        }
        let z = (t[0].z + t[1].z + t[2].z) / 3.0;
        if (t[0].z - z).abs() > 0.5 || (t[1].z - z).abs() > 0.5 {
            return None;
        }
        if n.z > 0.0 {
            haut = haut.max(z);
            aire_haut += a;
            dessus.push(z);
        } else {
            bas = bas.min(z);
            aire_bas += a;
            dessous.push(z);
        }
    }
    // un seul dessus et un seul dessous
    if haut - bas < 0.5 || dessus.iter().any(|z| (z - haut).abs() > 0.5) || dessous.iter().any(|z| (z - bas).abs() > 0.5) || (aire_haut - aire_bas).abs() > 0.005 * aire_haut {
        return None;
    }
    // contours : bords du dessus (arêtes vives du maillage à l'altitude du dessus), en m
    // pour le pas de fusion du module dessin
    let en_m: Vec<[DVec3; 3]> = tris.iter().map(|t| t.map(|p| p / 1000.0)).collect();
    let segs: Vec<[[f64; 2]; 2]> = crate::dessin::aretes(&en_m)
        .vives
        .into_iter()
        .filter(|s| s.iter().all(|p| (p.z * 1000.0 - haut).abs() < 0.5))
        .map(|s| s.map(|p| [p.x, p.y]))
        .collect();
    let (boucles, ouvertes) = crate::dessin::chainer(&segs);
    if !ouvertes.is_empty() || boucles.is_empty() {
        return None;
    }
    let contours: Vec<Vec<[f64; 2]>> = boucles.into_iter().map(|b| b.into_iter().map(|p| [p[0] * 1000.0, p[1] * 1000.0]).collect()).collect();
    let a = aire_pair_impair(&contours);
    ((a - aire_haut).abs() <= 0.005 * aire_haut).then_some(Prisme { contours, bas, haut })
}

/// Direction de la plus longue arête vive horizontale (vecteur unitaire du plan).
pub fn direction_principale(tris: &[[DVec3; 3]]) -> Option<DVec2> {
    let en_m: Vec<[DVec3; 3]> = tris.iter().map(|t| t.map(|p| p / 1000.0)).collect();
    let mut meilleure: Option<(f64, DVec2)> = None;
    for [a, b] in crate::dessin::aretes(&en_m).vives {
        let d = b - a;
        if d.z.abs() > 1e-6 * d.length().max(1e-9) {
            continue;
        }
        let l = d.truncate().length();
        if meilleure.is_none_or(|m| l > m.0) {
            meilleure = Some((l, d.truncate() / l));
        }
    }
    meilleure.map(|m| m.1)
}

/// Étendue d'un maillage dans le repère (origine `o`, axe `u` unitaire du plan, normale
/// `v` = u tourné de 90°) : (s_min, s_max) le long de u, (t_min, t_max) le long de v,
/// (z_min, z_max).
pub fn etendue(tris: &[[DVec3; 3]], o: DVec2, u: DVec2) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let v = u.perp();
    let (mut s, mut t, mut z) = ([f64::MAX, f64::MIN], [f64::MAX, f64::MIN], [f64::MAX, f64::MIN]);
    for p in tris.iter().flatten() {
        let q = p.truncate() - o;
        let (a, b) = (q.dot(u), q.dot(v));
        s = [s[0].min(a), s[1].max(a)];
        t = [t[0].min(b), t[1].max(b)];
        z = [z[0].min(p.z), z[1].max(p.z)];
    }
    (s, t, z)
}

/// Étendue radiale et angulaire d'un maillage autour de `c` : (r_min, r_max), angle de
/// départ et balayage (sens direct, le plus petit arc qui couvre tous les points),
/// (z_min, z_max).
pub fn etendue_arc(tris: &[[DVec3; 3]], c: DVec2) -> ([f64; 2], f64, f64, [f64; 2]) {
    let (mut r, mut z) = ([f64::MAX, f64::MIN], [f64::MAX, f64::MIN]);
    let mut angles: Vec<f64> = Vec::new();
    for p in tris.iter().flatten() {
        let q = p.truncate() - c;
        r = [r[0].min(q.length()), r[1].max(q.length())];
        z = [z[0].min(p.z), z[1].max(p.z)];
        angles.push(q.y.atan2(q.x));
    }
    angles.sort_by(f64::total_cmp);
    angles.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    // le plus grand écart entre deux angles successifs est hors de l'arc
    let tour = std::f64::consts::TAU;
    let n = angles.len();
    let (mut ecart, mut apres) = (0.0, 0);
    for i in 0..n {
        let e = if i + 1 < n { angles[i + 1] - angles[i] } else { angles[0] + tour - angles[n - 1] };
        if e > ecart {
            (ecart, apres) = (e, (i + 1) % n);
        }
    }
    let depart = angles.get(apres).copied().unwrap_or(0.0);
    (r, depart, tour - ecart, z)
}

/// Cercle passant par trois points du plan (centre, rayon), None s'ils sont alignés.
pub fn cercle(a: DVec2, b: DVec2, c: DVec2) -> Option<(DVec2, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 {
        return None;
    }
    let (a2, b2, c2) = (a.length_squared(), b.length_squared(), c.length_squared());
    let centre = DVec2::new((a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d, (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d);
    Some((centre, (a - centre).length()))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Boîte (mm) en 12 triangles orientés vers l'extérieur.
    pub fn boite(mn: DVec3, mx: DVec3) -> Vec<[DVec3; 3]> {
        let p = |i: usize| DVec3::new([mn.x, mx.x][i & 1], [mn.y, mx.y][(i >> 1) & 1], [mn.z, mx.z][i >> 2]);
        let faces = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
        faces.iter().flat_map(|f| [[p(f[0]), p(f[1]), p(f[2])], [p(f[0]), p(f[2]), p(f[3])]]).collect()
    }

    #[test]
    fn volume_boite() {
        let b = boite(DVec3::new(1e6, 2e6, 0.0), DVec3::new(1e6 + 5000.0, 2e6 + 200.0, 3000.0));
        assert!((volume(&b) - 3e9).abs() < 1.0);
    }

    #[test]
    fn prisme_boite() {
        let b = boite(DVec3::new(0.0, 0.0, -200.0), DVec3::new(5000.0, 2000.0, 0.0));
        let p = prisme_vertical(&b).unwrap();
        assert!((p.haut - 0.0).abs() < 1e-9 && (p.bas + 200.0).abs() < 1e-9);
        assert!((aire_pair_impair(&p.contours) - 1e7).abs() < 1.0);
        // un coin relevé : plus un prisme
        let mut t = b.clone();
        for tri in &mut t {
            for q in tri.iter_mut() {
                if q.x > 4999.0 && q.y > 1999.0 && q.z > -1.0 {
                    q.z = 100.0;
                }
            }
        }
        assert!(prisme_vertical(&t).is_none());
    }

    #[test]
    fn aire_avec_trou() {
        let carre = |c: f64| vec![[-c, -c], [c, -c], [c, c], [-c, c]];
        assert!((aire_pair_impair(&[carre(2.0), carre(1.0)]) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn etendues() {
        let b = boite(DVec3::new(0.0, 0.0, 0.0), DVec3::new(5000.0, 200.0, 3000.0));
        assert!((direction_principale(&b).unwrap().x.abs() - 1.0).abs() < 1e-9);
        let (s, t, z) = etendue(&b, DVec2::ZERO, DVec2::X);
        assert_eq!((s, t, z), ([0.0, 5000.0], [0.0, 200.0], [0.0, 3000.0]));
        let (c, r) = cercle(DVec2::new(1.0, 0.0), DVec2::new(0.0, 1.0), DVec2::new(-1.0, 0.0)).unwrap();
        assert!(c.length() < 1e-9 && (r - 1.0).abs() < 1e-9);
        // points sur un quart de cercle, de 10° à 100°
        let pts: Vec<[DVec3; 3]> = (0..10).map(|k| {
            let a = (10.0 + 10.0 * k as f64).to_radians();
            let p = DVec3::new(a.cos(), a.sin(), 0.0) * 1000.0;
            [p, p, p]
        }).collect();
        let (r, depart, balayage, _) = etendue_arc(&pts, DVec2::ZERO);
        assert!((r[0] - 1000.0).abs() < 1e-6 && (depart - 10f64.to_radians()).abs() < 1e-9 && (balayage - 90f64.to_radians()).abs() < 1e-9);
    }
}
