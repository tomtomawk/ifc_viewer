//! Commandes des profilés de mur : création, type, suivi du mur hôte.
use commandes::{Commande, Execution};
use modele::creation::{Implantation, ParametresMur};
use modele::elements::modenature::{Modenature, TypeModenature};
use modele::*;

#[cfg(test)]
fn projet() -> (Projet, String, String) {
    let mut p = Projet::depuis_gabarit("Essai", &modele::gabarit::Gabarit::defaut());
    let rdc = p.niveaux[0].id.clone();
    let t = p.types_murs.iter().find(|t| t.couches.len() > 1).unwrap().id.clone();
    let prm = ParametresMur { hauteur: 2700.0, implantation: Implantation::Axe, niveau: Some(rdc), type_mur: Some(t), ..Default::default() };
    let id = p.ajouter_mur_axe(Axe::Droit { debut: [0.0, 0.0], fin: [4000.0, 0.0] }, &prm).unwrap();
    let t = TypeModenature { id: String::new(), nom: "Bandeau".into(), creux: false, profil: None, largeur: 50.0, hauteur: 200.0, materiau: None, decalage: 0.0, retrait: 0.0 };
    let (p, ids) = p.executer(Commande::TypeModenature { type_modenature: t }).unwrap();
    (p, id, ids[0].clone())
}

#[test]
fn pose_copie_et_suppression_avec_le_mur() {
    let (p, mur, t) = projet();
    let couche = p.assemblage(&mur).unwrap().couches[2].clone();
    let m = Modenature { id: String::new(), nom: "Bandeau".into(), mur: couche, type_modenature: t.clone(), interieur: false, hauteur: 1000.0, depuis_haut: false, debut: None, fin: None };
    let (q, ids) = p.executer(Commande::Modenature { modenature: m.clone() }).unwrap();
    assert_eq!(q.modenatures[0].mur, mur, "une couche désigne son assemblage");
    // copie du mur : le profilé suit sur la copie
    let (c, _) = q.executer(Commande::Copier { ids: vec![mur.clone()], delta: [0.0, 5000.0, 0.0] }).unwrap();
    assert_eq!(c.modenatures.len(), 2);
    assert!(c.modenatures.iter().any(|x| x.mur != mur && c.assemblage(&x.mur).is_some()));
    // suppression du mur : le profilé part avec lui ; suppression du profilé seul
    assert!(q.executer(Commande::Supprimer { ids: vec![mur.clone()] }).unwrap().0.modenatures.is_empty());
    let (s, _) = q.executer(Commande::Supprimer { ids: ids.clone() }).unwrap();
    assert!(s.modenatures.is_empty() && s.assemblage(&mur).is_some());
    // refus : type absent, hôte absent, type sans nom
    assert!(p.executer(Commande::Modenature { modenature: Modenature { type_modenature: "absent".into(), ..m.clone() } }).is_err());
    assert!(p.executer(Commande::Modenature { modenature: Modenature { mur: "absent".into(), ..m } }).is_err());
    let mut t = p.type_modenature(&t).unwrap().clone();
    t.nom.clear();
    assert!(p.executer(Commande::TypeModenature { type_modenature: t }).is_err());
}
