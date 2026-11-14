//! Tests déplacés de `modele/murs/assemblage.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use crate::edition::Execution;
use crate::modele::*;
use crate::modele::creation::ParametresMur;
use crate::modele::murs::assemblage::*;

fn type_trois_couches() -> TypeCompose {
    let c = |m: &str, e: f64, f| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f, plume: None, retour: false, ossature: None };
    TypeCompose {
        id: "t".into(),
        nom: "Mur isolé".into(),
        couches: vec![c("Enduit", 20.0, FonctionCouche::Finition), c("Béton", 200.0, FonctionCouche::Structure), c("Laine de verre", 100.0, FonctionCouche::Isolation)],
        categorie: None, couches_detaillees: true,
        origine: None,
    }
}

#[test]
fn modification_type_actualise_tous_les_murs_sans_detacher_les_couches() {
    let mut p = Projet::nouveau("A");
    p.types_murs.push(type_trois_couches());
    let mut ids = Vec::new();
    for y in [0.0, 1000.0] {
        let id = p.ajouter_mur([0.0, y], [5000.0, y], &ParametresMur::default()).unwrap();
        p.murs.iter_mut().find(|m| m.id == id).unwrap().type_mur = "t".into();
        p.eclater_mur(&id).unwrap();
        p.implantations.insert(id.clone(), Reference::FaceExterieure);
        ids.push(id);
    }
    let ancienne_couche = p.assemblage(&ids[0]).unwrap().couches[2].clone();
    let nouvel_hote = p.assemblage(&ids[0]).unwrap().couches[1].clone();
    p.ouvertures.push(Ouverture { id: "ouverture-test".into(), nom: "Ouverture".into(), genre: GenreOuverture::Libre, mur: ancienne_couche, type_ouverture: None, abscisse: 2500.0, largeur: 600.0, hauteur: 1000.0, allege: 500.0, representation: None, valeurs: Default::default(), materiaux: Default::default(), retournee: false, inversee: false, origine: None });
    let avant = p.clone();
    let axes: Vec<_> = ids.iter().map(|id| p.trajectoire_justification(id).unwrap().0).collect();
    let t = p.types_murs.iter_mut().find(|t| t.id == "t").unwrap();
    t.couches[0].epaisseur = 45.0;
    t.couches.pop();
    p.actualiser_types_murs_modifies(&avant).unwrap();
    assert!(p.detacher_couches_disjointes(&avant).is_empty());
    assert_eq!(p.ouvertures[0].mur, nouvel_hote);
    p.recaler_menuiseries(&avant).unwrap();
    for (id, axe) in ids.iter().zip(axes) {
        let assemblage = p.assemblage(id).unwrap();
        assert_eq!(assemblage.type_mur, "t");
        assert_eq!(assemblage.couches.len(), 2);
        assert_eq!(p.trajectoire_justification(id).unwrap().0, axe);
        assert_eq!(p.couches_assemblage(id).iter().map(|m| p.type_mur(&m.type_mur).unwrap().epaisseur()).collect::<Vec<_>>(), [45.0, 200.0]);
    }
    p.verifier().unwrap();
}

#[test]
fn couche_editee_hors_du_mur_compose_se_detache() {
    use crate::edition::commandes::Commande;
    let mut p = Projet::nouveau("A");
    p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur::default()).unwrap();
    p.types_murs.push(type_trois_couches());
    let id = p.murs[0].id.clone();
    p.murs[0].type_mur = "t".into();
    p.eclater_mur(&id).unwrap();
    let couches = p.assemblage(&id).unwrap().couches.clone();
    let assemblage = |p: &Projet, c: &str| p.murs.iter().find(|m| m.id == c).unwrap().assemblage.clone();
    // glissée le long de l'axe : toujours parallèle et en contact
    let (q, _) = p.executer(Commande::Deplacer { ids: vec![couches[2].clone()], delta: [300.0, 0.0, 0.0] }).unwrap();
    assert_eq!(assemblage(&q, &couches[2]).as_deref(), Some(id.as_str()));
    // écartée de 500 mm : plus en contact
    let (q, _) = p.executer(Commande::Deplacer { ids: vec![couches[2].clone()], delta: [0.0, -500.0, 0.0] }).unwrap();
    assert!(assemblage(&q, &couches[2]).is_none());
    assert_eq!(q.assemblage(&id).unwrap().couches.len(), 2);
    // tournée de 10° : plus colinéaire
    let (q, _) = p.executer(Commande::Tourner { ids: vec![couches[0].clone()], centre: [0.0, 150.0], angle: 10.0 }).unwrap();
    assert!(assemblage(&q, &couches[0]).is_none());
    // poignée d'une couche tirée de biais
    let mut q = p.clone();
    q.deplacer_extremite_justification(&couches[1], true, [5000.0, 800.0]).unwrap();
    assert_eq!(q.detacher_couches_disjointes(&p), vec![couches[1].clone()]);
    // assemblage entier déplacé et tourné : rien ne se détache
    let (q, _) = p.executer(Commande::Tourner { ids: vec![id.clone()], centre: [0.0, 0.0], angle: 30.0 }).unwrap();
    assert_eq!(q.assemblage(&id).unwrap().couches, couches);
}
