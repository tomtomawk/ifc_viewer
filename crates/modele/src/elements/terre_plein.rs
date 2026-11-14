//! Terre-plein : élément horizontal (sorte `TerrePlein`) tracé comme une dalle. Chaque
//! arête peut porter un talus : son angle (degrés) est rangé dans `Dalle::pentes`, sa
//! tangente vaut hauteur / base (1/1 : 45°, 2/3 : 33,7°). Sans talus, l'arête descend à
//! la verticale jusqu'au terrain.
//!
//! Le remblai remplit l'espace entre le terrain naturel (terrains du projet, cotes NGF
//! ramenées à l'origine) et la sous-face du terre-plein prolongée par ses talus. Hors du
//! contour, la surface des talus est la plus haute des surfaces descendant de chaque
//! arête à talus selon sa pente (cônes aux angles). Coordonnées en mm.

use crate::{geometrie, Dalle, Projet, SorteDalle};
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

/// Libellé d'un talus : pente en pourcentage, et fraction proposée s'il en est une.
pub fn libelle(deg: f64) -> String {
    let r = rapport(deg);
    match TALUS.iter().find(|(_, x)| (x - r).abs() < 1e-3) {
        Some((nom, _)) => format!("{:.0} % ({nom})", r * 100.0),
        None => libelle_court(deg),
    }
}

/// Libellé court : pente en pourcentage (hauteur / base).
pub fn libelle_court(deg: f64) -> String {
    let p = rapport(deg) * 100.0;
    if (p - p.round()).abs() < 0.05 { format!("{p:.0} %") } else { format!("{p:.1} %") }
}

/// Angle (degrés) d'un talus saisi en pourcentage (« 67 % », « 67 ») : la fraction
/// proposée de même pourcentage arrondi, sinon la valeur. None : sans talus.
pub fn depuis_pourcentage(texte: &str) -> Option<f64> {
    let p: f64 = texte.trim().trim_end_matches('%').trim().replace(',', ".").parse().ok()?;
    if !(p > 0.0 && p.is_finite()) {
        return None;
    }
    Some(angle(TALUS.iter().map(|(_, r)| *r).find(|r| ((r * 100.0).round() - p).abs() < 1e-6).unwrap_or(p / 100.0)))
}

