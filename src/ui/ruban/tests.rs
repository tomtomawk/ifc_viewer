use super::*;

#[test]
fn croquis_mur_suit_hauteur_niveaux_et_decalages() {
    let projet = crate::modele::Projet::nouveau("Croquis");
    let mut params = crate::modele::edition::ParametresMur::default();
    params.niveau_haut = Some("haut".into());
    assert_eq!(limites_croquis_mur(&params, &projet, 10.0, 46.0), (10.0, 46.0));
    for valeur in [1.0, 20_000.0] {
        params.decalage_haut = valeur;
        params.decalage = -valeur;
        assert_eq!(limites_croquis_mur(&params, &projet, 10.0, 46.0), (0.0, 56.0));
        params.decalage_haut = -valeur;
        params.decalage = valeur;
        assert_eq!(limites_croquis_mur(&params, &projet, 10.0, 46.0), (20.0, 36.0));
    }
    params.niveau_haut = None;
    assert_eq!(limites_croquis_mur(&params, &projet, 10.0, 46.0), (0.0, 36.0));
}

#[test]
fn boutons_modifier_declenchent_les_actions_sans_changer_outil() {
    let ctx = egui::Context::default();
    let mut ruban = RubanEdition::default();
    ruban.activer_mur();
    for outil in [Outil::CreerIdentique, Outil::Supprimer] {
        let mut action = None;
        let mut frame = |events| { let mut output = ctx.run_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1500.0, 200.0))),
            events,
            ..Default::default()
        }, |ui| {
            egui::Panel::top("modifier_test").show(ui, |ui| {
                if let Some(a) = ruban.afficher(ui, Menu::Modifier, Some(&crate::modele::Projet::default())) { action = Some(a); }
            });
        }); output.textures_delta.clear(); output };
        frame(vec![]);
        let output = frame(vec![]);
        let position = output.shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == outil.nom() => Some(t.pos + t.galley.size() / 2.0),
            _ => None,
        }).expect("commande du ruban absente");
        for pressed in [true, false] {
            frame(vec![egui::Event::PointerMoved(position), egui::Event::PointerButton {
                pos: position, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
            }]);
        }
        drop(frame);
        assert_eq!(action, Some(outil));
        assert_eq!(ruban.outil(), Some(Outil::Mur));
    }
}

#[test]
fn bouton_attacher_toiture_active_la_selection_graphique() {
    let ctx = egui::Context::default();
    let mut ruban = RubanEdition::default();
    let mut frame = |events| { let mut output = ctx.run_ui(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1800.0, 200.0))),
        events, ..Default::default()
    }, |ui| { egui::Panel::top("attacher_test").show(ui, |ui| {
        ruban.afficher(ui, Menu::Modifier, Some(&crate::modele::Projet::default()));
    }); }); output.textures_delta.clear(); output };
    frame(vec![]);
    let output = frame(vec![]);
    let position = output.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Text(t) if t.galley.text() == Outil::AttacherToiture.nom() => Some(t.pos + t.galley.size() / 2.0),
        _ => None,
    }).expect("bouton Attacher toit absent");
    for pressed in [true, false] {
        frame(vec![egui::Event::PointerMoved(position), egui::Event::PointerButton {
            pos: position, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
        }]);
    }
    drop(frame);
    assert_eq!(ruban.outil(), Some(Outil::AttacherToiture));
}

#[test]
fn apercu_des_couches_dessine_le_motif_de_coupe() {
    use crate::modele::{Materiau, MotifCoupe, Projet};
    let ctx = egui::Context::default();
    let projet = Projet::nouveau("Aperçu");
    let dessin = |motif| {
        let materiau = Materiau { motif, ..Default::default() };
        let mut sortie = ctx.run_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0))),
            ..Default::default()
        }, |ui| {
            egui::Panel::top("motif_test").show(ui, |ui| {
                let r = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(80.0, 40.0));
                peindre_materiau_coupe(ui.painter(), r, &projet, &materiau, true);
            });
        });
        sortie.textures_delta.clear();
        sortie.shapes.iter().filter(|s| matches!(s.shape, egui::Shape::LineSegment { .. })).count()
    };
    let sans_motif = dessin(MotifCoupe::Vide);
    assert!(dessin(MotifCoupe::Diagonales) > sans_motif);
    assert!(dessin(MotifCoupe::Perso("Béton".into())) > sans_motif);
}

