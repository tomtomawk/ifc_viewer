//! Tests déplacés de `modele/elements/arbre.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use crate::modele::elements::arbre::*;

fn arbre(essence: Essence) -> Arbre {
    let (hauteur, diametre) = essence.dimensions();
    Arbre { id: "a1".into(), nom: String::new(), niveau: "n0".into(), position: [1000.0, 2000.0], decalage: 0.0, essence, hauteur, diametre, asset: None }
}

#[test]
fn creer_puis_supprimer_un_arbre() {
    use crate::modele::edition::commandes::Commande;
    let mut p = crate::modele::Projet::nouveau("Arbres");
    p.niveaux.push(crate::modele::Niveau { id: "n0".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
    let a = Arbre { id: String::new(), ..arbre(Essence::Conifere) };
    let (p, ids) = p.executer(Commande::Arbre { arbre: a }).unwrap();
    assert_eq!(p.arbres.len(), 1);
    let (p, _) = p.executer(Commande::Supprimer { ids }).unwrap();
    assert!(p.arbres.is_empty());
}
