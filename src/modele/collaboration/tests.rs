use super::*;
use super::stockage::*;
use super::texte::*;
use std::path::PathBuf;

fn dossier()->PathBuf{let p=std::env::temp_dir().join(format!("bimedit-collab-{}",uuid()));std::fs::create_dir_all(&p).unwrap();p}
fn exemple()->Projet{
    let mut p=Projet::lire_json(include_str!("../../../samples/maison_native.modele.json")).unwrap();
    // La fixture n'est jamais modifiée sur disque.
    p.identite=uuid();p.verifier().unwrap();p
}
fn autoriser(e:&mut Etat,t:&mut Transaction){for c in &t.changements{if sensible(&c.cle){e.baux.insert(c.cle.clone(),Bail{proprietaire:t.session.clone(),generation:1,actif:true});t.reservations.insert(c.cle.clone(),1);}}}
fn accepter(e:&mut Etat,mut t:Transaction){autoriser(e,&mut t);*e=e.accepter(&t).unwrap();}
fn transaction(a:&Projet,b:&Projet)->Transaction{Transaction::entre(a,b,"poste-a",0).unwrap()}
fn texte_annotation(p:&mut Projet, id:&str,s:&str){p.annotations.push(super::super::documentation::Annotation{id:id.into(),vue:p.vues[0].id.clone(),hauteur:2.5,forme:super::super::documentation::AnnotationForme::Texte{point:[0.0,0.0],texte:s.into()}});}

