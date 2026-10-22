//! Rendu « dessin » des plans et élévations, à la manière d'un logiciel de DAO (CADCraft) :
//! fond blanc, faces blanches qui masquent ce qu'elles cachent (tampon de profondeur),
//! arêtes vives des maillages en trait fin, et coupe par un plan : poché (plein, hachuré ou
//! vide) et contour en trait fort. Épaisseurs et pas des hachures en mm du papier,
//! multipliés par l'échelle du dessin.

use crate::rvt::vues::{GrilleMotif, Motif, hachures_plan};
use crate::scene::{Scene, Vertex};
use glam::{DVec3, Vec3, Vec4};
use rayon::prelude::*;
use std::collections::HashMap;

/// Écart angulaire au-delà duquel deux faces voisines sont séparées par une arête vive
/// (cosinus de 30°) ; en deçà, facettes d'une même surface courbe.
const COS_VIF: f64 = 0.866;
/// Pas de quantification des positions (m).
const PAS: f64 = 1e-4;
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
    type Cle = ([i64; 3], [i64; 3]);
    /// Direction, point et arêtes (t0, t1, normale, sens) d'une droite.
    type Droite = (DVec3, DVec3, Vec<(f64, f64, DVec3, i32)>);
    let q = |x: f64, pas: f64| (x / pas).round() as i64;
    let mut droites: HashMap<Cle, Droite> = HashMap::new();
    for t in tris {
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
            e.2.push((ta.min(tb), ta.max(tb), n, sens));
        }
    }
    let mut out = Vec::new();
    let mut douces = Vec::new();
    for (u, p0, segs) in droites.into_values() {
        // bornes des intervalles élémentaires, rapprochées au pas
        let mut bornes: Vec<f64> = segs.iter().flat_map(|s| [s.0, s.1]).collect();
        bornes.sort_by(f64::total_cmp);
        bornes.dedup_by(|a, b| (*a - *b).abs() < PAS);
        let mut courant: Option<(f64, f64)> = None;
        for w in bornes.windows(2) {
            let (a, b) = (w[0], w[1]);
            let actifs = segs.iter().filter(|s| s.0 <= a + PAS && s.1 >= b - PAS);
            // groupes d'orientation : (normale de référence, somme des sens, normale la
            // plus écartée de la référence)
            let mut groupes: Vec<(DVec3, i32, DVec3)> = Vec::new();
            for s in actifs {
                match groupes.iter_mut().find(|g| g.0.dot(s.2) > COS_VIF) {
                    Some(g) => {
                        g.1 += s.3;
                        if s.2.dot(g.0) < g.2.dot(g.0) {
                            g.2 = s.2;
                        }
                    }
                    None => groupes.push((s.2, s.3, s.2)),
                }
            }
            let vive = groupes.iter().any(|g| g.1 != 0);
            // une seule face, courbe à cet endroit (plus d'un quart de degré entre les
            // facettes ; en deçà, coplanaires aux arrondis près)
            if let [g] = groupes[..]
                && !vive
                && g.0.dot(g.2) < 0.99999
                && b - a > PAS
            {
                douces.push(([p0 + u * a, p0 + u * b], g.0, g.2));
            }
            match (&mut courant, vive) {
                (Some(c), true) if (c.1 - a).abs() < PAS => c.1 = b,
                (_, true) => {
                    if let Some(c) = courant.take() {
                        out.push([p0 + u * c.0, p0 + u * c.1]);
                    }
                    courant = Some((a, b));
                }
                (_, false) => {
                    if let Some(c) = courant.take() {
                        out.push([p0 + u * c.0, p0 + u * c.1]);
                    }
                }
            }
        }
        if let Some(c) = courant {
            out.push([p0 + u * c.0, p0 + u * c.1]);
        }
    }
    out.retain(|s| s[0].distance_squared(s[1]) > PAS * PAS);
    Aretes { vives: out, douces }
}

/// Remplissage des parties coupées.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Poche {
    /// Gris clair hachuré à 45°.
    Hachure,
    /// Gris foncé uni.
    Plein,
    /// Blanc : contour seul.
    Vide,
}

impl Poche {
    pub fn nom(self) -> &'static str {
        match self {
            Poche::Hachure => "hachure",
            Poche::Plein => "plein",
            Poche::Vide => "vide",
        }
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Poche::Hachure => "Hachuré",
            Poche::Plein => "Plein",
            Poche::Vide => "Vide",
        }
    }

    pub fn depuis(nom: &str) -> Option<Poche> {
        [Poche::Hachure, Poche::Plein, Poche::Vide].into_iter().find(|p| p.nom() == nom)
    }
}

