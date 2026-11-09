//! Terre-plein : élément horizontal (sorte `TerrePlein`) tracé comme une dalle. Chaque
//! arête peut porter un talus : son angle (degrés) est rangé dans `Dalle::pentes`, sa
//! tangente vaut hauteur / base (1/1 : 45°, 2/3 : 33,7°). Sans talus, l'arête descend à
//! la verticale jusqu'au terrain.
//!
//! Le remblai remplit l'espace entre le terrain naturel (terrains du projet, cotes NGF
//! ramenées à l'origine) et la sous-face du terre-plein prolongée par ses talus. Hors du
//! contour, la surface des talus est la plus haute des surfaces descendant de chaque
//! arête à talus selon sa pente (cônes aux angles). Coordonnées en mm.

use crate::modele::{geometrie, Dalle, Projet, SorteDalle};
use glam::{DVec2, DVec3};
use std::sync::{Arc, Mutex};

/// Talus proposés : libellé (hauteur/base) et rapport hauteur / base.
pub const TALUS: [(&str, f64); 5] = [("1/1", 1.0), ("2/3", 2.0 / 3.0), ("1/2", 0.5), ("1/3", 1.0 / 3.0), ("3/2", 1.5)];

/// Angle (degrés) d'un talus de rapport hauteur / base `r`.
pub fn angle(r: f64) -> f64 {
    r.atan().to_degrees()
}

/// Rapport hauteur / base d'un talus d'angle `deg`.
pub fn rapport(deg: f64) -> f64 {
    deg.to_radians().tan()
}

/// Libellé d'un talus : fraction proposée s'il en est une, et pente en pourcentage.
pub fn libelle(deg: f64) -> String {
    let r = rapport(deg);
    match TALUS.iter().find(|(_, x)| (x - r).abs() < 1e-3) {
        Some((nom, _)) => format!("{nom} ({:.0} %)", r * 100.0),
        None => format!("{:.0} %", r * 100.0),
    }
}

/// Libellé court : fraction proposée, sinon pente en pourcentage.
pub fn libelle_court(deg: f64) -> String {
    let r = rapport(deg);
    TALUS.iter().find(|(_, x)| (x - r).abs() < 1e-3).map_or_else(|| format!("{:.0} %", r * 100.0), |(nom, _)| nom.to_string())
}

/// Remblai d'un terre-plein : maillage (mm, repère du projet) et volume (mm³).
#[derive(Debug, Default)]
pub struct Remblai {
    pub tris: Vec<[DVec3; 3]>,
    pub volume: f64,
    /// Volume du déblai (mm³) : terrain naturel au-dessus du fond de forme et des talus.
    pub deblai: f64,
}

/// Terrain naturel : triangles de tous les terrains (repère du projet), répartis dans
/// une grille de recherche.
struct Sol {
    tris: Vec<[DVec3; 3]>,
    min: DVec2,
    pas: DVec2,
    n: usize,
    cases: Vec<Vec<usize>>,
    zmin: f64,
    zmax: f64,
}

impl Sol {
    fn nouveau(projet: &Projet) -> Option<Sol> {
        let decalage = DVec3::new(0.0, 0.0, -projet.ngf());
        let tris: Vec<[DVec3; 3]> = projet.terrains.iter().filter_map(|t| t.triangles().ok()).flatten().map(|t| t.map(|p| p + decalage)).collect();
        if tris.is_empty() {
            return None;
        }
        let (mut mn, mut mx) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        for p in tris.iter().flatten() {
            mn = mn.min(*p);
            mx = mx.max(*p);
        }
        let n = ((tris.len() as f64).sqrt().ceil() as usize).clamp(1, 128);
        let pas = ((mx - mn).truncate() / n as f64).max(DVec2::splat(1e-6));
        let mut sol = Sol { tris, min: mn.truncate(), pas, n, cases: vec![Vec::new(); n * n], zmin: mn.z, zmax: mx.z };
        for (i, t) in sol.tris.iter().enumerate() {
            let (a, b) = (t.iter().fold(DVec2::MAX, |m, p| m.min(p.truncate())), t.iter().fold(DVec2::MIN, |m, p| m.max(p.truncate())));
            let (i0, j0) = sol.case(a);
            let (i1, j1) = sol.case(b);
            for j in j0..=j1 {
                for k in i0..=i1 {
                    sol.cases[j * n + k].push(i);
                }
            }
        }
        Some(sol)
    }

