//! Sélecteur de type et éditeur des garde-corps. L'éditeur suit celui des types de mur :
//! liste des éléments à gauche (poignées pour réordonner, profil, cote, matériau propre
//! ou global), élévation à droite où lisses, main courante et limites du remplissage se
//! glissent à la souris.
use super::*;
use crate::config::options::{Dimensions, unite};
use crate::modele::elements::garde_corps::{BarreauExtremite, BarreauMotif, Lisse, Section};
use crate::modele::{Projet, Remplissage, TypeGardeCorps, identifiant};
use crate::ui::ruban::Outil;
use glam::DVec2;

/// Élément du type désigné dans la liste ou dans l'élévation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Element {
    MainCourante,
    Lisse(usize),
    Depart,
    Fin,
    Motif(usize),
    Remplissage,
    /// Bas des barreaux et du remplissage.
    Bas,
    /// Haut des barreaux et du remplissage (jeu sous la main courante).
    Haut,
}

/// Liste dont une ligne est glissée par sa poignée.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Liste {
    Lisses,
    Motif,
}

pub(super) struct EditeurGardeCorps {
    brouillon: TypeGardeCorps,
    occurrence: Option<String>,
    creation: bool,
    longueur: f64,
    erreur: Option<String>,
    selection: Option<Element>,
    /// Ligne glissée par sa poignée (liste, indice).
    glisse: Option<(Liste, usize)>,
    /// Élément glissé dans l'élévation.
    deplace: Option<Element>,
}

/// Matériaux nommés par le type (global, main courante, remplissage, éléments).
fn materiaux_du_type(t: &TypeGardeCorps) -> Vec<String> {
    let propres = t.lisses.iter().map(|l| &l.materiau)
        .chain(t.motif.iter().map(|b| &b.materiau))
        .chain(t.depart.iter().chain(&t.arrivee).map(|b| &b.materiau));
    let mut v: Vec<String> = [&t.materiau, &t.materiau_main_courante, &t.materiau_remplissage].into_iter().chain(propres).flatten().cloned().collect();
    v.sort();
    v.dedup();
    v
}

/// Ajoute au projet les matériaux du type qui lui manquent (couleur par défaut).
fn completer_materiaux(q: &mut Projet, t: &TypeGardeCorps) {
    for nom in materiaux_du_type(t) {
        if q.materiau(&nom).is_none() {
            q.materiaux.push(crate::modele::murs::assemblage::materiau_par_defaut(&nom));
        }
    }
}

/// Nom libre d'après `base` parmi les types de garde-corps du projet.
fn nom_libre(p: &Projet, base: &str) -> String {
    if p.types_garde_corps.iter().all(|t| t.nom != base) {
        return base.to_owned();
    }
    (2..).map(|n| format!("{base} {n}")).find(|nom| p.types_garde_corps.iter().all(|t| &t.nom != nom)).unwrap()
}

/// Ajoute un type au projet (identifiant et nom libres, matériaux manquants).
fn ajouter_type(p: &Projet, mut t: TypeGardeCorps) -> (Projet, String) {
    let mut q = p.clone();
    t.nom = nom_libre(p, t.nom.trim());
    t.id = (0..).map(|n| identifiant("type-garde-corps", &format!("{}:{n}", t.nom))).find(|id| q.types_garde_corps.iter().all(|x| &x.id != id)).unwrap();
    completer_materiaux(&mut q, &t);
    let id = t.id.clone();
    q.types_garde_corps.push(t);
    (q, id)
}

impl ViewerApp {
    pub(super) fn standard_garde_corps(&self) -> TypeGardeCorps {
        let remplissage = match self
            .ruban_edition
            .texte_option(Outil::GardeCorps, 2)
            .as_str()
        {
            "Panneau plein" => Remplissage::Plein,
            "Vitrage" => Remplissage::Vitre,
            _ => Remplissage::Barreaudage,
        };
        TypeGardeCorps::standard(
            self.ruban_edition.nombre_option(Outil::GardeCorps, 0),
            remplissage,
        )
    }

