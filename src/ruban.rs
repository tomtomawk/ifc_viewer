//! Présentation des commandes d'édition. Le contrôleur du viewer consomme les
//! paramètres de Mur ; les autres commandes restent des sélections d'interface.
use eframe::egui::{self, Align, Color32, RichText, Sense};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Modeliser,
    Modifier,
    Annoter,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Outil {
    Niveau,
    Quadrillage,
    Mur,
    Surfaces,
    Dalle,
    Sol,
    Toiture,
    Plafond,
    Poteau,
    Escalier,
    GardeCorps,
    Ouverture,
    Composant,
    Deplacer,
    Copier,
    Tourner,
    Aligner,
    Ajuster,
    Cote,
    Altimetrie,
    Texte,
    Etiquette,
    Ligne,
    Arc,
    Zone,
    Symbole,
    Legende,
}

impl Outil {
    fn nom(self) -> &'static str {
        match self {
            Self::Niveau => "Niveau",
            Self::Quadrillage => "Quadrillage",
            Self::Mur => "Mur",
            Self::Surfaces => "Éléments horizontaux",
            Self::Dalle => "Dalle",
            Self::Sol => "Sol",
            Self::Toiture => "Toiture",
            Self::Plafond => "Plafond",
            Self::Poteau => "Poteau",
            Self::Escalier => "Escalier",
            Self::GardeCorps => "Garde-corps",
            Self::Ouverture => "Ouverture",
            Self::Composant => "Composant",
            Self::Deplacer => "Déplacer",
            Self::Copier => "Copier",
            Self::Tourner => "Tourner",
            Self::Aligner => "Aligner",
            Self::Ajuster => "Ajuster",
            Self::Cote => "Cote",
            Self::Altimetrie => "Altimétrie",
            Self::Texte => "Texte",
            Self::Etiquette => "Étiquette",
            Self::Ligne => "Ligne",
            Self::Arc => "Arc",
            Self::Zone => "Zone remplie",
            Self::Symbole => "Symbole",
            Self::Legende => "Légende",
        }
    }

    /// Fréquence présumée d'usage : seul le premier rang gagne un grand bouton.
    /// L'ordre permet de conserver Mur / Déplacer / Cote en grand le plus longtemps.
    fn priorite(self) -> u8 {
        match self {
            Self::Mur | Self::Deplacer | Self::Cote => 3,
            Self::Ouverture | Self::Copier | Self::Texte => 2,
            Self::Surfaces | Self::Aligner => 1,
            _ => 0,
        }
    }

    fn commande(self) -> Self {
        match self {
            Self::Dalle | Self::Sol | Self::Plafond | Self::Toiture => Self::Surfaces,
            _ => self,
        }
    }
}

const SURFACES: &[(Outil, &str)] = &[
    (Outil::Dalle, "Dalle structurelle porteuse"),
    (Outil::Sol, "Revêtement et finition de sol"),
    (Outil::Plafond, "Plafond ou faux plafond"),
    (Outil::Toiture, "Couverture et toiture"),
];

fn groupes(menu: Menu) -> &'static [(&'static str, &'static [Outil])] {
    use Outil::*;
    match menu {
        Menu::Modeliser => &[
            ("Construire", &[Mur, Surfaces]),
            ("Structure et accès", &[Poteau, Escalier, GardeCorps]),
            ("Placer", &[Ouverture, Composant]),
            ("Références", &[Niveau, Quadrillage]),
        ],
        Menu::Modifier => &[
            ("Transformer", &[Deplacer, Copier, Tourner]),
            ("Ajuster", &[Aligner, Ajuster]),
        ],
        Menu::Annoter => &[
            ("Coter", &[Cote, Altimetrie]),
            ("Documenter", &[Texte, Etiquette]),
            ("Détail", &[Ligne, Arc, Zone]),
            ("Bibliothèque", &[Symbole, Legende]),
        ],
    }
}

const HAUTEUR_OUTILS: f32 = 66.0;
const HAUTEUR_GROUPE: f32 = 86.0;
const ESPACE_COLONNES: f32 = 4.0;
const ESPACE_GROUPES: f32 = 16.0;
const LARGEUR_OPTIONS: f32 = 440.0;
const LARGEUR_REGLAGES: f32 = 34.0;

struct Colonne {
    outils: Vec<Outil>,
    grande: bool,
    largeur: f32,
}

fn largeur_texte(ui: &egui::Ui, texte: &str, taille: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(
            texte.into(),
            egui::FontId::proportional(taille),
            ui.visuals().text_color(),
        )
        .size()
        .x
}

fn colonnes(ui: &egui::Ui, outils: &[Outil], grands: &[Outil]) -> Vec<Colonne> {
    let mut resultat: Vec<Colonne> = Vec::new();
    for &outil in outils {
        if grands.contains(&outil) {
            resultat.push(Colonne {
                outils: vec![outil],
                grande: true,
                largeur: (largeur_texte(ui, outil.nom(), 12.0) + 20.0).max(62.0),
            });
        } else {
            let largeur = largeur_texte(ui, outil.nom(), 12.0) + 34.0;
            if let Some(colonne) = resultat
                .last_mut()
                .filter(|c| !c.grande && c.outils.len() < 3)
            {
                colonne.outils.push(outil);
                colonne.largeur = colonne.largeur.max(largeur);
            } else {
                resultat.push(Colonne {
                    outils: vec![outil],
                    grande: false,
                    largeur,
                });
            }
        }
    }
    resultat
}

fn largeur_groupe(ui: &egui::Ui, nom: &str, colonnes: &[Colonne]) -> f32 {
    (colonnes.iter().map(|c| c.largeur).sum::<f32>()
        + ESPACE_COLONNES * colonnes.len().saturating_sub(1) as f32)
        .max(largeur_texte(ui, nom, 11.0) + 8.0)
}

fn largeur_commandes(ui: &egui::Ui, menu: Menu, grands: &[Outil]) -> f32 {
    groupes(menu)
        .iter()
        .map(|(nom, outils)| largeur_groupe(ui, nom, &colonnes(ui, outils, grands)))
        .sum::<f32>()
        + ESPACE_GROUPES * groupes(menu).len().saturating_sub(1) as f32
}