#[test]
fn crayon_du_type_courant_ouvre_son_editeur() {
    let ctx = egui::Context::default();
    let mut projet = crate::modele::Projet::nouveau("Test");
    projet.types_murs.push(crate::modele::TypeCompose {
        id: "mur-test".into(), nom: "Mur test".into(),
        couches: vec![crate::modele::Couche { epaisseur: 200.0, ..Default::default() }],
        categorie: None, couches_detaillees: true, origine: None,
    });
    let mut ruban = RubanEdition::default();
    ruban.type_mur = Some("mur-test".into());
    let mut frame = |events| {
        let mut position = egui::Pos2::ZERO;
        let mut sortie = ctx.run_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 300.0))),
            events, ..Default::default()
        }, |ui| {
            egui::Panel::top("selecteur_test").show(ui, |ui| {
                ui.set_max_width(300.0);
                let origine = ui.cursor().min;
                position = egui::pos2(origine.x + ui.available_width() - 15.0, origine.y + 42.0);
                ruban.selecteur_type_mur(ui, &projet, false);
            });
        });
        sortie.textures_delta.clear();
        position
    };
    let position = frame(vec![]);
    frame(vec![egui::Event::PointerMoved(position)]);
    for pressed in [true, false] {
        frame(vec![egui::Event::PointerMoved(position), egui::Event::PointerButton {
            pos: position, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
        }]);
    }
    drop(frame);
    assert!(ruban.editeur_type_ouvert());
    assert_eq!(ruban.editeur_type.as_ref().and_then(|e| e.id.as_deref()), Some("mur-test"));
}

fn afficher(
    ctx: &egui::Context,
    ruban: &mut RubanEdition,
    largeur: f32,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut sortie = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(largeur, 520.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            egui::Panel::top("ruban_test")
                .show(ui, |ui| ruban.afficher(ui, Menu::Modeliser, Some(&crate::modele::Projet::default())));
            egui::Panel::left("proprietes_test")
                .exact_size(284.0)
                .show(ui, |ui| {
                    ui.label("Propriétés");
                    ruban.proprietes_surfaces(ui, None);
                });
            egui::CentralPanel::default().show(ui, |_| {});
        },
    );
    sortie.textures_delta.clear();
    sortie
}

fn libelle_correspond(o: Outil, texte: &str) -> bool {
    texte == o.nom()
}

fn cliquer(
    ctx: &egui::Context,
    ruban: &mut RubanEdition,
    largeur: f32,
    texte: &str,
) -> egui::FullOutput {
    afficher(ctx, ruban, largeur, vec![]);
    let sortie = afficher(ctx, ruban, largeur, vec![]);
    // dernière occurrence : Propriétés est dessinée après le ruban, qui peut
    // afficher le même libellé (sorte d'élément horizontal)
    let position = sortie
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t)
                if t.galley.text() == texte =>
            {
                Some(t.pos + t.galley.size() / 2.0)
            }
            _ => None,
        })
        .last()
        .unwrap_or_else(|| panic!("commande absente : {texte}"));

    for pressed in [true, false] {
        afficher(
            ctx,
            ruban,
            largeur,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    afficher(ctx, ruban, largeur, vec![])
}

#[test]
fn palette_mur_affiche_contraintes_et_choix_du_document() {
    let ctx = egui::Context::default();
    let mut ruban = RubanEdition::default();
    ruban.activer_mur();
    let mut projet = crate::modele::Projet::nouveau("Mur");
    projet.phases = vec!["Existant".into(), "Construction".into()];
    projet.sous_projets = vec!["Enveloppe".into(), "Structure".into()];
    for largeur in [220.0, 260.0, 300.0, 420.0] {
        let mut sortie = None;
        for _ in 0..3 {
            let mut frame = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(largeur, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        assert!(ruban.proprietes_mur(ui, Some(&projet)));
                    });
                },
            );
            frame.textures_delta.clear();
            sortie = Some(frame);
        }
        let textes: Vec<_> = sortie
            .unwrap()
            .shapes
            .into_iter()
            .filter_map(|s| match s.shape {
                egui::Shape::Text(t) => Some((
                    t.galley.text().to_string(),
                    egui::Rect::from_min_size(t.pos, t.galley.size()),
                )),
                _ => None,
            })
            .collect();
        for attendu in ["Haut", "Bas", "Libre", "Épaisseur", "Implantation"] {
            assert!(textes.iter().any(|(t, _)| t == attendu), "Champ absent : {attendu}");
        }
        let position = |nom: &str| textes.iter().find(|(t, _)| t == nom).unwrap().1;
        // limites sur une ligne chacune, le haut au-dessus du bas, puis la grille
        assert!((position("Haut").center().y - position("Libre").center().y).abs() < 3.0);
        assert!(position("Bas").top() > position("Haut").bottom());
        assert!(position("Épaisseur").top() > position("Bas").bottom());
    }
}