    /// Sélecteur de type, comme celui des murs : types du projet, puis types
    /// prédéfinis (vitré, barreaudé…) copiés dans le projet au choix ; modifier ou
    /// dupliquer au survol, nouveau type en fin de liste.
    pub(super) fn selecteur_garde_corps(&mut self, ui: &mut egui::Ui, ids: &[String]) -> bool {
        use super::ruban::selecteur;
        let Some((p, _)) = &self.natif else {
            return false;
        };
        let creation = ids.is_empty()
            && self.onglet_ruban == OngletRuban::Modeliser
            && self.ruban_edition.outil() == Some(Outil::GardeCorps);
        let occurrence = match ids {
            [id] => p.garde_corps.iter().find(|g| &g.id == id),
            _ => None,
        };
        if !creation && occurrence.is_none() {
            return false;
        }
        let id_occurrence = occurrence.map(|g| g.id.clone());
        let choisi = if creation {
            self.ruban_edition.type_garde_corps.clone()
        } else {
            occurrence.and_then(|g| g.type_garde_corps.clone())
        };
        let standard = occurrence.map_or_else(
            || self.standard_garde_corps(),
            |g| TypeGardeCorps::standard(g.hauteur, g.remplissage),
        );
        let type_actuel = choisi.as_ref().and_then(|id| p.types_garde_corps.iter().find(|t| &t.id == id));
        let mut types: Vec<&TypeGardeCorps> = p.types_garde_corps.iter().collect();
        types.sort_by(|a, b| a.nom.cmp(&b.nom));
        let predefinis: Vec<TypeGardeCorps> = TypeGardeCorps::predefinis().into_iter().filter(|t| p.types_garde_corps.iter().all(|x| x.nom != t.nom)).collect();
        let detail = |t: &TypeGardeCorps| format!("{} · {}", t.remplissage.nom(), unite().afficher(t.hauteur));
        // (choix : Some(id) type du projet, None standard), prédéfini à copier, modifier, dupliquer, nouveau
        let mut choix: Option<Option<String>> = None;
        let mut copier: Option<TypeGardeCorps> = None;
        let mut modifier: Option<String> = None;
        let mut dupliquer: Option<TypeGardeCorps> = None;
        let mut nouveau = false;
        let titre = format!("Garde-corps · {}", type_actuel.map_or("Standard", |t| t.nom.as_str()));
        let accent = ui.visuals().selection.stroke.color;
        let vignette = |peintre: &egui::Painter, r: egui::Rect, c: Color32| { apercu(peintre, r, p, type_actuel.unwrap_or(&standard), 1600.0, None, c, accent); };
        let bouton = selecteur::entete(ui, &titre, Some(&detail(type_actuel.unwrap_or(&standard))), &format!("{titre}\nType du garde-corps : hauteur, profils, remplissage et matériaux."), Some(&vignette));
        let noms = types.iter().map(|t| t.nom.clone()).chain(predefinis.iter().map(|t| t.nom.clone())).chain(["Standard".to_string()]);
        let largeur = selecteur::largeur_liste(ui, noms, 140.0, 0, 0.0);
        let rangs = types.len() + predefinis.len() + 1;
        selecteur::liste(&bouton, largeur, 40.0 + rangs as f32 * selecteur::HAUTEUR_LIGNE, |ui| {
            let actions_projet: &[(selecteur::ActionType, &str)] = &[(selecteur::ActionType::Dupliquer, "Dupliquer : nouveau type d'après celui-ci"), (selecteur::ActionType::Modifier, "Modifier le type")];
            let actions_gabarit: &[(selecteur::ActionType, &str)] = &[(selecteur::ActionType::Dupliquer, "Dupliquer : nouveau type d'après celui-ci")];
            let ligne = |ui: &mut egui::Ui, t: &TypeGardeCorps, courant: bool, etiquette: Option<&str>, actions| {
                let l = selecteur::Ligne { nom: &t.nom, courant, etiquette, sous_titre: None, detail: Some(detail(t)), actions };
                let mut v = |ui: &mut egui::Ui, r: egui::Rect| { apercu(ui.painter(), r, p, t, 1600.0, None, ui.visuals().text_color(), accent); };
                selecteur::ligne(ui, &l, Some(&mut v))
            };
            let clic = ligne(ui, &TypeGardeCorps { nom: "Standard".into(), ..standard.clone() }, choisi.is_none(), Some("options du ruban"), actions_gabarit);
            if clic.choisi { choix = Some(None); }
            if clic.action.is_some() { dupliquer = Some(standard.clone()); }
            for t in &types {
                let clic = ligne(ui, t, choisi.as_ref() == Some(&t.id), None, actions_projet);
                match clic.action {
                    Some(selecteur::ActionType::Modifier) => modifier = Some(t.id.clone()),
                    Some(selecteur::ActionType::Dupliquer) => dupliquer = Some((*t).clone()),
                    None if clic.choisi => choix = Some(Some(t.id.clone())),
                    None => {}
                }
            }
            for t in &predefinis {
                let clic = ligne(ui, t, false, Some("prédéfini"), actions_gabarit);
                if clic.action.is_some() { dupliquer = Some(t.clone()); }
                if clic.choisi { copier = Some(t.clone()); }
            }
            nouveau = selecteur::ligne_ajout(ui, "Nouveau type de garde-corps…");
        });
        let t = type_actuel.unwrap_or(&standard);
        let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 70.0), Sense::hover());
        ui.painter().rect_filled(r, 4.0, ui.visuals().extreme_bg_color);
        apercu(ui.painter(), r.shrink(6.0), p, t, 1600.0, None, ui.visuals().text_color(), accent);
        // type prédéfini choisi : copié dans le projet, puis appliqué
        if let Some(t) = copier {
            let (q, id) = ajouter_type(p, t);
            match self.appliquer_projet(q) {
                Ok(()) => choix = Some(Some(id)),
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(choix) = choix.filter(|c| *c != choisi) {
            self.appliquer_type_garde_corps(id_occurrence.as_deref(), creation, choix);
        }
        if let Some(id) = modifier {
            self.ouvrir_type_garde_corps(None, Some(&id), false);
        } else if let Some(t) = dupliquer {
            self.ouvrir_brouillon_garde_corps(id_occurrence.as_deref(), t);
        } else if nouveau {
            self.ouvrir_type_garde_corps(id_occurrence.as_deref(), choisi.as_deref(), true);
        }
        true
    }

    /// Type choisi : outil de création, ou garde-corps sélectionné (cotes du type).
    fn appliquer_type_garde_corps(&mut self, occurrence: Option<&str>, creation: bool, choix: Option<String>) {
        if creation {
            self.ruban_edition.type_garde_corps = choix;
            return;
        }
        let (Some(id), Some((p, _))) = (occurrence, &self.natif) else { return };
        let mut q = p.clone();
        let t = choix.as_ref().and_then(|id| q.types_garde_corps.iter().find(|t| &t.id == id)).cloned();
        if let Some(dest) = q.garde_corps.iter_mut().find(|x| x.id == id) {
            dest.type_garde_corps = choix;
            if let Some(t) = t {
                dest.hauteur = t.hauteur;
                dest.remplissage = t.remplissage;
            }
        }
        if let Err(e) = self.appliquer_projet(q) {
            self.error = Some(e);
        }
    }

    pub(super) fn ouvrir_type_garde_corps(
        &mut self,
        occurrence: Option<&str>,
        type_id: Option<&str>,
        nouveau: bool,
    ) {
        let Some((p, _)) = &self.natif else { return };
        let garde = occurrence.and_then(|id| p.garde_corps.iter().find(|g| g.id == id));
        let id = type_id.or_else(|| garde.and_then(|g| g.type_garde_corps.as_deref()));
        let brouillon = id
            .and_then(|id| p.types_garde_corps.iter().find(|t| t.id == id))
            .cloned()
            .unwrap_or_else(|| {
                garde.map_or_else(
                    || self.standard_garde_corps(),
                    |g| TypeGardeCorps::standard(g.hauteur, g.remplissage),
                )
            });
        if nouveau || brouillon.id.is_empty() {
            self.ouvrir_brouillon_garde_corps(occurrence, brouillon);
        } else {
            self.ouvrir_editeur_garde_corps(occurrence, brouillon);
        }
    }

    /// Éditeur sur une copie de `t` à enregistrer comme nouveau type.
    fn ouvrir_brouillon_garde_corps(&mut self, occurrence: Option<&str>, mut t: TypeGardeCorps) {
        let Some((p, _)) = &self.natif else { return };
        t.id.clear();
        t.nom = nom_libre(p, &t.nom);
        self.ouvrir_editeur_garde_corps(occurrence, t);
    }

    fn ouvrir_editeur_garde_corps(&mut self, occurrence: Option<&str>, brouillon: TypeGardeCorps) {
        let Some((p, _)) = &self.natif else { return };
        let garde = occurrence.and_then(|id| p.garde_corps.iter().find(|g| g.id == id));
        let longueur = garde
            .and_then(|g| {
                g.trace
                    .windows(2)
                    .map(|w| DVec2::from_array(w[0]).distance(DVec2::from_array(w[1])))
                    .find(|l| *l > 1e-6)
            })
            .unwrap_or(1600.0);
        self.editeur_garde_corps = Some(EditeurGardeCorps {
            brouillon,
            occurrence: occurrence.map(str::to_owned),
            longueur,
            erreur: None,
            creation: occurrence.is_none() && self.ruban_edition.outil() == Some(Outil::GardeCorps),
            selection: None,
            glisse: None,
            deplace: None,
        });
    }

    pub(super) fn fenetre_garde_corps(&mut self, ctx: &egui::Context) {
        let (Some(mut ed), Some((p, _))) = (self.editeur_garde_corps.take(), &self.natif) else {
            return;
        };
        let (mut ouvert, mut enregistrer, mut annuler) = (true, false, false);
        let mut materiaux: Vec<String> = p.materiaux.iter().map(|m| m.nom.clone()).chain(materiaux_du_type(&ed.brouillon)).chain([crate::modele::elements::garde_corps::MATERIAU_ACIER.to_string(), crate::modele::elements::garde_corps::MATERIAU_VERRE.to_string()]).collect();
        materiaux.sort();
        materiaux.dedup();
        egui::Window::new(if ed.brouillon.id.is_empty() {
            "Nouveau type de garde-corps"
        } else {
            "Modifier le type de garde-corps"
        })
        .id(egui::Id::unique("editeur_garde_corps"))
        .open(&mut ouvert)
        .collapsible(false)
        .resizable(true)
        .default_width(1000.0)
        .default_height(560.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            let t = &mut ed.brouillon;
            ui.horizontal(|ui| {
                ui.label("Nom");
                ui.add(egui::TextEdit::singleline(&mut t.nom).desired_width(220.0));
                ui.label("Remplissage");
                egui::ComboBox::from_id_salt("remplissage")
                    .selected_text(t.remplissage.nom())
                    .show_ui(ui, |ui| {
                        for r in Remplissage::TOUS {
                            ui.selectable_value(&mut t.remplissage, r, r.nom());
                        }
                    });
                ui.label("Hauteur");
                ui.add(cote(&mut t.hauteur).range(201.0..=100_000.0)).on_hover_text("Dessus de la main courante");
                ui.label("Matériau global");
                choix_materiau(ui, "materiau_global", &materiaux, &mut t.materiau, "Par défaut")
                    .on_hover_text("Matériau des éléments qui n'ont pas de matériau propre");
            });
            ui.separator();
            let hauteur = (ui.available_height() - 40.0).max(320.0);
            ui.horizontal_top(|ui| {
                // liste des éléments, à gauche
                ui.allocate_ui_with_layout(egui::vec2(460.0, hauteur), egui::Layout::top_down(egui::Align::Min), |ui| {
                    egui::ScrollArea::vertical().id_salt("elements_garde_corps").max_height(hauteur).show(ui, |ui| {
                        ui.set_width(450.0);
                        elements(ui, p, &materiaux, &mut ed);
                    });
                });
                ui.separator();
                // élévation interactive, à droite
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Élévation").strong());
                        ui.label(RichText::new("glisser lisses, main courante, bas et haut du remplissage ; clic : sélectionner").small().weak());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add(cote(&mut ed.longueur).range(300.0..=100_000.0));
                            ui.label("Longueur d'essai");
                        });
                    });
                    elevation_interactive(ui, p, &mut ed, hauteur - 30.0);
                });
            });
            if let Some(e) = &ed.erreur {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            ui.separator();
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                annuler = ui.button("Annuler").clicked();
                enregistrer = ui.button(RichText::new("Enregistrer le type").strong()).clicked();
            });
        });
        if enregistrer {
            let t = &ed.brouillon;
            let resultat = (|| {
                t.valider(p)?;
                if p.types_garde_corps
                    .iter()
                    .any(|x| x.id != t.id && x.nom.eq_ignore_ascii_case(t.nom.trim()))
                {
                    return Err("Ce nom de type existe déjà.".into());
                }
                let mut type_enregistre = t.clone();
                type_enregistre.nom = t.nom.trim().to_owned();
                let (mut q, id) = if t.id.is_empty() {
                    ajouter_type(p, type_enregistre.clone())
                } else {
                    let mut q = p.clone();
                    completer_materiaux(&mut q, &type_enregistre);
                    let dest = q
                        .types_garde_corps
                        .iter_mut()
                        .find(|x| x.id == t.id)
                        .ok_or("Ce type de garde-corps a été supprimé.")?;
                    *dest = type_enregistre.clone();
                    (q, t.id.clone())
                };
                if let Some(occ) = &ed.occurrence {
                    let garde = q
                        .garde_corps
                        .iter_mut()
                        .find(|g| &g.id == occ)
                        .ok_or("Ce garde-corps a été supprimé.")?;
                    garde.type_garde_corps = Some(id.clone());
                }
                for garde in &mut q.garde_corps {
                    if garde.type_garde_corps.as_ref() == Some(&id) {
                        garde.hauteur = t.hauteur;
                        garde.remplissage = t.remplissage;
                    }
                }
                Ok((q, id))
            })()
            .and_then(|(q, id)| {
                self.appliquer_projet(q)?;
                if ed.creation {
                    self.ruban_edition.type_garde_corps = Some(id);
                }
                Ok(())
            });
            match resultat {
                Ok(()) => return,
                Err(e) => ed.erreur = Some(e),
            }
        }
        if ouvert && !annuler {
            self.editeur_garde_corps = Some(ed);
        }
    }
}

