//! Découpe des primitives d'un plan par un contour simple, y compris concave.
use noyau::trace2d::{Forme, Primitive};
use glam::{DVec2, DVec3};

fn cote(a: DVec2, b: DVec2, p: DVec3) -> f64 { (b - a).perp_dot(p.truncate() - a) }

fn coupe_polygone(points: &[DVec3], a: DVec2, b: DVec2) -> Vec<DVec3> {
    if points.is_empty() { return Vec::new(); }
    let mut sortie = Vec::new();
    let mut precedent = *points.last().unwrap();
    let mut cp = cote(a, b, precedent);
    for &p in points {
        let c = cote(a, b, p);
        if (cp >= -1e-10) != (c >= -1e-10) {
            let t = cp / (cp - c);
            sortie.push(precedent + (p - precedent) * t);
        }
        if c >= -1e-10 { sortie.push(p); }
        precedent = p;
        cp = c;
    }
    sortie
}

fn segment_dans_triangle(segment: [DVec3; 2], triangle: &[DVec2; 3]) -> Option<[DVec3; 2]> {
    let [a, b] = segment;
    let (mut debut, mut fin): (f64, f64) = (0.0, 1.0);
    for i in 0..3 {
        let x = cote(triangle[i], triangle[(i + 1) % 3], a);
        let y = cote(triangle[i], triangle[(i + 1) % 3], b);
        if x < -1e-10 && y < -1e-10 { return None; }
        if (x < -1e-10) != (y < -1e-10) {
            let t = x / (x - y);
            if x < -1e-10 { debut = debut.max(t); } else { fin = fin.min(t); }
        }
    }
    (fin - debut > 1e-10).then(|| [a + (b-a)*debut, a + (b-a)*fin])
}

pub fn decouper(primitives: Vec<Primitive>, contour: &[[f64; 2]]) -> Result<Vec<Primitive>, String> {
    if contour.is_empty() { return Ok(primitives); }
    let contour = crate::creation::contour_valide(contour)?;
    let coords: Vec<f64> = contour.iter().flat_map(|p| [p[0] * 0.001, p[1] * 0.001]).collect();
    let indices = earcutr::earcut(&coords, &[], 2).map_err(|_| "Triangulation du cadrage impossible.")?;
    if indices.is_empty() || indices.len() % 3 != 0 { return Err("Triangulation du cadrage impossible.".into()); }
    let triangles: Vec<[DVec2; 3]> = indices.chunks_exact(3).map(|t| {
        let mut p = t.iter().map(|&i| DVec2::new(coords[2*i], coords[2*i+1])).collect::<Vec<_>>();
        if (p[1]-p[0]).perp_dot(p[2]-p[0]) < 0.0 { p.swap(1, 2); }
        [p[0], p[1], p[2]]
    }).collect();
    let mut out = Vec::new();
    for mut primitive in primitives {
        let (segments, faces) = match primitive.forme {
            Forme::Segments(s) => (s, Vec::new()),
            Forme::Polyligne { points, fermee } => {
                let mut s: Vec<_> = points.windows(2).map(|p| [p[0], p[1]]).collect();
                if fermee && points.len() > 2 { s.push([*points.last().unwrap(), points[0]]); }
                (s, Vec::new())
            }
            Forme::Triangles(t) => (Vec::new(), t),
            Forme::Surface(c) => (Vec::new(), noyau::trace2d::trianguler(&c, DVec3::Z)),
        };
        if !segments.is_empty() {
            let morceaux: Vec<_> = segments.into_iter().flat_map(|s| triangles.iter().filter_map(move |t| segment_dans_triangle(s, t))).collect();
            if !morceaux.is_empty() { primitive.forme = Forme::Segments(morceaux); out.push(primitive); }
        } else if !faces.is_empty() {
            let mut morceaux = Vec::new();
            for face in faces {
                for triangle in &triangles {
                    let mut p = face.to_vec();
                    for i in 0..3 { p = coupe_polygone(&p, triangle[i], triangle[(i+1)%3]); }
                    for i in 1..p.len().saturating_sub(1) { morceaux.push([p[0],p[i],p[i+1]]); }
                }
            }
            if !morceaux.is_empty() { primitive.forme = Forme::Triangles(morceaux); out.push(primitive); }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use noyau::trace2d::{Role, Style};
    #[test]
    fn contour_concave_decoupe_traits_et_faces() {
        let contour = [[0.,0.],[4000.,0.],[4000.,1000.],[1000.,1000.],[1000.,4000.],[0.,4000.]];
        let style = Style { couleur: [0,0,0,255], epaisseur: 0.1 };
        let ligne = Primitive { forme: Forme::Segments(vec![[DVec3::new(-1.,2.,0.),DVec3::new(5.,2.,0.)]]), style: style.clone(), role: Role::Vu, element: None };
        let face = Primitive { forme: Forme::Triangles(vec![[DVec3::new(-1.,-1.,0.),DVec3::new(10.,-1.,0.),DVec3::new(-1.,10.,0.)]]), style, role: Role::Poche, element: None };
        let decoupe = decouper(vec![ligne, face], &contour).unwrap();
        let Forme::Segments(s) = &decoupe[0].forme else { panic!("segments attendus") };
        let longueur: f64 = s.iter().map(|x| x[0].distance(x[1])).sum();
        assert!((longueur - 1.0).abs() < 1e-8, "{longueur}");
        let Forme::Triangles(t) = &decoupe[1].forme else { panic!("triangles attendus") };
        let aire: f64 = t.iter().map(|t| (t[1]-t[0]).cross(t[2]-t[0]).length()*0.5).sum();
        assert!((aire - 7.0).abs() < 1e-8, "{aire}");
    }
}
