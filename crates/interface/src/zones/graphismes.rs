//! Fenêtres matériau, plumes et motifs.
use crate::*;

impl Application {
    /// Graphismes d'un matériau du projet natif, comme dans Revit (sans ombrage) : motifs
    /// de surface et de coupe, chacun en premier plan et en arrière-plan (motif, couleur).
    /// L'arrière-plan est uni ou hachuré ; « Motifs… » ouvre l'éditeur de hachures.
    pub(crate) fn fenetre_materiau(&mut self, ctx: &egui::Context) {
        use modele::MotifCoupe;
        let Some(nom) = self.materiau_edite.clone() else { return };
        let Some((projet, chemin_projet)) = &self.natif else { self.materiau_edite = None; return };
        let dossier = chemin_projet.parent().map(|d| d.to_path_buf());
        let Some(mut m) = projet.materiau(&nom).cloned() else { self.materiau_edite = None; return };
        let avant = m.clone();
        let hachures: Vec<MotifCoupe> = projet.noms_motifs().iter().map(|n| MotifCoupe::hachures(n)).collect();
        // en coupe, motifs de dessin seulement (comme Revit)
        let de_dessin: Vec<MotifCoupe> = hachures.iter().filter(|x| projet.definition_motif(x).is_some_and(|d| d.genre == modele::GenreMotif::Dessin)).cloned().collect();
        let mut ouvert = true;
        let mut onglet_apparence = self.materiau_apparence;
        egui::Window::new(format!("Matériau · {nom}")).id(egui::Id::unique("fenetre_materiau")).open(&mut ouvert).resizable(false).collapsible(false).show(ctx, |ui| {
            let choix = |ui: &mut egui::Ui, id: &str, x: &mut MotifCoupe, speciaux: &[MotifCoupe]| {
                let liste = if id.starts_with("coupe") { &de_dessin } else { &hachures };
                egui::ComboBox::from_id_salt(id).width(130.0).selected_text(x.libelle().to_string()).show_ui(ui, |ui| {
                    for t in speciaux.iter().chain(liste.iter()) {
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
                apparence_materiau(ui, &mut m, dossier.clone());
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
            ui.label(RichText::new("Motifs : arborescence du projet › Motifs de hachures.").small().weak());
        });
        self.materiau_apparence = onglet_apparence;
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

    /// Fenêtre « Épaisseurs de traits » : épaisseur de chaque plume (mm du papier),
    /// aperçu, couches qui l'utilisent ; table du projet, ou celle du logiciel rétablie.
    pub(crate) fn fenetre_plumes(&mut self, ctx: &egui::Context) {
        let Some(mut table) = self.editeur_plumes.take() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let defaut = modele::Projet::default().epaisseurs_plumes();
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
                if ui.add_enabled(table.iter().zip(&defaut).any(|(a, b)| (a - b).abs() > 1e-9), egui::Button::new("Valeurs par défaut"))
                    .on_hover_text("Reprendre les épaisseurs par défaut du logiciel (Appliquer pour valider)")
                    .clicked()
                {
                    table = defaut.clone();
                }
                if ui.button("Fermer").clicked() {
                    fermer = true;
                }
            });
        });
        if appliquer {
            let mut p = projet.clone();
            // le projet porte sa table (le gabarit n'est plus relu)
            p.plumes = table.clone();
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

}

/// Onglet Apparence de la fenêtre Matériau : réglages physiques et cartes de texture
/// (image, taille réelle, décalage, rotation, intensité).
fn apparence_materiau(ui: &mut egui::Ui, m: &mut modele::Materiau, dossier: Option<std::path::PathBuf>) {
    use modele::{CarteTexture, Texture};
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
        ui.label("Herbe").on_hover_text("Brins en volume au-dessus de la surface (style Réaliste, près de la caméra) ; 0 : surface plate");
        ui.add(egui::DragValue::new(&mut a.herbe).speed(1.0).range(0.0..=300.0).suffix(" mm"));
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
                    // dans le dossier du projet : chemin relatif (le projet reste déplaçable)
                    let chemin = match dossier.as_deref().and_then(|d| f.strip_prefix(d).ok()) {
                        Some(r) => r.to_string_lossy().replace('\\', "/"),
                        None => f.to_string_lossy().into_owned(),
                    };
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
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut t.anti_repetition, "Anti-répétition").on_hover_text("Casse le motif répété au loin (deux échantillonnages tournés, mélangés)");
                        if carte == CarteTexture::Relief {
                            ui.label("Profondeur").on_hover_text("Parallaxe : profondeur du relief pour toute la plage de gris");
                            ui.add(egui::DragValue::new(&mut t.profondeur).speed(0.5).range(0.0..=200.0).suffix(" mm"));
                        }
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
