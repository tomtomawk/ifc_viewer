//! Motif de hachures dessiné sur une tuile carrée répétée : segments accrochés à une grille
//! de `divisions` × `divisions` nœuds sur un carré de côté `cote` (mm du papier). La tuile
//! se convertit exactement en familles de traits (comme un fichier .pat de Revit) : chaque
//! segment, répété sur le réseau des tuiles, donne une famille de droites parallèles
//! (direction entière de la grille), avec son pas, son glissement et ses tirets.
use crate::LigneMotif;
use serde::{Deserialize, Serialize};

/// Nombre de divisions de la grille au plus.
pub const DIVISIONS_MAX: u32 = 64;

/// Tuile d'un motif dessiné.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TuileMotif {
    /// Côté du carré répété (mm du papier).
    pub cote: f64,
    /// Nœuds de la grille par côté (pas de la grille : `cote / divisions`).
    pub divisions: u32,
    /// Segments, extrémités en nœuds de la grille (0 à `divisions`).
    pub segments: Vec<[[i32; 2]; 2]>,
}

/// (pgcd, x, y) avec a·x + b·y = pgcd (a, b ≥ 0, non tous deux nuls).
fn euclide(a: i64, b: i64) -> (i64, i64, i64) {
    if b == 0 { (a, 1, 0) } else { let (g, x, y) = euclide(b, a % b); (g, y, x - (a / b) * y) }
}

impl TuileMotif {
    /// Contrôle : côté positif et fini, divisions de 1 à `DIVISIONS_MAX`, extrémités sur
    /// la grille, segments non nuls.
    pub fn valider(&self) -> Result<(), String> {
        if !(self.cote.is_finite() && self.cote > 0.0) {
            return Err("Motif dessiné : côté de la tuile non valide.".into());
        }
        if !(1..=DIVISIONS_MAX).contains(&self.divisions) {
            return Err(format!("Motif dessiné : de 1 à {DIVISIONS_MAX} divisions."));
        }
        let n = self.divisions as i32;
        if self.segments.iter().flatten().flatten().any(|c| !(0..=n).contains(c)) {
            return Err("Motif dessiné : extrémité hors de la tuile.".into());
        }
        if self.segments.iter().any(|s| s[0] == s[1]) {
            return Err("Motif dessiné : segment de longueur nulle.".into());
        }
        Ok(())
    }

    /// Familles de traits équivalentes (mm du papier) : une par segment.
    pub fn lignes(&self) -> Vec<LigneMotif> {
        let a = self.cote;
        let h = a / self.divisions.max(1) as f64;
        self.segments
            .iter()
            .filter(|s| s[0] != s[1])
            .map(|[d, f]| {
                let (vx, vy) = ((f[0] - d[0]) as i64, (f[1] - d[1]) as i64);
                let (g, _, _) = euclide(vx.abs(), vy.abs());
                // direction entière réduite (p, q) et vecteur (r, s) du réseau tel que
                // p·s − q·r = 1 : ligne voisine du réseau, à la distance du pas
                let (p, q) = (vx / g, vy / g);
                // |p|·x + |q|·y = 1, d'où p·(x·sgn p) + q·(y·sgn q) = 1 : s = x·sgn p, r = −y·sgn q
                let (_, x, y) = euclide(p.abs(), q.abs());
                let (s, r) = (x * p.signum(), -y * q.signum());
                let norme = ((p * p + q * q) as f64).sqrt();
                let periode = a * norme;
                let pas = a / norme;
                let glissement = (a * (p * r + q * s) as f64 / norme).rem_euclid(periode);
                let longueur = g as f64 * norme * h;
                let tirets = if longueur >= periode - 1e-9 { Vec::new() } else { vec![longueur, -(periode - longueur)] };
                LigneMotif {
                    angle: (q as f64).atan2(p as f64).to_degrees(),
                    pas,
                    glissement,
                    origine: [d[0] as f64 * h, d[1] as f64 * h],
                    tirets,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Points tracés par une famille de traits dans un carré [0, l]² (échantillonnés).
    fn traces(l: &LigneMotif, cote: f64) -> Vec<[f64; 2]> {
        let (d, n) = ([l.angle.to_radians().cos(), l.angle.to_radians().sin()], [-l.angle.to_radians().sin(), l.angle.to_radians().cos()]);
        let periode: f64 = l.tirets.iter().map(|t| t.abs()).sum();
        let mut out = Vec::new();
        for k in -60..=60 {
            let o = [l.origine[0] + k as f64 * (l.pas * n[0] + l.glissement * d[0]), l.origine[1] + k as f64 * (l.pas * n[1] + l.glissement * d[1])];
            for i in -600..=600 {
                let t = i as f64 * 0.05;
                let dans_trait = periode == 0.0 || t.rem_euclid(periode) <= l.tirets[0] + 1e-9;
                let p = [o[0] + t * d[0], o[1] + t * d[1]];
                if dans_trait && (0.0..=cote).contains(&p[0]) && (0.0..=cote).contains(&p[1]) {
                    out.push(p);
                }
            }
        }
        out
    }

    /// Chaque point tracé par la famille d'un segment appartient à une copie du segment sur
    /// le réseau des tuiles ; le segment d'origine est bien tracé.
    #[test]
    fn familles_exactes() {
        let cote = 4.0;
        for seg in [[[0, 0], [8, 0]], [[0, 0], [0, 8]], [[1, 1], [5, 3]], [[2, 6], [6, 2]], [[0, 0], [8, 8]], [[3, 1], [3, 5]], [[7, 2], [1, 5]]] {
            let t = TuileMotif { cote, divisions: 8, segments: vec![seg] };
            t.valider().unwrap();
            let l = &t.lignes()[0];
            let h = cote / 8.0;
            let (a, b) = ([seg[0][0] as f64 * h, seg[0][1] as f64 * h], [seg[1][0] as f64 * h, seg[1][1] as f64 * h]);
            let sur_copie = |p: [f64; 2]| (-3..=3).any(|i| (-3..=3).any(|j| {
                let (a, b) = ([a[0] + i as f64 * cote, a[1] + j as f64 * cote], [b[0] + i as f64 * cote, b[1] + j as f64 * cote]);
                let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
                let l2 = ab[0] * ab[0] + ab[1] * ab[1];
                let t = (ap[0] * ab[0] + ap[1] * ab[1]) / l2;
                let ecart = (ap[0] * ab[1] - ap[1] * ab[0]).abs() / l2.sqrt();
                (-1e-6..=1.0 + 1e-6).contains(&t) && ecart < 1e-6
            }));
            let pts = traces(l, cote);
            assert!(!pts.is_empty(), "{seg:?}");
            for p in &pts {
                assert!(sur_copie(*p), "{seg:?} : point {p:?} hors des copies du segment ({l:?})");
            }
            // le milieu du segment d'origine est tracé
            let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            assert!(pts.iter().any(|p| (p[0] - m[0]).hypot(p[1] - m[1]) < 0.06), "{seg:?} : milieu non tracé ({l:?})");
        }
    }

    #[test]
    fn tuile_non_valide() {
        let t = |segments| TuileMotif { cote: 4.0, divisions: 8, segments };
        assert!(t(vec![[[0, 0], [9, 0]]]).valider().is_err());
        assert!(t(vec![[[2, 2], [2, 2]]]).valider().is_err());
        assert!(TuileMotif { cote: 0.0, divisions: 8, segments: vec![] }.valider().is_err());
    }
}
