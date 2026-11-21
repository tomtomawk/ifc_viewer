//! Fiche d'un objet sélectionné (onglet Modifier) : le même schéma que la palette de
//! création. Chaque objet déclare son sélecteur (son type, ou un en-tête « Générique » quand
//! il n'a pas de type) puis ses paramètres. `fiche` est un `match` exhaustif sur `Genre`.
//! Les murs, éléments horizontaux, ouvertures, garde-corps, terrains et composants gardent le
//! sélecteur de type de `saisie/selection.rs` (`selecteur_type_selection`).
use crate::*;
use commandes::Commande;
use crate::zones::palette_creation::titre_parametres;
use crate::zones::ruban::selecteur;
use crate::outils::modeliser::bim::widgets::{barre_ui, choix_famille_etiquette};

type EmplacementObjet = fn(&mut Application, &mut egui::Ui, &str);

struct Fiche {
    selecteur: EmplacementObjet,
    parametres: EmplacementObjet,
}

#[derive(Clone, Copy)]
enum Genre {
    Terrain,
    Limite,
    Coupe,
    Quadrillage,
    Annotation,
    Piece,
    Composant,
    Poteau,
    Niveau,
}

fn genre(p: &modele::Projet, id: &str) -> Option<Genre> {
    if p.terrains.iter().any(|x| x.id == id) {
        Some(Genre::Terrain)
    } else if p.limites_propriete.iter().any(|x| x.id == id) {
        Some(Genre::Limite)
    } else if p.vues.iter().any(|v| v.id == id && matches!(v.genre, modele::GenreVue::Coupe { .. })) {
        Some(Genre::Coupe)
    } else if p.quadrillages.iter().any(|x| x.id == id) {
        Some(Genre::Quadrillage)
    } else if p.annotations.iter().any(|x| x.id == id) {
        Some(Genre::Annotation)
    } else if p.pieces.iter().any(|x| x.id == id) {
        Some(Genre::Piece)
    } else if p.composants.iter().any(|x| x.id == id) {
        Some(Genre::Composant)
    } else if p.barres.iter().any(|x| x.id == id) {
        Some(Genre::Poteau)
    } else if p.niveaux.iter().any(|x| x.id == id) {
        Some(Genre::Niveau)
    } else {
        None
    }
}

fn fiche(g: Genre) -> Fiche {
    match g {
        // sélecteur et paramètres : `selecteur_type_selection` et `proprietes_terrains`
        Genre::Terrain => Fiche { selecteur: deja_present, parametres: deja_present },
        Genre::Limite => Fiche { selecteur: selecteur_limite, parametres: parametres_limite },
        Genre::Coupe => Fiche { selecteur: selecteur_coupe, parametres: parametres_coupe },
        Genre::Quadrillage => Fiche { selecteur: selecteur_quadrillage, parametres: parametres_quadrillage },
        Genre::Annotation => Fiche { selecteur: selecteur_annotation, parametres: parametres_annotation },
        Genre::Piece => Fiche { selecteur: selecteur_piece, parametres: parametres_piece },
        Genre::Composant => Fiche { selecteur: deja_present, parametres: parametres_composant },
        Genre::Poteau => Fiche { selecteur: selecteur_poteau, parametres: parametres_poteau },
        Genre::Niveau => Fiche { selecteur: selecteur_niveau, parametres: parametres_niveau },
    }
}

impl Application {
    /// Fiche de l'objet sélectionné seul (onglet Modifier) : sélecteur puis paramètres.
    pub(crate) fn fiche_selection(&mut self, ui: &mut egui::Ui) {
        if self.natif.is_none() || self.onglet_ruban != OngletRuban::Modifier {
            return;
        }
        let ids = self.selection_bim();
        let genre = match (ids.as_slice(), &self.natif) {
            ([id], Some((p, _))) => genre(p, id),
            _ => None,
        };
        if !matches!(genre, Some(Genre::Poteau)) {
            self.bim.barre_editee = None;
        }
        let Some(genre) = genre else { return };
        let f = fiche(genre);
        (f.selecteur)(self, ui, &ids[0]);
        (f.parametres)(self, ui, &ids[0]);
    }
}

fn executer(a: &mut Application, commande: Option<Commande>) {
    if let Some(c) = commande
        && let Err(e) = a.executer_bim(c)
    {
        a.error = Some(e);
    }
}

/// Objet sans type : en-tête standard « objet · Générique », pour que chaque objet ait un
/// sélecteur au même endroit.
fn entete_generique(ui: &mut egui::Ui, objet: &str, detail: Option<&str>) {
    selecteur::entete_fixe(ui, &format!("{objet} · {}", detail.unwrap_or("Générique")), "Cet objet n'a pas de type à choisir.");
}

/// Déjà affiché plus haut (`selecteur_type_selection`, `proprietes_terrains`).
fn deja_present(_: &mut Application, _: &mut egui::Ui, _: &str) {}

