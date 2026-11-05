//! Motifs de coupe : ligne d'isolant en série de S.

/// Nombre maximal de boucles par morceau : borne le coût d'une couche très mince et longue.
const BOUCLES_ISOLANT_MAX: f64 = 2000.0;

/// Ligne d'isolant en série de S (symbole traditionnel) d'une région de coupe (triangles
/// 2D) : chaque morceau connexe est traité seul, le motif suit sa plus longue face.
pub fn ligne_isolation(tris: &[[[f64; 2]; 3]]) -> Vec<[[f64; 2]; 2]> {
    composantes(tris).iter().flat_map(|c| ligne_isolation_morceau(c)).collect()
}

/// Regroupe les triangles qui partagent une arête (morceaux disjoints d'une même couche).
pub(super) fn composantes(tris: &[[[f64; 2]; 3]]) -> Vec<Vec<[[f64; 2]; 3]>> {
    let cle = |p: [f64; 2]| ((p[0] * 100.0).round() as i64, (p[1] * 100.0).round() as i64);
    let mut parent: Vec<usize> = (0..tris.len()).collect();
    fn racine(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut vues: std::collections::HashMap<((i64, i64), (i64, i64)), usize> = std::collections::HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let (ka, kb) = (cle(t[k]), cle(t[(k + 1) % 3]));
            let k = if ka <= kb { (ka, kb) } else { (kb, ka) };
            if let Some(&j) = vues.get(&k) {
                let (ri, rj) = (racine(&mut parent, i), racine(&mut parent, j));
                parent[ri] = rj;
            } else {
                vues.insert(k, i);
            }
        }
    }
    let mut groupes: std::collections::BTreeMap<usize, Vec<[[f64; 2]; 3]>> = Default::default();
    for (i, t) in tris.iter().enumerate() {
        groupes.entry(racine(&mut parent, i)).or_default().push(*t);
    }
    groupes.into_values().collect()
}

