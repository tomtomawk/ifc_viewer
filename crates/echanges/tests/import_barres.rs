//! Poteaux et poutres IFC simples convertis en barres natives
//! (`samples/test_ifc4_proprietes.ifc`).
#![allow(clippy::unwrap_used, reason = "tests d'intégration : les fonctions d'aide hors #[test] échouent par panique, comme les tests")]
use echanges::{depuis_ifc, export_ifc};
use modele::volumes::{geometrie, solides};
use modele::{GenreBarre, Projet};
use std::path::Path;

const ECHANTILLON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_proprietes.ifc");

fn convertir_texte(texte: &str, nom: &str) -> Projet {
    let chemin = std::env::temp_dir().join(format!("bimedit-{nom}-{}.ifc", std::process::id()));
    std::fs::write(&chemin, texte).unwrap();
    let p = depuis_ifc::convertir(&chemin, &|_| {}).unwrap().0;
    let _ = std::fs::remove_file(&chemin);
    p
}

#[test]
fn poteau_et_poutre_rectangulaires_en_barres() {
    let p = depuis_ifc::convertir(Path::new(ECHANTILLON), &|_| {}).unwrap().0;
    p.verifier().unwrap();
    let barre = |nom: &str| p.barres.iter().find(|b| b.nom == nom).unwrap_or_else(|| panic!("{nom} non converti"));
    // poteau 300 × 400 tourné de 30°, du niveau à 3 m
    let poteau = barre("Poteau P1");
    assert_eq!(poteau.genre, GenreBarre::Poteau);
    let t = p.type_barre(poteau).unwrap();
    assert!((t.largeur - 300.0).abs() < 1e-6 && (t.hauteur - 400.0).abs() < 1e-6, "{t:?}");
    assert!((poteau.rotation - 30.0).abs() < 1e-6, "{}", poteau.rotation);
    let proche = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(proche(poteau.debut, [6000.0, 1000.0, 0.0]) && proche(poteau.fin, [6000.0, 1000.0, 3000.0]), "{poteau:?}");
    assert_eq!(poteau.origine.as_ref().and_then(|o| o.guid.as_deref()), Some("0PrctVUKr0kugbFTf53OC9"));
    assert!((geometrie::volume(&solides::barre(&p, poteau).unwrap()) - 3.6e8).abs() < 1.0);
    // poutre 200 × 500 de x = 0 à 4 000, axe à 2 750 (sous-face à 2 500)
    let poutre = barre("Poutre B1");
    assert_eq!(poutre.genre, GenreBarre::Poutre);
    let t = p.type_barre(poutre).unwrap();
    assert!((t.largeur - 200.0).abs() < 1e-6 && (t.hauteur - 500.0).abs() < 1e-6 && t.nom.starts_with("Poutre"), "{t:?}");
    assert!(proche(poutre.debut, [0.0, -1500.0, 2750.0]) && proche(poutre.fin, [4000.0, -1500.0, 2750.0]), "{poutre:?}");
    let solide = solides::barre(&p, poutre).unwrap();
    assert!((geometrie::volume(&solide) - 4e8).abs() < 1.0);
    let z = solide.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v.z), b.max(v.z)));
    assert!((z.0 - 2500.0).abs() < 1e-6 && (z.1 - 3000.0).abs() < 1e-6, "{z:?}");
    // section ronde : reste importé, avec la raison
    let rond = p.importes.iter().find(|e| e.nom == "Poteau rond").unwrap();
    assert!(rond.raison.contains("rectangulaire"), "{}", rond.raison);
    // export : classe et GUID d'origine
    let s = export_ifc::exporter(&p).unwrap();
    assert!(s.contains("IFCCOLUMN('0PrctVUKr0kugbFTf53OC9'") && s.contains("IFCBEAM('0PrctVUKr0kugbFTf53OJ9'"));
}

/// Poutre dont la section est tournée autour de son axe, poteau incliné : importés.
#[test]
fn poutre_tournee_et_poteau_incline_importes() {
    let texte = std::fs::read_to_string(ECHANTILLON).unwrap();
    let tournee = texte.replace("#512=IFCDIRECTION((0.,1.,0.));", "#512=IFCDIRECTION((0.,0.7071,0.7071));");
    let incline = tournee.replace("#303=IFCAXIS2PLACEMENT3D(#304,#109,#305);", "#303=IFCAXIS2PLACEMENT3D(#304,#390,$);\n#390=IFCDIRECTION((0.3,0.,1.));");
    let p = convertir_texte(&incline, "barres");
    let raison = |nom: &str| p.importes.iter().find(|e| e.nom == nom).map(|e| e.raison.clone()).unwrap_or_else(|| panic!("{nom} non importé"));
    assert_eq!(raison("Poutre B1"), "section de poutre tournée autour de son axe");
    assert_eq!(raison("Poteau P1"), "poteau non vertical");
    assert!(p.barres.is_empty());
}