    fn case(&self, p: DVec2) -> (usize, usize) {
        let q = (p - self.min) / self.pas;
        let c = |x: f64| (x.floor().max(0.0) as usize).min(self.n - 1);
        (c(q.x), c(q.y))
    }

    /// Altitude du terrain au point `p` du plan (None hors des terrains).
    fn z(&self, p: DVec2) -> Option<f64> {
        let (i, j) = self.case(p);
        for &k in &self.cases[j * self.n + i] {
            let [a, b, c] = self.tris[k];
            let (a2, b2, c2) = (a.truncate(), b.truncate(), c.truncate());
            let det = (b2 - a2).perp_dot(c2 - a2);
            if det.abs() < 1e-9 {
                continue;
            }
            // u, v : coordonnées barycentriques de b et c
            let u = (p - a2).perp_dot(c2 - a2) / det;
            let v = (b2 - a2).perp_dot(p - a2) / det;
            let eps = 1e-7;
            if u >= -eps && v >= -eps && u + v <= 1.0 + eps {
                return Some(a.z + (b.z - a.z) * u + (c.z - a.z) * v);
            }
        }
        None
    }
}

fn distance_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let t = b - a;
    let l = t.length_squared();
    let s = if l > 0.0 { ((p - a).dot(t) / l).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + t * s)
}

/// Polygone convexe `sujet` découpé par le triangle `t` (sens direct).
fn decouper(sujet: &[DVec2], t: &[DVec2; 3]) -> Vec<DVec2> {
    let mut poly = sujet.to_vec();
    for k in 0..3 {
        if poly.is_empty() {
            break;
        }
        let (a, b) = (t[k], t[(k + 1) % 3]);
        let cote = |p: DVec2| (b - a).perp_dot(p - a);
        let mut sortie = Vec::with_capacity(poly.len() + 2);
        for i in 0..poly.len() {
            let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
            let (sp, sq) = (cote(p), cote(q));
            if sp >= 0.0 {
                sortie.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                sortie.push(p + (q - p) * (sp / (sp - sq)));
            }
        }
        poly = sortie;
    }
    poly
}

/// Triangles (sens direct) d'un ensemble de contours avec trous.
fn trianguler(exterieur: &[[f64; 2]], trous: &[&Vec<[f64; 2]>]) -> Vec<[DVec2; 3]> {
    let mut coords: Vec<f64> = exterieur.iter().flatten().copied().collect();
    let mut debuts = Vec::new();
    for t in trous {
        debuts.push(coords.len() / 2);
        coords.extend(t.iter().flatten());
    }
    let Ok(indices) = earcutr::earcut(&coords, &debuts, 2) else { return Vec::new() };
    let p = |i: usize| DVec2::new(coords[2 * i], coords[2 * i + 1]);
    indices
        .chunks_exact(3)
        .map(|t| {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            if (b - a).perp_dot(c - a) < 0.0 { [a, c, b] } else { [a, b, c] }
        })
        .collect()
}

/// Sommet d'un morceau : position, dessus du remblai, terrain.
#[derive(Clone, Copy)]
struct Sommet {
    p: DVec2,
    dessus: f64,
    sol: f64,
}

impl Sommet {
    fn h(&self) -> f64 {
        self.dessus - self.sol
    }
    fn vers(&self, o: &Sommet, t: f64) -> Sommet {
        Sommet { p: self.p + (o.p - self.p) * t, dessus: self.dessus + (o.dessus - self.dessus) * t, sol: self.sol + (o.sol - self.sol) * t }
    }
}

/// Partie d'un polygone de sommets où le remblai est positif.
fn positif(poly: &[Sommet]) -> Vec<Sommet> {
    let mut v = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        if p.h() >= 0.0 {
            v.push(p);
        }
        if (p.h() >= 0.0) != (q.h() >= 0.0) {
            v.push(p.vers(&q, p.h() / (p.h() - q.h())));
        }
    }
    v
}

