//! Export DXF (format d'échange d'AutoCAD, lu par AutoCAD et DWG TrueView) d'un dessin
//! ou d'une feuille : ASCII R12, entités seules, coordonnées en mm du papier (y vers le
//! haut). Traits en LINE, arcs et cercles des annotations en ARC et CIRCLE (`arcs2d`),
//! surfaces pleines (poché, glyphes des textes) en SOLID, un calque par rôle. Le DWG, format fermé, n'est pas écrit.
use noyau::arcs2d::{morceaux, Morceau};
use noyau::trace2d::{Dessin, Forme, Primitive, Role};
use std::fmt::Write;

fn calque(r: Role) -> &'static str {
    match r {
        Role::Poche => "POCHE",
        Role::Hachure => "HACHURE",
        Role::Vu => "VU",
        Role::Coupe => "COUPE",
        _ => "DESSIN",
    }
}

/// Couleur AutoCAD (ACI) la plus proche parmi le noir/blanc et les gris usuels :
/// 7 (noir sur fond blanc), 8 et 9 (gris).
fn couleur(c: [u8; 4]) -> u8 {
    let l = (c[0] as u32 + c[1] as u32 + c[2] as u32) / 3;
    if l < 90 { 7 } else if l < 190 { 8 } else { 9 }
}

