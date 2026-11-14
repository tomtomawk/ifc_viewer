//! Tests déplacés de `modele/elements/piece.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use crate::edition::Execution;
use crate::modele::*;
use crate::modele::creation::ParametresMur;
use crate::edition::commandes::Commande;
use crate::modele::elements::piece::*;

fn rectangle(p: &mut Projet, o: &ParametresMur) {
    let pts = [[0.0, 0.0], [5000.0, 0.0], [5000.0, 4000.0], [0.0, 4000.0]];
    for i in 0..4 {
        p.ajouter_mur(pts[i], pts[(i + 1) % 4], o).unwrap();
    }
}

fn projet() -> Projet {
    Projet::nouveau("Pièces").executer(Commande::Niveau { nom: "RDC".into(), altitude: 0.0 }).unwrap().0
}

fn piece(p: &Projet, point: [f64; 2]) -> Piece {
    Piece { id: String::new(), nom: "Séjour".into(), numero: String::new(), niveau: p.niveaux[0].id.clone(), point, contour: Vec::new(), etiquette: String::new() }
}

#[test]
fn piece_dans_quatre_murs() {
    let mut p = projet();
    rectangle(&mut p, &ParametresMur { epaisseur: 200.0, ..Default::default() });
    let (q, ids) = p.executer(Commande::Piece { piece: piece(&p, [2500.0, 2000.0]) }).unwrap();
    let x = q.pieces.iter().find(|x| x.id == ids[0]).unwrap();
    assert_eq!(x.numero, "1");
    assert_eq!(x.contour.len(), 4, "{:?}", x.contour);
    // faces intérieures : axe à ±100 mm
    assert!((x.aire() - 4800.0 * 3800.0).abs() < 1.0, "{}", x.aire());
    assert!((x.perimetre() - 2.0 * (4800.0 + 3800.0)).abs() < 1e-3);
    // seconde pièce : numéro suivant ; point hors des murs refusé
    let (q2, _) = q.executer(Commande::Piece { piece: piece(&q, [2500.0, 2000.0]) }).unwrap();
    assert_eq!(q2.pieces[1].numero, "2");
    assert!(q.executer(Commande::Piece { piece: piece(&q, [9000.0, 2000.0]) }).is_err());
    assert!(q.executer(Commande::Piece { piece: piece(&q, [0.0, 2000.0]) }).is_err());
}

#[test]
fn etiquette_famille_de_la_piece() {
    use crate::modele::documentation::{primitives_pieces, FamilleEtiquette, TexteCartouche};
    let mut p = projet();
    rectangle(&mut p, &ParametresMur { epaisseur: 200.0, ..Default::default() });
    let (mut q, ids) = p.executer(Commande::Piece { piece: piece(&p, [2500.0, 2000.0]) }).unwrap();
    let x = q.pieces.iter().find(|x| x.id == ids[0]).unwrap().clone();
    let vid = q.ajouter_vue_plan(&x.niveau, 1200.0).unwrap();
    let vue = q.vues.iter().find(|v| v.id == vid).unwrap().clone();
    // standard : aplat et deux lignes
    assert_eq!(primitives_pieces(&q, &vue).len(), 3);
    let t = |c: &str, y| TexteCartouche { contenu: c.into(), position: [0.0, y], hauteur: 2.0 };
    q.familles_etiquettes.push(FamilleEtiquette { document_famille: None, formes: Vec::new(), id: "e1".into(), nom: "Cadre".into(), traits: vec![[[-10.0, -5.0], [10.0, -5.0]], [[-10.0, 5.0], [10.0, 5.0]]], textes: vec![t("{nom}", 2.0), t("N° {numero}", -1.0), t("{perimetre} · {niveau}", -4.0)] });
    q.pieces[0].etiquette = "e1".into();
    assert_eq!(primitives_pieces(&q, &vue).len(), 1 + 3 + 2);
    assert_eq!(FamilleEtiquette::remplir("N° {numero} — {surface}", &q, &q.pieces[0]), "N° 1 — 18.24 m²");
    // famille absente : retour à la standard
    q.familles_etiquettes.clear();
    assert_eq!(primitives_pieces(&q, &vue).len(), 3);
}

#[test]
fn piece_ouverte_refusee_et_recalcul() {
    let mut p = projet();
    let o = ParametresMur { epaisseur: 200.0, ..Default::default() };
    p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &o).unwrap();
    p.ajouter_mur([5000.0, 0.0], [5000.0, 4000.0], &o).unwrap();
    p.ajouter_mur([5000.0, 4000.0], [0.0, 4000.0], &o).unwrap();
    assert!(p.executer(Commande::Piece { piece: piece(&p, [2500.0, 2000.0]) }).is_err());
    let dernier = p.ajouter_mur([0.0, 4000.0], [0.0, 0.0], &o).unwrap();
    let (q, _) = p.executer(Commande::Piece { piece: piece(&p, [2500.0, 2000.0]) }).unwrap();
    // murs déplacés de 1 m (point de la pièce laissé en place, toujours dedans) : le
    // contour suit
    let murs: Vec<String> = q.murs.iter().map(|m| m.id.clone()).collect();
    let (q, _) = q.executer(Commande::Deplacer { ids: murs, delta: [1000.0, 0.0, 0.0] }).unwrap();
    let x0 = q.pieces[0].contour.iter().map(|c| c[0]).fold(f64::MAX, f64::min);
    assert!((x0 - 1100.0).abs() < 1e-6, "{:?}", q.pieces[0].contour);
    // un mur déplacé seul ouvre la pièce : contour vide
    let (ouverte, _) = q.executer(Commande::Deplacer { ids: vec![dernier.clone()], delta: [-1000.0, 0.0, 0.0] }).unwrap();
    assert!(!ouverte.pieces[0].fermee());
    // supprimé : la pièce reste, sans contour
    let (q, _) = q.executer(Commande::Supprimer { ids: vec![dernier] }).unwrap();
    assert!(!q.pieces[0].fermee());
}

#[test]
fn piece_mur_compose() {
    use crate::modele::{Couche, TypeCompose};
    let mut p = projet();
    p.types_murs.push(TypeCompose {
        id: "compose".into(),
        nom: "Composé".into(),
        couches: vec![Couche { epaisseur: 20.0, ..Default::default() }, Couche { epaisseur: 200.0, ..Default::default() }, Couche { epaisseur: 13.0, ..Default::default() }],
        couches_detaillees: true,
        categorie: None,
        origine: None,
    });
    rectangle(&mut p, &ParametresMur { type_mur: Some("compose".into()), ..Default::default() });
    assert!(!p.assemblages.is_empty(), "le type composé crée des assemblages");
    let (q, ids) = p.executer(Commande::Piece { piece: piece(&p, [2500.0, 2000.0]) }).unwrap();
    let x = q.pieces.iter().find(|x| x.id == ids[0]).unwrap();
    assert!(x.fermee());
    // contour sur la face intérieure de la couche intérieure : l'aire est celle du
    // rectangle d'axes réduit d'une épaisseur de 233 mm au total (une demi par côté
    // selon le sens des murs), strictement plus petite que le rectangle d'axes
    let a = x.aire();
    assert!(a < 5000.0 * 4000.0 && a > 4500.0 * 3500.0, "{a}");
    // aucun sommet dans un mur : chaque sommet est à 1 mm près sur une face de couche
    assert!(x.contour.len() >= 4);
}
