//! Liens IFC dans la scène native : objets ajoutés, placés, rattachés à un niveau ; un
//! lien absent n'empêche pas d'ouvrir le projet.
#![allow(clippy::unwrap_used, reason = "tests d'intégration : les fonctions d'aide hors #[test] échouent par panique, comme les tests")]
use modele::liens::LienIfc;
use modele::identifiants::id_natif;

fn maison() -> modele::Projet {
    modele::Projet::lire_json(include_str!("../../../samples/maison_native.bem")).unwrap()
}

fn lien(chemin: &str, decalage: [f64; 3], rotation: f64) -> LienIfc {
    LienIfc { id: "lien-1".into(), nom: "Structure".into(), chemin: chemin.into(), decalage, rotation, visible: true }
}

const IFC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4.ifc");

#[test]
fn objets_lies_dans_la_scene() {
    let p = maison();
    let seule = maillage::depuis_natif(&p, std::path::Path::new("maison.modele.json")).unwrap();
    let fichier = maillage::liens::charger(std::path::Path::new(IFC)).unwrap();
    let mut q = p.clone();
    q.liens.push(lien(IFC, [10_000.0, 0.0, 0.0], 0.0));
    q.verifier().unwrap();
    let s = maillage::depuis_natif(&q, std::path::Path::new("maison.modele.json")).unwrap();
    assert_eq!(s.objects.len(), seule.objects.len() + fichier.elements.len());
    let niveaux: Vec<u64> = q.niveaux.iter().map(|n| id_natif("niveau", &n.id)).collect();
    for e in &fichier.elements {
        let id = id_natif("lien", &format!("lien-1/{}", e.step));
        let o = &s.objects[*s.object_of.get(&id).expect("objet lié dans la scène") as usize];
        assert_eq!(o.class, e.classe);
        assert!(o.storey.is_some_and(|n| niveaux.contains(&n)), "rattaché à un niveau du projet");
        // décalage de 10 m en X, sans rotation (repère de la scène : origine retirée)
        let x_min = e.parts.iter().flat_map(|p| p.1.iter().flatten()).map(|v| v.x).fold(f64::INFINITY, f64::min);
        assert!(((o.min.x as f64 + s.origin.x) - (x_min + 10.0)).abs() < 1e-3);
    }
    // masqué : rien n'est ajouté
    q.liens[0].visible = false;
    assert_eq!(maillage::depuis_natif(&q, std::path::Path::new("m.json")).unwrap().objects.len(), seule.objects.len());
}

#[test]
fn lien_absent_signale_sans_bloquer() {
    let mut p = maison();
    p.liens.push(lien("/dossier/absent.ifc", [0.0; 3], 0.0));
    let s = maillage::depuis_natif(&p, std::path::Path::new("m.json")).unwrap();
    assert!(s.unsupported.iter().any(|(t, _)| t.contains("absent.ifc")), "{:?}", s.unsupported);
}

/// Rotation de 90° : un point du fichier en (x, 0) passe en (0, x).
#[test]
fn rotation_du_lien() {
    let mut p = maison();
    p.liens.push(lien(IFC, [0.0; 3], 90.0));
    let s = maillage::depuis_natif(&p, std::path::Path::new("m.json")).unwrap();
    let fichier = maillage::liens::charger(std::path::Path::new(IFC)).unwrap();
    let e = &fichier.elements[0];
    let o = &s.objects[*s.object_of.get(&id_natif("lien", &format!("lien-1/{}", e.step))).unwrap() as usize];
    let ymax_fichier = e.parts.iter().flat_map(|p| p.1.iter().flatten()).map(|v| v.x).fold(f64::NEG_INFINITY, f64::max);
    assert!(((o.max.y as f64 + s.origin.y) - ymax_fichier).abs() < 1e-3);
}