/// Plate-forme d'un terre-plein : sous-face `zr`, arêtes à talus et verticales, emprise
/// d'influence des talus (remblai et déblai), régions intérieures / extérieures
/// (triangles) et pas de la grille de calcul.
struct Plateforme {
    zr: f64,
    contours: Vec<Vec<[f64; 2]>>,
    talus: Vec<(DVec2, DVec2, f64)>,
    verticales: Vec<(DVec2, DVec2)>,
    mn: DVec2,
    mx: DVec2,
    pas: f64,
    regions: Vec<([DVec2; 3], bool)>,
}

impl Plateforme {
    fn nouvelle(projet: &Projet, d: &Dalle, sol: &Sol) -> Option<Plateforme> {
        if d.sorte != SorteDalle::TerrePlein || d.contours.is_empty() {
            return None;
        }
        let e = projet.type_dalle(&d.type_dalle).map_or(0.0, |t| t.epaisseur());
        let zr = projet.niveau(&d.niveau)?.altitude + d.decalage - e;
        let mut talus = Vec::new();
        let mut verticales = Vec::new();
        for (c, contour) in d.contours.iter().enumerate() {
            let n = contour.len();
            for k in 0..n {
                let (a, b) = (DVec2::from_array(contour[k]), DVec2::from_array(contour[(k + 1) % n]));
                match d.pente(c, k).filter(|x| *x > 0.0 && *x < 89.0) {
                    Some(deg) => talus.push((a, b, rapport(deg))),
                    None => verticales.push((a, b)),
                }
            }
        }
        let (mut mn, mut mx) = (DVec2::MAX, DVec2::MIN);
        for p in d.contours.iter().flatten() {
            mn = mn.min(DVec2::from_array(*p));
            mx = mx.max(DVec2::from_array(*p));
        }
        // portée des talus : jusqu'au plus bas (remblai) ou au plus haut (déblai) du terrain
        let hauteur = (zr - sol.zmin).max(sol.zmax - zr).max(0.0);
        let portee = talus.iter().map(|(_, _, t)| hauteur / t).fold(0.0, f64::max).min(300_000.0);
        let pas = (((mx - mn).max_element() + 2.0 * portee).max(1.0) / 120.0).max(100.0);
        mn -= DVec2::splat(portee + pas);
        mx += DVec2::splat(portee + pas);
        let est_trou = |i: usize| {
            let p = d.contours[i][0];
            d.contours.iter().enumerate().filter(|&(j, c)| j != i && geometrie::dedans(p, c)).count() % 2 == 1
        };
        let mut regions: Vec<([DVec2; 3], bool)> = Vec::new();
        let mut exterieurs = Vec::new();
        for i in 0..d.contours.len() {
            if est_trou(i) {
                regions.extend(trianguler(&d.contours[i], &[]).into_iter().map(|t| (t, false)));
                continue;
            }
            let trous: Vec<&Vec<[f64; 2]>> = (0..d.contours.len()).filter(|&j| j != i && est_trou(j) && geometrie::dedans(d.contours[j][0], &d.contours[i])).map(|j| &d.contours[j]).collect();
            regions.extend(trianguler(&d.contours[i], &trous).into_iter().map(|t| (t, true)));
            exterieurs.push(&d.contours[i]);
        }
        let cadre = [[mn.x, mn.y], [mx.x, mn.y], [mx.x, mx.y], [mn.x, mx.y]];
        regions.extend(trianguler(&cadre, &exterieurs).into_iter().map(|t| (t, false)));
        Some(Plateforme { zr, contours: d.contours.clone(), talus, verticales, mn, mx, pas, regions })
    }

    fn interieur(&self, p: DVec2) -> bool {
        self.contours.iter().filter(|c| geometrie::dedans(p.to_array(), c)).count() % 2 == 1
    }

    /// Dessus du remblai hors du contour : plus haute des surfaces de talus descendantes.
    fn remblai_ext(&self, p: DVec2) -> f64 {
        self.talus.iter().map(|(a, b, t)| self.zr - t * distance_segment(p, *a, *b)).fold(f64::NEG_INFINITY, f64::max)
    }

    /// Fond du déblai hors du contour : plus basse des surfaces de talus montantes.
    fn deblai_ext(&self, p: DVec2) -> f64 {
        self.talus.iter().map(|(a, b, t)| self.zr + t * distance_segment(p, *a, *b)).fold(f64::INFINITY, f64::min)
    }

