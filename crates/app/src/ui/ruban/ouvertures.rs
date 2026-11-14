//! Sélecteur de type d'ouverture, sur le modèle du sélecteur de type de mur : onglets
//! de catégorie (porte, fenêtre, libre), types en liste avec vignette et cotes, crayon
//! (modifier la famille) au survol, nouvelle famille en fin de liste.
use super::*;
use modele::GenreOuverture;

/// Ce que l'utilisateur a demandé dans le sélecteur.
#[derive(Default)]
pub struct ChoixOuverture {
    /// Type choisi (None : générique), avec sa catégorie.
    pub type_choisi: Option<(Option<String>, GenreOuverture)>,
    /// Famille du type à ouvrir dans l'éditeur.
    pub editer: Option<(String, GenreOuverture)>,
    /// Charger une famille depuis un fichier (`.famille.json`).
    pub charger: bool,
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
    /// dans l'onglet Libre (portes et fenêtres ont toujours une famille). En fin de
    /// liste : charger une famille depuis un fichier.
    pub fn selecteur_type_ouverture(&mut self, ui: &mut egui::Ui, projet: &modele::Projet, genre: GenreOuverture, actuel: Option<&str>, categories: &[GenreOuverture], generique: bool) -> ChoixOuverture {
        let mut sortie = ChoixOuverture::default();
        let unite = crate::config::options::unite();
        let type_actuel = actuel.and_then(|id| projet.types_ouvertures.iter().find(|t| t.id == id));
        let genre_actuel = type_actuel.map_or(genre, |t| t.genre);
        let onglet = self.categorie_ouverture.filter(|c| categories.contains(c)).unwrap_or(genre_actuel);
        // (id, nom, largeur, hauteur)
        type Ligne = (Option<String>, String, f64, f64);
        let lignes_de = |g: GenreOuverture| -> Vec<Ligne> {
            let mut v: Vec<Ligne> = Vec::new();
            if generique && g == GenreOuverture::Libre {
                v.push((None, "Générique (cotes du ruban)".into(), 0.0, 0.0));
            }
            let mut t: Vec<_> = projet.types_ouvertures.iter().filter(|t| t.genre == g).collect();
            t.sort_by(|a, b| a.nom.cmp(&b.nom));
            v.extend(t.into_iter().map(|t| (Some(t.id.clone()), t.nom.clone(), t.largeur, t.hauteur)));
            v
        };
        let lignes = lignes_de(onglet);
        let rangs = categories.iter().map(|g| lignes_de(*g).len()).max().unwrap_or(1);
        // Sélecteur fermé : vignette, catégorie · nom, cotes.
        let (nom, cotes) = match type_actuel {
            Some(t) => (t.nom.clone(), Some(format!("{} × {}", unite.afficher(t.largeur), unite.afficher(t.hauteur)))),
            None if genre_actuel == GenreOuverture::Libre => ("Générique (cotes du ruban)".into(), None),
            None => ("Aucun type : charger une famille".into(), None),
        };
        let (l, h) = type_actuel.map_or(if genre_actuel == GenreOuverture::Porte { (900.0, 2150.0) } else { (1200.0, 1350.0) }, |t| (t.largeur, t.hauteur));
        let titre = format!("{} · {nom}", nom_genre(genre_actuel));
        let vignette = |p: &egui::Painter, r: egui::Rect, c: Color32| vignette_ouverture(p, r, genre_actuel, l, h, c);
        let bouton = selecteur::entete(ui, &titre, cotes.as_deref(), &format!("{titre}
Type de l'ouverture : il impose ses cotes et sa famille."), Some(&vignette));
        let noms: Vec<String> = categories.iter().flat_map(|g| lignes_de(*g)).map(|l| l.1).collect();
        let largeur_liste = selecteur::largeur_liste(ui, noms.iter(), 110.0, categories.len(), 96.0);
        let mut choix_onglet = None;
        selecteur::liste(&bouton, largeur_liste, selecteur::HAUTEUR_CADRE + rangs as f32 * selecteur::HAUTEUR_LIGNE, |ui| {
            choix_onglet = selecteur::onglets(ui, categories, onglet, 96.0, |c| nom_genre(c).into(), aide_genre(onglet), |p, r, c, encre| {
                let (l, h) = if c == GenreOuverture::Porte { (900.0, 2150.0) } else { (1200.0, 1100.0) };
                vignette_ouverture(p, r, c, l, h, encre);
            });
            let modifier: &[(selecteur::ActionType, &str)] = &[(selecteur::ActionType::Modifier, "Modifier la famille du type")];
            for ligne in &lignes {
                let l = selecteur::Ligne {
                    nom: &ligne.1,
                    courant: ligne.0.as_deref() == actuel && (ligne.0.is_some() || onglet == genre_actuel),
                    etiquette: None,
                    sous_titre: None,
                    detail: ligne.0.as_ref().map(|_| format!("{} × {}", unite.afficher(ligne.2), unite.afficher(ligne.3))),
                    actions: if generique && onglet != GenreOuverture::Libre && ligne.0.is_some() { modifier } else { &[] },
                };
                let mut v = |ui: &mut egui::Ui, r: egui::Rect| {
                    if ligne.0.is_some() {
                        vignette_ouverture(ui.painter(), r.expand2(egui::vec2(-5.0, 2.0)), onglet, ligne.2, ligne.3, ui.visuals().text_color());
                    }
                };
                let clic = selecteur::ligne(ui, &l, Some(&mut v));
                if clic.action == Some(selecteur::ActionType::Modifier) && let Some(id) = &ligne.0 {
                    sortie.editer = Some((id.clone(), onglet));
                } else if clic.choisi {
                    sortie.type_choisi = Some((ligne.0.clone(), onglet));
                }
            }
            if onglet != GenreOuverture::Libre {
                sortie.charger = selecteur::ligne_ajout(ui, "Charger une famille…");
            }
        });
        if let Some(c) = choix_onglet {
            self.categorie_ouverture = Some(c);
        }
        if sortie.type_choisi.is_some() || sortie.editer.is_some() || sortie.charger {
            self.categorie_ouverture = None;
        }
        sortie
    }
}
