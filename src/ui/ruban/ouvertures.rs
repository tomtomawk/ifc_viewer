//! Sélecteur de type d'ouverture, sur le modèle du sélecteur de type de mur : onglets
//! de catégorie (porte, fenêtre, libre), types en liste avec vignette et cotes, crayon
//! (modifier la famille) au survol, nouvelle famille en fin de liste.
use super::*;
use crate::modele::GenreOuverture;

/// Ce que l'utilisateur a demandé dans le sélecteur.
#[derive(Default)]
pub struct ChoixOuverture {
    /// Type choisi (None : générique), avec sa catégorie.
    pub type_choisi: Option<(Option<String>, GenreOuverture)>,
    /// Famille du type à ouvrir dans l'éditeur.
    pub editer: Option<(String, GenreOuverture)>,
    /// Nouvelle famille de cette catégorie.
    pub nouveau: Option<GenreOuverture>,
    /// Type du gabarit choisi, à charger dans le projet.
    pub gabarit: Option<crate::config::gabarit::TypeOuvertureGabarit>,
}

pub(crate) fn nom_genre(g: GenreOuverture) -> &'static str {
    match g {
        GenreOuverture::Porte => "Porte",
        GenreOuverture::Fenetre => "Fenêtre",
        GenreOuverture::Libre => "Libre",
    }
}

fn aide_genre(g: GenreOuverture) -> &'static str {
    match g {
        GenreOuverture::Porte => "Porte : vantail posé au sol dans un mur hôte.",
        GenreOuverture::Fenetre => "Fenêtre : baie à une hauteur d'allège dans un mur hôte.",
        GenreOuverture::Libre => "Libre : réservation ou baie sans menuiserie.",
    }
}

/// Vignette en élévation d'une ouverture, aux proportions largeur × hauteur.
pub(crate) fn vignette_ouverture(p: &egui::Painter, r: egui::Rect, genre: GenreOuverture, largeur: f64, hauteur: f64, encre: Color32) {
    let ratio = (largeur / hauteur.max(1.0)).clamp(0.2, 5.0) as f32;
    let (l, h) = if ratio * r.height() <= r.width() { (ratio * r.height(), r.height()) } else { (r.width(), r.width() / ratio) };
    let b = egui::Rect::from_center_size(r.center(), egui::vec2(l, h));
    let trait_ = egui::Stroke::new(1.2, encre);
    match genre {
        GenreOuverture::Porte => {
            p.rect_stroke(b, 0.0, trait_, egui::StrokeKind::Inside);
            p.rect_filled(b.shrink(2.0), 0.0, encre.gamma_multiply(0.15));
            p.circle_filled(egui::pos2(b.right() - l * 0.22, b.center().y), 1.3, encre);
        }
        GenreOuverture::Fenetre => {
            p.rect_stroke(b, 0.0, trait_, egui::StrokeKind::Inside);
            p.rect_filled(b.shrink(2.0), 0.0, Color32::from_rgb(120, 170, 220).gamma_multiply(0.45));
            p.line_segment([b.center_top(), b.center_bottom()], trait_);
        }
        GenreOuverture::Libre => {
            p.rect_stroke(b, 0.0, egui::Stroke::new(1.0, encre.gamma_multiply(0.7)), egui::StrokeKind::Inside);
            p.line_segment([b.left_top(), b.right_bottom()], egui::Stroke::new(0.8, encre.gamma_multiply(0.5)));
            p.line_segment([b.right_top(), b.left_bottom()], egui::Stroke::new(0.8, encre.gamma_multiply(0.5)));
        }
    }
}

