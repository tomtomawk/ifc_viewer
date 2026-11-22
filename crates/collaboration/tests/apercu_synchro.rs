use collaboration::{partage, session::Session};
use modele::Projet;
use std::time::{Duration, Instant};

fn synchroniser(session: &mut Session) -> Projet {
    session.synchroniser(None, false).unwrap();
    let limite = Instant::now() + Duration::from_secs(10);
    while session.synchronisation_en_cours() {
        if session.recevoir().unwrap().is_some() {
            return session.affichage().unwrap();
        }
        assert!(Instant::now() < limite, "{}", session.etat);
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("{}", session.etat);
}

#[test]
fn le_conflit_ne_masque_pas_les_publications_apres_synchro_et_reouverture() {
    let mut p = Projet::depuis_gabarit("Partage", &modele::gabarit::Gabarit::defaut());
    let id_partage = p
        .ajouter_mur([30000.0, 0.0], [34000.0, 0.0], &Default::default())
        .unwrap();
    let root = std::env::temp_dir().join(format!("bimedit-apercu-{}", noyau::uuid::uuid()));
    std::fs::create_dir_all(&root).unwrap();
    let entree = root.join("projet.bimpartage");
    let cache_b = root.join("b");
    partage::creer(&p, &entree).unwrap();
    let (a, _, mut sa) = partage::ouvrir(&entree, &root.join("a")).unwrap();
    let (b, _, mut sb) = partage::ouvrir(&entree, &cache_b).unwrap();

    let mut qa = a.clone();
    let id_a = qa
        .ajouter_mur([0.0, 0.0], [4000.0, 0.0], &Default::default())
        .unwrap();
    let mut qb = b.clone();
    let id_b = qb
        .ajouter_mur([2000.0, -2000.0], [2000.0, 2000.0], &Default::default())
        .unwrap();
    sa.editer(&a, &qa).unwrap();
    sb.editer(&b, &qb).unwrap();
    let mut publie = synchroniser(&mut sa);
    assert!(sa.copie.attente.is_empty());

    let verifier_apercu = |visible: &Projet, nom: &str| {
        assert_eq!(
            visible
                .murs
                .iter()
                .find(|m| m.id == id_partage)
                .unwrap()
                .nom,
            nom
        );
        assert!(
            visible.murs.iter().any(|m| m.id == id_a),
            "mur publié manquant"
        );
        assert!(
            visible.murs.iter().any(|m| m.id == id_b),
            "proposition locale manquante"
        );
    };
    for nom in ["Mur publié depuis A", "Nouveau nom publié depuis A"] {
        let mut suite = publie.clone();
        suite
            .murs
            .iter_mut()
            .find(|m| m.id == id_partage)
            .unwrap()
            .nom = nom.into();
        sa.editer(&publie, &suite).unwrap();
        publie = synchroniser(&mut sa);
        assert!(sa.copie.attente.is_empty());
        let visible = synchroniser(&mut sb);
        assert_eq!(sb.copie.base.revision, sa.copie.base.revision);
        assert_eq!(sb.copie.conflits.len(), 1);
        assert_eq!(sb.copie.attente.len(), 1);
        verifier_apercu(&visible, nom);
        assert!(!sb.travail().unwrap().murs.iter().any(|m| m.id == id_b));
    }

    drop(sb);
    let (visible, _, sb) = partage::ouvrir(&entree, &cache_b).unwrap();
    verifier_apercu(&visible, "Nouveau nom publié depuis A");
    assert_eq!(sb.copie.conflits.len(), 1);
    assert!(sb.copie.apercu_conflit.is_some());
    drop(sb);
    drop(sa);
    std::fs::remove_dir_all(root).unwrap();
}