#[test]
fn identites_distribuees_et_migration_preserve_les_objets(){
    use super::super::commandes::Commande;
    let p=exemple();let a=p.executer(Commande::Niveau{nom:"A".into(),altitude:6000.0}).unwrap();let b=p.executer(Commande::Niveau{nom:"B".into(),altitude:9000.0}).unwrap();assert_ne!(a.1,b.1);
    let ancien=include_str!("../../../samples/maison_native.modele.json");let a=Projet::lire_json(ancien).unwrap();let b=Projet::lire_json(ancien).unwrap();assert_eq!(a.identite,b.identite);assert_eq!(a.version,1);
    let original:Value=serde_json::from_str(ancien).unwrap();assert_eq!(a.murs[0].id,original["murs"][0]["id"]);assert_eq!(a.identite,Projet::lire_json(&a.json().unwrap()).unwrap().identite);
}
#[test]
fn objets_independants_et_fusion_explicite_deux_champs(){
    let p=exemple();let mut a=p.clone();let mut b=p.clone();
    a.vues[0].nom="Architecte".into();b.vues[0].type_vue="Coordination".into();
    let mut e=Etat::initial(&p).unwrap();accepter(&mut e,transaction(&p,&a));accepter(&mut e,transaction(&p,&b));
    let r=e.projection().unwrap();assert_eq!(r.vues[0].nom,"Architecte");assert_eq!(r.vues[0].type_vue,"Coordination");
    let mut c=p.clone();c.quadrillages.push(super::super::documentation::Quadrillage{id:uuid(),nom:"Axe".into(),debut:[0.0,0.0],fin:[500.0,0.0]});accepter(&mut e,transaction(&p,&c));assert!(!e.projection().unwrap().quadrillages.is_empty());
}
#[test]
fn meme_champ_conserve_trois_valeurs_et_pas_de_fusion_geometrique(){
    let p=exemple();let mut a=p.clone();let mut b=p.clone();a.vues[0].nom="A".into();b.vues[0].nom="B".into();
    let mut e=Etat::initial(&p).unwrap();accepter(&mut e,transaction(&p,&a));let erreur=e.accepter(&transaction(&p,&b)).unwrap_err();assert!(erreur.iter().any(|c|c.base.is_some()&&c.locale.is_some()&&c.acceptee.is_some()));
    b=p.clone();b.vues[0].echelle=200.0;assert!(e.accepter(&transaction(&p,&b)).is_err());
}
#[test]
fn suppression_hote_observe_ajout_ouverture_et_collection(){
    let p=exemple();let mut sup=p.clone();let hote=p.murs[0].id.clone();sup.murs.retain(|m|m.id!=hote);sup.ouvertures.retain(|o|o.mur!=hote);sup.contextes.remove(&hote);
    // La précondition dérivée est testée indépendamment de la validation finale.
    let t=Transaction::entre(&p,&sup,"a",0);if let Ok(t)=t {
        let distant=p.clone();let mur=distant.murs[0].id.clone();
        let cmd=super::super::commandes::Commande::Ouverture{mur,genre:super::super::GenreOuverture::Porte,type_ouverture:None,abscisse:1000.0,largeur:600.0,hauteur:2000.0,allege:0.0};
        let distant=distant.executer(cmd).unwrap().0;let e=Etat::initial(&distant).unwrap();
        assert!(appliquer(&t,&e.objets,1,&p.identite).unwrap_err().iter().any(|c|c.cle.starts_with('#')));
    }else{ // La fixture peut porter une cote ancrée : la suppression est déjà refusée.
        assert!(sup.verifier().is_err());
    }
}
#[test]
fn reservation_generation_et_compensation_refusent_ancien_detenteur(){
    let p=exemple();let d=Depot::nouveau(dossier());d.preparer().unwrap();let mut a=d.proteger().unwrap();let mut e=a.initialiser(&p).unwrap();let key=format!("murs/{}",p.murs[0].id);
    a.reserver(&mut e,&[key.clone()],"a",false,false).unwrap();let generation=e.baux[&key].generation;
    a.reserver(&mut e,&[key.clone()],"b",true,false).unwrap();assert!(e.baux[&key].generation>generation);
    let mut change=p.clone();change.murs[0].nom="Local".into();let mut t=Transaction::entre(&p,&change,"a",0).unwrap();t.reservations.insert(key.clone(),generation);
    assert!(!a.publier(&mut e,&t).unwrap().is_empty());
    let mut local=p.clone();local.vues[0].nom="A".into();let t=transaction(&p,&local);accepter(&mut e,t.clone());let mut collegue=local.clone();collegue.vues[0].nom="B".into();accepter(&mut e,Transaction::entre(&local,&collegue,"b",1).unwrap());assert!(e.accepter(&t.inverse(e.revision)).is_err());
}
#[test]
fn yrs_converge_desordre_doublons_unicode_et_annulation_locale(){
    let mut a=Editeur::ouvrir(&EtatTexte::default(),"a").unwrap();let u=a.remplacer("été 🏠");let mut etat=EtatTexte::default();etat.recevoir(&u).unwrap();
    let mut b=Editeur::ouvrir(&etat,"b").unwrap();let ua=a.remplacer("été 🏠 A");let ub=b.remplacer("B été 🏠");a.recevoir(&ub).unwrap();b.recevoir(&ua).unwrap();a.recevoir(&ub).unwrap();assert_eq!(a.contenu(),b.contenu());
    let annule=a.annuler(false).unwrap();b.recevoir(&annule).unwrap();assert_eq!(a.contenu(),b.contenu());assert!(a.contenu().contains('B'));assert!(!a.contenu().contains('A'));
    let mut restaure=EtatTexte::default();for u in [&ub,&ua,&u,&annule,&ub]{restaure.recevoir(u).unwrap();}assert_eq!(restaure.contenu().unwrap(),a.contenu());
}
#[test]
fn texte_avant_creation_et_apres_suppression_sans_resurrection(){
    let mut p=exemple();let mut e=Etat::initial(&p).unwrap();let id=uuid();let mut edit=Editeur::ouvrir(&EtatTexte::default(),"a").unwrap();let u=edit.remplacer("initial");
    let mut t=transaction(&p,&p);t.textes.insert(id.clone(),u.clone());accepter(&mut e,t);assert!(!e.projection().unwrap().annotations.iter().any(|a|a.id==id));
    texte_annotation(&mut p,&id,"projection ignorée");let mut t=transaction(&e.projection().unwrap(),&p);t.textes.insert(id.clone(),u);accepter(&mut e,t);assert_eq!(e.textes[&id].contenu().unwrap(),"initial");
    let avant=e.projection().unwrap();let mut apres=avant.clone();apres.annotations.retain(|a|a.id!=id);accepter(&mut e,transaction(&avant,&apres));
    let mut tardif=transaction(&apres,&apres);tardif.textes.insert(id.clone(),edit.remplacer("texte tardif récupérable"));accepter(&mut e,tardif);
    assert!(e.textes[&id].supprime);assert!(e.textes[&id].contenu().unwrap().contains("récupérable"));assert!(!e.projection().unwrap().annotations.iter().any(|a|a.id==id));
}
#[test]
fn protocole_validation_et_atomicite(){
    let p=exemple();let mut a=p.clone();a.vues[0].nom="A".into();let mut t=transaction(&p,&a);let e=Etat::initial(&p).unwrap();t.protocole=999;assert!(e.accepter(&t).is_err());t.protocole=PROTOCOLE;t.projet=uuid();assert!(e.accepter(&t).is_err());
    let avant=hash(&e);t.projet=p.identite.clone();t.changements.push(Changement{cle:"@/orientation_nord".into(),avant:Some(Value::from(0.0)),apres:Some(Value::String("invalide".into()))});assert!(e.accepter(&t).is_err());assert_eq!(hash(&e),avant);
}
#[test]
fn registre_queue_interrompue_corruption_et_instantane_crdt(){
    let mut p=exemple();texte_annotation(&mut p,"texte-snapshot","Bonjour");let root=dossier();let d=Depot::nouveau(&root);d.preparer().unwrap();
    let attendu;{
        let mut a=d.proteger().unwrap();let mut e=a.initialiser(&p).unwrap();let mut change=p.clone();change.vues[0].nom="accepté".into();let t=transaction(&p,&change);assert!(a.publier(&mut e,&t).unwrap().is_empty());assert!(a.publier(&mut e,&t).unwrap().is_empty());a.instantane(&e).unwrap();attendu=hash(&e);
    }
    use std::io::Write;let controle=root.join("controle");let len=std::fs::metadata(&controle).unwrap().len();std::fs::OpenOptions::new().append(true).open(&controle).unwrap().write_all(&[12,0,0]).unwrap();
    {let mut a=d.proteger().unwrap();let e=a.lire().unwrap().unwrap();assert_eq!(hash(&e),attendu);assert_eq!(e.textes["texte-snapshot"].contenu().unwrap(),"Bonjour");}
    assert_eq!(std::fs::metadata(&controle).unwrap().len(),len);
    let mut bytes=std::fs::read(&controle).unwrap();bytes[10]^=1;std::fs::write(&controle,bytes).unwrap();assert!(d.proteger().unwrap().lire().unwrap_err().contains("Corruption"));
}
#[test]
fn ressources_immutables_taille_empreinte_et_absence(){
    let d=Depot::nouveau(dossier());d.preparer().unwrap();let v=serde_json::json!({"maillage":{"sommets":[1,2,3],"faces":[0,1,2]}});let id=d.sauver("paquets",&v).unwrap();let disque=String::from_utf8(d.lire("paquets",&id).unwrap()).unwrap();assert!(disque.contains("$ressource"));assert_eq!(d.charger::<Value>("paquets",&id).unwrap(),v);
    let ressources:Vec<_>=std::fs::read_dir(d.racine.join("ressources")).unwrap().map(|x|x.unwrap().path()).collect();std::fs::remove_file(&ressources[0]).unwrap();assert!(d.charger::<Value>("paquets",&id).is_err());
}

