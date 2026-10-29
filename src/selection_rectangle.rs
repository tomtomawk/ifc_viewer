//! Sélection à l'écran des triangles et traits visibles, après découpe de la vue.
use crate::scene::Scene;
use eframe::egui::{Pos2, Rect};
use glam::{Mat4, Vec3, Vec4};
use std::collections::HashMap;

fn clip(poly: &[Vec4], distance: impl Fn(Vec4) -> f32) -> Vec<Vec4> {
    if poly.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut a = *poly.last().unwrap();
    let mut da = distance(a);
    for &b in poly {
        let db = distance(b);
        if (da >= 0.0) != (db >= 0.0) {
            out.push(a + (b - a) * (da / (da - db)));
        }
        if db >= 0.0 {
            out.push(b);
        }
        a = b;
        da = db;
    }
    out
}

/// Retourne pour chaque objet (géométrie visible, entièrement incluse, touchée).
pub fn objets(
    scene: &Scene,
    visible: &[bool],
    vp: Mat4,
    vue: Rect,
    selection: Rect,
    coupe: Option<Vec4>,
    boite: Option<(Vec3, Vec3)>,
    cadrage: &[Vec4],
) -> Vec<(bool, bool, bool)> {
    let mut plans = cadrage.to_vec();
    if let Some(p) = coupe {
        plans.push(p);
    }
    if let Some((a, b)) = boite {
        plans.extend([
            Vec4::new(1., 0., 0., -a.x),
            Vec4::new(-1., 0., 0., b.x),
            Vec4::new(0., 1., 0., -a.y),
            Vec4::new(0., -1., 0., b.y),
            Vec4::new(0., 0., 1., -a.z),
            Vec4::new(0., 0., -1., b.z),
        ]);
    }
    scene
        .objects
        .iter()
        .enumerate()
        .map(|(i, o)| {
            if !visible.get(i).copied().unwrap_or(false) {
                return (false, false, false);
            }
            let mut existe = false;
            let mut entier = true;
            let mut touche = false;
            for (r, taille) in [(o.opaque, 3), (o.transparent, 3), (o.lines, 2)] {
                let Some(vs) = scene
                    .vertices
                    .get(r.start as usize..(r.start + r.count) as usize)
                else {
                    continue;
                };
                for primitive in vs.chunks_exact(taille) {
                    let mut poly: Vec<Vec4> = primitive
                        .iter()
                        .map(|v| Vec3::from(v.pos).extend(1.0))
                        .collect();
                    if primitive[0].normal[3] <= 0 {
                        for p in &plans {
                            poly = clip(&poly, |v| p.dot(v));
                        }
                    }
                    poly = poly.into_iter().map(|v| vp * v).collect();
                    poly = clip(&poly, |v| v.w - 1e-6);
                    poly = clip(&poly, |v| v.z + v.w);
                    poly = clip(&poly, |v| v.w - v.z);
                    if poly.is_empty() {
                        continue;
                    }
                    let pts: Vec<Pos2> = poly
                        .iter()
                        .map(|v| {
                            Pos2::new(
                                vue.left() + (v.x / v.w + 1.0) * 0.5 * vue.width(),
                                vue.top() + (1.0 - v.y / v.w) * 0.5 * vue.height(),
                            )
                        })
                        .collect();
                    existe = true;
                    entier &= pts.iter().all(|p| selection.contains(*p));
                    if !touche {
                        let mut q: Vec<Vec4> =
                            pts.iter().map(|p| Vec4::new(p.x, p.y, 0., 1.)).collect();
                        for p in [
                            Vec4::new(1., 0., 0., -selection.left()),
                            Vec4::new(-1., 0., 0., selection.right()),
                            Vec4::new(0., 1., 0., -selection.top()),
                            Vec4::new(0., -1., 0., selection.bottom()),
                        ] {
                            q = clip(&q, |v| p.dot(v));
                        }
                        touche = !q.is_empty();
                    }
                }
            }
            (existe, entier, touche)
        })
        .collect()
}