    /// Cases de la grille (coin bas, coin haut) qui recouvrent la boîte [a, b].
    fn cases(&self, a: DVec2, b: DVec2) -> Vec<(DVec2, DVec2)> {
        let nx = ((self.mx.x - self.mn.x) / self.pas).ceil().max(1.0) as usize;
        let ny = ((self.mx.y - self.mn.y) / self.pas).ceil().max(1.0) as usize;
        let rang = |v: f64, o: f64, n: usize| (((v - o) / self.pas).floor().max(0.0) as usize).min(n - 1);
        let (i0, i1, j0, j1) = (rang(a.x, self.mn.x, nx), rang(b.x, self.mn.x, nx), rang(a.y, self.mn.y, ny), rang(b.y, self.mn.y, ny));
        let mut v = Vec::new();
        for j in j0..=j1 {
            for i in i0..=i1 {
                let c = self.mn + DVec2::new(i as f64, j as f64) * self.pas;
                v.push((c, c + DVec2::splat(self.pas)));
            }
        }
        v
    }

    /// Paroi verticale le long des arêtes sans talus, entre `bas(p)` et `haut(p)` là où
    /// `haut > bas`, tournée vers l'intérieur (`vers_interieur`) ou l'extérieur.
    fn parois(&self, haut: impl Fn(DVec2) -> Option<f64>, bas: impl Fn(DVec2) -> Option<f64>, vers_interieur: bool, tris: &mut Vec<[DVec3; 3]>) {
        for &(a, b) in &self.verticales {
            let l = a.distance(b);
            if l < 1e-6 {
                continue;
            }
            let t = (b - a) / l;
            let mut normale = DVec2::new(t.y, -t.x);
            if self.interieur((a + b) * 0.5 + normale * (l * 1e-3).min(1.0)) != vers_interieur {
                normale = -normale;
            }
            let n = (l / self.pas).ceil().max(1.0) as usize;
            for k in 0..n {
                let (mut p, mut q) = (a + (b - a) * (k as f64 / n as f64), a + (b - a) * ((k + 1) as f64 / n as f64));
                let (Some(mut hp), Some(mut hq), Some(mut bp), Some(mut bq)) = (haut(p), haut(q), bas(p), bas(q)) else { continue };
                let (dp, dq) = (hp - bp, hq - bq);
                if dp <= 0.0 && dq <= 0.0 {
                    continue;
                }
                if dp < 0.0 || dq < 0.0 {
                    let s = dp / (dp - dq);
                    let (m, hm, bm) = (p + (q - p) * s, hp + (hq - hp) * s, bp + (bq - bp) * s);
                    if dp < 0.0 {
                        p = m;
                        hp = hm;
                        bp = bm;
                    } else {
                        q = m;
                        hq = hm;
                        bq = bm;
                    }
                }
                let quad = [p.extend(bp), q.extend(bq), q.extend(hq), p.extend(hp)];
                let n3 = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
                let quad = if n3.truncate().dot(normale) >= 0.0 { quad } else { [quad[1], quad[0], quad[3], quad[2]] };
                tris.push([quad[0], quad[1], quad[2]]);
                tris.push([quad[0], quad[2], quad[3]]);
            }
        }
    }

    /// Morceau convexe du dessus d'un terrain, abaissé au fond du déblai dans l'emprise.
    fn creuser(&self, morceau: Vec<Point>) -> Vec<Vec<Point>> {
        let (a, b) = (morceau.iter().fold(DVec2::MAX, |m, v| m.min(v.p)), morceau.iter().fold(DVec2::MIN, |m, v| m.max(v.p)));
        if b.x <= self.mn.x || a.x >= self.mx.x || b.y <= self.mn.y || a.y >= self.mx.y {
            return vec![morceau];
        }
        let mut sortie = Vec::new();
        let mut reste = morceau;
        // hors du cadre : inchangé
        for (n, c) in [(DVec2::NEG_X, -self.mn.x), (DVec2::X, self.mx.x), (DVec2::NEG_Y, -self.mn.y), (DVec2::Y, self.mx.y)] {
            let dehors = couper(&reste, |v| c - n.dot(v.p), entre);
            if dehors.len() >= 3 {
                sortie.push(dehors);
            }
            reste = couper(&reste, |v| n.dot(v.p) - c, entre);
            if reste.len() < 3 {
                return sortie;
            }
        }
        for (t, dedans) in &self.regions {
            let mut sous = reste.clone();
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                sous = couper(&sous, |v| -(b - a).perp_dot(v.p - a), entre);
                if sous.len() < 3 {
                    break;
                }
            }
            if sous.len() < 3 {
                continue;
            }
            let (a, b) = (sous.iter().fold(DVec2::MAX, |m, v| m.min(v.p)), sous.iter().fold(DVec2::MIN, |m, v| m.max(v.p)));
            for (c0, c1) in self.cases(a, b) {
                let mut case = sous.clone();
                for (n, c) in [(DVec2::NEG_X, -c0.x), (DVec2::X, c1.x), (DVec2::NEG_Y, -c0.y), (DVec2::Y, c1.y)] {
                    case = couper(&case, |v| n.dot(v.p) - c, entre);
                    if case.len() < 3 {
                        break;
                    }
                }
                if case.len() < 3 {
                    continue;
                }
                for v in &mut case {
                    v.z = v.z.min(if *dedans { self.zr } else { self.deblai_ext(v.p) });
                }
                sortie.push(case);
            }
        }
        sortie
    }
}

