use super::super::essais::{boite, geom, geom_de};
use glam::{DMat4, DVec3};
use noyau::csg::signed_volume;

const PROFIL: &str = "#90=IFCRECTANGLEPROFILEDEF(.AREA.,$,$,1.,0.5);#91=IFCCARTESIANPOINT((0.,0.,0.));#92=IFCAXIS2PLACEMENT3D(#91,$,$);";

fn volume_et_boite(donnees: &str, id: u64) -> (f64, DVec3, DVec3) {
    let (m, stats) = geom_de(&format!("{PROFIL}{donnees}"));
    let g = geom(&m, &stats);
    let tris = g.solid(id, DMat4::IDENTITY, 0).expect("solide");
    let (a, b) = boite(&tris);
    (signed_volume(&tris), a, b)
}

fn proche(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-9
}

/// Profil 1 × 0,5 le long d'une polyligne en L (4 + 3 m) sur un plan horizontal :
/// onglet exact, volume 0,5 × 7.
#[test]
fn surface_curve_polyligne_en_l() {
    let (v, a, b) = volume_et_boite(
        "#1=IFCCARTESIANPOINT((0.,0.,0.));#2=IFCCARTESIANPOINT((4.,0.,0.));#3=IFCCARTESIANPOINT((4.,3.,0.));\
         #4=IFCPOLYLINE((#1,#2,#3));#5=IFCPLANE(#92);\
         #10=IFCSURFACECURVESWEPTAREASOLID(#90,#92,#4,$,$,#5);",
        10,
    );
    assert!((v - 3.5).abs() < 1e-9, "volume {v}");
    assert!(proche(a, DVec3::new(0.0, -0.5, -0.25)) && proche(b, DVec3::new(4.5, 3.0, 0.25)), "boîte {a} {b}");
}

/// Paramètres de début et de fin d'une polyligne : seul [0,5 ; 1,5] est balayé (3,5 + 1,5 m).
#[test]
fn surface_curve_parametres_de_polyligne() {
    let (v, _, _) = volume_et_boite(
        "#1=IFCCARTESIANPOINT((0.,0.,0.));#2=IFCCARTESIANPOINT((4.,0.,0.));#3=IFCCARTESIANPOINT((4.,3.,0.));\
         #4=IFCPOLYLINE((#1,#2,#3));#5=IFCPLANE(#92);\
         #10=IFCSURFACECURVESWEPTAREASOLID(#90,#92,#4,0.5,1.5,#5);",
        10,
    );
    // 2 m sur le premier segment (de x = 2 à 4) et 1,5 m sur le second
    assert!((v - 0.5 * 3.5).abs() < 1e-9, "volume {v}");
}

/// Référence fixe verticale le long d'un quart de cercle de rayon 5 : volume 0,5 × 5π/2
/// (arc échantillonné : écart de corde < 1 %).
#[test]
fn reference_fixe_arc() {
    let (v, a, b) = volume_et_boite(
        "#1=IFCCARTESIANPOINT((0.,0.,0.));#2=IFCAXIS2PLACEMENT3D(#1,$,$);#3=IFCCIRCLE(#2,5.);\
         #4=IFCTRIMMEDCURVE(#3,(IFCPARAMETERVALUE(0.)),(IFCPARAMETERVALUE(1.5707963267948966)),.T.,.PARAMETER.);\
         #5=IFCDIRECTION((0.,0.,1.));\
         #10=IFCFIXEDREFERENCESWEPTAREASOLID(#90,#92,#4,$,$,#5);",
        10,
    );
    let attendu = 0.5 * 5.0 * std::f64::consts::FRAC_PI_2;
    assert!((v - attendu).abs() / attendu < 0.01, "volume {v}, attendu {attendu}");
    // X du profil (1 m) vertical, Y (0,5 m) dans le plan : rayon 4,75 à 5,25
    assert!((a.z + 0.5).abs() < 1e-9 && (b.z - 0.5).abs() < 1e-9, "boîte {a} {b}");
    // extrémités portées par la première et la dernière corde (inclinées de π/32)
    assert!((b.x - 5.25).abs() < 0.03 && (b.y - 5.25).abs() < 0.03 && a.x.abs() < 0.03, "boîte {a} {b}");
}

/// Directrice composite de deux polylignes alignées (2 + 3 m) : volume 0,5 × 5.
#[test]
fn surface_curve_composite() {
    let (v, a, b) = volume_et_boite(
        "#1=IFCCARTESIANPOINT((0.,0.,0.));#2=IFCCARTESIANPOINT((2.,0.,0.));#3=IFCCARTESIANPOINT((5.,0.,0.));\
         #4=IFCPOLYLINE((#1,#2));#5=IFCPOLYLINE((#2,#3));\
         #6=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#4);#7=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#5);\
         #8=IFCCOMPOSITECURVE((#6,#7),.F.);#9=IFCPLANE(#92);\
         #10=IFCSURFACECURVESWEPTAREASOLID(#90,#92,#8,$,$,#9);",
        10,
    );
    assert!((v - 2.5).abs() < 1e-9, "volume {v}");
    assert!(proche(a, DVec3::new(0.0, -0.5, -0.25)) && proche(b, DVec3::new(5.0, 0.5, 0.25)), "boîte {a} {b}");
}

/// Solide sectionné : section 1 × 1 à 0 m, 2 × 1 à 10 m (décalée de 1 m vers le haut) :
/// aire linéaire de 1 à 2, volume 15.
#[test]
fn solide_sectionne_horizontal() {
    let (v, a, b) = volume_et_boite(
        "#1=IFCCARTESIANPOINT((0.,0.,0.));#2=IFCCARTESIANPOINT((10.,0.,0.));#3=IFCPOLYLINE((#1,#2));\
         #4=IFCRECTANGLEPROFILEDEF(.AREA.,$,$,1.,1.);#5=IFCRECTANGLEPROFILEDEF(.AREA.,$,$,2.,1.);\
         #6=IFCPOINTBYDISTANCEEXPRESSION(IFCNONNEGATIVELENGTHMEASURE(0.),$,$,$,#3);#7=IFCAXIS2PLACEMENTLINEAR(#6,$,$);\
         #8=IFCPOINTBYDISTANCEEXPRESSION(IFCNONNEGATIVELENGTHMEASURE(10.),$,1.,$,#3);#9=IFCAXIS2PLACEMENTLINEAR(#8,$,$);\
         #10=IFCSECTIONEDSOLIDHORIZONTAL(#3,(#4,#5),(#7,#9));",
        10,
    );
    assert!((v - 15.0).abs() < 1e-9, "volume {v}");
    assert!(proche(a, DVec3::new(0.0, -1.0, -0.5)) && proche(b, DVec3::new(10.0, 1.0, 1.5)), "boîte {a} {b}");
}

/// Directrice absente : None, sans erreur.
#[test]
fn directrice_absente() {
    let (m, stats) = geom_de(&format!("{PROFIL}#10=IFCSURFACECURVESWEPTAREASOLID(#90,#92,#999,$,$,$);"));
    assert!(geom(&m, &stats).solid(10, DMat4::IDENTITY, 0).is_none());
}