pub fn segment(rect: Rect, a: Pos2, b: Pos2, croisement: bool) -> bool {
    if !croisement {
        return rect.contains(a) && rect.contains(b);
    }
    let mut poly = vec![Vec4::new(a.x, a.y, 0., 1.), Vec4::new(b.x, b.y, 0., 1.)];
    for p in [
        Vec4::new(1., 0., 0., -rect.left()),
        Vec4::new(-1., 0., 0., rect.right()),
        Vec4::new(0., 1., 0., -rect.top()),
        Vec4::new(0., -1., 0., rect.bottom()),
    ] {
        poly = clip(&poly, |v| p.dot(v));
    }
    !poly.is_empty()
}

/// Une fenêtre inclut toutes les couches visibles d'un assemblage ; un croisement en touche une.
pub fn groupes<T: Copy + Eq + std::hash::Hash>(
    objets: impl IntoIterator<Item = (T, (bool, bool, bool))>,
    croisement: bool,
) -> Vec<T> {
    let mut groupes: HashMap<T, (bool, bool)> = HashMap::new();
    for (id, (existe, entier, touche)) in objets {
        if existe {
            let g = groupes.entry(id).or_insert((true, false));
            g.0 &= entier;
            g.1 |= touche;
        }
    }
    groupes
        .into_iter()
        .filter_map(|(id, (entier, touche))| {
            (if croisement { touche } else { entier }).then_some(id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoupe_et_regroupement() {
        let triangle = [
            Vec4::new(-2., 0., 0., 1.),
            Vec4::new(2., 0., 0., 1.),
            Vec4::new(0., 2., 0., 1.),
        ];
        let q = clip(&triangle, |p| p.x + 0.5);
        let q = clip(&q, |p| 0.5 - p.x);
        let q = clip(&q, |p| p.y - 0.5);
        let q = clip(&q, |p| 1.5 - p.y);
        assert!(!q.is_empty());
        let objets = [
            (1, (true, true, true)),
            (1, (true, false, true)),
            (2, (true, true, true)),
            (3, (false, false, false)),
        ];
        assert_eq!(groupes(objets, false), vec![2]);
        let mut ids = groupes(objets, true);
        ids.sort();
        assert_eq!(ids, vec![1, 2]);
    }
    #[test]
    fn fenetre_croisement_visibilite_et_coupe() {
        use crate::modele::{Barre, GenreBarre, Niveau, Projet};
        let mut p = Projet::nouveau("test");
        p.niveaux.push(Niveau {
            id: "n".into(),
            nom: "Niveau".into(),
            altitude: 0.0,
            origine: None,
        });
        for (id, x) in [("a", -2000.0), ("b", 2000.0)] {
            p.barres.push(Barre {
                id: id.into(),
                nom: id.into(),
                genre: GenreBarre::Poteau,
                niveau: "n".into(),
                debut: [x, 0.0, 0.0],
                fin: [x, 0.0, 2000.0],
                largeur: 1000.0,
                hauteur: 1000.0,
                rotation: 0.0,
            });
        }
        let s = crate::scene::depuis_natif(&p, std::path::Path::new("test.json")).unwrap();
        let camera = Mat4::from_cols(
            Vec4::new(0.2, 0., 0., 0.),
            Vec4::new(0., 0.2, 0., 0.),
            Vec4::new(0., 0., 0.1, 0.),
            Vec4::W,
        );
        let vue = Rect::from_min_max(Pos2::ZERO, Pos2::new(100., 100.));
        let fenetre = Rect::from_min_max(Pos2::new(20., 40.), Pos2::new(40., 60.));
        let result = objets(&s, &[true, true], camera, vue, fenetre, None, None, &[]);
        assert_eq!(result[0], (true, true, true));
        assert_eq!(result[1], (true, false, false));
        let etroite = Rect::from_min_max(Pos2::new(34., 45.), Pos2::new(36., 55.));
        let result = objets(&s, &[true, true], camera, vue, etroite, None, None, &[]);
        assert_eq!(result[0], (true, false, true));
        let result = objets(&s, &[false, true], camera, vue, fenetre, None, None, &[]);
        assert!(!result[0].0);
        let result = objets(
            &s,
            &[true, true],
            camera,
            vue,
            fenetre,
            Some(Vec4::new(1., 0., 0., 0.)),
            None,
            &[],
        );
        assert!(!result[0].0);
    }
}