fn selecteur_limite(_: &mut Application, ui: &mut egui::Ui, _: &str) {
    entete_generique(ui, "Limite de propriété", None);
}
fn selecteur_quadrillage(_: &mut Application, ui: &mut egui::Ui, _: &str) {
    entete_generique(ui, "Quadrillage", None);
}
fn selecteur_poteau(_: &mut Application, ui: &mut egui::Ui, _: &str) {
    entete_generique(ui, "Poteau", Some("Section rectangulaire"));
}
/// Coupe ou élévation : type de vue du projet (même liste qu'à la création, de la même
/// sorte) ; le type choisi donne aussi son échelle.
fn selecteur_coupe(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let Some(vue) = a.natif.as_ref().and_then(|(p, _)| p.vues.iter().find(|v| v.id == id)).cloned() else { return };
    let modele::GenreVue::Coupe { elevation, .. } = vue.genre else { return };
    let types: Vec<modele::gabarit::TypeVue> = a.natif.as_ref().map_or_else(modele::gabarit::TypeVue::par_defaut, |(p, _)| p.types_de_vues()).into_iter().filter(|t| t.elevation == elevation).collect();
    let sorte = if elevation { "Élévation" } else { "Coupe" };
    let nom = if vue.type_vue.is_empty() { "Générique" } else { vue.type_vue.as_str() };
    let titre = format!("{sorte} · {nom}");
    let vignette = |p: &egui::Painter, r: egui::Rect, c: Color32| crate::zones::ruban::icone_vue(p, r, elevation, c);
    let bouton = selecteur::entete(ui, &titre, Some(&format!("1:{}", vue.echelle)), &format!("{titre}\nType de la vue : le choisir règle aussi son échelle."), Some(&vignette));
    let largeur = selecteur::largeur_liste(ui, types.iter().map(|t| t.aide.as_str()), 90.0, 0, 0.0);
    let mut choix = None;
    selecteur::liste(&bouton, largeur, selecteur::HAUTEUR_CADRE.min(types.len().max(1) as f32 * selecteur::HAUTEUR_LIGNE_DOUBLE + 8.0), |ui| {
        for t in &types {
            let detail = format!("1:{}", t.echelle);
            let l = selecteur::Ligne { nom: &t.nom, courant: t.nom == vue.type_vue, etiquette: None, sous_titre: Some(&t.aide), detail: Some(detail), actions: &[] };
            if selecteur::ligne(ui, &l, None).choisi {
                choix = Some(t.clone());
                ui.close();
            }
        }
    });
    let commande = choix.filter(|t| t.nom != vue.type_vue || t.echelle != vue.echelle).map(|t| Commande::ModifierVue { vue: modele::Vue { type_vue: t.nom, echelle: t.echelle, ..vue } });
    executer(a, commande);
}
/// Niveau : pas de type de niveau dans le modèle, en-tête « Générique ».
fn selecteur_niveau(_: &mut Application, ui: &mut egui::Ui, _: &str) {
    entete_generique(ui, "Niveau", None);
}
fn parametres_niveau(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let Some(n) = a.natif.as_ref().and_then(|(p, _)| p.niveaux.iter().find(|n| n.id == id)).cloned() else { return };
    titre_parametres(ui, "Niveau");
    let (mut nom, mut altitude) = (n.nom.clone(), n.altitude);
    egui::Grid::new("niveau_selection").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
        ui.label("Nom");
        ui.text_edit_singleline(&mut nom);
        ui.end_row();
        ui.label("Altitude");
        ui.add(egui::DragValue::new(&mut altitude).speed(10.0).dimensions());
        ui.end_row();
    });
    let commande = (nom != n.nom || altitude != n.altitude).then(|| Commande::ModifierNiveau { id: n.id.clone(), nom, altitude });
    executer(a, commande);
}
/// Annotation : son genre. Les dessins 2D (polyligne, spline, cercle, arc) ont déjà leur sélecteur
/// de type de trait dans `proprietes_formes_2d`.
fn selecteur_annotation(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    use modele::documentation::AnnotationForme as F;
    if a.natif.as_ref().and_then(|(p, _)| p.annotations.iter().find(|x| x.id == id)).is_some_and(|x| x.forme.forme_2d().is_some()) {
        return;
    }
    let nom = a.natif.as_ref().and_then(|(p, _)| p.annotations.iter().find(|x| x.id == id)).map(|x| match x.forme {
        F::Texte { .. } => "Texte",
        F::Ligne { .. } | F::Polyligne { .. } | F::Arc { .. } => "Polyligne",
        F::Spline { .. } => "Spline",
        F::Cercle { .. } => "Cercle",
        F::Zone { .. } => "Zone remplie",
        F::Cote { .. } => "Cote",
        F::Etiquette { .. } => "Étiquette",
        F::CoteAngulaire { .. } => "Cote angulaire",
        F::CoteDiametre { .. } => "Cote de diamètre",
        F::Altimetrie { .. } => "Altimétrie",
    });
    entete_generique(ui, "Annotation", nom);
}
/// Pièce : famille d'étiquette (son type).
fn selecteur_piece(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let Some((p, _)) = &a.natif else { return };
    let Some(original) = p.pieces.iter().find(|x| x.id == id) else { return };
    let mut x = original.clone();
    let change = choix_famille_etiquette(ui, &mut x.etiquette, &p.familles_etiquettes);
    let nouvelle = ui.button("Nouvelle famille d'étiquette").on_hover_text("Nouvelle famille d'étiquette, sur la base de celle-ci, affectée à cette pièce").clicked();
    let commande = (change && !x.nom.trim().is_empty() && !x.numero.trim().is_empty()).then(|| Commande::Piece { piece: x.clone() });
    executer(a, commande);
    if nouvelle {
        a.editer_famille_etiquette(None, Some(id.to_string()));
    }
}