fn cote(valeur: &mut f64) -> egui::DragValue<'_> {
    egui::DragValue::new(valeur).speed(1.0).dimensions()
}

/// Matériau du projet, ou `aucun` (None : matériau global ou par défaut).
fn choix_materiau(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, materiaux: &[String], valeur: &mut Option<String>, aucun: &str) -> egui::Response {
    egui::ComboBox::from_id_salt(salt)
        .width(110.0)
        .truncate()
        .selected_text(valeur.as_deref().unwrap_or(aucun))
        .show_ui(ui, |ui| {
            ui.selectable_value(valeur, None, aucun);
            for m in materiaux {
                ui.selectable_value(valeur, Some(m.clone()), m);
            }
        })
        .response
}

fn profils(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, p: &Projet, id: &mut Option<String>, standard: &str) {
    let nom = id
        .as_ref()
        .and_then(|id| p.profils.iter().find(|t| &t.id == id))
        .map_or(standard, |p| &p.nom);
    egui::ComboBox::from_id_salt(salt)
        .width(100.0)
        .truncate()
        .selected_text(nom)
        .show_ui(ui, |ui| {
            ui.selectable_value(id, None, standard);
            for profil in &p.profils {
                ui.selectable_value(id, Some(profil.id.clone()), &profil.nom);
            }
        });
}

