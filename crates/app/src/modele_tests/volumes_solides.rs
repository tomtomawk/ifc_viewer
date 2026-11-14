//! Tests déplacés de `modele/volumes/solides.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use crate::modele::*;
use crate::echanges::depuis_ifc;
use crate::modele::volumes::solides::*;

#[test]
fn conversion_ifc_rouvre_et_indices_invalides_refuses() {
    for fichier in [concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4.ifc"), concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")] {
        let (p, _) = depuis_ifc::convertir(std::path::Path::new(fichier), &|_| {}).unwrap();
        let p = Projet::lire_json(&p.json().unwrap()).unwrap();
        let scene = crate::rendu::scene::depuis_natif(&p, std::path::Path::new("test.json")).unwrap();
        assert!(!scene.objects.is_empty());
    }
    let m = Maillage {
        sommets: vec![[0.0; 3]],
        triangles: vec![[0, 1, 2]],
    };
    assert!(maillage(&m).is_err());
}

/// Recalcul partiel : après le déplacement d'un mur, le calcul groupé (qui reprend de
/// la mémoire les découpes inchangées) rend encore exactement le calcul mur par mur.
#[test]
fn recalcul_partiel_identique_apres_modification() {
    let f = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_native.modele.json");
    let mut p = Projet::lire_json(&std::fs::read_to_string(f).unwrap()).unwrap();
    let j = crate::modele::murs::jonctions::coupes(&p);
    let avant = murs(&p, &j);
    for k in 0..3 {
        let id = p.murs[k * 3 % p.murs.len()].id.clone();
        p = p.executer(crate::edition::commandes::Commande::Deplacer { ids: vec![id], delta: [0.0, 137.0 + k as f64, 0.0] }).unwrap().0;
        let j = crate::modele::murs::jonctions::coupes(&p);
        let groupe = murs(&p, &j);
        assert!(groupe != avant, "le déplacement doit changer des solides");
        for (m, g) in p.murs.iter().zip(groupe) {
            assert!(g == mur(&p, m, &j), "déplacement {k} : {}", m.id);
        }
    }
}
