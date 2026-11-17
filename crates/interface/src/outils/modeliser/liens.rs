//! Liens IFC : Modéliser > Lien IFC, fichier IFC déposé dans la fenêtre, rubrique
//! « Liens » de l'arborescence (visibilité, position, rotation, recharger, changer de
//! fichier, retirer). Un fichier IFC ne s'ouvre plus seul : il se lie à un projet natif
//! (ou se convertit par Début > Importer).
use crate::*;
use modele::liens::LienIfc;
use std::path::Path;

/// Message d'un fichier IFC qu'on tente d'ouvrir.
pub(crate) const IFC_NON_OUVERT: &str = "Un fichier IFC ne s'ouvre pas seul : dans un projet natif, Modéliser > Lien IFC (ou glisser le fichier dans la fenêtre) ; Début > Importer pour le convertir en projet natif.";

pub(crate) fn est_ifc(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ifc") || e.eq_ignore_ascii_case("ifczip"))
}

impl ViewerApp {
    /// Modéliser > Lien IFC : choix du fichier, puis lecture en arrière-plan.
    pub(crate) fn inserer_lien_dialogue(&mut self, ctx: &egui::Context) {
        if self.natif.is_none() {
            self.error = Some("Lien IFC : ouvrir ou créer d'abord un projet natif.".into());
            return;
        }
        let Some(chemin) = rfd::FileDialog::new().set_title("Insérer un lien IFC").add_filter("IFC", &["ifc", "IFC"]).pick_file() else { return };
        self.inserer_lien(ctx, chemin);
    }

    /// Lie `chemin` au projet natif actif, à l'origine, sans rotation. Le fichier est lu
    /// en arrière-plan (étape et Annuler dans la barre haute) ; le lien est ajouté à la
    /// fin, en une transaction annulable.
    pub(crate) fn inserer_lien(&mut self, ctx: &egui::Context, chemin: PathBuf) {
        if self.natif.is_none() {
            self.error = Some(IFC_NON_OUVERT.into());
            return;
        }
        let chemin = chemin.canonicalize().unwrap_or(chemin);
        let nom = chemin.file_stem().map_or_else(|| "Lien".into(), |n| n.to_string_lossy().into_owned());
        let lien = LienIfc { id: String::new(), nom, chemin: chemin.to_string_lossy().into_owned(), decalage: [0.0; 3], rotation: 0.0, visible: true };
        self.lier_ifc_arriere_plan(ctx, lien);
    }

    /// Rubrique « Liens » d'un projet natif.
    pub(crate) fn liens_ui(&mut self, ui: &mut egui::Ui) {
        let Some((p, _)) = &self.natif else { return };
        let liens = p.liens.clone();
        if liens.is_empty() {
            ui.label(RichText::new("Aucun lien. Modéliser > Lien IFC, ou glisser un fichier IFC dans la fenêtre.").weak());
        }
        let (mut commande, mut recharger, mut changer) = (None, false, None);
        for l in &liens {
            let absent = !Path::new(&l.chemin).is_file();
            let titre = if absent { RichText::new(format!("{} (fichier introuvable)", l.nom)).color(Color32::from_rgb(185, 35, 35)) } else { RichText::new(&l.nom) };
            egui::CollapsingHeader::new(titre).id_salt(("lien", &l.id)).show(ui, |ui| {
                let mut v = l.clone();
                let mut valider = ui.checkbox(&mut v.visible, "Visible").changed();
                ui.label(RichText::new(&l.chemin).small().weak()).on_hover_text(&l.chemin);
                egui::Grid::new(("position_lien", &l.id)).num_columns(2).show(ui, |ui| {
                    for (i, axe) in ["X", "Y", "Z"].into_iter().enumerate() {
                        ui.label(axe);
                        let mut m = v.decalage[i] / 1000.0;
                        let r = ui.add(egui::DragValue::new(&mut m).speed(0.05).suffix(" m").max_decimals(3));
                        v.decalage[i] = m * 1000.0;
                        valider |= r.drag_stopped() || r.lost_focus();
                        ui.end_row();
                    }
                    ui.label("Rotation");
                    let r = ui.add(egui::DragValue::new(&mut v.rotation).speed(0.5).range(-360.0..=360.0).suffix(" °").max_decimals(2));
                    valider |= r.drag_stopped() || r.lost_focus();
                    ui.end_row();
                }).response.on_hover_text("Position du fichier lié dans le projet : rotation autour de son origine, puis décalage");
                ui.horizontal(|ui| {
                    if ui.button("Recharger").on_hover_text("Relire le fichier s'il a changé sur le disque").clicked() { recharger = true; }
                    if ui.button("Changer de fichier…").clicked() { changer = Some(l.clone()); }
                    if ui.button("Retirer").on_hover_text("Retire le lien du projet ; le fichier IFC n'est pas touché").clicked() {
                        commande = Some(commandes::Commande::Supprimer { ids: vec![l.id.clone()] });
                    }
                });
                if valider && v != *l { commande = Some(commandes::Commande::Lien { lien: v }); }
            });
        }
        if let Some(c) = commande && let Err(e) = self.executer_bim(c) { self.error = Some(e); }
        if recharger { self.recharger_liens(); }
        if let Some(l) = changer && let Some(chemin) = rfd::FileDialog::new().set_title(format!("Nouveau fichier du lien « {} »", l.nom)).add_filter("IFC", &["ifc", "IFC"]).pick_file() {
            let chemin = chemin.canonicalize().unwrap_or(chemin);
            self.lier_ifc_arriere_plan(ui.ctx(), LienIfc { chemin: chemin.to_string_lossy().into_owned(), ..l });
        }
    }

    /// Reconstruit la scène : les fichiers liés modifiés sur le disque sont relus. Le
    /// projet ne change pas (aucune étape d'historique).
    pub(crate) fn recharger_liens(&mut self) {
        let Some((p, _)) = &self.natif else { return };
        let p = p.clone();
        let n = p.liens.len();
        match self.appliquer_projet_interne(p, true) {
            Ok(()) => self.status = format!("{n} lien(s) rechargé(s)."),
            Err(e) => self.error = Some(e),
        }
    }
}
