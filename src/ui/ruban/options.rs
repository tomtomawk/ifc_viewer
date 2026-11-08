//! Propriétés des ouvertures et surfaces, options des outils.
use super::*;

impl RubanEdition {
    /// Sélecteur de création placé en tête de la palette Propriétés, comme un
    /// sélecteur de type : vignette, choix explicite, puis paramètres de l'outil.
    /// Retourne false pour laisser la palette afficher la sélection du modèle.
    /// Placement d'une ouverture : sélecteur de type en tête de Propriétés (générique :
    /// cotes du ruban), édition de la famille du type ou nouvelle famille.
    pub fn proprietes_ouverture(&mut self, ui: &mut egui::Ui, projet: Option<&crate::modele::Projet>) -> bool {
        if self.selection != Some(Outil::Ouverture) {
            return false;
        }
        use crate::modele::GenreOuverture;
        let genre = match self.texte_option(Outil::Ouverture, 0).as_str() {
            "Fenêtre" => GenreOuverture::Fenetre,
            "Libre" => GenreOuverture::Libre,
            _ => GenreOuverture::Porte,
        };
        let Some(projet) = projet else {
            ui.label("Créez ou ouvrez un projet natif.");
            return true;
        };
        if self.ouverture_type_identique.as_ref().is_some_and(|id| !projet.types_ouvertures.iter().any(|t| &t.id == id && t.genre == genre)) {
            self.ouverture_type_identique = None;
        }
        // porte et fenêtre : toujours une famille, la première du projet à défaut ; sans
        // famille dans le projet, il faut en charger une
        if self.ouverture_type_identique.is_none() && genre != GenreOuverture::Libre {
            let mut t: Vec<_> = projet.types_ouvertures.iter().filter(|t| t.genre == genre).collect();
            t.sort_by(|a, b| a.nom.cmp(&b.nom));
            if let Some(t) = t.first() {
                self.valeurs.insert((Outil::Ouverture, 1), Valeur::Nombre(t.largeur));
                self.valeurs.insert((Outil::Ouverture, 2), Valeur::Nombre(t.hauteur));
                self.ouverture_type_identique = Some(t.id.clone());
            }
        }
        // sélecteur de type à onglets de catégorie, comme celui des murs
        let actuel = self.ouverture_type_identique.clone();
        let choix = self.selecteur_type_ouverture(ui, projet, genre, actuel.as_deref(), &[GenreOuverture::Porte, GenreOuverture::Fenetre, GenreOuverture::Libre], true);
        let prendre_genre = |s: &mut Self, g: GenreOuverture| {
            s.valeurs.insert((Outil::Ouverture, 0), Valeur::Texte(super::ouvertures::nom_genre(g).into()));
        };
        if let Some((id, g)) = choix.type_choisi {
            prendre_genre(self, g);
            if let Some(t) = id.as_ref().and_then(|id| projet.types_ouvertures.iter().find(|t| &t.id == id)) {
                self.valeurs.insert((Outil::Ouverture, 1), Valeur::Nombre(t.largeur));
                self.valeurs.insert((Outil::Ouverture, 2), Valeur::Nombre(t.hauteur));
            }
            self.ouverture_type_identique = id;
        }
        if let Some((id, g)) = choix.editer {
            prendre_genre(self, g);
            self.demande_famille = Some(Some(id));
        }
        if choix.charger {
            self.demande_charger_famille = true;
        }
        ui.separator();
        true
    }

