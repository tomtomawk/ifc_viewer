//! Arbres : élément de site placé au clic (niveau, position), défini par son essence
//! (silhouette), sa hauteur et le diamètre de son houppier. Géométrie générée : tronc
//! (cylindre à 8 pans) et houppier en grappe de volumes bosselés, différente d'un arbre
//! à l'autre (tirée de l'identifiant). En plan : cercle du houppier et tronc.
//!
//! Plus tard, `asset` désignera un modèle de bibliothèque qui remplacera cette
//! géométrie dans le style Réaliste (comme les assets d'Enscape).
use glam::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Essence {
    /// Houppier arrondi (chêne, tilleul, érable).
    #[default]
    Feuillu,
    /// Cône (pin, sapin, cèdre).
    Conifere,
    /// Colonnaire (peuplier d'Italie, charme fastigié).
    Fastigie,
}

impl Essence {
    pub const TOUTES: [Essence; 3] = [Essence::Feuillu, Essence::Conifere, Essence::Fastigie];
    pub fn nom(self) -> &'static str {
        match self {
            Essence::Feuillu => "Feuillu",
            Essence::Conifere => "Conifère",
            Essence::Fastigie => "Fastigié",
        }
    }
    /// Hauteur et diamètre du houppier par défaut (mm).
    pub fn dimensions(self) -> (f64, f64) {
        match self {
            Essence::Feuillu => (10_000.0, 7_000.0),
            Essence::Conifere => (12_000.0, 5_000.0),
            Essence::Fastigie => (14_000.0, 3_000.0),
        }
    }
    /// Couleur du feuillage sans matériau « Feuillage » dans le projet.
    pub fn couleur(self) -> [u8; 3] {
        match self {
            Essence::Feuillu => [86, 120, 52],
            Essence::Conifere => [64, 100, 60],
            Essence::Fastigie => [96, 128, 56],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Arbre {
    pub id: String,
    #[serde(default)]
    pub nom: String,
    pub niveau: String,
    /// Pied du tronc (mm, plan).
    pub position: [f64; 2],
    /// Décalage vertical depuis le niveau (mm), pour un arbre sur un talus.
    #[serde(default)]
    pub decalage: f64,
    #[serde(default)]
    pub essence: Essence,
    pub hauteur: f64,
    /// Diamètre du houppier (mm).
    pub diametre: f64,
    /// Modèle de bibliothèque (rendu Réaliste) ; None : géométrie générée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
}

/// Matériaux des arbres, s'ils existent dans le projet (sinon couleurs par défaut).
pub const MATERIAU_FEUILLAGE: &str = "Feuillage";
pub const MATERIAU_ECORCE: &str = "Écorce";

/// Générateur pseudo-aléatoire déterministe (tiré de l'identifiant).
struct Alea(u64);
impl Alea {
    fn de(texte: &str) -> Self {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in texte.bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
        Alea(h | 1)
    }
    /// Uniforme dans [0, 1[.
    fn f(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn entre(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.f()
    }
}

/// Icosphère (2 subdivisions) déformée : rayons (rx, ry, rz), bosses de ±`relief`.
fn boule(c: DVec3, r: DVec3, relief: f64, alea: &mut Alea) -> Vec<[DVec3; 3]> {
    let t = (1.0 + 5f64.sqrt()) / 2.0;
    let mut v: Vec<DVec3> = [
        [-1.0, t, 0.0], [1.0, t, 0.0], [-1.0, -t, 0.0], [1.0, -t, 0.0],
        [0.0, -1.0, t], [0.0, 1.0, t], [0.0, -1.0, -t], [0.0, 1.0, -t],
        [t, 0.0, -1.0], [t, 0.0, 1.0], [-t, 0.0, -1.0], [-t, 0.0, 1.0],
    ]
    .iter()
    .map(|p| DVec3::from_array(*p).normalize())
    .collect();
    let mut f: Vec<[usize; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    for _ in 0..2 {
        let mut milieux = std::collections::HashMap::new();
        let mut milieu = |a: usize, b: usize, v: &mut Vec<DVec3>| {
            *milieux.entry((a.min(b), a.max(b))).or_insert_with(|| {
                v.push(((v[a] + v[b]) * 0.5).normalize());
                v.len() - 1
            })
        };
        let mut g = Vec::with_capacity(f.len() * 4);
        for [a, b, c] in f {
            let (ab, bc, ca) = (milieu(a, b, &mut v), milieu(b, c, &mut v), milieu(c, a, &mut v));
            g.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        f = g;
    }
    let bosses: Vec<f64> = (0..v.len()).map(|_| 1.0 + alea.entre(-relief, relief)).collect();
    let p: Vec<DVec3> = v.iter().zip(&bosses).map(|(d, k)| c + *d * r * *k).collect();
    f.into_iter().map(|[a, b, c]| [p[a], p[b], p[c]]).collect()
}

/// Cylindre à `n` pans, de `bas` à `haut` (rayons r0 en bas, r1 en haut), sans fond.
fn tronc(bas: DVec3, haut: f64, r0: f64, r1: f64, n: usize) -> Vec<[DVec3; 3]> {
    let mut out = Vec::new();
    for k in 0..n {
        let (a, b) = (k as f64 / n as f64 * std::f64::consts::TAU, (k + 1) as f64 / n as f64 * std::f64::consts::TAU);
        let p = |ang: f64, r: f64, z: f64| bas + DVec3::new(ang.cos() * r, ang.sin() * r, z);
        let (a0, b0, a1, b1) = (p(a, r0, 0.0), p(b, r0, 0.0), p(a, r1, haut), p(b, r1, haut));
        out.push([a0, b0, b1]);
        out.push([a0, b1, a1]);
    }
    out
}

/// Plan de feuillage carré de côté `c`, centré en `centre`, de normale `n`.
fn carte(centre: DVec3, n: DVec3, c: f64, tourne: f64) -> [[DVec3; 3]; 2] {
    let n = n.normalize_or(DVec3::Z);
    let t0 = if n.z.abs() < 0.9 { n.cross(DVec3::Z) } else { n.cross(DVec3::X) }.normalize();
    let b0 = n.cross(t0);
    let (s, co) = tourne.sin_cos();
    let (t, b) = (t0 * co + b0 * s, b0 * co - t0 * s);
    let h = c * 0.5;
    let p = |x: f64, y: f64| centre + t * x * h + b * y * h;
    [[p(-1.0, -1.0), p(1.0, -1.0), p(1.0, 1.0)], [p(-1.0, -1.0), p(1.0, 1.0), p(-1.0, 1.0)]]
}

/// Point tiré uniformément dans la boule unité.
fn dans_boule(alea: &mut Alea) -> DVec3 {
    loop {
        let p = DVec3::new(alea.entre(-1.0, 1.0), alea.entre(-1.0, 1.0), alea.entre(-1.0, 1.0));
        if p.length_squared() <= 1.0 {
            return p;
        }
    }
}

/// Géométrie de remplacement (style Réaliste) : tronc et branches, plans de feuillage
/// (texture de feuilles découpée par le matériau « Feuillage »).
pub struct Remplacement {
    pub bois: Vec<[DVec3; 3]>,
    pub feuillage: Vec<[DVec3; 3]>,
}

impl Arbre {
    /// Arbre de remplacement du style Réaliste (mm, pied à l'altitude `z0`) : tronc,
    /// branches vers le houppier et 150 à 260 plans de feuillage orientés au hasard dans
    /// son volume (ellipsoïde, cône ou colonne selon l'essence).
    pub fn remplacement(&self, z0: f64) -> Remplacement {
        let mut alea = Alea::de(&format!("{}-feuillage", self.id));
        let pied = DVec3::new(self.position[0], self.position[1], z0 + self.decalage);
        let (h, d) = (self.hauteur.max(500.0), self.diametre.max(300.0));
        let rt = (d * 0.035).clamp(60.0, 400.0);
        let mut bois = Vec::new();
        let mut feuillage = Vec::new();
        // volume du houppier : centre, demi-axes, forme
        let (fut, centre, rh, rv) = match self.essence {
            Essence::Feuillu => (h * 0.35, h * 0.35 + (h - h * 0.35) * 0.5, d * 0.5, (h - h * 0.35) * 0.5),
            Essence::Conifere => (h * 0.12, h * 0.12 + (h - h * 0.12) * 0.5, d * 0.5, (h - h * 0.12) * 0.5),
            Essence::Fastigie => (h * 0.1, h * 0.1 + (h - h * 0.1) * 0.5, d * 0.5, (h - h * 0.1) * 0.5),
        };
        let haut_tronc = match self.essence { Essence::Conifere => h * 0.92, _ => fut + rv * 0.9 };
        bois.extend(tronc(pied, haut_tronc, rt, rt * 0.35, 10));
        // branches : du tronc vers la périphérie du houppier
        let nb_branches = match self.essence { Essence::Feuillu => 7, Essence::Conifere => 10, Essence::Fastigie => 5 };
        for k in 0..nb_branches {
            let z = fut + (haut_tronc - fut) * (k as f64 + 0.5) / nb_branches as f64 * 0.8;
            let a = k as f64 * 2.39996 + alea.entre(-0.3, 0.3);
            let portee = rh * match self.essence { Essence::Conifere => 0.9 * (1.0 - (z - fut) / (h - fut)), _ => 0.65 };
            let depart = pied + DVec3::Z * z;
            let fin = depart + DVec3::new(a.cos() * portee, a.sin() * portee, portee * 0.5);
            let dir = fin - depart;
            let l = dir.length();
            // cylindre orienté : rebâti dans le repère de la branche
            let n = dir / l;
            let t = n.cross(DVec3::Z).normalize_or(DVec3::X);
            let b = n.cross(t);
            let r0 = rt * 0.35;
            for i in 0..6 {
                let (u0, u1) = (i as f64 / 6.0 * std::f64::consts::TAU, (i + 1) as f64 / 6.0 * std::f64::consts::TAU);
                let p = |u: f64, r: f64, s: f64| depart + n * s + (t * u.cos() + b * u.sin()) * r;
                bois.push([p(u0, r0, 0.0), p(u1, r0, 0.0), p(u1, r0 * 0.3, l)]);
                bois.push([p(u0, r0, 0.0), p(u1, r0 * 0.3, l), p(u0, r0 * 0.3, l)]);
            }
        }
        // plans de feuillage
        let (n_cartes, cote) = match self.essence {
            Essence::Feuillu => ((d * d / 220_000.0) as usize, d * 0.2),
            Essence::Conifere => ((d * h / 260_000.0) as usize, d * 0.22),
            Essence::Fastigie => ((d * h / 160_000.0) as usize, d * 0.38),
        };
        for _ in 0..n_cartes.clamp(120, 260) {
            let q = dans_boule(&mut alea);
            let c = match self.essence {
                Essence::Conifere => {
                    // cône : rayon décroissant avec la hauteur
                    let zr = alea.entre(-1.0, 0.95);
                    let rmax = (1.0 - (zr + 1.0) / 2.0) * 0.95;
                    DVec3::new(q.x * rh * rmax, q.y * rh * rmax, zr * rv)
                }
                _ => {
                    // vers la surface : le cœur du houppier reste plus clair
                    let r = q.length().max(1e-6);
                    let q = q / r * r.powf(0.45) * 0.88;
                    DVec3::new(q.x * rh, q.y * rh, q.z * rv)
                }
            };
            let centre_carte = pied + DVec3::Z * centre + c;
            // orientation : surtout vers l'extérieur, un peu au hasard
            let n = (DVec3::new(c.x / rh, c.y / rh, c.z / rv * 0.6) + dans_boule(&mut alea) * 0.9).normalize_or(DVec3::Z);
            let tc = cote * alea.entre(0.75, 1.2);
            feuillage.extend(carte(centre_carte, n, tc, alea.entre(0.0, std::f64::consts::TAU)));
        }
        Remplacement { bois, feuillage }
    }

    /// Géométrie (mm, repère du projet) : tronc et houppier, pied à l'altitude `z0`.
    pub fn solides(&self, z0: f64) -> (Vec<[DVec3; 3]>, Vec<[DVec3; 3]>) {
        let mut alea = Alea::de(&self.id);
        let pied = DVec3::new(self.position[0], self.position[1], z0 + self.decalage);
        let (h, d) = (self.hauteur.max(500.0), self.diametre.max(300.0));
        let rt = (d * 0.035).clamp(60.0, 400.0);
        match self.essence {
            Essence::Feuillu => {
                let fut = h * 0.35;
                let corps = tronc(pied, fut + (h - fut) * 0.3, rt, rt * 0.6, 8);
                // grappe : un volume central et 6 à 8 autour
                let centre = pied + DVec3::Z * (fut + (h - fut) * 0.5);
                let (rh, rv) = (d * 0.5, (h - fut) * 0.5);
                let mut houppier = boule(centre, DVec3::new(rh * 0.7, rh * 0.7, rv * 0.75), 0.12, &mut alea);
                let n = 6 + (alea.f() * 3.0) as usize;
                for k in 0..n {
                    let a = (k as f64 + alea.entre(-0.3, 0.3)) / n as f64 * std::f64::consts::TAU;
                    let z = alea.entre(-0.35, 0.45) * rv;
                    let ecart = rh * alea.entre(0.35, 0.55);
                    let c = centre + DVec3::new(a.cos() * ecart, a.sin() * ecart, z);
                    let r = rh * alea.entre(0.38, 0.5);
                    houppier.extend(boule(c, DVec3::new(r, r, r * alea.entre(0.8, 1.0)), 0.15, &mut alea));
                }
                (corps, houppier)
            }
            Essence::Conifere => {
                // trois cônes lisses (16 pans), du plus large au plus étroit, sans aspérité
                let fut = h * 0.12;
                let corps = tronc(pied, h * 0.6, rt, rt * 0.5, 8);
                let mut houppier = Vec::new();
                let n = 16;
                for (z, r, hc) in [(0.0, 0.5, 0.5), (0.3, 0.38, 0.42), (0.55, 0.26, 0.45)] {
                    let base = pied + DVec3::Z * (fut + (h - fut) * z);
                    let (rb, hh) = (d * r, (h - fut) * hc);
                    let sommet = base + DVec3::Z * hh;
                    for i in 0..n {
                        let (a, b) = (i as f64 / n as f64 * std::f64::consts::TAU, (i + 1) as f64 / n as f64 * std::f64::consts::TAU);
                        let p = |ang: f64| base + DVec3::new(ang.cos() * rb, ang.sin() * rb, 0.0);
                        houppier.push([p(a), p(b), sommet]);
                        houppier.push([p(b), p(a), base]);
                    }
                }
                let _ = &mut alea;
                (corps, houppier)
            }
            Essence::Fastigie => {
                let fut = h * 0.1;
                let corps = tronc(pied, h * 0.5, rt, rt * 0.6, 8);
                let centre = pied + DVec3::Z * (fut + (h - fut) * 0.5);
                let (rh, rv) = (d * 0.5, (h - fut) * 0.5);
                let mut houppier = boule(centre, DVec3::new(rh, rh, rv), 0.1, &mut alea);
                for k in 0..4 {
                    let z = (k as f64 / 3.0 - 0.5) * rv * 1.2;
                    let a = alea.entre(0.0, std::f64::consts::TAU);
                    let c = centre + DVec3::new(a.cos(), a.sin(), 0.0) * rh * 0.35 + DVec3::Z * z;
                    let r = rh * alea.entre(0.55, 0.7);
                    houppier.extend(boule(c, DVec3::new(r, r, r * 1.6), 0.12, &mut alea));
                }
                (corps, houppier)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arbre(essence: Essence) -> Arbre {
        let (hauteur, diametre) = essence.dimensions();
        Arbre { id: "a1".into(), nom: String::new(), niveau: "n0".into(), position: [1000.0, 2000.0], decalage: 0.0, essence, hauteur, diametre, asset: None }
    }

    #[test]
    fn dimensions_respectees() {
        for e in Essence::TOUTES {
            let a = arbre(e);
            let (t, h) = a.solides(0.0);
            assert!(!t.is_empty() && !h.is_empty());
            let pts: Vec<DVec3> = t.iter().chain(&h).flatten().copied().collect();
            let zmax = pts.iter().map(|p| p.z).fold(f64::MIN, f64::max);
            let rmax = pts.iter().map(|p| ((p.x - 1000.0).powi(2) + (p.y - 2000.0).powi(2)).sqrt()).fold(0.0, f64::max);
            assert!((zmax - a.hauteur).abs() < a.hauteur * 0.15, "{e:?} hauteur {zmax}");
            assert!(rmax < a.diametre * 0.62 && rmax > a.diametre * 0.35, "{e:?} rayon {rmax}");
            assert!(pts.iter().map(|p| p.z).fold(f64::MAX, f64::min) >= -1.0);
        }
    }

    #[test]
    fn deux_arbres_differents() {
        let a = arbre(Essence::Feuillu);
        let b = Arbre { id: "a2".into(), ..a.clone() };
        assert_ne!(a.solides(0.0).1[5][0], b.solides(0.0).1[5][0]);
        assert_eq!(a.solides(0.0).1[7], a.solides(0.0).1[7]);
    }

    #[test]
    fn remplacement_dans_le_volume() {
        for e in Essence::TOUTES {
            let a = arbre(e);
            let r = a.remplacement(0.0);
            assert!(r.feuillage.len() >= 240 && !r.bois.is_empty());
            let pts: Vec<DVec3> = r.feuillage.iter().flatten().copied().collect();
            let zmax = pts.iter().map(|p| p.z).fold(f64::MIN, f64::max);
            let rmax = pts.iter().map(|p| ((p.x - 1000.0).powi(2) + (p.y - 2000.0).powi(2)).sqrt()).fold(0.0, f64::max);
            assert!(zmax < a.hauteur * 1.12 && zmax > a.hauteur * 0.8, "{e:?} {zmax}");
            assert!(rmax < a.diametre * 0.75, "{e:?} {rmax}");
        }
    }
}
