//! Propriétés, types et croquis des murs.
use super::*;

impl RubanEdition {
    /// Palette de création du mur : croquis des deux limites (haut, bas) avec leur
    /// réglage sur la même ligne, puis les propriétés du type en grille.
    pub fn proprietes_mur(
        &mut self,
        ui: &mut egui::Ui,
        projet: Option<&crate::modele::Projet>,
    ) -> bool {
        if !self.mur_actif() {
            return false;
        }
        let Some(projet) = projet else {
            ui.label("Créez ou ouvrez un projet natif pour dessiner un mur.");
            return true;
        };
        // Un changement de document ne doit jamais réutiliser une référence étrangère.
        for niveau in [&mut self.niveau_mur, &mut self.niveau_haut_mur] {
            if niveau
                .as_ref()
                .is_some_and(|id| projet.niveau(id).is_none())
            {
                *niveau = None;
            }
        }
        if self.type_mur.as_ref().is_some_and(|id| projet.type_mur(id).is_none()) {
            self.type_mur = None;
        }
        egui::ScrollArea::vertical()
            .id_salt("creation_mur")
            .show(ui, |ui| {
                self.selecteur_type_mur(ui, projet, true);
                ui.add_space(2.0);
                egui::Frame::new()
                    .fill(ui.visuals().faint_bg_color)
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        self.croquis_mur_creation(ui, projet);
                    });
                if let Err(e) = self.parametres_mur().bornes(projet) {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                ui.add_space(4.0);
                egui::Grid::new("grille_creation_mur")
                    .num_columns(2)
                    .striped(true)
                    .spacing([10.0, 4.0])
                    .min_col_width(80.0)
                    .show(ui, |ui| {
                        ui.label("Épaisseur");
                        match self.type_mur.as_ref().and_then(|id| projet.type_mur(id)) {
                            Some(t) => {
                                ui.label(crate::config::options::unite().afficher(t.epaisseur()))
                                    .on_hover_text("Épaisseur du type (somme des couches)");
                            }
                            None => self.nombre_mur(ui, 2, 300.0),
                        }
                        ui.end_row();
                        ui.label("Implantation");
                        let v = self
                            .valeurs
                            .entry((Outil::Mur, 0))
                            .or_insert_with(|| Valeur::Texte("Face extérieure".into()));
                        if let Valeur::Texte(v) = v {
                            egui::ComboBox::from_id_salt("implantation_proprietes_mur")
                                .width(130.0)
                                .selected_text(v.as_str())
                                .show_ui(ui, |ui| {
                                    for nom in ["Face extérieure", "Axe", "Face intérieure", "Axe du porteur"] {
                                        ui.selectable_value(v, nom.into(), nom);
                                    }
                                });
                        }
                        ui.end_row();
                    });
            });
        true
    }

    /// Sélecteur de type en tête de la palette, comme dans Revit : générique (épaisseur
    /// saisie) ou un type du projet ; composition en bande sous la liste ; Modifier /
    /// Nouveau ouvrent l'éditeur de type.
    pub fn selecteur_type_mur(&mut self, ui: &mut egui::Ui, projet: &crate::modele::Projet, generique: bool) {
        use crate::modele::CategorieMur;
        let actuel = self.type_mur.as_ref().and_then(|id| projet.type_mur(id));
        let categorie_actuelle = actuel.map_or(CategorieMur::Mur, |t| t.categorie_mur());
        let nom = actuel.map_or("Générique (épaisseur libre)", |t| t.nom.as_str());
        let onglet = self.categorie_mur.unwrap_or(categorie_actuelle);
        // largeur du panneau mesurée hors de la ligne, qui grandirait avec la liste
        let largeur = ui.available_width();
        // types de la catégorie : (type du projet, type du gabarit à ajouter, nom, couches)
        type Ligne = (Option<String>, Option<crate::config::gabarit::TypeGabarit>, String, Vec<crate::modele::Couche>);
        let mut lignes: Vec<Ligne> = Vec::new();
        if generique && onglet == CategorieMur::Mur {
            lignes.push((None, None, "Générique (épaisseur libre)".into(), Vec::new()));
        }
        let mut types: Vec<_> = projet.types_murs.iter().filter(|t| t.categorie_mur() == onglet).collect();
        types.sort_by(|a, b| a.nom.cmp(&b.nom));
        lignes.extend(types.into_iter().map(|t| (Some(t.id.clone()), None, t.nom.clone(), t.couches.clone())));
        // types du gabarit absents du projet : copiés dans le projet au choix
        let gabarit = crate::config::gabarit::actif();
        lignes.extend(gabarit.types_murs.iter().filter(|t| t.categorie_mur() == onglet && !projet.types_murs.iter().any(|p| p.nom == t.nom)).map(|t| (None, Some(t.clone()), t.nom.clone(), t.couches.clone())));
        let rangs = CategorieMur::TOUTES.iter().map(|c| projet.types_murs.iter().filter(|t| t.categorie_mur() == *c).count() + gabarit.types_murs.iter().filter(|t| t.categorie_mur() == *c).count() + 1).max().unwrap_or(1);
        let mut choix_onglet = None;
        let mut choisi: Option<Ligne> = None;
        let mut editer: Option<String> = None;
        let mut dupliquer: Option<(String, Vec<crate::modele::Couche>)> = None;
        let mut nouveau = false;
        let unite = crate::config::options::unite();
        let epaisseur = |c: &[crate::modele::Couche]| c.iter().map(|c| c.epaisseur).sum::<f64>();
        let police = egui::FontId::proportional(12.0);
        let largeur_texte = |ui: &egui::Ui, t: &str, f: &egui::FontId| ui.fonts_mut(|x| x.layout_no_wrap(t.to_string(), f.clone(), Color32::WHITE).size().x);
        // Sélecteur fermé : catégorie, nom et épaisseur en haut ; dessous, sur toute la
        // largeur, le plan des couches (de l'extérieur vers l'intérieur).
        let couches_actuelles = actuel.map(|t| t.couches.clone()).unwrap_or_default();
        let hauteur = if couches_actuelles.is_empty() { 30.0 } else { 56.0 };
        let (r, bouton) = ui.allocate_exact_size(egui::vec2(largeur, hauteur), Sense::click());
        let visuel = ui.style().interact(&bouton).clone();
        ui.painter().rect(r, 3.0, visuel.bg_fill, visuel.bg_stroke, egui::StrokeKind::Inside);
        let ligne = egui::Rect::from_min_size(r.min, egui::vec2(r.width(), 28.0));
        let fleche = egui::Rect::from_center_size(egui::pos2(r.right() - 12.0, ligne.center().y), egui::vec2(8.0, 8.0));
        triangle_bas(ui.painter(), fleche.center(), visuel.fg_stroke.color);
        let droite_texte = if couches_actuelles.is_empty() { fleche.left() - 6.0 } else {
            let e = unite.afficher(epaisseur(&couches_actuelles));
            let x = fleche.left() - 6.0;
            ui.painter().text(egui::pos2(x, ligne.center().y), egui::Align2::RIGHT_CENTER, &e, egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
            x - largeur_texte(ui, &e, &egui::FontId::proportional(11.0)) - 8.0
        };
        let titre = format!("{} · {nom}", categorie_actuelle.nom());
        let zone_titre = egui::Rect::from_min_max(egui::pos2(r.left() + 10.0, ligne.top()), egui::pos2(droite_texte, ligne.bottom()));
        ui.painter().with_clip_rect(zone_titre).text(egui::pos2(zone_titre.left(), ligne.center().y), egui::Align2::LEFT_CENTER, &titre, egui::FontId::proportional(13.0), visuel.text_color());
        let mut bulle = format!("{titre}\nType des prochains murs. Un type à plusieurs couches crée un mur par couche, regroupés en assemblage.");
        if !couches_actuelles.is_empty() {
            let plan = egui::Rect::from_min_max(egui::pos2(r.left() + 8.0, ligne.bottom()), egui::pos2(r.right() - 8.0, r.bottom() - 7.0));
            if let Some(c) = peindre_bande_couches(ui.painter(), plan, &projet.materiaux, &couches_actuelles, bouton.hover_pos()) {
                bulle = texte_couche(c);
            }
        }
        let bouton = bouton.on_hover_text(bulle);
        // Liste : assez large pour le dessin en plan, le nom le plus long et l'épaisseur.
        let plus_long = lignes.iter().map(|l| largeur_texte(ui, &l.2, &police)).fold(0.0f32, f32::max);
        let largeur_liste = (8.0 + 40.0 + 10.0 + plus_long + 16.0 + 64.0 + 50.0 + 8.0).max(largeur).max(CategorieMur::TOUTES.len() as f32 * 98.0 + 12.0);
        egui::Popup::from_toggle_button_response(&bouton)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .width(largeur_liste)
            .show(|ui| {
                ui.set_min_width(largeur_liste);
                // hauteur fixe (catégorie la plus fournie) : la liste ne saute pas d'un onglet à l'autre
                ui.set_min_height(116.0 + rangs as f32 * 27.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for c in CategorieMur::TOUTES {
                        let (r, reponse) = ui.allocate_exact_size(egui::vec2(96.0, 50.0), Sense::click());
                        let actif = onglet == c;
                        if actif || reponse.hovered() {
                            ui.painter().rect_filled(r, 4.0, if actif { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                        }
                        icone_categorie_mur(ui, egui::Rect::from_center_size(r.center_top() + egui::vec2(0.0, 17.0), egui::vec2(26.0, 26.0)), c);
                        ui.painter().text(r.center_bottom() - egui::vec2(0.0, 9.0), egui::Align2::CENTER_CENTER, c.nom(), egui::FontId::proportional(12.0), ui.visuals().text_color());
                        if reponse.clicked() && !actif {
                            choix_onglet = Some(c);
                            ui.ctx().request_repaint();
                        }
                    }
                });
                ui.label(RichText::new(onglet.aide()).small().weak());
                ui.separator();
                for ligne in &lignes {
                    let courant = match (&ligne.0, &self.type_mur) {
                        (None, None) => ligne.1.is_none(),
                        (Some(a), Some(b)) => a == b,
                        _ => false,
                    };
                    let (r, reponse) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::click());
                    let survol = ui.rect_contains_pointer(r);
                    if courant || survol {
                        ui.painter().rect_filled(r, 3.0, if courant { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                    }
                    if !ligne.3.is_empty() {
                        peindre_bande_couches(ui.painter(), egui::Rect::from_min_size(r.min + egui::vec2(8.0, 6.0), egui::vec2(40.0, 14.0)), &projet.materiaux, &ligne.3, None);
                    }
                    ui.painter().text(r.min + egui::vec2(58.0, 13.0), egui::Align2::LEFT_CENTER, &ligne.2, police.clone(), ui.visuals().text_color());
                    if ligne.1.is_some() {
                        ui.painter().text(r.min + egui::vec2(66.0 + largeur_texte(ui, &ligne.2, &police), 13.0), egui::Align2::LEFT_CENTER, "gabarit", egui::FontId::proportional(10.0), ui.visuals().weak_text_color());
                    }
                    // au survol : crayon (modifier) et duplication (nouveau type d'après celui-ci)
                    let mut icone_cliquee = false;
                    if survol && !ligne.3.is_empty() {
                        let mut x = r.right() - 8.0;
                        for (n, (forme, bulle)) in [("dupliquer", "Dupliquer : nouveau type d'après celui-ci"), ("crayon", "Modifier le type")].into_iter().enumerate() {
                            if forme == "crayon" && ligne.0.is_none() {
                                continue;
                            }
                            let zone = egui::Rect::from_min_max(egui::pos2(x - 22.0, r.top() + 2.0), egui::pos2(x, r.bottom() - 2.0));
                            x -= 24.0;
                            let rep = ui.interact(zone, ui.scope_id().with(("icone_type", &ligne.2, n)), Sense::click()).on_hover_text(bulle);
                            if rep.hovered() {
                                ui.painter().rect_filled(zone, 3.0, ui.visuals().widgets.active.bg_fill);
                            }
                            let c = if rep.hovered() { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                            icone_action_type(ui.painter(), zone.shrink(4.0), forme == "crayon", c);
                            if rep.clicked() {
                                icone_cliquee = true;
                                if forme == "crayon" {
                                    editer = ligne.0.clone();
                                } else {
                                    dupliquer = Some((ligne.2.clone(), ligne.3.clone()));
                                }
                                ui.close();
                            }
                        }
                    } else if !ligne.3.is_empty() {
                        ui.painter().text(r.right_center() - egui::vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, unite.afficher(epaisseur(&ligne.3)), egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
                    }
                    if reponse.clicked() && !icone_cliquee {
                        choisi = Some(ligne.clone());
                        ui.close();
                    }
                }
                // en fin de liste : création d'un nouveau type
                ui.separator();
                let (r, reponse) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::click());
                if reponse.hovered() {
                    ui.painter().rect_filled(r, 3.0, ui.visuals().widgets.hovered.bg_fill);
                }
                ui.painter().text(r.min + egui::vec2(28.0, 13.0), egui::Align2::CENTER_CENTER, "+", egui::FontId::proportional(16.0), ui.visuals().strong_text_color());
                ui.painter().text(r.min + egui::vec2(58.0, 13.0), egui::Align2::LEFT_CENTER, "Nouveau type de mur…", police.clone(), ui.visuals().text_color());
                if reponse.clicked() {
                    nouveau = true;
                    ui.close();
                }
            });
        if let Some(id) = editer {
            self.ouvrir_editeur_type(projet, Some(id));
        }
        if let Some((nom, couches)) = dupliquer {
            self.dupliquer_type(projet, &nom, couches);
        }
        if nouveau {
            self.ouvrir_editeur_type(projet, None);
        }
        if let Some(c) = choix_onglet {
            self.categorie_mur = Some(c);
        }
        if let Some((id, gabarit, _, _)) = choisi {
            self.categorie_mur = None;
            match gabarit {
                Some(t) => self.type_mur_gabarit = Some(t),
                None => self.type_mur = id,
            }
        }
    }

    /// Type de mur du gabarit choisi dans le sélecteur, à ajouter au projet.
    pub fn prendre_type_gabarit(&mut self) -> Option<crate::config::gabarit::TypeGabarit> {
        self.type_mur_gabarit.take()
    }

    pub fn editeur_type_ouvert(&self) -> bool {
        self.editeur_type.is_some()
    }

    /// Dupliquer un type : l'éditeur s'ouvre sur ses couches, enregistrées sous un
    /// nouveau nom (le type d'origine ne change pas).
    pub(super) fn dupliquer_type(&mut self, projet: &crate::modele::Projet, nom: &str, couches: Vec<crate::modele::Couche>) {
        let categorie = projet.types_murs.iter().find(|t| t.nom == nom).map(|t| t.categorie_mur());
        self.ouvrir_editeur_type(projet, None);
        if let Some(ed) = self.editeur_type.as_mut() {
            let mut k = 1;
            let mut copie = format!("{nom} (copie)");
            while projet.types_murs.iter().any(|t| t.nom == copie) {
                k += 1;
                copie = format!("{nom} (copie {k})");
            }
            ed.nom = copie;
            for c in &couches {
                if let Some(m) = &c.materiau
                    && !ed.materiaux.iter().any(|x| &x.nom == m)
                {
                    ed.materiaux.push(crate::modele::murs::assemblage::materiau_par_defaut(m));
                }
            }
            ed.couches = couches;
            if let Some(c) = categorie {
                ed.categorie = c;
            }
        }
    }

    pub(super) fn ouvrir_editeur_type(&mut self, projet: &crate::modele::Projet, id: Option<String>) {
        use crate::modele::{Couche, FonctionCouche};
        let existant = id.as_ref().and_then(|id| projet.type_mur(id));
        let (nom, couches) = match existant {
            Some(t) => (t.nom.clone(), t.couches.clone()),
            None => {
                // générique en cours : sa seule couche ; sinon un mur isolé type
                let mut n = projet.types_murs.len() + 1;
                while projet.types_murs.iter().any(|t| t.nom == format!("Mur composé {n}")) {
                    n += 1;
                }
                let c = |m: &str, e: f64, f| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f, plume: None, retour: false };
                (format!("Mur composé {n}"), vec![c("Enduit", 15.0, FonctionCouche::Finition), c("Béton", 200.0, FonctionCouche::Structure), c("Laine minérale", 120.0, FonctionCouche::Isolation), c("Plâtre", 13.0, FonctionCouche::Finition)])
            }
        };
        let mut materiaux = projet.materiaux.clone();
        for c in &couches {
            if let Some(m) = &c.materiau
                && !materiaux.iter().any(|x| &x.nom == m)
            {
                materiaux.push(crate::modele::murs::assemblage::materiau_par_defaut(m));
            }
        }
        self.editeur_type = Some(EditeurType { id: existant.map(|t| t.id.clone()), nom, categorie: existant.map_or(crate::modele::CategorieMur::Compose, |t| t.categorie_mur()), couches, materiaux, erreur: None, glisse: None });
    }

    /// Fenêtre d'édition d'un type de mur composé : une ligne par couche, de
    /// l'extérieur (en haut) vers l'intérieur. Retourne vrai si le projet a changé.
    pub fn editeur_type_mur(&mut self, ctx: &egui::Context, projet: &mut crate::modele::Projet) -> bool {
        use crate::modele::{Couche, FonctionCouche};
        let Some(ed) = &mut self.editeur_type else { return false };
        let (mut ouvert, mut enregistrer, mut annuler) = (true, false, false);
        egui::Window::new(if ed.id.is_some() { "Modifier le type de mur" } else { "Nouveau type de mur" })
            .id(egui::Id::unique("editeur_type_mur"))
            .open(&mut ouvert)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Nom");
                    ui.add(egui::TextEdit::singleline(&mut ed.nom).desired_width(300.0));
                    egui::ComboBox::from_id_salt("categorie_type_mur").selected_text(ed.categorie.nom()).show_ui(ui, |ui| {
                        for c in crate::modele::CategorieMur::TOUTES {
                            ui.selectable_value(&mut ed.categorie, c, c.nom());
                        }
                    }).response.on_hover_text("Catégorie du type : mur, mur composé ou cloison");
                });
                ui.add_space(4.0);
                bande_couches_brouillon(ui, &ed.materiaux, &ed.couches, 18.0);
                ui.label(RichText::new("Extérieur").small().weak());
                let mut action: Option<(usize, i32)> = None; // (couche, 0 supprimer)
                // glisser-déposer : lignes (hauteur) et relâchement de la poignée
                let mut lignes: Vec<egui::Rangef> = Vec::new();
                let mut lacher = false;
                egui::Grid::new("couches_type_mur").num_columns(9).spacing([6.0, 4.0]).striped(true).show(ui, |ui| {
                    for t in ["", "Fonction", "Matériau", "", "Motif", "Épaisseur", "Plume", "Retour", ""] {
                        ui.label(RichText::new(t).small().weak());
                    }
                    ui.end_row();
                    let n = ed.couches.len();
                    for (k, c) in ed.couches.iter_mut().enumerate() {
                        ui.push_id(k, |ui| {
                            // poignée : glisser la ligne vers une autre position
                            let (r, poignee) = ui.allocate_exact_size(egui::vec2(18.0, 20.0), Sense::drag());
                            let v = ui.visuals();
                            let encre = if poignee.hovered() || ed.glisse == Some(k) { v.strong_text_color() } else { v.weak_text_color() };
                            for dy in [-4.0, 0.0, 4.0] {
                                ui.painter().line_segment([r.center() + egui::vec2(-5.0, dy), r.center() + egui::vec2(5.0, dy)], egui::Stroke::new(1.3, encre));
                            }
                            if poignee.drag_started() {
                                ed.glisse = Some(k);
                            }
                            if poignee.drag_stopped() {
                                lacher = true;
                            }
                            lignes.push(r.y_range());
                            poignee.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text("Glisser pour déplacer la couche");
                        });
                        egui::ComboBox::from_id_salt(("fonction", k)).width(84.0).selected_text(c.fonction.libelle()).show_ui(ui, |ui| {
                            for f in FonctionCouche::TOUTES {
                                ui.selectable_value(&mut c.fonction, f, f.libelle());
                            }
                        });
                        // matériau : nom libre, ou choisi dans ceux du projet
                        let mut nom = c.materiau.clone().unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.add(egui::TextEdit::singleline(&mut nom).desired_width(130.0).hint_text("Matériau"));
                            let liste = petit_bouton(ui, !ed.materiaux.is_empty(), Glyphe::Liste).on_hover_text("Matériaux du projet");
                            egui::Popup::menu(&liste).show(|ui| {
                                for m in &ed.materiaux {
                                    if ui.button(&m.nom).clicked() {
                                        nom = m.nom.clone();
                                        ui.close();
                                    }
                                }
                            });
                        });
                        let nom = nom.trim().to_string();
                        c.materiau = (!nom.is_empty()).then_some(nom.clone());
                        if !nom.is_empty() && !ed.materiaux.iter().any(|m| m.nom == nom) {
                            ed.materiaux.push(crate::modele::murs::assemblage::materiau_par_defaut(&nom));
                        }
                        // couleur et motif : hérités du matériau, affichés en lecture seule
                        match ed.materiaux.iter().find(|m| m.nom == nom) {
                            Some(m) => {
                                let [r, g, b] = m.couleur;
                                let (rect, rep) = ui.allocate_exact_size(egui::vec2(20.0, 16.0), Sense::hover());
                                ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(r, g, b));
                                ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, ui.visuals().weak_text_color()), egui::StrokeKind::Inside);
                                rep.on_hover_text("Couleur héritée du matériau");
                                ui.label(m.motif.libelle()).on_hover_text("Motif de coupe hérité du matériau");
                            }
                            None => {
                                ui.label("");
                                ui.label("");
                            }
                        }
                        ui.add(egui::DragValue::new(&mut c.epaisseur).speed(1.0).range(1.0..=2000.0).max_decimals(1).dimensions());
                        // plume du contour de coupe : numéro et épaisseur de la table
                        let plumes = projet.epaisseurs_plumes();
                        let mut n_plume = c.plume();
                        egui::ComboBox::from_id_salt(("plume", k)).width(70.0).selected_text(format!("{n_plume} · {}", crate::formats::ifc::step::format_real(plumes[(n_plume - 1) as usize]))).show_ui(ui, |ui| {
                            for (i, e) in plumes.iter().enumerate() {
                                ui.selectable_value(&mut n_plume, i as u8 + 1, format!("{} · {} mm", i + 1, crate::formats::ifc::step::format_real(*e)));
                            }
                        }).response.on_hover_text("Plume du contour de coupe de la couche");
                        if n_plume != c.plume() {
                            c.plume = Some(n_plume);
                        }
                        ui.add_enabled(c.fonction != FonctionCouche::Structure, egui::Checkbox::without_text(&mut c.retour))
                            .on_hover_text("Retour aux insertions : la couche tourne dans les tableaux et le linteau des ouvertures, jusqu'au plan « Retour extérieur » ou « Retour intérieur » de leur famille")
                            .on_disabled_hover_text("Une couche de structure ne se retourne pas");
                        if petit_bouton(ui, n > 1, Glyphe::Croix).on_hover_text("Supprimer la couche").clicked() { action = Some((k, 0)); }
                        ui.end_row();
                    }
                });
                ui.label(RichText::new("Intérieur").small().weak());
                // position d'insertion sous le pointeur : nombre de lignes dont le milieu est au-dessus
                if let (Some(k), Some(y)) = (ed.glisse, ui.input(|i| i.pointer.interact_pos().map(|p| p.y))) {
                    let cible = lignes.iter().filter(|l| l.center() < y).count();
                    if let (Some(premiere), Some(derniere)) = (lignes.first(), lignes.last()) {
                        let yl = if cible == 0 { premiere.min - 2.0 } else { lignes[cible - 1].max + 2.0 }.clamp(premiere.min - 2.0, derniere.max + 2.0);
                        let x = ui.min_rect().x_range();
                        ui.painter().line_segment([egui::pos2(x.min, yl), egui::pos2(x.max, yl)], egui::Stroke::new(2.0, ui.visuals().selection.stroke.color));
                    }
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    if lacher {
                        let couche = ed.couches.remove(k);
                        let cible = if cible > k { cible - 1 } else { cible };
                        ed.couches.insert(cible.min(ed.couches.len()), couche);
                        ed.glisse = None;
                    }
                }
                if lacher {
                    ed.glisse = None;
                }
                if let Some((k, 0)) = action {
                    ed.couches.remove(k);
                }
                ui.horizontal(|ui| {
                    if ui.button("+ Couche").clicked() {
                        ed.couches.push(Couche { materiau: None, epaisseur: 50.0, fonction: FonctionCouche::Autre, plume: None, retour: false });
                    }
                    let total: f64 = ed.couches.iter().map(|c| c.epaisseur).sum();
                    ui.label(format!("Épaisseur totale : {}", crate::config::options::unite().afficher(total)));
                });
                if let Some(e) = &ed.erreur {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                ui.label(RichText::new("Chaque couche sera tracée comme un mur indépendant, regroupé avec les autres en assemblage. Les murs déjà tracés gardent leurs couches.").small().weak());
                ui.separator();
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), egui::Layout::right_to_left(Align::Center), |ui| {
                    annuler = ui.button("Annuler").clicked();
                    enregistrer = ui.button(RichText::new("Enregistrer").strong()).clicked();
                });
            });
        if !ouvert || annuler {
            self.editeur_type = None;
            return false;
        }
        if !enregistrer {
            return false;
        }
        let ed = self.editeur_type.as_mut().unwrap();
        let nom = ed.nom.trim().to_string();
        if nom.is_empty() {
            ed.erreur = Some("Donnez un nom au type.".into());
            return false;
        }
        if ed.couches.is_empty() || ed.couches.iter().any(|c| !(c.epaisseur > 0.0 && c.epaisseur.is_finite())) {
            ed.erreur = Some("Chaque couche doit avoir une épaisseur positive.".into());
            return false;
        }
        if projet.types_murs.iter().any(|t| t.nom == nom && Some(&t.id) != ed.id.as_ref()) {
            ed.erreur = Some("Un autre type porte déjà ce nom.".into());
            return false;
        }
        let id = match &ed.id {
            Some(id) => {
                let t = projet.types_murs.iter_mut().find(|t| &t.id == id).unwrap();
                t.nom = nom;
                t.couches = ed.couches.clone();
                t.couches_detaillees = true;
                t.categorie = Some(ed.categorie);
                id.clone()
            }
            None => {
                let id = crate::modele::collaboration::uuid();
                projet.types_murs.push(crate::modele::TypeCompose { id: id.clone(), nom, couches: ed.couches.clone(), categorie: Some(ed.categorie), couches_detaillees: true, origine: None });
                id
            }
        };
        // matériaux nouveaux utilisés : ajoutés au projet (les existants ne sont pas modifiés ici)
        for m in &ed.materiaux {
            if ed.couches.iter().any(|c| c.materiau.as_ref() == Some(&m.nom)) && projet.materiau(&m.nom).is_none() {
                projet.materiaux.push(m.clone());
            }
        }
        self.type_mur = Some(id);
        self.editeur_type = None;
        true
    }

    /// Crée (ou remplace) un type composé et le choisit pour les prochains murs (pilote).
    pub fn definir_type_mur(&mut self, projet: &mut crate::modele::Projet, nom: &str, couches: Vec<crate::modele::Couche>, categorie: Option<crate::modele::CategorieMur>) -> Result<String, String> {
        if couches.is_empty() || couches.iter().any(|c| !(c.epaisseur > 0.0 && c.epaisseur.is_finite())) {
            return Err("Chaque couche doit avoir une épaisseur positive.".into());
        }
        let id = match projet.types_murs.iter_mut().find(|t| t.nom == nom) {
            Some(t) => {
                t.couches = couches;
                t.categorie = categorie;
                t.id.clone()
            }
            None => {
                let id = crate::modele::collaboration::uuid();
                projet.types_murs.push(crate::modele::TypeCompose { id: id.clone(), nom: nom.into(), couches, categorie, couches_detaillees: true, origine: None });
                id
            }
        };
        projet.completer_materiaux();
        self.type_mur = Some(id.clone());
        Ok(id)
    }

    /// Croquis de saisie : mur en perspective à gauche, ses deux limites à droite, sur
    /// une ligne chacune (niveau, puis décalage, ou hauteur si le haut est libre),
    /// rappelées par un pointillé depuis l'arête correspondante du mur.
    pub(super) fn croquis_mur_creation(&mut self, ui: &mut egui::Ui, projet: &crate::modele::Projet) {
        use egui::{Align2, FontId, Stroke, pos2, vec2};
        let bleu = Color32::from_rgb(18, 158, 205);
        let largeur_croquis = 64.0;
        let origine = ui.cursor().min;
        let mut centres = [0.0f32; 2];
        let mut gauche_champs = 0.0f32;
        // Réserver de la place autour du croquis pour les décalages des limites.
        ui.add_space(14.0);
        ui.horizontal_top(|ui| {
            ui.add_space(largeur_croquis);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 16.0;
                for haut in [true, false] {
                    let ligne = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add_sized(
                            [30.0, 20.0],
                            egui::Label::new(RichText::new(if haut { "Haut" } else { "Bas" }).color(bleu)),
                        );
                        let niveau = if haut {
                            &mut self.niveau_haut_mur
                        } else {
                            &mut self.niveau_mur
                        };
                        let nom = niveau
                            .as_ref()
                            .and_then(|id| projet.niveau(id))
                            .or_else(|| if haut { None } else { projet.niveaux.first() })
                            .map_or(if haut { "Libre" } else { "Niveau 0" }, |n| n.nom.as_str());
                        let champ = 72.0;
                        egui::ComboBox::from_id_salt(("limite_creation_mur", haut))
                            .width((ui.available_width() - champ - 4.0).max(60.0))
                            .truncate()
                            .selected_text(nom)
                            .show_ui(ui, |ui| {
                                if haut || projet.niveaux.is_empty() {
                                    ui.selectable_value(
                                        niveau,
                                        None,
                                        if haut { "Libre (hauteur)" } else { "Niveau 0" },
                                    );
                                }
                                for n in &projet.niveaux {
                                    ui.selectable_value(niveau, Some(n.id.clone()), &n.nom);
                                }
                            })
                            .response
                            .on_hover_text(if haut {
                                "Limite haute : un niveau, ou libre (hauteur saisie)"
                            } else {
                                "Limite basse : niveau de base du mur"
                            });
                        if haut && self.niveau_haut_mur.is_none() {
                            // haut libre : la valeur est la hauteur, sans décalage
                            self.decalage_haut_mur = 0.0;
                            let v = self
                                .valeurs
                                .entry((Outil::Mur, 1))
                                .or_insert(Valeur::Nombre(2800.0));
                            if let Valeur::Nombre(v) = v {
                                ui.add_sized(
                                    [champ, 20.0],
                                    egui::DragValue::new(v).speed(10.0).range(1.0..=f64::MAX).dimensions(),
                                )
                                .on_hover_text("Hauteur du mur");
                            }
                        } else {
                            let v = if haut {
                                &mut self.decalage_haut_mur
                            } else {
                                match self.valeurs.entry((Outil::Mur, 3)).or_insert(Valeur::Nombre(0.0)) {
                                    Valeur::Nombre(v) => v,
                                    Valeur::Texte(_) => unreachable!(),
                                }
                            };
                            ui.add_sized(
                                [champ, 20.0],
                                egui::DragValue::new(v)
                                    .speed(10.0)
                                    .dimensions(),
                            )
                            .on_hover_text("Décalage par rapport au niveau");
                        }
                    });
                    gauche_champs = ligne.response.rect.left();
                    centres[if haut { 0 } else { 1 }] = ligne.response.rect.center().y;
                }
            });
        });
        ui.add_space(14.0);
        let p = ui.painter();
        let bord = Stroke::new(1.0, bleu);
        let profondeur = vec2(4.5, -3.0);
        // mur vu de face (largeur), son chant à droite et son dessus
        let x1 = origine.x + largeur_croquis - 20.0;
        let x0 = origine.x + 16.0;
        let (yh, yb) = limites_croquis_mur(&self.parametres_mur(), projet, centres[0] + 3.0, centres[1] + 3.0);
        let (a, h, b, bb) = (pos2(x0, yh), pos2(x1, yh), pos2(x1, yb), pos2(x0, yb));
        p.add(egui::Shape::convex_polygon(vec![a, h, b, bb], Color32::from_rgb(246, 251, 253), bord));
        p.add(egui::Shape::convex_polygon(
            vec![h, h + profondeur, b + profondeur, b],
            Color32::from_rgb(222, 242, 249),
            bord,
        ));
        p.add(egui::Shape::convex_polygon(
            vec![a, a + profondeur, h + profondeur, h],
            Color32::from_rgb(236, 248, 252),
            bord,
        ));
        // rappels : de l'arête du mur à la ligne de sa limite
        let rappel = Stroke::new(0.8, bleu.gamma_multiply(0.8));
        for (depuis, y) in [(h + profondeur, centres[0]), (b + profondeur, centres[1])] {
            let fin = gauche_champs - 3.0;
            let mut x = depuis.x + 3.0;
            while x < fin {
                p.line_segment([pos2(x, y), pos2((x + 2.5).min(fin), y)], rappel);
                x += 5.0;
            }
            p.line_segment([depuis, pos2(depuis.x + 2.0, y)], rappel);
            p.circle_filled(depuis, 1.5, bleu);
        }
        // hauteur résultante, le long du mur
        if let Ok((bas_z, haut_z)) = self.parametres_mur().bornes(projet) {
            p.text(
                pos2((x0 + x1) * 0.5, (yh + yb) * 0.5),
                Align2::CENTER_CENTER,
                crate::config::options::unite().afficher(haut_z - bas_z),
                FontId::proportional(8.0),
                ui.visuals().weak_text_color(),
            );
        }
    }

    pub(super) fn nombre_mur(&mut self, ui: &mut egui::Ui, cle: usize, defaut: f64) {
        let v = self
            .valeurs
            .entry((Outil::Mur, cle))
            .or_insert(Valeur::Nombre(defaut));
        if let Valeur::Nombre(v) = v {
            ui.add_sized([130.0, 20.0], egui::DragValue::new(v).speed(10.0).range(1.0..=f64::MAX).dimensions());
        }
    }

}