/// Poignée de glissement d'une ligne, comme celle des couches de mur. Retourne la
/// plage verticale de la ligne et vrai au relâchement.
fn poignee(ui: &mut egui::Ui, glisse: &mut Option<(Liste, usize)>, liste: Liste, k: usize) -> (egui::Rangef, bool) {
    let (r, rep) = ui.allocate_exact_size(egui::vec2(16.0, 20.0), Sense::drag());
    let v = ui.visuals();
    let encre = if rep.hovered() || *glisse == Some((liste, k)) { v.strong_text_color() } else { v.weak_text_color() };
    for dy in [-4.0, 0.0, 4.0] {
        ui.painter().line_segment([r.center() + egui::vec2(-5.0, dy), r.center() + egui::vec2(5.0, dy)], egui::Stroke::new(1.3, encre));
    }
    if rep.drag_started() {
        *glisse = Some((liste, k));
    }
    let lacher = rep.drag_stopped();
    rep.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text("Glisser pour déplacer la ligne");
    (r.y_range(), lacher)
}

/// Déplace la ligne glissée de `liste` sous le pointeur ; trace le repère d'insertion.
fn reordonner<T>(ui: &egui::Ui, v: &mut Vec<T>, lignes: &[egui::Rangef], glisse: &mut Option<(Liste, usize)>, liste: Liste, lacher: bool) -> Option<(usize, usize)> {
    let Some((l, k)) = *glisse else { return None };
    if l != liste || k >= v.len() {
        return None;
    }
    let y = ui.input(|i| i.pointer.interact_pos().map(|p| p.y))?;
    let cible = lignes.iter().filter(|l| l.center() < y).count();
    if let (Some(premiere), Some(derniere)) = (lignes.first(), lignes.last()) {
        let yl = if cible == 0 { premiere.min - 2.0 } else { lignes[cible - 1].max + 2.0 }.clamp(premiere.min - 2.0, derniere.max + 2.0);
        let x = ui.min_rect().x_range();
        ui.painter().line_segment([egui::pos2(x.min, yl), egui::pos2(x.max, yl)], egui::Stroke::new(2.0, ui.visuals().selection.stroke.color));
    }
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    if !lacher {
        return None;
    }
    *glisse = None;
    let cible = if cible > k { cible - 1 } else { cible }.min(v.len() - 1);
    let e = v.remove(k);
    v.insert(cible, e);
    Some((k, cible))
}

