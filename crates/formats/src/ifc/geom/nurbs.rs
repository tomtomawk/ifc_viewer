//! Faces B-spline des `IfcAdvancedBrep` : `IfcBSplineSurfaceWithKnots` et
//! `IfcRationalBSplineSurfaceWithKnots` échantillonnées en grille sur leur domaine.
//!
//! Une face n'est rendue par sa surface que si toutes ses bornes suivent le bord du
//! domaine (face non découpée, cas des faces exportées par les modeleurs pour une
//! surface entière) ; une face découpée garde la facettisation par ses arêtes.
use super::{Geom, Tri};
use glam::{DMat4, DVec3};
use ifc_base::step::Entity;

/// Nombre maximal d'échantillons par direction.
const ECHANTILLONS_MAX: usize = 64;
/// Nombre maximal de points de contrôle d'une surface.
const POINTS_CONTROLE_MAX: usize = 1_000_000;
/// Nombre maximal de points de bord comparés à la grille.
const POINTS_BORD_MAX: usize = 4096;

/// Surface B-spline lue : points de contrôle `pts[i][j]` (i selon u), poids, nœuds.
struct Surface {
    pts: Vec<Vec<DVec3>>,
    poids: Vec<Vec<f64>>,
    du: usize,
    dv: usize,
    nu: Vec<f64>,
    nv: Vec<f64>,
}

/// Fonctions de base non nulles en `t` : (premier indice, valeurs).
fn bases(noeuds: &[f64], degre: usize, n: usize, t: f64) -> (usize, Vec<f64>) {
    let mut k = degre;
    while k < n - 1 && noeuds[k + 1] <= t {
        k += 1;
    }
    let mut b = vec![0.0; degre + 1];
    b[0] = 1.0;
    let mut gauche = vec![0.0; degre + 1];
    let mut droite = vec![0.0; degre + 1];
    for j in 1..=degre {
        gauche[j] = t - noeuds[k + 1 - j];
        droite[j] = noeuds[k + j] - t;
        let mut reste = 0.0;
        for r in 0..j {
            let den = droite[r + 1] + gauche[j - r];
            let tmp = if den.abs() < 1e-14 { 0.0 } else { b[r] / den };
            b[r] = reste + droite[r + 1] * tmp;
            reste = gauche[j - r] * tmp;
        }
        b[j] = reste;
    }
    (k - degre, b)
}

impl Surface {
    fn domaine(&self) -> ((f64, f64), (f64, f64)) {
        let (n, m) = (self.pts.len(), self.pts[0].len());
        ((self.nu[self.du], self.nu[n]), (self.nv[self.dv], self.nv[m]))
    }

    fn point(&self, u: f64, v: f64) -> DVec3 {
        let (n, m) = (self.pts.len(), self.pts[0].len());
        let (iu, bu) = bases(&self.nu, self.du, n, u);
        let (iv, bv) = bases(&self.nv, self.dv, m, v);
        let (mut s, mut w) = (DVec3::ZERO, 0.0);
        for (a, ba) in bu.iter().enumerate() {
            for (b, bb) in bv.iter().enumerate() {
                let (i, j) = ((iu + a).min(n - 1), (iv + b).min(m - 1));
                let c = ba * bb * self.poids[i][j];
                s += self.pts[i][j] * c;
                w += c;
            }
        }
        if w.abs() > 1e-14 {
            s / w
        } else {
            s
        }
    }
}

