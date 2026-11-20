//! Propriétés, types et croquis des murs.
use super::*;
use crate::outils::croquis::{self, CroquisVertical};

impl RubanEdition {
    pub(crate) fn implantation_suivante(&mut self) {
        let choix = ["Face extérieure", "Axe", "Face intérieure", "Axe du porteur"];
        let valeur = self.valeurs.entry((Outil::Mur, 0)).or_insert_with(|| Valeur::Texte(choix[0].into()));
        if let Valeur::Texte(nom) = valeur {
            let suivant = choix.iter().position(|c| *c == nom).map_or(0, |i| (i + 1) % choix.len());
            *nom = choix[suivant].into();
        }
    }

    /// Sélecteur de type des prochains murs, à la place du sélecteur standard de Propriétés.
    pub fn selecteur_type_creation_mur(&mut self, ui: &mut egui::Ui, projet: &modele::Projet) {
        if !self.mur_actif() {
            return;
        }
        if self.type_mur.as_ref().is_some_and(|id| projet.type_mur(id).is_none()) {
            self.type_mur = None;
        }
        self.selecteur_type_mur(ui, projet, true);
        ui.add_space(2.0);
    }

    /// Emplacement « schéma » du mur : croquis des deux limites (haut, bas).
    pub fn schema_mur(&mut self, ui: &mut egui::Ui, projet: &modele::Projet) {
        if !self.mur_actif() {
            return;
        }
        // Un changement de document ne doit jamais réutiliser une référence étrangère.
        for niveau in [&mut self.niveau_mur, &mut self.niveau_haut_mur] {
            if niveau.as_ref().is_some_and(|id| projet.niveau(id).is_none()) {
                *niveau = None;
            }
        }
        croquis::cadre(ui, |ui| self.croquis_mur_creation(ui, projet));
        if let Err(e) = self.parametres_mur().bornes(projet) {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
    }

    /// Emplacement « paramètres » du mur : épaisseur (celle du type, ou saisie) et implantation.
    pub fn parametres_mur_creation(&mut self, ui: &mut egui::Ui, projet: &modele::Projet) {
        if !self.mur_actif() {
            return;
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Paramètres — Mur").strong());
        egui::Grid::new("grille_creation_mur")
            .num_columns(2)
            .striped(true)
            .spacing([10.0, 4.0])
            .min_col_width(80.0)
            .show(ui, |ui| {
                ui.label("Épaisseur");
                match self.type_mur.as_ref().and_then(|id| projet.type_mur(id)) {
                    Some(t) => {
                        ui.label(modele::unite::unite().afficher(t.epaisseur()))
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
    }

    /// Sélecteur de type en tête de la palette, comme dans Revit : générique (épaisseur
    /// saisie) ou un type du projet ; composition en bande sous la liste ; Modifier /
    /// Nouveau ouvrent l'éditeur de type.
    pub fn selecteur_type_mur(&mut self, ui: &mut egui::Ui, projet: &modele::Projet, generique: bool) {
        use modele::CategorieMur;
        let actuel = self.type_mur.as_ref().and_then(|id| projet.type_mur(id));
        let categorie_actuelle = actuel.map_or(CategorieMur::Mur, |t| t.categorie_mur());
        let nom = actuel.map_or("Générique (épaisseur libre)", |t| t.nom.as_str());
        let onglet = self.categorie_mur.unwrap_or(categorie_actuelle);
        // largeur du panneau mesurée hors de la ligne, qui grandirait avec la liste
        let largeur = ui.available_width();
        // types de la catégorie : (type du projet, type du gabarit à ajouter, nom, couches)
        type Ligne = (Option<String>, Option<modele::gabarit::TypeGabarit>, String, Vec<modele::Couche>);
        let mut lignes: Vec<Ligne> = Vec::new();
        if generique && onglet == CategorieMur::Mur {
            lignes.push((None, None, "Générique (épaisseur libre)".into(), Vec::new()));
        }
        let mut types: Vec<_> = projet.types_murs.iter().filter(|t| t.categorie_mur() == onglet).collect();
        types.sort_by(|a, b| a.nom.cmp(&b.nom));
        lignes.extend(types.into_iter().map(|t| (Some(t.id.clone()), None, t.nom.clone(), t.couches.clone())));
        // types du gabarit absents du projet : copiés dans le projet au choix
        let gabarit = modele::gabarit::actif();
        lignes.extend(gabarit.types_murs.iter().filter(|t| t.categorie_mur() == onglet && !projet.types_murs.iter().any(|p| p.nom == t.nom)).map(|t| (None, Some(t.clone()), t.nom.clone(), t.couches.clone())));
        let rangs = CategorieMur::TOUTES.iter().map(|c| projet.types_murs.iter().filter(|t| t.categorie_mur() == *c).count() + gabarit.types_murs.iter().filter(|t| t.categorie_mur() == *c).count() + 1).max().unwrap_or(1);
        let mut choix_onglet = None;
        let mut choisi: Option<Ligne> = None;
        let mut editer: Option<String> = None;
        let mut dupliquer: Option<(String, Vec<modele::Couche>)> = None;
        let mut nouveau = false;
        let unite = modele::unite::unite();
        let epaisseur = |c: &[modele::Couche]| c.iter().map(|c| c.epaisseur).sum::<f64>();

        let largeur_texte = |ui: &egui::Ui, t: &str, f: &egui::FontId| ui.fonts_mut(|x| x.layout_no_wrap(t.to_string(), f.clone(), Color32::WHITE).size().x);
        // Sélecteur fermé : catégorie, nom et épaisseur en haut ; dessous, sur toute la
        // largeur, le plan des couches (de l'extérieur vers l'intérieur).
        let couches_actuelles = actuel.map(|t| t.couches.clone()).unwrap_or_default();
        let hauteur = if couches_actuelles.is_empty() { 30.0 } else { 56.0 };
        let (r, _) = ui.allocate_exact_size(egui::vec2(largeur, hauteur), Sense::hover());
        let zone_crayon = actuel.map(|_| egui::Rect::from_min_max(egui::pos2(r.right() - 27.0, r.top() + 29.0), egui::pos2(r.right() - 4.0, r.bottom() - 3.0)));
        // Le crayon possède sa propre zone cliquable, distincte du sélecteur de type.
        let bouton = if let Some(zone) = zone_crayon {
            let haut = egui::Rect::from_min_max(r.min, egui::pos2(r.right(), zone.top()));
            let bas = egui::Rect::from_min_max(egui::pos2(r.left(), zone.top()), egui::pos2(zone.left(), r.bottom()));
            ui.interact(haut, ui.scope_id().with("type_mur_selecteur_haut"), Sense::click())
                .union(ui.interact(bas, ui.scope_id().with("type_mur_selecteur_bas"), Sense::click()))
        } else { ui.interact(r, ui.scope_id().with("type_mur_selecteur"), Sense::click()) };
        let visuel = *ui.style().interact(&bouton);
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
            if let Some(c) = peindre_bande_couches(ui.painter(), plan, projet, &projet.materiaux, &couches_actuelles, bouton.hover_pos()) {
                bulle = texte_couche(c);
            }
        }
        if let (Some(zone), Some(type_mur)) = (zone_crayon, actuel) {
            let survol = ui.rect_contains_pointer(r);
            let crayon = ui.interact(zone, ui.scope_id().with("type_mur_modifier_courant"), Sense::click())
                .on_hover_text("Modifier le type de mur");
            if survol || crayon.hovered() {
                ui.painter().rect_filled(zone, 3.0, if crayon.hovered() { ui.visuals().widgets.hovered.bg_fill } else { visuel.bg_fill });
                icone_action_type(ui.painter(), zone.shrink(5.0), true, ui.visuals().text_color());
            }
            if crayon.clicked() { editer = Some(type_mur.id.clone()); }
        }
        let bouton = bouton.on_hover_text(bulle);
        let largeur_liste = selecteur::largeur_liste(ui, lignes.iter().map(|l| &l.2), 64.0 + 50.0, CategorieMur::TOUTES.len(), 96.0);
        selecteur::liste(&bouton, largeur_liste, selecteur::HAUTEUR_CADRE + rangs as f32 * selecteur::HAUTEUR_LIGNE, |ui| {
            choix_onglet = selecteur::onglets(ui, &CategorieMur::TOUTES, onglet, 96.0, |c| c.nom().into(), onglet.aide(), icone_categorie_mur);
            for ligne in &lignes {
                let courant = match (&ligne.0, &self.type_mur) {
                    (None, None) => ligne.1.is_none(),
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                };
                let actions: &[(selecteur::ActionType, &str)] = match (ligne.3.is_empty(), ligne.0.is_some()) {
                    (true, _) => &[],
                    (false, true) => &[(selecteur::ActionType::Dupliquer, "Dupliquer : nouveau type d'après celui-ci"), (selecteur::ActionType::Modifier, "Modifier le type")],
                    (false, false) => &[(selecteur::ActionType::Dupliquer, "Dupliquer : nouveau type d'après celui-ci")],
                };
                let l = selecteur::Ligne {
                    nom: &ligne.2,
                    courant,
                    etiquette: ligne.1.as_ref().map(|_| "gabarit"),
                    sous_titre: None,
                    detail: (!ligne.3.is_empty()).then(|| unite.afficher(epaisseur(&ligne.3))),
                    actions,
                };
                let mut bande = |ui: &mut egui::Ui, r: egui::Rect| {
                    if !ligne.3.is_empty() {
                        peindre_bande_couches(ui.painter(), r.shrink2(egui::vec2(0.0, 3.0)), projet, &projet.materiaux, &ligne.3, None);
                    }
                };
                let clic = selecteur::ligne(ui, &l, Some(&mut bande));
                match clic.action {
                    Some(selecteur::ActionType::Modifier) => editer = ligne.0.clone(),
                    Some(selecteur::ActionType::Dupliquer) => dupliquer = Some((ligne.2.clone(), ligne.3.clone())),
                    None if clic.choisi => choisi = Some(ligne.clone()),
                    None => {}
                }
            }
            nouveau = selecteur::ligne_ajout(ui, "Nouveau type de mur…");
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
    pub fn prendre_type_gabarit(&mut self) -> Option<modele::gabarit::TypeGabarit> {
        self.type_mur_gabarit.take()
    }

    /// Crée (ou remplace) un type composé et le choisit pour les prochains murs (pilote).
    pub fn definir_type_mur(&mut self, projet: &mut modele::Projet, nom: &str, couches: Vec<modele::Couche>, categorie: Option<modele::CategorieMur>) -> Result<String, String> {
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
                let id = noyau::uuid::uuid();
                projet.types_murs.push(modele::TypeCompose { sorte: None, id: id.clone(), nom: nom.into(), couches, categorie, couches_detaillees: true, origine: None });
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
    pub(super) fn croquis_mur_creation(&mut self, ui: &mut egui::Ui, projet: &modele::Projet) {
        let croquis = CroquisVertical::lignes(ui, |ui, haut| {
            let niveau = if haut { &mut self.niveau_haut_mur } else { &mut self.niveau_mur };
            let defaut = if haut { "Libre" } else { projet.niveaux.first().map_or("Niveau 0", |n| &n.nom) };
            let libre = if haut { Some("Libre (hauteur)") } else { projet.niveaux.is_empty().then_some("Niveau 0") };
            croquis::champ_niveau(ui, ("limite_creation_mur", haut), &projet.niveaux, niveau, defaut, libre);
            let hauteur_libre = haut && niveau.is_none();
            let (valeur, info) = if hauteur_libre {
                self.decalage_haut_mur = 0.0;
                (self.valeurs.entry((Outil::Mur, 1)).or_insert(Valeur::Nombre(2800.0)), "Hauteur du mur")
            } else if haut {
                ui.add_sized([72.0, 20.0], egui::DragValue::new(&mut self.decalage_haut_mur).speed(10.0).dimensions())
                    .on_hover_text("Décalage par rapport au niveau");
                return;
            } else {
                (self.valeurs.entry((Outil::Mur, 3)).or_insert(Valeur::Nombre(0.0)), "Décalage par rapport au niveau")
            };
            if let Valeur::Nombre(v) = valeur {
                let mut champ = egui::DragValue::new(v).speed(10.0);
                if hauteur_libre { champ = champ.range(1.0..=f64::MAX); }
                ui.add_sized([72.0, 20.0], champ.dimensions()).on_hover_text(info);
            }
        });
        let parametres = self.parametres_mur();
        let [haut, bas] = croquis.references();
        let (haut, bas) = limites_croquis_mur(&parametres, projet, haut, bas);
        let mesure = parametres.bornes(projet).ok().map(|(bas, haut)| haut - bas);
        croquis.peindre(ui, [haut, bas], 28.0, mesure);
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