/// Libellé cliquable d'une ligne : sélectionne l'élément (surligné dans l'élévation).
fn libelle(ui: &mut egui::Ui, texte: &str, element: Element, selection: &mut Option<Element>) {
    let actif = *selection == Some(element);
    let rep = ui.add_sized([86.0, 20.0], egui::Button::selectable(actif, texte));
    if rep.clicked() {
        *selection = if actif { None } else { Some(element) };
    }
}

/// Liste des éléments : horizontaux (main courante, lisses), verticaux (montants,
/// barreaux du motif), remplissage.
fn elements(ui: &mut egui::Ui, p: &Projet, materiaux: &[String], ed: &mut EditeurGardeCorps) {
    let t = &mut ed.brouillon;
    let entete = |ui: &mut egui::Ui, colonnes: [&str; 4]| {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            for (texte, l) in colonnes.into_iter().zip([86.0, 100.0, 70.0, 110.0]) {
                ui.add_sized([l, 14.0], egui::Label::new(RichText::new(texte).small().weak()));
            }
        });
    };
    ui.label(RichText::new("Horizontaux").strong());
    entete(ui, ["Élément", "Profil", "Cote", "Matériau"]);
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        libelle(ui, "Main courante", Element::MainCourante, &mut ed.selection);
        profils(ui, "profil_main", p, &mut t.main_courante, "Carré 50 × 50");
        ui.add_sized([70.0, 20.0], cote(&mut t.hauteur)).on_hover_text("Dessus de la main courante");
        choix_materiau(ui, "materiau_main", materiaux, &mut t.materiau_main_courante, "Global");
    });
    // lisses, de la plus haute à la plus basse à l'ouverture de l'éditeur ; réordonnables
    let mut lignes = Vec::new();
    let mut lacher = false;
    let mut retirer = None;
    for (k, l) in t.lisses.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.push_id(("lisse", k), |ui| {
                let (r, x) = poignee(ui, &mut ed.glisse, Liste::Lisses, k);
                lignes.push(r);
                lacher |= x;
                libelle(ui, &format!("Lisse {}", k + 1), Element::Lisse(k), &mut ed.selection);
                profils(ui, "profil", p, &mut l.profil, "Carré 40 × 40");
                ui.add_sized([70.0, 20.0], cote(&mut l.altitude)).on_hover_text("Axe de la lisse depuis le sol");
                choix_materiau(ui, "materiau", materiaux, &mut l.materiau, "Global");
                if ui.small_button("✕").on_hover_text("Retirer la lisse").clicked() {
                    retirer = Some(k);
                }
            });
        });
    }
    if let Some((a, b)) = reordonner(ui, &mut t.lisses, &lignes, &mut ed.glisse, Liste::Lisses, lacher) {
        suivre_deplacement(&mut ed.selection, Element::Lisse, a, b);
    }
    if lacher && ed.glisse.is_some_and(|g| g.0 == Liste::Lisses) {
        ed.glisse = None;
    }
    if let Some(k) = retirer {
        t.lisses.remove(k);
        ed.selection = None;
    }
    if ui.small_button("+ Lisse").clicked() {
        // à mi-hauteur de l'intervalle libre le plus grand
        let mut z: Vec<f64> = t.lisses.iter().map(|l| l.altitude).chain([0.0, t.hauteur]).collect();
        z.sort_by(f64::total_cmp);
        let altitude = z.windows(2).max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0]))).map_or(t.hauteur / 2.0, |w| ((w[0] + w[1]) / 2.0).round());
        t.lisses.push(Lisse { altitude, profil: None, materiau: None });
        ed.selection = Some(Element::Lisse(t.lisses.len() - 1));
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Verticaux").strong());
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        libelle(ui, "Bas", Element::Bas, &mut ed.selection);
        ui.add_sized([70.0, 20.0], cote(&mut t.bas)).on_hover_text("Bas des barreaux et du remplissage, depuis le sol");
        libelle(ui, "Jeu haut", Element::Haut, &mut ed.selection);
        ui.add_sized([70.0, 20.0], cote(&mut t.jeu_haut)).on_hover_text("Jeu entre le haut des barreaux et le dessus de la main courante");
    });
    entete(ui, ["Élément", "Profil", "Retrait", "Matériau"]);
    for (nom, element, bord) in [("Départ", Element::Depart, &mut t.depart), ("Fin", Element::Fin, &mut t.arrivee)] {
        ui.horizontal(|ui| {
            ui.push_id(nom, |ui| {
                let mut actif = bord.is_some();
                if ui.checkbox(&mut actif, "").on_hover_text("Montant d'extrémité").changed() {
                    *bord = actif.then_some(BarreauExtremite { retrait: 0.0, profil: None, materiau: None });
                }
                libelle(ui, nom, element, &mut ed.selection);
                if let Some(b) = bord {
                    profils(ui, "profil", p, &mut b.profil, "Carré 50 × 50");
                    ui.add_sized([70.0, 20.0], cote(&mut b.retrait)).on_hover_text("Distance de l'extrémité à l'axe du montant");
                    choix_materiau(ui, "materiau", materiaux, &mut b.materiau, "Global");
                } else {
                    ui.label(RichText::new("sans montant").small().weak());
                }
            });
        });
    }
    if t.remplissage == Remplissage::Barreaudage {
        entete(ui, ["Motif", "Profil", "Entraxe", "Matériau"]);
        let mut lignes = Vec::new();
        let mut lacher = false;
        let mut retirer = None;
        for (k, b) in t.motif.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.push_id(("motif", k), |ui| {
                    let (r, x) = poignee(ui, &mut ed.glisse, Liste::Motif, k);
                    lignes.push(r);
                    lacher |= x;
                    libelle(ui, &format!("Barreau {}", k + 1), Element::Motif(k), &mut ed.selection);
                    profils(ui, "profil", p, &mut b.profil, "Carré 20 × 20");
                    ui.add_sized([70.0, 20.0], cote(&mut b.pas)).on_hover_text("Distance entre l'axe précédent et celui de ce barreau");
                    choix_materiau(ui, "materiau", materiaux, &mut b.materiau, "Global");
                    if ui.small_button("✕").on_hover_text("Retirer le barreau du motif").clicked() {
                        retirer = Some(k);
                    }
                });
            });
        }
        if let Some((a, b)) = reordonner(ui, &mut t.motif, &lignes, &mut ed.glisse, Liste::Motif, lacher) {
            suivre_deplacement(&mut ed.selection, Element::Motif, a, b);
        }
        if lacher && ed.glisse.is_some_and(|g| g.0 == Liste::Motif) {
            ed.glisse = None;
        }
        if let Some(k) = retirer {
            t.motif.remove(k);
            ed.selection = None;
        }
        if ui.small_button("+ Barreau").clicked() {
            let pas = t.motif.last().map_or(110.0, |b| b.pas);
            t.motif.push(BarreauMotif { pas, profil: None, materiau: None });
        }
        ui.label(RichText::new("Le motif se répète sur chaque segment ; la fin garde son retrait.").small().weak());
    } else {
        ui.add_space(6.0);
        ui.label(RichText::new("Remplissage").strong());
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            libelle(ui, t.remplissage.nom(), Element::Remplissage, &mut ed.selection);
            choix_materiau(ui, "materiau_remplissage", materiaux, &mut t.materiau_remplissage, if t.remplissage == Remplissage::Vitre { "Vitrage" } else { "Panneau" });
        });
    }
}

