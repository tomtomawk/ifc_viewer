//! Tests déplacés de `modele/volumes/solides.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use commandes::Execution;
use modele::*;
use modele::volumes::solides::*;

/// Recalcul partiel : après le déplacement d'un mur, le calcul groupé (qui reprend de
/// la mémoire les découpes inchangées) rend encore exactement le calcul mur par mur.
#[test]
fn recalcul_partiel_identique_apres_modification() {
    let f = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_native.modele.json");
    let mut p = Projet::lire_json(&std::fs::read_to_string(f).unwrap()).unwrap();
    let j = modele::murs::jonctions::coupes(&p);
    let avant = murs(&p, &j);
    for k in 0..3 {
        let id = p.murs[k * 3 % p.murs.len()].id.clone();
        p = p.executer(commandes::Commande::Deplacer { ids: vec![id], delta: [0.0, 137.0 + k as f64, 0.0] }).unwrap().0;
        let j = modele::murs::jonctions::coupes(&p);
        let groupe = murs(&p, &j);
        assert!(groupe != avant, "le déplacement doit changer des solides");
        for (m, g) in p.murs.iter().zip(groupe) {
            assert!(g == mur(&p, m, &j), "déplacement {k} : {}", m.id);
        }
    }
}
