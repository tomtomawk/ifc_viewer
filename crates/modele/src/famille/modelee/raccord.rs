//! Correspondance de deux profils d'un raccordement et capuchons de leurs sections.
//!
//! Même nombre de sommets : correspondance sommet à sommet, dans l'ordre. Sinon, les
//! centres des deux profils sont alignés, le sommet d'arrivée le plus proche du premier
//! sommet de départ devient le premier, puis les deux contours sont rééchantillonnés
//! par abscisse curviligne relative : chaque sommet de l'un reçoit un point homologue
//! sur l'autre, et tous les sommets d'origine sont conservés.

pub(super) fn aire(p: &[[f64; 2]]) -> f64 {
    (0..p.len()).map(|i| p[i][0] * p[(i + 1) % p.len()][1] - p[(i + 1) % p.len()][0] * p[i][1]).sum()
}

pub(super) fn interpoler(a: &[[f64; 2]], b: &[[f64; 2]], t: f64) -> Vec<[f64; 2]> {
    a.iter().zip(b).map(|(a, b)| [a[0] * (1.0 - t) + b[0] * t, a[1] * (1.0 - t) + b[1] * t]).collect()
}

/// Centre de surface d'un contour simple.
fn centre(p: &[[f64; 2]]) -> [f64; 2] {
    let a = aire(p);
    let (mut x, mut y) = (0.0, 0.0);
    for i in 0..p.len() {
        let (u, w) = (p[i], p[(i + 1) % p.len()]);
        let c = u[0] * w[1] - w[0] * u[1];
        x += (u[0] + w[0]) * c;
        y += (u[1] + w[1]) * c;
    }
    [x / (3.0 * a), y / (3.0 * a)]
}

/// Abscisses curvilignes relatives (dans [0, 1[) des sommets.
fn abscisses(p: &[[f64; 2]]) -> Vec<f64> {
    let longueurs: Vec<f64> = (0..p.len()).map(|i| { let (a, b) = (p[i], p[(i + 1) % p.len()]); (b[0] - a[0]).hypot(b[1] - a[1]) }).collect();
    let total: f64 = longueurs.iter().sum();
    let mut s = 0.0;
    longueurs.iter().map(|l| { let t = s / total; s += l; t }).collect()
}

/// Point du contour à l'abscisse relative `t`.
fn point_a(p: &[[f64; 2]], ts: &[f64], t: f64) -> [f64; 2] {
    let i = ts.iter().rposition(|&x| x <= t + 1e-12).unwrap_or(0);
    let (t0, t1) = (ts[i], if i + 1 < ts.len() { ts[i + 1] } else { 1.0 });
    let u = if t1 - t0 > 1e-15 { ((t - t0) / (t1 - t0)).clamp(0.0, 1.0) } else { 0.0 };
    let (a, b) = (p[i], p[(i + 1) % p.len()]);
    [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]
}

/// Deux contours de même nombre de points, en correspondance.
pub(super) fn correspondance(depart: &[[f64; 2]], arrivee: &[[f64; 2]]) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    if depart.len() == arrivee.len() || depart.len() < 3 || arrivee.len() < 3 { return (depart.to_vec(), arrivee.to_vec()); }
    let (ca, cb) = (centre(depart), centre(arrivee));
    let d = |q: [f64; 2]| (q[0] - cb[0] + ca[0] - depart[0][0]).hypot(q[1] - cb[1] + ca[1] - depart[0][1]);
    let premier = (0..arrivee.len()).min_by(|&i, &j| d(arrivee[i]).total_cmp(&d(arrivee[j]))).unwrap_or(0);
    let arrivee: Vec<_> = (0..arrivee.len()).map(|k| arrivee[(premier + k) % arrivee.len()]).collect();
    let (ta, tb) = (abscisses(depart), abscisses(&arrivee));
    let mut ts: Vec<f64> = ta.iter().chain(&tb).copied().collect();
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    (ts.iter().map(|&t| point_a(depart, &ta, t)).collect(), ts.iter().map(|&t| point_a(&arrivee, &tb, t)).collect())
}

/// Triangulation d'un contour qui peut porter des points alignés (rééchantillonnage) :
/// les sommets saillants sont triangulés, puis chaque triangle bordé de points
/// intermédiaires est découpé en éventail depuis son centre. Rend les points ajoutés
/// (indices à la suite du contour) et les triangles.
pub(super) fn capuchon(contour: &[[f64; 2]]) -> Option<(Vec<[f64; 2]>, Vec<[usize; 3]>)> {
    let n = contour.len();
    let croix = |i: usize| {
        let (a, b, c) = (contour[(i + n - 1) % n], contour[i], contour[(i + 1) % n]);
        let (u, w) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
        (u[0] * w[1] - u[1] * w[0]).abs() > 1e-9 * (u[0].hypot(u[1]) * w[0].hypot(w[1])).max(1e-12)
    };
    let coins: Vec<usize> = (0..n).filter(|&i| croix(i)).collect();
    if coins.len() < 3 { return None; }
    let coords: Vec<f64> = coins.iter().flat_map(|&i| contour[i]).collect();
    let indices = earcutr::earcut(&coords, &[], 2).ok()?;
    let suivant = |k: usize| coins[(coins.iter().position(|&c| c == k).unwrap() + 1) % coins.len()];
    let mut ajoutes = Vec::new();
    let mut tris = Vec::new();
    for t in indices.chunks_exact(3) {
        let sommets = [coins[t[0]], coins[t[1]], coins[t[2]]];
        let mut tour = Vec::new();
        for e in 0..3 {
            let (x, y) = (sommets[e], sommets[(e + 1) % 3]);
            tour.push(x);
            if suivant(x) == y {
                let mut i = (x + 1) % n;
                while i != y { tour.push(i); i = (i + 1) % n; }
            } else if suivant(y) == x {
                let mut i = (x + n - 1) % n;
                while i != y { tour.push(i); i = (i + n - 1) % n; }
            }
        }
        if tour.len() == 3 { tris.push(sommets); continue; }
        let g = sommets.iter().fold([0.0, 0.0], |s, &i| [s[0] + contour[i][0] / 3.0, s[1] + contour[i][1] / 3.0]);
        ajoutes.push(g);
        let ig = n + ajoutes.len() - 1;
        for k in 0..tour.len() { tris.push([ig, tour[k], tour[(k + 1) % tour.len()]]); }
    }
    Some((ajoutes, tris))
}
