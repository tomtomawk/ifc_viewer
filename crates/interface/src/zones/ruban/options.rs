//! Propriétés des ouvertures et surfaces, options des outils.
use super::*;

impl RubanEdition {
    /// Sélecteur de création placé en tête de la palette Propriétés, comme un
    /// sélecteur de type : vignette, choix explicite, puis paramètres de l'outil.
    /// Retourne false pour laisser la palette afficher la sélection du modèle.
    /// Placement d'une ouverture : sélecteur de type en tête de Propriétés (générique :
    /// cotes du ruban), édition de la famille du type ou nouvelle famille.
    pub fn proprietes_ouverture(&mut self, ui: &mut egui::Ui, projet: Option<&modele::Projet>) -> bool {
        if self.selection != Some(Outil::Ouverture) {
            return false;
        }
        use modele::GenreOuverture;
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
        // toujours une famille, la première du projet à défaut ; sans famille de porte ou
        // de fenêtre dans le projet, il faut en charger une ; une ouverture libre sans
        // type en reçoit un à la pose (un seul vide, aux cotes du ruban)
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

    /// Emplacement « sélecteur » de l'élément horizontal : type, et sorte (onglets).
    pub fn selecteur_surface(&mut self, ui: &mut egui::Ui, projet: Option<&modele::Projet>) {
        let Some(mut surface) = self.selection.filter(|o| o.commande() == Outil::Surfaces) else {
            return;
        };
        let projet_vide = modele::Projet::default();
        let projet_apercu = projet.unwrap_or(&projet_vide);
        ui.add_space(6.0);
        {
            let types = types_surface(projet, surface);
            // type choisi : recopié du projet, ses couches suivent les modifications
            let courant = self.type_surface.get(&surface)
                .and_then(|c| types.iter().find(|t| (!c.id.is_empty() && t.id == c.id) || t.nom == c.nom))
                .cloned().unwrap_or_else(|| types[0].clone());
            self.type_surface.insert(surface, courant.clone());
            let identique = self.surface_identique.as_ref().filter(|(o, _)| *o == surface).map(|(_, t)| t);
            let affiche = identique.unwrap_or(&courant);
            let unite = modele::unite::unite();
            let titre = format!("{} · {}", surface.nom(), affiche.nom);
            let vignette = |p: &egui::Painter, r: egui::Rect, c: Color32| icone(p, r, surface, c);
            let detail = (!affiche.couches.is_empty()).then(|| unite.afficher(affiche.epaisseur()));
            let bouton = selecteur::entete(ui, &titre, detail.as_deref(), &format!("{titre}
Type des prochains éléments horizontaux."), Some(&vignette));
            let liste = if surface == Outil::TerrePlein { TERRE_PLEIN } else { SURFACES };
            let sortes: Vec<Outil> = liste.iter().map(|(o, _)| *o).collect();
            let rangs = sortes.iter().map(|o| types_surface(projet, *o).len()).max().unwrap_or(0) + 1;
            let noms: Vec<String> = sortes.iter().flat_map(|o| types_surface(projet, *o)).map(|t| t.nom).collect();
            let largeur_liste = selecteur::largeur_liste(ui, noms.iter(), 90.0, sortes.len(), 74.0);
            let aide = SURFACES.iter().chain(TERRE_PLEIN).find(|(o, _)| *o == surface).map_or("", |(_, a)| *a);
            let actions: &[(selecteur::ActionType, &str)] = &[(selecteur::ActionType::Dupliquer, "Dupliquer : nouveau type d'après celui-ci"), (selecteur::ActionType::Modifier, "Modifier le type : couches, matériaux, épaisseurs")];
            let mut choix_onglet: Option<Outil> = None;
            let mut choix_type: Option<modele::TypeCompose> = None;
            let mut editer: Option<(modele::TypeCompose, bool)> = None;
            let mut nouveau = false;
            selecteur::liste(&bouton, largeur_liste, selecteur::HAUTEUR_CADRE + rangs as f32 * selecteur::HAUTEUR_LIGNE, |ui| {
                if let Some(o) = selecteur::onglets(ui, &sortes, surface, 74.0, |o| o.nom().into(), aide, icone) {
                    choix_onglet = Some(o);
                }
                for t in &types {
                    let l = selecteur::Ligne { nom: &t.nom, courant: identique.is_none() && t.nom == courant.nom && t.id == courant.id, etiquette: t.id.is_empty().then_some("par défaut"), sous_titre: None, detail: Some(unite.afficher(t.epaisseur())), actions };
                    let mut pile = |ui: &mut egui::Ui, r: egui::Rect| pile_couches(ui, egui::Rect::from_center_size(r.center(), egui::vec2(26.0, 16.0)), projet_apercu, &t.couches);
                    let clic = selecteur::ligne(ui, &l, Some(&mut pile));
                    match clic.action {
                        Some(selecteur::ActionType::Modifier) => editer = Some((t.clone(), false)),
                        Some(selecteur::ActionType::Dupliquer) => editer = Some((t.clone(), true)),
                        None if clic.choisi => choix_type = Some(t.clone()),
                        None => {}
                    }
                }
                nouveau = selecteur::ligne_ajout(ui, &format!("Nouveau type de {}…", surface.nom().to_lowercase()));
            });
            if let Some(o) = choix_onglet {
                surface = o;
                self.surface_identique = None;
            }
            if let Some(t) = choix_type {
                self.type_surface.insert(surface, t);
                self.surface_identique = None;
            }
            // éditeur des types de murs, en mode élément horizontal
            if let (Some(p), Some(sorte)) = (projet, sorte_surface(surface)) {
                if let Some((t, copie)) = editer {
                    self.ouvrir_editeur_type_surface(p, sorte, &t, copie);
                } else if nouveau {
                    self.ouvrir_editeur_type_surface(p, sorte, &modele::TypeCompose::surface_par_defaut(sorte), true);
                }
            }
        }
        self.selection = Some(surface);
        if surface != Outil::TerrePlein { self.derniere_surface = Some(surface); }
    }

    /// Emplacement « schéma » de l'élément horizontal : composition du type choisi.
    pub fn schema_surface(&mut self, ui: &mut egui::Ui, projet: Option<&modele::Projet>) {
        let Some(surface) = self.selection.filter(|o| o.commande() == Outil::Surfaces) else {
            return;
        };
        let projet_vide = modele::Projet::default();
        let projet_apercu = projet.unwrap_or(&projet_vide);
        let unite = modele::unite::unite();
        {
            // composition du type choisi, du haut vers le bas
            let types = types_surface(projet, surface);
            let choisi = self.type_surface.get(&surface).and_then(|c| types.iter().find(|t| t.nom == c.nom && t.id == c.id)).or(types.first());
            if let Some(couches) = choisi.map(|t| &t.couches) {
                ui.add_space(4.0);
                for c in couches {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 12.0), Sense::hover());
                        pile_couches(ui, r, projet_apercu, std::slice::from_ref(c));
                        ui.label(RichText::new(format!("{} · {}", c.materiau.as_deref().unwrap_or("Couche"), unite.afficher(c.epaisseur))).small());
                        ui.label(RichText::new(c.fonction.libelle()).small().weak());
                    });
                }
            }
        }
    }

    /// Emplacement « paramètres » de l'élément horizontal : niveau, pente, décalage.
    pub fn parametres_surface(&mut self, ui: &mut egui::Ui, projet: Option<&modele::Projet>) {
        let Some(surface) = self.selection.filter(|o| o.commande() == Outil::Surfaces) else {
            return;
        };
        ui.add_space(8.0);
        ui.label(RichText::new(format!("Paramètres — {}", surface.nom())).strong());
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
                let epaisseur_type: Option<f64> = self.surface_identique.as_ref().filter(|(o,_)| *o == surface).map(|(_, t)| t.epaisseur())
                    .or_else(|| self.type_surface.get(&surface).map(|t| t.epaisseur()))
                    .or_else(|| types_surface(projet, surface).first().map(|t| t.epaisseur()));
                for (indice, champ) in champs(surface).into_iter().enumerate() {
                    if surface == Outil::Toiture && nom_champ(&champ) == "Pente" && self.toiture_plate {
                        continue;
                    }
                    if let Champ::Nombre(nom, defaut, suffixe) = champ {
                        ui.label(nom);
                        // épaisseur : celle du type choisi
                        if let (Some(e), "Épaisseur") = (epaisseur_type, nom) {
                            ui.label(modele::unite::unite().afficher(e)).on_hover_text("Épaisseur du type (somme des couches)");
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
                    } else if let Champ::Choix(nom, options) = champ {
                        // terre-plein : talus appliqué à toutes les arêtes tracées
                        ui.label(nom);
                        let mut v = self.texte_option(surface, indice);
                        egui::ComboBox::from_id_salt(("choix_surface", surface, indice)).width(90.0).selected_text(v.clone()).show_ui(ui, |ui| {
                            for o in options {
                                ui.selectable_value(&mut v, o.to_string(), *o);
                            }
                        }).response.on_hover_text("Pente du talus en % (hauteur / base : 100 % = 1/1) de toutes les arêtes tracées ; chaque arête se règle ensuite dans les propriétés");
                        self.valeurs.insert((surface, indice), Valeur::Texte(v));
                        ui.end_row();
                    }
                }
            });
    }

    /// Un outil du menu `menu` est-il actif ? (pendant un tracé de Modéliser, l'onglet
    /// Modifier reste ouvert)
    pub fn outil_actif_dans(&self, menu: Menu) -> Option<Outil> {
        let outil = self.selection?;
        let dans = |m: Menu| groupes(m).iter().any(|(_, o)| o.contains(&outil));
        (dans(menu) || (menu == Menu::Modifier && dans(Menu::Modeliser))).then_some(outil)
    }

    /// Emplacement « paramètres » commun : un titre, puis une ligne par champ de l'outil
    /// (`champs`). Même présentation pour tous les objets.
    pub fn section_parametres(&mut self, ui: &mut egui::Ui, outil: Outil, projet: Option<&modele::Projet>) {
        if champs(outil).is_empty() {
            return;
        }
        ui.add_space(6.0);
        ui.label(RichText::new(format!("Paramètres — {}", outil.nom())).strong());
        self.options_outil(ui, outil, projet);
        ui.add_space(4.0);
        ui.separator();
    }

    fn options_outil(&mut self, ui: &mut egui::Ui, outil: Outil, projet: Option<&modele::Projet>) {
        egui::Grid::new(("parametres_outil", outil)).num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| self.champs_outil(ui, outil, projet, true));
    }

    /// Champs de l'outil dans le bandeau de dessin : les mêmes que dans Propriétés, sur une
    /// ligne. Faux si l'outil n'en a pas.
    pub fn options_bandeau(&mut self, ui: &mut egui::Ui, outil: Outil, projet: Option<&modele::Projet>) -> bool {
        if champs(outil).is_empty() {
            return false;
        }
        self.champs_outil(ui, outil, projet, false);
        true
    }

    /// Libellé et réglage de chaque champ (grille de Propriétés ou ligne du bandeau :
    /// `end_row` n'a d'effet que dans une grille).
    fn champs_outil(&mut self, ui: &mut egui::Ui, outil: Outil, projet: Option<&modele::Projet>, grille: bool) {
        let fenetre = matches!(self.valeurs.get(&(Outil::Ouverture, 0)), Some(Valeur::Texte(t)) if t == "Fenêtre" || t == "Libre");
        let garde_corps_type = outil == Outil::GardeCorps && self.type_garde_corps.as_ref()
            .is_some_and(|id| projet.is_some_and(|p| p.types_garde_corps.iter().any(|t| &t.id == id)));
        let champs: Vec<_> = champs(outil)
            .into_iter()
            .enumerate()
            .filter(|(k, _)| outil != Outil::Ouverture || (*k != 0 && (*k != 3 || fenetre)))
            .filter(|(k, _)| !garde_corps_type || *k == 1)
            // la hauteur de marche se règle dans le schéma de l'escalier
            .filter(|(k, _)| outil != Outil::Escalier || *k != 1)
            .collect();
        let mut changement_ouverture = false;
        {
            for (k, champ) in &champs {
                match champ {
                    Champ::Choix(nom, choix) => {
                        ui.label(*nom);
                        let v = self.valeurs.entry((outil, *k)).or_insert_with(|| Valeur::Texte(choix[0].into()));
                        if let Valeur::Texte(v) = v {
                            egui::ComboBox::from_id_salt(("choix", outil, *k)).width(130.0).selected_text(&*v).show_ui(ui, |ui| {
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
                        let v = self.valeurs.entry((outil, *k)).or_insert(Valeur::Nombre(*defaut));
                        if let Valeur::Nombre(v) = v {
                            let pas = if *suffixe == " mm" { 10.0 } else { 0.1 };
                            let champ = egui::DragValue::new(v).speed(pas);
                            ui.add(if *suffixe == " mm" && !nom.contains("papier") { champ.dimensions() } else { champ.max_decimals(2).suffix(*suffixe) });
                        }
                    }
                    Champ::Case(nom) => {
                        if grille { ui.label(""); }
                        let v = self.valeurs.entry((outil, *k)).or_insert_with(|| Valeur::Texte("non".into()));
                        if let Valeur::Texte(v) = v {
                            let mut coche = v == "oui";
                            if ui.checkbox(&mut coche, *nom).changed() { *v = if coche { "oui" } else { "non" }.into(); }
                        }
                    }
                    Champ::Saisie(nom, defaut) => {
                        ui.label(*nom);
                        let v = self.valeurs.entry((outil, *k)).or_insert_with(|| Valeur::Texte((*defaut).into()));
                        if let Valeur::Texte(v) = v {
                            ui.add(egui::TextEdit::singleline(v).desired_width(130.0));
                        }
                    }
                }
                ui.end_row();
            }
        }
        if changement_ouverture { self.ouverture_type_identique = None; }
    }
}
