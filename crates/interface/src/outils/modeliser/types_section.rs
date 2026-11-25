//! Types de poteaux et de poutres (profil ou rectangle, dimensions, matériau) : sélecteur
//! commun des palettes de création, choix du type d'un élément, type par défaut.
use crate::*;
use modele::elements::poutre::TypeSection;
use modele::GenreBarre;

fn sorte(genre: GenreBarre) -> &'static str {
    match genre { GenreBarre::Poteau => "Poteau", GenreBarre::Poutre => "Poutre" }
}

/// Type proposé quand le projet n'en a aucun de cette sorte.
pub(crate) fn type_par_defaut(genre: GenreBarre) -> TypeSection {
    let (l, h) = match genre { GenreBarre::Poteau => (300.0, 300.0), GenreBarre::Poutre => (200.0, 400.0) };
    TypeSection { id: format!("type-{}-{}", sorte(genre).to_lowercase(), noyau::uuid::uuid()), nom: format!("{} {l} × {h}", sorte(genre)), profil: None, largeur: l, hauteur: h, materiau: None }
}

/// Liste déroulante des types de la sorte de `b` (propriétés d'un élément) ; vrai si le
/// type a changé.
pub(crate) fn choix_type_section(ui: &mut egui::Ui, p: &modele::Projet, genre: GenreBarre, type_barre: &mut String) -> bool {
    let types = p.types_section(genre);
    let nom = types.iter().find(|t| &t.id == type_barre).map_or("Choisir un type", |t| t.nom.as_str());
    let mut change = false;
    egui::ComboBox::from_id_salt(("type_section", sorte(genre))).selected_text(nom).show_ui(ui, |ui| {
        for t in types {
            change |= ui.selectable_value(type_barre, t.id.clone(), &t.nom).changed();
        }
    });
    change
}

impl Application {
    /// Type choisi pour les prochains éléments de la sorte `genre`.
    fn type_section_choisi(&mut self, genre: GenreBarre) -> &mut String {
        match genre { GenreBarre::Poteau => &mut self.bim.barre.type_barre, GenreBarre::Poutre => &mut self.bim.poutre.type_barre }
    }

    /// Emplacement « sélecteur » des poteaux et des poutres : types du projet (détail :
    /// profil ou section), Dupliquer, nouveau type ; champs du type choisi. Un projet sans
    /// type de cette sorte en reçoit un par défaut.
    pub(crate) fn selecteur_section(&mut self, ui: &mut egui::Ui, genre: GenreBarre) {
        use crate::zones::ruban::selecteur;
        let Some((p, _)) = &self.natif else { return };
        let (types, profils, materiaux) = (p.types_section(genre).to_vec(), p.profils.clone(), p.materiaux.iter().map(|m| m.nom.clone()).collect::<Vec<_>>());
        let mut nouveau: Option<TypeSection> = types.is_empty().then(|| type_par_defaut(genre));
        let choisi = self.type_section_choisi(genre);
        if !types.iter().any(|t| &t.id == choisi) {
            *choisi = types.first().map(|t| t.id.clone()).unwrap_or_default();
        }
        let choisi = choisi.clone();
        let u = modele::unite::unite();
        let details: Vec<String> = types.iter().map(|t| {
            let forme = t.profil.as_ref().and_then(|id| profils.iter().find(|x| &x.id == id)).map_or("Rectangulaire", |x| x.nom.as_str());
            format!("{forme} · {} × {}", u.afficher(t.largeur), u.afficher(t.hauteur))
        }).collect();
        let actions: &[(selecteur::ActionType, &str)] = &[(selecteur::ActionType::Dupliquer, "Dupliquer : nouveau type d'après celui-ci")];
        let lignes: Vec<selecteur::Ligne> = types.iter().zip(&details).map(|(t, d)| selecteur::Ligne { nom: &t.nom, courant: t.id == choisi, etiquette: None, sous_titre: None, detail: Some(d.clone()), actions }).collect();
        let sections: Vec<(f64, f64)> = types.iter().map(|t| (t.largeur, t.hauteur)).collect();
        let vignette = |pe: &egui::Painter, z: egui::Rect, k: usize, c: Color32| crate::zones::ruban::vignettes::poutre(pe, z, sections[k].0, sections[k].1, c);
        let courant = types.iter().find(|t| t.id == choisi);
        let titre = format!("{} · {}", sorte(genre), courant.map_or("aucun type", |t| t.nom.as_str()));
        let choix = selecteur::simple(ui, &titre, None, &format!("{titre}\nType de {} : profil, dimensions, matériau.", sorte(genre).to_lowercase()), &lignes, Some("+ Nouveau type"), Some(&vignette));
        if let Some(k) = choix.ligne { *self.type_section_choisi(genre) = types[k].id.clone(); }
        let copie = |base: &TypeSection| {
            let nom = (2..).map(|n| format!("{} {n}", base.nom)).find(|n| types.iter().all(|t| &t.nom != n)).unwrap_or_default();
            TypeSection { id: format!("type-{}-{}", sorte(genre).to_lowercase(), noyau::uuid::uuid()), nom, ..base.clone() }
        };
        if let Some((k, _)) = choix.action { nouveau = Some(copie(&types[k])); }
        if choix.ajout { nouveau = Some(copie(&type_par_defaut(genre))); }
        // champs du type choisi (tous les éléments de ce type changent)
        if let Some(mut t) = courant.cloned() {
            let avant = t.clone();
            egui::Grid::new(("champs_type_section", sorte(genre))).num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Nom");
                ui.text_edit_singleline(&mut t.nom);
                ui.end_row();
                ui.label("Profil");
                egui::ComboBox::from_id_salt(("profil_section", sorte(genre))).selected_text(t.profil.as_ref().and_then(|id| profils.iter().find(|x| &x.id == id)).map_or("Rectangulaire", |x| x.nom.as_str())).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.profil, None, "Rectangulaire");
                    for x in &profils { ui.selectable_value(&mut t.profil, Some(x.id.clone()), &x.nom); }
                });
                ui.end_row();
                ui.label("Largeur");
                ui.add(egui::DragValue::new(&mut t.largeur).range(10.0..=5000.0).dimensions());
                ui.end_row();
                ui.label(if genre == GenreBarre::Poteau { "Profondeur" } else { "Hauteur" });
                ui.add(egui::DragValue::new(&mut t.hauteur).range(10.0..=5000.0).dimensions());
                ui.end_row();
                ui.label("Matériau");
                egui::ComboBox::from_id_salt(("materiau_section", sorte(genre))).selected_text(t.materiau.clone().unwrap_or_else(|| "Par défaut".into())).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.materiau, None, "Par défaut");
                    for m in &materiaux { ui.selectable_value(&mut t.materiau, Some(m.clone()), m); }
                });
                ui.end_row();
            });
            if t != avant { nouveau = Some(t); }
        }
        if let Some(t) = nouveau && let Some((projet, _)) = self.natif.as_ref() {
            let mut q = projet.clone();
            let liste = match genre { GenreBarre::Poteau => &mut q.types_poteaux, GenreBarre::Poutre => &mut q.types_poutres };
            match liste.iter_mut().find(|x| x.id == t.id) {
                Some(x) => *x = t.clone(),
                None => liste.push(t.clone()),
            }
            match self.appliquer_projet(q) {
                Ok(()) => *self.type_section_choisi(genre) = t.id,
                Err(e) => self.error = Some(e),
            }
        }
    }
}
