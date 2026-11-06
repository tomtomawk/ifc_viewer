//! Fenêtres matériau, plumes et motifs.
use super::*;

impl ViewerApp {
    /// Graphismes d'un matériau du projet natif, comme dans Revit (sans ombrage) : motifs
    /// de surface et de coupe, chacun en premier plan et en arrière-plan (motif, couleur).
    /// L'arrière-plan est uni ou hachuré ; « Motifs… » ouvre l'éditeur de hachures.
    pub(super) fn fenetre_materiau(&mut self, ctx: &egui::Context) {
        use crate::modele::MotifCoupe;
        let Some(nom) = self.materiau_edite.clone() else { return };
        let Some((projet, _)) = &self.natif else { self.materiau_edite = None; return };
        let Some(mut m) = projet.materiau(&nom).cloned() else { self.materiau_edite = None; return };
        let avant = m.clone();
        let hachures: Vec<MotifCoupe> = projet.noms_motifs().iter().map(|n| MotifCoupe::hachures(n)).collect();
        let mut ouvert = true;
        let mut editeur = false;
        let mut onglet_apparence = self.materiau_apparence;
        egui::Window::new(format!("Matériau · {nom}")).id(egui::Id::unique("fenetre_materiau")).open(&mut ouvert).resizable(false).collapsible(false).show(ctx, |ui| {
            let choix = |ui: &mut egui::Ui, id: &str, x: &mut MotifCoupe, speciaux: &[MotifCoupe]| {
                egui::ComboBox::from_id_salt(id).width(130.0).selected_text(x.libelle().to_string()).show_ui(ui, |ui| {
                    for t in speciaux.iter().chain(hachures.iter()) {
                        let l = t.libelle().to_string();
                        ui.selectable_value(x, t.clone(), l);
                    }
                });
            };
            let couleur = |ui: &mut egui::Ui, c: &mut Option<[u8; 3]>, defaut: [u8; 3]| {
                let mut v = c.unwrap_or(defaut);
                if ui.color_edit_button_srgb(&mut v).changed() {
                    *c = Some(v);
                }
            };
            ui.horizontal(|ui| {
                ui.selectable_value(&mut onglet_apparence, false, "Graphique");
                ui.selectable_value(&mut onglet_apparence, true, "Apparence");
            });
            ui.separator();
            if onglet_apparence {
                apparence_materiau(ui, &mut m);
                return;
            }
            egui::Grid::new("graphismes_materiau").num_columns(3).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(RichText::new("Motifs de surface").strong());
                ui.end_row();
                ui.label("Premier plan");
                let mut p = m.surface_motif.clone().unwrap_or(MotifCoupe::Vide);
                choix(ui, "surface_premier", &mut p, &[MotifCoupe::Vide, MotifCoupe::Uni]);
                m.surface_motif = (p != MotifCoupe::Vide).then_some(p);
                let d = m.encre_surface();
                couleur(ui, &mut m.surface_couleur, d);
                ui.end_row();
                ui.label("Arrière-plan");
                let mut f = m.fond_surface();
                choix(ui, "surface_arriere", &mut f, &[MotifCoupe::Uni]);
                m.surface_fond_motif = (f != MotifCoupe::Uni).then_some(f);
                ui.color_edit_button_srgb(&mut m.couleur).on_hover_text("Couleur des faces (arrière-plan uni) ; aussi celle du style Réaliste");
                ui.end_row();
                ui.label(RichText::new("Motifs de coupe").strong());
                ui.end_row();
                ui.label("Premier plan");
                choix(ui, "coupe_premier", &mut m.motif, &[MotifCoupe::Vide, MotifCoupe::Uni, MotifCoupe::Isolation]);
                let d = m.encre_coupe();
                couleur(ui, &mut m.coupe_couleur, d);
                ui.end_row();
                ui.label("Arrière-plan");
                let (mut f, teinte) = m.fond_coupe();
                choix(ui, "coupe_arriere", &mut f, &[MotifCoupe::Uni]);
                m.coupe_fond_motif = (f != MotifCoupe::Uni).then_some(f);
                couleur(ui, &mut m.coupe_fond, teinte);
                ui.end_row();
            });
            ui.add_space(4.0);
            if ui.button("Motifs de hachures…").on_hover_text("Créer et modifier les motifs").clicked() {
                editeur = true;
            }
        });
        self.materiau_apparence = onglet_apparence;
        if editeur {
            self.editeur_motifs = true;
        }
        if !ouvert {
            self.materiau_edite = None;
        }
        if m != avant {
            let mut p = projet.clone();
            if let Some(x) = p.materiaux.iter_mut().find(|x| x.nom == nom) {
                *x = m;
            }
            if let Err(e) = self.appliquer_projet(p) {
                self.error = Some(e);
            }
        }
    }

    /// Éditeur de hachures : motifs du projet, du gabarit et de base ; nom, familles de
    /// traits (angle, pas, glissement, origine, tirets en mm du papier) et aperçu. Un motif
    /// du gabarit ou de base modifié est copié dans le projet ; renommer un motif met à
    /// jour les matériaux qui s'en servent.
    /// Fenêtre « Épaisseurs de traits » : épaisseur de chaque plume (mm du papier),
    /// aperçu, couches qui l'utilisent ; table du projet, ou celle du gabarit rétablie.
    pub(super) fn fenetre_plumes(&mut self, ctx: &egui::Context) {
        let Some(mut table) = self.editeur_plumes.take() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let gabarit = crate::modele::Projet::default().epaisseurs_plumes();
        let actuelle = projet.epaisseurs_plumes();
        // couches (types de murs et de dalles) par plume
        let mut usages = [0usize; 12];
        for c in projet.types_murs.iter().chain(&projet.types_dalles).flat_map(|t| &t.couches) {
            usages[(c.plume().clamp(1, 12) - 1) as usize] += 1;
        }
        let (mut ouvert, mut fermer, mut appliquer) = (true, false, false);
        egui::Window::new("Épaisseurs de traits").id(egui::Id::unique("fenetre_plumes")).open(&mut ouvert).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new("Épaisseur de chaque plume en mm du papier : un trait de 0,35 mm mesure 35 mm réels au 1:100.").small().weak());
            ui.add_space(4.0);
            egui::Grid::new("grille_plumes").num_columns(4).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
                ui.label(RichText::new("Plume").strong());
                ui.label(RichText::new("Épaisseur").strong());
                ui.label(RichText::new("Aperçu").strong());
                ui.label(RichText::new("Couches").strong());
                ui.end_row();
                for (i, e) in table.iter_mut().enumerate() {
                    let n = i + 1;
                    let defaut = match n { 5 => " (structure)", 3 => " (isolation)", 2 => " (autres)", _ => "" };
                    ui.label(format!("{n}{defaut}"));
                    ui.add(egui::DragValue::new(e).speed(0.005).range(0.01..=5.0).max_decimals(2).suffix(" mm"));
                    // aperçu à l'échelle 1:1 du papier (environ 3,8 pixels par mm)
                    let (r, _) = ui.allocate_exact_size(egui::vec2(90.0, 14.0), Sense::hover());
                    ui.painter().line_segment([r.left_center(), r.right_center()], egui::Stroke::new((*e as f32 * 3.78).max(0.5), ui.visuals().text_color()));
                    ui.label(if usages[i] > 0 { usages[i].to_string() } else { "—".into() }).on_hover_text("Couches de types de murs et de dalles qui utilisent cette plume (réglée ou par défaut)");
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let change = table.iter().zip(&actuelle).any(|(a, b)| (a - b).abs() > 1e-9);
                if ui.add_enabled(change, egui::Button::new("Appliquer")).clicked() {
                    appliquer = true;
                }
                if ui.add_enabled(table.iter().zip(&gabarit).any(|(a, b)| (a - b).abs() > 1e-9), egui::Button::new("Valeurs du gabarit"))
                    .on_hover_text("Reprendre les épaisseurs du gabarit (Appliquer pour valider)")
                    .clicked()
                {
                    table = gabarit.clone();
                }
                if ui.button("Fermer").clicked() {
                    fermer = true;
                }
            });
        });
        if appliquer {
            let mut p = projet.clone();
            // table identique à celle du gabarit : le projet suit le gabarit
            p.plumes = if table.iter().zip(&gabarit).all(|(a, b)| (a - b).abs() < 1e-9) { Vec::new() } else { table.clone() };
            match self.appliquer_projet(p) {
                Ok(()) => {
                    self.coupe_dirty = true;
                    self.status = "Épaisseurs de traits appliquées".into();
                }
                Err(e) => self.error = Some(e),
            }
        }
        if ouvert && !fermer {
            self.editeur_plumes = Some(table);
        }
    }

    pub(super) fn fenetre_motifs(&mut self, ctx: &egui::Context) {
        use crate::modele::{DefinitionMotif, LigneMotif, MotifCoupe};
        if !self.editeur_motifs {
            return;
        }
        let Some((projet, _)) = &self.natif else { self.editeur_motifs = false; return };
        let noms = projet.noms_motifs();
        let choisi = self.motif_choisi.clone().filter(|n| noms.contains(n)).or_else(|| noms.first().cloned());
        let Some(choisi) = choisi else { return };
        let Some(mut d) = projet.definition_motif(&MotifCoupe::hachures(&choisi)) else { return };
        let avant = d.clone();
        let du_projet = projet.motifs.iter().any(|x| x.nom == choisi);
        let de_base = DefinitionMotif::base().iter().any(|x| x.nom == choisi) || crate::config::gabarit::actif().motifs.iter().any(|x| x.nom == choisi);
        let utilise = projet.materiaux.iter().any(|m| [Some(&m.motif), m.surface_motif.as_ref(), m.surface_fond_motif.as_ref(), m.coupe_fond_motif.as_ref()].into_iter().flatten().any(|x| x.nom_hachures() == Some(choisi.as_str())));
        if self.tirets_saisis.0 != choisi {
            self.tirets_saisis = (choisi.clone(), d.lignes.iter().map(|l| l.tirets.iter().map(|t| crate::formats::ifc::step::format_real(*t)).collect::<Vec<_>>().join(" ")).collect());
            self.nom_motif_saisi = choisi.clone();
        }
        let mut ouvert = true;
        let mut action: Option<&str> = None;
        let mut nouveau_choix: Option<String> = None;
        egui::Window::new("Motifs de hachures").id(egui::Id::unique("fenetre_motifs")).open(&mut ouvert).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                // liste des motifs, avec un aperçu
                ui.vertical(|ui| {
                    ui.set_width(190.0);
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for n in &noms {
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(22.0, 16.0), Sense::hover());
                                if let Some(x) = projet.definition_motif(&MotifCoupe::hachures(n)) {
                                    apercu_motif(ui.painter(), r, &x, 2.5);
                                }
                                if ui.selectable_label(*n == choisi, n).clicked() {
                                    nouveau_choix = Some(n.clone());
                                }
                            });
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Nouveau").clicked() { action = Some("nouveau"); }
                        if ui.button("Dupliquer").clicked() { action = Some("dupliquer"); }
                    });
                    let libelle = if de_base && du_projet { "Rétablir" } else { "Supprimer" };
                    let possible = du_projet && (de_base || !utilise);
                    if ui.add_enabled(possible, egui::Button::new(libelle))
                        .on_disabled_hover_text(if !du_projet { "Motif du gabarit ou de base, inchangé" } else { "Motif utilisé par un matériau" })
                        .clicked()
                    {
                        action = Some("supprimer");
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Nom");
                        let r = ui.add(egui::TextEdit::singleline(&mut self.nom_motif_saisi).desired_width(180.0));
                        if r.lost_focus() && self.nom_motif_saisi.trim() != choisi && !self.nom_motif_saisi.trim().is_empty() {
                            action = Some("renommer");
                        }
                    });
                    ui.add_space(4.0);
                    let mut retirer = None;
                    egui::Grid::new("lignes_motif").num_columns(7).spacing([6.0, 4.0]).striped(true).show(ui, |ui| {
                        for t in ["Angle", "Pas", "Glissement", "Origine x", "Origine y", "Tirets (trait, blanc… mm)", ""] {
                            ui.label(RichText::new(t).small().weak());
                        }
                        ui.end_row();
                        for (k, l) in d.lignes.iter_mut().enumerate() {
                            ui.add(egui::DragValue::new(&mut l.angle).speed(1.0).range(-180.0..=180.0).suffix(" °"));
                            ui.add(egui::DragValue::new(&mut l.pas).speed(0.05).range(0.1..=100.0).max_decimals(2).suffix(" mm"));
                            ui.add(egui::DragValue::new(&mut l.glissement).speed(0.05).max_decimals(2));
                            ui.add(egui::DragValue::new(&mut l.origine[0]).speed(0.05).max_decimals(2));
                            ui.add(egui::DragValue::new(&mut l.origine[1]).speed(0.05).max_decimals(2));
                            if let Some(texte) = self.tirets_saisis.1.get_mut(k) {
                                if ui.add(egui::TextEdit::singleline(texte).desired_width(110.0).hint_text("continu")).changed() {
                                    let v: Option<Vec<f64>> = texte.replace(',', ".").split_whitespace().map(|x| x.parse::<f64>().ok().filter(|x| x.is_finite() && *x != 0.0)).collect();
                                    if let Some(v) = v { l.tirets = v; }
                                }
                            }
                            if ui.add_enabled(k > 0, egui::Button::new("Retirer").small()).on_hover_text("Retirer la famille").clicked() {
                                retirer = Some(k);
                            }
                            ui.end_row();
                        }
                    });
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
                });
            });
        });
        if !ouvert {
            self.editeur_motifs = false;
        }
        if let Some(n) = nouveau_choix {
            self.motif_choisi = Some(n);
            return;
        }
        let mut p = projet.clone();
        let mut change = false;
        let libre = |p: &crate::modele::Projet, base: &str| {
            let mut k = 1;
            loop {
                let n = if k == 1 { base.to_string() } else { format!("{base} {k}") };
                if !p.noms_motifs().contains(&n) { break n; }
                k += 1;
            }
        };
        match action {
            Some("nouveau") => {
                let n = libre(&p, "Motif");
                p.motifs.push(DefinitionMotif { id: crate::modele::collaboration::uuid(), nom: n.clone(), lignes: vec![LigneMotif { angle: 45.0, pas: 2.0, glissement: 0.0, origine: [0.0, 0.0], tirets: Vec::new() }] });
                self.motif_choisi = Some(n);
                change = true;
            }
            Some("dupliquer") => {
                let n = libre(&p, &format!("{} (copie)", d.nom));
                p.motifs.push(DefinitionMotif { id: crate::modele::collaboration::uuid(), nom: n.clone(), ..d.clone() });
                self.motif_choisi = Some(n);
                change = true;
            }
            Some("supprimer") => {
                p.motifs.retain(|x| x.nom != choisi);
                self.tirets_saisis.0.clear();
                change = true;
            }
            Some("renommer") => {
                let n = self.nom_motif_saisi.trim().to_string();
                if p.noms_motifs().contains(&n) {
                    self.error = Some(format!("Un motif s'appelle déjà « {n} »."));
                } else {
                    // copie renommée ; les matériaux qui se servaient de l'ancien suivent
                    p.motifs.retain(|x| x.nom != choisi);
                    p.motifs.push(DefinitionMotif { id: if d.id.is_empty() { crate::modele::collaboration::uuid() } else { d.id.clone() }, nom: n.clone(), ..d.clone() });
                    let (ancien, neuf) = (MotifCoupe::hachures(&choisi), MotifCoupe::hachures(&n));
                    for m in &mut p.materiaux {
                        for x in [&mut m.motif].into_iter().chain([&mut m.surface_motif, &mut m.surface_fond_motif, &mut m.coupe_fond_motif].into_iter().flatten()) {
                            if *x == ancien { *x = neuf.clone(); }
                        }
                    }
                    self.motif_choisi = Some(n);
                    change = true;
                }
            }
            _ if d != avant => {
                match p.motifs.iter_mut().find(|x| x.nom == choisi) {
                    Some(x) => *x = d,
                    None => p.motifs.push(d),
                }
                change = true;
            }
            _ => {}
        }
        if change && let Err(e) = self.appliquer_projet(p) {
            self.error = Some(e);
        }
    }

}