/// Partie d'un polygone convexe où `f <= 0` (f affine le long des arêtes).
fn couper<T: Copy>(poly: &[T], f: impl Fn(&T) -> f64, entre: impl Fn(&T, &T, f64) -> T) -> Vec<T> {
    let mut v = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (p, q) = (&poly[i], &poly[(i + 1) % poly.len()]);
        let (fp, fq) = (f(p), f(q));
        if fp <= 0.0 {
            v.push(*p);
        }
        if (fp <= 0.0) != (fq <= 0.0) {
            v.push(entre(p, q, fp / (fp - fq)));
        }
    }
    v
}

/// Sommet du dessus d'un terrain creusé : position, altitude, altitude d'origine.
#[derive(Clone, Copy)]
struct Point {
    p: DVec2,
    z: f64,
    z0: f64,
}

fn entre(a: &Point, b: &Point, t: f64) -> Point {
    Point { p: a.p + (b.p - a.p) * t, z: a.z + (b.z - a.z) * t, z0: a.z0 + (b.z0 - a.z0) * t }
}

type Cache<T> = Mutex<Vec<(u64, Arc<T>)>>;
static CACHE: Cache<Remblai> = Mutex::new(Vec::new());
static CACHE_TERRAINS: Cache<Vec<[DVec3; 3]>> = Mutex::new(Vec::new());

fn en_cache<T>(cache: &Cache<T>, cle: u64, calcul: impl FnOnce() -> Option<T>) -> Option<Arc<T>> {
    if let Some((_, r)) = cache.lock().ok()?.iter().find(|(k, _)| *k == cle) {
        return Some(r.clone());
    }
    let r = Arc::new(calcul()?);
    let mut c = cache.lock().ok()?;
    c.push((cle, r.clone()));
    if c.len() > 16 {
        c.remove(0);
    }
    Some(r)
}

fn empreinte(valeurs: impl serde::Serialize, nombres: &[f64]) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(&valeurs).ok()?.hash(&mut h);
    nombres.iter().for_each(|x| x.to_bits().hash(&mut h));
    Some(h.finish())
}

/// Remblai et déblai d'un terre-plein (None : pas un terre-plein, ou aucun terrain).
/// Résultat mis en cache selon le terre-plein, son type, son niveau et les terrains.
pub fn remblai(projet: &Projet, d: &Dalle) -> Option<Arc<Remblai>> {
    if d.sorte != SorteDalle::TerrePlein || projet.terrains.is_empty() {
        return None;
    }
    let e = projet.type_dalle(&d.type_dalle).map_or(0.0, |t| t.epaisseur());
    let zr = projet.niveau(&d.niveau)?.altitude + d.decalage - e;
    let cle = empreinte((&d.contours, &d.pentes, &projet.terrains), &[zr, projet.ngf()])?;
    en_cache(&CACHE, cle, || calculer(projet, d))
}