/// Sélection qui suit une ligne déplacée de `a` à `b`.
fn suivre_deplacement(selection: &mut Option<Element>, genre: fn(usize) -> Element, a: usize, b: usize) {
    for k in 0..=a.max(b) {
        if *selection == Some(genre(k)) {
            let n = if k == a { b } else if a < k && k <= b { k - 1 } else if b <= k && k < a { k + 1 } else { k };
            *selection = Some(genre(n));
            return;
        }
    }
}

/// Repère de l'élévation : origine (bas gauche) et échelle (px/mm).
struct Repere {
    origine: egui::Pos2,
    s: f32,
}

impl Repere {
    fn de(zone: egui::Rect, longueur: f64, hauteur: f64) -> Self {
        let s = (zone.width() / longueur as f32).min(zone.height() / hauteur as f32);
        Repere { origine: egui::pos2(zone.center().x - longueur as f32 * s / 2.0, zone.center().y + hauteur as f32 * s / 2.0), s }
    }
    fn ecran(&self, x: f64, z: f64) -> egui::Pos2 {
        self.origine + egui::vec2(x as f32 * self.s, -z as f32 * self.s)
    }
    fn z(&self, y: f32) -> f64 {
        ((self.origine.y - y) / self.s) as f64
    }
}

/// Couleur d'un élément : son matériau (ou le global), sinon `defaut`.
fn couleur(p: &Projet, materiau: Option<&str>, defaut: Color32) -> Color32 {
    materiau.and_then(|n| p.materiau(n)).map_or(defaut, |m| {
        let [r, g, b] = m.couleur;
        Color32::from_rgb(r, g, b)
    })
}

