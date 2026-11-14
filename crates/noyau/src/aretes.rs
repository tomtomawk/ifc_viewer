//! Arêtes d'un maillage pour le dessin et chaînage de segments 2D en contours, avec les
//! constantes de trait communes (trait fort, noir des arêtes).

use glam::DVec3;
use std::collections::HashMap;

/// Écart angulaire au-delà duquel deux faces voisines sont séparées par une arête vive
/// (cosinus de 30°) ; en deçà, facettes d'une même surface courbe.
const COS_VIF: f64 = 0.866;
/// Pas de quantification des positions (m).
pub const PAS: f64 = 1e-4;
/// Pas de quantification des directions.
const PAS_DIR: f64 = 1e-4;

/// Arêtes d'un maillage pour le dessin.
#[derive(Default)]
pub struct Aretes {
    /// Arêtes vives : toujours tracées (sauf parties cachées).
    pub vives: Vec<[DVec3; 2]>,
    /// Arêtes entre deux facettes d'une surface courbe, avec leurs normales : tracées
    /// seulement sur la silhouette (une facette vers l'observateur, l'autre non).
    pub douces: Vec<([DVec3; 2], DVec3, DVec3)>,
}

/// Arêtes d'un maillage (soupe de triangles, repère du fichier) : bords des régions planes
/// ou courbes douces, sans les diagonales de triangulation ni les découpes des faces
/// coplanaires (jonctions en T de la CSG comprises), et arêtes douces des surfaces
/// courbes facettisées (silhouettes).
///
/// Chaque arête de triangle est rangée sur sa droite porteuse, avec la normale du triangle
/// et son sens de parcours. Sur chaque intervalle élémentaire de la droite, les arêtes
/// sont groupées par orientation de face (normales à moins de 30°) : une face continue
/// parcourt l'intervalle une fois dans chaque sens (somme nulle) ; une somme non nulle
/// marque le bord d'une face, donc une arête vive ou un bord libre. Une face continue dont
/// les deux facettes ne sont pas coplanaires donne une arête douce.
pub fn aretes(tris: &[[DVec3; 3]]) -> Aretes {
    let (vives, douces) = aretes_proprietaires(tris.iter().map(|t| (*t, 0)), false);
    Aretes { vives: vives.into_iter().map(|(s, _)| s).collect(), douces: douces.into_iter().map(|(d, _)| d).collect() }
}

/// Arêtes d'éléments d'un même matériau pris ensemble, chacune rendue à l'élément
/// (indice de `tris`) qui la porte : deux faces collées l'une contre l'autre (normales
/// opposées, parcours opposés : contact entre deux murs) s'annulent, et les faces
/// coplanaires de part et d'autre d'une jonction se raccordent sans arête. Les angles
/// réels (faces non coplanaires) restent.
pub fn aretes_jointes(tris: &[([DVec3; 3], usize)]) -> (Vec<([DVec3; 2], usize)>, Vec<(([DVec3; 2], DVec3, DVec3), usize)>) {
    aretes_proprietaires(tris.iter().copied(), true)
}