#[test]
fn selecteur_surfaces_et_reglages_independants() {
    for largeur in [800.0, 1500.0] {
        let ctx = egui::Context::default();
        let mut ruban = RubanEdition::default();
        let sortie = cliquer(&ctx, &mut ruban, largeur, "Éléments horizontaux");
        assert_eq!(ruban.selection, Some(Outil::Dalle));
        let encart = sortie
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Dalle · Dalle BA 200" => Some(t.pos),
                _ => None,
            })
            .last()
            .expect("sélecteur Dalle dans Propriétés");
        assert!(
            encart.x < 284.0 && encart.y > HAUTEUR_GROUPE,
            "le sélecteur doit se trouver dans la palette sous le ruban"
        );
        // la liste reste ouverte d'un onglet de sorte à l'autre
        cliquer(&ctx, &mut ruban, largeur, "Dalle · Dalle BA 200");
        cliquer(
            &ctx,
            &mut ruban,
            largeur,
            "Sol",
        );
        assert_eq!(ruban.selection, Some(Outil::Sol));
        assert!(matches!(
            ruban.valeurs.get(&(Outil::Sol, 1)),
            Some(Valeur::Nombre(0.0))
        ));
        ruban.valeurs.insert((Outil::Sol, 1), Valeur::Nombre(21.0));
        cliquer(
            &ctx,
            &mut ruban,
            largeur,
            "Plafond",
        );
        assert_eq!(ruban.selection, Some(Outil::Plafond));
        cliquer(
            &ctx,
            &mut ruban,
            largeur,
            "Dalle",
        );
        assert!(matches!(
            ruban.valeurs.get(&(Outil::Dalle, 1)),
            Some(Valeur::Nombre(0.0))
        ));
        cliquer(
            &ctx,
            &mut ruban,
            largeur,
            "Sol",
        );
        assert!(matches!(
            ruban.valeurs.get(&(Outil::Sol, 1)),
            Some(Valeur::Nombre(21.0))
        ));
        let sortie = cliquer(&ctx, &mut ruban, largeur, "Toiture");
        assert_eq!(ruban.selection, Some(Outil::Toiture));
        assert!(
            sortie.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Pente")
            )
        );
        cliquer(&ctx, &mut ruban, largeur, "Mur");
        assert!(
            ruban.mur_actif(),
            "le sélecteur ne doit pas réactiver les surfaces après le clic Mur"
        );
        cliquer(&ctx, &mut ruban, largeur, "Éléments horizontaux");
        assert_eq!(
            ruban.selection,
            Some(Outil::Toiture),
            "le groupe retrouve le dernier choix"
        );
    }
}