/// Élévation du type (vignettes et éditeur) ; `selection` surligne un élément.
/// Retourne le repère utilisé.
fn apercu(peintre: &egui::Painter, zone: egui::Rect, p: &Projet, t: &TypeGardeCorps, longueur: f64, selection: Option<Element>, encre: Color32, accent: Color32) -> Option<Repere> {
    if longueur <= 0.0 || t.hauteur <= 0.0 || !longueur.is_finite() || !t.hauteur.is_finite() {
        return None;
    }
    let r = Repere::de(zone, longueur, t.hauteur);
    let surface = |x0: f64, x1: f64, z0: f64, z1: f64, c: Color32, choisi: bool| {
        let rect = egui::Rect::from_two_pos(r.ecran(x0, z0), r.ecran(x1, z1));
        peintre.rect_filled(rect, 0.0, c);
        if choisi {
            peintre.rect_stroke(rect.expand(1.5), 1.0, egui::Stroke::new(2.0, accent), egui::StrokeKind::Outside);
        }
    };
    let metal = |m: Option<&str>| couleur(p, m, encre);
    let haut = t.hauteur - t.jeu_haut;
    if t.remplissage != Remplissage::Barreaudage {
        let defaut = if t.remplissage == Remplissage::Vitre { Color32::from_rgba_unmultiplied(64, 164, 212, 105) } else { encre.gamma_multiply(0.35) };
        let mut c = couleur(p, t.materiau_remplissage.as_deref(), defaut);
        if t.remplissage == Remplissage::Vitre {
            c = c.gamma_multiply(0.55);
        }
        surface(0.0, longueur, t.bas, haut, c, selection == Some(Element::Remplissage));
    }
    if let Ok(barreaux) = t.barreaux(longueur) {
        let mut n_motif = 0;
        for (i, b) in barreaux.iter().enumerate() {
            let (bas, carre) = if b.extremite { (0.0, 50.0) } else { (t.bas, 20.0) };
            let element = if b.extremite {
                if i == 0 && t.depart.is_some() { Element::Depart } else { Element::Fin }
            } else {
                let k = n_motif % t.motif.len().max(1);
                n_motif += 1;
                Element::Motif(k)
            };
            if let Ok(section) = Section::lire(p, b.profil, carre) {
                let demi = (section.largeur / 2.0).max(0.6 / r.s as f64);
                surface(b.x - demi, b.x + demi, bas, haut, metal(b.materiau.or(t.materiau.as_deref())), selection == Some(element));
            }
        }
    }
    if let Ok(main) = Section::lire(p, t.main_courante.as_deref(), 50.0) {
        surface(0.0, longueur, t.hauteur - main.hauteur, t.hauteur, metal(t.materiau_de(&t.materiau_main_courante)), selection == Some(Element::MainCourante));
    }
    for (k, l) in t.lisses.iter().enumerate() {
        if let Ok(section) = Section::lire(p, l.profil.as_deref(), 40.0) {
            let demi = (section.hauteur / 2.0).max(0.6 / r.s as f64);
            surface(0.0, longueur, l.altitude - demi, l.altitude + demi, metal(t.materiau_de(&l.materiau)), selection == Some(Element::Lisse(k)));
        }
    }
    Some(r)
}

/// Élément horizontal (ligne glissable) le plus proche d'une altitude, à `tol` mm près.
fn horizontal_proche(t: &TypeGardeCorps, z: f64, tol: f64) -> Option<Element> {
    let haut = t.hauteur - t.jeu_haut;
    let mut candidats: Vec<(Element, f64)> = vec![(Element::MainCourante, t.hauteur - 25.0)];
    candidats.extend(t.lisses.iter().enumerate().map(|(k, l)| (Element::Lisse(k), l.altitude)));
    candidats.push((Element::Bas, t.bas));
    candidats.push((Element::Haut, haut));
    candidats.into_iter().map(|(e, a)| (e, (a - z).abs())).filter(|(_, d)| *d <= tol).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(e, _)| e)
}