/// Remblai d'un terre-plein : maillage (mm, repère du projet) et volume (mm³).
#[derive(Debug, Default)]
pub struct Remblai {
    pub tris: Vec<[DVec3; 3]>,
    pub volume: f64,
    /// Volume du déblai (mm³) : terrain naturel au-dessus du fond de forme et des talus.
    pub deblai: f64,
    /// Part de `volume` qui rebouche les fouilles talutées des arêtes remblayées (mm³).
    pub rebouchage: f64,
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
    /// Départ des talus : sous-face, ou arête du dessus (option du terre-plein).
    zt: f64,
    contours: Vec<Vec<[f64; 2]>>,
    aretes: Vec<Arete>,
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
        let zt = if d.talus_dessus { zr + e } else { zr };
        let interieur = |p: DVec2| d.contours.iter().filter(|c| geometrie::dedans(p.to_array(), c)).count() % 2 == 1;
        let mut aretes = Vec::new();
        for (c, contour) in d.contours.iter().enumerate() {
            let n = contour.len();
            let normale = |k: usize| {
                let (a, b) = (DVec2::from_array(contour[k % n]), DVec2::from_array(contour[(k + 1) % n]));
                let t = (b - a).normalize_or_zero();
                let m = DVec2::new(t.y, -t.x);
                let pas = (a.distance(b) * 1e-3).min(1.0);
                if interieur((a + b) * 0.5 + m * pas) { -m } else { m }
            };
            for k in 0..n {
                let (a, b) = (DVec2::from_array(contour[k]), DVec2::from_array(contour[(k + 1) % n]));
                if a.distance(b) < 1e-6 {
                    continue;
                }
                let n_k = normale(k);
                // angle saillant à une extrémité : l'arête voisine revient vers l'intérieur
                let precedent = DVec2::from_array(contour[(k + n - 1) % n]);
                let suivant = DVec2::from_array(contour[(k + 2) % n]);
                let saillant_debut = (a - precedent).dot(n_k) >= -1e-9;
                let saillant_fin = (suivant - b).dot(n_k) <= 1e-9;
                let pente = d.pente(c, k).filter(|x| *x > 0.0 && *x < 89.0).map(rapport);
                aretes.push(Arete { a, b, n: n_k, pente, remblai: pente.is_some() && d.remblaye(c, k), saillant_debut, saillant_fin });
            }
        }
        let talus: Vec<(DVec2, DVec2, f64)> = aretes.iter().filter_map(|x| x.pente.map(|t| (x.a, x.b, t))).collect();
        let (mut mn, mut mx) = (DVec2::MAX, DVec2::MIN);
        for p in d.contours.iter().flatten() {
            mn = mn.min(DVec2::from_array(*p));
            mx = mx.max(DVec2::from_array(*p));
        }
        // portée des talus : jusqu'au plus bas (remblai) ou au plus haut (déblai) du terrain
        let hauteur = (zt - sol.zmin).max(sol.zmax - zt).max(0.0);
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
        Some(Plateforme { zr, zt, contours: d.contours.clone(), aretes, mn, mx, pas, regions })
    }

    fn interieur(&self, p: DVec2) -> bool {
        self.contours.iter().filter(|c| geometrie::dedans(p.to_array(), c)).count() % 2 == 1
    }

    /// Hauteur à franchir par le talus au point `p` hors du contour : plus grande des
    /// hauteurs `pente × distance` des arêtes (talus plans, arêtiers aux angles
    /// saillants, cône seulement dans un angle rentrant). Infinie au-delà d'une arête
    /// sans talus : le talus voisin s'arrête dans le prolongement de la paroi.
    fn ecart(&self, p: DVec2) -> f64 {
        let mut h = f64::NEG_INFINITY;
        for x in &self.aretes {
            let d = x.distance(p);
            if d <= 0.0 {
                continue;
            }
            h = h.max(match x.pente { Some(r) => r * d, None => f64::INFINITY });
        }
        h.max(0.0)
    }

    /// Hauteurs de talus au point `p` hors du contour, par groupe d'arêtes : remblayées,
    /// et les autres (paroi sans talus : pente très raide). Distances signées : les
    /// hauteurs restent affines de part et d'autre d'un arêtier. L'arête remblayée
    /// gouverne la fouille là où sa hauteur dépasse celle des autres.
    fn hauteurs(&self, p: DVec2) -> (f64, f64) {
        let (mut remblayees, mut autres) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for x in &self.aretes {
            let d = x.distance(p);
            match (x.remblai, x.pente) {
                (true, Some(r)) => remblayees = remblayees.max(r * d),
                (_, r) => autres = autres.max(r.unwrap_or(1e4) * d),
            }
        }
        (remblayees, autres)
    }

    /// Dessus du remblai hors du contour : talus descendant depuis l'arête de départ.
    fn remblai_ext(&self, p: DVec2) -> f64 {
        self.zt - self.ecart(p)
    }

    /// Fond du déblai hors du contour : talus montant depuis l'arête de départ.
    fn deblai_ext(&self, p: DVec2) -> f64 {
        self.zt + self.ecart(p)
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

    /// Paroi verticale le long des arêtes du contour, entre `bas(p)` et `haut(p)` là où
    /// `haut > bas`, tournée vers l'intérieur (`vers_interieur`) ou l'extérieur : arêtes
    /// sans talus, ou marche de l'épaisseur quand le talus part de l'arête du dessus.
    fn parois(&self, haut: impl Fn(DVec2) -> Option<f64>, bas: impl Fn(DVec2) -> Option<f64>, vers_interieur: bool, tris: &mut Vec<[DVec3; 3]>) {
        self.parois_si(|_| true, haut, bas, vers_interieur, tris);
    }

    /// Comme `parois`, pour les seules arêtes retenues par `filtre`.
    fn parois_si(&self, filtre: impl Fn(&Arete) -> bool, haut: impl Fn(DVec2) -> Option<f64>, bas: impl Fn(DVec2) -> Option<f64>, vers_interieur: bool, tris: &mut Vec<[DVec3; 3]>) {
        for &Arete { a, b, .. } in self.aretes.iter().filter(|x| filtre(x)) {
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

/// Arête du contour d'un terre-plein : extrémités, normale extérieure, pente du talus
/// (hauteur / base ; None : paroi verticale), angles saillants à ses extrémités.
#[derive(Clone, Copy)]
struct Arete {
    a: DVec2,
    b: DVec2,
    n: DVec2,
    pente: Option<f64>,
    /// Fouille talutée remblayée jusqu'à l'arête (arête à talus seulement).
    remblai: bool,
    saillant_debut: bool,
    saillant_fin: bool,
}

impl Arete {
    /// Distance du point `p` à l'arête, comptée vers l'extérieur : à la droite de
    /// l'arête (signée), ou à son extrémité dans un angle rentrant.
    fn distance(&self, p: DVec2) -> f64 {
        let t = self.b - self.a;
        let u = (p - self.a).dot(t) / t.length_squared();
        if u < 0.0 && !self.saillant_debut {
            p.distance(self.a)
        } else if u > 1.0 && !self.saillant_fin {
            p.distance(self.b)
        } else {
            self.n.dot(p - self.a)
        }
    }
}

/// Sommet d'un morceau de rebouchage : dessus (terrain naturel), fond de fouille, et
/// avance `f` de l'arête remblayée sur les autres (rebouché où `f >= 0`).
#[derive(Clone, Copy)]
struct Bouche {
    s: Sommet,
    f: f64,
}

fn entre_bouches(a: &Bouche, b: &Bouche, t: f64) -> Bouche {
    Bouche { s: a.s.vers(&b.s, t), f: a.f + (b.f - a.f) * t }
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
    let cle = empreinte((&d.contours, &d.pentes, &d.remblais, d.talus_dessus, &projet.terrains), &[zr, e, projet.ngf()])?;
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
    let rebouche = pf.aretes.iter().any(|x| x.remblai);
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
                // fouille des arêtes remblayées : rebouchée du fond de fouille au terrain
                // naturel, là où l'arête remblayée gouverne le talus
                if rebouche && !*dedans {
                    let bouches: Vec<Bouche> = sommets
                        .iter()
                        .map(|(s, _)| {
                            let (remblayees, autres) = pf.hauteurs(s.p);
                            Bouche { s: Sommet { p: s.p, dessus: s.sol, sol: pf.zt + remblayees }, f: if autres == f64::NEG_INFINITY { 1.0 } else { remblayees - autres } }
                        })
                        .collect();
                    let poly = couper(&couper(&bouches, |v| -v.s.h(), entre_bouches), |v| -v.f, entre_bouches);
                    if poly.len() >= 3 {
                        let avant = r.volume;
                        emettre(&poly.iter().map(|v| v.s).collect::<Vec<_>>(), &mut r);
                        r.rebouchage += r.volume - avant;
                        // flanc du rebouchage le long de l'arêtier qui le sépare d'un talus ouvert
                        let direct = (0..poly.len()).map(|i| poly[i].s.p.perp_dot(poly[(i + 1) % poly.len()].s.p)).sum::<f64>() >= 0.0;
                        for i in 0..poly.len() {
                            let (mut a, mut b) = (poly[i], poly[(i + 1) % poly.len()]);
                            if a.f.abs() > 1e-3 || b.f.abs() > 1e-3 || a.s.p.distance(b.s.p) < 1e-3 {
                                continue;
                            }
                            if !direct {
                                std::mem::swap(&mut a, &mut b);
                            }
                            let quad = [a.s.p.extend(a.s.sol), b.s.p.extend(b.s.sol), b.s.p.extend(b.s.dessus), a.s.p.extend(a.s.dessus)];
                            r.tris.push([quad[0], quad[1], quad[2]]);
                            r.tris.push([quad[0], quad[2], quad[3]]);
                        }
                    }
                }
            }
        }
    }
    // arêtes sans talus : paroi du remblai, du dessous du terre-plein au terrain (ou au
    // talus voisin), tournée vers l'extérieur
    // parois du remblai le long du contour : vers l'extérieur là où il s'arrête (arête
    // sans talus), vers l'intérieur contre la tranche du terre-plein (talus depuis le dessus)
    let dedans = |p: DVec2| sol.z(p).map(|s| zr.max(s));
    let dehors = |p: DVec2| sol.z(p).map(|s| pf.remblai_ext(p).max(s));
    pf.parois(dedans, dehors, false, &mut r.tris);
    pf.parois(dehors, dedans, true, &mut r.tris);
    // rebouchage : paroi contre la plate-forme, de l'arête de départ au terrain naturel
    pf.parois_si(|x| x.remblai, |p| sol.z(p), |_| Some(pf.zt), true, &mut r.tris);
    Some(r)
}