#[test]
fn reglages_accessibles_en_mode_compact() {
    let ctx = egui::Context::default();
    let mut ruban = RubanEdition::default();
    ruban.activer_mur();
    let mut afficher = |events| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 520.0),
            )),
            events,
            ..Default::default()
        };
        let mut sortie = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .show(ui, |ui| ruban.afficher(ui, Menu::Modeliser, Some(&crate::modele::Projet::default())));
        });
        sortie.textures_delta.clear();
        sortie
    };
    let sortie = afficher(vec![]);
    let bouton = sortie
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "..." => {
                Some(t.pos + t.galley.size() / 2.0)
            }
            _ => None,
        })
        .expect("bouton de réglages compact");
    afficher(vec![
        egui::Event::PointerMoved(bouton),
        egui::Event::PointerButton {
            pos: bouton,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    afficher(vec![egui::Event::PointerButton {
        pos: bouton,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    let sortie = afficher(vec![]);
    assert!(
        sortie.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Implantation")
        ),
        "le clic sur les réglages doit rendre les options Mur accessibles"
    );
}

#[test]
fn commandes_et_options_tiennent_aux_largeurs_du_viewer() {
    let ctx = egui::Context::default();
    ctx.set_visuals(egui::Visuals::light());
    let projet = crate::modele::Projet::nouveau("Ruban");
    for largeur in [800.0, 1000.0, 1280.0, 1500.0] {
        for menu in [Menu::Modeliser, Menu::Modifier, Menu::Annoter] {
            let outils: Vec<_> = groupes(menu)
                .iter()
                .flat_map(|(_, outils)| outils.iter().copied())
                .collect();
            let selections = outils
                .iter()
                .copied()
                .map(|o| {
                    if o == Outil::Surfaces {
                        Outil::Dalle
                    } else {
                        o
                    }
                })
                .chain(
                    SURFACES
                        .iter()
                        .map(|&(o, _)| o)
                        .filter(|_| menu == Menu::Modeliser),
                );
            for selection in std::iter::once(None).chain(selections.map(Some)) {
                let mut ruban = RubanEdition {
                    selection,
                    ..Default::default()
                };
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(largeur, 520.0),
                    )),
                    ..Default::default()
                };
                let mut sortie = ctx.run_ui(input, |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        let debut = ui.next_widget_position().y;
                        ruban.afficher(ui, menu, Some(&projet));
                        assert!(
                            ui.next_widget_position().y - debut <= 100.0,
                            "ruban trop haut à {largeur}, {selection:?}"
                        );
                    });
                });
                sortie.textures_delta.clear();
                let mut textes = Vec::new();
                fn relever(forme: &egui::Shape, textes: &mut Vec<(String, egui::Rect)>) {
                    match forme {
                        egui::Shape::Text(t) => textes.push((
                            t.galley.text().into(),
                            egui::Rect::from_min_size(t.pos, t.galley.size()),
                        )),
                        egui::Shape::Vec(formes) => {
                            formes.iter().for_each(|f| relever(f, textes))
                        }
                        _ => {}
                    }
                }
                for forme in &sortie.shapes {
                    relever(&forme.shape, &mut textes);
                }
                for outil in &outils {
                    let (_, rect) = textes
                        .iter()
                        .find(|(texte, _)| libelle_correspond(*outil, texte))
                        .unwrap_or_else(|| panic!("commande absente : {outil:?}"));
                    assert!(
                        rect.left() >= 0.0 && rect.right() <= largeur,
                        "commande hors écran à {largeur}, {selection:?} : {outil:?} {rect:?}"
                    );
                }
                let commandes: Vec<_> = textes
                    .iter()
                    .filter(|(texte, _)| outils.iter().any(|o| libelle_correspond(*o, texte)))
                    .collect();
                for (indice, (nom, rect)) in commandes.iter().enumerate() {
                    for (autre, voisin) in commandes.iter().skip(indice + 1) {
                        assert!(
                            !rect.shrink(0.5).intersects(*voisin),
                            "commandes superposées à {largeur}, {selection:?} : {nom} / {autre}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn creation_identique_reutilise_types_niveaux_et_vue() {
    let p = crate::modele::Projet::lire_json(&std::fs::read_to_string("samples/maison_native.modele.json").unwrap()).unwrap();
    let d = &p.dalles[0];
    let mut ruban = RubanEdition::default();
    ruban.surface_identique(&p, d).unwrap();
    let options = ruban.dalle_active().unwrap();
    assert_eq!(options.type_dalle.nom, p.type_dalle(&d.type_dalle).unwrap().nom);
    assert_eq!(options.type_dalle.couches, p.type_dalle(&d.type_dalle).unwrap().couches);
    assert_eq!(options.niveau.as_ref(), Some(&d.niveau));
    assert_eq!(options.decalage, d.decalage);
    let vue = p.vues.iter().find(|v| v.id == "coupe").unwrap();
    ruban.coupe_identique(vue).unwrap();
    let c = ruban.coupe_active().unwrap();
    assert_eq!(c.echelle, vue.echelle);
    assert_eq!(c.type_vue, vue.type_vue);
    assert_eq!(c.profondeur, 6000.0);
}

