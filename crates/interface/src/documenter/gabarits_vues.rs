//! Éditeur des gabarits de vue du projet.
use crate::*;
use modele::gabarits_vues::GabaritVue;

pub(crate) struct EditeurGabaritVue {
    vue: String,
    gabarit: GabaritVue,
    nouveau: bool,
    erreur: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dupliquer_attribue_la_copie_a_la_vue_courante() {
        let mut projet = modele::Projet::nouveau("Gabarits");
        projet.niveaux.push(modele::Niveau {
            id: "rdc".into(),
            nom: "RDC".into(),
            altitude: 0.0,
            origine: None,
        });
        let vue = projet.ajouter_vue_plan("rdc", 1200.0).unwrap();
        let source = GabaritVue {
            id: "plan".into(),
            nom: "Plan détaillé".into(),
            echelle: 50.0,
            poche: "plein".into(),
            phase: Some("Projet".into()),
            variante: None,
        };
        projet.enregistrer_gabarit_vue(source.clone()).unwrap();
        projet.affecter_gabarit_vue("3d", Some(&source.id)).unwrap();
        projet.affecter_gabarit_vue(&vue, Some(&source.id)).unwrap();
        projet.enregistrer_gabarit_vue(GabaritVue {
            id: "copie".into(),
            nom: "Plan détaillé (COPIE)".into(),
            ..source.clone()
        }).unwrap();

        let ctx = egui::Context::default();
        let mut app = ViewerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        let chemin = std::env::temp_dir().join(format!("gabarit-{}.modele.json", noyau::uuid::uuid()));
        app.natif = Some((projet, chemin.clone()));
        app.ouvrir_gabarit_par_id(&vue, &source.id, true);
        let brouillon = app.editeur_gabarit_vue.as_ref().unwrap();
        assert!(brouillon.gabarit.id.is_empty());
        assert_eq!(brouillon.gabarit.nom, "Plan détaillé (copie 2)");

        let mut image = |events| { let mut s = ctx.run_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
            events,
            ..Default::default()
        }, |ui| app.fenetre_gabarit_vue(ui.ctx())); s.textures_delta.clear(); s };
        image(vec![]);
        let sortie = image(vec![]);
        let bouton = sortie.shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Créer et attribuer" => Some(t.pos + t.galley.size() / 2.0),
            _ => None,
        }).expect("Bouton de confirmation absent");
        image(vec![egui::Event::PointerMoved(bouton)]);
        for pressed in [true, false] {
            image(vec![egui::Event::PointerButton {
                pos: bouton, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
            }]);
        }
        echanges::enregistrement::effacer_recuperation(&chemin);

        let projet = &app.natif.as_ref().unwrap().0;
        assert_eq!(projet.gabarit_vue("3d"), Some(&source));
        let copie = projet.gabarit_vue(&vue).unwrap();
        assert_ne!(copie.id, source.id);
        assert_eq!(copie.nom, "Plan détaillé (copie 2)");
        assert_eq!((copie.echelle, &copie.poche, &copie.phase, &copie.variante),
            (source.echelle, &source.poche, &source.phase, &source.variante));
        assert_eq!(projet.gabarits_vues.len(), 3);
    }
}

impl ViewerApp {
    pub(crate) fn vue_gabaritee(&self) -> bool {
        self.id_vue_contexte().and_then(|id| self.natif.as_ref()?.0.gabarit_vue(&id)).is_some()
    }
    pub(crate) fn ouvrir_editeur_gabarit_vue(&mut self, vue: &str) {
        let Some((projet, _)) = &self.natif else { return };
        let contexte = projet.contextes_vues.get(vue).cloned().unwrap_or_default();
        let mut nom = "Gabarit de vue".to_string();
        let mut n = 2;
        while projet.gabarits_vues.iter().any(|g| g.nom.eq_ignore_ascii_case(&nom)) { nom = format!("Gabarit de vue {n}"); n += 1; }
        let gabarit = GabaritVue { id: String::new(), nom, echelle: self.echelle, poche: self.poche.nom().into(), phase: contexte.phase, variante: contexte.variante };
        self.editeur_gabarit_vue = Some(EditeurGabaritVue { vue: vue.into(), gabarit, nouveau: true, erreur: None });
    }

    pub(crate) fn ouvrir_gabarit_par_id(&mut self, vue: &str, id: &str, dupliquer: bool) {
        let Some((projet, _)) = &self.natif else { return };
        let Some(mut gabarit) = projet.gabarits_vues.iter().find(|g| g.id == id).cloned() else { return };
        if dupliquer {
            let nom_source = gabarit.nom.clone();
            let mut n = 2;
            gabarit.nom = format!("{nom_source} (copie)");
            while projet.gabarits_vues.iter().any(|g| g.nom.eq_ignore_ascii_case(&gabarit.nom)) {
                gabarit.nom = format!("{nom_source} (copie {n})");
                n += 1;
            }
            gabarit.id.clear();
        }
        self.editeur_gabarit_vue = Some(EditeurGabaritVue { vue: vue.into(), gabarit, nouveau: dupliquer, erreur: None });
    }