enum Champ {
    Choix(&'static str, &'static [&'static str]),
    Nombre(&'static str, f64, &'static str),
    Saisie(&'static str, &'static str),
}
enum Valeur {
    Texte(String),
    Nombre(f64),
}

#[derive(Default)]
pub struct RubanEdition {
    selection: Option<Outil>,
    derniere_surface: Option<Outil>,
    valeurs: BTreeMap<(Outil, usize), Valeur>,
    pub niveau_mur: Option<String>,
    pub niveau_haut_mur: Option<String>,
    pub decalage_haut_mur: f64,
    pub phase_creation: Option<String>,
    pub sous_projet_creation: Option<String>,
    pub variante_creation: Option<String>,
    pub chainer_murs: bool,
    ajout_creation: Option<crate::modele::edition::ListeCreation>,
    nom_creation: String,
    jeu_creation: String,
    erreur_creation: Option<String>,
}

fn champs(o: Outil) -> Vec<Champ> {
    use Champ::*;
    use Outil::*;
    match o {
        Surfaces => vec![],
        Niveau => vec![Saisie("Nom", "Niveau 1"), Nombre("Altitude", 3000.0, " mm")],
        Quadrillage => vec![Saisie("Repère", "A"), Choix("Tracé", &["Droit", "Arc"])],
        Mur => vec![
            Choix(
                "Implantation",
                &["Axe", "Face extérieure", "Face intérieure"],
            ),
            Nombre("Hauteur", 2800.0, " mm"),
            Nombre("Épaisseur", 300.0, " mm"),
            Nombre("Décalage", 0.0, " mm"),
        ],
        Dalle => vec![
            Nombre("Épaisseur", 200.0, " mm"),
            Nombre("Décalage", 0.0, " mm"),
        ],
        Sol => vec![
            Nombre("Épaisseur", 15.0, " mm"),
            Nombre("Décalage", 0.0, " mm"),
        ],
        Toiture => vec![
            Nombre("Décalage", 2800.0, " mm"),
            Nombre("Pente", 35.0, " °"),
        ],
        Plafond => vec![
            Nombre("Hauteur", 2500.0, " mm"),
            Nombre("Épaisseur", 25.0, " mm"),
        ],
        Poteau => vec![
            Choix("Section", &["Rectangulaire", "Circulaire"]),
            Nombre("Largeur / diamètre", 300.0, " mm"),
            Nombre("Profondeur", 300.0, " mm"),
            Nombre("Hauteur", 2800.0, " mm"),
        ],
        Escalier => vec![
            Choix("Forme", &["Droit", "Quart tournant", "Demi tournant"]),
            Nombre("Largeur", 1000.0, " mm"),
            Nombre("Hauteur à franchir", 2800.0, " mm"),
            Nombre("Marches", 16.0, ""),
        ],
        GardeCorps => vec![
            Nombre("Hauteur", 1000.0, " mm"),
            Nombre("Décalage", 0.0, " mm"),
        ],
        Ouverture => vec![
            Choix("Ouverture", &["Porte", "Fenêtre"]),
            Nombre("Largeur", 900.0, " mm"),
            Nombre("Hauteur", 2100.0, " mm"),
            Nombre("Allège", 900.0, " mm"),
        ],
        Composant => vec![
            Choix(
                "Composant",
                &["Mobilier", "Équipement", "Élément générique"],
            ),
            Nombre("Rotation", 0.0, " °"),
        ],
        Deplacer => vec![
            Nombre("Distance", 1000.0, " mm"),
            Choix("Contrainte", &["Libre", "Horizontale", "Verticale"]),
        ],
        Copier => vec![Nombre("Copies", 1.0, ""), Nombre("Distance", 1000.0, " mm")],
        Tourner => vec![Nombre("Angle", 90.0, " °")],
        Aligner => vec![Choix("Référence", &["Face", "Axe"])],
        Ajuster => vec![Choix("Mode", &["Raccorder", "Prolonger"])],
        Cote => vec![
            Choix("Mode", &["Alignée", "Orthogonale"]),
            Choix("Unité", &["Millimètres", "Centimètres", "Mètres"]),
            Nombre("Décalage papier", 8.0, " mm"),
        ],
        Altimetrie => vec![
            Choix("Référence", &["Projet", "Niveau courant"]),
            Choix("Précision", &["0.00 m", "0.000 m"]),
        ],
        Texte => vec![
            Choix("Ligne de repère", &["Sans flèche", "Avec flèche"]),
            Nombre("Hauteur papier", 2.5, " mm"),
            Choix("Alignement", &["Gauche", "Centre", "Droite"]),
        ],
        Etiquette => vec![
            Choix("Contenu", &["Type", "Repère", "Matériau"]),
            Nombre("Hauteur papier", 2.5, " mm"),
        ],
        Ligne | Arc => vec![
            Choix("Style", &["Trait fin", "Trait fort", "Pointillé"]),
            Nombre("Épaisseur papier", 0.18, " mm"),
        ],
        Zone => vec![
            Choix("Motif", &["Diagonales 45°", "Béton", "Isolant", "Plein"]),
            Nombre("Pas papier", 2.0, " mm"),
        ],
        Symbole => vec![Choix(
            "Symbole",
            &["Nord", "Repère de coupe", "Repère de détail"],
        )],
        Legende => vec![Choix(
            "Contenu",
            &["Matériaux", "Types de murs", "Symboles"],
        )],
    }
}

impl RubanEdition {
    pub fn mur_actif(&self) -> bool {
        self.selection == Some(Outil::Mur)
    }
    pub fn a_un_outil(&self) -> bool {
        self.selection.is_some()
    }
    pub fn activer_mur(&mut self) {
        self.selection = Some(Outil::Mur);
    }
    pub fn annuler(&mut self) {
        self.selection = None;
    }
    pub fn parametres_mur(&self) -> crate::modele::edition::ParametresMur {
        use crate::modele::edition::{Implantation, ParametresMur};
        let nombre = |k, defaut| match self.valeurs.get(&(Outil::Mur, k)) {
            Some(Valeur::Nombre(v)) => *v,
            _ => defaut,
        };
        let implantation = match self.valeurs.get(&(Outil::Mur, 0)) {
            Some(Valeur::Texte(v)) if v == "Face extérieure" => Implantation::Exterieure,
            Some(Valeur::Texte(v)) if v == "Face intérieure" => Implantation::Interieure,
            _ => Implantation::Axe,
        };
        ParametresMur {
            hauteur: nombre(1, 2800.0),
            epaisseur: nombre(2, 300.0),
            implantation,
            niveau: self.niveau_mur.clone(),
            decalage: nombre(3, 0.0),
            niveau_haut: self.niveau_haut_mur.clone(),
            decalage_haut: self.decalage_haut_mur,
            phase: self.phase_creation.clone(),
            sous_projet: self.sous_projet_creation.clone(),
            variante: self.variante_creation.clone(),
        }
    }

    pub fn afficher(
        &mut self,
        ui: &mut egui::Ui,
        menu: Menu,
        projet: Option<&crate::modele::Projet>,
    ) {
        let largeur = ui.available_width();
        let outil_options = self.selection.filter(|o| {
            groupes(menu)
                .iter()
                .any(|(_, outils)| outils.contains(&o.commande()))
        });
        // Les surfaces se règlent dans l'encart en tête de Propriétés.
        let outil_options = outil_options.filter(|o| o.commande() != Outil::Surfaces);
        let selection_options = outil_options.map(Outil::commande);
        let compact = largeur_commandes(ui, menu, &[]);
        let options_inline =
            selection_options.is_some() && compact + LARGEUR_OPTIONS + ESPACE_COLONNES <= largeur;
        let largeur_options = if selection_options.is_none() {
            0.0
        } else if options_inline {
            LARGEUR_OPTIONS + ESPACE_COLONNES
        } else {
            LARGEUR_REGLAGES + ESPACE_COLONNES
        };
        let budget = largeur - largeur_options;
        let mut grands = Vec::new();
        // Les références et les commandes occasionnelles restent toujours empilées.
        // En réduisant la fenêtre, les outils les moins fréquents se replient d'abord.
        let mut candidats: Vec<_> = groupes(menu)
            .iter()
            .flat_map(|(_, outils)| outils.iter().copied())
            .filter(|o| o.priorite() > 0 && largeur >= 1500.0 - 200.0 * o.priorite() as f32)
            .collect();
        candidats.sort_by_key(|o| std::cmp::Reverse(o.priorite()));
        for outil in candidats {
            let mut essai = grands.clone();
            essai.push(outil);
            if largeur_commandes(ui, menu, &essai) <= budget {
                grands = essai;
            }
        }
        // Défilement de secours pour les très petites surfaces / grandes polices.
        // À la taille minimale de l'application, toutes les commandes restent visibles.
        egui::ScrollArea::horizontal()
            .id_salt("ruban_commandes")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = ESPACE_GROUPES;
                ui.horizontal_top(|ui| {
                    for (indice, (nom, outils)) in groupes(menu).iter().enumerate() {
                        let colonnes = colonnes(ui, outils, &grands);
                        let contient_selection =
                            selection_options.is_some_and(|o| outils.contains(&o));
                        let largeur_groupe = largeur_groupe(ui, nom, &colonnes)
                            + if contient_selection {
                                largeur_options
                            } else {
                                0.0
                            };
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(largeur_groupe, HAUTEUR_GROUPE),
                            Sense::hover(),
                        );
                        if indice > 0 {
                            let x = rect.left() - ESPACE_GROUPES / 2.0;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(x, rect.top() + 4.0),
                                    egui::pos2(x, rect.bottom() - 4.0),
                                ],
                                ui.visuals().widgets.noninteractive.bg_stroke,
                            );
                        }
                        let mut x = rect.left();
                        for colonne in colonnes {
                            for (ligne, &outil) in colonne.outils.iter().enumerate() {
                                let hauteur = if colonne.grande {
                                    HAUTEUR_OUTILS
                                } else {
                                    HAUTEUR_OUTILS / 3.0
                                };
                                let bouton = egui::Rect::from_min_size(
                                    egui::pos2(x, rect.top() + ligne as f32 * hauteur),
                                    egui::vec2(colonne.largeur, hauteur),
                                );
                                if bouton_outil(
                                    ui,
                                    bouton,
                                    outil,
                                    colonne.grande,
                                    self.selection.map(Outil::commande) == Some(outil),
                                )
                                .clicked()
                                {
                                    self.selection = Some(if outil == Outil::Surfaces {
                                        let surface = self.derniere_surface.unwrap_or(Outil::Dalle);
                                        self.derniere_surface = Some(surface);
                                        surface
                                    } else {
                                        outil
                                    });
                                }
                            }
                            x += colonne.largeur + ESPACE_COLONNES;
                            // Réglages directement à droite de la colonne de l'outil actif.
                            if colonne.outils.iter().any(|o| Some(*o) == selection_options) {
                                let outil = outil_options.unwrap();
                                let reglages = egui::Rect::from_min_size(
                                    egui::pos2(x, rect.top()),
                                    egui::vec2(
                                        if options_inline {
                                            LARGEUR_OPTIONS
                                        } else {
                                            LARGEUR_REGLAGES
                                        },
                                        HAUTEUR_OUTILS,
                                    ),
                                );
                                let mut options_ui = ui.new_child(
                                    egui::UiBuilder::new()
                                        .id_salt(("reglages", outil.commande()))
                                        .max_rect(reglages)
                                        .layout(egui::Layout::top_down(Align::Min)),
                                );
                                {
                                    let ui = &mut options_ui;
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    if options_inline {
                                        self.options_outil(ui, outil, projet);
                                    } else {
                                        let r = ui
                                            .add_sized(
                                                [LARGEUR_REGLAGES, HAUTEUR_OUTILS],
                                                egui::Button::new("..."),
                                            )
                                            .on_hover_text(format!("Réglages — {}", outil.nom()));
                                        egui::Popup::menu(&r)
                                            .close_behavior(
                                                egui::PopupCloseBehavior::CloseOnClickOutside,
                                            )
                                            .show(|ui| {
                                                ui.set_width(LARGEUR_OPTIONS);
                                                ui.label(
                                                    RichText::new(format!(
                                                        "Réglages — {}",
                                                        outil.nom()
                                                    ))
                                                    .strong(),
                                                );
                                                ui.separator();
                                                self.options_outil(ui, outil, projet);
                                            });
                                    }
                                }
                                x += largeur_options;
                            }
                        }
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.bottom() - 8.0),
                            egui::Align2::CENTER_CENTER,
                            *nom,
                            egui::FontId::proportional(11.0),
                            ui.visuals().weak_text_color(),
                        );
                    }
                });
            });
    }

    fn proposer_ajout_creation(
        &mut self,
        ui: &mut egui::Ui,
        liste: crate::modele::edition::ListeCreation,
    ) {
        ui.separator();
        if ui
            .button(format!("+ Ajouter : {}…", liste.libelle()))
            .clicked()
        {
            self.ajout_creation = Some(liste);
            self.nom_creation.clear();
            self.jeu_creation.clear();
            self.erreur_creation = None;
            ui.close();
        }
    }

    pub fn reinitialiser_contexte_creation(&mut self) {
        self.phase_creation = None;
        self.sous_projet_creation = None;
        self.variante_creation = None;
        self.ajout_creation = None;
        self.erreur_creation = None;
    }

    /// Contexte commun aux créations, affiché indépendamment de l'outil actif.
    /// Seuls les ajouts modifient le document ; choisir une valeur règle les prochaines créations.
    pub fn contexte_creation(
        &mut self,
        ui: &mut egui::Ui,
        projet: &mut crate::modele::Projet,
    ) -> bool {
        use crate::modele::edition::ListeCreation;
        let phases = if projet.phases.is_empty() {
            vec!["Projet".into()]
        } else {
            projet.phases.clone()
        };
        let sous_projets = if projet.sous_projets.is_empty() {
            vec!["Sous-projet 1".into()]
        } else {
            projet.sous_projets.clone()
        };
        if self
            .phase_creation
            .as_ref()
            .is_none_or(|v| !phases.contains(v))
        {
            self.phase_creation = phases.last().cloned();
        }
        if self
            .sous_projet_creation
            .as_ref()
            .is_none_or(|v| !sous_projets.contains(v))
        {
            self.sous_projet_creation = sous_projets.first().cloned();
        }
        if self
            .variante_creation
            .as_ref()
            .is_some_and(|id| !projet.variantes.iter().any(|v| &v.id == id))
        {
            self.variante_creation = None;
        }
        let largeur = ((ui.available_width() - 8.0) / 3.0).max(30.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.vertical(|ui| {
                ui.set_width(largeur);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(RichText::new("Phase").small()).truncate());
                egui::ComboBox::from_id_salt("phase_creation")
                    .width(largeur)
                    .truncate()
                    .selected_text(self.phase_creation.as_deref().unwrap_or("Projet"))
                    .show_ui(ui, |ui| {
                        for nom in &phases {
                            ui.selectable_value(&mut self.phase_creation, Some(nom.clone()), nom);
                        }
                        self.proposer_ajout_creation(ui, ListeCreation::Phase);
                    })
                    .response
                    .on_hover_text(format!(
                        "Phase des prochaines créations : {}",
                        self.phase_creation.as_deref().unwrap_or("Projet")
                    ));
            });
            ui.vertical(|ui| {
                ui.set_width(largeur);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(RichText::new("Sous-projet").small()).truncate());
                egui::ComboBox::from_id_salt("sous_projet_creation")
                    .width(largeur)
                    .truncate()
                    .selected_text(
                        self.sous_projet_creation
                            .as_deref()
                            .unwrap_or("Sous-projet 1"),
                    )
                    .show_ui(ui, |ui| {
                        for nom in &sous_projets {
                            ui.selectable_value(
                                &mut self.sous_projet_creation,
                                Some(nom.clone()),
                                nom,
                            );
                        }
                        self.proposer_ajout_creation(ui, ListeCreation::SousProjet);
                    })
                    .response
                    .on_hover_text(format!(
                        "Sous-projet des prochaines créations : {}",
                        self.sous_projet_creation
                            .as_deref()
                            .unwrap_or("Sous-projet 1")
                    ));
            });
            let nom = self
                .variante_creation
                .as_ref()
                .and_then(|id| projet.variantes.iter().find(|v| &v.id == id))
                .map_or_else(|| "Modèle principal".into(), |v| v.libelle());
            ui.vertical(|ui| {
                ui.set_width(largeur);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(RichText::new("Variante").small()).truncate());
                egui::ComboBox::from_id_salt("variante_creation")
                    .width(largeur)
                    .truncate()
                    .selected_text(&nom)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.variante_creation, None, "Modèle principal");
                        for v in &projet.variantes {
                            ui.selectable_value(
                                &mut self.variante_creation,
                                Some(v.id.clone()),
                                v.libelle(),
                            );
                        }
                        self.proposer_ajout_creation(ui, ListeCreation::Variante);
                    })
                    .response
                    .on_hover_text(format!("Variante des prochaines créations : {nom}"));
            });
        });
        let mut modifie = false;
        if let Some(liste) = self.ajout_creation {
            let mut ouvert = true;
            let mut valider = false;
            let mut annuler = false;
            egui::Window::new(format!("Ajouter : {}", liste.libelle()))
                .id(egui::Id::unique("ajout_contexte_creation"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .default_width(280.0)
                .open(&mut ouvert)
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    egui::Grid::new("grille_ajout_creation").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("Nom");
                        ui.add(egui::TextEdit::singleline(&mut self.nom_creation).desired_width(180.0).hint_text(liste.libelle()));
                        ui.end_row();
                        if liste == ListeCreation::Variante {
                            ui.label("Jeu");
                            ui.add(egui::TextEdit::singleline(&mut self.jeu_creation).desired_width(180.0).hint_text("Jeu de variantes"));
                            ui.end_row();
                        }
                    });
                    if let Some(erreur) = &self.erreur_creation {
                        ui.colored_label(ui.visuals().error_fg_color, erreur);
                    }
                    ui.add_space(4.0);
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), egui::Layout::right_to_left(Align::Center), |ui| {
                        annuler = ui.button("Annuler").clicked();
                        valider = ui.add_enabled(!self.nom_creation.trim().is_empty(), egui::Button::new(RichText::new("Ajouter").strong())).clicked();
                    });
                    // touches consommées : Échap ne doit pas aussi quitter l'outil en cours
                    let (entree, echap) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::Enter), i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)));
                    valider |= entree && !self.nom_creation.trim().is_empty();
                    annuler |= echap;
                });
            if valider {
                match projet.ajouter_choix_creation(liste, &self.nom_creation, &self.jeu_creation) {
                    Ok(valeur) => {
                        match liste {
                            ListeCreation::Phase => self.phase_creation = Some(valeur),
                            ListeCreation::SousProjet => self.sous_projet_creation = Some(valeur),
                            ListeCreation::Variante => self.variante_creation = Some(valeur),
                        }
                        modifie = true;
                        self.ajout_creation = None;
                    }
                    Err(e) => self.erreur_creation = Some(e),
                }
            }
            if !ouvert || annuler {
                self.ajout_creation = None;
            }
        }
        modifie
    }

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
        ui.label(RichText::new("Créer un mur").strong());
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
        egui::ScrollArea::vertical()
            .id_salt("creation_mur")
            .show(ui, |ui| {
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
                        self.nombre_mur(ui, 2, 300.0);
                        ui.end_row();
                        ui.label("Implantation");
                        let v = self
                            .valeurs
                            .entry((Outil::Mur, 0))
                            .or_insert_with(|| Valeur::Texte("Axe".into()));
                        if let Valeur::Texte(v) = v {
                            egui::ComboBox::from_id_salt("implantation_proprietes_mur")
                                .width(130.0)
                                .selected_text(v.as_str())
                                .show_ui(ui, |ui| {
                                    for nom in ["Axe", "Face extérieure", "Face intérieure"] {
                                        ui.selectable_value(v, nom.into(), nom);
                                    }
                                });
                        }
                        ui.end_row();
                        ui.label("Chaîner");
                        ui.checkbox(&mut self.chainer_murs, "")
                            .on_hover_text("Le mur suivant part de la fin du précédent");
                        ui.end_row();
                    });
            });
        true
    }

    /// Croquis de saisie : mur en perspective à gauche, ses deux limites à droite, sur
    /// une ligne chacune (niveau, puis décalage, ou hauteur si le haut est libre),
    /// rappelées par un pointillé depuis l'arête correspondante du mur.
    fn croquis_mur_creation(&mut self, ui: &mut egui::Ui, projet: &crate::modele::Projet) {
        use egui::{Align2, FontId, Stroke, pos2, vec2};
        let bleu = Color32::from_rgb(18, 158, 205);
        let largeur_croquis = 64.0;
        let origine = ui.cursor().min;
        let mut centres = [0.0f32; 2];
        let mut gauche_champs = 0.0f32;
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
                                    egui::DragValue::new(v).speed(10.0).range(1.0..=f64::MAX).suffix(" mm"),
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
                                    .custom_formatter(|x, _| format!("{x:+.0} mm"))
                                    .custom_parser(|t| t.trim().trim_end_matches("mm").trim().trim_start_matches('+').parse().ok()),
                            )
                            .on_hover_text("Décalage par rapport au niveau");
                        }
                    });
                    gauche_champs = ligne.response.rect.left();
                    centres[if haut { 0 } else { 1 }] = ligne.response.rect.center().y;
                }
            });
        });
        let p = ui.painter();
        let bord = Stroke::new(1.5, bleu);
        let profondeur = vec2(10.0, -7.0);
        // mur vu de face (largeur), son chant à droite et son dessus
        let x1 = origine.x + largeur_croquis - 22.0;
        let x0 = origine.x + 10.0;
        let (yh, yb) = (centres[0] + 4.0, centres[1] + 4.0);
        let (a, h, b, bb) = (pos2(x0, yh), pos2(x1, yh), pos2(x1, yb), pos2(x0, yb));
        p.add(egui::Shape::convex_polygon(vec![a, h, b, bb], Color32::from_rgb(244, 250, 252), bord));
        p.add(egui::Shape::convex_polygon(
            vec![h, h + profondeur, b + profondeur, b],
            Color32::from_rgb(214, 239, 247),
            bord,
        ));
        p.add(egui::Shape::convex_polygon(
            vec![a, a + profondeur, h + profondeur, h],
            Color32::from_rgb(233, 247, 252),
            bord,
        ));
        // rappels : de l'arête du mur à la ligne de sa limite
        for (depuis, y) in [(h + profondeur, centres[0]), (b + profondeur, centres[1])] {
            let fin = gauche_champs - 3.0;
            let mut x = depuis.x + 3.0;
            while x < fin {
                p.line_segment([pos2(x, y), pos2((x + 3.0).min(fin), y)], Stroke::new(1.0, bleu));
                x += 6.0;
            }
            p.line_segment([depuis, pos2(depuis.x + 2.0, y)], Stroke::new(1.0, bleu));
            p.circle_filled(depuis, 2.0, bleu);
        }
        // hauteur résultante, le long du mur
        if let Ok((bas_z, haut_z)) = self.parametres_mur().bornes(projet) {
            p.text(
                pos2((x0 + x1) * 0.5, (yh + yb) * 0.5),
                Align2::CENTER_CENTER,
                format!("{:.0}", haut_z - bas_z),
                FontId::proportional(11.0),
                ui.visuals().weak_text_color(),
            );
        }
    }

    fn nombre_mur(&mut self, ui: &mut egui::Ui, cle: usize, defaut: f64) {
        let v = self
            .valeurs
            .entry((Outil::Mur, cle))
            .or_insert(Valeur::Nombre(defaut));
        if let Valeur::Nombre(v) = v {
            ui.add_sized([130.0, 20.0], egui::DragValue::new(v).speed(10.0).range(1.0..=f64::MAX).suffix(" mm"));
        }
    }

    /// Sélecteur de création placé en tête de la palette Propriétés, comme un
    /// sélecteur de type : vignette, choix explicite, puis paramètres de l'outil.
    /// Retourne false pour laisser la palette afficher la sélection du modèle.
    pub fn proprietes_surfaces(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(mut surface) = self.selection.filter(|o| o.commande() == Outil::Surfaces) else {
            return false;
        };
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
                        egui::ComboBox::from_id_salt("surface_proprietes")
                            .width(ui.available_width())
                            .height(240.0)
                            .selected_text(RichText::new(surface.nom()).strong())
                            .show_ui(ui, |ui| {
                                ui.set_min_width(280.0);
                                for &(choix, aide) in SURFACES {
                                    let (r, reponse) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 50.0),
                                        Sense::click(),
                                    );
                                    if surface == choix || reponse.hovered() {
                                        ui.painter().rect_filled(
                                            r,
                                            4.0,
                                            if surface == choix {
                                                ui.visuals().selection.bg_fill
                                            } else {
                                                ui.visuals().widgets.hovered.bg_fill
                                            },
                                        );
                                    }
                                    icone(
                                        ui.painter(),
                                        egui::Rect::from_min_size(
                                            r.min + egui::vec2(6.0, 8.0),
                                            egui::vec2(34.0, 34.0),
                                        ),
                                        choix,
                                        ui.visuals().text_color(),
                                    );
                                    ui.painter().text(
                                        r.min + egui::vec2(48.0, 25.0),
                                        egui::Align2::LEFT_CENTER,
                                        format!("{}\n{aide}", choix.nom()),
                                        egui::FontId::proportional(12.0),
                                        ui.visuals().text_color(),
                                    );
                                    if reponse.clicked() {
                                        surface = choix;
                                        ui.close();
                                    }
                                }
                            });
                        let aide = SURFACES
                            .iter()
                            .find(|(o, _)| *o == surface)
                            .map_or("", |(_, aide)| *aide);
                        ui.add(egui::Label::new(RichText::new(aide).small().weak()).wrap());
                    });
                });
            });
        self.selection = Some(surface);
        self.derniere_surface = Some(surface);
        ui.add_space(8.0);
        ui.label(RichText::new("Paramètres").strong());
        ui.separator();
        egui::Grid::new(("proprietes_surface", surface))
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for (indice, champ) in champs(surface).into_iter().enumerate() {
                    if let Champ::Nombre(nom, defaut, suffixe) = champ {
                        ui.label(nom);
                        let valeur = self
                            .valeurs
                            .entry((surface, indice))
                            .or_insert(Valeur::Nombre(defaut));
                        if let Valeur::Nombre(valeur) = valeur {
                            ui.add_sized(
                                [90.0, 22.0],
                                egui::DragValue::new(valeur).speed(0.1).suffix(suffixe),
                            );
                        }
                        ui.end_row();
                    }
                }
            });
        true
    }

    fn options_outil(
        &mut self,
        ui: &mut egui::Ui,
        outil: Outil,
        projet: Option<&crate::modele::Projet>,
    ) {
        let fenetre = matches!(self.valeurs.get(&(Outil::Ouverture, 0)), Some(Valeur::Texte(t)) if t == "Fenêtre");
        let champs: Vec<_> = champs(outil)
            .into_iter()
            .enumerate()
            .filter(|(k, _)| outil != Outil::Ouverture || *k != 3 || fenetre)
            // mur : décalage bas dans Propriétés ; hauteur seulement si le haut est libre
            .filter(|(k, _)| outil != Outil::Mur || (*k != 3 && (*k != 1 || self.niveau_haut_mur.is_none())))
            .collect();
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
                                                ui.selectable_value(v, (*choix).into(), *choix);
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
                                    ui.add(egui::DragValue::new(v).speed(pas).max_decimals(if *suffixe == " mm" { 0 } else { 2 }).suffix(*suffixe));
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
        if outil == Outil::Mur {
            self.options_mur(ui, projet);
        }
    }

    fn options_mur(&mut self, ui: &mut egui::Ui, projet: Option<&crate::modele::Projet>) {
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
            ui.checkbox(&mut self.chainer_murs, "Chaîner");
        });
    }
}

