//! Motifs de hachures : familles de lignes parallèles et leur découpe sur une région.

/// Famille de lignes parallèles d'un motif de remplissage (`FillGrid`), en mm : angle
/// (radians), origine, décalage entre deux lignes (`m_deltas[1]`), glissement le long
/// des lignes d'une ligne à la suivante (`m_deltas[0]`), tirets (longueurs alternées
/// trait / blanc, `m_segs` ; vide : trait continu).
#[derive(Clone)]
pub struct GrilleMotif {
    pub angle: f64,
    pub origine: [f64; 2],
    pub glissement: f64,
    pub decalage: f64,
    pub tirets: Vec<f64>,
}

/// Motif de remplissage (`FillPatternElem` → `FillPattern`) ; sans grille : uni.
#[derive(Clone)]
pub struct Motif {
    pub nom: String,
    /// Motif de modèle (`m_fpTarget` = 1, mesures en mm du modèle), sinon de dessin (mm
    /// du papier, à multiplier par l'échelle de la vue).
    pub modele: bool,
    pub grilles: Vec<GrilleMotif>,
}

/// Intervalles de t où p + t·d est dans le triangle (a, b, c) du plan (u, v).
fn dans_triangle(p: [f64; 2], d: [f64; 2], t: [[f64; 2]; 3]) -> Option<(f64, f64)> {
    let aire = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[1][1] - t[0][1]) * (t[2][0] - t[0][0]);
    if aire.abs() < 1e-12 {
        return None;
    }
    let (mut t0, mut t1) = (f64::MIN, f64::MAX);
    for i in 0..3 {
        let (a, b) = (t[i], t[(i + 1) % 3]);
        // demi-plan intérieur : signe(aire) × ((b − a) × (x − a)) ≥ 0
        let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
        let f0 = aire.signum() * (ex * (p[1] - a[1]) - ey * (p[0] - a[0]));
        let f1 = aire.signum() * (ex * d[1] - ey * d[0]);
        if f1.abs() < 1e-15 {
            if f0 < 0.0 {
                return None;
            }
        } else {
            let t = -f0 / f1;
            if f1 > 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 < t1).then_some((t0, t1))
}

/// Traits d'un motif découpés sur des triangles du plan du motif : triangles en (x, y),
/// mesures du motif multipliées par `f` ; segments en (x, y), au plus `max`.
pub fn hachures_plan(tris: &[[[f64; 2]; 3]], m: &Motif, f: f64, max: usize) -> Vec<[[f64; 2]; 2]> {
    let mut out = Vec::new();
    for g in &m.grilles {
        let (dec, gl) = (g.decalage * f, g.glissement * f);
        if dec.abs() < 1e-6 {
            continue;
        }
        let (d, nr) = ([g.angle.cos(), g.angle.sin()], [-g.angle.sin(), g.angle.cos()]);
        let o = [g.origine[0] * f, g.origine[1] * f];
        let s: Vec<f64> = tris.iter().flatten().map(|q| (q[0] - o[0]) * nr[0] + (q[1] - o[1]) * nr[1]).collect();
        let (smin, smax) = s.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        let (k0, k1) = ((smin / dec).floor() as i64, (smax / dec).ceil() as i64);
        if k1 < k0 || (k1 - k0) as usize > max {
            continue;
        }
        let tirets: Vec<f64> = g.tirets.iter().map(|x| x * f).collect();
        let periode: f64 = tirets.iter().sum();
        for k in k0..=k1 {
            let p = [o[0] + k as f64 * (dec * nr[0] + gl * d[0]), o[1] + k as f64 * (dec * nr[1] + gl * d[1])];
            let mut iv: Vec<(f64, f64)> = tris.iter().filter_map(|t| dans_triangle(p, d, *t)).collect();
            iv.sort_by(|a, b| a.0.total_cmp(&b.0));
            // réunion des intervalles des triangles voisins
            let mut union: Vec<(f64, f64)> = Vec::new();
            for (a, b) in iv {
                match union.last_mut() {
                    Some(l) if a <= l.1 + 1e-6 => l.1 = l.1.max(b),
                    _ => union.push((a, b)),
                }
            }
            for (a, b) in union {
                let mut morceaux = Vec::new();
                if periode > 1e-6 {
                    // tirets : phase 0 au point p de la ligne
                    let mut m0 = (a / periode).floor() * periode;
                    while m0 < b && morceaux.len() < max {
                        let mut x = m0;
                        for (i, l) in tirets.iter().enumerate() {
                            if i % 2 == 0 && x + l > a && x < b {
                                morceaux.push((x.max(a), (x + l).min(b)));
                            }
                            x += l;
                        }
                        m0 += periode;
                    }
                } else {
                    morceaux.push((a, b));
                }
                for (a, b) in morceaux {
                    let pt = |t: f64| [p[0] + t * d[0], p[1] + t * d[1]];
                    out.push([pt(a), pt(b)]);
                    if out.len() >= max {
                        return out;
                    }
                }
            }
        }
    }
    out
}
