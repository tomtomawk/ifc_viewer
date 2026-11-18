//! Gestion du plan de construction actif dans Propriétés : renommer, supprimer (refusé
//! tant que des formes y sont esquissées) ou détacher en repère fixe équivalent.
use super::*;

/// Ligne de gestion du plan de construction actif.
pub(super) fn gestion_plan_construction(ui: &mut egui::Ui, ed: &mut EditeurFamille) {
    let Some(plan) = ed.construction.and_then(|k| ed.famille.plans_construction.get(k)).cloned() else { return };
    ui.horizontal(|ui| {
        ui.label("Nom");
        // saisie tamponnée : le nom est appliqué à la validation (Entrée ou perte du focus)
        let cle = egui::Id::unique(("nom_plan_construction", plan.id.as_str()));
        let mut nom = ui.data_mut(|d| d.get_temp::<String>(cle)).unwrap_or_else(|| plan.nom.clone());
        let reponse = ui.text_edit_singleline(&mut nom);
        if reponse.lost_focus() {
            ui.data_mut(|d| d.remove::<String>(cle));
            if nom != plan.nom && let Err(e) = ed.famille.renommer_plan_construction(&plan.id, &nom) { ed.erreur = Some(e); }
        } else if reponse.has_focus() {
            ui.data_mut(|d| d.insert_temp(cle, nom));
        }
    });
    let formes = ed.famille.formes_sur_construction(&plan.id);
    match &plan.reference {
        Some(reference) => ui.label(format!("Suit le plan de référence « {reference} » ; {} forme(s).", formes.len())),
        None => ui.label(format!("Repère fixe ; {} forme(s).", formes.len())),
    };
    ui.horizontal(|ui| {
        if ui.button("Supprimer").on_hover_text("Refusé tant que des formes sont esquissées sur ce plan").clicked() {
            supprimer_plan_construction(ed, &plan.id);
        }
        let detacher = plan.reference.is_some() || formes.is_empty();
        if ui.add_enabled(detacher, egui::Button::new("Détacher"))
            .on_hover_text("Les formes gardent leur position, ancrées sur un repère fixe équivalent ; le plan de référence est libéré")
            .clicked()
        {
            detacher_plan_construction(ed, &plan.id);
        }
    });
}

/// Suppression du plan de construction `id` ; le refus liste les formes portées.
pub(super) fn supprimer_plan_construction(ed: &mut EditeurFamille, id: &str) {
    match ed.famille.supprimer_plan_construction(id) {
        Ok(plan) => {
            ed.construction = None;
            ed.erreur = None;
            ed.message = Some(format!("Plan de construction « {} » supprimé — Ctrl+Z pour annuler.", plan.nom));
        }
        Err(e) => ed.erreur = Some(e),
    }
}

/// Détachement du plan de construction `id` en repère fixe à sa position courante.
pub(super) fn detacher_plan_construction(ed: &mut EditeurFamille, id: &str) {
    let r = ed.resoudre();
    match ed.famille.detacher_plan_construction(id, &r) {
        Ok(Some(nom)) => {
            ed.erreur = None;
            ed.message = Some(format!("Plan détaché : repère fixe « {nom} » — Ctrl+Z pour annuler."));
        }
        Ok(None) => {
            ed.construction = None;
            ed.erreur = None;
            ed.message = Some("Plan de construction sans forme supprimé.".into());
        }
        Err(e) => ed.erreur = Some(e),
    }
}
