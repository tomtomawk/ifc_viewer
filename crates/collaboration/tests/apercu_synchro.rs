#![allow(clippy::unwrap_used, clippy::panic, reason = "tests d'intégration : les fonctions d'aide hors #[test] échouent par panique, comme les tests")]
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
fn resolution_locale_publie_le_champ_choisi_et_garde_les_autres_valeurs_du_central() {
    let mut p = Projet::depuis_gabarit("Partage", &modele::gabarit::Gabarit::defaut());
    p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &Default::default())
        .unwrap();
    let root = std::env::temp_dir().join(format!("bimedit-resolution-{}", noyau::uuid::uuid()));
    std::fs::create_dir_all(&root).unwrap();
    let entree = root.join("projet.bimpartage");
    partage::creer(&p, &entree).unwrap();
    let (a, _, mut sa) = partage::ouvrir(&entree, &root.join("a")).unwrap();
    let (b, _, mut sb) = partage::ouvrir(&entree, &root.join("b")).unwrap();

    let mut central = a.clone();
    central.murs[0].nom = "Nom central".into();
    central.murs[0].decalage_bas = 125.0;
    sa.editer(&a, &central).unwrap();
    let mut local = b.clone();
    local.murs[0].nom = "Nom local".into();
    sb.editer(&b, &local).unwrap();
    let mut suite = local.clone();
    let id_suite = suite
        .ajouter_mur([30000.0, 0.0], [34000.0, 0.0], &Default::default())
        .unwrap();
    sb.editer(&local, &suite).unwrap();

    synchroniser(&mut sa);
    synchroniser(&mut sb);
    let bloque = sb.copie.attente[0].id.clone();
    let suivant = sb.copie.attente[1].id.clone();
    assert!(sb.copie.conflits.contains_key(&bloque));
    sb.resoudre_locale(&bloque).unwrap();
    assert_eq!(
        sb.copie.attente[0].compense.as_deref(),
        Some(bloque.as_str())
    );
    assert_eq!(sb.copie.attente[1].id, suivant);
    synchroniser(&mut sb);
    assert!(sb.copie.attente.is_empty());

    let central = synchroniser(&mut sa);
    let mur = central
        .murs
        .iter()
        .find(|mur| mur.id == p.murs[0].id)
        .unwrap();
    assert_eq!(mur.nom, "Nom local");
    assert_eq!(mur.decalage_bas, 125.0);
    assert!(central.murs.iter().any(|mur| mur.id == id_suite));
    drop(sa);
    drop(sb);
    std::fs::remove_dir_all(root).unwrap();
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
