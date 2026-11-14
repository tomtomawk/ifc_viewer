//! Familles livrées dans le gabarit de projet : fenêtre ouvrant à la française à deux
//! vantaux, porte-fenêtre à tableaux et seuil,
//! porte intérieure à un vantail. Le gabarit (`gabarits/defaut.gabarit.json`)
//! en est la copie sérialisée, contrôlée par test.
#![cfg_attr(not(test), allow(dead_code))] // source du gabarit, lue par les tests
use super::*;

fn plan(nom: &str, axe: AxePlan, position: f64) -> PlanRef {
    PlanRef { nom: nom.into(), axe, position, fixe: false, hote: None }
}
fn fixe(nom: &str, axe: AxePlan) -> PlanRef {
    PlanRef { nom: nom.into(), axe, position: 0.0, fixe: true, hote: None }
}
fn hote(nom: &str, h: Hote) -> PlanRef {
    PlanRef { nom: nom.into(), axe: AxePlan::Y, position: 0.0, fixe: false, hote: Some(h) }
}
/// b − a = v
fn dist(a: &str, b: &str, v: &str, priorite: u8) -> Contrainte {
    Contrainte::Distance { a: a.into(), b: b.into(), valeur: v.into(), priorite, ligne: None }
}
fn milieu(a: &str, m: &str, b: &str) -> Contrainte {
    Contrainte::Egalite { a: a.into(), m: m.into(), b: b.into(), priorite: 5, ligne: None }
}
fn face(plan: &str, decalage: f64) -> Face {
    Face { plan: plan.into(), decalage }
}
fn param(nom: &str, formule: &str) -> Parametre {
    Parametre { nom: nom.into(), formule: formule.into(), portee: Portee::Type, genre: GenreParametre::Longueur }
}
type Bornes<'a> = [(&'a str, f64); 2];
fn volume(nom: &str, materiau: Materiau, x: Bornes, y: Bornes, z: Bornes) -> Volume {
    Volume {
        nom: nom.into(),
        vide: false,
        materiau,
        min: [face(x[0].0, x[0].1), face(y[0].0, y[0].1), face(z[0].0, z[0].1)],
        max: [face(x[1].0, x[1].1), face(y[1].0, y[1].1), face(z[1].0, z[1].1)],
        visible: None,
        materiau_parametre: None,
    }
}
fn baie() -> Volume {
    Volume { vide: true, ..volume("Baie", Materiau::Menuiserie, [("Gauche", 0.0), ("Droite", 0.0)], [("Intérieur", -5.0), ("Extérieur", 5.0)], [("Bas", 0.0), ("Haut", 0.0)]) }
}
fn trait_(vue: VueEsquisse, points: Vec<PointEsquisse>, arc: bool) -> Symbole {
    Symbole { vue, points, arc, fermee: false, remplissage: None }
}

