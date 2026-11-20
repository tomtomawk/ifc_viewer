//! Éditeur de type composé (fenêtre « Type de mur ») : une ligne par couche, glisser-
//! déposer, matériau du projet ou nouveau, épaisseur, plume, retour, ossature. Le même
//! éditeur sert aux types de terrain (couches de haut en bas, une couche variable au plus,
//! profondeur nominale) : `ouvrir_editeur_type_terrain`.
use super::*;

impl RubanEdition {
    pub fn editeur_type_ouvert(&self) -> bool {
        self.editeur_type.is_some()
    }

    /// Matériau à ouvrir dans la fenêtre Matériau, demandé par un double-clic.
    pub fn prendre_materiau_a_editer(&mut self) -> Option<String> {
        self.materiau_a_editer.take()
    }

    /// Dupliquer un type : l'éditeur s'ouvre sur ses couches, enregistrées sous un
    /// nouveau nom (le type d'origine ne change pas).
    pub(super) fn dupliquer_type(&mut self, projet: &modele::Projet, nom: &str, couches: Vec<modele::Couche>) {
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
                    ed.materiaux.push(modele::murs::assemblage::materiau_par_defaut(m));
                }
            }
            ed.couches = couches;
            if let Some(c) = categorie {
                ed.categorie = c;
            }
        }
    }

    pub(super) fn ouvrir_editeur_type(&mut self, projet: &modele::Projet, id: Option<String>) {
        use modele::{Couche, FonctionCouche};
        let existant = id.as_ref().and_then(|id| projet.type_mur(id));
        let (nom, couches) = match existant {
            Some(t) => (t.nom.clone(), t.couches.clone()),
            None => {
                // générique en cours : sa seule couche ; sinon un mur isolé type
                let mut n = projet.types_murs.len() + 1;
                while projet.types_murs.iter().any(|t| t.nom == format!("Mur composé {n}")) {
                    n += 1;
                }
                let c = |m: &str, e: f64, f| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f, plume: None, retour: false, ossature: None };
                (format!("Mur composé {n}"), vec![c("Enduit", 15.0, FonctionCouche::Finition), c("Béton", 200.0, FonctionCouche::Structure), c("Laine minérale", 120.0, FonctionCouche::Isolation), c("Plâtre", 13.0, FonctionCouche::Finition)])
            }
        };
        let mut materiaux = projet.materiaux.clone();
        for c in &couches {
            if let Some(m) = &c.materiau
                && !materiaux.iter().any(|x| &x.nom == m)
            {
                materiaux.push(modele::murs::assemblage::materiau_par_defaut(m));
            }
        }
        self.editeur_type = Some(EditeurType { id: existant.map(|t| t.id.clone()), nom, categorie: existant.map_or(modele::CategorieMur::Compose, |t| t.categorie_mur()), couches, materiaux, erreur: None, glisse: None, terrain: None, surface: None });
    }

    /// Ouvre l'éditeur sur un type de terrain : ses couches de haut en bas (sans couche :
    /// une seule couche variable de son matériau) et sa profondeur nominale.
    pub(crate) fn ouvrir_editeur_type_terrain(&mut self, projet: &modele::Projet, id: &str) {
        use modele::{Couche, FonctionCouche};
        let Some(t) = projet.types_terrains_disponibles().into_iter().find(|t| t.id == id) else { return };
        let couche = |materiau: Option<String>, epaisseur: f64| Couche { materiau, epaisseur, fonction: FonctionCouche::Autre, plume: None, retour: false, ossature: None };
        let (couches, variables): (Vec<Couche>, Vec<bool>) = if t.couches.is_empty() {
            (vec![couche(t.materiau.clone(), t.epaisseur)], vec![true])
        } else {
            t.couches.iter().map(|c| (couche(c.materiau.clone(), c.epaisseur), c.variable)).unzip()
        };
        let mut materiaux = projet.materiaux.clone();
        for m in couches.iter().filter_map(|c| c.materiau.as_ref()) {
            if !materiaux.iter().any(|x| &x.nom == m) {
                materiaux.push(modele::murs::assemblage::materiau_par_defaut(m));
            }
        }
        self.editeur_type = Some(EditeurType {
            id: Some(t.id.clone()), nom: t.nom.clone(), categorie: modele::CategorieMur::Compose, couches, materiaux, erreur: None, glisse: None,
            terrain: Some(TerrainEdite { profondeur: t.epaisseur, materiau: t.materiau.clone(), variables }),
            surface: None,
        });
    }

    /// Ouvre l'éditeur sur un type d'élément horizontal (couches du dessus vers le
    /// dessous) : le type du projet `source`, ou, avec `copie` ou pour un type absent du
    /// projet (type par défaut), un nouveau type d'après lui.
    pub(super) fn ouvrir_editeur_type_surface(&mut self, projet: &modele::Projet, sorte: modele::SorteDalle, source: &modele::TypeCompose, copie: bool) {
        let existant = (!copie && !source.id.is_empty() && projet.types_dalles.iter().any(|t| t.id == source.id)).then(|| source.id.clone());
        let mut nom = source.nom.clone();
        if existant.is_none() {
            let base = if copie && !source.id.is_empty() { format!("{} (copie)", source.nom) } else { source.nom.clone() };
            nom = base.clone();
            let mut k = 2;
            while projet.types_dalles.iter().any(|t| t.nom == nom) {
                nom = format!("{base} {k}");
                k += 1;
            }
        }
        let mut materiaux = projet.materiaux.clone();
        for m in source.couches.iter().filter_map(|c| c.materiau.as_ref()) {
            if !materiaux.iter().any(|x| &x.nom == m) {
                materiaux.push(modele::murs::assemblage::materiau_par_defaut(m));
            }
        }
        self.editeur_type = Some(EditeurType {
            id: existant, nom, categorie: modele::CategorieMur::Compose, couches: source.couches.clone(), materiaux, erreur: None, glisse: None,
            terrain: None, surface: Some(sorte),
        });
    }

    /// Fenêtre d'édition d'un type de mur composé : une ligne par couche, de
    /// l'extérieur (en haut) vers l'intérieur. Retourne vrai si le projet a changé.
    pub fn editeur_type_mur(&mut self, ctx: &egui::Context, projet: &mut modele::Projet) -> bool {
        use modele::{Couche, FonctionCouche};
        let Some(ed) = &mut self.editeur_type else { return false };
        // matériaux existants : toujours ceux du projet (modifiés dans la fenêtre Matériau)
        for m in &mut ed.materiaux {
            if let Some(x) = projet.materiau(&m.nom) {
                *m = x.clone();
            }
        }
        let mut a_editer: Option<String> = None;
        let (mut ouvert, mut enregistrer, mut annuler) = (true, false, false);
        let terrain = ed.terrain.is_some();
        let mut variable_clic: Option<usize> = None;
        let surface = ed.surface;
        let titre = match (terrain, surface, ed.id.is_some()) {
            (true, _, _) => "Modifier le type de terrain".to_string(),
            (_, Some(s), true) => format!("Modifier le type de {}", s.nom().to_lowercase()),
            (_, Some(s), false) => format!("Nouveau type de {}", s.nom().to_lowercase()),
            (_, None, true) => "Modifier le type de mur".to_string(),
            (_, None, false) => "Nouveau type de mur".to_string(),
        };
        egui::Window::new(&titre)
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
                    if let Some(t) = ed.terrain.as_mut() {
                        ui.label("Profondeur nominale").on_hover_text("Du point le plus bas du terrain naturel au fond plat, pour tous les terrains de ce type");
                        ui.add(egui::DragValue::new(&mut t.profondeur).speed(10.0).range(1.0..=1e6).dimensions());
                    } else if surface.is_none() {
                        egui::ComboBox::from_id_salt("categorie_type_mur").selected_text(ed.categorie.nom()).show_ui(ui, |ui| {
                            for c in modele::CategorieMur::TOUTES {
                                ui.selectable_value(&mut ed.categorie, c, c.nom());
                            }
                        }).response.on_hover_text("Catégorie du type : mur, mur composé ou cloison");
                    }
                });
                ui.add_space(4.0);
                bande_couches_brouillon(ui, projet, &ed.materiaux, &ed.couches, 18.0);
                ui.label(RichText::new(if terrain { "Dessus (terrain naturel)" } else if surface.is_some() { "Dessus" } else { "Extérieur" }).small().weak());
                let mut action: Option<(usize, i32)> = None; // (couche, 0 supprimer)
                // glisser-déposer : lignes (hauteur) et relâchement de la poignée
                let mut lignes: Vec<egui::Rangef> = Vec::new();
                let mut lacher = false;
                let entetes: &[&str] = if terrain { &["", "Matériau", "", "Motif", "Épaisseur", "Variable", ""] }
                    else if surface.is_some() { &["", "Fonction", "Matériau", "", "Motif", "Épaisseur", "Plume", "Ossature", ""] }
                    else { &["", "Fonction", "Matériau", "", "Motif", "Épaisseur", "Plume", "Retour", "Ossature", ""] };
                egui::Grid::new("couches_type_mur").num_columns(entetes.len()).spacing([6.0, 4.0]).striped(true).show(ui, |ui| {
                    for t in entetes {
                        ui.label(RichText::new(*t).small().weak());
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
                        if !terrain {
                            egui::ComboBox::from_id_salt(("fonction", k)).width(84.0).selected_text(c.fonction.libelle()).show_ui(ui, |ui| {
                                for f in FonctionCouche::TOUTES {
                                    ui.selectable_value(&mut c.fonction, f, f.libelle());
                                }
                            });
                        }
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
                            ed.materiaux.push(modele::murs::assemblage::materiau_par_defaut(&nom));
                        }
                        // couleur et motif : hérités du matériau, affichés en lecture seule
                        match ed.materiaux.iter().find(|m| m.nom == nom) {
                            Some(m) => {
                                let [r, g, b] = m.couleur;
                                let existe = projet.materiau(&m.nom).is_some();
                                let aide = if existe { "Hérité du matériau · double-clic : modifier le matériau" } else { "Hérité du matériau · enregistrez le type pour pouvoir modifier ce nouveau matériau" };
                                let (rect, rep) = ui.allocate_exact_size(egui::vec2(20.0, 16.0), Sense::click());
                                ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(r, g, b));
                                ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, ui.visuals().weak_text_color()), egui::StrokeKind::Inside);
                                let rep2 = ui.add(egui::Label::new(m.motif.libelle()).sense(Sense::click()));
                                let double = rep.double_clicked() || rep2.double_clicked();
                                rep.on_hover_text(aide);
                                rep2.on_hover_text(aide);
                                if double && existe {
                                    a_editer = Some(m.nom.clone());
                                }
                            }
                            None => {
                                ui.label("");
                                ui.label("");
                            }
                        }
                        // couche variable d'un terrain : épaisseur donnée par la profondeur
                        let variable = ed.terrain.as_ref().is_some_and(|t| t.variables.get(k).copied().unwrap_or(false));
                        ui.add_enabled(!variable, egui::DragValue::new(&mut c.epaisseur).speed(1.0).range(1.0..=100_000.0).max_decimals(1).dimensions())
                            .on_disabled_hover_text("Couche variable : elle comble jusqu'au fond plat");
                        if terrain {
                            if ui.radio(variable, "").on_hover_text("Couche variable : comble jusqu'au fond plat (une seule par type)").clicked() {
                                variable_clic = Some(k);
                            }
                            if petit_bouton(ui, n > 1, Glyphe::Croix).on_hover_text("Supprimer la couche").clicked() { action = Some((k, 0)); }
                            ui.end_row();
                            continue;
                        }
                        // plume du contour de coupe : numéro et épaisseur de la table
                        let plumes = projet.epaisseurs_plumes();
                        let mut n_plume = c.plume();
                        egui::ComboBox::from_id_salt(("plume", k)).width(70.0).selected_text(format!("{n_plume} · {}", noyau::nombres::format_real(plumes[(n_plume - 1) as usize]))).show_ui(ui, |ui| {
                            for (i, e) in plumes.iter().enumerate() {
                                ui.selectable_value(&mut n_plume, i as u8 + 1, format!("{} · {} mm", i + 1, noyau::nombres::format_real(*e)));
                            }
                        }).response.on_hover_text("Plume du contour de coupe de la couche");
                        if n_plume != c.plume() {
                            c.plume = Some(n_plume);
                        }
                        // retour aux insertions : murs seulement
                        if surface.is_none() {
                            ui.add_enabled(c.fonction != FonctionCouche::Structure, egui::Checkbox::without_text(&mut c.retour))
                                .on_hover_text("Retour aux insertions : la couche tourne dans les tableaux et le linteau des ouvertures, jusqu'au plan « Retour extérieur » ou « Retour intérieur » de leur famille")
                                .on_disabled_hover_text("Une couche de structure ne se retourne pas");
                        }
                        // ossature : montants dans l'épaisseur de la couche, réglés dans un menu
                        let resume = c.ossature.as_ref().map_or("—".to_string(), |o| format!("{:.0} / {:.0}", o.largeur, o.entraxe));
                        let bouton = ui.button(resume).on_hover_text("Ossature : montants à entraxe régulier dans la couche (largeur / entraxe, mm). Le matériau de la couche remplit les travées.");
                        egui::Popup::menu(&bouton).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                            let mut active = c.ossature.is_some();
                            if ui.checkbox(&mut active, "Couche à ossature").changed() {
                                c.ossature = active.then(Default::default);
                            }
                            if let Some(o) = &mut c.ossature {
                                egui::Grid::new("ossature_couche").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                                    ui.label("Largeur des montants");
                                    ui.add(egui::DragValue::new(&mut o.largeur).speed(1.0).range(5.0..=500.0).max_decimals(1).dimensions());
                                    ui.end_row();
                                    ui.label("Entraxe");
                                    ui.add(egui::DragValue::new(&mut o.entraxe).speed(5.0).range(50.0..=5000.0).max_decimals(1).dimensions());
                                    ui.end_row();
                                    ui.label("Matériau des montants");
                                    let mut nom = o.materiau.clone().unwrap_or_default();
                                    ui.add(egui::TextEdit::singleline(&mut nom).desired_width(130.0).hint_text(modele::murs::ossature::MATERIAU_OSSATURE));
                                    let nom = nom.trim().to_string();
                                    o.materiau = (!nom.is_empty()).then_some(nom);
                                    ui.end_row();
                                });
                                egui::Grid::new("ossature_plume").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                                    ui.label("Plume du remplissage").on_hover_text("Contour de l'isolant entre les membrures, en coupe. Les membrures gardent la plume de la couche.");
                                    let mut plume = o.plume_remplissage();
                                    if ui.add(egui::DragValue::new(&mut plume).range(1..=12)).changed() {
                                        o.plume_remplissage = (plume != modele::murs::ossature::PLUME_REMPLISSAGE).then_some(plume);
                                    }
                                    ui.end_row();
                                });
                                ui.checkbox(&mut o.horizontale, "Ossature horizontale").on_hover_text("Lisses horizontales à l'entraxe sur la hauteur du mur, au lieu de montants (contre-ossature, bardage vertical).");
                                ui.checkbox(&mut o.lisses, "Lisses, linteaux et appuis").on_hover_text("Lisses basse et haute ; linteau et appui autour des ouvertures. Les montants de part et d'autre des ouvertures sont toujours ajoutés.");
                                if o.entraxe <= o.largeur {
                                    ui.colored_label(ui.visuals().error_fg_color, "L'entraxe doit dépasser la largeur.");
                                }
                            }
                        });
                        if petit_bouton(ui, n > 1, Glyphe::Croix).on_hover_text("Supprimer la couche").clicked() { action = Some((k, 0)); }
                        ui.end_row();
                    }
                });
                ui.label(RichText::new(if terrain { "Fond" } else if surface.is_some() { "Dessous" } else { "Intérieur" }).small().weak());
                if let (Some(k), Some(t)) = (variable_clic, ed.terrain.as_mut()) {
                    let avant = t.variables.get(k).copied().unwrap_or(false);
                    t.variables.iter_mut().enumerate().for_each(|(j, v)| *v = j == k && !avant);
                }
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
                        if let Some(t) = ed.terrain.as_mut() {
                            let v = t.variables.remove(k);
                            t.variables.insert(cible.min(t.variables.len()), v);
                        }
                        ed.glisse = None;
                    }
                }
                if lacher {
                    ed.glisse = None;
                }
                if let Some((k, 0)) = action {
                    ed.couches.remove(k);
                    if let Some(t) = ed.terrain.as_mut() { t.variables.remove(k); }
                }
                ui.horizontal(|ui| {
                    if ui.button("+ Couche").clicked() {
                        ed.couches.push(Couche { materiau: None, epaisseur: if terrain { 300.0 } else { 50.0 }, fonction: FonctionCouche::Autre, plume: None, retour: false, ossature: None });
                        if let Some(t) = ed.terrain.as_mut() { t.variables.push(false); }
                    }
                    if !terrain {
                        let total: f64 = ed.couches.iter().map(|c| c.epaisseur).sum();
                        ui.label(format!("Épaisseur totale : {}", modele::unite::unite().afficher(total)));
                    }
                });
                if let Some(e) = &ed.erreur {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                ui.label(RichText::new(if surface.is_some() {
                    "Couches du dessus vers le dessous. Les éléments déjà tracés de ce type suivent ses couches ; l'épaisseur de l'élément est leur somme."
                } else if terrain {
                    "Couches de haut en bas sous la surface du terrain ; la couche variable (à défaut, la dernière) comble jusqu'au fond plat. Tous les terrains de ce type suivent."
                } else {
                    "Chaque couche sera tracée comme un mur indépendant, regroupé avec les autres en assemblage. Les murs déjà tracés gardent leurs couches."
                }).small().weak());
                ui.separator();
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), egui::Layout::right_to_left(Align::Center), |ui| {
                    annuler = ui.button("Annuler").clicked();
                    enregistrer = ui.button(RichText::new("Enregistrer").strong()).clicked();
                });
            });
        if a_editer.is_some() {
            self.materiau_a_editer = a_editer;
        }
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
        if let Some(t) = ed.terrain.take() {
            let ok = enregistrer_terrain(projet, ed, nom, &t);
            ed.terrain = Some(t);
            if ok { self.editeur_type = None; }
            return ok;
        }
        if ed.couches.iter().any(|c| c.ossature.as_ref().is_some_and(|o| !o.valide())) {
            ed.erreur = Some("Ossature : l'entraxe doit dépasser la largeur des montants.".into());
            return false;
        }
        if let Some(sorte) = ed.surface {
            let id = enregistrer_surface(projet, ed, nom, sorte);
            if let Some(t) = id.and_then(|id| projet.types_dalles.iter().find(|t| t.id == id)).cloned() {
                let outil = Outil::TOUTES_SURFACES.into_iter().find(|o| sorte_surface(*o) == Some(sorte));
                if let Some(o) = outil { self.type_surface.insert(o, t); }
                self.editeur_type = None;
                return true;
            }
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
                let id = noyau::uuid::uuid();
                projet.types_murs.push(modele::TypeCompose { sorte: None, id: id.clone(), nom, couches: ed.couches.clone(), categorie: Some(ed.categorie), couches_detaillees: true, origine: None });
                id
            }
        };
        // matériaux nouveaux utilisés : ajoutés au projet (les existants ne sont pas modifiés ici)
        for m in &ed.materiaux {
            if ed.couches.iter().any(|c| c.materiau.as_ref() == Some(&m.nom)) && projet.materiau(&m.nom).is_none() {
                projet.materiaux.push(m.clone());
            }
        }
        projet.completer_materiaux();
        self.type_mur = Some(id);
        self.editeur_type = None;
        true
    }

}

