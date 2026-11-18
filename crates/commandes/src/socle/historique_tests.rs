use super::*;
use crate::socle::libelles::CATEGORIES_PROJET;
use crate::{Commande, Execution};

fn maison() -> Projet {
    Projet::lire_json(include_str!("../../../../samples/maison_native.modele.json")).unwrap()
}

/// Ancien calcul du libellé (lecture complète des deux états), pour comparaison.
fn libelle_complet(a: &Value, b: &Value) -> String {
    libelle_collections(a, b, &CATEGORIES_PROJET, "id", "Modifier le projet")
}

/// Pousse puis vérifie qu'annuler et rétablir rendent exactement les états.
fn aller_retour(h: &mut Historique, p: &Projet, q: &Projet) {
    h.pousser(p, q).unwrap();
    let r = h.cible(q, false).unwrap();
    h.confirmer(false).unwrap();
    assert_eq!(r.json().unwrap(), p.json().unwrap());
    let s = h.cible(&r, true).unwrap();
    h.confirmer(true).unwrap();
    assert_eq!(s.json().unwrap(), q.json().unwrap());
}

/// Libellé tiré de la différence identique au calcul sur les états complets.
#[test]
fn libelle_par_difference_identique() {
    let p = maison();
    let mur = p.murs[0].id.clone();
    let niveau = p.niveaux[0].id.clone();
    let etats = [
        p.executer(Commande::Supprimer { ids: vec![p.ouvertures[0].id.clone()] }).unwrap().0,
        p.executer(Commande::Deplacer { ids: vec![mur], delta: [100.0, 0.0, 0.0] }).unwrap().0,
        p.executer(Commande::ModifierNiveau { id: niveau, nom: "Rez".into(), altitude: 0.0 }).unwrap().0,
    ];
    let libelle = |a: &str, b: &str| libelle_projet(a, b, &calculer_texte(a, b).unwrap().unwrap());
    let complet = |a: &str, b: &str| libelle_complet(&serde_json::from_str(a).unwrap(), &serde_json::from_str(b).unwrap());
    let a = p.json().unwrap();
    for e in &etats {
        let b = e.json().unwrap();
        assert_eq!(libelle(&a, &b), complet(&a, &b));
        assert_eq!(libelle(&b, &a), complet(&b, &a));
    }
    assert!(libelle(&a, &etats[2].json().unwrap()).contains("Rez"));
}

/// Annuler et rétablir rendent exactement les états, sur des commandes variées :
/// déplacement, rotation, suppression (avec ouvertures hébergées), niveau, copie,
/// commande composée, champ global du projet.
#[test]
fn commandes_variees_annulees_et_retablies() {
    let p = maison();
    let mur = p.murs[0].id.clone();
    let mut nom = p.clone();
    nom.nom = "Autre nom".into();
    let etats = [
        p.executer(Commande::Deplacer { ids: vec![mur.clone()], delta: [100.0, 50.0, 0.0] }).unwrap().0,
        p.executer(Commande::Tourner { ids: vec![mur.clone()], centre: [0.0, 0.0], angle: 30.0 }).unwrap().0,
        p.executer(Commande::Supprimer { ids: vec![mur.clone()] }).unwrap().0,
        p.executer(Commande::Supprimer { ids: vec![p.ouvertures[0].id.clone()] }).unwrap().0,
        p.executer(Commande::ModifierNiveau { id: p.niveaux[0].id.clone(), nom: "Rez".into(), altitude: 10.0 }).unwrap().0,
        p.executer(Commande::Reseau { ids: vec![mur.clone()], nombre: 4, delta: [0.0, 3000.0, 0.0], centre: [0.0, 0.0], angle: 0.0 }).unwrap().0,
        p.executer(Commande::Symetrie { ids: p.murs.iter().map(|m| m.id.clone()).collect(), a: [0.0, 0.0], b: [0.0, 1000.0] }).unwrap().0,
        nom,
    ];
    for q in &etats {
        let mut h: Historique = Historique::default();
        aller_retour(&mut h, &p, q);
    }
    // enchaînement sur une seule pile, puis tout annuler d'un coup
    let mut h: Historique = Historique::default();
    let mut courant = p.clone();
    let mut q = courant.executer(Commande::Deplacer { ids: vec![mur.clone()], delta: [10.0, 0.0, 0.0] }).unwrap().0;
    h.pousser(&courant, &q).unwrap();
    courant = q;
    q = courant.executer(Commande::Supprimer { ids: vec![mur.clone()] }).unwrap().0;
    h.pousser(&courant, &q).unwrap();
    courant = q;
    let retour = h.cible_n(&courant, false, 2).unwrap();
    h.confirmer_n(false, 2).unwrap();
    assert_eq!(retour.json().unwrap(), p.json().unwrap());
    assert_eq!(h.cible_n(&retour, true, 2).unwrap().json().unwrap(), courant.json().unwrap());
}

