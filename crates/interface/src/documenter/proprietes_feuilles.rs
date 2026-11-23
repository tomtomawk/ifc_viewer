//! Propriétés du jeu de feuilles ouvert et de sa sélection : feuille, vue placée,
//! nomenclature posée, cartouche, annotation.
use crate::*;
use crate::documenter::feuilles::SelectionFeuille;
use commandes::Commande;
use modele::documentation as doc;

impl Application {

    /// Propriétés de la sélection dans le jeu, ou du jeu si rien n'est sélectionné.
    pub(crate) fn proprietes_jeu(&mut self, ui: &mut egui::Ui, jeu: &str) {
        let Some((p, _)) = &self.natif else { return };
        let mut commande: Option<Commande> = None;
        let mut projet_modifie: Option<modele::Projet> = None;
        let mut ouvrir: Option<String> = None;
        let selection = self.feuilles.selection.clone();
        if matches!(selection, SelectionFeuille::Aucune) {
        ui.heading("Jeu de feuilles");
        // nom du jeu : tampon propre, appliqué à Entrée ou en quittant le champ (le jeu
        // ouvert est retrouvé par son nom)
        let cle = egui::Id::unique(("nom_jeu", jeu));
        let mut nom_jeu = ui.data(|d| d.get_temp::<String>(cle)).unwrap_or_else(|| jeu.to_string());
        let r = ui.horizontal(|ui| {
            ui.label("Nom");
            ui.text_edit_singleline(&mut nom_jeu)
        }).inner;
        if r.has_focus() {
            ui.data_mut(|d| d.insert_temp(cle, nom_jeu.clone()));
        } else {
            ui.data_mut(|d| d.remove::<String>(cle));
        }
        if r.lost_focus() && !self.echap_saisie && nom_jeu.trim() != jeu && !nom_jeu.trim().is_empty() {
            let nom_jeu = nom_jeu.trim().to_string();
            if doc::jeux(p).contains(&nom_jeu) {
                self.error = Some("Un jeu porte déjà ce nom.".into());
            } else {
            let mut q = p.clone();
            for f in q.feuilles.iter_mut().filter(|f| f.jeu == jeu) {
                f.jeu = nom_jeu.clone();
            }
            projet_modifie = Some(q);
            ouvrir = Some(nom_jeu);
            }
        }
        let n = p.feuilles.iter().filter(|f| f.jeu == jeu).count();
        ui.label(RichText::new(format!("{n} feuille(s)")).weak());
        }
        let fid = selection.feuille().map(str::to_string);
        let feuille = fid.and_then(|id| p.feuilles.iter().find(|f| f.id == id && f.jeu == jeu));
        match (feuille, &selection) {
            (Some(f), SelectionFeuille::Vue(_, j)) if *j >= f.vues.len() && *j < f.vues.len() + f.nomenclatures.len() => {
                let mut g = f.clone();
                let k = *j - f.vues.len();
                let n = &mut g.nomenclatures[k];
                let nom = p.nomenclatures.iter().find(|x| x.id == n.nomenclature).map_or("Nomenclature absente".to_string(), |x| x.nom.clone());
                ui.heading("Nomenclature posée");
                ui.label(RichText::new(&nom).strong());
                egui::Grid::new("table_posee").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    ui.label("Position X");
                    ui.add(egui::DragValue::new(&mut n.position[0]).speed(1.0).max_decimals(1).suffix(" mm"));
                    ui.end_row();
                    ui.label("Position Y");
                    ui.add(egui::DragValue::new(&mut n.position[1]).speed(1.0).max_decimals(1).suffix(" mm"));
                    ui.end_row();
                });
                ui.label(RichText::new("Coin haut gauche du tableau ; colonnes et regroupement : ouvrir la nomenclature").small().weak());
                if serde_json::to_string(&g).ok() != serde_json::to_string(f).ok() {
                    commande = Some(Commande::Feuille { feuille: g });
                }
            }
            (Some(f), SelectionFeuille::Vue(_, j)) if *j < f.vues.len() => {
                let mut g = f.clone();
                let v = &mut g.vues[*j];
                let source = p.vues.iter().position(|x| x.id == v.vue);
                let nom = source.map_or("Vue absente", |i| p.vues[i].nom.as_str());
                let echelle_avant = v.echelle_sur_feuille(p);
                let mut echelle = echelle_avant;
                let gabaritee = p.gabarit_vue(&v.vue).is_some();
                ui.heading("Vue placée");
                ui.label(RichText::new(nom).strong());
                egui::Grid::new("vue_placee").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    ui.label("Échelle");
                    ui.add_enabled(source.is_some() && !gabaritee, egui::DragValue::new(&mut echelle).speed(1.0).range(1.0..=10_000.0).prefix("1 : "))
                        .on_disabled_hover_text("Échelle contrôlée par le gabarit de vue.")
                        .on_hover_text("Échelle commune à la vue et à ses placements sur les feuilles.");
                    ui.end_row();
                    ui.label("Position X");
                    ui.add(egui::DragValue::new(&mut v.position[0]).speed(1.0).max_decimals(1).suffix(" mm"));
                    ui.end_row();
                    ui.label("Position Y");
                    ui.add(egui::DragValue::new(&mut v.position[1]).speed(1.0).max_decimals(1).suffix(" mm"));
                    ui.end_row();
                });
                if let Some(i) = source.filter(|_| echelle != echelle_avant) {
                    let mut q = p.clone();
                    q.vues[i].echelle = echelle;
                    if let Some(dest) = q.feuilles.iter_mut().find(|x| x.id == g.id) { *dest = g; }
                    projet_modifie = Some(q);
                } else if serde_json::to_string(&g).ok() != serde_json::to_string(f).ok() {
                    commande = Some(Commande::Feuille { feuille: g });
                }
            }
            (Some(f), SelectionFeuille::Cartouche(_)) => {
                ui.heading("Cartouche");
                projet_modifie = selecteur_cartouche(ui, p, f);
                if projet_modifie.is_none() {
                    commande = crate::documenter::reglages_feuille::reglages_cartouche(ui, p, f);
                }
            }
            (Some(f), SelectionFeuille::Annotation(_, i)) if *i < f.annotations.len() => {
                commande = crate::documenter::reglages_feuille::reglages_annotation(ui, p, f, *i);
            }
            (Some(f), _) => {
                let mut g = f.clone();
                ui.heading("Feuille");
                egui::Grid::new("feuille_proprietes").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    ui.label("Numéro");
                    ui.text_edit_singleline(&mut g.numero);
                    ui.end_row();
                    ui.label("Nom");
                    ui.text_edit_singleline(&mut g.nom);
                    ui.end_row();
                    ui.label("Format");
                    let nom_format = doc::FORMATS.iter().find(|(_, l, h)| (g.largeur.max(g.hauteur) - l).abs() < 0.5 && (g.largeur.min(g.hauteur) - h).abs() < 0.5).map_or("Personnalisé", |x| x.0);
                    let portrait = g.hauteur > g.largeur;
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("format_feuille").selected_text(nom_format).width(70.0).show_ui(ui, |ui| {
                            for (n, l, h) in doc::FORMATS {
                                if ui.selectable_label(n == nom_format, n).clicked() {
                                    (g.largeur, g.hauteur) = if portrait { (h, l) } else { (l, h) };
                                }
                            }
                        });
                        if ui.selectable_label(!portrait, "Paysage").clicked() && portrait {
                            std::mem::swap(&mut g.largeur, &mut g.hauteur);
                        }
                        if ui.selectable_label(portrait, "Portrait").clicked() && !portrait {
                            std::mem::swap(&mut g.largeur, &mut g.hauteur);
                        }
                    });
                    ui.end_row();
                    ui.label("Dimensions");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut g.largeur).speed(1.0).range(50.0..=5000.0).suffix(" mm"));
                        ui.add(egui::DragValue::new(&mut g.hauteur).speed(1.0).range(50.0..=5000.0).suffix(" mm"));
                    });
                    ui.end_row();
                    ui.label("Cartouche");
                    let mut avec = !g.sans_cartouche;
                    ui.checkbox(&mut avec, "Afficher").on_hover_text("Décocher retire le cartouche de cette feuille ; sa position est gardée.");
                    g.sans_cartouche = !avec;
                    ui.end_row();
                });
                ui.label(RichText::new(format!("{} vue(s) placée(s), {} nomenclature(s), {} annotation(s) · glisser une vue ou une nomenclature de l'arborescence sur la feuille", f.vues.len(), f.nomenclatures.len(), f.annotations.len())).small().weak());
                if commande.is_none() && serde_json::to_string(&g).ok() != serde_json::to_string(f).ok() {
                    commande = Some(Commande::Feuille { feuille: g });
                }
            }
            _ => {
                ui.label(RichText::new("Cliquer une feuille ou une vue placée pour la régler.").weak());
            }
        }
        if let Some(q) = projet_modifie
            && let Err(e) = self.appliquer_projet(q) {
                self.error = Some(e);
            }
        // cartouche ou annotation retirés depuis Propriétés : la feuille reste choisie
        if let Some(Commande::Feuille { feuille: g }) = &commande {
            let retire = match &selection {
                SelectionFeuille::Cartouche(_) => g.sans_cartouche,
                SelectionFeuille::Annotation(_, i) => *i >= g.annotations.len(),
                _ => false,
            };
            if retire { self.feuilles.selection = SelectionFeuille::Feuille(g.id.clone()); }
        }
        if let Some(c) = commande
            && let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
        if let Some(id) = ouvrir.as_deref().and_then(|o| o.strip_prefix("nomenclature:")) {
            self.ouvrir_nomenclature(id);
        } else if let Some(o) = ouvrir {
            match o.strip_prefix("vue:") {
                Some(id) => {
                    let n = modele::identifiants::numero_vue(id);
                    if let Some(i) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|nav| nav.vues.iter().position(|v| v.revit_id == n)) {
                        self.activer_vue(i);
                    }
                }
                None => self.ouvrir_jeu(&o, None),
            }
        }
    }
}