/// Enregistre le brouillon d'un type de terrain dans le projet. Une seule couche variable
/// s'écrit comme un type sans couches (son matériau) ; sinon les couches, de haut en bas.
fn enregistrer_terrain(projet: &mut modele::Projet, ed: &mut EditeurType, nom: String, t: &TerrainEdite) -> bool {
    use modele::elements::terrain_couches::CoucheTerrain;
    let id = ed.id.clone().unwrap_or_default();
    if projet.types_terrains.iter().any(|x| x.nom == nom && x.id != id) {
        ed.erreur = Some("Un autre type de terrain porte déjà ce nom.".into());
        return false;
    }
    if t.variables.iter().filter(|v| **v).count() > 1 {
        ed.erreur = Some("Une seule couche variable par type de terrain.".into());
        return false;
    }
    let (couches, materiau) = match (ed.couches.as_slice(), t.variables.as_slice()) {
        ([c], [true]) => (Vec::new(), c.materiau.clone().or(t.materiau.clone())),
        _ => (ed.couches.iter().zip(&t.variables).map(|(c, v)| CoucheTerrain { materiau: c.materiau.clone(), epaisseur: c.epaisseur, variable: *v }).collect(), t.materiau.clone()),
    };
    projet.fixer_types_terrains();
    let Some(x) = projet.types_terrains.iter_mut().find(|x| x.id == id) else {
        ed.erreur = Some("Type de terrain introuvable.".into());
        return false;
    };
    (x.nom, x.epaisseur, x.materiau, x.couches) = (nom, t.profondeur, materiau, couches);
    for m in &ed.materiaux {
        if ed.couches.iter().any(|c| c.materiau.as_ref() == Some(&m.nom)) && projet.materiau(&m.nom).is_none() {
            projet.materiaux.push(m.clone());
        }
    }
    projet.completer_materiaux();
    true
}