pub(super) fn ligne_isolation_morceau(tris: &[[[f64; 2]; 3]]) -> Vec<[[f64; 2]; 2]> {
    use glam::DVec2;
    let pts: Vec<DVec2> = tris.iter().flatten().map(|p| DVec2::from_array(*p)).collect();
    if pts.len() < 3 {
        return Vec::new();
    }
    let c = pts.iter().copied().sum::<DVec2>() / pts.len() as f64;
    // direction : plus long côté du contour (arête portée par un seul triangle). La
    // covariance des sommets est faussée par la triangulation (diagonales, doublons) et
    // inclinait le motif dans la couche.
    let cle = |p: DVec2| ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64);
    let mut aretes: std::collections::HashMap<((i64, i64), (i64, i64)), (usize, DVec2, DVec2)> = std::collections::HashMap::new();
    for t in tris {
        let t = t.map(DVec2::from_array);
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let (ka, kb) = (cle(a), cle(b));
            let k = if ka <= kb { (ka, kb) } else { (kb, ka) };
            aretes.entry(k).or_insert((0, a, b)).0 += 1;
        }
    }
    let contour: Vec<(DVec2, DVec2)> = aretes.values().filter(|(n, _, _)| *n == 1).map(|(_, a, b)| (*a, *b)).collect();
    let Some(&(fa, fb)) = contour.iter().max_by(|x, y| x.0.distance(x.1).total_cmp(&y.0.distance(y.1))) else {
        return Vec::new();
    };
    let Some(u) = (fb - fa).try_normalize() else {
        return Vec::new();
    };
    let v = u.perp();
    // Épaisseur de la couche (pas de l'enveloppe) : une bande d'aire A et de périmètre P
    // vérifie e² − (P/2)·e + A = 0, y compris pour un retour en L en tableau.
    let aire: f64 = tris.iter().map(|t| {
        let [a, b, c] = t.map(DVec2::from_array);
        0.5 * (b - a).perp_dot(c - a).abs()
    }).sum();
    let perimetre: f64 = contour.iter().map(|(a, b)| a.distance(*b)).sum();
    let delta = perimetre * perimetre / 4.0 - 4.0 * aire;
    let e = if delta >= 0.0 { 0.5 * (perimetre / 2.0 - delta.sqrt()) } else { perimetre / 4.0 };
    // Faces : la plus longue arête, et sa parallèle à e vers l'intérieur.
    let face = (fa - c).dot(v);
    let (v0, v1) = if face <= 0.0 { (face, face + e) } else { (face - e, face) };
    let (mut u0, mut u1) = (f64::MAX, f64::MIN);
    for p in &pts {
        let d = *p - c;
        if (d.dot(v) - v0) > -1e-6 && (d.dot(v) - v1) < 1e-6 {
            u0 = u0.min(d.dot(u));
            u1 = u1.max(d.dot(u));
        }
    }
    if !(e > 1e-6) || !(u1 - u0 >= e) || (u1 - u0) / (0.25 * e) > BOUCLES_ISOLANT_MAX {
        return Vec::new();
    }
    // Série de S (isolant souple, motif traditionnel) : cercles de rayon e/4 posés
    // alternativement contre chaque face, au pas e/4, parcourus en sens alternés et
    // reliés par leurs tangentes intérieures ; les boucles se recoupent en gouttes.
    let r = 0.25 * e;
    let pas = 0.25 * e;
    let boites: Vec<(DVec2, DVec2)> = tris.iter().map(|t| {
        let [a, b, c] = t.map(DVec2::from_array);
        (a.min(b).min(c) - DVec2::splat(1e-9), a.max(b).max(c) + DVec2::splat(1e-9))
    }).collect();
    let dedans = |p: DVec2| tris.iter().zip(&boites).any(|(t, (mn, mx))| {
        if p.x < mn.x || p.y < mn.y || p.x > mx.x || p.y > mx.y {
            return false;
        }
        let [a, b, c] = t.map(DVec2::from_array);
        let s = |x: DVec2, y: DVec2| (y - x).perp_dot(p - x);
        let (s1, s2, s3) = (s(a, b), s(b, c), s(c, a));
        (s1 >= -1e-12 && s2 >= -1e-12 && s3 >= -1e-12) || (s1 <= 1e-12 && s2 <= 1e-12 && s3 <= 1e-12)
    });
    let local = |x: f64, y: f64| c + u * x + v * y;
    // cercle k : centre (x, y) local, sens (−1 horaire contre la face v1, +1 antihoraire contre v0)
    let n = ((u1 - u0) / pas).ceil() as i64 + 2;
    let cercles: Vec<(DVec2, f64)> = (-1..=n)
        .map(|k| {
            let haut = k.rem_euclid(2) == 0;
            (DVec2::new(u0 + k as f64 * pas, if haut { v1 - r } else { v0 + r }), if haut { -1.0 } else { 1.0 })
        })
        .collect();
    // tangente intérieure de A (sens sa) vers B : passe par le milieu, vitesse continue
    let tangente = |a: DVec2, sa: f64, b: DVec2| -> Option<(DVec2, DVec2)> {
        let m = (a + b) * 0.5;
        let w = m - a;
        let l = w.length();
        if l <= r { return None; }
        let phi = (r / l).acos();
        let w = w / l;
        [phi, -phi].into_iter().map(|ang| a + DVec2::from_angle(ang).rotate(w) * r).find(|t| {
            let rayon = *t - a;
            let vitesse = if sa < 0.0 { DVec2::new(rayon.y, -rayon.x) } else { DVec2::new(-rayon.y, rayon.x) };
            vitesse.dot(m - *t) > 0.0
        }).map(|t| (t, 2.0 * m - t))
    };
    let mut trace: Vec<DVec2> = Vec::new();
    let mut entree: Option<DVec2> = None;
    for k in 0..cercles.len() - 1 {
        let ((ca, sa), (cb, _)) = (cercles[k], cercles[k + 1]);
        let Some((sortie, arrivee)) = tangente(ca, sa, cb) else { return Vec::new() };
        // arc de l'entrée à la sortie dans le sens du cercle
        if let Some(e0) = entree {
            let (t0, t1) = ((e0 - ca).to_angle(), (sortie - ca).to_angle());
            let balayage = if sa > 0.0 { (t1 - t0).rem_euclid(std::f64::consts::TAU) } else { -(t0 - t1).rem_euclid(std::f64::consts::TAU) };
            let pas_arc = ((balayage.abs() / 0.15).ceil() as usize).max(2);
            for i in 0..=pas_arc {
                trace.push(ca + DVec2::from_angle(t0 + balayage * i as f64 / pas_arc as f64) * r);
            }
        } else {
            trace.push(sortie);
        }
        trace.push(arrivee);
        entree = Some(arrivee);
    }
    let mut out = Vec::new();
    const PIECES: usize = 6;
    for w in trace.windows(2) {
        let (p, q) = (local(w[0].x, w[0].y), local(w[1].x, w[1].y));
        let morceaux = if p.distance(q) > 0.3 * r { PIECES } else { 1 };
        for i in 0..morceaux {
            let (a, b) = (p.lerp(q, i as f64 / morceaux as f64), p.lerp(q, (i + 1) as f64 / morceaux as f64));
            if dedans((a + b) * 0.5) {
                out.push([a.to_array(), b.to_array()]);
            }
        }
    }
    out
}
