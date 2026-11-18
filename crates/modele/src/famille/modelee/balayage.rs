//! Balayage d'un profil le long d'un chemin : discrétisation des arcs, repère transporté
//! et sections en onglet aux changements de direction.
//!
//! Chaque tronçon porte son repère (transport parallèle depuis le précédent). À un
//! sommet intérieur, la section est la projection du profil du tronçon entrant, le long
//! de ce tronçon, sur le plan bissecteur : le raccord d'onglet est exact (le tronçon
//! sortant y aboutit au même contour). Sur un chemin fermé non plan, le vrillage
//! résiduel du transport est réparti uniformément sur les tronçons ; l'onglet n'y est
//! alors exact qu'au vrillage près (nul pour un chemin plan).
use glam::{DQuat, DVec3};

/// Pas angulaire maximal de discrétisation des arcs.
pub(super) const PAS_ARC: f64 = 5.0 * std::f64::consts::PI / 180.0;
/// Changement de direction maximal à un sommet (au-delà, l'onglet s'étire trop).
const VIRAGE_MAX: f64 = 150.0 * std::f64::consts::PI / 180.0;

/// Points de passage d'un arc par trois points `a`, `m`, `b` (sans `a`, avec `b`).
pub(super) fn arc(a: DVec3, m: DVec3, b: DVec3) -> Result<Vec<DVec3>, String> {
    let (p, q) = (m - a, b - a);
    let n = p.cross(q);
    if n.length() < 1e-6 * p.length() * q.length() { return Err("Arc du chemin : trois points alignés.".into()); }
    let centre = a + (q * p.length_squared() - p * q.length_squared()).cross(n) / (2.0 * n.length_squared());
    // a, m, b parcourus dans le sens direct autour de `axe`
    let axe = (m - a).cross(b - m).normalize();
    let rayon = a.distance(centre);
    let e1 = (a - centre) / rayon;
    let e2 = axe.cross(e1);
    let angle = |x: DVec3| { let t = ((x - centre).dot(e2)).atan2((x - centre).dot(e1)); if t < 0.0 { t + std::f64::consts::TAU } else { t } };
    let total = angle(b);
    let pas = (total / PAS_ARC - 1e-9).ceil().max(2.0) as usize;
    Ok((1..=pas).map(|k| if k == pas { b } else { let t = total * k as f64 / pas as f64; centre + (e1 * t.cos() + e2 * t.sin()) * rayon }).collect())
}

/// Chemin en points : les arcs sont remplacés par des cordes de 5° au plus. Sur un
/// chemin fermé, le dernier point n'est pas répété.
pub(super) fn discretiser(chemin: &[[f64; 3]], arcs: &[usize], ferme: bool) -> Result<Vec<DVec3>, String> {
    let n = chemin.len();
    if n < 2 { return Err("Le chemin doit contenir au moins deux points.".into()); }
    if ferme && n < 3 { return Err("Un chemin fermé doit contenir au moins trois points.".into()); }
    let p: Vec<DVec3> = chemin.iter().map(|x| DVec3::from_array(*x)).collect();
    for (k, &i) in arcs.iter().enumerate() {
        let borne = if ferme { i < n } else { i >= 1 && i + 1 < n };
        if !borne { return Err(format!("Arc du chemin : le point {} n'a pas deux voisins.", i + 1)); }
        if arcs[..k].iter().any(|&j| j == i || j == (i + 1) % n || (j + 1) % n == i) {
            return Err("Arc du chemin : deux points de passage d’arcs ne peuvent être ni confondus ni voisins.".into());
        }
    }
    let troncons = if ferme { n } else { n - 1 };
    for k in 0..troncons {
        let d = p[(k + 1) % n].distance(p[k]);
        if !d.is_finite() || d < 0.1 { return Err("Le chemin contient un segment nul ou trop court.".into()); }
    }
    if !ferme && p[0].distance(p[n - 1]) < 0.1 {
        return Err("Chemin dont les extrémités se rejoignent : cocher « Chemin fermé » (extrusion par chemin).".into());
    }
    let mut points = Vec::new();
    // départ : sur un chemin fermé dont le premier point est un passage d'arc, partir du dernier
    let depart = if ferme && arcs.contains(&0) { n - 1 } else { 0 };
    points.push(p[depart]);
    let mut k = depart;
    let mut faits = 0;
    while faits < troncons {
        let suivant = (k + 1) % n;
        if arcs.contains(&suivant) {
            let fin = (suivant + 1) % n;
            points.extend(arc(p[k], p[suivant], p[fin])?);
            k = fin; faits += 2;
        } else {
            points.push(p[suivant]);
            k = suivant; faits += 1;
        }
    }
    if ferme { points.pop(); }
    Ok(points)
}