/// Enregistre le brouillon d'un type d'élément horizontal dans `types_dalles`, avec sa
/// sorte ; renvoie son identifiant. Les éléments du type suivent ses couches.
fn enregistrer_surface(projet: &mut modele::Projet, ed: &mut EditeurType, nom: String, sorte: modele::SorteDalle) -> Option<String> {
    if projet.types_dalles.iter().any(|t| t.nom == nom && Some(&t.id) != ed.id.as_ref()) {
        ed.erreur = Some("Un autre type d'élément horizontal porte déjà ce nom.".into());
        return None;
    }
    let id = match ed.id.as_ref().and_then(|id| projet.types_dalles.iter_mut().find(|t| &t.id == id)) {
        Some(t) => {
            (t.nom, t.couches, t.couches_detaillees, t.sorte) = (nom, ed.couches.clone(), true, Some(sorte));
            t.id.clone()
        }
        None => {
            let id = format!("type-dalle-{}", noyau::uuid::uuid());
            projet.types_dalles.push(modele::TypeCompose { id: id.clone(), nom, couches: ed.couches.clone(), categorie: None, couches_detaillees: true, origine: None, sorte: Some(sorte) });
            id
        }
    };
    for m in &ed.materiaux {
        if ed.couches.iter().any(|c| c.materiau.as_ref() == Some(&m.nom)) && projet.materiau(&m.nom).is_none() {
            projet.materiaux.push(m.clone());
        }
    }
    projet.completer_materiaux();
    Some(id)
}