/// Solide d'un terrain (repère du projet, mm), creusé par les terre-pleins : dans leur
/// emprise, le dessus descend au fond du déblai (sous-face du terre-plein, talus
/// montants, parois sur les arêtes sans talus). Le dessous reste celui du terrain.
pub fn solide_terrain(projet: &Projet, t: &crate::elements::site::Terrain, epaisseur: f64) -> Result<Vec<[DVec3; 3]>, String> {
    let pleins: Vec<&Dalle> = projet.dalles.iter().filter(|d| d.sorte == SorteDalle::TerrePlein).collect();
    if pleins.is_empty() {
        return t.solide(projet.ngf(), epaisseur);
    }
    let mut nombres = vec![projet.ngf(), epaisseur];
    nombres.extend(pleins.iter().filter_map(|d| projet.niveau(&d.niveau).map(|n| n.altitude + d.decalage - projet.type_dalle(&d.type_dalle).map_or(0.0, |x| x.epaisseur()))));
    nombres.extend(pleins.iter().map(|d| projet.type_dalle(&d.type_dalle).map_or(0.0, |x| x.epaisseur())));
    let formes: Vec<_> = pleins.iter().map(|d| (&d.contours, &d.pentes, d.talus_dessus)).collect();
    let cle = empreinte((t, &projet.terrains, formes), &nombres).ok_or("Empreinte du terrain impossible.")?;
    let mut erreur = None;
    let r = en_cache(&CACHE_TERRAINS, cle, || creuser_terrain(projet, t, epaisseur, &pleins).map_err(|e| erreur = Some(e)).ok());
    match r {
        Some(v) => Ok((*v).clone()),
        None => Err(erreur.unwrap_or_else(|| "Terrain non valide.".into())),
    }
}

