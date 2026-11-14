//! Primitives planes indépendantes du modèle.

fn aire(c: &[[f64; 2]]) -> f64 {
    let origine = c[0];
    (0..c.len()).map(|i| {
        let a = [c[i][0] - origine[0], c[i][1] - origine[1]];
        let b = [c[(i + 1) % c.len()][0] - origine[0], c[(i + 1) % c.len()][1] - origine[1]];
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f64>() / 2.0
}

fn dedans(p: [f64; 2], c: &[[f64; 2]]) -> bool {
    let mut d = false;
    for i in 0..c.len() {
        let (a, b) = (c[i], c[(i + 1) % c.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) { d = !d; }
    }
    d
}

/// Triangule des contours plans par parité ; les îlots et trous sont permis.
/// `aire_min` est exprimée dans les unités du plan au carré. Les contours
/// dégénérés ou non finis sont ignorés. Les contours ne doivent pas se croiser.
pub(crate) fn trianguler_contours(mut contours: Vec<Vec<[f64; 2]>>, aire_min: f64) -> Vec<[[f64; 2]; 3]> {
    for c in &mut contours { if c.len() > 1 && c.first() == c.last() { c.pop(); } }
    contours.retain(|c| c.len() >= 3 && c.iter().flatten().all(|v| v.is_finite()) && aire(c).abs() > aire_min);
    let n = contours.len();
    let aires: Vec<_> = contours.iter().map(|c| aire(c).abs()).collect();
    let profondeur: Vec<usize> = (0..n).map(|i| (0..n).filter(|&j| j != i && aires[j] > aires[i] && dedans(contours[i][0], &contours[j])).count()).collect();
    let mut out = Vec::new();
    for i in (0..n).filter(|&i| profondeur[i] % 2 == 0) {
        let trous = (0..n).filter(|&j| profondeur[j] == profondeur[i] + 1 && dedans(contours[j][0], &contours[i]));
        let mut coords = Vec::new();
        let mut indices_trous = Vec::new();
        for (k, c) in std::iter::once(i).chain(trous).enumerate() {
            if k > 0 { indices_trous.push(coords.len() / 2); }
            coords.extend(contours[c].iter().flatten().copied());
        }
        if let Ok(idx) = earcutr::earcut(&coords, &indices_trous, 2) {
            let pt = |k: usize| [coords[2*k], coords[2*k+1]];
            out.extend(idx.chunks_exact(3).map(|t| [pt(t[0]), pt(t[1]), pt(t[2])]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn triangule_concavite_trou_et_petites_dimensions() {
        let exterieur = vec![[0.,0.],[4.,0.],[4.,4.],[2.,4.],[2.,2.],[0.,2.]];
        let trou = vec![[2.5,0.5],[3.5,0.5],[3.5,1.5],[2.5,1.5]];
        for echelle in [1.0, 1e-6] {
            let contours = [exterieur.clone(), trou.clone()].map(|c| c.into_iter().map(|p| [p[0]*echelle,p[1]*echelle]).collect()).to_vec();
            let triangles = trianguler_contours(contours, 0.0);
            let surface: f64 = triangles.iter().map(|t| aire(t).abs()).sum();
            assert!((surface/(echelle*echelle)-11.0).abs() < 1e-9);
        }
    }
}