// Test exécuté dans de vrais processus. L'environnement est fourni par le parent.
#[test]
fn enfant_reservation(){
    let Ok(root)=std::env::var("BIMEDIT_TEST_DEPOT")else{return;};let nom=std::env::var("BIMEDIT_TEST_POSTE").unwrap();let d=Depot::nouveau(&root);
    let limite=std::time::Instant::now()+std::time::Duration::from_secs(15);
    while !std::path::Path::new(&root).join("depart").exists(){assert!(std::time::Instant::now()<limite);std::thread::sleep(std::time::Duration::from_millis(5));}
    loop {if let Ok(mut a)=d.proteger(){let mut e=a.lire().unwrap().unwrap();let resultat=a.reserver(&mut e,&["groupe-test".into()],&nom,false,false);std::fs::write(std::path::Path::new(&root).join(format!("resultat-{nom}")),if resultat.is_ok(){"gagnant"}else{"refusé"}).unwrap();break;}assert!(std::time::Instant::now()<limite);std::thread::sleep(std::time::Duration::from_millis(5));}
}
#[test]
fn course_entre_deux_processus_et_controle_non_remplacable(){
    let root=dossier();let d=Depot::nouveau(&root);d.preparer().unwrap();{d.proteger().unwrap().initialiser(&exemple()).unwrap();}
    let exe=std::env::current_exe().unwrap();let mut enfants=Vec::new();for poste in ["a","b"]{enfants.push(std::process::Command::new(&exe).args(["--exact","modele::collaboration::tests::enfant_reservation","--nocapture"]).env("BIMEDIT_TEST_DEPOT",&root).env("BIMEDIT_TEST_POSTE",poste).spawn().unwrap());}
    std::fs::write(root.join("depart"),b"go").unwrap();for e in &mut enfants{assert!(e.wait().unwrap().success());}
    let a=std::fs::read_to_string(root.join("resultat-a")).unwrap();let b=std::fs::read_to_string(root.join("resultat-b")).unwrap();assert_ne!(a,b);
    let _protection=d.proteger().unwrap();assert!(d.proteger().is_err());
    #[cfg(windows)] {assert!(std::fs::remove_file(root.join("controle")).is_err());assert!(std::fs::rename(root.join("controle"),root.join("remplace")).is_err());}
}