/// Une commande composée (réseau) donne une seule étape, qui liste toutes les copies.
#[test]
fn commande_composee_une_etape() {
    let p = maison();
    let mur = p.murs[0].id.clone();
    let q = p.executer(Commande::Reseau { ids: vec![mur], nombre: 5, delta: [0.0, 3000.0, 0.0], centre: [0.0, 0.0], angle: 0.0 }).unwrap().0;
    let mut h: Historique = Historique::default();
    h.pousser(&p, &q).unwrap();
    assert_eq!(h.libelles(false).len(), 1);
    let nouveaux: BTreeSet<String> = q.identifiants().difference(&p.identifiants()).cloned().collect();
    assert!(nouveaux.len() >= 4);
    let m = h.modifications(false, 1).unwrap();
    assert!(nouveaux.is_subset(&m.identifiants), "{nouveaux:?} / {m:?}");
    assert!(m.champs.contains("murs"));
}

/// Glissement : les poussées d'un geste sont fusionnées en une étape qui ramène à
/// l'état d'avant le geste ; les identifiants restent ceux de l'objet glissé.
#[test]
fn fusion_des_glissements() {
    let p = maison();
    let mur = p.murs[1].id.clone();
    let mut h: Historique = Historique::default();
    h.geste(true);
    let mut courant = p.clone();
    for _ in 0..8 {
        let q = courant.executer(Commande::Deplacer { ids: vec![mur.clone()], delta: [25.0, 0.0, 0.0] }).unwrap().0;
        h.pousser(&courant, &q).unwrap();
        courant = q;
    }
    assert!(h.libelles(false).is_empty());
    assert!(h.derniere_modification().unwrap().identifiants.contains(&mur));
    h.geste(false);
    assert_eq!(h.libelles(false).len(), 1);
    let m = h.modifications(false, 1).unwrap();
    assert!(m.identifiants.contains(&mur));
    assert!(m.identifiants.len() < 10, "{m:?}");
    let retour = h.cible(&courant, false).unwrap();
    assert_eq!(retour.json().unwrap(), p.json().unwrap());
    h.confirmer(false).unwrap();
    assert_eq!(h.cible(&retour, true).unwrap().json().unwrap(), courant.json().unwrap());
    // un geste revenu à son point de départ ne laisse rien
    let mut h: Historique = Historique::default();
    let q = p.executer(Commande::Deplacer { ids: vec![mur.clone()], delta: [25.0, 0.0, 0.0] }).unwrap().0;
    h.geste(true);
    h.pousser(&p, &q).unwrap();
    h.pousser(&q, &p).unwrap();
    h.geste(false);
    assert!(h.libelles(false).is_empty());
}

/// Liste des objets touchés : seulement le mur déplacé (et ce qu'il entraîne), pour
/// faire, défaire et refaire ; un champ global est signalé comme champ.
#[test]
fn liste_des_identifiants() {
    let p = maison();
    let mur = p.murs[0].id.clone();
    let q = p.executer(Commande::Deplacer { ids: vec![mur.clone()], delta: [100.0, 0.0, 0.0] }).unwrap().0;
    let mut h: Historique = Historique::default();
    h.pousser(&p, &q).unwrap();
    let faire = h.derniere_modification().unwrap().clone();
    assert!(faire.identifiants.contains(&mur));
    assert!(faire.identifiants.len() < p.identifiants().len() / 2);
    assert_eq!(h.modifications(false, 1).unwrap(), faire);
    h.confirmer(false).unwrap();
    assert_eq!(h.modifications(true, 1).unwrap(), faire);
    assert!(h.modifications(true, 2).is_err());
    let mut r = q.clone();
    r.nom = "Renommé".into();
    h.pousser(&q, &r).unwrap();
    let m = h.derniere_modification().unwrap();
    assert!(m.identifiants.is_empty());
    assert_eq!(m.champs.iter().collect::<Vec<_>>(), ["nom"]);
}

/// Mémoire d'une étape de déplacement d'un mur dans l'immeuble d'essai (1,2 Mo de
/// JSON) : quelques centaines d'octets, contre environ 200 ko par instantané compressé.
#[test]
fn memoire_proportionnelle_au_changement() {
    let p = Projet::lire_json(include_str!("../../../../samples/immeuble_essai_murs/immeuble_essai_murs.modele.json")).unwrap();
    let taille = p.json().unwrap().len();
    let q = p.executer(Commande::Deplacer { ids: vec![p.murs[0].id.clone()], delta: [10.0, 0.0, 0.0] }).unwrap().0;
    let mut h: Historique = Historique::default();
    h.pousser(&p, &q).unwrap();
    assert!(h.memoire() * 500 < taille, "{} octets pour {taille}", h.memoire());
    let retour = h.cible(&q, false).unwrap();
    assert_eq!(retour.json().unwrap(), p.json().unwrap());
}

