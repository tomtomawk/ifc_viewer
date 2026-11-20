//! Générateur intégré de végétaux pour le style Réaliste : essences (silhouettes) et
//! géométrie de remplacement (tronc, branches, plans de feuillage) tirée d'une graine.
//! Les familles de la catégorie Végétation le désignent par leur paramètre `Rendu`
//! (`vegetation::REFERENCES`).
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

/// Végétal de remplacement du style Réaliste (mm) pour une essence, pied en `pied`,
/// hauteur `h`, diamètre du houppier `d` : tronc, branches et plans de feuillage tirés
/// d'une suite pseudo-aléatoire déterminée par `graine` (même végétal à chaque calcul).
pub fn remplacement_vegetal(essence: Essence, pied: DVec3, h: f64, d: f64, graine: &str) -> Remplacement {
    {
        let mut alea = Alea::de(&format!("{graine}-feuillage"));
        let (h, d) = (h.max(500.0), d.max(300.0));
        let rt = (d * 0.035).clamp(60.0, 400.0);
        let mut bois = Vec::new();
        let mut feuillage = Vec::new();
        // volume du houppier : centre, demi-axes, forme
        let (fut, centre, rh, rv) = match essence {
            Essence::Feuillu => (h * 0.35, h * 0.35 + (h - h * 0.35) * 0.5, d * 0.5, (h - h * 0.35) * 0.5),
            Essence::Conifere => (h * 0.12, h * 0.12 + (h - h * 0.12) * 0.5, d * 0.5, (h - h * 0.12) * 0.5),
            Essence::Fastigie => (h * 0.1, h * 0.1 + (h - h * 0.1) * 0.5, d * 0.5, (h - h * 0.1) * 0.5),
        };
        let haut_tronc = match essence { Essence::Conifere => h * 0.92, _ => fut + rv * 0.9 };
        bois.extend(tronc(pied, haut_tronc, rt, rt * 0.35, 10));
        // branches : du tronc vers la périphérie du houppier
        let nb_branches = match essence { Essence::Feuillu => 7, Essence::Conifere => 10, Essence::Fastigie => 5 };
        for k in 0..nb_branches {
            let z = fut + (haut_tronc - fut) * (k as f64 + 0.5) / nb_branches as f64 * 0.8;
            let a = k as f64 * 2.39996 + alea.entre(-0.3, 0.3);
            let portee = rh * match essence { Essence::Conifere => 0.9 * (1.0 - (z - fut) / (h - fut)), _ => 0.65 };
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
        let (n_cartes, cote) = match essence {
            Essence::Feuillu => ((d * d / 220_000.0) as usize, d * 0.2),
            Essence::Conifere => ((d * h / 260_000.0) as usize, d * 0.22),
            Essence::Fastigie => ((d * h / 160_000.0) as usize, d * 0.38),
        };
        for _ in 0..n_cartes.clamp(120, 260) {
            let q = dans_boule(&mut alea);
            let c = match essence {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remplacement_dans_le_volume() {
        for e in Essence::TOUTES {
            let (hauteur, diametre) = e.dimensions();
            let r = remplacement_vegetal(e, DVec3::new(1000.0, 2000.0, 0.0), hauteur, diametre, "a1");
            assert!(r.feuillage.len() >= 240 && !r.bois.is_empty());
            let pts: Vec<DVec3> = r.feuillage.iter().flatten().copied().collect();
            let zmax = pts.iter().map(|p| p.z).fold(f64::MIN, f64::max);
            let rmax = pts.iter().map(|p| ((p.x - 1000.0).powi(2) + (p.y - 2000.0).powi(2)).sqrt()).fold(0.0, f64::max);
            assert!(zmax < hauteur * 1.12 && zmax > hauteur * 0.8, "{e:?} {zmax}");
            assert!(rmax < diametre * 0.75, "{e:?} {rmax}");
        }
    }
}