#[test]
fn enfant_publication(){
    let Ok(root)=std::env::var("BIMEDIT_TEST_DEPOT")else{return;};let d=Depot::nouveau(&root);
    let mut a=loop{if let Ok(a)=d.proteger(){break a;}std::thread::sleep(std::time::Duration::from_millis(5));};
    let mut e=a.lire().unwrap().unwrap();
    if std::env::var_os("BIMEDIT_TEST_SNAPSHOT").is_some(){a.instantane(&e).unwrap();return;}
    let t:Transaction=serde_json::from_slice(&std::fs::read(d.racine.join("a-publier.json")).unwrap()).unwrap();
    assert!(a.publier(&mut e,&t).unwrap().is_empty());
}
#[test]
fn crash_processus_aux_frontieres_et_resultat_incertain(){
    let p=exemple();let mut change=p.clone();change.vues[0].nom="changement atomique".into();let t=transaction(&p,&change);
    let exe=std::env::current_exe().unwrap();
    for point in ["temporaire-cree","temporaire-ecrit","temporaire-durable","ressource-publiee","registre-longueur","registre-contenu","registre-empreinte","registre-durable"]{
        let root=dossier();let d=Depot::nouveau(&root);d.preparer().unwrap();{d.proteger().unwrap().initialiser(&p).unwrap();}
        std::fs::write(root.join("a-publier.json"),encoder(&t).unwrap()).unwrap();
        let statut=std::process::Command::new(&exe).args(["--exact","modele::collaboration::tests::enfant_publication"]).env("BIMEDIT_TEST_DEPOT",&root).env("BIMEDIT_TEST_CRASH",point).status().unwrap();assert_eq!(statut.code(),Some(86),"{point}");
        let mut a=d.proteger().unwrap();let mut e=a.lire().unwrap().unwrap();
        let accepte=e.acceptes.contains_key(&t.id);assert_eq!(e.revision,if accepte{1}else{0},"{point}");
        assert!(a.publier(&mut e,&t).unwrap().is_empty(),"{point}");assert_eq!(e.revision,1);assert_eq!(e.projection().unwrap().vues[0].nom,"changement atomique");
    }
}
#[test]
fn instantane_et_publication_dans_deux_processus(){
    let p=exemple();let mut change=p.clone();change.vues[0].nom="après instantané".into();let t=transaction(&p,&change);let root=dossier();let d=Depot::nouveau(&root);d.preparer().unwrap();{d.proteger().unwrap().initialiser(&p).unwrap();}
    std::fs::write(root.join("a-publier.json"),encoder(&t).unwrap()).unwrap();let exe=std::env::current_exe().unwrap();
    let mut publication=std::process::Command::new(&exe).args(["--exact","modele::collaboration::tests::enfant_publication"]).env("BIMEDIT_TEST_DEPOT",&root).spawn().unwrap();
    let mut instantane=std::process::Command::new(&exe).args(["--exact","modele::collaboration::tests::enfant_publication"]).env("BIMEDIT_TEST_DEPOT",&root).env("BIMEDIT_TEST_SNAPSHOT","1").spawn().unwrap();
    assert!(publication.wait().unwrap().success());assert!(instantane.wait().unwrap().success());assert_eq!(d.proteger().unwrap().lire().unwrap().unwrap().projection().unwrap().vues[0].nom,"après instantané");
}

fn attendre(s:&mut session::Session)->Projet{
    let fin=std::time::Instant::now()+std::time::Duration::from_secs(10);
    loop{if let Some(p)=s.recevoir().unwrap(){return p;}assert!(std::time::Instant::now()<fin,"{}",s.etat);std::thread::sleep(std::time::Duration::from_millis(5));}
}
#[test]
fn reconnexion_copies_locales_et_compensation_sans_ecraser_collegue(){
    let p=exemple();let root=dossier();let local=dossier();let mut a=session::Session::ouvrir(&p,local.join("a.json"),root.clone()).unwrap();let mut b=session::Session::ouvrir(&p,local.join("b.json"),root).unwrap();
    a.synchroniser(None,false).unwrap();attendre(&mut a);b.synchroniser(None,false).unwrap();attendre(&mut b);
    let mut pa=p.clone();pa.vues[0].nom="A".into();a.editer(&p,&pa).unwrap();
    let mut pb=p.clone();pb.vues[0].type_vue="Collègue".into();b.editer(&p,&pb).unwrap();b.synchroniser(None,false).unwrap();attendre(&mut b);
    // Redémarrage avec une proposition persistante hors connexion.
    let central=a.copie.central.clone();drop(a);let mut a=session::Session::ouvrir(&p,local.join("a.json"),central).unwrap();
    assert_eq!(a.copie.attente.len(),1);a.synchroniser(None,false).unwrap();let r=attendre(&mut a);assert_eq!(r.vues[0].nom,"A");assert_eq!(r.vues[0].type_vue,"Collègue");
    let annule=a.annuler(false).unwrap();assert_ne!(annule.vues[0].nom,"A");assert_eq!(annule.vues[0].type_vue,"Collègue");
}
