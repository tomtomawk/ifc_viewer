//! Commande `ProfilMur` et effets des autres commandes sur les profils de mur.
use commandes::{Commande, Execution};
use modele::creation::{Implantation, ParametresMur};
use modele::murs::profil::ProfilMur;
use modele::*;

#[cfg(test)]
fn projet() -> (Projet, String) {
    let mut p = Projet::depuis_gabarit("Essai", &modele::gabarit::Gabarit::defaut());
    let rdc = p.niveaux[0].id.clone();
    let t = p.types_murs.iter().find(|t| t.couches.len() > 1).unwrap().id.clone();
    let prm = ParametresMur { hauteur: 2700.0, implantation: Implantation::Axe, niveau: Some(rdc), type_mur: Some(t), ..Default::default() };
    let id = p.ajouter_mur_axe(Axe::Droit { debut: [0.0, 0.0], fin: [4000.0, 0.0] }, &prm).unwrap();
    (p, id)
}

fn pignon() -> ProfilMur {
    ProfilMur { contours: vec![vec![[0.0, 0.0], [4000.0, 0.0], [4000.0, 2700.0], [2000.0, 4000.0], [0.0, 2700.0]]] }
}

#[test]
fn profil_pose_par_une_couche_retire_et_copie() {
    let (p, id) = projet();
    let couche = p.assemblage(&id).unwrap().couches[1].clone();
    // une couche désigne son assemblage
    let (q, ids) = p.executer(Commande::ProfilMur { id: couche, profil: Some(pignon()) }).unwrap();
    assert_eq!(ids, vec![id.clone()]);
    assert_eq!(q.profils_murs[&id], pignon());
    // copie : le profil suit le nouvel assemblage
    let (c, nouveaux) = q.executer(Commande::Copier { ids: vec![id.clone()], delta: [0.0, 5000.0, 0.0] }).unwrap();
    assert!(c.profils_murs.contains_key(&nouveaux[0]) && c.profils_murs.len() == 2);
    // Scinder et Attacher refusés tant que le profil est modifié
    assert!(q.executer(Commande::Scinder { id: id.clone(), point: [2000.0, 0.0], ecart: 0.0 }).is_err());
    // retour au rectangle, puis suppression du mur : plus de profil
    let (r, _) = q.executer(Commande::ProfilMur { id: id.clone(), profil: None }).unwrap();
    assert!(r.profils_murs.is_empty());
    let (s, _) = q.executer(Commande::Supprimer { ids: vec![id] }).unwrap();
    assert!(s.profils_murs.is_empty());
}

#[test]
fn profil_invalide_refuse() {
    let (p, id) = projet();
    let croise = ProfilMur { contours: vec![vec![[0.0, 0.0], [4000.0, 3000.0], [4000.0, 0.0], [0.0, 3000.0]]] };
    assert!(p.executer(Commande::ProfilMur { id: id.clone(), profil: Some(croise) }).is_err());
    assert!(p.executer(Commande::ProfilMur { id: "absent".into(), profil: Some(pignon()) }).is_err());
}

#[test]
fn deplacer_et_inverser_le_mur() {
    let (p, id) = projet();
    let (q, _) = p.executer(Commande::ProfilMur { id: id.clone(), profil: Some(pignon()) }).unwrap();
    let (r, _) = q.executer(Commande::Deplacer { ids: vec![id.clone()], delta: [500.0, 0.0, 0.0] }).unwrap();
    // déplacé en bloc : mêmes coordonnées dans le repère du mur
    assert_eq!(r.profils_murs[&id], pignon());
    // retourné sur sa ligne : le profil reste en place dans le projet
    let q = q.executer(Commande::ProfilMur { id: id.clone(), profil: Some(ProfilMur { contours: vec![vec![[0.0, 0.0], [4000.0, 0.0], [4000.0, 3500.0], [0.0, 2700.0]]] }) }).unwrap().0;
    let (s, _) = q.executer(Commande::InverserMur { id: id.clone() }).unwrap();
    s.verifier().unwrap();
    let monde = |p: &Projet| {
        let (o, u, _) = p.repere_profil(&id).unwrap();
        let mut v: Vec<[i64; 2]> = p.profils_murs[&id].contours[0].iter().map(|q| { let w = o + u * q[0]; [w.x.round() as i64, q[1].round() as i64] }).collect();
        v.sort();
        v
    };
    assert_eq!(monde(&q), monde(&s));
}
