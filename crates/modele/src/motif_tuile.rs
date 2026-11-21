//! Motif de hachures dessiné sur une tuile carrée répétée : segments accrochés à une grille
//! de `divisions` × `divisions` nœuds sur un carré de côté `cote` (mm du papier). La tuile
//! se convertit exactement en familles de traits (comme un fichier .pat de Revit) : chaque
//! segment, répété sur le réseau des tuiles, donne une famille de droites parallèles
//! (direction entière de la grille), avec son pas, son glissement et ses tirets.
use crate::LigneMotif;
use serde::{Deserialize, Serialize};

/// Nombre de divisions de la grille au plus.
pub const DIVISIONS_MAX: u32 = 240;

/// Divisions des motifs tracés dans leur vue : 240 (multiple de 2, 3, 4, 5, 6, 8, 10, 12,
/// 15, 16 : coins, milieux, tiers et quarts exacts), invisibles à l'écran.
pub const DIVISIONS_VUE: u32 = 240;

/// Direction la plus fine gardée telle quelle (composantes entières réduites) ; au-delà,
/// la famille de traits serait trop serrée pour être dessinée : la direction est
/// remplacée par la plus proche dans cette limite.
pub const DIRECTION_MAX: i64 = 24;

/// Côté du carré de la tuile dans la vue de dessin du motif (mm de la vue), centré sur
/// l'origine de la vue ; indépendant du côté papier du motif.
pub const TAILLE_VUE: f64 = 400.0;

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
        // un trait peut déborder du carré (ses répétitions se recouvrent), pas au-delà
        // de dix carrés
        let n = self.divisions as i32;
        if self.segments.iter().flatten().flatten().any(|c| c.abs() > 11 * n) {
            return Err("Motif dessiné : extrémité trop loin du carré.".into());
        }
        if self.segments.iter().any(|s| s[0] == s[1]) {
            return Err("Motif dessiné : segment de longueur nulle.".into());
        }
        Ok(())
    }

    /// Traits de la tuile dans la vue du motif : carré de [`TAILLE_VUE`] centré sur
    /// l'origine de la vue.
    pub fn traits_vue(&self) -> Vec<[[f64; 2]; 2]> {
        let pas = TAILLE_VUE / self.divisions.max(1) as f64;
        self.segments.iter().map(|s| s.map(|q| q.map(|c| c as f64 * pas - TAILLE_VUE / 2.0))).collect()
    }

    /// Tuile tirée des traits de la vue du motif, sur [`DIVISIONS_VUE`] divisions :
    /// extrémités arrondies au 1/240 du carré, direction ramenée au besoin dans
    /// [`DIRECTION_MAX`] ; un trait et ses répétitions (décalé d'un nombre entier de carrés,
    /// ex. bord bas et bord haut) ne comptent qu'une fois. Rend aussi le nombre de traits
    /// dont la direction a été corrigée.
    pub fn depuis_traits(cote: f64, traits: &[[[f64; 2]; 2]]) -> Result<(Self, usize), String> {
        let n = DIVISIONS_VUE as i64;
        let pas = TAILLE_VUE / n as f64;
        let noeud = |q: [f64; 2]| q.map(|c| ((c + TAILLE_VUE / 2.0) / pas).round() as i64);
        let mut segments: Vec<[[i32; 2]; 2]> = Vec::new();
        let mut corriges = 0;
        for t in traits {
            if t.iter().flatten().any(|c| !c.is_finite() || c.abs() > 10.0 * TAILLE_VUE) {
                return Err("Motif dessiné : trait trop loin du carré.".into());
            }
            let (a, mut b) = (noeud(t[0]), noeud(t[1]));
            let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
            if vx == 0 && vy == 0 { continue; }
            let g = euclide(vx.abs(), vy.abs()).0;
            if (vx / g).abs().max((vy / g).abs()) > DIRECTION_MAX {
                // direction entière proche, même sens ; longueur conservée au mieux
                let angle = (vy as f64).atan2(vx as f64);
                let (p, q) = (-DIRECTION_MAX..=DIRECTION_MAX)
                    .flat_map(|p| (-DIRECTION_MAX..=DIRECTION_MAX).map(move |q| (p, q)))
                    .filter(|&(p, q)| (p, q) != (0, 0) && euclide(p.abs(), q.abs()).0 == 1)
                    .min_by(|x, y| {
                        let e = |(p, q): (i64, i64)| { let d = (q as f64).atan2(p as f64) - angle; d.sin().abs() + (1.0 - d.cos()) };
                        e(*x).total_cmp(&e(*y))
                    })
                    .unwrap_or((1, 0));
                let k = ((((vx * vx + vy * vy) as f64) / ((p * p + q * q) as f64)).sqrt()).round().max(1.0) as i64;
                b = [a[0] + k * p, a[1] + k * q];
                corriges += 1;
            }
            // extrémités ordonnées, première ramenée dans le carré : forme unique d'un
            // trait et de ses répétitions
            let (a, b) = if (a[0], a[1]) <= (b[0], b[1]) { (a, b) } else { (b, a) };
            let d = [a[0].div_euclid(n) * n, a[1].div_euclid(n) * n];
            let s = [[(a[0] - d[0]) as i32, (a[1] - d[1]) as i32], [(b[0] - d[0]) as i32, (b[1] - d[1]) as i32]];
            if !segments.contains(&s) {
                segments.push(s);
            }
        }
        let tuile = TuileMotif { cote, divisions: DIVISIONS_VUE, segments };
        tuile.valider()?;
        Ok((tuile, corriges))
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

    /// Traits de la vue → tuile : extrémités au 1/240, trait du bord haut confondu avec
    /// celui du bord bas, direction trop fine corrigée, aller-retour exact.
    #[test]
    fn traits_de_la_vue() {
        let h = TAILLE_VUE / 2.0;
        let (bas, haut, diagonale) = ([[-h, -h], [h, -h]], [[h, h], [-h, h]], [[-h, -h], [h, h]]);
        let (t, corriges) = TuileMotif::depuis_traits(3.0, &[bas, haut, diagonale]).unwrap();
        assert_eq!((t.segments.len(), corriges), (2, 0));
        assert_eq!(t.segments[0], [[0, 0], [240, 0]]);
        assert_eq!(TuileMotif::depuis_traits(3.0, &t.traits_vue()).unwrap().0, t);
        // presque horizontal : direction (239, 1) ramenée à l'horizontale
        let (t, corriges) = TuileMotif::depuis_traits(3.0, &[[[-h, 0.0], [h - 1.0, 1.6]]]).unwrap();
        assert_eq!(corriges, 1);
        assert_eq!(t.segments[0][1][1], t.segments[0][0][1]);
        // un trait qui déborde est admis ; un trait à plus de dix carrés, non
        assert!(TuileMotif::depuis_traits(3.0, &[[[0.0, 0.0], [TAILLE_VUE, 0.0]]]).is_ok());
        assert!(TuileMotif::depuis_traits(3.0, &[[[0.0, 0.0], [20.0 * TAILLE_VUE, 0.0]]]).is_err());
    }

    #[test]
    fn tuile_non_valide() {
        let t = |segments| TuileMotif { cote: 4.0, divisions: 8, segments };
        assert!(t(vec![[[0, 0], [90, 0]]]).valider().is_err());
        assert!(t(vec![[[2, 2], [2, 2]]]).valider().is_err());
        assert!(TuileMotif { cote: 0.0, divisions: 8, segments: vec![] }.valider().is_err());
    }
}