fn calculer(projet: &Projet, d: &Dalle) -> Option<Remblai> {
    let sol = Sol::nouveau(projet)?;
    let pf = Plateforme::nouvelle(projet, d, &sol)?;
    let zr = pf.zr;
    let mut r = Remblai::default();
    let emettre = |poly: &[Sommet], r: &mut Remblai| {
        for k in 1..poly.len().saturating_sub(1) {
            let (a, b, c) = (poly[0], poly[k], poly[k + 1]);
            let aire = (b.p - a.p).perp_dot(c.p - a.p) * 0.5;
            if aire.abs() < 1e-3 {
                continue;
            }
            let haut = |s: Sommet| s.p.extend(s.dessus);
            let bas = |s: Sommet| s.p.extend(s.sol);
            let (b, c) = if aire > 0.0 { (b, c) } else { (c, b) };
            r.tris.push([haut(a), haut(b), haut(c)]);
            r.tris.push([bas(a), bas(c), bas(b)]);
            r.volume += aire.abs() * (a.h() + b.h() + c.h()) / 3.0;
        }
    };
    for (t, dedans) in &pf.regions {
        let (a, b) = (t.iter().fold(DVec2::MAX, |m, p| m.min(*p)), t.iter().fold(DVec2::MIN, |m, p| m.max(*p)));
        for (c0, c1) in pf.cases(a, b) {
            for case in [[c0, DVec2::new(c1.x, c0.y), c1], [c0, c1, DVec2::new(c0.x, c1.y)]] {
                let morceau = decouper(&case, t);
                if morceau.len() < 3 {
                    continue;
                }
                let sommets: Option<Vec<(Sommet, f64)>> = morceau
                    .iter()
                    .map(|&p| {
                        let s = sol.z(p)?;
                        let (dessus, fond) = if *dedans { (zr, zr) } else { (pf.remblai_ext(p).max(s - 1e7), pf.deblai_ext(p).min(s + 1e7)) };
                        Some((Sommet { p, dessus, sol: s }, fond))
                    })
                    .collect();
                let Some(sommets) = sommets else { continue };
                let remblai: Vec<Sommet> = sommets.iter().map(|x| x.0).collect();
                let poly = positif(&remblai);
                if poly.len() >= 3 {
                    emettre(&poly, &mut r);
                }
                // déblai : terrain au-dessus du fond de forme et des talus
                let deblai: Vec<Sommet> = sommets.iter().map(|(s, fond)| Sommet { p: s.p, dessus: s.sol, sol: *fond }).collect();
                let poly = positif(&deblai);
                for k in 1..poly.len().saturating_sub(1) {
                    let (a, b, c) = (poly[0], poly[k], poly[k + 1]);
                    r.deblai += ((b.p - a.p).perp_dot(c.p - a.p) * 0.5).abs() * (a.h() + b.h() + c.h()) / 3.0;
                }
            }
        }
    }
    // arêtes sans talus : paroi du remblai, du dessous du terre-plein au terrain (ou au
    // talus voisin), tournée vers l'extérieur
    pf.parois(|_| Some(zr), |p| sol.z(p).map(|s| pf.remblai_ext(p).max(s)), false, &mut r.tris);
    Some(r)
}

/// Solide d'un terrain (repère du projet, mm), creusé par les terre-pleins : dans leur
/// emprise, le dessus descend au fond du déblai (sous-face du terre-plein, talus
/// montants, parois sur les arêtes sans talus). Le dessous reste celui du terrain.
pub fn solide_terrain(projet: &Projet, t: &crate::modele::elements::site::Terrain, epaisseur: f64) -> Result<Vec<[DVec3; 3]>, String> {
    let pleins: Vec<&Dalle> = projet.dalles.iter().filter(|d| d.sorte == SorteDalle::TerrePlein).collect();
    if pleins.is_empty() {
        return t.solide(projet.ngf(), epaisseur);
    }
    let mut nombres = vec![projet.ngf(), epaisseur];
    nombres.extend(pleins.iter().filter_map(|d| projet.niveau(&d.niveau).map(|n| n.altitude + d.decalage - projet.type_dalle(&d.type_dalle).map_or(0.0, |x| x.epaisseur()))));
    let formes: Vec<_> = pleins.iter().map(|d| (&d.contours, &d.pentes)).collect();
    let cle = empreinte((t, &projet.terrains, formes), &nombres).ok_or("Empreinte du terrain impossible.")?;
    let mut erreur = None;
    let r = en_cache(&CACHE_TERRAINS, cle, || creuser_terrain(projet, t, epaisseur, &pleins).map_err(|e| erreur = Some(e)).ok());
    match r {
        Some(v) => Ok((*v).clone()),
        None => Err(erreur.unwrap_or_else(|| "Terrain non valide.".into())),
    }
}