impl RubanEdition {
    /// Sélecteur de type d'ouverture. `actuel` : type courant (None : générique) ;
    /// `categories` : onglets proposés ; `generique` : ligne « Générique » proposée
    /// dans l'onglet Libre (portes et fenêtres ont toujours une famille). Les types du
    /// gabarit absents du projet suivent ceux du projet et y sont chargés au choix.
    pub fn selecteur_type_ouverture(&mut self, ui: &mut egui::Ui, projet: &crate::modele::Projet, genre: GenreOuverture, actuel: Option<&str>, categories: &[GenreOuverture], generique: bool) -> ChoixOuverture {
        let mut sortie = ChoixOuverture::default();
        let unite = crate::config::options::unite();
        let type_actuel = actuel.and_then(|id| projet.types_ouvertures.iter().find(|t| t.id == id));
        let genre_actuel = type_actuel.map_or(genre, |t| t.genre);
        let onglet = self.categorie_ouverture.filter(|c| categories.contains(c)).unwrap_or(genre_actuel);
        let largeur = ui.available_width();
        // (id, nom, largeur, hauteur, type du gabarit à charger)
        type Ligne = (Option<String>, String, f64, f64, Option<crate::config::gabarit::TypeOuvertureGabarit>);
        let gabarit = crate::config::gabarit::actif();
        let lignes_de = |g: GenreOuverture| -> Vec<Ligne> {
            let mut v: Vec<Ligne> = Vec::new();
            if generique && g == GenreOuverture::Libre {
                v.push((None, "Générique (cotes du ruban)".into(), 0.0, 0.0, None));
            }
            let mut t: Vec<_> = projet.types_ouvertures.iter().filter(|t| t.genre == g).collect();
            t.sort_by(|a, b| a.nom.cmp(&b.nom));
            v.extend(t.into_iter().map(|t| (Some(t.id.clone()), t.nom.clone(), t.largeur, t.hauteur, None)));
            v.extend(gabarit.types_ouvertures.iter().filter(|t| t.famille.genre == g && !projet.types_ouvertures.iter().any(|p| p.nom == t.nom)).map(|t| (None, t.nom.clone(), t.largeur, t.hauteur, Some(t.clone()))));
            v
        };
        let lignes = lignes_de(onglet);
        let rangs = categories.iter().map(|g| lignes_de(*g).len()).max().unwrap_or(1);
        let police = egui::FontId::proportional(12.0);
        let largeur_texte = |ui: &egui::Ui, t: &str, f: &egui::FontId| ui.fonts_mut(|x| x.layout_no_wrap(t.to_string(), f.clone(), Color32::WHITE).size().x);
        // Sélecteur fermé : vignette, catégorie · nom, cotes.
        let (r, bouton) = ui.allocate_exact_size(egui::vec2(largeur, 34.0), Sense::click());
        let visuel = ui.style().interact(&bouton).clone();
        ui.painter().rect(r, 3.0, visuel.bg_fill, visuel.bg_stroke, egui::StrokeKind::Inside);
        let fleche = egui::Rect::from_center_size(egui::pos2(r.right() - 12.0, r.center().y), egui::vec2(8.0, 8.0));
        triangle_bas(ui.painter(), fleche.center(), visuel.fg_stroke.color);
        let (nom, cotes) = match type_actuel {
            Some(t) => (t.nom.clone(), Some(format!("{} × {}", unite.afficher(t.largeur), unite.afficher(t.hauteur)))),
            None if genre_actuel == GenreOuverture::Libre => ("Générique (cotes du ruban)".into(), None),
            None => ("Aucun type : choisir une famille".into(), None),
        };
        let mut droite = fleche.left() - 6.0;
        if let Some(c) = &cotes {
            ui.painter().text(egui::pos2(droite, r.center().y), egui::Align2::RIGHT_CENTER, c, egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
            droite -= largeur_texte(ui, c, &egui::FontId::proportional(11.0)) + 8.0;
        }
        let (l, h) = type_actuel.map_or(if genre_actuel == GenreOuverture::Porte { (900.0, 2150.0) } else { (1200.0, 1350.0) }, |t| (t.largeur, t.hauteur));
        vignette_ouverture(ui.painter(), egui::Rect::from_min_size(r.min + egui::vec2(8.0, 5.0), egui::vec2(28.0, 24.0)), genre_actuel, l, h, visuel.text_color());
        let titre = format!("{} · {nom}", nom_genre(genre_actuel));
        let zone = egui::Rect::from_min_max(egui::pos2(r.left() + 44.0, r.top()), egui::pos2(droite, r.bottom()));
        ui.painter().with_clip_rect(zone).text(egui::pos2(zone.left(), r.center().y), egui::Align2::LEFT_CENTER, &titre, egui::FontId::proportional(13.0), visuel.text_color());
        let bouton = bouton.on_hover_text(format!("{titre}\nType de l'ouverture : il impose ses cotes et sa famille."));
        let plus_long = categories.iter().flat_map(|g| lignes_de(*g)).map(|l| largeur_texte(ui, &l.1, &police)).fold(0.0f32, f32::max);
        let largeur_liste = (8.0 + 30.0 + 12.0 + plus_long + 16.0 + 110.0 + 8.0).max(largeur).max(categories.len() as f32 * 98.0 + 12.0);
        let mut choix_onglet = None;
        egui::Popup::from_toggle_button_response(&bouton)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .width(largeur_liste)
            .show(|ui| {
                ui.set_min_width(largeur_liste);
                // hauteur fixe (catégorie la plus fournie) : la liste ne saute pas d'un onglet à l'autre
                ui.set_min_height(116.0 + rangs as f32 * 31.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for &c in categories {
                        let (r, reponse) = ui.allocate_exact_size(egui::vec2(96.0, 50.0), Sense::click());
                        let actif = onglet == c;
                        if actif || reponse.hovered() {
                            ui.painter().rect_filled(r, 4.0, if actif { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                        }
                        let (l, h) = if c == GenreOuverture::Porte { (900.0, 2150.0) } else { (1200.0, 1100.0) };
                        vignette_ouverture(ui.painter(), egui::Rect::from_center_size(r.center_top() + egui::vec2(0.0, 17.0), egui::vec2(26.0, 26.0)), c, l, h, ui.visuals().text_color());
                        ui.painter().text(r.center_bottom() - egui::vec2(0.0, 9.0), egui::Align2::CENTER_CENTER, nom_genre(c), egui::FontId::proportional(12.0), ui.visuals().text_color());
                        if reponse.clicked() && !actif {
                            choix_onglet = Some(c);
                            ui.ctx().request_repaint();
                        }
                    }
                });
                ui.label(RichText::new(aide_genre(onglet)).small().weak());
                ui.separator();
                for ligne in &lignes {
                    let courant = ligne.4.is_none() && ligne.0.as_deref() == actuel && (ligne.0.is_some() || onglet == genre_actuel);
                    let (r, reponse) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), Sense::click());
                    let survol = ui.rect_contains_pointer(r);
                    if courant || survol {
                        ui.painter().rect_filled(r, 3.0, if courant { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                    }
                    if ligne.0.is_some() || ligne.4.is_some() {
                        vignette_ouverture(ui.painter(), egui::Rect::from_min_size(r.min + egui::vec2(8.0, 3.0), egui::vec2(30.0, 24.0)), onglet, ligne.2, ligne.3, ui.visuals().text_color());
                    }
                    ui.painter().text(r.min + egui::vec2(50.0, 15.0), egui::Align2::LEFT_CENTER, &ligne.1, police.clone(), ui.visuals().text_color());
                    if ligne.4.is_some() {
                        ui.painter().text(r.min + egui::vec2(58.0 + largeur_texte(ui, &ligne.1, &police), 15.0), egui::Align2::LEFT_CENTER, "gabarit", egui::FontId::proportional(10.0), ui.visuals().weak_text_color());
                    }
                    let mut icone_cliquee = false;
                    // au survol : crayon (modifier la famille) ; sinon les cotes
                    if survol && onglet != GenreOuverture::Libre && let Some(id) = &ligne.0 {
                        let zone = egui::Rect::from_min_max(egui::pos2(r.right() - 30.0, r.top() + 4.0), egui::pos2(r.right() - 8.0, r.bottom() - 4.0));
                        let rep = ui.interact(zone, ui.scope_id().with(("crayon_ouverture", id)), Sense::click()).on_hover_text("Modifier la famille du type");
                        if rep.hovered() {
                            ui.painter().rect_filled(zone, 3.0, ui.visuals().widgets.active.bg_fill);
                        }
                        let c = if rep.hovered() { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                        icone_action_type(ui.painter(), zone.shrink(4.0), true, c);
                        if rep.clicked() {
                            icone_cliquee = true;
                            sortie.editer = Some((id.clone(), onglet));
                            ui.close();
                        }
                    } else if ligne.0.is_some() || ligne.4.is_some() {
                        ui.painter().text(r.right_center() - egui::vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, format!("{} × {}", unite.afficher(ligne.2), unite.afficher(ligne.3)), egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
                    }
                    if reponse.clicked() && !icone_cliquee && let Some(t) = &ligne.4 {
                        sortie.gabarit = Some(t.clone());
                        ui.close();
                    } else if reponse.clicked() && !icone_cliquee {
                        sortie.type_choisi = Some((ligne.0.clone(), onglet));
                        ui.close();
                    }
                }
                if onglet != GenreOuverture::Libre {
                    ui.separator();
                    let (r, reponse) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::click());
                    if reponse.hovered() {
                        ui.painter().rect_filled(r, 3.0, ui.visuals().widgets.hovered.bg_fill);
                    }
                    ui.painter().text(r.min + egui::vec2(23.0, 13.0), egui::Align2::CENTER_CENTER, "+", egui::FontId::proportional(16.0), ui.visuals().strong_text_color());
                    ui.painter().text(r.min + egui::vec2(50.0, 13.0), egui::Align2::LEFT_CENTER, format!("Nouvelle famille de {}…", nom_genre(onglet).to_lowercase()), police.clone(), ui.visuals().text_color());
                    if reponse.clicked() {
                        sortie.nouveau = Some(onglet);
                        ui.close();
                    }
                }
            });
        if let Some(c) = choix_onglet {
            self.categorie_ouverture = Some(c);
        }
        if sortie.type_choisi.is_some() || sortie.editer.is_some() || sortie.nouveau.is_some() || sortie.gabarit.is_some() {
            self.categorie_ouverture = None;
        }
        sortie
    }
}
