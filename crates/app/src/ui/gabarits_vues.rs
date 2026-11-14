//! Éditeur des gabarits de vue du projet.
use super::*;
use modele::gabarits_vues::GabaritVue;

pub(super) struct EditeurGabaritVue {
    vue: String,
    gabarit: GabaritVue,
    nouveau: bool,
    erreur: Option<String>,
}

impl ViewerApp {
    pub(super) fn vue_gabaritee(&self) -> bool {
        self.id_vue_contexte().and_then(|id| self.natif.as_ref()?.0.gabarit_vue(&id)).is_some()
    }
    pub(super) fn ouvrir_editeur_gabarit_vue(&mut self, vue: &str, editer: bool) {
        let Some((projet, _)) = &self.natif else { return };
        let existant = projet.gabarit_vue(vue).cloned();
        if editer && existant.is_none() { return; }
        let gabarit = if editer { existant.unwrap() } else {
            let contexte = projet.contextes_vues.get(vue).cloned().unwrap_or_default();
            let mut nom = "Gabarit de vue".to_string();
            let mut n = 2;
            while projet.gabarits_vues.iter().any(|g| g.nom.eq_ignore_ascii_case(&nom)) { nom = format!("Gabarit de vue {n}"); n += 1; }
            GabaritVue { id: String::new(), nom, echelle: self.echelle, poche: self.poche.nom().into(), phase: contexte.phase, variante: contexte.variante }
        };
        self.editeur_gabarit_vue = Some(EditeurGabaritVue { vue: vue.into(), gabarit, nouveau: !editer, erreur: None });
    }

    pub(super) fn ouvrir_gabarit_par_id(&mut self, vue: &str, id: &str) {
        let Some(gabarit) = self.natif.as_ref().and_then(|(p, _)| p.gabarits_vues.iter().find(|g| g.id == id)).cloned() else { return };
        self.editeur_gabarit_vue = Some(EditeurGabaritVue { vue: vue.into(), gabarit, nouveau: false, erreur: None });
    }

    /// Applique immédiatement les paramètres pilotés à la vue ouverte et à son état
    /// mémorisé, sans changer la caméra, les catégories ou le cadrage.
    pub(super) fn actualiser_gabarit_vue_actif(&mut self) {
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

    pub(super) fn fenetre_gabarit_vue(&mut self, ctx: &egui::Context) {
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
