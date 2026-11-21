//! Motifs de hachures dans l'arborescence du projet, comme les matériaux : liste avec
//! aperçu, clic pour les propriétés, double-clic pour redessiner un motif dessiné, menu
//! (Redessiner, Dupliquer, Supprimer ou Rétablir), Nouveau. Propriétés du motif choisi :
//! nom (renommer met à jour les matériaux), familles de traits (angle, pas, glissement,
//! origine, tirets en mm du papier) et aperçu. Un motif de base modifié est copié dans le
//! projet.
use crate::*;
use crate::socle::aides::apercu_motif;
use modele::{DefinitionMotif, LigneMotif, MotifCoupe};

/// Action demandée sur un motif (arborescence ou propriétés).
enum ActionMotif {
    Choisir(String),
    Redessiner(String),
    Dessiner,
    Nouveau,
    Dupliquer(String),
    Supprimer(String),
}

/// Nom libre dérivé de `base` (« base », « base 2 »…).
fn nom_libre(p: &modele::Projet, base: &str) -> String {
    let noms = p.noms_motifs();
    (1..).map(|k| if k == 1 { base.to_string() } else { format!("{base} {k}") }).find(|n| !noms.contains(n)).unwrap_or_default()
}

/// Matériaux qui utilisent le motif `nom`.
fn utilise(p: &modele::Projet, nom: &str) -> bool {
    p.materiaux.iter().any(|m| [Some(&m.motif), m.surface_motif.as_ref(), m.surface_fond_motif.as_ref(), m.coupe_fond_motif.as_ref()].into_iter().flatten().any(|x| x.nom_hachures() == Some(nom)))
}

