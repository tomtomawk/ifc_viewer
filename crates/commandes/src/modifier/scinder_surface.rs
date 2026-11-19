//! Découpe en plan selon une polyligne. Les contours de chaque morceau suivent
//! la règle pair-impair du modèle, y compris lorsque la coupe traverse un trou.
use crate::socle::execution::identite;
use glam::{DVec2, DVec3};
use i_overlay::{
    core::fill_rule::FillRule,
    float::{simplify::SimplifyShape, slice::FloatSlice},
};
use modele::{Projet, elements::site::Terrain, volumes::geometrie::dedans};

type Point = [f64; 2];
type Contours = Vec<Vec<Point>>;

fn decouper(contours: &Contours, trace: &[Point]) -> Result<Vec<Contours>, String> {
    if trace.len() < 2 || trace.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Tracez une ligne de coupe avec au moins deux points.".into());
    }
    let morceaux = contours.slice_by_as::<i64>(&trace, FillRule::EvenOdd);
    if morceaux.len() <= contours.simplify_shape_as::<i64>(FillRule::EvenOdd).len() {
        return Err("La ligne doit traverser la surface de bord à bord.".into());
    }
    for morceau in &morceaux {
        for contour in morceau {
            modele::creation::contour_valide(contour)?;
        }
    }
    Ok(morceaux)
}

// Cote sur une face par interpolation barycentrique.
fn altitude(faces: &[[DVec3; 3]], p: DVec2) -> Result<f64, String> {
    for &[a, b, c] in faces {
        let (u, v, w) = ((b - a).truncate(), (c - a).truncate(), p - a.truncate());
        let det = u.perp_dot(v);
        let (i, j) = (w.perp_dot(v) / det, u.perp_dot(w) / det);
        if i >= -1e-7 && j >= -1e-7 && i + j <= 1.0 + 1e-7 {
            return Ok(a.z + i * (b.z - a.z) + j * (c.z - a.z));
        }
    }
    Err("Altitude de la coupe hors du terrain.".into())
}

// Insère les changements de pente de la triangulation sur les nouveaux bords.
fn bord_terrain(contour: &[Point], faces: &[[DVec3; 3]]) -> Result<Vec<[f64; 3]>, String> {
    let mut bord = Vec::new();
    for (a, b) in contour.iter().zip(contour.iter().cycle().skip(1)) {
        let a = DVec2::from_array(*a);
        let v = DVec2::from_array(*b) - a;
        let mut coupes = vec![0.0];
        for face in faces {
            for k in 0..3 {
                let p = face[k].truncate();
                let w = face[(k + 1) % 3].truncate() - p;
                let det = v.perp_dot(w);
                if det.abs() < 1e-12 {
                    continue;
                }
                let t = (p - a).perp_dot(w) / det;
                let s = (p - a).perp_dot(v) / det;
                if t > 0.0 && t < 1.0 && (-1e-8..=1.0 + 1e-8).contains(&s) {
                    coupes.push(t);
                }
            }
        }
        coupes.sort_by(f64::total_cmp);
        coupes.dedup_by(|a, b| (*a - *b).abs() * v.length() < 1e-5);
        for t in coupes {
            if (1.0 - t) * v.length() < 1e-5 {
                continue;
            }
            let p = a + t * v;
            bord.push([p.x, p.y, altitude(faces, p)?]);
        }
    }
    Ok(bord)
}

fn terrains(source: &Terrain, trace: &[Point]) -> Result<Vec<Terrain>, String> {
    let faces = source.triangles()?;
    let contour = source.contour.iter().map(|p| [p[0], p[1]]).collect();
    decouper(&vec![contour], trace)?
        .into_iter()
        .map(|partie| {
            let [contour] = partie.as_slice() else {
                return Err("Le périmètre d’un terrain doit rester sans trou.".into());
            };
            let bord = bord_terrain(contour, &faces)?;
            let points = source
                .points
                .iter()
                .copied()
                .filter(|p| {
                    dedans([p[0], p[1]], contour)
                        && !bord.iter().any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < 1e-5)
                })
                .collect();
            let terrain = Terrain {
                contour: bord,
                points,
                ..source.clone()
            };
            terrain.triangles()?;
            Ok(terrain)
        })
        .collect()
}