/// Contour des parties coupées, mm du papier.
pub const TRAIT_FORT_MM: f64 = 0.35;
/// Pas des hachures du poché, mm du papier.
pub const PAS_HACHURE_MM: f64 = 1.5;

/// Couleurs des traits du dessin (arêtes vives et contours).
pub const NOIR: [u8; 4] = [20, 20, 20, 255];

/// Coupe des objets visibles par le plan `plan` (n, d ; repère de la scène, n·p + d ≥ 0
/// gardé) : triangles (poché et contours en trait fort) et traits (hachures), à tracer
/// sans plan de découpe. `echelle` : dénominateur de l'échelle du dessin.
pub fn coupe(scene: &Scene, visible: &[bool], plan: Vec4, echelle: f64, poche: Poche) -> (Vec<Vertex>, Vec<Vertex>) {
    let n = plan.truncate().as_dvec3();
    let d = plan.w as f64 / n.length().max(1e-12);
    let n = n.normalize_or_zero();
    if n == DVec3::ZERO {
        return (Vec::new(), Vec::new());
    }
    // repère du plan : origine, deux axes ; x suit l'axe X du modèle quand c'est possible
    let o = -n * d;
    let ref_x = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let e1 = (ref_x - n * ref_x.dot(n)).normalize();
    let e2 = e1.cross(n);
    let f = echelle / 1000.0;
    let (largeur, pas) = (TRAIT_FORT_MM * f, PAS_HACHURE_MM * f);
    // poché, puis contour, légèrement du côté retiré (vers l'observateur d'un plan)
    let (dp, dc) = (-n * (0.2 * largeur).min(0.002), -n * (0.4 * largeur).min(0.004));
    let motif = Motif {
        nom: "Poché".into(),
        modele: true,
        grilles: vec![GrilleMotif { angle: std::f64::consts::FRAC_PI_4, origine: [0.0, 0.0], glissement: 0.0, decalage: pas, tirets: Vec::new() }],
    };
    let (fond, hachure) = match poche {
        Poche::Hachure => ([226, 226, 226, 255], Some([70, 70, 70, 255])),
        Poche::Plein => ([75, 75, 75, 255], None),
        Poche::Vide => ([255, 255, 255, 255], None),
    };
    let parties: Vec<(Vec<Vertex>, Vec<Vertex>)> = (0..scene.objects.len())
        .into_par_iter()
        .filter_map(|i| {
            let ob = &scene.objects[i];
            if !visible[i] || &*ob.class == "IFCANNOTATION" {
                return None;
            }
            // boîte entièrement d'un côté du plan : pas de coupe
            let (mut smin, mut smax) = (f64::MAX, f64::MIN);
            for c in 0..8 {
                let p = DVec3::new([ob.min.x, ob.max.x][c & 1] as f64, [ob.min.y, ob.max.y][(c >> 1) & 1] as f64, [ob.min.z, ob.max.z][c >> 2] as f64);
                let s = n.dot(p) + d;
                smin = smin.min(s);
                smax = smax.max(s);
            }
            if smin > 0.0 || smax < 0.0 {
                return None;
            }
            let mut segs = Vec::new();
            for r in [ob.opaque, ob.transparent] {
                for t in scene.vertices[r.start as usize..(r.start + r.count) as usize].chunks_exact(3) {
                    let p = [0, 1, 2].map(|k| Vec3::from(t[k].pos).as_dvec3());
                    if let Some(s) = section(p, n, d) {
                        segs.push(s.map(|q| [(q - o).dot(e1), (q - o).dot(e2)]));
                    }
                }
            }
            if segs.is_empty() {
                return None;
            }
            let (boucles, ouvertes) = chainer(&segs);
            let obj = i as u32;
            // drapeau « annotation » (normal.w) : jamais découpé par le plan
            let sommet = |x: f64, y: f64, decal: DVec3, color: [u8; 4]| {
                let p = o + e1 * x + e2 * y + decal;
                Vertex { pos: p.as_vec3().to_array(), normal: [0, 0, 0, 127], color, obj }
            };
            let mut tris = Vec::new();
            let mut traits = Vec::new();
            let contours: Vec<Vec<[f32; 2]>> = boucles.iter().map(|b| b.iter().map(|p| [p[0] as f32, p[1] as f32]).collect()).collect();
            let remplis = crate::texte::trianguler(contours);
            for t in &remplis {
                for p in t {
                    tris.push(sommet(p[0] as f64, p[1] as f64, dp, fond));
                }
            }
            if let Some(c) = hachure {
                let t2: Vec<[[f64; 2]; 3]> = remplis.iter().map(|t| t.map(|p| [p[0] as f64, p[1] as f64])).collect();
                for s in hachures_plan(&t2, &motif, 1.0, 20_000) {
                    traits.push(sommet(s[0][0], s[0][1], dp, c));
                    traits.push(sommet(s[1][0], s[1][1], dp, c));
                }
            }
            // contour en trait fort : un rectangle par segment, prolongé d'une demi
            // largeur à chaque bout pour fermer les angles
            let h = largeur / 2.0;
            for (pts, fermee) in boucles.iter().map(|b| (b, true)).chain(ouvertes.iter().map(|b| (b, false))) {
                let k = if fermee { pts.len() } else { pts.len() - 1 };
                for j in 0..k {
                    let (a, b) = (pts[j], pts[(j + 1) % pts.len()]);
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let l = (dx * dx + dy * dy).sqrt();
                    if l < 1e-9 {
                        continue;
                    }
                    let (tx, ty) = (dx / l * h, dy / l * h);
                    let (nx, ny) = (-ty, tx);
                    let c = [
                        [a[0] - tx + nx, a[1] - ty + ny],
                        [a[0] - tx - nx, a[1] - ty - ny],
                        [b[0] + tx - nx, b[1] + ty - ny],
                        [b[0] + tx + nx, b[1] + ty + ny],
                    ];
                    for q in [c[0], c[1], c[2], c[0], c[2], c[3]] {
                        tris.push(sommet(q[0], q[1], dc, NOIR));
                    }
                }
            }
            Some((tris, traits))
        })
        .collect();
    let mut tris = Vec::new();
    let mut traits = Vec::new();
    for (t, l) in parties {
        tris.extend(t);
        traits.extend(l);
    }
    (tris, traits)
}