fn bouton_outil(
    ui: &mut egui::Ui,
    r: egui::Rect,
    outil: Outil,
    grand: bool,
    actif: bool,
) -> egui::Response {
    let resp = ui.interact(r, ui.scope_id().with(("outil", outil)), Sense::click());
    if actif || resp.hovered() {
        ui.painter().rect_filled(
            r.shrink(1.0),
            4.0,
            if actif {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().widgets.hovered.weak_bg_fill
            },
        );
    }
    let couleur = if actif {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().text_color()
    };
    let (centre, taille, texte, ancrage) = if grand {
        (
            egui::pos2(r.center().x, r.top() + 24.0),
            32.0,
            egui::pos2(r.center().x, r.bottom() - 12.0),
            egui::Align2::CENTER_CENTER,
        )
    } else {
        (
            egui::pos2(r.left() + 12.0, r.center().y),
            17.0,
            egui::pos2(r.left() + 27.0, r.center().y),
            egui::Align2::LEFT_CENTER,
        )
    };
    icone(
        ui.painter(),
        egui::Rect::from_center_size(centre, egui::vec2(taille, taille)),
        outil,
        couleur,
    );
    ui.painter().text(
        texte,
        ancrage,
        outil.nom(),
        egui::FontId::proportional(12.0),
        couleur,
    );
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), actif, outil.nom())
    });
    resp.on_hover_text(outil.nom())
}