/// Limite mémoire minuscule : 10 niveaux restent ; sans limite atteinte : 100 au plus.
#[test]
fn niveaux_garantis_et_plafond() {
    let mut h: Historique<FichierFamille> = Historique::default();
    use modele::famille::modelee::{FamilleModelee, FORMAT_FAMILLE};
    let mut f = FichierFamille { format: FORMAT_FAMILLE.into(), nom: "OF".into(), largeur: 1000.0, hauteur: 1000.0, profondeur: 300.0, famille: FamilleModelee::gabarit_fenetre() };
    let limite = LIMITE_MEMOIRE.load(Ordering::Relaxed);
    LIMITE_MEMOIRE.store(1, Ordering::Relaxed);
    for i in 0..30 {
        let mut g = f.clone();
        g.largeur = 1000.0 + i as f64;
        h.pousser(&f, &g).unwrap();
        f = g;
    }
    assert_eq!(h.libelles(false).len(), NIVEAUX_GARANTIS);
    LIMITE_MEMOIRE.store(limite, Ordering::Relaxed);
    for i in 0..150 {
        let mut g = f.clone();
        g.hauteur = 1000.0 + i as f64;
        h.pousser(&f, &g).unwrap();
        f = g;
    }
    assert_eq!(h.libelles(false).len(), NIVEAUX_MAX);
}

#[test]
fn famille_a_sa_propre_pile_et_ses_libelles() {
    use modele::famille::modelee::{AxePlan, FamilleModelee, PlanRef, FORMAT_FAMILLE};
    let a = FichierFamille { format: FORMAT_FAMILLE.into(), nom: "OF".into(), largeur: 1200.0, hauteur: 1350.0, profondeur: 300.0, famille: FamilleModelee::gabarit_fenetre() };
    let mut b = a.clone();
    b.famille.plans.push(PlanRef { nom: "Plan 9".into(), axe: AxePlan::X, position: 100.0, fixe: false, hote: None });
    let mut h: Historique<FichierFamille> = Historique::default();
    h.pousser(&a, &b).unwrap();
    assert_eq!(h.libelles(false), vec!["Ajouter plan de référence « Plan 9 »".to_string()]);
    let retour = h.cible(&b, false).unwrap();
    assert_eq!(retour, a);
    h.confirmer(false).unwrap();
    assert_eq!(h.cible(&retour, true).unwrap(), b);
    let mut plus_profond = b.clone();
    plus_profond.profondeur = 750.0;
    let mut profondeur: Historique<FichierFamille> = Historique::default();
    profondeur.pousser(&b, &plus_profond).unwrap();
    assert_eq!(profondeur.cible(&plus_profond, false).unwrap().profondeur, 300.0);
    profondeur.confirmer(false).unwrap();
    assert_eq!(profondeur.cible(&b, true).unwrap().profondeur, 750.0);
}

#[test]
fn sauvegarde_annulation_branche_et_documents_isoles() {
    let a = Projet::nouveau("a");
    let b = Projet::nouveau("b");
    let c = Projet::nouveau("c");
    let mut h = Historique::default();
    let mut autre = Historique::default();
    h.enregistrer(&a).unwrap();
    autre.enregistrer(&c).unwrap();
    h.pousser(&a, &b).unwrap();
    assert!(h.modifie(&b));
    let retour = h.cible(&b, false).unwrap();
    h.confirmer(false).unwrap();
    assert!(!h.modifie(&retour));
    assert!(h.peut_retablir());
    assert!(!autre.peut_annuler());
    h.pousser(&retour, &c).unwrap();
    assert!(!h.peut_retablir());
}

#[test]
fn plusieurs_etapes_conservent_ordre_et_annulent_atomiquement() {
    let a = Projet::nouveau("a");
    let b = Projet::nouveau("b");
    let c = Projet::nouveau("c");
    let d = Projet::nouveau("d");
    let mut h = Historique::default();
    h.enregistrer(&a).unwrap();
    h.pousser(&a, &b).unwrap();
    h.pousser(&b, &c).unwrap();
    h.pousser(&c, &d).unwrap();
    assert!(h.cible_n(&d, false, 4).is_err());
    assert!(h.confirmer_n(false, 4).is_err());
    assert_eq!(h.libelles(false).len(), 3);
    let cible = h.cible_n(&d, false, 2).unwrap();
    assert_eq!(cible.nom, "b");
    h.confirmer_n(false, 2).unwrap();
    assert_eq!(h.cible_n(&cible, true, 1).unwrap().nom, "c");
    assert_eq!(h.cible_n(&cible, true, 2).unwrap().nom, "d");
    h.confirmer_n(true, 2).unwrap();
    assert_eq!(h.cible_n(&d, false, 2).unwrap().nom, "b");
}

#[test]
fn les_libelles_identifient_les_elements_et_suivent_le_retablissement() {
    let a = Projet::nouveau("test");
    let mut b = a.clone();
    b.niveaux.push(modele::Niveau { id: "n1".into(), nom: "Étage".into(), altitude: 3000.0, origine: None });
    let mut h = Historique::default();
    h.pousser(&a, &b).unwrap();
    assert_eq!(h.libelles(false), ["Ajouter niveau « Étage »"]);
    assert!(h.derniere_modification().unwrap().identifiants.contains("n1"));
    h.confirmer_n(false, 1).unwrap();
    assert_eq!(h.libelles(true), ["Ajouter niveau « Étage »"]);
}