impl Geom<'_> {
    fn surface_bspline(&self, e: &Entity) -> Option<Surface> {
        if !(e.is("IFCBSPLINESURFACEWITHKNOTS") || e.is("IFCRATIONALBSPLINESURFACEWITHKNOTS")) {
            return None;
        }
        let lignes = e.arg(2).as_list();
        let m = lignes.first()?.as_list().len();
        if lignes.len().checked_mul(m)? > POINTS_CONTROLE_MAX || m < 2 || lignes.len() < 2 {
            return None;
        }
        let mut pts = Vec::with_capacity(lignes.len());
        for l in lignes {
            let r: Vec<DVec3> = l.refs().filter_map(|p| self.point(Some(p))).collect();
            if r.len() != m {
                return None;
            }
            pts.push(r);
        }
        let n = pts.len();
        let liste = |i: usize| -> Vec<f64> { e.arg(i).as_list().iter().filter_map(|v| v.as_f64()).collect() };
        let entiers = |i: usize| -> Vec<i64> { e.arg(i).as_list().iter().filter_map(|v| v.as_i64()).collect() };
        let (du, nu) = super::super::bornes::noeuds(&liste(9), &entiers(7), e.arg(0).as_i64()?, n)?;
        let (dv, nv) = super::super::bornes::noeuds(&liste(10), &entiers(8), e.arg(1).as_i64()?, m)?;
        let poids = if e.is("IFCRATIONALBSPLINESURFACEWITHKNOTS") {
            let p: Vec<Vec<f64>> = e.arg(12).as_list().iter().map(|l| l.as_list().iter().filter_map(|v| v.as_f64()).collect()).collect();
            if p.len() != n || p.iter().any(|l| l.len() != m || l.iter().any(|w| !(w.is_finite() && *w > 0.0))) {
                return None;
            }
            p
        } else {
            vec![vec![1.0; m]; n]
        };
        Some(Surface { pts, poids, du, dv, nu, nv })
    }

    /// Triangles d'une `IfcAdvancedFace` à surface B-spline non découpée, dans le repère
    /// `m`, ou None (la face est alors facettisée par ses arêtes).
    pub(super) fn face_bspline(&self, face: &Entity, m: DMat4) -> Option<Vec<Tri>> {
        if !face.is("IFCADVANCEDFACE") {
            return None;
        }
        let s = self.surface_bspline(self.get(face.arg(1).as_ref()?)?)?;
        let ((u0, u1), (v0, v1)) = s.domaine();
        if !(u1 > u0 && v1 > v0) {
            return None;
        }
        let (n, mm) = (s.pts.len(), s.pts[0].len());
        let cu = (n * 4 * s.du.max(1)).clamp(4, ECHANTILLONS_MAX);
        let cv = (mm * 4 * s.dv.max(1)).clamp(4, ECHANTILLONS_MAX);
        let grille: Vec<Vec<DVec3>> = (0..=cu)
            .map(|i| {
                let u = u0 + (u1 - u0) * i as f64 / cu as f64;
                (0..=cv).map(|j| s.point(u, v0 + (v1 - v0) * j as f64 / cv as f64)).collect()
            })
            .collect();
        // Bornes : chaque point doit être proche d'un échantillon du bord du domaine.
        let mut bord: Vec<DVec3> = Vec::new();
        for b in face.arg(0).refs() {
            let bound = self.get(b)?;
            bord.extend(self.loop_points(bound.arg(0).as_ref()));
            if bord.len() > POINTS_BORD_MAX {
                return None;
            }
        }
        if bord.len() < 3 {
            return None;
        }
        let mut pas: f64 = 0.0;
        for i in 0..cu {
            for j in 0..cv {
                pas = pas.max(grille[i][j].distance(grille[i + 1][j + 1])).max(grille[i + 1][j].distance(grille[i][j + 1]));
            }
        }
        let tol = pas * 1.01 + 1e-9;
        for q in &bord {
            let (mut best, mut bi, mut bj) = (f64::INFINITY, 0, 0);
            for (i, l) in grille.iter().enumerate() {
                for (j, p) in l.iter().enumerate() {
                    let d = p.distance_squared(*q);
                    if d < best {
                        (best, bi, bj) = (d, i, j);
                    }
                }
            }
            let sur_bord = bi <= 1 || bi + 1 >= cu || bj <= 1 || bj + 1 >= cv;
            if best.sqrt() > tol || !sur_bord {
                return None;
            }
        }
        let mut out = Vec::with_capacity(cu * cv * 2);
        let inverse = face.arg(2).as_bool() == Some(false);
        let g = |i: usize, j: usize| m.transform_point3(grille[i][j]);
        for i in 0..cu {
            for j in 0..cv {
                let (a, b, c, d) = (g(i, j), g(i + 1, j), g(i + 1, j + 1), g(i, j + 1));
                for t in [[a, b, c], [a, c, d]] {
                    if (t[1] - t[0]).cross(t[2] - t[0]).length_squared() < 1e-24 {
                        continue; // triangle dégénéré (pôle, couture)
                    }
                    out.push(if inverse { [t[0], t[2], t[1]] } else { t });
                }
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::essais::geom_de;
    use glam::DMat4;

    /// Quart de cylindre rationnel (rayon 1, longueur 2), face bornée par ses 4 coins.
    #[test]
    fn surface_rationnelle_echantillonnee_sur_le_cylindre() {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let donnees = format!(
            "#1=IFCCARTESIANPOINT((1.,0.,0.));#2=IFCCARTESIANPOINT((1.,0.,2.));\
             #3=IFCCARTESIANPOINT((1.,1.,0.));#4=IFCCARTESIANPOINT((1.,1.,2.));\
             #5=IFCCARTESIANPOINT((0.,1.,0.));#6=IFCCARTESIANPOINT((0.,1.,2.));\
             #10=IFCRATIONALBSPLINESURFACEWITHKNOTS(2,1,((#1,#2),(#3,#4),(#5,#6)),.UNSPECIFIED.,.F.,.F.,.F.,(3,3),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.,((1.,1.),({w},{w}),(1.,1.)));\
             #20=IFCVERTEXPOINT(#1);#21=IFCVERTEXPOINT(#5);#22=IFCVERTEXPOINT(#6);#23=IFCVERTEXPOINT(#2);\
             #30=IFCEDGECURVE(#20,#21,$,.T.);#31=IFCEDGECURVE(#21,#22,$,.T.);#32=IFCEDGECURVE(#22,#23,$,.T.);#33=IFCEDGECURVE(#23,#20,$,.T.);\
             #40=IFCORIENTEDEDGE(*,*,#30,.T.);#41=IFCORIENTEDEDGE(*,*,#31,.T.);#42=IFCORIENTEDEDGE(*,*,#32,.T.);#43=IFCORIENTEDEDGE(*,*,#33,.T.);\
             #50=IFCEDGELOOP((#40,#41,#42,#43));#51=IFCFACEOUTERBOUND(#50,.T.);\
             #60=IFCADVANCEDFACE((#51),#10,.T.);#70=IFCOPENSHELL((#60));"
        );
        let (modele, stats) = geom_de(&donnees);
        let g = super::super::essais::geom(&modele, &stats);
        let tris = g.solid(70, DMat4::IDENTITY, 0).unwrap();
        assert!(tris.len() > 16, "surface échantillonnée, pas 2 triangles : {}", tris.len());
        for t in &tris {
            for p in t {
                let r = (p.x * p.x + p.y * p.y).sqrt();
                assert!((r - 1.0).abs() < 1e-9, "point hors du cylindre : {p:?}");
            }
        }
        let aire: f64 = tris.iter().map(|t| (t[1] - t[0]).cross(t[2] - t[0]).length() / 2.0).sum();
        let attendu = std::f64::consts::PI; // quart de 2π × 1 × 2
        assert!((aire - attendu).abs() / attendu < 0.01, "aire {aire}, attendu {attendu}");
    }

    /// Face découpée (bornes à l'intérieur du domaine) : facettisation par les arêtes.
    #[test]
    fn face_decoupee_garde_ses_aretes() {
        let donnees = "#1=IFCCARTESIANPOINT((0.,0.,0.));#2=IFCCARTESIANPOINT((0.,4.,0.));\
             #3=IFCCARTESIANPOINT((4.,0.,0.));#4=IFCCARTESIANPOINT((4.,4.,0.));\
             #10=IFCBSPLINESURFACEWITHKNOTS(1,1,((#1,#2),(#3,#4)),.UNSPECIFIED.,.F.,.F.,.F.,(2,2),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.);\
             #11=IFCCARTESIANPOINT((1.,1.,0.));#12=IFCCARTESIANPOINT((2.,1.,0.));#13=IFCCARTESIANPOINT((2.,2.,0.));\
             #20=IFCPOLYLOOP((#11,#12,#13));#51=IFCFACEOUTERBOUND(#20,.T.);\
             #60=IFCADVANCEDFACE((#51),#10,.T.);#70=IFCOPENSHELL((#60));";
        let (modele, stats) = geom_de(donnees);
        let g = super::super::essais::geom(&modele, &stats);
        assert_eq!(g.solid(70, DMat4::IDENTITY, 0).unwrap().len(), 1);
    }
}
