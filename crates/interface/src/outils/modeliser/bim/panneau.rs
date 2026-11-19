//! Panneau d'options des outils BIM (`panneau_bim`).

use super::*;
use super::widgets::*;

impl Application {
    pub(crate) fn panneau_bim(&mut self, ui: &mut egui::Ui) -> bool {
        use commandes::Commande;
        use crate::zones::ruban::Outil;
        if self.natif.is_none() {
            return false;
        }
        let outil = self.ruban_edition.outil();
        if let Some(annotation) = &self.bim.annotation_identique {
            use modele::documentation::AnnotationForme as F;
            let attendu = match annotation.forme {
                F::Texte { .. } => Outil::Texte,
                F::Ligne { .. } | F::Polyligne { .. } | F::Arc { .. } => Outil::Polyligne,
                F::Spline { .. } => Outil::Spline,
                F::Cercle { .. } => Outil::Cercle,
                F::Zone { .. } => Outil::Zone,
                F::Cote { .. } => Outil::Cote,
                F::Etiquette { .. } => Outil::Etiquette,
                F::CoteAngulaire { .. } => Outil::CoteAngulaire,
                F::CoteDiametre { .. } => Outil::CoteDiametre,
                F::Altimetrie { .. } => Outil::Altimetrie,
            };
            if self.onglet_ruban != OngletRuban::Annoter || outil != Some(attendu) {
                self.bim.annotation_identique = None;
            }
        }
        let ids = self.selection_bim();
        let mut commande = None;
        let mut ouvrir_composant: Option<Option<String>> = None;
        let mut ouvrir_etiquette: Option<(Option<String>, Option<String>)> = None;
        if self.onglet_ruban == OngletRuban::Modifier {
            if ids.len() == 1 {
                let p = &self.natif.as_ref().unwrap().0;
                if let Some(original) = p.terrains.iter().find(|t| t.id == ids[0]) {
                    let mut terrain = original.clone();
                    ui.label(RichText::new("Terrain").strong());
                    let change = ui.text_edit_singleline(&mut terrain.nom).changed();
                    // volume du solide (épaisseur du type sous la surface) ; les cotes des
                    // sommets et points se modifient dans la vue (outil Modifier)
                    let e = p.type_terrain(&terrain).epaisseur;
                    match modele::elements::terre_plein::solide_terrain(p, &terrain, e) {
                        Ok(tris) => ui.label(format!("Volume : {:.2} m³", modele::volumes::geometrie::volume(&tris) / 1e9)),
                        Err(e) => ui.label(RichText::new(e).weak()),
                    };
                    if change { commande = Some(Commande::Terrain { terrain }); }
                }
                if let Some(original) = p.limites_propriete.iter().find(|l| l.id == ids[0]) {
                    let mut limite = original.clone();
                    ui.label(RichText::new("Limite de propriété").strong());
                    let mut change = ui.text_edit_singleline(&mut limite.nom).changed();
                    ui.horizontal(|ui| { ui.label("Altitude"); change |= ui.add(egui::DragValue::new(&mut limite.altitude).speed(10.0).suffix(" mm")).changed(); });
                    if change { commande = Some(Commande::LimitePropriete { limite }); }
                }
                if let Some(original) = p.vues.iter().find(|v| {
                    v.id == ids[0] && matches!(v.genre, modele::GenreVue::Coupe { .. })
                }) {
                    let mut vue = original.clone();
                    ui.label(RichText::new("Propriétés de la coupe / élévation").strong());
                    ui.scope(|ui| {
                        let mut change = ui.text_edit_singleline(&mut vue.nom).changed();
                        if let modele::GenreVue::Coupe {
                            debut,
                            fin,
                            profondeur,
                            ..
                        } = &mut vue.genre
                        {
                            for (nom, point) in [("Départ", debut), ("Arrivée", fin)] {
                                ui.horizontal(|ui| {
                                    ui.label(nom);
                                    for x in point {
                                        change |= ui
                                            .add(egui::DragValue::new(x).speed(10.0).dimensions())
                                            .changed();
                                    }
                                });
                            }
                            ui.horizontal(|ui| {
                                ui.label("Profondeur");
                                change |= ui
                                    .add(
                                        egui::DragValue::new(profondeur)
                                            .speed(10.0)
                                            .range(1.0..=200000.0)
                                            .dimensions(),
                                    )
                                    .changed();
                            });
                        }
                        if change {
                            commande = Some(Commande::ModifierVue { vue });
                        }
                    });
                }
                if let Some(original) = p.quadrillages.iter().find(|g| g.id == ids[0]) {
                    let mut quadrillage = original.clone();
                    ui.label(RichText::new("Propriétés du quadrillage").strong());
                    ui.scope(|ui| {
                        let mut change = ui.text_edit_singleline(&mut quadrillage.nom).changed();
                        for (nom, point) in [
                            ("Départ", &mut quadrillage.debut),
                            ("Arrivée", &mut quadrillage.fin),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(nom);
                                for x in point {
                                    change |= ui
                                        .add(egui::DragValue::new(x).speed(10.0).dimensions())
                                        .changed();
                                }
                            });
                        }
                        if change {
                            commande = Some(Commande::ModifierQuadrillage { quadrillage });
                        }
                    });
                }
                if let Some(original) = p.annotations.iter().find(|a| a.id == ids[0]) {
                    let mut annotation = original.clone();
                    ui.label(RichText::new("Propriétés de l'annotation").strong());
                    ui.scope(|ui| {
                        use modele::documentation::{AnnotationForme as F, PointCote};
                        let mut change = false;
                        match &mut annotation.forme {
                            F::Polyligne { points, fermee, courbures, .. } => {
                                let arcs = courbures.iter().filter(|k| **k != 0.0).count();
                                ui.label(if arcs == 0 { format!("{} points", points.len()) } else { format!("{} points · {arcs} arc(s)", points.len()) });
                                let avant = *fermee;
                                change |= ui.checkbox(fermee, "Fermée").changed();
                                // une courbure par segment : le segment de fermeture, ajouté ou retiré, est droit
                                if *fermee != avant && !courbures.is_empty() {
                                    courbures.resize(F::segments_polyligne(points.len(), *fermee), 0.0);
                                }
                            }
                            F::Spline { points, fermee } => {
                                ui.label(format!("{} points", points.len()));
                                change |= ui.checkbox(fermee, "Fermée").changed();
                            }
                            F::Cercle { centre, rayon } => {
                                ui.horizontal(|ui| {
                                    ui.label("Centre");
                                    for x in centre.iter_mut() {
                                        change |= ui.add(egui::DragValue::new(x).speed(10.0).dimensions()).changed();
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Rayon");
                                    change |= ui.add(egui::DragValue::new(rayon).speed(10.0).range(1.0..=f64::MAX).dimensions()).changed();
                                });
                            }
                            F::Arc { .. } => {
                                ui.label("Arc par trois points");
                            }
                            F::CoteAngulaire { sommet, debut, fin, rayon, .. } => {
                                if let Some((a, ..)) = modele::documentation::angle_branches((*sommet).into(), (*debut).into(), (*fin).into()) {
                                    ui.label(format!("Angle : {:.2}°", a.to_degrees()));
                                }
                                ui.horizontal(|ui| {
                                    ui.label("Rayon de l'arc");
                                    change |= ui.add(egui::DragValue::new(rayon).speed(10.0).range(1.0..=f64::MAX).dimensions()).changed();
                                });
                            }
                            F::CoteDiametre { centre, point } => {
                                ui.label(format!("Diamètre : {}", modele::unite::unite().afficher(2.0 * glam::DVec2::from(*centre).distance((*point).into()))));
                                ui.horizontal(|ui| {
                                    ui.label("Centre");
                                    for x in centre.iter_mut() {
                                        change |= ui.add(egui::DragValue::new(x).speed(10.0).dimensions()).changed();
                                    }
                                });
                            }
                            F::Altimetrie { relative_niveau, decimales, .. } => {
                                egui::ComboBox::from_label("Référence").selected_text(if *relative_niveau { "Niveau sous le point" } else { "Projet" }).show_ui(ui, |ui| {
                                    change |= ui.selectable_value(relative_niveau, false, "Projet").changed();
                                    change |= ui.selectable_value(relative_niveau, true, "Niveau sous le point").changed();
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Décimales (m)");
                                    change |= ui.add(egui::DragValue::new(decimales).range(0..=4)).changed();
                                });
                            }
                            F::Texte { point, texte } => {
                                change |= crate::outils::annoter::outils_texte::proprietes_texte(ui, texte, point, false, crate::outils::annoter::outils_texte::CHAMPS_PROJET);
                                change |= crate::outils::annoter::outils_texte::hauteur_texte(ui, &mut annotation.hauteur);
                            }
                            F::Etiquette { point, .. } => {
                                ui.horizontal(|ui| {
                                    ui.label("Position");
                                    for x in point {
                                        change |= ui
                                            .add(egui::DragValue::new(x).speed(10.0).dimensions())
                                            .changed();
                                    }
                                });
                            }
                            F::Ligne { debut, fin } => {
                                for (nom, point) in [("Départ", debut), ("Arrivée", fin)] {
                                    ui.horizontal(|ui| {
                                        ui.label(nom);
                                        for x in point {
                                            change |= ui
                                                .add(
                                                    egui::DragValue::new(x)
                                                        .speed(10.0)
                                                        .dimensions(),
                                                )
                                                .changed();
                                        }
                                    });
                                }
                            }
                            F::Cote {
                                debut,
                                fin,
                                decalage,
                            } => {
                                ui.horizontal(|ui| {
                                    ui.label("Décalage");
                                    change |= ui
                                        .add(
                                            egui::DragValue::new(decalage).speed(10.0).dimensions(),
                                        )
                                        .changed();
                                });
                                for (nom, point) in [("Départ", debut), ("Arrivée", fin)] {
                                    if let PointCote::Fixe { point } = point {
                                        ui.horizontal(|ui| {
                                            ui.label(nom);
                                            for x in point {
                                                change |= ui
                                                    .add(
                                                        egui::DragValue::new(x)
                                                            .speed(10.0)
                                                            .dimensions(),
                                                    )
                                                    .changed();
                                            }
                                        });
                                    }
                                }
                            }
                            F::Zone { contour } => {
                                for (i, point) in contour.iter_mut().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("Sommet {}", i + 1));
                                        for x in point {
                                            change |= ui
                                                .add(
                                                    egui::DragValue::new(x)
                                                        .speed(10.0)
                                                        .dimensions(),
                                                )
                                                .changed();
                                        }
                                    });
                                }
                            }
                        }
                        // une polyligne ouverte perd son remplissage
                        if let F::Polyligne { fermee: false, remplissage, .. } = &mut annotation.forme {
                            *remplissage = None;
                        }
                        if change {
                            commande = Some(Commande::Annoter { annotation });
                        }
                    });
                }
                if let Some(original) = p.pieces.iter().find(|x| x.id == ids[0]) {
                    let mut x = original.clone();
                    ui.label(RichText::new("Pièce").strong());
                    let mut change = false;
                    egui::Grid::new("proprietes_piece").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Nom");
                        change |= ui.text_edit_singleline(&mut x.nom).changed();
                        ui.end_row();
                        ui.label("Numéro");
                        change |= ui.text_edit_singleline(&mut x.numero).changed();
                        ui.end_row();
                        ui.label("Surface");
                        ui.label(if x.fermee() { format!("{:.2} m²", x.aire() / 1e6) } else { "Non fermée".into() });
                        ui.end_row();
                        ui.label("Périmètre");
                        ui.label(modele::unite::unite().afficher(x.perimetre()));
                        ui.end_row();
                        ui.label("Étiquette");
                        change |= choix_famille_etiquette(ui, "etiquette_piece_selection", &mut x.etiquette, &p.familles_etiquettes);
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Nouvelle famille").on_hover_text("Nouvelle famille d'étiquette, sur la base de celle-ci, affectée à cette pièce").clicked() {
                            ouvrir_etiquette = Some((None, Some(x.id.clone())));
                        }
                    });
                    crate::zones::donnees_importees::afficher(ui, &x.id, &x.donnees);
                    if change && !x.nom.trim().is_empty() && !x.numero.trim().is_empty() {
                        commande = Some(Commande::Piece { piece: x });
                    }
                }
                if let Some(original) = p.composants.iter().find(|x| x.id == ids[0]) {
                    let mut x = original.clone();
                    ui.label(RichText::new("Composant").strong());
                    let mut change = false;
                    egui::Grid::new("proprietes_composant").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Nom");
                        change |= ui.text_edit_singleline(&mut x.nom).changed();
                        ui.end_row();
                        ui.label("Type");
                        change |= choix_type_composant(ui, "type_composant_selection", &mut x.type_composant, &p.types_composants);
                        ui.end_row();
                        ui.label("Position");
                        ui.horizontal(|ui| {
                            for v in &mut x.position {
                                change |= ui.add(egui::DragValue::new(v).speed(10.0).dimensions()).changed();
                            }
                        });
                        ui.end_row();
                        ui.label("Décalage");
                        change |= ui.add(egui::DragValue::new(&mut x.decalage).speed(10.0).dimensions()).changed();
                        ui.end_row();
                        ui.label("Rotation");
                        change |= ui.add(egui::DragValue::new(&mut x.rotation).speed(1.0).suffix(" °")).changed();
                        ui.end_row();
                    });
                    if change {
                        commande = Some(Commande::Composant { composant: x });
                    }
                }
                if let Some(original) = p.barres.iter().find(|b| b.id == ids[0]) {
                    if self
                        .bim
                        .barre_editee
                        .as_ref()
                        .is_none_or(|b| b.id != original.id)
                    {
                        self.bim.barre_editee = Some(original.clone());
                    }
                    ui.label(RichText::new("Propriétés structurelles").strong());
                    ui.scope(|ui| {
                        let b = self.bim.barre_editee.as_mut().unwrap();
                        let avant = serde_json::to_string(&*b).ok();
                        barre_ui(ui, b, &p.niveaux);
                        if serde_json::to_string(&*b).ok() != avant {
                            commande = Some(Commande::Barre { barre: b.clone() });
                        }
                    });
                } else {
                    self.bim.barre_editee = None;
                }
            } else {
                self.bim.barre_editee = None;
            }
        } else if self.onglet_ruban == OngletRuban::Modeliser {
            match outil {
                Some(Outil::Niveau) => {
                    ui.heading("Niveaux");
                    let p = &self.natif.as_ref().unwrap().0;
                    for n in &p.niveaux {
                        ui.push_id(&n.id, |ui| {
                            let mut nom = n.nom.clone();
                            let mut altitude = n.altitude;
                            ui.horizontal(|ui| {
                                // largeur relative : l'altitude reste visible quel que soit le panneau
                                let largeur = (ui.available_width() - 90.0).clamp(40.0, 180.0);
                                ui.add(egui::TextEdit::singleline(&mut nom).desired_width(largeur));
                                ui.add(
                                    egui::DragValue::new(&mut altitude).speed(10.0).dimensions(),
                                );
                            });
                            if nom != n.nom || altitude != n.altitude {
                                commande = Some(Commande::ModifierNiveau {
                                    id: n.id.clone(),
                                    nom,
                                    altitude,
                                });
                            }
                        });
                    }
                    ui.label(RichText::new("Nom et altitude : valider par Entrée ou en cliquant ailleurs.").small().weak());
                    // nouveau niveau au-dessus du plus haut, à la hauteur du dernier étage
                    // (3 m s'il n'y en a qu'un)
                    let mut alts: Vec<f64> = p.niveaux.iter().map(|n| n.altitude).collect();
                    alts.sort_by(f64::total_cmp);
                    let etage = match alts.as_slice() {
                        [.., a, b] if b - a > 1.0 => b - a,
                        _ => 3000.0,
                    };
                    let altitude = alts.last().map_or(0.0, |h| h + etage);
                    let nom = if p.niveaux.is_empty() { "RDC".to_string() } else { (1..).map(|k| format!("R+{k}")).find(|n| !p.niveaux.iter().any(|x| &x.nom == n)).unwrap_or_default() };
                    if ui.button("Ajouter un niveau").on_hover_text(format!("« {nom} » à {}", modele::unite::unite().afficher(altitude))).clicked() {
                        commande = Some(Commande::Niveau { nom, altitude });
                    }
                }
                Some(Outil::Poutre) => self.panneau_poutre(ui),
                Some(Outil::Poteau) => commande = self.panneau_poteau(ui),
                Some(Outil::Composant) => {
                    let p = &self.natif.as_ref().unwrap().0;
                    let mut types = p.types_composants.clone();
                    let defaut = modele::elements::composant::type_table();
                    if !types.iter().any(|t| t.id == defaut.id) {
                        types.insert(0, defaut.clone());
                    }
                    let c = &mut self.bim.composant;
                    if !types.iter().any(|t| t.id == c.type_composant) {
                        c.type_composant = types[0].id.clone();
                    }
                    ui.label(RichText::new("Composant").strong());
                    egui::Grid::new("outil_composant").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Type");
                        choix_type_composant(ui, "type_composant_outil", &mut c.type_composant, &types);
                        ui.end_row();
                        ui.label("Rotation");
                        ui.add(egui::DragValue::new(&mut c.rotation).speed(1.0).suffix(" °"));
                        ui.end_row();
                        ui.label("Décalage");
                        ui.add(egui::DragValue::new(&mut c.decalage).speed(10.0).dimensions());
                        ui.end_row();
                    });
                    ui.label("Cliquez dans la vue pour placer le composant (niveau de la vue) ; Espace : rotation de 90°.");
                    let choisi = c.type_composant.clone();
                    ui.horizontal(|ui| {
                        if ui.button("Modifier la famille").on_hover_text("Ouvre la famille du type dans l'éditeur ; « Charger dans le projet » met à jour tous ses composants").clicked() {
                            ouvrir_composant = Some(Some(choisi));
                        }
                        if ui.button("Nouvelle famille").on_hover_text("Nouvelle famille de composant dans l'éditeur, sur la base de la table").clicked() {
                            ouvrir_composant = Some(None);
                        }
                    });
                }
                Some(Outil::Arbre) => {
                    use modele::elements::arbre::Essence;
                    let a = &mut self.bim.arbre;
                    ui.label(RichText::new("Arbre").strong());
                    egui::Grid::new("outil_arbre").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Essence");
                        let avant = a.essence;
                        egui::ComboBox::from_id_salt("essence_arbre").selected_text(a.essence.nom()).show_ui(ui, |ui| {
                            for e in Essence::TOUTES {
                                ui.selectable_value(&mut a.essence, e, e.nom());
                            }
                        });
                        if a.essence != avant {
                            (a.hauteur, a.diametre) = a.essence.dimensions();
                        }
                        ui.end_row();
                        ui.label("Hauteur");
                        ui.add(egui::DragValue::new(&mut a.hauteur).speed(50.0).range(500.0..=60_000.0).dimensions());
                        ui.end_row();
                        ui.label("Houppier").on_hover_text("Diamètre du houppier");
                        ui.add(egui::DragValue::new(&mut a.diametre).speed(50.0).range(300.0..=40_000.0).dimensions());
                        ui.end_row();
                        ui.label("Décalage").on_hover_text("Hauteur du pied par rapport au niveau de la vue");
                        ui.add(egui::DragValue::new(&mut a.decalage).speed(10.0).dimensions());
                        ui.end_row();
                    });
                    ui.label("Cliquez dans la vue pour planter l'arbre (niveau de la vue).");
                }
                Some(Outil::Piece) => {
                    ui.label(RichText::new("Pièce").strong());
                    ui.horizontal(|ui| {
                        ui.label("Nom");
                        ui.text_edit_singleline(&mut self.bim.nom_piece);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Étiquette");
                        let familles = self.natif.as_ref().map(|(p, _)| p.familles_etiquettes.clone()).unwrap_or_default();
                        choix_famille_etiquette(ui, "etiquette_piece_outil", &mut self.bim.etiquette_piece, &familles);
                        if ui.button("Modifier").on_hover_text("Éditer cette famille d'étiquette").clicked() {
                            ouvrir_etiquette = Some((Some(self.bim.etiquette_piece.clone()), None));
                        }
                    });
                    ui.label("Cliquez dans un espace fermé par des murs du niveau de la vue : le contour suit les faces intérieures ; le numéro s'incrémente.");
                }
                Some(Outil::Quadrillage) => {
                    points_ui(ui, &mut self.bim.points);
                    if ui.button("Créer le quadrillage droit").clicked() {
                        commande = Some(Commande::Quadrillage {
                            nom: self.ruban_edition.texte_option(Outil::Quadrillage, 0),
                            debut: self.bim.points[0],
                            fin: self.bim.points[1],
                        });
                    }
                }
                Some(Outil::Ouverture) => {
                    // type à placer (et éditeur de familles), en tête comme dans Revit
                    if self.ruban_edition.proprietes_ouverture(ui, self.natif.as_ref().map(|(p, _)| p)) {
                        if std::mem::take(&mut self.ruban_edition.demande_charger_famille)
                            && let Err(e) = self.charger_famille_dialogue(true)
                        {
                            self.error = Some(e);
                        }
                        if let Some(t) = self.ruban_edition.demande_famille.take() {
                            let genre = if self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Porte" { modele::GenreOuverture::Porte } else { modele::GenreOuverture::Fenetre };
                            self.ouvrir_editeur_famille_type(t, genre);
                        }
                    }
                    ui.label("Cliquez sur un mur pour placer l’ouverture, ou sélectionnez son hôte et saisissez l'abscisse.");
                    ui.add(
                        egui::DragValue::new(&mut self.bim.abscisse)
                            .speed(10.0)
                            .dimensions(),
                    );
                    if ui
                        .add_enabled(ids.len() == 1, egui::Button::new("Placer"))
                        .clicked()
                    {
                        match self.verifier_type_ouverture() {
                            Ok(()) => commande = Some(self.commande_ouverture(ids[0].clone(), self.bim.abscisse)),
                            Err(e) => self.error = Some(e),
                        }
                    }
                }
                _ => return false,
            }
        } else if self.onglet_ruban == OngletRuban::Annoter
            && matches!(
                outil,
                Some(Outil::Texte | Outil::Zone | Outil::Etiquette)
            )
        {
            use modele::documentation::{Annotation, AnnotationForme as F};
            let Some(vue) = self.vue_native_id() else {
                ui.label("Ouvrez un plan ou une coupe native pour annoter.");
                return true;
            };
            if outil == Some(Outil::Texte) {
                // même parcours que dans l'éditeur de famille : le clic dans la vue place le texte
                ui.label(RichText::new("Texte").strong());
                ui.horizontal(|ui| {
                    ui.label("Contenu");
                    ui.text_edit_singleline(&mut self.bim.texte);
                });
                ui.label("Cliquez dans la vue pour placer le texte. Il reste sélectionné : contenu, champs, position et hauteur se règlent dans Propriétés.");
                return true;
            }
            points_ui(ui, &mut self.bim.points);
            if ui.button("Utiliser le dernier point cliqué").clicked() {
                if let Some(pt) = self.point_clique {
                    let p = &self.natif.as_ref().unwrap().0;
                    let v = p.vues.iter().find(|v| v.id == vue).unwrap();
                    let (o, u, h) = modele::documentation::repere(p, v);
                    let d = pt * 1000.0 - o;
                    self.bim.points[0] = [d.dot(u), d.dot(h)];
                }
            }
            if ui.button("Ajouter à la vue").clicked() {
                let [a, b] = self.bim.points;
                let forme = match outil.unwrap() {
                    Outil::Texte => F::Texte {
                        point: a,
                        texte: self.bim.texte.clone(),
                    },
                    Outil::Etiquette => F::Etiquette {
                        element: ids.first().cloned().unwrap_or_default(),
                        point: a,
                    },
                    Outil::Zone => F::Zone {
                        contour: vec![a, [b[0], a[1]], b, [a[0], b[1]]],
                    },
                    _ => F::Ligne { debut: a, fin: b },
                };
                let mut forme = forme;
                let mut hauteur = 2.5;
                if let Some(modele) = &self.bim.annotation_identique {
                    hauteur = modele.hauteur;
                    match &modele.forme {
                        F::Etiquette { element, .. } => {
                            forme = F::Etiquette {
                                element: element.clone(),
                                point: a,
                            }
                        }
                        F::Zone { contour } if !contour.is_empty() => {
                            let origine = contour[0];
                            forme = F::Zone {
                                contour: contour
                                    .iter()
                                    .map(|p| [p[0] - origine[0] + a[0], p[1] - origine[1] + a[1]])
                                    .collect(),
                            };
                        }
                        _ => {}
                    }
                }
                commande = Some(Commande::Annoter {
                    annotation: Annotation {
                        id: String::new(),
                        vue,
                        hauteur,
                        forme,
                    },
                });
            }
            ui.label("Une cote créée avec un mur sélectionné suit ses extrémités.");
        } else {
            return false;
        }
        if let Some((famille, piece)) = ouvrir_etiquette {
            self.editer_famille_etiquette(famille, piece);
        }
        if let Some(t) = ouvrir_composant {
            self.ouvrir_editeur_composant(t);
        }
        if let Some(c) = commande {
            if let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
        }
        self.onglet_ruban != OngletRuban::Modifier
    }
}