/// Maillage fermé du balayage entre `depart` et `arrivee` (contours en correspondance).
pub(super) fn balayer(points: &[DVec3], ferme: bool, depart: &[[f64; 2]], arrivee: &[[f64; 2]]) -> Result<Vec<[DVec3; 3]>, String> {
    let m = points.len();
    let troncons = if ferme { m } else { m - 1 };
    let directions: Vec<DVec3> = (0..troncons).map(|k| (points[(k + 1) % m] - points[k]).normalize()).collect();
    let longueurs: Vec<f64> = (0..troncons).map(|k| points[(k + 1) % m].distance(points[k])).collect();
    // repères transportés
    let mut horizontaux = vec![(DQuat::from_rotation_arc(DVec3::Z, directions[0]) * DVec3::X).normalize()];
    for k in 1..troncons {
        let h = DQuat::from_rotation_arc(directions[k - 1], directions[k]) * horizontaux[k - 1];
        horizontaux.push((h - directions[k] * h.dot(directions[k])).normalize());
    }
    if ferme {
        let retour = DQuat::from_rotation_arc(directions[troncons - 1], directions[0]) * horizontaux[troncons - 1];
        let d0 = directions[0];
        let phi = d0.dot(horizontaux[0].cross(retour)).atan2(horizontaux[0].dot(retour));
        for k in 1..troncons {
            horizontaux[k] = DQuat::from_axis_angle(directions[k], -phi * k as f64 / troncons as f64) * horizontaux[k];
        }
    }
    let total: f64 = longueurs.iter().sum();
    let mut sections: Vec<Vec<DVec3>> = Vec::new();
    let mut parcouru = 0.0;
    let sommets = m;
    for i in 0..sommets {
        let entrant = if i > 0 { Some(i - 1) } else if ferme { Some(troncons - 1) } else { None };
        let sortant = if i < troncons { Some(i) } else { None };
        let k = entrant.unwrap_or(0);
        let (d, h) = (directions[k], horizontaux[k]);
        let v = d.cross(h);
        let plan = match (entrant, sortant) {
            (Some(e), Some(s)) => {
                let t = (directions[e] + directions[s]).normalize();
                if directions[e].angle_between(directions[s]) > VIRAGE_MAX {
                    return Err(format!("Chemin : changement de direction de plus de 150° au point {}.", i + 1));
                }
                Some(t)
            }
            _ => None,
        };
        if i > 0 { parcouru += longueurs[i - 1]; }
        let contour = interpoler(depart, arrivee, if total > 0.0 { parcouru / total } else { 0.0 });
        sections.push(contour.iter().map(|q| {
            let o = h * q[0] + v * q[1];
            let decale = plan.map_or(o, |t| o - d * (o.dot(t) / d.dot(t)));
            points[i] + decale
        }).collect());
    }
    // les sections avancent le long de chaque tronçon : pas de recouvrement d'onglets
    for k in 0..troncons {
        let (a, b) = (&sections[k], &sections[(k + 1) % m]);
        if a.iter().zip(b).any(|(p, q)| (*q - *p).dot(directions[k]) <= 1e-9) {
            return Err(format!("Chemin : le profil est trop grand pour le tronçon {} (les onglets se recouvrent).", k + 1));
        }
    }
    let sens = super::raccord::aire(depart) < 0.0;
    let n = depart.len();
    let mut tris = Vec::new();
    for k in 0..troncons {
        let (a, b) = (&sections[k], &sections[(k + 1) % m]);
        for i in 0..n {
            let j = (i + 1) % n;
            for mut t in [[a[i], a[j], b[j]], [a[i], b[j], b[i]]] {
                if sens { t.swap(1, 2); }
                tris.push(t);
            }
        }
    }
    if !ferme {
        for (contour, section, inverse) in [(depart, &sections[0], true), (arrivee, &sections[m - 1], false)] {
            let (ajoutes, faces) = super::raccord::capuchon(contour).ok_or("Profil dégénéré : triangulation impossible.")?;
            // image affine du plan du profil : origine et deux axes tirés de la section
            let image = affine(contour, section);
            let point = |i: usize| if i < n { section[i] } else { image(ajoutes[i - n]) };
            let plan = |i: usize| if i < n { contour[i] } else { ajoutes[i - n] };
            for f in faces {
                let (a, b, c) = (plan(f[0]), plan(f[1]), plan(f[2]));
                let s = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                let mut t = [point(f[0]), point(f[1]), point(f[2])];
                if (s < 0.0) != inverse { t.swap(1, 2); }
                tris.push(t);
            }
        }
    }
    if tris.iter().any(|t| t.iter().any(|p| !p.is_finite()) || (t[1] - t[0]).cross(t[2] - t[0]).length_squared() < 1e-16) {
        return Err("Forme dégénérée : revoir le chemin ou les profils.".into());
    }
    let origine = points[0];
    let volume: f64 = tris.iter().map(|t| (t[0] - origine).dot((t[1] - origine).cross(t[2] - origine))).sum();
    if volume.abs() < 1e-9 { return Err("Forme de volume nul.".into()); }
    if volume < 0.0 { for t in &mut tris { t.swap(1, 2); } }
    Ok(tris)
}

fn interpoler(a: &[[f64; 2]], b: &[[f64; 2]], t: f64) -> Vec<[f64; 2]> { super::raccord::interpoler(a, b, t) }

/// Application affine du plan du contour vers sa section (extrémités : sans onglet),
/// déterminée par trois sommets non alignés.
fn affine(contour: &[[f64; 2]], section: &[DVec3]) -> impl Fn([f64; 2]) -> DVec3 {
    let n = contour.len();
    let (mut meilleur, mut idx) = (0.0, (0, 1, 2));
    for j in 1..n {
        for k in j + 1..n {
            let (a, b, c) = (contour[0], contour[j], contour[k]);
            let s = ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs();
            if s > meilleur { meilleur = s; idx = (0, j, k); }
        }
    }
    let (a, b, c) = (contour[idx.0], contour[idx.1], contour[idx.2]);
    let (pa, pb, pc) = (section[idx.0], section[idx.1], section[idx.2]);
    let det = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    move |q: [f64; 2]| {
        let (x, y) = (q[0] - a[0], q[1] - a[1]);
        let u = (x * (c[1] - a[1]) - y * (c[0] - a[0])) / det;
        let w = ((b[0] - a[0]) * y - (b[1] - a[1]) * x) / det;
        pa + (pb - pa) * u + (pc - pa) * w
    }
}
