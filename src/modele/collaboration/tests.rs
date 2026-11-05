use super::stockage::*;
use super::texte::*;
use super::*;
use std::path::PathBuf;

fn dossier() -> PathBuf {
    let p = std::env::temp_dir().join(format!("bimedit-collab-{}", uuid()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn exemple() -> Projet {
    let mut p =
        Projet::lire_json(include_str!("../../../samples/maison_native.modele.json")).unwrap();
    // La fixture n'est jamais modifiée sur disque.
    p.identite = uuid();
    p.verifier().unwrap();
    p
}
fn autoriser(e: &mut Etat, t: &mut Transaction) {
    for cle in t.cles_reservation(&e.objets) {
        e.baux.insert(
            cle.clone(),
            Bail {
                proprietaire: t.session.clone(),
                generation: 1,
                actif: true,
            },
        );
        t.reservations.insert(cle, 1);
    }
}
fn accepter(e: &mut Etat, mut t: Transaction) {
    autoriser(e, &mut t);
    *e = e.accepter(&t).unwrap();
}
fn transaction(a: &Projet, b: &Projet) -> Transaction {
    Transaction::entre(a, b, "poste-a", 0).unwrap()
}
fn texte_annotation(p: &mut Projet, id: &str, s: &str) {
    p.annotations.push(super::super::documentation::Annotation {
        id: id.into(),
        vue: p.vues[0].id.clone(),
        hauteur: 2.5,
        forme: super::super::documentation::AnnotationForme::Texte {
            point: [0.0, 0.0],
            texte: s.into(),
        },
    });
}

#[test]
fn identites_distribuees_et_migration_preserve_les_objets() {
    use super::super::commandes::Commande;
    let p = exemple();
    let a = p
        .executer(Commande::Niveau {
            nom: "A".into(),
            altitude: 6000.0,
        })
        .unwrap();
    let b = p
        .executer(Commande::Niveau {
            nom: "B".into(),
            altitude: 9000.0,
        })
        .unwrap();
    assert_ne!(a.1, b.1);
    let ancien = include_str!("../../../samples/maison_native.modele.json");
    let a = Projet::lire_json(ancien).unwrap();
    let b = Projet::lire_json(ancien).unwrap();
    assert_eq!(a.identite, b.identite);
    assert_eq!(a.version, 1);
    let original: Value = serde_json::from_str(ancien).unwrap();
    assert_eq!(a.murs[0].id, original["murs"][0]["id"]);
    assert_eq!(
        a.identite,
        Projet::lire_json(&a.json().unwrap()).unwrap().identite
    );
}
#[test]
fn objets_independants_et_fusion_explicite_deux_champs() {
    let p = exemple();
    let mut a = p.clone();
    let mut b = p.clone();
    a.vues[0].nom = "Architecte".into();
    b.vues[0].type_vue = "Coordination".into();
    let mut e = Etat::initial(&p).unwrap();
    accepter(&mut e, transaction(&p, &a));
    accepter(&mut e, transaction(&p, &b));
    let r = e.projection().unwrap();
    assert_eq!(r.vues[0].nom, "Architecte");
    assert_eq!(r.vues[0].type_vue, "Coordination");
    let mut c = p.clone();
    c.quadrillages
        .push(super::super::documentation::Quadrillage {
            id: uuid(),
            nom: "Axe".into(),
            debut: [0.0, 0.0],
            fin: [500.0, 0.0],
        });
    accepter(&mut e, transaction(&p, &c));
    assert!(!e.projection().unwrap().quadrillages.is_empty());
}
#[test]
fn meme_champ_conserve_trois_valeurs_et_pas_de_fusion_geometrique() {
    let p = exemple();
    let mut a = p.clone();
    let mut b = p.clone();
    a.vues[0].nom = "A".into();
    b.vues[0].nom = "B".into();
    let mut e = Etat::initial(&p).unwrap();
    accepter(&mut e, transaction(&p, &a));
    let erreur = e.accepter(&transaction(&p, &b)).unwrap_err();
    assert!(
        erreur
            .iter()
            .any(|c| c.base.is_some() && c.locale.is_some() && c.acceptee.is_some())
    );
    b = p.clone();
    b.vues[0].echelle = 200.0;
    assert!(e.accepter(&transaction(&p, &b)).is_err());
}
#[test]
fn suppression_hote_observe_ajout_ouverture_et_collection() {
    let mut p = Projet::nouveau("Hôte");
    p.ajouter_mur(
        [0.0, 0.0],
        [5000.0, 0.0],
        &super::super::edition::ParametresMur::default(),
    )
    .unwrap();
    let mut sup = p.clone();
    let hote = p.murs[0].id.clone();
    sup.murs.retain(|m| m.id != hote);
    sup.ouvertures.retain(|o| o.mur != hote);
    sup.contextes.remove(&hote);
    let t = Transaction::entre(&p, &sup, "a", 0).unwrap();
    let cmd = super::super::commandes::Commande::Ouverture {
        mur: hote,
        genre: super::super::GenreOuverture::Porte,
        type_ouverture: None,
        abscisse: 1000.0,
        largeur: 600.0,
        hauteur: 2000.0,
        allege: 0.0,
    };
    let distant = p.executer(cmd).unwrap().0;
    let e = Etat::initial(&distant).unwrap();
    assert!(
        appliquer(&t, &e.objets, 1, &p.identite)
            .unwrap_err()
            .iter()
            .any(|c| c.cle.starts_with('#'))
    );
}
#[test]
fn reservation_generation_et_compensation_refusent_ancien_detenteur() {
    let p = exemple();
    let d = Depot::nouveau(dossier());
    d.preparer().unwrap();
    let mut a = d.proteger().unwrap();
    let mut e = a.initialiser(&p).unwrap();
    let key = format!("murs/{}", p.murs[0].id);
    a.reserver(&mut e, &[key.clone()], "a", false, false)
        .unwrap();
    let generation = e.baux[&key].generation;
    a.reserver(&mut e, &[key.clone()], "b", true, false)
        .unwrap();
    assert!(e.baux[&key].generation > generation);
    let mut change = p.clone();
    change.murs[0].nom = "Local".into();
    let mut t = Transaction::entre(&p, &change, "a", 0).unwrap();
    t.reservations.insert(key.clone(), generation);
    assert!(!a.publier(&mut e, &t).unwrap().is_empty());
    let mut local = p.clone();
    local.vues[0].nom = "A".into();
    let t = transaction(&p, &local);
    accepter(&mut e, t.clone());
    let mut collegue = local.clone();
    collegue.vues[0].nom = "B".into();
    accepter(
        &mut e,
        Transaction::entre(&local, &collegue, "b", 1).unwrap(),
    );
    assert!(e.accepter(&t.inverse(e.revision)).is_err());
}
#[test]
fn yrs_converge_desordre_doublons_unicode_et_annulation_locale() {
    let mut a = Editeur::ouvrir(&EtatTexte::default(), "a").unwrap();
    let u = a.remplacer("été 🏠");
    let mut etat = EtatTexte::default();
    etat.recevoir(&u).unwrap();
    let mut b = Editeur::ouvrir(&etat, "b").unwrap();
    let ua = a.remplacer("été 🏠 A");
    let ub = b.remplacer("B été 🏠");
    a.recevoir(&ub).unwrap();
    b.recevoir(&ua).unwrap();
    a.recevoir(&ub).unwrap();
    assert_eq!(a.contenu(), b.contenu());
    let annule = a.annuler(false).unwrap();
    b.recevoir(&annule).unwrap();
    assert_eq!(a.contenu(), b.contenu());
    assert!(a.contenu().contains('B'));
    assert!(!a.contenu().contains('A'));
    let mut restaure = EtatTexte::default();
    for u in [&ub, &ua, &u, &annule, &ub] {
        restaure.recevoir(u).unwrap();
    }
    assert_eq!(restaure.contenu().unwrap(), a.contenu());
}
#[test]
fn texte_avant_creation_et_apres_suppression_sans_resurrection() {
    let mut p = exemple();
    let mut e = Etat::initial(&p).unwrap();
    let id = uuid();
    let mut edit = Editeur::ouvrir(&EtatTexte::default(), "a").unwrap();
    let u = edit.remplacer("initial");
    let mut t = transaction(&p, &p);
    t.textes.insert(id.clone(), u.clone());
    accepter(&mut e, t);
    assert!(
        !e.projection()
            .unwrap()
            .annotations
            .iter()
            .any(|a| a.id == id)
    );
    texte_annotation(&mut p, &id, "projection ignorée");
    let mut t = transaction(&e.projection().unwrap(), &p);
    t.textes.insert(id.clone(), u);
    accepter(&mut e, t);
    assert_eq!(e.textes[&id].contenu().unwrap(), "initial");
    let avant = e.projection().unwrap();
    let mut apres = avant.clone();
    apres.annotations.retain(|a| a.id != id);
    accepter(&mut e, transaction(&avant, &apres));
    let mut tardif = transaction(&apres, &apres);
    tardif
        .textes
        .insert(id.clone(), edit.remplacer("texte tardif récupérable"));
    accepter(&mut e, tardif);
    assert!(e.textes[&id].supprime);
    assert!(e.textes[&id].contenu().unwrap().contains("récupérable"));
    assert!(
        !e.projection()
            .unwrap()
            .annotations
            .iter()
            .any(|a| a.id == id)
    );
}
#[test]
fn protocole_validation_et_atomicite() {
    let p = exemple();
    let mut a = p.clone();
    a.vues[0].nom = "A".into();
    let mut t = transaction(&p, &a);
    let e = Etat::initial(&p).unwrap();
    t.protocole = 999;
    assert!(e.accepter(&t).is_err());
    t.protocole = PROTOCOLE;
    t.projet = uuid();
    assert!(e.accepter(&t).is_err());
    let avant = hash(&e);
    t.projet = p.identite.clone();
    t.changements.push(Changement {
        cle: "@/orientation_nord".into(),
        avant: Some(Value::from(0.0)),
        apres: Some(Value::String("invalide".into())),
    });
    assert!(e.accepter(&t).is_err());
    assert_eq!(hash(&e), avant);
}
#[test]
fn registre_queue_interrompue_corruption_et_instantane_crdt() {
    let mut p = exemple();
    texte_annotation(&mut p, "texte-snapshot", "Bonjour");
    let root = dossier();
    let d = Depot::nouveau(&root);
    d.preparer().unwrap();
    let attendu;
    {
        let mut a = d.proteger().unwrap();
        let mut e = a.initialiser(&p).unwrap();
        let mut change = p.clone();
        change.vues[0].nom = "accepté".into();
        let t = transaction(&p, &change);
        assert!(a.publier(&mut e, &t).unwrap().is_empty());
        assert!(a.publier(&mut e, &t).unwrap().is_empty());
        a.instantane(&e).unwrap();
        attendu = hash(&e);
    }
    use std::io::Write;
    let controle = root.join("controle");
    let len = std::fs::metadata(&controle).unwrap().len();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&controle)
        .unwrap()
        .write_all(&[12, 0, 0])
        .unwrap();
    {
        let mut a = d.proteger().unwrap();
        let e = a.lire().unwrap().unwrap();
        assert_eq!(hash(&e), attendu);
        assert_eq!(e.textes["texte-snapshot"].contenu().unwrap(), "Bonjour");
    }
    assert_eq!(std::fs::metadata(&controle).unwrap().len(), len);
    let mut bytes = std::fs::read(&controle).unwrap();
    bytes[10] ^= 1;
    std::fs::write(&controle, bytes).unwrap();
    assert!(
        d.proteger()
            .unwrap()
            .lire()
            .unwrap_err()
            .contains("Corruption")
    );
}
#[test]
fn ressources_immutables_taille_empreinte_et_absence() {
    let d = Depot::nouveau(dossier());
    d.preparer().unwrap();
    let v = serde_json::json!({"maillage":{"sommets":[1,2,3],"faces":[0,1,2]}});
    let id = d.sauver("paquets", &v).unwrap();
    let disque = String::from_utf8(d.lire("paquets", &id).unwrap()).unwrap();
    assert!(disque.contains("$ressource"));
    assert_eq!(d.charger::<Value>("paquets", &id).unwrap(), v);
    let ressources: Vec<_> = std::fs::read_dir(d.racine.join("ressources"))
        .unwrap()
        .map(|x| x.unwrap().path())
        .collect();
    std::fs::remove_file(&ressources[0]).unwrap();
    assert!(d.charger::<Value>("paquets", &id).is_err());
}

// Test exécuté dans de vrais processus. L'environnement est fourni par le parent.
#[test]
fn enfant_reservation() {
    let Ok(root) = std::env::var("BIMEDIT_TEST_DEPOT") else {
        return;
    };
    let nom = std::env::var("BIMEDIT_TEST_POSTE").unwrap();
    let d = Depot::nouveau(&root);
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !std::path::Path::new(&root).join("depart").exists() {
        assert!(std::time::Instant::now() < limite);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    loop {
        if let Ok(mut a) = d.proteger() {
            let mut e = a.lire().unwrap().unwrap();
            let resultat = a.reserver(&mut e, &["groupe-test".into()], &nom, false, false);
            std::fs::write(
                std::path::Path::new(&root).join(format!("resultat-{nom}")),
                if resultat.is_ok() {
                    "gagnant"
                } else {
                    "refusé"
                },
            )
            .unwrap();
            break;
        }
        assert!(std::time::Instant::now() < limite);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
#[test]
fn course_entre_deux_processus_et_controle_non_remplacable() {
    let root = dossier();
    let d = Depot::nouveau(&root);
    d.preparer().unwrap();
    {
        d.proteger().unwrap().initialiser(&exemple()).unwrap();
    }
    let exe = std::env::current_exe().unwrap();
    let mut enfants = Vec::new();
    for poste in ["a", "b"] {
        enfants.push(
            std::process::Command::new(&exe)
                .args([
                    "--exact",
                    "modele::collaboration::tests::enfant_reservation",
                    "--nocapture",
                ])
                .env("BIMEDIT_TEST_DEPOT", &root)
                .env("BIMEDIT_TEST_POSTE", poste)
                .spawn()
                .unwrap(),
        );
    }
    std::fs::write(root.join("depart"), b"go").unwrap();
    for e in &mut enfants {
        assert!(e.wait().unwrap().success());
    }
    let a = std::fs::read_to_string(root.join("resultat-a")).unwrap();
    let b = std::fs::read_to_string(root.join("resultat-b")).unwrap();
    assert_ne!(a, b);
    let _protection = d.proteger().unwrap();
    assert!(d.proteger().is_err());
    #[cfg(windows)]
    {
        assert!(std::fs::remove_file(root.join("controle")).is_err());
        assert!(std::fs::rename(root.join("controle"), root.join("remplace")).is_err());
    }
}

#[test]
fn enfant_publication() {
    let Ok(root) = std::env::var("BIMEDIT_TEST_DEPOT") else {
        return;
    };
    let d = Depot::nouveau(&root);
    let mut a = loop {
        if let Ok(a) = d.proteger() {
            break a;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let mut e = a.lire().unwrap().unwrap();
    if std::env::var_os("BIMEDIT_TEST_SNAPSHOT").is_some() {
        a.instantane(&e).unwrap();
        return;
    }
    let t: Transaction =
        serde_json::from_slice(&std::fs::read(d.racine.join("a-publier.json")).unwrap()).unwrap();
    assert!(a.publier(&mut e, &t).unwrap().is_empty());
}
#[test]
fn crash_processus_aux_frontieres_et_resultat_incertain() {
    let p = exemple();
    let mut change = p.clone();
    change.vues[0].nom = "changement atomique".into();
    let t = transaction(&p, &change);
    let exe = std::env::current_exe().unwrap();
    for point in [
        "temporaire-cree",
        "temporaire-ecrit",
        "temporaire-durable",
        "ressource-publiee",
        "registre-longueur",
        "registre-contenu",
        "registre-empreinte",
        "registre-durable",
    ] {
        let root = dossier();
        let d = Depot::nouveau(&root);
        d.preparer().unwrap();
        {
            d.proteger().unwrap().initialiser(&p).unwrap();
        }
        std::fs::write(root.join("a-publier.json"), encoder(&t).unwrap()).unwrap();
        let statut = std::process::Command::new(&exe)
            .args([
                "--exact",
                "modele::collaboration::tests::enfant_publication",
            ])
            .env("BIMEDIT_TEST_DEPOT", &root)
            .env("BIMEDIT_TEST_CRASH", point)
            .status()
            .unwrap();
        assert_eq!(statut.code(), Some(86), "{point}");
        let mut a = d.proteger().unwrap();
        let mut e = a.lire().unwrap().unwrap();
        let accepte = e.acceptes.contains_key(&t.id);
        assert_eq!(e.revision, if accepte { 1 } else { 0 }, "{point}");
        assert!(a.publier(&mut e, &t).unwrap().is_empty(), "{point}");
        assert_eq!(e.revision, 1);
        assert_eq!(e.projection().unwrap().vues[0].nom, "changement atomique");
    }
}
#[test]
fn instantane_et_publication_dans_deux_processus() {
    let p = exemple();
    let mut change = p.clone();
    change.vues[0].nom = "après instantané".into();
    let t = transaction(&p, &change);
    let root = dossier();
    let d = Depot::nouveau(&root);
    d.preparer().unwrap();
    {
        d.proteger().unwrap().initialiser(&p).unwrap();
    }
    std::fs::write(root.join("a-publier.json"), encoder(&t).unwrap()).unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut publication = std::process::Command::new(&exe)
        .args([
            "--exact",
            "modele::collaboration::tests::enfant_publication",
        ])
        .env("BIMEDIT_TEST_DEPOT", &root)
        .spawn()
        .unwrap();
    let mut instantane = std::process::Command::new(&exe)
        .args([
            "--exact",
            "modele::collaboration::tests::enfant_publication",
        ])
        .env("BIMEDIT_TEST_DEPOT", &root)
        .env("BIMEDIT_TEST_SNAPSHOT", "1")
        .spawn()
        .unwrap();
    assert!(publication.wait().unwrap().success());
    assert!(instantane.wait().unwrap().success());
    assert_eq!(
        d.proteger()
            .unwrap()
            .lire()
            .unwrap()
            .unwrap()
            .projection()
            .unwrap()
            .vues[0]
            .nom,
        "après instantané"
    );
}

fn attendre(s: &mut session::Session) -> Projet {
    let fin = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(p) = s.recevoir().unwrap() {
            return p;
        }
        assert!(std::time::Instant::now() < fin, "{}", s.etat);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
#[test]
fn reconnexion_copies_locales_et_compensation_sans_ecraser_collegue() {
    let p = exemple();
    let root = dossier();
    let local = dossier();
    let mut a = session::Session::ouvrir(&p, local.join("a.json"), root.clone()).unwrap();
    let mut b = session::Session::ouvrir(&p, local.join("b.json"), root).unwrap();
    a.synchroniser(None, false).unwrap();
    attendre(&mut a);
    b.synchroniser(None, false).unwrap();
    attendre(&mut b);
    let mut pa = p.clone();
    pa.vues[0].nom = "A".into();
    a.editer(&p, &pa).unwrap();
    let mut pb = p.clone();
    pb.vues[0].type_vue = "Collègue".into();
    b.editer(&p, &pb).unwrap();
    b.synchroniser(None, false).unwrap();
    attendre(&mut b);
    // Redémarrage avec une proposition persistante hors connexion.
    let central = a.copie.central.clone();
    drop(a);
    let mut a = session::Session::ouvrir(&p, local.join("a.json"), central).unwrap();
    assert_eq!(a.copie.attente.len(), 1);
    a.synchroniser(None, false).unwrap();
    let r = attendre(&mut a);
    assert_eq!(r.vues[0].nom, "A");
    assert_eq!(r.vues[0].type_vue, "Collègue");
    let annule = a.annuler(false).unwrap();
    assert_ne!(annule.vues[0].nom, "A");
    assert_eq!(annule.vues[0].type_vue, "Collègue");
}

#[test]
fn migrations_textuelles_concurrentes_ne_dupliquent_pas_le_contenu_initial() {
    let mut p = exemple();
    texte_annotation(&mut p, "graine", "texte initial");
    let a = Etat::initial(&p).unwrap();
    let b = Etat::initial(&p).unwrap();
    assert_eq!(hash(&a.textes), hash(&b.textes));
    let mut ea = Editeur::ouvrir(&a.textes["graine"], "a").unwrap();
    let mut eb = Editeur::ouvrir(&b.textes["graine"], "b").unwrap();
    let ua = ea.remplacer("texte initial A");
    let ub = eb.remplacer("B texte initial");
    ea.recevoir(&ub).unwrap();
    eb.recevoir(&ua).unwrap();
    assert_eq!(ea.contenu(), eb.contenu());
    assert_eq!(ea.contenu().matches("texte initial").count(), 1);
}

#[test]
fn actualisation_assemblage_transporte_couches_et_effets_atomiques() {
    let mut p = Projet::nouveau("Assemblage");
    p.ajouter_mur(
        [0.0, 0.0],
        [5000.0, 0.0],
        &super::super::edition::ParametresMur::default(),
    )
    .unwrap();
    p.types_murs[0].couches = vec![
        super::super::Couche {
            epaisseur: 100.0,
            ..Default::default()
        },
        super::super::Couche {
            epaisseur: 200.0,
            ..Default::default()
        },
    ];
    let id = p.murs[0].id.clone();
    p.eclater_mur(&id).unwrap();
    let type_id = p.assemblages[0].type_mur.clone();
    p.types_murs
        .iter_mut()
        .find(|t| t.id == type_id)
        .unwrap()
        .couches
        .push(super::super::Couche {
            epaisseur: 50.0,
            ..Default::default()
        });
    let (q, _) = p
        .executer(super::super::commandes::Commande::ActualiserType {
            assemblage: id.clone(),
        })
        .unwrap();
    assert_eq!(q.assemblages[0].couches.len(), 3);
    let t = transaction(&p, &q);
    assert!(t.changements.iter().any(|c| c.cle.starts_with("murs/")));
    let mut e = Etat::initial(&p).unwrap();
    accepter(&mut e, t);
    assert_eq!(e.projection().unwrap().assemblages[0].couches.len(), 3);
}

#[test]
fn propagation_famille_valide_instances_et_transporte_tout_le_resultat() {
    use super::super::famille_modelee::*;
    let mut p = Projet::nouveau("Famille");
    p.ajouter_mur(
        [0.0, 0.0],
        [6000.0, 0.0],
        &super::super::edition::ParametresMur::default(),
    )
    .unwrap();
    let mur = p.murs[0].id.clone();
    let (p, ids) = p
        .executer(super::super::commandes::Commande::Ouverture {
            mur,
            genre: super::super::GenreOuverture::Porte,
            type_ouverture: None,
            abscisse: 1500.0,
            largeur: 900.0,
            hauteur: 2150.0,
            allege: 0.0,
        })
        .unwrap();
    let f = FichierFamille {
        format: FORMAT_FAMILLE.into(),
        nom: "Porte partagée".into(),
        largeur: 900.0,
        hauteur: 2150.0,
        famille: FamilleModelee::gabarit_porte(),
    };
    let (p, type_id) = p
        .avec_famille(None, Some(&ids[0]), &f, 200.0, BTreeMap::new())
        .unwrap();
    let mut f = f;
    f.largeur = 1000.0;
    let (q, _) = p
        .avec_famille(Some(&type_id), None, &f, 200.0, BTreeMap::new())
        .unwrap();
    assert_eq!(q.ouvertures[0].largeur, 1000.0);
    let t = transaction(&p, &q);
    assert!(
        t.changements
            .iter()
            .any(|c| c.cle.starts_with("types_ouvertures/"))
    );
    assert!(
        t.changements
            .iter()
            .any(|c| c.cle.starts_with("ouvertures/"))
    );
    let mut e = Etat::initial(&p).unwrap();
    accepter(&mut e, t);
    assert_eq!(e.projection().unwrap().ouvertures[0].largeur, 1000.0);
    f.largeur = -1.0;
    assert!(
        p.avec_famille(Some(&type_id), None, &f, 200.0, BTreeMap::new())
            .is_err()
    );
}

#[test]
fn reponse_perdue_retrouve_uuid_et_contenu_prepare_sans_republication() {
    let p = exemple();
    let local = dossier();
    let central = dossier();
    let fichier = local.join("copie.json");
    let mut s = session::Session::ouvrir(&p, fichier.clone(), central.clone()).unwrap();
    s.synchroniser(None, false).unwrap();
    let p = attendre(&mut s);
    let mut q = p.clone();
    q.vues[0].nom = "Sans accusé".into();
    s.editer(&p, &q).unwrap();
    let id = s.copie.attente[0].id.clone();
    s.synchroniser(None, false).unwrap();
    let depot = Depot::nouveau(&central);
    let fin = std::time::Instant::now() + std::time::Duration::from_secs(5);
    // Attendre le dépôt préparé sans prendre l'autorité que le worker doit acquérir.
    while !s
        .chemin
        .with_extension(format!("prepare-{id}.json"))
        .exists()
    {
        assert!(std::time::Instant::now() < fin);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    loop {
        if let Ok(mut a) = depot.proteger() {
            if a.lire().unwrap().unwrap().acceptes.contains_key(&id) {
                break;
            }
        }
        assert!(std::time::Instant::now() < fin);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    drop(s);
    let mut s = loop {
        if let Ok(s) = session::Session::ouvrir(&p, fichier.clone(), central.clone()) {
            break s;
        }
        assert!(std::time::Instant::now() < fin);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(s.copie.attente.len(), 1);
    s.synchroniser(None, false).unwrap();
    let q = attendre(&mut s);
    assert_eq!(q.vues[0].nom, "Sans accusé");
    assert!(s.copie.attente.is_empty());
    assert_eq!(s.copie.base.revision, 1);
}

#[test]
fn session_ancien_detenteur_ne_reacquiert_pas_silencieusement() {
    let p = exemple();
    let local = dossier();
    let central = dossier();
    let mut s = session::Session::ouvrir(&p, local.join("a.json"), central.clone()).unwrap();
    s.synchroniser(None, false).unwrap();
    let p = attendre(&mut s);
    let cle = format!("murs/{}", p.murs[0].id);
    s.synchroniser(Some((vec![cle.clone()], false, false)), false)
        .unwrap();
    attendre(&mut s);
    let mut q = p.clone();
    q.murs[0].nom = "Réservation révoquée".into();
    s.editer(&p, &q).unwrap();
    let generation = s.copie.attente[0].reservations[&cle];
    let d = Depot::nouveau(&central);
    {
        let mut a = d.proteger().unwrap();
        let mut e = a.lire().unwrap().unwrap();
        a.reserver(&mut e, &[cle.clone()], "b", true, false)
            .unwrap();
        assert!(e.baux[&cle].generation > generation);
    }
    s.synchroniser(None, false).unwrap();
    attendre(&mut s);
    assert_eq!(s.copie.attente.len(), 1);
    assert!(!s.copie.conflits.is_empty());
    assert_eq!(s.copie.base.revision, 0);
    assert_eq!(s.copie.base.baux[&cle].proprietaire, "b");
}

#[test]
fn annulation_textuelle_locale_restauree_apres_redemarrage() {
    let mut p = exemple();
    texte_annotation(&mut p, "texte-durable", "Initial");
    let local = dossier();
    let central = dossier();
    let mut a = session::Session::ouvrir(&p, local.join("a.json"), central.clone()).unwrap();
    a.synchroniser(None, false).unwrap();
    let pa = attendre(&mut a);
    let mut q = pa.clone();
    if let super::super::documentation::AnnotationForme::Texte { texte, .. } = &mut q
        .annotations
        .iter_mut()
        .find(|a| a.id == "texte-durable")
        .unwrap()
        .forme
    {
        *texte = "Initial A".into();
    }
    a.editer(&pa, &q).unwrap();
    a.synchroniser(None, false).unwrap();
    attendre(&mut a);
    let mut b = session::Session::ouvrir(&p, local.join("b.json"), central.clone()).unwrap();
    b.synchroniser(None, false).unwrap();
    let pb = attendre(&mut b);
    let mut q = pb.clone();
    if let super::super::documentation::AnnotationForme::Texte { texte, .. } = &mut q
        .annotations
        .iter_mut()
        .find(|a| a.id == "texte-durable")
        .unwrap()
        .forme
    {
        *texte = "B Initial A".into();
    }
    b.editer(&pb, &q).unwrap();
    b.synchroniser(None, false).unwrap();
    attendre(&mut b);
    a.synchroniser(None, false).unwrap();
    attendre(&mut a);
    drop(a);
    let mut a = session::Session::ouvrir(&p, local.join("a.json"), central).unwrap();
    let q = a.annuler(false).unwrap();
    let contenu = match &q
        .annotations
        .iter()
        .find(|a| a.id == "texte-durable")
        .unwrap()
        .forme
    {
        super::super::documentation::AnnotationForme::Texte { texte, .. } => texte,
        _ => panic!(),
    };
    assert_eq!(contenu, "B Initial");
    let q = a.annuler(true).unwrap();
    assert!(
        matches!(&q.annotations.iter().find(|a|a.id=="texte-durable").unwrap().forme,super::super::documentation::AnnotationForme::Texte{texte,..} if texte=="B Initial A")
    );
}

#[test]
fn parent_futur_et_ressource_requise_absente_ne_sont_pas_acceptes() {
    let p = exemple();
    let mut q = p.clone();
    q.vues[0].nom = "Attente ressource".into();
    let mut t = transaction(&p, &q);
    let e = Etat::initial(&p).unwrap();
    t.base = 99;
    assert!(e.accepter(&t).is_err());
    t.base = 0;
    t.ressources.insert(empreinte(b"ressource manquante"));
    let d = Depot::nouveau(dossier());
    d.preparer().unwrap();
    let mut a = d.proteger().unwrap();
    let mut e = a.initialiser(&p).unwrap();
    assert!(a.publier(&mut e, &t).is_err());
    assert_eq!(e.revision, 0);
    assert_eq!(a.lire().unwrap().unwrap().revision, 0);
}

#[test]
fn murs_distincts_crees_et_deplaces_hors_connexion_fusionnent_sans_verrou_global() {
    let mut p = exemple();
    p.figer_references();
    let n = p.niveaux[0].id.clone();
    let options = super::super::edition::ParametresMur {
        niveau: Some(n),
        implantation: super::super::edition::Implantation::Exterieure,
        ..Default::default()
    };
    let mut a = p.clone();
    let ia = a
        .ajouter_mur([20000.0, 0.0], [24000.0, 0.0], &options)
        .unwrap();
    let mut b = p.clone();
    let ib = b
        .ajouter_mur([20000.0, 20000.0], [24000.0, 20000.0], &options)
        .unwrap();
    let mut e = Etat::initial(&p).unwrap();
    accepter(&mut e, Transaction::entre(&p, &a, "a", 0).unwrap());
    accepter(&mut e, Transaction::entre(&p, &b, "b", 0).unwrap());
    let r = e.projection().unwrap();
    assert!(r.murs.iter().any(|m| m.id == ia));
    assert!(r.murs.iter().any(|m| m.id == ib));
    assert!(r.implantations.contains_key(&ia) && r.implantations.contains_key(&ib));
    assert!(!e.baux.contains_key("@/implantations"));
    let mut a = r.clone();
    let mut b = r.clone();
    a.deplacer_extremite(&ia, true, [25000.0, 0.0]).unwrap();
    b.deplacer_extremite(&ib, true, [25000.0, 20000.0]).unwrap();
    accepter(&mut e, Transaction::entre(&r, &a, "a", 2).unwrap());
    accepter(&mut e, Transaction::entre(&r, &b, "b", 2).unwrap());
}

#[test]
fn relations_de_tous_les_objets_sont_independantes_par_identite() {
    for nom in [
        "implantations",
        "contextes",
        "priorites_jonction",
        "attaches_toitures",
        "cartouches_feuilles",
    ] {
        let cle = format!("@/{nom}");
        let mut o = Objets::new();
        o.insert(cle.clone(), serde_json::json!({"a":0,"b":0}));
        let ca = concurrence::decomposer(vec![Changement {
            cle: cle.clone(),
            avant: o.get(&cle).cloned(),
            apres: Some(serde_json::json!({"a":1,"b":0})),
        }]);
        let cb = concurrence::decomposer(vec![Changement {
            cle: cle.clone(),
            avant: o.get(&cle).cloned(),
            apres: Some(serde_json::json!({"a":0,"b":2})),
        }]);
        for c in ca.iter().chain(&cb) {
            assert_eq!(concurrence::valeur(&o, &c.cle), c.avant.as_ref());
            concurrence::affecter(&mut o, &c.cle, c.apres.clone());
        }
        assert_eq!(o[&cle], serde_json::json!({"a":1,"b":2}));
    }
}

#[test]
fn session_publique_puis_autre_session_cree_un_mur_sans_prise_globale() {
    let p = exemple();
    let central = dossier();
    let local = dossier();
    let mut a = session::Session::ouvrir(&p, local.join("a.json"), central.clone()).unwrap();
    let mut b = session::Session::ouvrir(&p, local.join("b.json"), central).unwrap();
    a.synchroniser(None, false).unwrap();
    let pa = attendre(&mut a);
    b.synchroniser(None, false).unwrap();
    let pb = attendre(&mut b);
    let mut qa = pa.clone();
    qa.ajouter_mur([20000.0, 0.0], [24000.0, 0.0], &Default::default())
        .unwrap();
    a.editer(&pa, &qa).unwrap();
    a.synchroniser(None, false).unwrap();
    attendre(&mut a);
    let mut qb = pb.clone();
    qb.ajouter_mur([20000.0, 20000.0], [24000.0, 20000.0], &Default::default())
        .unwrap();
    b.editer(&pb, &qb).unwrap();
    b.synchroniser(None, false).unwrap();
    let r = attendre(&mut b);
    assert!(b.copie.conflits.is_empty(), "{:?}", b.copie.conflits);
    assert!(b.copie.attente.is_empty());
    assert_eq!(r.murs.len(), p.murs.len() + 2);
    assert!(b.copie.base.baux.values().all(|b| !b.actif));
}

#[test]
fn voisinage_geometrique_et_meme_objet_restent_conflictuels() {
    let p = exemple();
    let mut a = p.clone();
    let mut b = p.clone();
    a.ajouter_mur([20000.0, 0.0], [24000.0, 0.0], &Default::default())
        .unwrap();
    b.ajouter_mur([22000.0, -1000.0], [22000.0, 1000.0], &Default::default())
        .unwrap();
    let mut e = Etat::initial(&p).unwrap();
    accepter(&mut e, transaction(&p, &a));
    let mut t = transaction(&p, &b);
    autoriser(&mut e, &mut t);
    assert!(e.accepter(&t).is_err());
}

#[test]
fn premiere_synchro_de_plusieurs_murs_preserve_les_deux_copies_distinctes() {
    let mut p = Projet::depuis_gabarit("Première synchronisation", &crate::gabarit::Gabarit::defaut());
    p.ajouter_mur([30000.0, 30000.0], [34000.0, 30000.0], &Default::default()).unwrap();
    let root = dossier();
    let entree = root.join("projet.bimpartage");
    partage::creer(&p, &entree).unwrap();
    let (mut a, _, mut sa) = partage::ouvrir(&entree, &root.join("a")).unwrap();
    let (mut b, _, mut sb) = partage::ouvrir(&entree, &root.join("b")).unwrap();
    let options = super::super::edition::ParametresMur {epaisseur:300.0,..Default::default()};
    // Le rectangle englobant du mur diagonal atteint celui du mur vertical,
    // alors que leurs bandes de jonction sont séparées de plus de deux mètres.
    for (debut,fin) in [([-4000.0,-500.0],[0.0,-2500.0]),([-6000.0,-5000.0],[-2000.0,-5000.0]),([2000.0,-5000.0],[6000.0,-5000.0])] {
        let mut q=a.clone();q.ajouter_mur(debut,fin,&options).unwrap();sa.editer(&a,&q).unwrap();a=q;
    }
    for (debut,fin) in [([0.0,5000.0],[0.0,0.0]),([0.0,0.0],[4000.0,0.0]),([4000.0,0.0],[4000.0,5000.0])] {
        let mut q=b.clone();q.ajouter_mur(debut,fin,&options).unwrap();sb.editer(&b,&q).unwrap();b=q;
    }
    let id=b.murs.last().unwrap().id.clone();
    for dx in [50.0,100.0] {
        let (q,_)=b.executer(super::super::commandes::Commande::Deplacer {ids:vec![id.clone()],delta:[dx,0.0,0.0]}).unwrap();
        sb.editer(&b,&q).unwrap();b=q;
    }
    sa.synchroniser(None,false).unwrap();attendre(&mut sa);
    sb.synchroniser(None,false).unwrap();let resultat=attendre(&mut sb);
    assert!(sb.copie.conflits.is_empty(),"{:?}",sb.copie.conflits);
    assert!(sb.copie.attente.is_empty());
    assert_eq!(resultat.murs.len(),p.murs.len()+6);
    assert!(a.murs.iter().chain(&b.murs).all(|m|resultat.murs.iter().any(|r|r.id==m.id)));
    sa.synchroniser(None,false).unwrap();let autre=attendre(&mut sa);
    assert_eq!(objets(&autre).unwrap(),objets(&resultat).unwrap());
}

#[test]
fn conflit_reel_preserve_les_murs_locaux_a_lecran_et_apres_reouverture() {
    let p=Projet::depuis_gabarit("Conflit local",&crate::gabarit::Gabarit::defaut());
    let root=dossier();let entree=root.join("projet.bimpartage");
    partage::creer(&p,&entree).unwrap();
    let (a,_,mut sa)=partage::ouvrir(&entree,&root.join("a")).unwrap();
    let (b,local,mut sb)=partage::ouvrir(&entree,&root.join("b")).unwrap();
    let mut qa=a.clone();qa.ajouter_mur([0.0,0.0],[4000.0,0.0],&Default::default()).unwrap();
    let mut qb=b.clone();let id=qb.ajouter_mur([2000.0,-2000.0],[2000.0,2000.0],&Default::default()).unwrap();
    sa.editer(&a,&qa).unwrap();sb.editer(&b,&qb).unwrap();
    let mut suite=qb.clone();let id2=suite.ajouter_mur([10000.0,10000.0],[14000.0,10000.0],&Default::default()).unwrap();
    sb.editer(&qb,&suite).unwrap();
    sa.synchroniser(None,false).unwrap();attendre(&mut sa);
    sb.synchroniser(None,false).unwrap();attendre(&mut sb);
    assert!(!sb.copie.conflits.is_empty());
    for id in [&id,&id2] {
        assert!(sb.affichage().unwrap().murs.iter().any(|m|&m.id==id));
        assert!(!sb.travail().unwrap().murs.iter().any(|m|&m.id==id));
    }
    assert!(sb.editer(&suite,&suite).is_err());
    super::super::enregistrement::enregistrer_natif(&sb.affichage().unwrap(),&local).unwrap();
    drop(sb);
    let (visible,_,s)=partage::ouvrir(&entree,&root.join("b")).unwrap();
    assert!(s.copie.apercu_conflit.is_some());
    assert!(visible.murs.iter().any(|m|m.id==id));
    assert!(visible.murs.iter().any(|m|m.id==id2));
    assert!(s.copie.base.projection().unwrap().murs.iter().all(|m|m.id!=id&&m.id!=id2));
}

#[test]
fn historique_spatial_protocole_deux_reste_verifiable_apres_mise_a_jour() {
    let mut p=exemple();p.figer_references();
    let mut base=Etat::initial(&p).unwrap();base.protocole=2;
    let d=Depot::nouveau(dossier());d.preparer().unwrap();
    let mut a=d.proteger().unwrap();a.initialiser_etat(&base).unwrap();
    let mut e=a.lire().unwrap().unwrap();
    let mut q=p.clone();q.ajouter_mur([20000.0,0.0],[24000.0,0.0],&Default::default()).unwrap();
    let mut ancien=transaction(&p,&q);ancien.protocole=2;
    ancien.lectures.retain(|k,_|!k.starts_with("#voisinage3/"));
    concurrence::ajouter_spatial(&mut ancien,&objets(&p).unwrap());
    assert!(ancien.lectures.keys().any(|k|k.starts_with("#zones/")));
    let cles:Vec<_>=ancien.cles_reservation(&e.objets).into_iter().collect();
    a.reserver(&mut e,&cles,&ancien.session,false,false).unwrap();
    for k in cles {ancien.reservations.insert(k.clone(),e.baux[&k].generation);}
    assert!(a.publier(&mut e,&ancien).unwrap().is_empty());
    assert_eq!(e.protocole,2);
    let ancienne_revision=hash(&e);drop(a);
    let mut a=d.proteger().unwrap();let mut e=a.lire().unwrap().unwrap();
    assert_eq!(hash(&e),ancienne_revision);
    let p=e.projection().unwrap();let mut q=p.clone();q.vues[0].nom="Après mise à jour".into();
    let mut nouvelle=transaction(&p,&q);nouvelle.base=e.revision;
    assert!(a.publier(&mut e,&nouvelle).unwrap().is_empty());
    assert_eq!(e.protocole,3);drop(a);
    assert_eq!(d.proteger().unwrap().lire().unwrap().unwrap().protocole,3);
}

#[test]
fn creations_independantes_barres_escaliers_gardes_terrains_importes_et_types() {
    use super::super::*;
    let mut p = exemple();
    p.figer_references();
    let niveau = p.niveaux[0].id.clone();
    let mut multi = p.types_murs[0].clone();
    multi.id = "multi-essai".into();
    multi.couches = vec![
        Couche {
            epaisseur: 100.0,
            ..Default::default()
        },
        Couche {
            epaisseur: 200.0,
            ..Default::default()
        },
    ];
    p.types_murs.push(multi);
    for genre in [
        "barre",
        "escalier",
        "garde_corps",
        "terrain",
        "importe",
        "niveau",
        "type",
        "assemblage",
        "annotation",
        "vue",
        "feuille",
        "nomenclature",
        "materiau",
        "motif",
        "cartouche",
    ] {
        let mut propositions = Vec::new();
        for (poste, y) in [("a", 20000.0), ("b", 40000.0)] {
            let mut q = p.clone();
            let id = uuid();
            match genre {
                "assemblage" => {
                    q.ajouter_mur(
                        [20000.0, y],
                        [25000.0, y],
                        &edition::ParametresMur {
                            type_mur: Some("multi-essai".into()),
                            implantation: edition::Implantation::Exterieure,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                }
                "annotation" => q.annotations.push(documentation::Annotation {
                    id,
                    vue: p.vues[0].id.clone(),
                    hauteur: 2.5,
                    forme: documentation::AnnotationForme::Ligne {
                        debut: [20000.0, y],
                        fin: [25000.0, y],
                    },
                }),
                "vue" => {
                    let mut v = p.vues[0].clone();
                    v.id = id;
                    v.nom = poste.into();
                    q.vues.push(v);
                }
                "feuille" => {
                    let mut v = p.feuilles[0].clone();
                    v.id = id;
                    v.numero = poste.into();
                    q.feuilles.push(v);
                }
                "nomenclature" => {
                    let mut v = p.nomenclatures[0].clone();
                    v.id = id;
                    v.nom = poste.into();
                    q.nomenclatures.push(v);
                }
                "materiau" => {
                    let mut v = p.materiaux[0].clone();
                    v.id = id;
                    v.nom = poste.into();
                    q.materiaux.push(v);
                }
                "motif" => {
                    let mut v = p.motifs[0].clone();
                    v.id = id;
                    v.nom = poste.into();
                    q.motifs.push(v);
                }
                "cartouche" => {
                    let mut v = documentation::FamilleCartouche::default();
                    v.id = id;
                    v.nom = poste.into();
                    q.familles_cartouches.push(v);
                }
                "barre" => q.barres.push(Barre {
                    id,
                    nom: poste.into(),
                    genre: GenreBarre::Poutre,
                    niveau: niveau.clone(),
                    debut: [20000.0, y, 0.0],
                    fin: [25000.0, y, 0.0],
                    largeur: 200.0,
                    hauteur: 300.0,
                    rotation: 0.0,
                }),
                "escalier" => q.escaliers.push(Escalier {
                    id,
                    nom: poste.into(),
                    niveau: niveau.clone(),
                    niveau_haut: None,
                    decalage_bas: 0.0,
                    decalage_haut: 0.0,
                    hauteur: 2800.0,
                    trace: vec![[20000.0, y], [25000.0, y]],
                    largeur: 1000.0,
                    hauteur_marche: 175.0,
                    paillasse: 150.0,
                }),
                "garde_corps" => q.garde_corps.push(GardeCorps {
                    id,
                    nom: poste.into(),
                    niveau: niveau.clone(),
                    decalage: 0.0,
                    trace: vec![[20000.0, y], [25000.0, y]],
                    hauteur: 1000.0,
                    remplissage: Remplissage::Plein,
                    escalier: None,
                }),
                "terrain" => q.terrains.push(site::Terrain {
                    id,
                    nom: poste.into(),
                    contour: vec![
                        [20000.0, y, 0.0],
                        [25000.0, y, 0.0],
                        [25000.0, y + 5000.0, 0.0],
                        [20000.0, y + 5000.0, 0.0],
                    ],
                    points: vec![],
                    type_terrain: None,
                }),
                "importe" => q.importes.push(ElementImporte {
                    id,
                    nom: poste.into(),
                    categorie: "Divers".into(),
                    niveau: Some(niveau.clone()),
                    maillage: Maillage {
                        sommets: vec![
                            [20000.0, y, 0.0],
                            [25000.0, y, 0.0],
                            [25000.0, y + 5000.0, 0.0],
                        ],
                        triangles: vec![[0, 1, 2]],
                    },
                    raison: "Essai de concurrence".into(),
                    origine: None,
                }),
                "niveau" => q.niveaux.push(Niveau {
                    id,
                    nom: poste.into(),
                    altitude: y,
                    origine: None,
                }),
                "type" => {
                    let mut ty = q.types_murs[0].clone();
                    ty.id = id;
                    ty.nom = poste.into();
                    q.types_murs.push(ty);
                }
                _ => unreachable!(),
            }
            q.verifier().unwrap();
            propositions.push(Transaction::entre(&p, &q, poste, 0).unwrap());
        }
        let mut e = Etat::initial(&p).unwrap();
        for mut t in propositions {
            autoriser(&mut e, &mut t);
            e = e.accepter(&t).unwrap_or_else(|c| panic!("{genre} : {c:?}"));
        }
        assert_eq!(e.revision, 2);
    }
}

#[test]
fn ouverture_sur_deux_hotes_distincts_et_dalles_independantes() {
    use super::super::*;
    let mut p = exemple();
    let h1 = p
        .ajouter_mur([20000.0, 0.0], [25000.0, 0.0], &Default::default())
        .unwrap();
    let h2 = p
        .ajouter_mur([20000.0, 20000.0], [25000.0, 20000.0], &Default::default())
        .unwrap();
    let mut e = Etat::initial(&p).unwrap();
    for (host, poste) in [(h1, "a"), (h2, "b")] {
        let (q, _) = p
            .executer(commandes::Commande::Ouverture {
                mur: host,
                genre: GenreOuverture::Porte,
                type_ouverture: None,
                abscisse: 1500.0,
                largeur: 900.0,
                hauteur: 2100.0,
                allege: 0.0,
            })
            .unwrap();
        accepter(&mut e, Transaction::entre(&p, &q, poste, 0).unwrap());
    }
    let p = exemple();
    let mut e = Etat::initial(&p).unwrap();
    for (poste, y) in [("a", 20000.0), ("b", 40000.0)] {
        let mut q = p.clone();
        let mut ty = p.types_dalles[0].clone();
        ty.id = String::new();
        let opt = edition::ParametresDalle {
            sorte: SorteDalle::Dalle,
            type_dalle: ty,
            pente: None,
            niveau: Some(p.niveaux[0].id.clone()),
            decalage: 0.0,
            phase: None,
            sous_projet: None,
            variante: None,
        };
        q.ajouter_dalle_contours(
            &[vec![
                [20000.0, y],
                [25000.0, y],
                [25000.0, y + 5000.0],
                [20000.0, y + 5000.0],
            ]],
            &opt,
        )
        .unwrap();
        accepter(&mut e, Transaction::entre(&p, &q, poste, 0).unwrap());
    }
    assert_eq!(e.revision, 2);
}

#[test]
fn journal_protocole_un_reste_lisible_et_nouvelles_acceptations_passent_en_trois() {
    let p = exemple();
    let mut legacy = Etat::initial(&p).unwrap();
    legacy.protocole = 1;
    let d = Depot::nouveau(dossier());
    d.preparer().unwrap();
    {
        let mut a = d.proteger().unwrap();
        a.initialiser_etat(&legacy).unwrap();
    }
    let mut a = d.proteger().unwrap();
    let mut e = a.lire().unwrap().unwrap();
    assert_eq!(e.protocole, 1);
    let mut q = p.clone();
    q.vues[0].nom = "Nouveau protocole".into();
    a.publier(&mut e, &transaction(&p, &q)).unwrap();
    assert_eq!(e.protocole, 3);
    drop(a);
    assert_eq!(d.proteger().unwrap().lire().unwrap().unwrap().protocole, 3);
}

#[test]
fn depot_central_disparu_n_est_pas_recree_en_silence() {
    let p = exemple();
    let central = dossier();
    let local = dossier();
    let mut a = session::Session::ouvrir(&p, local.join("a.json"), central.clone()).unwrap();
    a.synchroniser(None, false).unwrap();
    attendre(&mut a);
    assert!(a.copie.central_connu);
    std::fs::remove_dir_all(&central).unwrap();
    a.synchroniser(None, false).unwrap();
    while a.synchronisation_en_cours() {
        a.recevoir().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(a.etat.starts_with("Hors connexion"), "{}", a.etat);
    assert!(!central.exists(), "dépôt recréé");
}

#[test]
fn abandon_refuse_pendant_synchro_et_retire_de_l_annulation() {
    let p = exemple();
    let central = dossier();
    let local = dossier();
    let mut a = session::Session::ouvrir(&p, local.join("a.json"), central.clone()).unwrap();
    let mut b = session::Session::ouvrir(&p, local.join("b.json"), central).unwrap();
    a.synchroniser(None, false).unwrap();
    attendre(&mut a);
    b.synchroniser(None, false).unwrap();
    attendre(&mut b);
    let mut pa = p.clone();
    pa.vues[0].nom = "A".into();
    a.editer(&p, &pa).unwrap();
    let mut pb = p.clone();
    pb.vues[0].nom = "B".into();
    b.editer(&p, &pb).unwrap();
    b.synchroniser(None, false).unwrap();
    attendre(&mut b);
    a.synchroniser(None, false).unwrap();
    let id = a.copie.attente[0].id.clone();
    assert!(a.abandonner(&id).is_err(), "abandon accepté pendant la synchro");
    assert!(a.resoudre_locale(&id).is_err(), "résolution acceptée pendant la synchro");
    attendre(&mut a);
    assert!(a.copie.conflits.contains_key(&id));
    let r = a.abandonner(&id).unwrap();
    assert_eq!(r.vues[0].nom, "B");
    assert!(a.copie.annulations.iter().all(|t| t.id != id));
}