    /// Applique immédiatement les paramètres pilotés à la vue ouverte et à son état
    /// mémorisé, sans changer la caméra, les catégories ou le cadrage.
    pub(crate) fn actualiser_gabarit_vue_actif(&mut self) {
        let Some(id) = self.id_vue_contexte() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let parametres = projet.gabarit_vue(&id).map(|g| (g.echelle, Some(g.poche.as_str())))
            .or_else(|| (id == "3d").then(|| projet.echelle_vue_3d.map(|e| (e, projet.poche_vue_3d.as_deref()))).flatten())
            .or_else(|| projet.vues.iter().find(|v| v.id == id).map(|v| (v.echelle, v.poche_vue.as_deref())));
        let Some((echelle, poche)) = parametres else { return };
        self.echelle = echelle;
        if let Some(poche) = poche.and_then(Poche::depuis) { self.poche = poche; }
        if let Some(etat) = self.etats_vues.get_mut(&self.vue_active) { etat.echelle = self.echelle; etat.poche = self.poche; }
        self.visibility_dirty = true;
        self.coupe_dirty = true;
    }

    pub(crate) fn fenetre_gabarit_vue(&mut self, ctx: &egui::Context) {
        let Some(mut editeur) = self.editeur_gabarit_vue.take() else { return };
        let Some((projet, _)) = &self.natif else { return };
        if editeur.vue != "3d" && !projet.vues.iter().any(|v| v.id == editeur.vue) { return; }
        let mut ouvert = true;
        let (mut enregistrer, mut supprimer, mut annuler) = (false, false, false);
        egui::Window::new(if editeur.nouveau { "Créer un gabarit de vue" } else { "Modifier le gabarit de vue" })
            .id(egui::Id::unique("editeur_gabarit_vue"))
            .default_width(380.0).collapsible(false).open(&mut ouvert).show(ctx, |ui| {
                ui.label("Ce gabarit pilote l'échelle, le poché, la phase et la variante des vues auxquelles il est attribué.");
                ui.add_space(6.0);
                egui::Grid::new("parametres_gabarit_vue").num_columns(2).striped(true).spacing([12.0, 7.0]).show(ui, |ui| {
                    ui.label("Nom");
                    ui.add(egui::TextEdit::singleline(&mut editeur.gabarit.nom).desired_width(200.0));
                    ui.end_row();
                    ui.label("Échelle");
                    ui.add(egui::DragValue::new(&mut editeur.gabarit.echelle).speed(1.0).range(1.0..=10000.0).prefix("1 : "));
                    ui.end_row();
                    ui.label("Poché de coupe");
                    egui::ComboBox::from_id_salt("gabarit_poche").selected_text(Poche::depuis(&editeur.gabarit.poche).map_or("Hachuré", |p| p.libelle())).show_ui(ui, |ui| {
                        for p in [Poche::Hachure, Poche::Plein, Poche::Vide, Poche::Materiau] { ui.selectable_value(&mut editeur.gabarit.poche, p.nom().into(), p.libelle()); }
                    });
                    ui.end_row();
                    ui.label("Phase affichée");
                    egui::ComboBox::from_id_salt("gabarit_phase").selected_text(editeur.gabarit.phase.as_deref().unwrap_or("Toutes")).show_ui(ui, |ui| {
                        ui.selectable_value(&mut editeur.gabarit.phase, None, "Toutes");
                        for phase in &projet.phases { ui.selectable_value(&mut editeur.gabarit.phase, Some(phase.clone()), phase); }
                    });
                    ui.end_row();
                    ui.label("Variante affichée");
                    let nom = editeur.gabarit.variante.as_ref().and_then(|id| projet.variantes.iter().find(|v| &v.id == id)).map_or("Toutes".into(), |v| v.libelle());
                    egui::ComboBox::from_id_salt("gabarit_variante").selected_text(nom).show_ui(ui, |ui| {
                        ui.selectable_value(&mut editeur.gabarit.variante, None, "Toutes");
                        for variante in &projet.variantes { ui.selectable_value(&mut editeur.gabarit.variante, Some(variante.id.clone()), variante.libelle()); }
                    });
                    ui.end_row();
                });
                if let Some(erreur) = &editeur.erreur { ui.colored_label(ui.visuals().error_fg_color, erreur); }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(if editeur.nouveau { "Créer et attribuer" } else { "Enregistrer" }).clicked() { enregistrer = true; }
                    if !editeur.nouveau && ui.button("Supprimer le gabarit").on_hover_text("Détache toutes les vues en conservant leurs réglages actuels.").clicked() { supprimer = true; }
                    if ui.button("Annuler").clicked() { annuler = true; }
                });
            });
        if enregistrer || supprimer {
            let mut copie = projet.clone();
            let resultat = if supprimer {
                copie.supprimer_gabarit_vue(&editeur.gabarit.id)
            } else {
                copie.enregistrer_gabarit_vue(editeur.gabarit.clone()).and_then(|id| {
                    if editeur.nouveau { copie.affecter_gabarit_vue(&editeur.vue, Some(&id))?; }
                    Ok(())
                })
            };
            match resultat.and_then(|()| self.appliquer_projet(copie)) {
                Ok(()) => {
                    self.actualiser_gabarit_vue_actif();
                    self.status = if supprimer { "Gabarit supprimé ; les vues conservent leurs réglages.".into() } else { "Gabarit de vue enregistré.".into() };
                    return;
                }
                Err(e) => editeur.erreur = Some(e),
            }
        }
        if ouvert && !annuler { self.editeur_gabarit_vue = Some(editeur); }
    }
}
