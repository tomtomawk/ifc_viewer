//! Voisinage spatial des transactions concurrentes (C4) : séparation verticale des murs
//! et changements de nom sans effet géométrique ; jonctions toujours protégées.
use super::*;
use super::stockage::Etat;
use modele::{Haut, Niveau};

/// maison_native avec un niveau à 12 m portant un mur de même tracé que rdc-m0.
fn projet() -> Projet {
    let mut p = Projet::lire_json(include_str!("../../../samples/maison_native.bem")).unwrap();
    p.identite = uuid();
    p.niveaux.push(Niveau { id: "haut".into(), nom: "Haut".into(), altitude: 12000.0, origine: None });
    let mut m = p.murs.iter().find(|m| m.id == "rdc-m0").unwrap().clone();
    m.id = "haut-m0".into();
    m.nom = "Mur haut".into();
    m.niveau = "haut".into();
    p.murs.push(m);
    p.verifier().unwrap();
    p
}

fn hauteur(p: &Projet, id: &str, h: f64) -> Projet {
    let mut q = p.clone();
    q.murs.iter_mut().find(|m| m.id == id).unwrap().haut = Haut::Libre { hauteur: h };
    q
}

fn nom(p: &Projet, id: &str, n: &str) -> Projet {
    let mut q = p.clone();
    q.murs.iter_mut().find(|m| m.id == id).unwrap().nom = n.into();
    q
}

/// Deux postes partent de `p` ; la proposition de A est acceptée, puis celle de B.
fn concurrentes(p: &Projet, a: &Projet, b: &Projet) -> bool {
    let mut e = Etat::initial(p).unwrap();
    for (q, poste) in [(a, "poste-a"), (b, "poste-b")] {
        let mut t = Transaction::entre(p, q, poste, 0).unwrap();
        for cle in t.cles_reservation(&e.objets) {
            e.baux.insert(cle.clone(), Bail { proprietaire: t.session.clone(), generation: 1, actif: true });
            t.reservations.insert(cle, 1);
        }
        match e.accepter(&t) {
            Ok(suivant) => e = suivant,
            Err(_) => return false,
        }
    }
    true
}

#[test]
fn murs_separes_verticalement_et_noms_ne_se_bloquent_plus() {
    let p = projet();
    let h = hauteur(&p, "rdc-m0", 3200.0);
    // cas signalé : hauteur d'un mur, nom du mur de même tracé 12 m plus haut, dans les deux ordres
    let renomme = nom(&p, "haut-m0", "Mur haut renommé");
    assert!(concurrentes(&p, &h, &renomme));
    assert!(concurrentes(&p, &renomme, &h));
    // deux hauteurs, murs séparés de 9 m
    assert!(concurrentes(&p, &h, &hauteur(&p, "haut-m0", 2800.0)));
    // nom d'un mur joint, puis hauteur de son voisin
    assert!(concurrentes(&p, &nom(&p, "rdc-m1", "Mur B"), &h));
}

#[test]
fn jonctions_et_murs_superposes_restent_conflictuels() {
    let p = projet();
    // murs joints au même niveau
    assert!(!concurrentes(&p, &hauteur(&p, "rdc-m0", 3100.0), &hauteur(&p, "rdc-m1", 3150.0)));
    // le mur du bas monte jusqu'au mur du haut
    assert!(!concurrentes(&p, &hauteur(&p, "rdc-m0", 12500.0), &hauteur(&p, "haut-m0", 2800.0)));
    // mur de l'étage posé sur le mur du rez-de-chaussée (même tracé, 3 m)
    assert!(!concurrentes(&p, &hauteur(&p, "rdc-m0", 3100.0), &hauteur(&p, "etage-m0", 2900.0)));
}