pub(super) fn executer(
    projet: &mut Projet,
    id: &str,
    trace: &[Point],
) -> Result<Vec<String>, String> {
    let mut ids = vec![id.to_owned()];
    if let Some(index) = projet.dalles.iter().position(|d| d.id == id) {
        let source = projet.dalles[index].clone();
        if source.pentes.iter().flatten().any(Option::is_some)
            || source.remblais.iter().flatten().any(|r| *r)
        {
            return Err("La découpe prend en charge les surfaces planes sans talus.".into());
        }
        if projet.attaches_toitures.values().any(|toit| toit == id) {
            return Err("Détachez les murs de cette toiture avant la découpe.".into());
        }
        for (i, contours) in decouper(&source.contours, trace)?.into_iter().enumerate() {
            let mut d = source.clone();
            d.contours = contours;
            d.pentes.clear();
            d.remblais.clear();
            if i == 0 {
                projet.dalles[index] = d;
            } else {
                d.id = identite(projet, "dalle");
                d.origine = None;
                ids.push(d.id.clone());
                projet.dalles.push(d);
            }
        }
    } else if let Some(index) = projet.terrains.iter().position(|t| t.id == id) {
        for (i, mut t) in terrains(&projet.terrains[index], trace)?
            .into_iter()
            .enumerate()
        {
            if i == 0 {
                projet.terrains[index] = t;
            } else {
                t.id = identite(projet, "terrain");
                ids.push(t.id.clone());
                projet.terrains.push(t);
            }
        }
    } else if let Some(index) = projet.limites_propriete.iter().position(|l| l.id == id) {
        let source = projet.limites_propriete[index].clone();
        for (i, mut partie) in decouper(&vec![source.contour.clone()], trace)?
            .into_iter()
            .enumerate()
        {
            if partie.len() != 1 {
                return Err("Une limite de propriété doit rester sans trou.".into());
            }
            let mut l = source.clone();
            l.contour = partie.remove(0);
            if i == 0 {
                projet.limites_propriete[index] = l;
            } else {
                l.id = identite(projet, "limite");
                ids.push(l.id.clone());
                projet.limites_propriete.push(l);
            }
        }
    } else {
        return Err("Choisissez une surface ou un terrain.".into());
    }
    for autre in ids.iter().skip(1) {
        if let Some(c) = projet.contextes.get(id).cloned() {
            projet.contextes.insert(autre.clone(), c);
        }
        if let Some(phase) = projet.demolitions.get(id).cloned() {
            projet.demolitions.insert(autre.clone(), phase);
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Commande, Execution};

    fn carre() -> Vec<Point> {
        vec![[0., 0.], [4000., 0.], [4000., 4000.], [0., 4000.]]
    }

    #[test]
    fn dalle_avec_tremie_coupe_brisee_et_annulation_sur_erreur() {
        let mut p = Projet::depuis_gabarit("Coupe", &modele::gabarit::Gabarit::defaut());
        let trou = vec![[500., 500.], [1500., 500.], [1500., 1500.], [500., 1500.]];
        let id = p
            .ajouter_dalle_contours(
                &[carre(), trou],
                &modele::creation::ParametresDalle {
                    sorte: modele::SorteDalle::Dalle,
                    type_dalle: p.types_murs[0].clone(),
                    pente: None,
                    niveau: None,
                    decalage: 0.,
                    phase: None,
                    sous_projet: None,
                    variante: None,
                },
            )
            .unwrap();
        let couper = |trace| Commande::ScinderSurface {
            id: id.clone(),
            trace,
        };
        assert!(
            p.executer(couper(vec![[2000., 2000.], [3000., 3000.]]))
                .is_err()
        );
        let (q, ids) = p
            .executer(couper(vec![[-100., 1000.], [2000., 1000.], [4100., 2500.]]))
            .unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(q.dalles.len(), 2);
        let aire: f64 = q
            .dalles
            .iter()
            .map(|d| modele::volumes::geometrie::aire_pair_impair(&d.contours))
            .sum();
        assert!((aire - 15_000_000.).abs() < 1.);
        assert_eq!(p.dalles.len(), 1);
    }

    #[test]
    fn terrain_coupe_au_sommet_conserve_les_cotes_communes() {
        let t = Terrain {
            id: "terrain".into(),
            nom: "Pente".into(),
            type_terrain: None,
            contour: carre().iter().map(|p| [p[0], p[1], 100.]).collect(),
            points: vec![[2000., 2000., 1700.], [800., 2200., 400.]],
        };
        let parties = terrains(&t, &[[-100., 2000.], [4200., 2000.]]).unwrap();
        assert_eq!(parties.len(), 2);
        for partie in &parties {
            assert!(partie.contour.iter().any(|p| (p[0] - 2000.).abs() < 0.01
                && (p[1] - 2000.).abs() < 0.01
                && (p[2] - 1700.).abs() < 0.01));
            assert!(partie.triangles().is_ok());
        }
        assert_eq!(parties.iter().map(|p| p.points.len()).sum::<usize>(), 1);
    }
}