impl Application {
    /// Branche « Motifs de hachures » de l'arborescence.
    pub(crate) fn motifs_ui(&mut self, ui: &mut egui::Ui) {
        let Some((p, _)) = &self.natif else { return };
        let q = self.filter.trim().to_lowercase();
        let mut action = None;
        for n in p.noms_motifs().into_iter().filter(|n| q.is_empty() || n.to_lowercase().contains(&q)) {
            let Some(d) = p.definition_motif(&MotifCoupe::hachures(&n)) else { continue };
            let du_projet = p.motifs.iter().any(|x| x.nom == n);
            let de_base = DefinitionMotif::base().iter().any(|x| x.nom == n);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(22.0, 14.0), Sense::hover());
                ui.painter().rect_filled(r, 0.0, Color32::WHITE);
                apercu_motif(ui.painter(), r, &d, 2.5);
                let texte = if d.tuile.is_some() { format!("{n} (dessiné)") } else { n.clone() };
                let resp = ui.selectable_label(self.motif_selectionne.as_deref() == Some(n.as_str()), texte)
                    .on_hover_text("Clic : propriétés ; double-clic : redessiner");
                if resp.double_clicked() {
                    action = Some(ActionMotif::Redessiner(n.clone()));
                } else if resp.clicked() {
                    action = Some(ActionMotif::Choisir(n.clone()));
                }
                resp.context_menu(|ui| {
                    if ui.button("Redessiner…").clicked() { action = Some(ActionMotif::Redessiner(n.clone())); ui.close(); }
                    if ui.button("Dupliquer").clicked() { action = Some(ActionMotif::Dupliquer(n.clone())); ui.close(); }
                    let libelle = if de_base && du_projet { "Rétablir" } else { "Supprimer" };
                    if ui.add_enabled(du_projet && (de_base || !utilise(p, &n)), egui::Button::new(libelle)).clicked() {
                        action = Some(ActionMotif::Supprimer(n.clone()));
                        ui.close();
                    }
                });
            });
        }
        ui.horizontal(|ui| {
            if ui.small_button("Dessiner un motif…").on_hover_text("Nouveau motif tracé sur un carré répété (vue de dessin)").clicked() { action = Some(ActionMotif::Dessiner); }
            if ui.small_button("Nouveau").on_hover_text("Nouveau motif à familles de traits").clicked() { action = Some(ActionMotif::Nouveau); }
        });
        if let Some(a) = action { self.action_motif(a); }
    }

    fn action_motif(&mut self, action: ActionMotif) {
        let Some((projet, _)) = &self.natif else { return };
        let mut p = projet.clone();
        match action {
            ActionMotif::Choisir(n) => {
                self.select(None, false);
                self.motif_selectionne = Some(n);
                return;
            }
            ActionMotif::Redessiner(n) => { self.ouvrir_editeur_hachure(Some(&n)); return; }
            ActionMotif::Dessiner => { self.ouvrir_editeur_hachure(None); return; }
            ActionMotif::Nouveau => {
                let n = nom_libre(&p, "Motif");
                p.motifs.push(DefinitionMotif { id: noyau::uuid::uuid(), nom: n.clone(), lignes: vec![LigneMotif { angle: 45.0, pas: 2.0, glissement: 0.0, origine: [0.0, 0.0], tirets: Vec::new() }], tuile: None });
                self.select(None, false);
                self.motif_selectionne = Some(n);
            }
            ActionMotif::Dupliquer(n) => {
                let Some(d) = p.definition_motif(&MotifCoupe::hachures(&n)) else { return };
                let copie = nom_libre(&p, &format!("{n} (copie)"));
                p.motifs.push(DefinitionMotif { id: noyau::uuid::uuid(), nom: copie.clone(), ..d });
                self.select(None, false);
                self.motif_selectionne = Some(copie);
            }
            ActionMotif::Supprimer(n) => {
                p.motifs.retain(|x| x.nom != n);
            }
        }
        if let Err(e) = self.appliquer_projet(p) {
            self.error = Some(e);
        }
    }

    /// Propriétés du motif choisi dans l'arborescence ; false s'il n'y en a pas.
    pub(crate) fn proprietes_motif_selectionne(&mut self, ui: &mut egui::Ui) -> bool {
        let (Some(choisi), Some((projet, _))) = (self.motif_selectionne.clone(), &self.natif) else { return false };
        let Some(mut d) = projet.definition_motif(&MotifCoupe::hachures(&choisi)) else {
            self.motif_selectionne = None;
            return false;
        };
        let avant = d.clone();
        if self.tirets_saisis.0 != choisi {
            self.tirets_saisis = (choisi.clone(), d.lignes.iter().map(|l| l.tirets.iter().map(|t| noyau::nombres::format_real(*t)).collect::<Vec<_>>().join(" ")).collect());
            self.nom_motif_saisi = choisi.clone();
        }
        let mut renommer = false;
        let mut action = None;
        ui.heading("Motif de hachures");
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Nom");
                let r = ui.add(egui::TextEdit::singleline(&mut self.nom_motif_saisi).desired_width(180.0));
                renommer = r.lost_focus() && self.nom_motif_saisi.trim() != choisi && !self.nom_motif_saisi.trim().is_empty();
            });
            if d.tuile.is_some() {
                ui.label(RichText::new("Motif dessiné : modifier ses familles à la main abandonne son dessin.").small().weak());
            }
            let mut retirer = None;
            for (k, l) in d.lignes.iter_mut().enumerate() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Famille {}", k + 1)).strong());
                    if ui.add_enabled(k > 0, egui::Button::new("Retirer").small()).on_hover_text("Retirer la famille").clicked() {
                        retirer = Some(k);
                    }
                });
                egui::Grid::new(("famille_motif", k)).num_columns(2).spacing([8.0, 3.0]).show(ui, |ui| {
                    ui.label("Angle");
                    ui.add(egui::DragValue::new(&mut l.angle).speed(1.0).range(-180.0..=180.0).suffix(" °"));
                    ui.end_row();
                    ui.label("Pas");
                    ui.add(egui::DragValue::new(&mut l.pas).speed(0.05).range(0.01..=100.0).max_decimals(3).suffix(" mm"));
                    ui.end_row();
                    ui.label("Glissement");
                    ui.add(egui::DragValue::new(&mut l.glissement).speed(0.05).max_decimals(3).suffix(" mm"));
                    ui.end_row();
                    ui.label("Origine");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut l.origine[0]).speed(0.05).max_decimals(3));
                        ui.add(egui::DragValue::new(&mut l.origine[1]).speed(0.05).max_decimals(3));
                    });
                    ui.end_row();
                    ui.label("Tirets");
                    if let Some(texte) = self.tirets_saisis.1.get_mut(k)
                        && ui.add(egui::TextEdit::singleline(texte).desired_width(120.0).hint_text("continu")).on_hover_text("Trait, blanc, trait… en mm du papier (blancs négatifs)").changed() {
                            let v: Option<Vec<f64>> = texte.replace(',', ".").split_whitespace().map(|x| x.parse::<f64>().ok().filter(|x| x.is_finite() && *x != 0.0)).collect();
                            if let Some(v) = v { l.tirets = v; }
                        }
                    ui.end_row();
                });
            }
            if let Some(k) = retirer {
                d.lignes.remove(k);
                self.tirets_saisis.1.remove(k);
            }
            if ui.button("+ Famille de traits").clicked() {
                d.lignes.push(LigneMotif { angle: 0.0, pas: 2.0, glissement: 0.0, origine: [0.0, 0.0], tirets: Vec::new() });
                self.tirets_saisis.1.push(String::new());
            }
            ui.add_space(6.0);
            ui.label(RichText::new("Aperçu (3 × 3 cm du papier)").small().weak());
            let (r, _) = ui.allocate_exact_size(egui::vec2(150.0, 150.0), Sense::hover());
            ui.painter().rect_filled(r, 0.0, Color32::WHITE);
            apercu_motif(ui.painter(), r, &d, 5.0);
            ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Redessiner…").on_hover_text("Ouvrir le motif dans sa vue de dessin (un motif à familles repart d'un carré vide)").clicked() { action = Some(ActionMotif::Redessiner(choisi.clone())); }
                if ui.button("Dupliquer").clicked() { action = Some(ActionMotif::Dupliquer(choisi.clone())); }
            });
        });
        if let Some(a) = action {
            self.action_motif(a);
            return true;
        }
        let mut p = projet.clone();
        if renommer {
            let n = self.nom_motif_saisi.trim().to_string();
            if p.noms_motifs().contains(&n) {
                self.error = Some(format!("Un motif s'appelle déjà « {n} »."));
                return true;
            }
            // copie renommée ; les matériaux qui se servaient de l'ancien suivent
            p.motifs.retain(|x| x.nom != choisi);
            p.motifs.push(DefinitionMotif { id: if d.id.is_empty() { noyau::uuid::uuid() } else { d.id.clone() }, nom: n.clone(), ..d });
            let (ancien, neuf) = (MotifCoupe::hachures(&choisi), MotifCoupe::hachures(&n));
            for m in &mut p.materiaux {
                for x in [&mut m.motif].into_iter().chain([&mut m.surface_motif, &mut m.surface_fond_motif, &mut m.coupe_fond_motif].into_iter().flatten()) {
                    if *x == ancien { *x = neuf.clone(); }
                }
            }
            self.motif_selectionne = Some(n);
        } else if d != avant {
            // familles de traits modifiées à la main : le dessin d'origine ne vaut plus
            if d.lignes != avant.lignes { d.tuile = None; }
            match p.motifs.iter_mut().find(|x| x.nom == choisi) {
                Some(x) => *x = d,
                None => p.motifs.push(d),
            }
        } else {
            return true;
        }
        if let Err(e) = self.appliquer_projet(p) {
            self.error = Some(e);
        }
        true
    }
}