#[allow(clippy::type_complexity)]
fn aretes_proprietaires(tris: impl Iterator<Item = ([DVec3; 3], usize)>, annuler_opposees: bool) -> (Vec<([DVec3; 2], usize)>, Vec<(([DVec3; 2], DVec3, DVec3), usize)>) {
    type Cle = ([i64; 3], [i64; 3]);
    /// Direction, point et arêtes (t0, t1, normale, sens, élément) d'une droite.
    type Droite = (DVec3, DVec3, Vec<(f64, f64, DVec3, i32, usize)>);
    let q = |x: f64, pas: f64| (x / pas).round() as i64;
    let mut droites: HashMap<Cle, Droite> = HashMap::new();
    for (t, proprietaire) in tris {
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        if n.length_squared() < 1e-20 {
            continue;
        }
        let n = n.normalize();
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let l = b - a;
            if l.length_squared() < PAS * PAS {
                continue;
            }
            let mut u = l.normalize();
            // direction canonique : composante de plus grande valeur absolue positive
            let m = if u.x.abs() >= u.y.abs() && u.x.abs() >= u.z.abs() { u.x } else if u.y.abs() >= u.z.abs() { u.y } else { u.z };
            let sens = if m < 0.0 {
                u = -u;
                -1
            } else {
                1
            };
            let p0 = a - u * a.dot(u);
            let cle = ([q(u.x, PAS_DIR), q(u.y, PAS_DIR), q(u.z, PAS_DIR)], [q(p0.x, PAS), q(p0.y, PAS), q(p0.z, PAS)]);
            let e = droites.entry(cle).or_insert((u, p0, Vec::new()));
            let (ta, tb) = (a.dot(e.0), b.dot(e.0));
            e.2.push((ta.min(tb), ta.max(tb), n, sens, proprietaire));
        }
    }
    let mut out = Vec::new();
    let mut douces = Vec::new();
    for (u, p0, segs) in droites.into_values() {
        // bornes des intervalles élémentaires, rapprochées au pas
        let mut bornes: Vec<f64> = segs.iter().flat_map(|s| [s.0, s.1]).collect();
        bornes.sort_by(f64::total_cmp);
        bornes.dedup_by(|a, b| (*a - *b).abs() < PAS);
        let mut courant: Option<(f64, f64, usize)> = None;
        for w in bornes.windows(2) {
            let (a, b) = (w[0], w[1]);
            let actifs = segs.iter().filter(|s| s.0 <= a + PAS && s.1 >= b - PAS);
            // groupes d'orientation : (normale de référence, somme des sens, normale la
            // plus écartée de la référence, élément)
            let mut groupes: Vec<(DVec3, i32, DVec3, usize)> = Vec::new();
            for s in actifs {
                match groupes.iter_mut().find(|g| g.0.dot(s.2) > COS_VIF) {
                    Some(g) => {
                        g.1 += s.3;
                        if s.2.dot(g.0) < g.2.dot(g.0) {
                            g.2 = s.2;
                        }
                    }
                    None => groupes.push((s.2, s.3, s.2, s.4)),
                }
            }
            if annuler_opposees {
                // faces collées : normales opposées, sommes opposées
                let mut k = 0;
                while k < groupes.len() {
                    match (k + 1..groupes.len()).find(|&l| groupes[k].0.dot(groupes[l].0) < -COS_VIF && groupes[k].1 + groupes[l].1 == 0 && groupes[k].1 != 0) {
                        Some(l) => {
                            groupes.remove(l);
                            groupes.remove(k);
                        }
                        None => k += 1,
                    }
                }
            }
            let porteur = groupes.iter().find(|g| g.1 != 0).map(|g| g.3);
            let vive = porteur.is_some();
            // une seule face, courbe à cet endroit (plus d'un quart de degré entre les
            // facettes ; en deçà, coplanaires aux arrondis près)
            if let [g] = groupes[..]
                && !vive
                && g.0.dot(g.2) < 0.99999
                && b - a > PAS
            {
                douces.push((([p0 + u * a, p0 + u * b], g.0, g.2), g.3));
            }
            match (&mut courant, porteur) {
                (Some(c), Some(e)) if (c.1 - a).abs() < PAS && c.2 == e => c.1 = b,
                (_, Some(e)) => {
                    if let Some(c) = courant.take() {
                        out.push(([p0 + u * c.0, p0 + u * c.1], c.2));
                    }
                    courant = Some((a, b, e));
                }
                (_, None) => {
                    if let Some(c) = courant.take() {
                        out.push(([p0 + u * c.0, p0 + u * c.1], c.2));
                    }
                }
            }
        }
        if let Some(c) = courant {
            out.push(([p0 + u * c.0, p0 + u * c.1], c.2));
        }
    }
    out.retain(|s| s.0[0].distance_squared(s.0[1]) > PAS * PAS);
    (out, douces)
}

/// Contour des parties coupées, mm du papier.
pub const TRAIT_FORT_MM: f64 = 0.18;

/// Couleurs des traits du dessin (arêtes vives et contours).
pub const NOIR: [u8; 4] = [20, 20, 20, 255];

/// Suite de points dans le plan de coupe.
pub type Polyligne = Vec<[f64; 2]>;

/// Segments (coordonnées du plan) chaînés en boucles fermées et en lignes ouvertes.
pub fn chainer(segs: &[[[f64; 2]; 2]]) -> (Vec<Polyligne>, Vec<Polyligne>) {
    let cle = |p: [f64; 2]| ((p[0] / PAS).round() as i64, (p[1] / PAS).round() as i64);
    let mut voisins: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        if cle(s[0]) == cle(s[1]) {
            continue;
        }
        voisins.entry(cle(s[0])).or_default().push(i);
        voisins.entry(cle(s[1])).or_default().push(i);
    }
    let mut pris = vec![false; segs.len()];
    let (mut boucles, mut ouvertes) = (Vec::new(), Vec::new());
    // segment libre suivant au point p
    let suivant = |p: [f64; 2], pris: &[bool]| -> Option<(usize, [f64; 2])> {
        let i = *voisins.get(&cle(p))?.iter().find(|&&i| !pris[i])?;
        let s = segs[i];
        Some((i, if cle(s[0]) == cle(p) { s[1] } else { s[0] }))
    };
    for i in 0..segs.len() {
        if pris[i] || cle(segs[i][0]) == cle(segs[i][1]) {
            continue;
        }
        pris[i] = true;
        let depart = segs[i][0];
        let mut pts = vec![segs[i][0], segs[i][1]];
        while let Some((j, q)) = suivant(*pts.last().unwrap(), &pris) {
            pris[j] = true;
            pts.push(q);
            if cle(q) == cle(depart) {
                break;
            }
        }
        if pts.len() > 3 && cle(*pts.last().unwrap()) == cle(depart) {
            pts.pop();
            boucles.push(pts);
            continue;
        }
        // ligne ouverte : prolongée aussi vers l'arrière
        let mut avant = Vec::new();
        while let Some((j, q)) = suivant(*avant.last().unwrap_or(&depart), &pris) {
            pris[j] = true;
            avant.push(q);
        }
        avant.reverse();
        avant.extend(pts);
        ouvertes.push(avant);
    }
    (boucles, ouvertes)
}
