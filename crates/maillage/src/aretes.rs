//! Arêtes des éléments du modèle natif : jointes par matériau, silhouette, traits de terrain.
use formats::ifc::geom::ProductGeom;
use glam::DVec3;
use modele::{Projet, SorteDalle};
use noyau::Id;
use rayon::prelude::*;

/// Arêtes des éléments calculées par matériau : là où deux éléments d'un même matériau se
/// joignent (murs raccordés, dalle sous un mur), pas d'arête sur les faces qui se
/// prolongent ; les parties sans matériau restent à part. Les éléments de `exclus`
/// (terrains : leurs arêtes sont calculées autrement) ne sont pas traités.
pub(crate) fn aretes_jointes(geoms: &[ProductGeom], materiaux: &[Vec<Option<String>>], exclus: &std::collections::HashSet<Id>) -> std::collections::HashMap<Id, noyau::aretes::Aretes> {
    use std::collections::HashMap;
    let mut groupes: HashMap<String, Vec<([DVec3; 3], usize)>> = HashMap::new();
    for (k, (g, noms)) in geoms.iter().zip(materiaux).enumerate().filter(|(_, (g, _))| !exclus.contains(&g.id)) {
        // parties du style Réaliste seulement (géométrie de rendu) : sans arête
        for (i, p) in g.parts.iter().enumerate().filter(|(i, _)| !formats::ifc::assemblage::rendu_seul(g, *i)) {
            let cle = noms.get(i).cloned().flatten().unwrap_or_else(|| format!("\u{0}{k}:{i}"));
            groupes.entry(cle).or_default().extend(p.tris.iter().map(|t| (*t, k)));
        }
    }
    let mut par_objet: HashMap<Id, noyau::aretes::Aretes> = geoms.iter().map(|g| (g.id, noyau::aretes::Aretes::default())).collect();
    let resultats: Vec<_> = groupes.into_par_iter().map(|(_, tris)| noyau::aretes::aretes_jointes(&tris)).collect();
    for (vives, douces) in resultats {
        for (s, k) in vives {
            if let Some(a) = par_objet.get_mut(&geoms[k].id) { a.vives.push(s); }
        }
        for (d, k) in douces {
            if let Some(a) = par_objet.get_mut(&geoms[k].id) { a.douces.push(d); }
        }
    }
    // groupes traités dans un ordre quelconque (table de hachage, parallélisme)
    par_objet.values_mut().for_each(noyau::aretes::Aretes::ordonner);
    par_objet
}

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