impl FamilleModelee {
    /// Fenêtre ouvrant à la française, deux vantaux vers l'intérieur : dormant en
    /// tableau (retrait réglable, retours d'enduit et de doublage jusqu'au dormant),
    /// ouvrants à profils et battue centrale, vitrage isolant, poignée sur le vantail
    /// de gauche, appui en pente. En plan : débattement des deux vantaux et appui
    /// hachuré ; en élévation : triangles d'ouverture (pointe côté paumelles).
    pub fn fenetre_of_2_vantaux() -> FamilleModelee {
        use AxePlan::*;
        FamilleModelee {
            categorie: CategorieFamille::Fenetre,
            section_profil: None,
            elements_2d: Vec::new(),
            plans_construction: Vec::new(),
            parametres: vec![
                param("Dormant", "55"),
                param("Ouvrant", "65"),
                param("TraverseBasse", "85"),
                param("Profondeur", "70"),
                param("Retrait", "50"),
                param("Appui", "40"),
                Parametre { nom: "AvecAppui".into(), formule: "1".into(), portee: Portee::Occurrence, genre: GenreParametre::OuiNon },
                Parametre { nom: "MateriauAppui".into(), formule: String::new(), portee: Portee::Type, genre: GenreParametre::Materiau },
                param("Demi", "(Largeur - 2 * Dormant) / 2"),
                param("Diagonale", "Demi * 0,7071"),
            ],
            plans: vec![
                fixe("Centre", X),
                plan("Gauche", X, -600.0),
                plan("Droite", X, 600.0),
                plan("Dormant gauche", X, -545.0),
                plan("Dormant droit", X, 545.0),
                plan("Ouvrant gauche", X, -480.0),
                plan("Battue gauche", X, -65.0),
                plan("Battue droite", X, 65.0),
                plan("Ouvrant droit", X, 480.0),
                plan("Arc gauche", X, -160.0),
                plan("Arc droit", X, 160.0),
                hote("Extérieur", Hote::Exterieur),
                hote("Intérieur", Hote::Interieur),
                plan("Cadre ext", Y, 100.0),
                plan("Cadre int", Y, 30.0),
                plan("Retour extérieur", Y, 100.0),
                plan("Retour intérieur", Y, 30.0),
                plan("Nu appui", Y, 190.0),
                plan("Ouvert", Y, -515.0),
                plan("Arc y", Y, -355.0),
                fixe("Bas", Z),
                plan("Haut", Z, 1350.0),
                plan("Dormant bas", Z, 55.0),
                plan("Ouvrant bas", Z, 140.0),
                plan("Ouvrant haut", Z, 1230.0),
                plan("Dormant haut", Z, 1295.0),
                plan("Mi-hauteur", Z, 675.0),
            ],
            contraintes: vec![
                milieu("Gauche", "Centre", "Droite"),
                dist("Gauche", "Droite", LARGEUR, 9),
                dist("Bas", "Haut", HAUTEUR, 9),
                dist("Gauche", "Dormant gauche", "Dormant", 5),
                dist("Dormant droit", "Droite", "Dormant", 5),
                dist("Dormant gauche", "Ouvrant gauche", "Ouvrant", 5),
                dist("Battue gauche", "Centre", "Ouvrant", 5),
                dist("Centre", "Battue droite", "Ouvrant", 5),
                dist("Ouvrant droit", "Dormant droit", "Ouvrant", 5),
                dist("Bas", "Dormant bas", "Dormant", 5),
                dist("Dormant haut", "Haut", "Dormant", 5),
                dist("Dormant bas", "Ouvrant bas", "TraverseBasse", 5),
                dist("Ouvrant haut", "Dormant haut", "Ouvrant", 5),
                milieu("Dormant bas", "Mi-hauteur", "Dormant haut"),
                dist("Cadre ext", "Extérieur", "Retrait", 5),
                dist("Cadre int", "Cadre ext", "Profondeur", 5),
                dist("Retour extérieur", "Extérieur", "Retrait", 5),
                dist("Retour intérieur", "Extérieur", "Retrait + Profondeur", 5),
                dist("Extérieur", "Nu appui", "Appui", 5),
                dist("Ouvert", "Cadre int", "Demi", 5),
                dist("Arc y", "Cadre int", "Diagonale", 5),
                dist("Dormant gauche", "Arc gauche", "Diagonale", 5),
                dist("Arc droit", "Dormant droit", "Diagonale", 5),
            ],
            volumes: {
                use Materiau::{Menuiserie as M, Vitrage as V};
                let dormant = [("Cadre int", 0.0), ("Cadre ext", 0.0)];
                // ouvrant : 8 mm en saillie côté intérieur, 12 mm en retrait du nu extérieur du dormant
                let ouvrant = [("Cadre int", -8.0), ("Cadre ext", -12.0)];
                let verre = [("Cadre int", 14.0), ("Cadre ext", -28.0)];
                let hauteur_ouvrant = [("Dormant bas", 0.0), ("Dormant haut", 0.0)];
                let mut v = vec![
                    baie(),
                    volume("Dormant gauche", M, [("Gauche", 0.0), ("Dormant gauche", 0.0)], dormant, [("Bas", 0.0), ("Haut", 0.0)]),
                    volume("Dormant droit", M, [("Dormant droit", 0.0), ("Droite", 0.0)], dormant, [("Bas", 0.0), ("Haut", 0.0)]),
                    volume("Dormant bas", M, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], dormant, [("Bas", 0.0), ("Dormant bas", 0.0)]),
                    volume("Dormant haut", M, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], dormant, [("Dormant haut", 0.0), ("Haut", 0.0)]),
                ];
                for (cote, ext, battue, x0, x1) in [("gauche", "Dormant gauche", "Centre", "Ouvrant gauche", "Battue gauche"), ("droit", "Dormant droit", "Centre", "Battue droite", "Ouvrant droit")] {
                    let (a, b) = if cote == "gauche" { (ext, x0) } else { (x1, ext) };
                    let (c, d) = if cote == "gauche" { (x1, battue) } else { (battue, x0) };
                    v.extend([
                        volume(&format!("Ouvrant {cote} · montant"), M, [(a, 0.0), (b, 0.0)], ouvrant, hauteur_ouvrant),
                        volume(&format!("Ouvrant {cote} · battue"), M, [(c, 0.0), (d, 0.0)], ouvrant, hauteur_ouvrant),
                        volume(&format!("Ouvrant {cote} · traverse basse"), M, [(x0, 0.0), (x1, 0.0)], ouvrant, [("Dormant bas", 0.0), ("Ouvrant bas", 0.0)]),
                        volume(&format!("Ouvrant {cote} · traverse haute"), M, [(x0, 0.0), (x1, 0.0)], ouvrant, [("Ouvrant haut", 0.0), ("Dormant haut", 0.0)]),
                        volume(&format!("Vitrage {cote}"), V, [(x0, 0.0), (x1, 0.0)], verre, [("Ouvrant bas", 0.0), ("Ouvrant haut", 0.0)]),
                    ]);
                }
                // l'appui s'encastre sous la baie : sa réservation creuse le mur, sinon les
                // deux se superposent en coupe
                v.push(Volume { vide: true, visible: Some("AvecAppui".into()), ..volume("Réservation appui", M, [("Gauche", -20.0), ("Droite", 20.0)], [("Cadre ext", -10.0), ("Extérieur", 5.0)], [("Bas", -45.0), ("Bas", 0.0)]) });
                // poignée (crémone) côté intérieur, sur la battue du vantail de gauche
                v.extend([
                    volume("Poignée · platine", M, [("Centre", -52.0), ("Centre", -24.0)], [("Cadre int", -20.0), ("Cadre int", -8.0)], [("Mi-hauteur", -40.0), ("Mi-hauteur", 40.0)]),
                    volume("Poignée · béquille", M, [("Centre", -46.0), ("Centre", -30.0)], [("Cadre int", -45.0), ("Cadre int", -20.0)], [("Mi-hauteur", -130.0), ("Mi-hauteur", 10.0)]),
                ]);
                v
            },
            // appui en pente : profil esquissé en coupe, extrudé sur la largeur
            extrusions: vec![FormeProfil {
                operation: OperationForme::Extrusion,
                construction: None,
                nom: "Appui".into(),
                vide: false,
                materiau: Materiau::Appui,
                vue: VueEsquisse::Coupe,
                profil: vec![point("Cadre ext", -10.0, "Bas", 0.0), point("Nu appui", 0.0, "Bas", -20.0), point("Nu appui", 0.0, "Bas", -45.0), point("Cadre ext", -10.0, "Bas", -45.0)],
                profondeur: [face("Gauche", -20.0), face("Droite", 20.0)],
                visible: Some("AvecAppui".into()),
                materiau_parametre: Some("MateriauAppui".into()),
            }],
            symboles: vec![
                // plan : vantaux ouverts à 90° vers l'intérieur et arcs de débattement
                trait_(VueEsquisse::Plan, vec![point("Dormant gauche", 0.0, "Cadre int", 0.0), point("Dormant gauche", 0.0, "Ouvert", 0.0)], false),
                trait_(VueEsquisse::Plan, vec![point("Centre", 0.0, "Cadre int", 0.0), point("Arc gauche", 0.0, "Arc y", 0.0), point("Dormant gauche", 0.0, "Ouvert", 0.0)], true),
                trait_(VueEsquisse::Plan, vec![point("Dormant droit", 0.0, "Cadre int", 0.0), point("Dormant droit", 0.0, "Ouvert", 0.0)], false),
                trait_(VueEsquisse::Plan, vec![point("Centre", 0.0, "Cadre int", 0.0), point("Arc droit", 0.0, "Arc y", 0.0), point("Dormant droit", 0.0, "Ouvert", 0.0)], true),
                Symbole {
                    vue: VueEsquisse::Plan,
                    points: vec![point("Gauche", -20.0, "Cadre ext", -10.0), point("Droite", 20.0, "Cadre ext", -10.0), point("Droite", 20.0, "Nu appui", 0.0), point("Gauche", -20.0, "Nu appui", 0.0)],
                    arc: false,
                    fermee: true,
                    remplissage: Some(crate::modele::documentation::RemplissageContour::Motif { motif: crate::modele::MotifCoupe::Diagonales }),
                },
                // élévation : pointe du triangle côté paumelles
                trait_(VueEsquisse::Elevation, vec![point("Centre", 0.0, "Dormant bas", 0.0), point("Dormant gauche", 0.0, "Mi-hauteur", 0.0), point("Centre", 0.0, "Dormant haut", 0.0)], false),
                trait_(VueEsquisse::Elevation, vec![point("Centre", 0.0, "Dormant bas", 0.0), point("Dormant droit", 0.0, "Mi-hauteur", 0.0), point("Centre", 0.0, "Dormant haut", 0.0)], false),
            ],
            cotes_mesure: Vec::new(),
            couleur: couleur_defaut(),
            genre: crate::modele::GenreOuverture::Fenetre,
        }
    }

    /// Porte-fenêtre à deux vantaux dérivée de la fenêtre OF : tableaux et linteau
    /// habillés (panneaux du dormant au nu extérieur, à la place du retour d'enduit) et
    /// seuil en pente devant la traverse basse. Matériaux des tableaux et du seuil au
    /// choix par paramètre.
    pub fn porte_fenetre_tableaux_seuil() -> FamilleModelee {
        use AxePlan::*;
        let mut f = FamilleModelee::fenetre_of_2_vantaux();
        f.parametres.retain(|p| p.nom != "AvecAppui" && p.nom != "MateriauAppui");
        f.parametres.extend([
            param("Tableau", "20"),
            param("Seuil", "40"),
            Parametre { nom: "MateriauTableau".into(), formule: String::new(), portee: Portee::Type, genre: GenreParametre::Materiau },
            Parametre { nom: "MateriauSeuil".into(), formule: String::new(), portee: Portee::Type, genre: GenreParametre::Materiau },
        ]);
        f.plans.extend([plan("Tableau gauche", X, -680.0), plan("Tableau droit", X, 680.0), plan("Tableau haut", Z, 2130.0), plan("Seuil haut", Z, 40.0)]);
        // les panneaux de tableau remplacent le retour d'enduit
        for c in &mut f.contraintes {
            if let Contrainte::Distance { a, valeur, .. } = c && a == "Retour extérieur" { *valeur = "0".into(); }
        }
        f.contraintes.extend([
            dist("Gauche", "Tableau gauche", "Tableau", 5),
            dist("Tableau droit", "Droite", "Tableau", 5),
            dist("Tableau haut", "Haut", "Tableau", 5),
            dist("Bas", "Seuil haut", "Seuil", 5),
        ]);
        f.volumes.retain(|v| v.nom != "Réservation appui");
        let tableau = [("Cadre ext", 0.0), ("Extérieur", 0.0)];
        let panneau = |nom: &str, x: Bornes, z: Bornes| Volume { materiau_parametre: Some("MateriauTableau".into()), ..volume(nom, Materiau::Panneau, x, tableau, z) };
        f.volumes.extend([
            panneau("Tableau gauche", [("Gauche", 0.0), ("Tableau gauche", 0.0)], [("Bas", 0.0), ("Haut", 0.0)]),
            panneau("Tableau droit", [("Tableau droit", 0.0), ("Droite", 0.0)], [("Bas", 0.0), ("Haut", 0.0)]),
            panneau("Tableau haut", [("Tableau gauche", 0.0), ("Tableau droit", 0.0)], [("Tableau haut", 0.0), ("Haut", 0.0)]),
        ]);
        f.extrusions = vec![FormeProfil {
                operation: OperationForme::Extrusion,
                construction: None,
            nom: "Seuil".into(),
            vide: false,
            materiau: Materiau::Appui,
            vue: VueEsquisse::Coupe,
            profil: vec![point("Cadre ext", 0.0, "Seuil haut", 0.0), point("Nu appui", 0.0, "Bas", 15.0), point("Nu appui", 0.0, "Bas", 0.0), point("Cadre ext", 0.0, "Bas", 0.0)],
            profondeur: [face("Tableau gauche", 0.0), face("Tableau droit", 0.0)],
            visible: None,
            materiau_parametre: Some("MateriauSeuil".into()),
        }];
        // plan : seuil hachuré entre les tableaux
        for s in f.symboles.iter_mut().filter(|s| s.remplissage.is_some()) {
            s.points = vec![point("Tableau gauche", 0.0, "Cadre ext", 0.0), point("Tableau droit", 0.0, "Cadre ext", 0.0), point("Tableau droit", 0.0, "Nu appui", 0.0), point("Tableau gauche", 0.0, "Nu appui", 0.0)];
        }
        f
    }

    /// Porte intérieure à un vantail : huisserie centrée dans l'épaisseur du mur, vantail
    /// plein détalonné, paumelles à gauche, béquilles des deux côtés. En plan : vantail
    /// ouvert à 90° (aplat) et arc de débattement ; en élévation : triangle d'ouverture.
    pub fn porte_1_vantail() -> FamilleModelee {
        use AxePlan::*;
        use Materiau::{Menuiserie as M, Panneau as P};
        let huisserie = [("Cadre int", 0.0), ("Cadre ext", 0.0)];
        FamilleModelee {
            categorie: CategorieFamille::Porte,
            section_profil: None,
            elements_2d: Vec::new(),
            plans_construction: Vec::new(),
            parametres: vec![
                param("Dormant", "45"),
                param("Profondeur", "72"),
                param("Retrait", "(Epaisseur - Profondeur) / 2"),
                param("Vantail", "40"),
                param("Detalonnage", "10"),
                param("HauteurPoignee", "1050"),
                param("Passage", "Largeur - 2 * Dormant"),
                param("Diagonale", "Passage * 0,7071"),
            ],
            plans: vec![
                fixe("Centre", X),
                plan("Gauche", X, -450.0),
                plan("Droite", X, 450.0),
                plan("Dormant gauche", X, -405.0),
                plan("Dormant droit", X, 405.0),
                plan("Arc x", X, 170.0),
                hote("Extérieur", Hote::Exterieur),
                hote("Intérieur", Hote::Interieur),
                plan("Cadre ext", Y, 36.0),
                plan("Cadre int", Y, -36.0),
                plan("Vantail ext", Y, 4.0),
                plan("Ouvert", Y, -846.0),
                plan("Arc y", Y, -609.0),
                fixe("Bas", Z),
                plan("Haut", Z, 2150.0),
                plan("Dormant haut", Z, 2105.0),
                plan("Pied vantail", Z, 10.0),
                plan("Poignée", Z, 1050.0),
                plan("Mi-hauteur", Z, 1052.5),
            ],
            contraintes: vec![
                milieu("Gauche", "Centre", "Droite"),
                dist("Gauche", "Droite", LARGEUR, 9),
                dist("Bas", "Haut", HAUTEUR, 9),
                dist("Gauche", "Dormant gauche", "Dormant", 5),
                dist("Dormant droit", "Droite", "Dormant", 5),
                dist("Dormant haut", "Haut", "Dormant", 5),
                dist("Bas", "Pied vantail", "Detalonnage", 5),
                dist("Bas", "Poignée", "HauteurPoignee", 5),
                milieu("Bas", "Mi-hauteur", "Dormant haut"),
                dist("Cadre ext", "Extérieur", "Retrait", 5),
                dist("Cadre int", "Cadre ext", "Profondeur", 5),
                dist("Cadre int", "Vantail ext", "Vantail", 5),
                dist("Ouvert", "Cadre int", "Passage", 5),
                dist("Dormant gauche", "Arc x", "Diagonale", 5),
                dist("Arc y", "Cadre int", "Diagonale", 5),
            ],
            volumes: vec![
                baie(),
                volume("Huisserie gauche", M, [("Gauche", 0.0), ("Dormant gauche", 0.0)], huisserie, [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Huisserie droite", M, [("Dormant droit", 0.0), ("Droite", 0.0)], huisserie, [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Huisserie haute", M, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], huisserie, [("Dormant haut", 0.0), ("Haut", 0.0)]),
                volume("Vantail", P, [("Dormant gauche", 3.0), ("Dormant droit", -3.0)], [("Cadre int", 0.0), ("Vantail ext", 0.0)], [("Pied vantail", 0.0), ("Dormant haut", -3.0)]),
                // béquilles et rosaces, côté intérieur puis côté extérieur du vantail
                volume("Rosace intérieure", M, [("Dormant droit", -85.0), ("Dormant droit", -45.0)], [("Cadre int", -8.0), ("Cadre int", 0.0)], [("Poignée", -20.0), ("Poignée", 20.0)]),
                volume("Béquille intérieure", M, [("Dormant droit", -190.0), ("Dormant droit", -55.0)], [("Cadre int", -60.0), ("Cadre int", -45.0)], [("Poignée", -9.0), ("Poignée", 9.0)]),
                volume("Tige intérieure", M, [("Dormant droit", -72.0), ("Dormant droit", -58.0)], [("Cadre int", -45.0), ("Cadre int", -8.0)], [("Poignée", -7.0), ("Poignée", 7.0)]),
                volume("Rosace extérieure", M, [("Dormant droit", -85.0), ("Dormant droit", -45.0)], [("Vantail ext", 0.0), ("Vantail ext", 8.0)], [("Poignée", -20.0), ("Poignée", 20.0)]),
                volume("Tige extérieure", M, [("Dormant droit", -72.0), ("Dormant droit", -58.0)], [("Vantail ext", 8.0), ("Vantail ext", 45.0)], [("Poignée", -7.0), ("Poignée", 7.0)]),
                volume("Béquille extérieure", M, [("Dormant droit", -190.0), ("Dormant droit", -55.0)], [("Vantail ext", 45.0), ("Vantail ext", 60.0)], [("Poignée", -9.0), ("Poignée", 9.0)]),
            ],
            extrusions: Vec::new(),
            symboles: vec![
                // plan : vantail ouvert à 90° (aplat de son épaisseur) et arc de débattement
                Symbole {
                    vue: VueEsquisse::Plan,
                    points: vec![point("Dormant gauche", 0.0, "Cadre int", 0.0), point("Dormant gauche", 40.0, "Cadre int", 0.0), point("Dormant gauche", 40.0, "Ouvert", 0.0), point("Dormant gauche", 0.0, "Ouvert", 0.0)],
                    arc: false,
                    fermee: true,
                    remplissage: Some(crate::modele::documentation::RemplissageContour::Motif { motif: crate::modele::MotifCoupe::Uni }),
                },
                trait_(VueEsquisse::Plan, vec![point("Dormant droit", 0.0, "Cadre int", 0.0), point("Arc x", 0.0, "Arc y", 0.0), point("Dormant gauche", 0.0, "Ouvert", 0.0)], true),
                trait_(VueEsquisse::Elevation, vec![point("Dormant droit", 0.0, "Pied vantail", 0.0), point("Dormant gauche", 0.0, "Mi-hauteur", 0.0), point("Dormant droit", 0.0, "Dormant haut", 0.0)], false),
            ],
            cotes_mesure: Vec::new(),
            couleur: [196, 170, 132],
            genre: crate::modele::GenreOuverture::Porte,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn familles_du_catalogue_valides() {
        for (f, l, h) in [(FamilleModelee::fenetre_of_2_vantaux(), 1200.0, 1350.0), (FamilleModelee::porte_1_vantail(), 900.0, 2150.0), (FamilleModelee::porte_fenetre_tableaux_seuil(), 1400.0, 2150.0)] {
            for e in [160.0, 200.0, 300.0, 450.0] {
                f.valider(l, h, e).unwrap();
            }
            f.valider(l * 1.5, h * 1.2, 300.0).unwrap();
        }
    }

    /// Écrit la section `types_ouvertures` du gabarit par défaut (une ligne par type),
    /// recopiée dans le gabarit par `python tools/gabarit-familles.py`.
    #[test]
    #[ignore]
    fn ecrire_familles_du_gabarit() {
        let lignes: Vec<String> = crate::modele::gabarit::familles_livrees().iter().map(|t| serde_json::to_string(t).unwrap()).collect();
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/familles-gabarit.json"), lignes.join("\n")).unwrap();
    }
}