fn parametres_piece(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let mut commande: Option<Commande> = None;
    {
        let Some((p, _)) = &a.natif else { return };
        let Some(original) = p.pieces.iter().find(|x| x.id == id) else { return };
        let mut x = original.clone();
        titre_parametres(ui, "Pièce");
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
        });
        crate::zones::donnees_importees::afficher(ui, &x.id, &x.donnees);
        if change && !x.nom.trim().is_empty() && !x.numero.trim().is_empty() {
            commande = Some(Commande::Piece { piece: x });
        }
    }
    executer(a, commande);
}

fn parametres_composant(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let mut commande: Option<Commande> = None;
    {
        let Some((p, _)) = &a.natif else { return };
        let Some(original) = p.composants.iter().find(|x| x.id == id) else { return };
        let mut x = original.clone();
        titre_parametres(ui, "Composant");
        let mut change = false;
        egui::Grid::new("proprietes_composant").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Nom");
            change |= ui.text_edit_singleline(&mut x.nom).changed();
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
    executer(a, commande);
}

fn parametres_limite(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let mut commande: Option<Commande> = None;
    {
        let Some((p, _)) = &a.natif else { return };
    if let Some(original) = p.limites_propriete.iter().find(|l| l.id == id) {
        let mut limite = original.clone();
        titre_parametres(ui, "Limite de propriété");
        let mut change = ui.text_edit_singleline(&mut limite.nom).changed();
        ui.horizontal(|ui| { ui.label("Altitude"); change |= ui.add(egui::DragValue::new(&mut limite.altitude).speed(10.0).suffix(" mm")).changed(); });
        if change { commande = Some(Commande::LimitePropriete { limite }); }
    }
    }
    executer(a, commande);
}

fn parametres_coupe(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let mut commande: Option<Commande> = None;
    {
        let Some((p, _)) = &a.natif else { return };
    if let Some(original) = p.vues.iter().find(|v| {
        v.id == id && matches!(v.genre, modele::GenreVue::Coupe { .. })
    }) {
        let mut vue = original.clone();
        titre_parametres(ui, "Coupe / élévation");
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
    }
    executer(a, commande);
}

fn parametres_quadrillage(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let mut commande: Option<Commande> = None;
    {
        let Some((p, _)) = &a.natif else { return };
    if let Some(original) = p.quadrillages.iter().find(|g| g.id == id) {
        let mut quadrillage = original.clone();
        titre_parametres(ui, "Quadrillage");
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
    }
    executer(a, commande);
}

fn parametres_annotation(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let mut commande: Option<Commande> = None;
    {
        let Some((p, _)) = &a.natif else { return };
    if let Some(original) = p.annotations.iter().find(|a| a.id == id) {
        let mut annotation = original.clone();
        titre_parametres(ui, "Annotation");
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
                F::Spline { points, fermee, .. } => {
                    ui.label(format!("{} points", points.len()));
                    change |= ui.checkbox(fermee, "Fermée").changed();
                }
                F::Cercle { centre, rayon, .. } => {
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
    }
    executer(a, commande);
}

fn parametres_poteau(a: &mut Application, ui: &mut egui::Ui, id: &str) {
    let mut commande: Option<Commande> = None;
    {
        let Some((p, _)) = &a.natif else { return };
    if let Some(original) = p.barres.iter().find(|b| b.id == id) {
        if a.bim.barre_editee.as_ref().is_none_or(|b| b.id != original.id) {
            a.bim.barre_editee = Some(original.clone());
        }
        titre_parametres(ui, "Poteau");
        ui.scope(|ui| {
            let Some(b) = a.bim.barre_editee.as_mut() else { return };
            let avant = serde_json::to_string(&*b).ok();
            barre_ui(ui, b, &p.niveaux);
            if serde_json::to_string(&*b).ok() != avant {
                commande = Some(Commande::Barre { barre: b.clone() });
            }
        });
    }
    }
    executer(a, commande);
}