fn icone(p: &egui::Painter, r: egui::Rect, o: Outil, c: Color32) {
    use Outil::*;
    let t = egui::Stroke::new(if r.width() < 20.0 { 1.1 } else { 1.4 }, c);
    let pt = |x: f32, y: f32| egui::pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let seg = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], t);
    };
    let cadre = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(
            egui::Rect::from_min_max(pt(x, y), pt(x + w, y + h)),
            0.0,
            t,
            egui::StrokeKind::Inside,
        );
    };
    let face = |points: &[(f32, f32)], opacite: f32| {
        p.add(egui::Shape::convex_polygon(
            points.iter().map(|&(x, y)| pt(x, y)).collect(),
            c.gamma_multiply(opacite),
            t,
        ));
    };
    match o {
        Niveau | Altimetrie => {
            seg((0.05, 0.65), (0.95, 0.65));
            seg((0.3, 0.2), (0.3, 0.65));
            seg((0.18, 0.48), (0.3, 0.65));
            seg((0.42, 0.48), (0.3, 0.65));
            if o == Niveau {
                seg((0.05, 0.88), (0.95, 0.88));
            }
        }
        Quadrillage => {
            for n in [0.25, 0.65] {
                seg((n, 0.1), (n, 0.9));
                seg((0.1, n), (0.9, n));
            }
        }
        Mur => {
            // Face lisse ; les trois couches se lisent uniquement sur la coupe
            // et sur le dessus, sans joints ni dessin de maçonnerie.
            face(
                &[(0.08, 0.28), (0.62, 0.48), (0.62, 0.94), (0.08, 0.74)],
                0.04,
            );
            face(
                &[(0.08, 0.28), (0.38, 0.10), (0.92, 0.30), (0.62, 0.48)],
                0.08,
            );
            face(
                &[(0.62, 0.48), (0.92, 0.30), (0.92, 0.76), (0.62, 0.94)],
                0.16,
            );
            for couche in [0.28, 0.72] {
                let dx = couche * 0.30;
                let dy = couche * -0.18;
                seg((0.08 + dx, 0.28 + dy), (0.62 + dx, 0.48 + dy));
                seg((0.62 + dx, 0.48 + dy), (0.62 + dx, 0.94 + dy));
            }
        }
        Surfaces => {
            // Trois silhouettes espacées : couverture, plafond suspendu et plancher.
            face(
                &[(0.08, 0.22), (0.36, 0.05), (0.86, 0.16), (0.58, 0.32)],
                0.06,
            );
            seg((0.36, 0.05), (0.60, 0.16));
            face(
                &[(0.08, 0.49), (0.49, 0.39), (0.92, 0.50), (0.51, 0.61)],
                0.04,
            );
            seg((0.20, 0.46), (0.20, 0.36));
            seg((0.79, 0.47), (0.79, 0.37));
            face(
                &[(0.08, 0.78), (0.49, 0.68), (0.92, 0.79), (0.51, 0.90)],
                0.10,
            );
            seg((0.08, 0.78), (0.08, 0.86));
            seg((0.08, 0.86), (0.51, 0.98));
            seg((0.51, 0.98), (0.92, 0.87));
            seg((0.92, 0.87), (0.92, 0.79));
            seg((0.51, 0.90), (0.51, 0.98));
        }
        Dalle => {
            // Béton porteur : tranche épaisse, avec hachures dans l'épaisseur.
            face(
                &[(0.06, 0.35), (0.55, 0.15), (0.94, 0.40), (0.45, 0.61)],
                0.04,
            );
            face(
                &[(0.06, 0.35), (0.45, 0.61), (0.45, 0.88), (0.06, 0.62)],
                0.13,
            );
            face(
                &[(0.45, 0.61), (0.94, 0.40), (0.94, 0.67), (0.45, 0.88)],
                0.20,
            );
            for k in [0.0, 0.18, 0.36] {
                seg((0.50 + k, 0.69 - k * 0.42), (0.56 + k, 0.80 - k * 0.42));
            }
        }
        Sol => {
            // Finition mince : carrelage sur le dessus, très faible épaisseur.
            face(
                &[(0.06, 0.44), (0.55, 0.20), (0.94, 0.46), (0.45, 0.70)],
                0.05,
            );
            for k in [0.33, 0.66] {
                seg(
                    (0.06 + 0.49 * k, 0.44 - 0.24 * k),
                    (0.45 + 0.49 * k, 0.70 - 0.24 * k),
                );
                seg(
                    (0.06 + 0.39 * k, 0.44 + 0.26 * k),
                    (0.55 + 0.39 * k, 0.20 + 0.26 * k),
                );
            }
            seg((0.06, 0.50), (0.45, 0.76));
            seg((0.45, 0.76), (0.94, 0.52));
            seg((0.06, 0.44), (0.06, 0.50));
            seg((0.94, 0.46), (0.94, 0.52));
        }
        Toiture => {
            // Deux pans bien distincts autour d'un faîtage, avec débords.
            face(
                &[(0.04, 0.62), (0.32, 0.20), (0.75, 0.08), (0.47, 0.50)],
                0.16,
            );
            face(
                &[(0.32, 0.20), (0.68, 0.68), (0.97, 0.54), (0.75, 0.08)],
                0.04,
            );
            seg((0.32, 0.20), (0.75, 0.08));
            seg((0.04, 0.69), (0.68, 0.80));
            seg((0.68, 0.80), (0.97, 0.65));
            seg((0.68, 0.68), (0.68, 0.80));
            seg((0.19, 0.59), (0.47, 0.17));
        }
        Plafond => {
            // Panneau haut, suspendu à trois tiges ; lecture par le dessous.
            for (x, y) in [(0.13, 0.50), (0.55, 0.30), (0.89, 0.53)] {
                seg((x, y), (x, y - 0.24));
                seg((x - 0.05, y - 0.24), (x + 0.05, y - 0.24));
            }
            face(
                &[(0.06, 0.55), (0.55, 0.34), (0.94, 0.59), (0.45, 0.81)],
                0.13,
            );
            seg((0.06, 0.61), (0.45, 0.87));
            seg((0.45, 0.87), (0.94, 0.65));
            seg((0.25, 0.68), (0.74, 0.47));
        }
        Poteau => {
            face(
                &[(0.33, 0.22), (0.57, 0.12), (0.75, 0.22), (0.51, 0.33)],
                0.09,
            );
            face(
                &[(0.33, 0.22), (0.51, 0.33), (0.51, 0.86), (0.33, 0.75)],
                0.06,
            );
            face(
                &[(0.51, 0.33), (0.75, 0.22), (0.75, 0.75), (0.51, 0.86)],
                0.17,
            );
            seg((0.16, 0.82), (0.49, 0.98));
            seg((0.49, 0.98), (0.91, 0.79));
        }
        Escalier => {
            for k in 0..4 {
                let x = 0.07 + k as f32 * 0.13;
                let y = 0.84 - k as f32 * 0.17;
                seg((x, y), (x, y - 0.16));
                seg((x, y - 0.16), (x + 0.13, y - 0.16));
                seg((x, y - 0.16), (x + 0.35, y - 0.31));
                seg((x + 0.35, y - 0.31), (x + 0.48, y - 0.31));
            }
            seg((0.07, 0.84), (0.55, 0.84));
            seg((0.55, 0.84), (0.94, 0.18));
        }
        GardeCorps => {
            // Main courante en perspective, avec retour et platines distinctes.
            seg((0.08, 0.36), (0.73, 0.12));
            seg((0.73, 0.12), (0.94, 0.25));
            seg((0.08, 0.46), (0.73, 0.22));
            seg((0.73, 0.22), (0.94, 0.35));
            seg((0.10, 0.66), (0.73, 0.43));
            seg((0.73, 0.43), (0.92, 0.55));
            for (x, y) in [(0.12, 0.36), (0.42, 0.25), (0.73, 0.14), (0.91, 0.27)] {
                seg((x, y), (x, y + 0.52));
                seg((x - 0.06, y + 0.54), (x + 0.06, y + 0.50));
            }
        }
        Ouverture => {
            cadre(0.15, 0.08, 0.7, 0.84);
            seg((0.5, 0.08), (0.5, 0.92));
            seg((0.15, 0.5), (0.85, 0.5));
        }
        Composant => {
            for (x, y) in [(0.1, 0.1), (0.55, 0.1), (0.1, 0.55), (0.55, 0.55)] {
                cadre(x, y, 0.32, 0.32);
            }
        }
        Deplacer => {
            for (a, b) in [
                ((0.1, 0.5), (0.9, 0.5)),
                ((0.5, 0.1), (0.5, 0.9)),
                ((0.1, 0.5), (0.27, 0.35)),
                ((0.1, 0.5), (0.27, 0.65)),
                ((0.9, 0.5), (0.73, 0.35)),
                ((0.9, 0.5), (0.73, 0.65)),
                ((0.5, 0.1), (0.35, 0.27)),
                ((0.5, 0.1), (0.65, 0.27)),
                ((0.5, 0.9), (0.35, 0.73)),
                ((0.5, 0.9), (0.65, 0.73)),
            ] {
                seg(a, b);
            }
        }
        Copier => {
            cadre(0.1, 0.1, 0.55, 0.55);
            cadre(0.35, 0.35, 0.55, 0.55);
        }
        Tourner | Arc => {
            let pts = (0..24)
                .map(|k| {
                    let a = 0.2 + k as f32 / 23.0 * 4.4;
                    pt(0.5 + 0.36 * a.cos(), 0.5 + 0.36 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, t));
            if o == Tourner {
                seg((0.85, 0.57), (0.9, 0.28));
                seg((0.85, 0.57), (0.62, 0.52));
            }
        }
        Aligner => {
            seg((0.15, 0.05), (0.15, 0.95));
            cadre(0.25, 0.15, 0.65, 0.23);
            cadre(0.25, 0.6, 0.4, 0.23);
        }
        Ajuster => {
            seg((0.1, 0.8), (0.9, 0.8));
            seg((0.3, 0.15), (0.3, 0.95));
            seg((0.6, 0.2), (0.9, 0.5));
            seg((0.6, 0.5), (0.9, 0.2));
        }
        Cote => {
            seg((0.15, 0.15), (0.15, 0.85));
            seg((0.85, 0.15), (0.85, 0.85));
            seg((0.05, 0.6), (0.95, 0.6));
            seg((0.06, 0.7), (0.24, 0.5));
            seg((0.76, 0.7), (0.94, 0.5));
        }
        Texte => {
            seg((0.12, 0.16), (0.88, 0.16));
            seg((0.5, 0.16), (0.5, 0.88));
            seg((0.32, 0.88), (0.68, 0.88));
        }
        Etiquette => {
            p.add(egui::Shape::closed_line(
                vec![
                    pt(0.08, 0.1),
                    pt(0.6, 0.1),
                    pt(0.93, 0.5),
                    pt(0.5, 0.93),
                    pt(0.08, 0.5),
                ],
                t,
            ));
            p.circle_stroke(pt(0.3, 0.3), 2.0, t);
        }
        Ligne => seg((0.15, 0.85), (0.85, 0.15)),
        Zone => {
            cadre(0.08, 0.08, 0.84, 0.84);
            for k in 0..5 {
                let x = 0.1 + k as f32 * 0.17;
                seg((x, 0.85), (0.85, x));
            }
        }
        Symbole => {
            p.circle_stroke(r.center(), r.width() * 0.38, t);
            seg((0.5, 0.25), (0.5, 0.75));
            seg((0.25, 0.5), (0.75, 0.5));
        }
        Legende => {
            for y in [0.2, 0.5, 0.8] {
                cadre(0.08, y - 0.07, 0.17, 0.14);
                seg((0.4, y), (0.92, y));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                    .show(ui, |ui| ruban.afficher(ui, Menu::Modeliser, None));
                egui::Panel::left("proprietes_test")
                    .exact_size(284.0)
                    .show(ui, |ui| {
                        ui.label("Propriétés");
                        ruban.proprietes_surfaces(ui);
                    });
                egui::CentralPanel::default().show(ui, |_| {});
            },
        );
        sortie.textures_delta.clear();
        sortie
    }

    fn cliquer(
        ctx: &egui::Context,
        ruban: &mut RubanEdition,
        largeur: f32,
        texte: &str,
    ) -> egui::FullOutput {
        afficher(ctx, ruban, largeur, vec![]);
        let sortie = afficher(ctx, ruban, largeur, vec![]);
        let position = sortie
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == texte => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
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
    fn contexte_creation_independant_de_l_outil_et_sur_une_ligne() {
        let ctx = egui::Context::default();
        let mut ruban = RubanEdition::default();
        let mut projet = crate::modele::Projet::nouveau("Contexte");
        projet.phases = vec!["Phase de construction avec un nom très long".into()];
        projet.sous_projets = vec!["Sous-projet de structure avec un nom très long".into()];
        projet.variantes = vec![crate::modele::Variante {
            id: "v".into(),
            nom: "Option très longue".into(),
            jeu: "Façade extérieure".into(),
        }];
        ruban.variante_creation = Some("v".into());
        for outil in [
            None,
            Some(Outil::Mur),
            Some(Outil::Dalle),
            Some(Outil::Texte),
        ] {
            ruban.selection = outil;
            for largeur in [220.0, 260.0, 300.0, 420.0] {
                let mut sortie = None;
                for _ in 0..3 {
                    let mut frame = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(largeur, 520.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            egui::CentralPanel::default().show(ui, |ui| {
                                assert!(!ruban.contexte_creation(ui, &mut projet));
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
                for (texte, rect) in &textes {
                    assert!(
                        rect.right() <= largeur + 0.5,
                        "Débordement à {largeur} px : {texte} {rect:?}"
                    );
                }
                let mut ligne: Option<f32> = None;
                for attendu in ["Phase", "Sous-projet", "Variante"] {
                    let (_, r) = textes
                        .iter()
                        .find(|(t, _)| t == attendu)
                        .unwrap_or_else(|| panic!("Absent : {attendu}, textes : {textes:?}"));
                    assert!(r.right() <= largeur, "{attendu} déborde à {largeur} px");
                    if attendu != "+" {
                        if let Some(y) = ligne {
                            assert!((r.center().y - y).abs() < 2.0);
                        } else {
                            ligne = Some(r.center().y);
                        }
                    }
                }
                assert_eq!(ruban.parametres_mur().phase, projet.phases.last().cloned());
                assert_eq!(
                    ruban.parametres_mur().sous_projet,
                    projet.sous_projets.first().cloned()
                );
                assert_eq!(ruban.parametres_mur().variante.as_deref(), Some("v"));
            }
        }
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
            for attendu in ["Haut", "Bas", "Libre", "Épaisseur", "Implantation", "Chaîner"] {
                let (_, r) = textes
                    .iter()
                    .find(|(t, _)| t == attendu)
                    .unwrap_or_else(|| panic!("Champ absent : {attendu}"));
                assert!(
                    r.right() <= largeur,
                    "{attendu} déborde à {largeur} px : {r:?}"
                );
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
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == "Dalle" => Some(t.pos),
                    _ => None,
                })
                .expect("sélecteur Dalle dans Propriétés");
            assert!(
                encart.x < 284.0 && encart.y > HAUTEUR_GROUPE,
                "le sélecteur doit se trouver dans la palette sous le ruban"
            );
            cliquer(&ctx, &mut ruban, largeur, "Dalle");
            cliquer(
                &ctx,
                &mut ruban,
                largeur,
                "Sol\nRevêtement et finition de sol",
            );
            assert_eq!(ruban.selection, Some(Outil::Sol));
            assert!(matches!(
                ruban.valeurs.get(&(Outil::Sol, 0)),
                Some(Valeur::Nombre(15.0))
            ));
            ruban.valeurs.insert((Outil::Sol, 0), Valeur::Nombre(21.0));
            cliquer(&ctx, &mut ruban, largeur, "Sol");
            cliquer(
                &ctx,
                &mut ruban,
                largeur,
                "Plafond\nPlafond ou faux plafond",
            );
            assert_eq!(ruban.selection, Some(Outil::Plafond));
            cliquer(&ctx, &mut ruban, largeur, "Plafond");
            cliquer(
                &ctx,
                &mut ruban,
                largeur,
                "Dalle\nDalle structurelle porteuse",
            );
            assert!(matches!(
                ruban.valeurs.get(&(Outil::Dalle, 0)),
                Some(Valeur::Nombre(200.0))
            ));
            cliquer(&ctx, &mut ruban, largeur, "Dalle");
            cliquer(
                &ctx,
                &mut ruban,
                largeur,
                "Sol\nRevêtement et finition de sol",
            );
            assert!(matches!(
                ruban.valeurs.get(&(Outil::Sol, 0)),
                Some(Valeur::Nombre(21.0))
            ));
            cliquer(&ctx, &mut ruban, largeur, "Sol");
            let sortie = cliquer(&ctx, &mut ruban, largeur, "Toiture\nCouverture et toiture");
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
                    .show(ui, |ui| ruban.afficher(ui, Menu::Modeliser, None));
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
                            .find(|(texte, _)| texte == outil.nom())
                            .unwrap_or_else(|| panic!("commande absente : {outil:?}"));
                        assert!(
                            rect.left() >= 0.0 && rect.right() <= largeur,
                            "commande hors écran à {largeur}, {selection:?} : {outil:?} {rect:?}"
                        );
                    }
                    let commandes: Vec<_> = textes
                        .iter()
                        .filter(|(texte, _)| outils.iter().any(|o| o.nom() == texte))
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
}
