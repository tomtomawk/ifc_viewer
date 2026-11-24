//! Arêtes des éléments du modèle natif : traits de terrain (arêtes jointes par matériau :
//! `aretes_incrementales`).
use modele::{Projet, SorteDalle};

/// Traits d'un terrain en plan : périmètre de sa surface et courbes de niveau à un pas
/// rond (une douzaine au plus). La surface lissée n'a pas d'arête : sans ces traits, la
/// partie vue sous le plan de coupe (faces blanches) ne se distingue pas du papier.
pub(crate) fn aretes_terrain(projet: &Projet, t: &modele::elements::site::Terrain) -> noyau::aretes::Aretes {
    use glam::DVec3;
    let decalage = DVec3::new(0.0, 0.0, -projet.ngf());
    let m = |p: DVec3| p * 0.001;
    let mut vives: Vec<[DVec3; 2]> = (0..t.contour.len()).map(|i| [t.contour[i], t.contour[(i + 1) % t.contour.len()]].map(|p| m(DVec3::from_array(p) + decalage))).collect();
    // dessus du terrain, creusé par les terre-pleins : faces tournées vers le haut du solide
    let dessus = if projet.dalles.iter().any(|d| d.sorte == SorteDalle::TerrePlein) {
        modele::elements::terre_plein::solide_terrain(projet, t, projet.type_terrain(t).epaisseur)
            .map(|v| v.into_iter().filter(|t| (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero().z > 0.05).collect::<Vec<_>>())
    } else {
        t.triangles().map(|v| v.into_iter().map(|t| t.map(|p| p + decalage)).collect())
    };
    if let Ok(tris) = dessus {
        let z = tris.iter().flatten().map(|p| p.z);
        let (bas, haut) = z.fold((f64::MAX, f64::MIN), |(a, b), z| (a.min(z), b.max(z)));
        if haut - bas > 1.0 {
            let etendue = haut - bas;
            let pas = [50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0].into_iter().find(|p| etendue / p <= 12.0).unwrap_or(20_000.0);
            for tri in &tris {
                let zmax = tri.iter().map(|p| p.z).fold(f64::MIN, f64::max);
                let mut z = (tri.iter().map(|p| p.z).fold(f64::MAX, f64::min) / pas).ceil() * pas;
                while z < zmax {
                    let pts: Vec<DVec3> = (0..3).filter_map(|i| {
                        let (a, b) = (tri[i], tri[(i + 1) % 3]);
                        ((a.z - z) * (b.z - z) < 0.0).then(|| a + (b - a) * ((z - a.z) / (b.z - a.z)))
                    }).collect();
                    if let [a, b] = pts[..] {
                        vives.push([m(a), m(b)]);
                    }
                    z += pas;
                }
            }
        }
    }
    noyau::aretes::Aretes { vives, douces: Vec::new() }
}