    pub fn proprietes_surfaces(&mut self, ui: &mut egui::Ui, projet: Option<&crate::modele::Projet>) -> bool {
        let Some(mut surface) = self.selection.filter(|o| o.commande() == Outil::Surfaces) else {
            return false;
        };
        let projet_vide = crate::modele::Projet::default();
        let projet_apercu = projet.unwrap_or(&projet_vide);
        ui.add_space(6.0);
        egui::Frame::group(ui.style())
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    let (vignette, _) =
                        ui.allocate_exact_size(egui::vec2(52.0, 58.0), Sense::hover());
                    ui.painter()
                        .rect_filled(vignette, 3.0, ui.visuals().faint_bg_color);
                    icone(
                        ui.painter(),
                        egui::Rect::from_center_size(vignette.center(), egui::vec2(36.0, 36.0)),
                        surface,
                        ui.visuals().text_color(),
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Éléments horizontaux").small().weak());
                        let types = types_surface(surface);
                        let k = self.type_surface.get(&surface).copied().unwrap_or(0).min(types.len().saturating_sub(1));
                        let nom_type = self.surface_identique.as_ref().filter(|(o,_)| *o == surface).map(|(_, t)| t.nom.as_str()).unwrap_or_else(|| types.get(k).map_or("", |t| t.0.as_str()));
                        let mut choix_type: Option<(Outil, usize)> = None;
                        egui::ComboBox::from_id_salt("surface_proprietes")
                            .width(ui.available_width())
                            .height(400.0)
                            // les onglets de sorte gardent la liste ouverte ; un type la ferme
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .truncate()
                            .selected_text(RichText::new(format!("{} · {nom_type}", surface.nom())).strong())
                            .show_ui(ui, |ui| {
                                ui.set_min_width(300.0);
                                // hauteur fixe (sorte la plus fournie) : la liste ne saute pas d'un onglet à l'autre
                                let rangs = SURFACES.iter().map(|(o, _)| types_surface(*o).len()).max().unwrap_or(0);
                                ui.set_min_height(86.0 + rangs as f32 * 27.0);
                                // onglets des sortes : choisir une sorte garde la liste ouverte
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    for &(choix, _) in SURFACES {
                                        let (r, reponse) = ui.allocate_exact_size(egui::vec2(74.0, 50.0), Sense::click());
                                        let actif = surface == choix;
                                        if actif || reponse.hovered() {
                                            ui.painter().rect_filled(r, 4.0, if actif { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                                        }
                                        icone(ui.painter(), egui::Rect::from_center_size(r.center_top() + egui::vec2(0.0, 17.0), egui::vec2(26.0, 26.0)), choix, ui.visuals().text_color());
                                        ui.painter().text(r.center_bottom() - egui::vec2(0.0, 9.0), egui::Align2::CENTER_CENTER, choix.nom(), egui::FontId::proportional(12.0), ui.visuals().text_color());
                                        if reponse.clicked() && !actif {
                                            choix_type = Some((choix, self.type_surface.get(&choix).copied().unwrap_or(0)));
                                            // liste redimensionnée à l'image suivante
                                            ui.ctx().request_repaint();
                                        }
                                    }
                                });
                                let aide = SURFACES.iter().find(|(o, _)| *o == surface).map_or("", |(_, a)| *a);
                                ui.label(RichText::new(aide).small().weak());
                                ui.separator();
                                // types de la sorte : composition, nom, épaisseur
                                for (i, (nom, couches)) in types.iter().enumerate() {
                                    let (r, reponse) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), Sense::click());
                                    if i == k || reponse.hovered() {
                                        ui.painter().rect_filled(r, 3.0, if i == k { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                                    }
                                    let e: f64 = couches.iter().map(|c| c.epaisseur).sum();
                                    pile_couches(ui, egui::Rect::from_min_size(r.min + egui::vec2(8.0, 4.0), egui::vec2(26.0, 16.0)), projet_apercu, couches);
                                    ui.painter().text(r.min + egui::vec2(42.0, 12.0), egui::Align2::LEFT_CENTER, nom, egui::FontId::proportional(12.0), ui.visuals().text_color());
                                    ui.painter().text(r.right_center() - egui::vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, crate::config::options::unite().afficher(e), egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
                                    if reponse.clicked() {
                                        choix_type = Some((surface, i));
                                        ui.close();
                                    }
                                }
                            });
                        if let Some((sorte, i)) = choix_type {
                            surface = sorte;
                            self.type_surface.insert(sorte, i);
                            self.surface_identique = None;
                        }
                        let aide = SURFACES
                            .iter()
                            .find(|(o, _)| *o == surface)
                            .map_or("", |(_, aide)| *aide);
                        ui.add(egui::Label::new(RichText::new(aide).small().weak()).wrap());
                    });
                });
                // composition du type choisi, du haut vers le bas
                let types = types_surface(surface);
                if let Some((_, couches)) = types.get(self.type_surface.get(&surface).copied().unwrap_or(0)) {
                    ui.add_space(4.0);
                    for c in couches {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 12.0), Sense::hover());
                            pile_couches(ui, r, projet_apercu, std::slice::from_ref(c));
                            ui.label(RichText::new(format!("{} · {}", c.materiau.as_deref().unwrap_or("Couche"), crate::config::options::unite().afficher(c.epaisseur))).small());
                            ui.label(RichText::new(c.fonction.libelle()).small().weak());
                        });
                    }
                }
            });
        self.selection = Some(surface);
        self.derniere_surface = Some(surface);
        ui.add_space(8.0);
        ui.label(RichText::new("Paramètres").strong());
        ui.separator();
        // tracé dans la vue (projet natif), niveau et mode de tracé
        {
            match projet {
                Some(projet) => {
                    if self.niveau_dalle.as_ref().is_some_and(|id| projet.niveau(id).is_none()) {
                        self.niveau_dalle = None;
                    }
                    egui::Grid::new("dalle_trace").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Niveau");
                        let nom = self.niveau_dalle.as_ref().and_then(|id| projet.niveau(id)).or(projet.niveaux.first()).map_or("Niveau 0", |n| n.nom.as_str());
                        egui::ComboBox::from_id_salt("niveau_dalle").width(130.0).selected_text(nom).show_ui(ui, |ui| {
                            for n in &projet.niveaux {
                                ui.selectable_value(&mut self.niveau_dalle, Some(n.id.clone()), &n.nom);
                            }
                        });
                        ui.end_row();
                        if surface == Outil::Toiture {
                            ui.label("Pente");
                            let mut pente = !self.toiture_plate;
                            ui.checkbox(&mut pente, "Toutes les arêtes la définissent")
                                .on_hover_text("Arêtes tracées en pente (croupes). Décocher pour une toiture plate ; chaque arête se règle ensuite dans les propriétés de la toiture.");
                            self.toiture_plate = !pente;
                            ui.end_row();
                        }
                    });
                }
                None => {
                    ui.label(RichText::new(format!("Créez ou ouvrez un projet natif pour dessiner : {}.", surface.nom().to_lowercase())).weak());
                }
            }
        }
        egui::Grid::new(("proprietes_surface", surface))
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                let epaisseur_type: Option<f64> = self.surface_identique.as_ref().filter(|(o,_)| *o == surface).map(|(_, t)| t.epaisseur()).or_else(|| types_surface(surface)
                    .get(self.type_surface.get(&surface).copied().unwrap_or(0))
                    .map(|(_, cs)| cs.iter().map(|c| c.epaisseur).sum()));
                for (indice, champ) in champs(surface).into_iter().enumerate() {
                    if surface == Outil::Toiture && nom_champ(&champ) == "Pente" && self.toiture_plate {
                        continue;
                    }
                    if let Champ::Nombre(nom, defaut, suffixe) = champ {
                        ui.label(nom);
                        // épaisseur : celle du type choisi
                        if let (Some(e), "Épaisseur") = (epaisseur_type, nom) {
                            ui.label(crate::config::options::unite().afficher(e)).on_hover_text("Épaisseur du type (somme des couches)");
                            ui.end_row();
                            continue;
                        }
                        let valeur = self
                            .valeurs
                            .entry((surface, indice))
                            .or_insert(Valeur::Nombre(defaut));
                        if let Valeur::Nombre(valeur) = valeur {
                            ui.add_sized(
                                [90.0, 22.0],
                                if suffixe == " mm" { egui::DragValue::new(valeur).speed(10.0).dimensions() }
                                else { egui::DragValue::new(valeur).speed(0.1).max_decimals(2).suffix(suffixe) },
                            );
                        }
                        ui.end_row();
                    }
                }
            });
        true
    }

    pub(super) fn options_outil(
        &mut self,
        ui: &mut egui::Ui,
        outil: Outil,
        projet: Option<&crate::modele::Projet>,
    ) {
        let fenetre = matches!(self.valeurs.get(&(Outil::Ouverture, 0)), Some(Valeur::Texte(t)) if t == "Fenêtre" || t == "Libre");
        let champs: Vec<_> = champs(outil)
            .into_iter()
            .enumerate()
            .filter(|(k, _)| outil != Outil::Ouverture || (*k != 0 && (*k != 3 || fenetre)))
            // mur : décalage bas dans Propriétés ; hauteur seulement si le haut est libre
            // et l'épaisseur seulement pour un mur générique (sans type)
            .filter(|(k, _)| outil != Outil::Mur || (*k != 3 && (*k != 1 || self.niveau_haut_mur.is_none()) && (*k != 2 || self.type_mur.is_none())))
            .collect();
        let mut changement_ouverture = false;
        for ligne in champs.chunks(2) {
            ui.horizontal(|ui| {
                for (k, champ) in ligne {
                    ui.push_id(("option_ruban", outil, *k), |ui| {
                        ui.horizontal(|ui| match champ {
                            Champ::Choix(nom, choix) => {
                                ui.label(*nom);
                                let v = self
                                    .valeurs
                                    .entry((outil, *k))
                                    .or_insert_with(|| Valeur::Texte(choix[0].into()));
                                if let Valeur::Texte(v) = v {
                                    egui::ComboBox::from_id_salt("choix")
                                        .width(110.0)
                                        .selected_text(&*v)
                                        .show_ui(ui, |ui| {
                                            for choix in *choix {
                                                if ui.selectable_value(v, (*choix).into(), *choix).changed() && outil == Outil::Ouverture && *k == 0 {
                                                    changement_ouverture = true;
                                                }
                                            }
                                        });
                                }
                            }
                            Champ::Nombre(nom, defaut, suffixe) => {
                                ui.label(*nom);
                                let v = self
                                    .valeurs
                                    .entry((outil, *k))
                                    .or_insert(Valeur::Nombre(*defaut));
                                if let Valeur::Nombre(v) = v {
                                    let pas = if *suffixe == " mm" { 10.0 } else { 0.1 };
                                    let champ = egui::DragValue::new(v).speed(pas);
                                    ui.add(if *suffixe == " mm" && !nom.contains("papier") { champ.dimensions() } else { champ.max_decimals(2).suffix(*suffixe) });
                                }
                            }
                            Champ::Saisie(nom, defaut) => {
                                ui.label(*nom);
                                let v = self
                                    .valeurs
                                    .entry((outil, *k))
                                    .or_insert_with(|| Valeur::Texte((*defaut).into()));
                                if let Valeur::Texte(v) = v {
                                    ui.add(egui::TextEdit::singleline(v).desired_width(100.0));
                                }
                            }
                        });
                    });
                }
            });
        }
        if changement_ouverture { self.ouverture_type_identique = None; }
        if outil == Outil::Mur {
            self.options_mur(ui, projet);
        }
    }

    pub(super) fn options_mur(&mut self, ui: &mut egui::Ui, projet: Option<&crate::modele::Projet>) {
        if !self.mur_actif() {
            return;
        }
        let Some(projet) = projet else {
            ui.label(RichText::new("Créez ou ouvrez un projet natif pour dessiner un mur.").weak());
            return;
        };
        if self
            .niveau_mur
            .as_ref()
            .is_some_and(|id| projet.niveau(id).is_none())
        {
            self.niveau_mur = None;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Niveau de base");
            let defaut = projet.niveaux.first();
            let nom = self
                .niveau_mur
                .as_ref()
                .and_then(|id| projet.niveau(id))
                .or(defaut)
                .map_or("Niveau 0", |n| n.nom.as_str());
            egui::ComboBox::from_id_salt("niveau_mur")
                .selected_text(nom)
                .show_ui(ui, |ui| {
                    if projet.niveaux.is_empty() {
                        ui.selectable_value(&mut self.niveau_mur, None, "Niveau 0");
                    }
                    for n in &projet.niveaux {
                        ui.selectable_value(&mut self.niveau_mur, Some(n.id.clone()), &n.nom);
                    }
                });
        });
    }
}
