//! Tests déplacés de `modele/volumes/solides.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use modele::*;
use echanges::depuis_ifc;
use modele::volumes::solides::*;

#[test]
fn conversion_ifc_rouvre_et_indices_invalides_refuses() {
    for fichier in [concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4.ifc"), concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")] {
        let (p, _) = depuis_ifc::convertir(std::path::Path::new(fichier), &|_| {}).unwrap();
        let p = Projet::lire_json(&p.json().unwrap()).unwrap();
        let scene = maillage::depuis_natif(&p, std::path::Path::new("test.json")).unwrap();
        assert!(!scene.objects.is_empty());
    }
    let m = Maillage {
        sommets: vec![[0.0; 3]],
        triangles: vec![[0, 1, 2]],
    };
    assert!(maillage(&m).is_err());
}