fn creuser_terrain(projet: &Projet, t: &crate::elements::site::Terrain, epaisseur: f64, pleins: &[&Dalle]) -> Result<Vec<[DVec3; 3]>, String> {
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
    use crate::elements::site::Terrain;

    pub(super) fn projet(talus: Option<f64>) -> (Projet, Dalle) {
        let mut p = Projet::nouveau("TP");
        p.niveaux.push(crate::Niveau { id: "n0".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
        p.types_dalles.push(crate::TypeCompose { id: "tp".into(), nom: "Grave".into(), couches: vec![crate::Couche { epaisseur: 200.0, ..Default::default() }], categorie: Default::default(), couches_detaillees: true, origine: None });
        // terrain plat à -1 m, 60 × 60 m
        p.terrains.push(Terrain { id: "t".into(), nom: "TN".into(), contour: vec![[-30_000., -30_000., -1000.], [30_000., -30_000., -1000.], [30_000., 30_000., -1000.], [-30_000., 30_000., -1000.]], points: vec![], type_terrain: None });
        let mut d = Dalle { variante: None, phase: None, sous_projet: None, id: "d".into(), nom: "Terre-plein".into(), type_dalle: "tp".into(), niveau: "n0".into(), sorte: SorteDalle::TerrePlein, decalage: 200.0, contours: vec![vec![[0., 0.], [10_000., 0.], [10_000., 10_000.], [0., 10_000.]]], pentes: vec![], talus_dessus: false, remblais: Vec::new(), type_remblai: None, origine: None };
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
        // talus 1/1 sur 1 m : 4 prismes triangulaires (10 × 0,5) et 4 pyramides d'angle
        // (arêtiers : 1 × 1 × 1 / 3)
        let attendu = 100.0 + 4.0 * 10.0 * 0.5 + 4.0 / 3.0;
        assert!((r.volume / 1e9 - attendu).abs() < 0.3, "{} / {attendu}", r.volume / 1e9);
        assert_eq!(libelle(angle(2.0 / 3.0)), "67 % (2/3)");
        assert_eq!(libelle_court(angle(0.125)), "12.5 %");
        assert!((depuis_pourcentage("67 %").unwrap() - angle(2.0 / 3.0)).abs() < 1e-9);
        assert!((rapport(depuis_pourcentage("40").unwrap()) - 0.4).abs() < 1e-9);
        assert_eq!(depuis_pourcentage("Sans talus"), None);
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
        let attendu = 100.0 + 20.0 + 4.0 / 3.0;
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

#[cfg(test)]
mod tests_talus {
    use super::*;

    #[test]
    fn talus_arrete_au_droit_d_une_arete_sans_talus() {
        // arête 1 (y = 0) sans talus : aucun remblai au-delà de son prolongement
        let (mut p, _) = tests::projet(Some(angle(1.0)));
        p.dalles[0].definir_pente(0, 0, None);
        let r = calculer(&p, &p.dalles[0]).unwrap();
        assert!(r.tris.iter().flatten().all(|v| v.y > -1.0), "remblai au-delà de la paroi");
        // 100 + 3 prismes (10 × 0,5) + 2 pyramides d'angle (côté opposé)
        let attendu = 100.0 + 15.0 + 2.0 / 3.0;
        assert!((r.volume / 1e9 - attendu).abs() < 0.3, "{}", r.volume / 1e9);
    }

    #[test]
    fn talus_depuis_l_arete_du_dessus() {
        // dessus à +0,2 m, sous-face à 0, terrain à -1 m : talus de 1,2 m
        let (mut p, _) = tests::projet(Some(angle(1.0)));
        p.dalles[0].talus_dessus = true;
        let r = calculer(&p, &p.dalles[0]).unwrap();
        let attendu = 100.0 + 4.0 * 10.0 * 1.2 * 1.2 / 2.0 + 4.0 * 1.2f64.powi(3) / 3.0;
        assert!((r.volume / 1e9 - attendu).abs() < 0.4, "{} / {attendu}", r.volume / 1e9);
    }
}

#[cfg(test)]
mod tests_rebouchage {
    use super::*;

    /// Terre-plein enterré : terrain à -1 m, fond de forme à -2 m, talus 1/1.
    fn enterre() -> Projet {
        let (mut p, _) = tests::projet(Some(angle(1.0)));
        p.dalles[0].decalage = -1800.0;
        p
    }

    #[test]
    fn fouille_rebouchee_sur_toutes_les_aretes() {
        let mut p = enterre();
        for k in 0..4 { p.dalles[0].definir_remblai(0, k, true); }
        let r = calculer(&p, &p.dalles[0]).unwrap();
        // 4 prismes (10 × 0,5) et 4 pyramides d'angle (1/3) ; le déblai ne change pas
        let attendu = 20.0 + 4.0 / 3.0;
        assert!((r.rebouchage / 1e9 - attendu).abs() < 0.3, "{}", r.rebouchage / 1e9);
        assert!((r.volume - r.rebouchage).abs() < 1e6);
        assert!((r.deblai / 1e9 - (100.0 + attendu)).abs() < 0.3, "{}", r.deblai / 1e9);
        // dessus au terrain naturel, fond à l'arête de départ
        let (bas, haut) = r.tris.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v.z), b.max(v.z)));
        assert!((bas + 2000.0).abs() < 1.0 && (haut + 1000.0).abs() < 1.0, "{bas} {haut}");
    }

    #[test]
    fn fouille_rebouchee_sur_une_arete() {
        // arête 1 (y = 0) seule : son prisme et la moitié des deux pyramides d'angle
        let mut p = enterre();
        p.dalles[0].definir_remblai(0, 0, true);
        let r = calculer(&p, &p.dalles[0]).unwrap();
        let attendu = 5.0 + 2.0 / 6.0;
        assert!((r.rebouchage / 1e9 - attendu).abs() < 0.1, "{}", r.rebouchage / 1e9);
        assert!(r.tris.iter().flatten().all(|v| v.y < 1.0), "rebouchage hors de l'arête");
        // solide fermé : flancs le long des arêtiers, paroi contre la plate-forme
        assert!((geometrie::volume(&r.tris) / 1e9 - attendu).abs() < 0.1, "{}", geometrie::volume(&r.tris) / 1e9);
    }

    #[test]
    fn remblai_sans_effet_hors_fouille() {
        // plate-forme au-dessus du terrain : rien à reboucher
        let (mut p, _) = tests::projet(Some(angle(1.0)));
        for k in 0..4 { p.dalles[0].definir_remblai(0, k, true); }
        let r = calculer(&p, &p.dalles[0]).unwrap();
        assert!(r.rebouchage < 1e6, "{}", r.rebouchage);
    }
}