/// Onglet Apparence de la fenêtre Matériau : réglages physiques et cartes de texture
/// (image, taille réelle, décalage, rotation, intensité).
fn apparence_materiau(ui: &mut egui::Ui, m: &mut crate::modele::Materiau) {
    use crate::modele::{CarteTexture, Texture};
    let a = &mut m.apparence;
    egui::Grid::new("apparence_materiau").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label(RichText::new("Réglages").strong());
        ui.end_row();
        ui.label("Teinte");
        ui.horizontal(|ui| {
            let mut v = a.teinte.unwrap_or(m.couleur);
            if ui.color_edit_button_srgb(&mut v).on_hover_text("Multipliée à la texture de couleur ; par défaut la couleur du matériau").changed() {
                a.teinte = Some(v);
            }
            if a.teinte.is_some() && ui.small_button("Couleur du matériau").clicked() {
                a.teinte = None;
            }
        });
        ui.end_row();
        ui.label("Rugosité");
        ui.add(egui::Slider::new(&mut a.rugosite, 0.0..=1.0));
        ui.end_row();
        ui.label("Métallique");
        ui.add(egui::Slider::new(&mut a.metallique, 0.0..=1.0));
        ui.end_row();
        ui.label("Transparence");
        ui.add(egui::Slider::new(&mut a.transparence, 0.0..=1.0));
        ui.end_row();
        ui.label("Indice de réfraction");
        ui.add(egui::DragValue::new(&mut a.refraction).speed(0.01).range(1.0..=3.0));
        ui.end_row();
        ui.label("Émission");
        ui.add(egui::DragValue::new(&mut a.emission).speed(1.0).range(0.0..=100_000.0).suffix(" cd/m²"));
        ui.end_row();
    });
    ui.add_space(6.0);
    ui.label(RichText::new("Textures").strong());
    let mut retirer = None;
    for carte in CarteTexture::TOUTES {
        ui.push_id(carte, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized([130.0, 18.0], egui::Label::new(carte.libelle()));
                let nom = a.textures.get(&carte).map_or("Aucune".to_string(), |t| std::path::Path::new(&t.chemin).file_name().map_or(t.chemin.clone(), |n| n.to_string_lossy().into_owned()));
                let r = ui.add_sized([170.0, 18.0], egui::Button::new(nom).truncate());
                let r = match a.textures.get(&carte) { Some(t) => r.on_hover_text(&t.chemin), None => r.on_hover_text("Choisir une image") };
                if r.clicked()
                    && let Some(f) = rfd::FileDialog::new().set_title(carte.libelle()).add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga", "tif", "tiff", "exr", "hdr", "webp"]).pick_file()
                {
                    let chemin = f.to_string_lossy().into_owned();
                    match a.textures.get_mut(&carte) {
                        Some(t) => t.chemin = chemin,
                        None => { a.textures.insert(carte, Texture::nouvelle(chemin)); }
                    }
                }
                if a.textures.contains_key(&carte) && ui.small_button("✕").on_hover_text("Retirer la texture").clicked() {
                    retirer = Some(carte);
                }
            });
            if let Some(t) = a.textures.get_mut(&carte) {
                ui.indent("placement", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Taille");
                        ui.add(egui::DragValue::new(&mut t.largeur).speed(10.0).range(1.0..=100_000.0).suffix(" mm"));
                        ui.label("×");
                        ui.add(egui::DragValue::new(&mut t.hauteur).speed(10.0).range(1.0..=100_000.0).suffix(" mm"));
                        ui.label("Rotation");
                        ui.add(egui::DragValue::new(&mut t.rotation).speed(1.0).range(-360.0..=360.0).suffix(" °"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Décalage");
                        ui.add(egui::DragValue::new(&mut t.decalage[0]).speed(10.0).suffix(" mm"));
                        ui.add(egui::DragValue::new(&mut t.decalage[1]).speed(10.0).suffix(" mm"));
                        ui.label("Intensité");
                        ui.add(egui::Slider::new(&mut t.intensite, 0.0..=1.0));
                        ui.checkbox(&mut t.inverser, "Inverser");
                    });
                });
            }
        });
    }
    if let Some(c) = retirer {
        a.textures.remove(&c);
    }
    ui.label(RichText::new("Rendu dans le style Réaliste. Textures projetées sur le plan de chaque face, à leur taille réelle. Transparence et découpe : noir = transparent / retiré.").small().weak());
}