/// Segment d'intersection d'un triangle et du plan (n, d), s'il le traverse.
fn section(p: [DVec3; 3], n: DVec3, d: f64) -> Option<[DVec3; 2]> {
    // sommet exactement sur le plan : compté du côté gardé
    let s = p.map(|q| {
        let s = n.dot(q) + d;
        if s == 0.0 { 1e-12 } else { s }
    });
    if s.iter().all(|&x| x > 0.0) || s.iter().all(|&x| x < 0.0) {
        return None;
    }
    let mut pts = Vec::with_capacity(2);
    for k in 0..3 {
        let (a, b) = (k, (k + 1) % 3);
        if (s[a] > 0.0) != (s[b] > 0.0) {
            let t = s[a] / (s[a] - s[b]);
            pts.push(p[a] + (p[b] - p[a]) * t);
        }
    }
    (pts.len() == 2 && pts[0].distance_squared(pts[1]) > 1e-14).then(|| [pts[0], pts[1]])
}

/// Suite de points dans le plan de coupe.
type Polyligne = Vec<[f64; 2]>;

/// Segments (coordonnées du plan) chaînés en boucles fermées et en lignes ouvertes.
fn chainer(segs: &[[[f64; 2]; 2]]) -> (Vec<Polyligne>, Vec<Polyligne>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Cube unité en 12 triangles : 12 arêtes, aucune diagonale.
    fn cube() -> Vec<[DVec3; 3]> {
        let v = |x: f64, y: f64, z: f64| DVec3::new(x, y, z);
        let faces = [
            [v(0., 0., 0.), v(0., 1., 0.), v(1., 1., 0.), v(1., 0., 0.)],
            [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)],
            [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)],
            [v(1., 1., 0.), v(0., 1., 0.), v(0., 1., 1.), v(1., 1., 1.)],
            [v(0., 1., 0.), v(0., 0., 0.), v(0., 0., 1.), v(0., 1., 1.)],
            [v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.), v(1., 0., 1.)],
        ];
        faces.iter().flat_map(|f| [[f[0], f[1], f[2]], [f[0], f[2], f[3]]]).collect()
    }

    fn longueur(a: &[[DVec3; 2]]) -> f64 {
        a.iter().map(|s| s[0].distance(s[1])).sum()
    }

    #[test]
    fn aretes_cube() {
        let a = aretes(&cube());
        assert!((longueur(&a.vives) - 12.0).abs() < 1e-6, "{}", longueur(&a.vives));
        assert!(a.douces.is_empty());
    }

    /// Face du dessus découpée avec une jonction en T (comme après une CSG) : la découpe
    /// n'est pas une arête.
    #[test]
    fn jonction_en_t() {
        let mut t = cube();
        t.retain(|t| !t.iter().all(|p| p.z == 1.0));
        let v = |x: f64, y: f64| DVec3::new(x, y, 1.0);
        // moitié gauche en deux triangles, moitié droite en trois (sommet en T à (0.5, 0.5))
        t.extend([[v(0., 0.), v(0.5, 0.), v(0.5, 1.)], [v(0., 0.), v(0.5, 1.), v(0., 1.)]]);
        t.extend([[v(0.5, 0.), v(1., 0.), v(0.5, 0.5)], [v(1., 0.), v(1., 1.), v(0.5, 0.5)], [v(0.5, 0.5), v(1., 1.), v(0.5, 1.)]]);
        let a = aretes(&t).vives;
        assert!((longueur(&a) - 12.0).abs() < 1e-6, "{}", longueur(&a));
    }

    /// Cylindre à 32 facettes : seuls les deux cercles sont des arêtes vives ; les 32
    /// génératrices sont des arêtes douces (silhouettes).
    #[test]
    fn cylindre_sans_generatrices() {
        let k = 32;
        let p = |i: usize, z: f64| {
            let a = i as f64 / k as f64 * std::f64::consts::TAU;
            DVec3::new(a.cos(), a.sin(), z)
        };
        let mut t = Vec::new();
        for i in 0..k {
            let j = (i + 1) % k;
            t.push([p(i, 0.), p(j, 0.), p(j, 1.)]);
            t.push([p(i, 0.), p(j, 1.), p(i, 1.)]);
            t.push([DVec3::ZERO, p(j, 0.), p(i, 0.)]);
            t.push([DVec3::Z, p(i, 1.), p(j, 1.)]);
        }
        let a = aretes(&t);
        let perimetre = 2.0 * k as f64 * 2.0 * (std::f64::consts::PI / k as f64).sin();
        assert!((longueur(&a.vives) - perimetre).abs() < 1e-6, "{} {}", longueur(&a.vives), perimetre);
        assert_eq!(a.douces.len(), k);
        assert!(a.douces.iter().all(|(s, n1, n2)| (s[0].distance(s[1]) - 1.0).abs() < 1e-9 && n1.dot(*n2) < 0.99));
    }

    #[test]
    fn coupe_carree() {
        let segs: Vec<[[f64; 2]; 2]> = vec![[[0., 0.], [1., 0.]], [[1., 1.], [1., 0.]], [[1., 1.], [0., 1.]], [[0., 1.], [0., 0.]], [[3., 0.], [4., 0.]]];
        let (b, o) = chainer(&segs);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].len(), 4);
        assert_eq!(o.len(), 1);
        let s = section([DVec3::ZERO, DVec3::new(1., 0., 2.), DVec3::new(0., 1., 2.)], DVec3::NEG_Z, 1.0).unwrap();
        assert!((s[0].z - 1.0).abs() < 1e-12 && (s[1].z - 1.0).abs() < 1e-12);
    }
}

#[cfg(test)]
mod mesure {
    /// Sphère de 500 000 triangles : durée du calcul des arêtes.
    #[test]
    #[ignore]
    fn sphere_500k() {
        use glam::DVec3;
        let (nu, nv) = (500, 500);
        let p = |i: usize, j: usize| {
            let (u, v) = (i as f64 / nu as f64 * std::f64::consts::TAU, j as f64 / nv as f64 * std::f64::consts::PI);
            DVec3::new(v.sin() * u.cos(), v.sin() * u.sin(), v.cos()) * 10.0
        };
        let mut t = Vec::new();
        for i in 0..nu {
            for j in 0..nv {
                let (a, b, c, d) = (p(i, j), p((i + 1) % nu, j), p((i + 1) % nu, j + 1), p(i, j + 1));
                t.push([a, b, c]);
                t.push([a, c, d]);
            }
        }
        let t0 = std::time::Instant::now();
        let a = super::aretes(&t);
        println!("{} triangles : {} vives, {} douces en {:?}", t.len(), a.vives.len(), a.douces.len(), t0.elapsed());
    }
}