fn creuser_terrain(projet: &Projet, t: &crate::modele::elements::site::Terrain, epaisseur: f64, pleins: &[&Dalle]) -> Result<Vec<[DVec3; 3]>, String> {
    if !(epaisseur > 0.0 && epaisseur.is_finite()) {
        return Err("L'épaisseur du terrain doit être positive.".into());
    }
    let sol = Sol::nouveau(projet).ok_or("Terrain non valide.")?;
    let pfs: Vec<Plateforme> = pleins.iter().filter_map(|d| Plateforme::nouvelle(projet, d, &sol)).collect();
    let decalage = DVec3::new(0.0, 0.0, -projet.ngf());
    let dessus: Vec<[DVec3; 3]> = t.triangles()?.into_iter().map(|t| t.map(|p| p + decalage)).collect();
    let mut morceaux: Vec<Vec<Point>> = dessus.iter().map(|t| t.iter().map(|v| Point { p: v.truncate(), z: v.z, z0: v.z }).collect()).collect();
    for pf in &pfs {
        morceaux = morceaux.into_iter().flat_map(|m| pf.creuser(m)).collect();
    }
    // dessus creusé ; le dessous suit à l'épaisseur du type (comme une topographie
    // Revit) : une fouille plus profonde que l'épaisseur ne traverse pas le terrain
    let mut tris = Vec::new();
    let bas = DVec3::new(0.0, 0.0, -epaisseur);
    for m in &morceaux {
        let haut = |v: &Point| v.p.extend(v.z);
        for k in 1..m.len().saturating_sub(1) {
            let (a, b, c) = (&m[0], &m[k], &m[k + 1]);
            let aire = (b.p - a.p).perp_dot(c.p - a.p);
            if aire.abs() < 1e-3 {
                continue;
            }
            let [a, b, c] = if aire > 0.0 { [haut(a), haut(b), haut(c)] } else { [haut(a), haut(c), haut(b)] };
            tris.push([a, b, c]);
            tris.push([a + bas, c + bas, b + bas]);
        }
    }
    let n = t.contour.len();
    for i in 0..n {
        let (a, b) = (DVec3::from_array(t.contour[i]) + decalage, DVec3::from_array(t.contour[(i + 1) % n]) + decalage);
        tris.push([a, a + bas, b + bas]);
        tris.push([a, b + bas, b]);
    }
    // parois du déblai le long des arêtes sans talus, dans l'emprise de ce terrain
    let contour: Vec<[f64; 2]> = t.contour.iter().map(|p| [p[0], p[1]]).collect();
    let z_terrain = |p: DVec2| geometrie::dedans(p.to_array(), &contour).then(|| sol.z(p)).flatten();
    for pf in &pfs {
        let dehors = |p: DVec2| z_terrain(p).map(|s| s.min(pf.deblai_ext(p)));
        let dedans = |p: DVec2| z_terrain(p).map(|s| s.min(pf.zr));
        pf.parois(dehors, dedans, true, &mut tris);
        // même marche sous le terrain, tournée vers l'extérieur
        pf.parois(|p| dehors(p).map(|z| z - epaisseur), |p| dedans(p).map(|z| z - epaisseur), false, &mut tris);
    }
    Ok(tris)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::elements::site::Terrain;

    pub(super) fn projet(talus: Option<f64>) -> (Projet, Dalle) {
        let mut p = Projet::nouveau("TP");
        p.niveaux.push(crate::modele::Niveau { id: "n0".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
        p.types_dalles.push(crate::modele::TypeCompose { id: "tp".into(), nom: "Grave".into(), couches: vec![crate::modele::Couche { epaisseur: 200.0, ..Default::default() }], categorie: Default::default(), couches_detaillees: true, origine: None });
        // terrain plat à -1 m, 60 × 60 m
        p.terrains.push(Terrain { id: "t".into(), nom: "TN".into(), contour: vec![[-30_000., -30_000., -1000.], [30_000., -30_000., -1000.], [30_000., 30_000., -1000.], [-30_000., 30_000., -1000.]], points: vec![], type_terrain: None });
        let mut d = Dalle { variante: None, phase: None, sous_projet: None, id: "d".into(), nom: "Terre-plein".into(), type_dalle: "tp".into(), niveau: "n0".into(), sorte: SorteDalle::TerrePlein, decalage: 200.0, contours: vec![vec![[0., 0.], [10_000., 0.], [10_000., 10_000.], [0., 10_000.]]], pentes: vec![], origine: None };
        if let Some(t) = talus {
            for k in 0..4 { d.definir_pente(0, k, Some(t)); }
        }
        p.dalles.push(d.clone());
        (p, d)
    }

    #[test]
    fn remblai_sans_talus_vaut_le_prisme() {
        let (p, d) = projet(None);
        let r = calculer(&p, &d).unwrap();
        // 10 × 10 m sur 1 m
        assert!((r.volume / 1e9 - 100.0).abs() < 0.01, "{}", r.volume / 1e9);
        assert!((geometrie::volume(&r.tris) / 1e9 - 100.0).abs() < 0.5);
    }

    #[test]
    fn remblai_avec_talus_ajoute_les_cones_et_les_prismes() {
        let (p, d) = projet(Some(angle(1.0)));
        let r = calculer(&p, &d).unwrap();
        // talus 1/1 sur 1 m : 4 prismes triangulaires (10 × 0,5) et 4 quarts de cône (π/3)
        let attendu = 100.0 + 4.0 * 10.0 * 0.5 + std::f64::consts::PI / 3.0;
        assert!((r.volume / 1e9 - attendu).abs() < 0.3, "{} / {attendu}", r.volume / 1e9);
        assert_eq!(libelle(angle(2.0 / 3.0)), "2/3 (67 %)");
    }
}

#[cfg(test)]
fn tests_projet() -> (Projet, Dalle) { tests::projet(Some(45.0)) }

#[cfg(test)]
mod tests_scene {
    #[test]
    fn scene_terre_plein() {
        let (mut p, _) = super::tests_projet();
        p.types_dalles[0].couches[0].materiau = Some("Grave non traitée".into());
        p.completer_materiaux();
        let s = crate::rendu::scene::depuis_natif(&p, std::path::Path::new("tp.json"));
        assert!(s.is_ok(), "{:?}", s.err());
    }
}

#[cfg(test)]
mod tests_deblai {
    use super::*;

    #[test]
    fn terre_plein_enterre_creuse_le_terrain() {
        // terrain plat à -1 m ; dessus du terre-plein à -1,8 m (sous-face à -2 m) : déblai
        // de 10 × 10 × 1 m, plus les talus 1/1
        let (mut p, _) = tests::projet(Some(angle(1.0)));
        p.dalles[0].decalage = -1800.0;
        let r = calculer(&p, &p.dalles[0]).unwrap();
        assert!(r.volume < 1e6, "{}", r.volume);
        let attendu = 100.0 + 20.0 + std::f64::consts::PI / 3.0;
        assert!((r.deblai / 1e9 - attendu).abs() < 0.3, "{}", r.deblai / 1e9);
        // terrain creusé jusqu'au fond de forme (-2 m), plus profond que son épaisseur
        // (0,5 m) : le solide reste fermé et garde son épaisseur
        let t = p.terrains[0].clone();
        for talus in [true, false] {
            if !talus { for k in 0..4 { p.dalles[0].definir_pente(0, k, None); } }
            let plein = geometrie::volume(&t.solide(0.0, 500.0).unwrap()) / 1e9;
            let tris = solide_terrain(&p, &t, 500.0).unwrap();
            let creuse = geometrie::volume(&tris) / 1e9;
            assert!((plein - creuse).abs() < 0.5, "{plein} {creuse}");
            // faces du dessus (tournées vers le haut) dans l'emprise : au fond de forme
            let dessus: Vec<f64> = tris.iter().filter(|t| (t[1] - t[0]).cross(t[2] - t[0]).z > 0.0)
                .flatten().filter(|v| v.x > 100.0 && v.x < 9900.0 && v.y > 100.0 && v.y < 9900.0).map(|v| v.z).collect();
            assert!(!dessus.is_empty() && dessus.iter().all(|z| (z + 2000.0).abs() < 1.0), "{:?}", dessus.iter().take(5).collect::<Vec<_>>());
        }
    }
}