fn selecteur_cartouche(ui: &mut egui::Ui, p: &modele::Projet, f: &doc::Feuille) -> Option<modele::Projet> {
    let actuel = doc::famille_cartouche(p, &f.id).map(|f| f.nom.as_str()).filter(|nom| *nom != doc::NOM_CARTOUCHE_STANDARD);
    let mut choix = actuel;
    {
        use crate::zones::ruban::selecteur;
        let titre = format!("Cartouche · {}", actuel.unwrap_or("Standard"));
        let bouton = selecteur::entete(ui, &titre, None, &format!("{titre}
Famille de cartouche de la feuille."), None);
        let largeur = selecteur::largeur_liste(ui, p.familles_cartouches.iter().map(|f| f.nom.as_str()).chain(["Cartouche standard"]), 0.0, 0, 0.0);
        selecteur::liste(&bouton, largeur, 0.0, |ui| {
            let l = selecteur::Ligne { nom: "Cartouche standard", courant: actuel.is_none(), etiquette: None, sous_titre: None, detail: None, actions: &[] };
            if selecteur::ligne(ui, &l, None).choisi { choix = None; }
            for famille in p.familles_cartouches.iter().filter(|famille| famille.nom != doc::NOM_CARTOUCHE_STANDARD) {
                let l = selecteur::Ligne { nom: &famille.nom, courant: actuel == Some(famille.nom.as_str()), etiquette: None, sous_titre: None, detail: None, actions: &[] };
                if selecteur::ligne(ui, &l, None).choisi { choix = Some(famille.nom.as_str()); }
            }
        });
    }
    if choix == actuel { return None; }
    let mut projet = p.clone();
    if let Some(nom) = choix {
        projet.cartouches_feuilles.insert(f.id.clone(), nom.to_string());
    } else {
        projet.cartouches_feuilles.remove(&f.id);
    }
    Some(projet)
}