/// Élévation de l'éditeur : clic pour sélectionner, glisser pour déplacer lisses,
/// main courante (hauteur), bas et haut du remplissage. Cotes des lisses à gauche.
fn elevation_interactive(ui: &mut egui::Ui, p: &Projet, ed: &mut EditeurGardeCorps, hauteur: f32) {
    let (rect, rep) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(300.0), hauteur.max(240.0)), Sense::click_and_drag());
    let peintre = ui.painter_at(rect);
    peintre.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let zone = rect.shrink2(egui::vec2(16.0, 24.0));
    // marge à gauche pour les cotes de niveau, en bas pour la longueur
    let zone = egui::Rect::from_min_max(zone.min + egui::vec2(70.0, 0.0), zone.max - egui::vec2(10.0, 10.0));
    let encre = ui.visuals().text_color();
    let Some(r) = apercu(&peintre, zone, p, &ed.brouillon, ed.longueur, ed.selection, encre, ui.visuals().selection.stroke.color) else { return };
    let t = &mut ed.brouillon;
    let tol = 6.0 / r.s as f64;
    let survol = rep.hover_pos().and_then(|q| horizontal_proche(t, r.z(q.y), tol));
    if rep.drag_started() {
        ed.deplace = rep.interact_pointer_pos().and_then(|q| horizontal_proche(t, r.z(q.y), tol));
        if ed.deplace.is_some() {
            ed.selection = ed.deplace;
        }
    }
    if let (Some(e), Some(q)) = (ed.deplace, rep.interact_pointer_pos().filter(|_| rep.dragged())) {
        let z = (r.z(q.y) / 5.0).round() * 5.0;
        match e {
            Element::MainCourante => t.hauteur = z.max(t.bas + t.jeu_haut + 10.0).max(201.0),
            Element::Lisse(k) => if let Some(l) = t.lisses.get_mut(k) { l.altitude = z.clamp(0.0, t.hauteur) },
            Element::Bas => t.bas = z.clamp(0.0, t.hauteur - t.jeu_haut - 10.0),
            Element::Haut => t.jeu_haut = (t.hauteur - z).clamp(0.0, t.hauteur - t.bas - 10.0),
            _ => {}
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    } else if survol.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if rep.drag_stopped() {
        ed.deplace = None;
    }
    if rep.clicked() {
        // clic : élément horizontal proche, sinon barreau ou remplissage sous le pointeur
        ed.selection = rep.interact_pointer_pos().and_then(|q| {
            let z = r.z(q.y);
            let x = ((q.x - r.origine.x) / r.s) as f64;
            horizontal_proche(t, z, tol).or_else(|| {
                let barreaux = t.barreaux(ed.longueur).ok()?;
                let (mut n, mut trouve) = (0, None);
                for (i, b) in barreaux.iter().enumerate() {
                    let element = if b.extremite { if i == 0 && t.depart.is_some() { Element::Depart } else { Element::Fin } } else { let k = n % t.motif.len().max(1); n += 1; Element::Motif(k) };
                    if (b.x - x).abs() <= tol.max(15.0) {
                        trouve = Some(element);
                    }
                }
                trouve.or_else(|| (t.remplissage != Remplissage::Barreaudage && z > t.bas && z < t.hauteur - t.jeu_haut).then_some(Element::Remplissage))
            })
        });
    }
    // cotes de niveau à gauche : main courante, lisses, bas et haut du remplissage
    let gris = ui.visuals().weak_text_color();
    let accent = ui.visuals().selection.stroke.color;
    let police = egui::FontId::proportional(11.0);
    let x_cote = zone.left() - 8.0;
    let mut niveaux: Vec<(Element, f64, String)> = vec![(Element::MainCourante, t.hauteur, "Main courante".into())];
    niveaux.extend(t.lisses.iter().enumerate().map(|(k, l)| (Element::Lisse(k), l.altitude, format!("Lisse {}", k + 1))));
    if t.bas > 0.0 {
        niveaux.push((Element::Bas, t.bas, "Bas".into()));
    }
    niveaux.push((Element::Haut, t.hauteur - t.jeu_haut, "Haut".into()));
    let sol = r.ecran(0.0, 0.0);
    peintre.line_segment([egui::pos2(x_cote, sol.y), egui::pos2(zone.left(), sol.y)], egui::Stroke::new(1.0, gris));
    for (e, z, nom) in niveaux {
        let y = r.ecran(0.0, z).y;
        let actif = ed.selection == Some(e) || survol == Some(e);
        let c = if actif { accent } else { gris };
        peintre.line_segment([egui::pos2(x_cote, y), egui::pos2(zone.left() - 2.0, y)], egui::Stroke::new(1.0, c));
        peintre.text(egui::pos2(x_cote - 2.0, y), egui::Align2::RIGHT_BOTTOM, unite().afficher(z), police.clone(), c);
        if actif {
            peintre.text(egui::pos2(x_cote - 2.0, y), egui::Align2::RIGHT_TOP, nom, egui::FontId::proportional(10.0), c);
        }
    }
    // longueur en bas
    let a = r.ecran(0.0, 0.0) + egui::vec2(0.0, 8.0);
    let b = r.ecran(ed.longueur, 0.0) + egui::vec2(0.0, 8.0);
    peintre.line_segment([a, b], egui::Stroke::new(1.0, gris));
    for q in [a, b] {
        peintre.line_segment([q - egui::vec2(0.0, 3.0), q + egui::vec2(0.0, 3.0)], egui::Stroke::new(1.0, gris));
    }
    peintre.text(a.lerp(b, 0.5) + egui::vec2(0.0, 2.0), egui::Align2::CENTER_TOP, unite().afficher(ed.longueur), police, gris);
}