/// DXF du dessin `d` (même cadrage que le SVG, sans marge).
pub fn dxf(d: &Dessin) -> String {
    let k = 1000.0 / d.echelle;
    let xy = |p: glam::DVec3| [p.dot(d.droite) * k + 0.0, p.dot(d.haut) * k + 0.0];
    let mut s = String::from("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n");
    let ligne = |s: &mut String, p: &Primitive, a: [f64; 2], b: [f64; 2]| {
        let _ = write!(s, "0\nLINE\n8\n{}\n62\n{}\n10\n{:.4}\n20\n{:.4}\n30\n0.0\n11\n{:.4}\n21\n{:.4}\n31\n0.0\n", calque(p.role), couleur(p.style.couleur), a[0], a[1], b[0], b[1]);
    };
    for p in &d.primitives {
        match &p.forme {
            Forme::Polyligne { points, fermee } if p.role == Role::Annotation => {
                let q: Vec<glam::DVec2> = points.iter().map(|v| glam::DVec2::from_array(xy(*v))).collect();
                for m in morceaux(&q, *fermee) {
                    match m {
                        Morceau::Segment(a, b) => ligne(&mut s, p, a.to_array(), b.to_array()),
                        Morceau::Cercle { centre, rayon } => {
                            let _ = write!(s, "0\nCIRCLE\n8\n{}\n62\n{}\n10\n{:.4}\n20\n{:.4}\n30\n0.0\n40\n{:.4}\n", calque(p.role), couleur(p.style.couleur), centre.x, centre.y, rayon);
                        }
                        Morceau::Arc { centre, rayon, angle_debut, balayage, .. } => {
                            // ARC : toujours dans le sens trigonométrique, angles en degrés
                            let (a, b) = if balayage > 0.0 { (angle_debut, angle_debut + balayage) } else { (angle_debut + balayage, angle_debut) };
                            let deg = |x: f64| x.to_degrees().rem_euclid(360.0);
                            let _ = write!(s, "0\nARC\n8\n{}\n62\n{}\n10\n{:.4}\n20\n{:.4}\n30\n0.0\n40\n{:.4}\n50\n{:.4}\n51\n{:.4}\n", calque(p.role), couleur(p.style.couleur), centre.x, centre.y, rayon, deg(a), deg(b));
                        }
                    }
                }
            }
            Forme::Polyligne { points, fermee } => {
                let q: Vec<_> = points.iter().map(|v| xy(*v)).collect();
                for w in q.windows(2) { ligne(&mut s, p, w[0], w[1]); }
                if *fermee && q.len() > 2 { ligne(&mut s, p, q[q.len() - 1], q[0]); }
            }
            Forme::Segments(segs) => for t in segs { ligne(&mut s, p, xy(t[0]), xy(t[1])); },
            Forme::Surface(contours) => for c in contours {
                let q: Vec<_> = c.iter().map(|v| xy(*v)).collect();
                for i in 0..q.len() { ligne(&mut s, p, q[i], q[(i + 1) % q.len()]); }
            },
            Forme::Triangles(tris) => for t in tris {
                let [a, b, c] = t.map(xy);
                // SOLID : 4ᵉ sommet confondu avec le 3ᵉ pour un triangle
                let _ = write!(s, "0\nSOLID\n8\n{}\n62\n{}\n10\n{:.4}\n20\n{:.4}\n30\n0.0\n11\n{:.4}\n21\n{:.4}\n31\n0.0\n12\n{:.4}\n22\n{:.4}\n32\n0.0\n13\n{:.4}\n23\n{:.4}\n33\n0.0\n", calque(p.role), couleur(p.style.couleur), a[0], a[1], b[0], b[1], c[0], c[1], c[0], c[1]);
            },
        }
    }
    s.push_str("0\nENDSEC\n0\nEOF\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use noyau::trace2d::Style;

    /// N° 13 : une feuille de 420 mm donne des coordonnées en mm, une ligne par segment,
    /// un SOLID par triangle, un fichier fermé par EOF.
    #[test]
    fn traits_et_surfaces_en_mm() {
        let st = Style { couleur: [0, 0, 0, 255], epaisseur: 0.25 };
        let d = Dessin { droite: DVec3::X, haut: DVec3::Y, echelle: 1.0, primitives: vec![
            Primitive { forme: Forme::Polyligne { points: vec![DVec3::ZERO, DVec3::new(0.42, 0.0, 0.0), DVec3::new(0.42, 0.297, 0.0)], fermee: true }, style: st, role: Role::Coupe, element: None },
            Primitive { forme: Forme::Triangles(vec![[DVec3::ZERO, DVec3::new(0.01, 0.0, 0.0), DVec3::new(0.0, 0.01, 0.0)]]), style: st, role: Role::Poche, element: None },
        ] };
        let t = dxf(&d);
        assert_eq!(t.matches("\nLINE\n").count(), 3);
        assert_eq!(t.matches("\nSOLID\n").count(), 1);
        assert!(t.contains("\n420.0000\n") && t.contains("\n297.0000\n"));
        assert!(t.ends_with("EOF\n"));
    }

    /// X4 : un arc et un cercle d'annotation donnent une entité ARC et une CIRCLE en DXF,
    /// des commandes `A` en SVG ; le PDF se produit.
    #[test]
    fn arc_et_cercle_ecrits_en_vrai() {
        use glam::DVec2;
        use noyau::forme2d::points_arc;
        let st = Style { couleur: [0, 0, 0, 255], epaisseur: 0.25 };
        let en3 = |v: Vec<DVec2>| v.into_iter().map(|p| p.extend(0.0)).collect::<Vec<_>>();
        let mut arc = vec![DVec2::new(0.0, -0.1)];
        arc.extend(points_arc(DVec2::ZERO, 0.05, 0.0, std::f64::consts::FRAC_PI_2));
        let cercle = points_arc(DVec2::new(0.2, 0.2), 0.03, 0.0, std::f64::consts::TAU);
        let d = Dessin { droite: DVec3::X, haut: DVec3::Y, echelle: 1.0, primitives: vec![
            Primitive { forme: Forme::Polyligne { points: en3(arc), fermee: false }, style: st, role: Role::Annotation, element: None },
            Primitive { forme: Forme::Polyligne { points: en3(cercle[..cercle.len() - 1].to_vec()), fermee: true }, style: st, role: Role::Annotation, element: None },
            // même cercle en arête de maillage : segments
            Primitive { forme: Forme::Polyligne { points: en3(points_arc(DVec2::ZERO, 0.03, 0.0, std::f64::consts::TAU)), fermee: false }, style: st, role: Role::Coupe, element: None },
        ] };
        let t = dxf(&d);
        assert_eq!(t.matches("\nARC\n").count(), 1, "{t}");
        assert_eq!(t.matches("\nCIRCLE\n").count(), 1);
        // arc : centre (0, 0), rayon 50 mm, de 0 à 90°
        assert!(t.contains("\nARC\n8\nDESSIN\n62\n7\n10\n0.0000\n20\n0.0000\n30\n0.0\n40\n50.0000\n50\n0.0000\n51\n90.0000\n"), "{t}");
        assert!(t.contains("\n40\n30.0000\n"));
        assert_eq!(t.matches("\nLINE\n").count(), 1 + 72, "segment de l'annotation + cercle maillé en 72 segments");
        let svg = d.svg("arcs");
        let chemins: Vec<&str> = svg.lines().filter(|l| l.contains("<path d=\"M") && l.contains('A')).collect();
        assert_eq!(chemins.len(), 2, "{svg}");
        assert!(chemins[0].contains("A50.000 50.000 0 0 0 "), "arc vers le haut : sens négatif en y papier : {}", chemins[0]);
        assert!(crate::pdf(&svg).is_ok_and(|p| p.starts_with(b"%PDF")));
    }
}
