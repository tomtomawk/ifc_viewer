//! Édition des familles de profils plans.
use super::*;
use modele::famille::profil::{FichierProfil, ParametreProfil, PointProfil, Profil};

use super::familles_planes::FamillePlane;

impl ViewerApp {
    pub(super) fn ouvrir_profil(&mut self, id: Option<&str>) {
        if let Some((fichier, id)) = id.and_then(|id| self.natif.as_ref()?.0.profils.iter().find(|p| p.id == id)).and_then(|p| p.document_famille.as_ref().map(|f| ((**f).clone(), p.id.clone()))) {
            self.ouvrir_famille_document_projet(fichier, Some(id));
            return;
        }
        let projet_vide = modele::Projet::nouveau("Famille");
        let projet = self.natif.as_ref().map(|(p, _)| p).unwrap_or(&projet_vide);
        let profil = if let Some(id) = id {
            let Some(p) = projet.profils.iter().find(|p| p.id == id) else { return };
            p.clone()
        } else {
            let Some(mut p) = modele::gabarit::actif().profils.first().cloned().or_else(|| modele::gabarit::Gabarit::defaut().profils.first().cloned()) else { return };
            p.id.clear();
            p.nom = (1..).map(|n| format!("Profil {n}")).find(|nom| !projet.profils.iter().any(|x| x.nom.eq_ignore_ascii_case(nom))).unwrap();
            p
        };
        if let Some(mut document) = profil.document_famille.clone() {
            document.nom = profil.nom.clone();
            self.ouvrir_famille_document_projet(*document, id.map(str::to_string));
            return;
        }
        self.ouvrir_famille_plane(FamillePlane::Profil { profil, original: id.map(str::to_string) });
    }

    pub(super) fn charger_profil_fichier(&mut self) {
        let Some(chemin) = rfd::FileDialog::new().add_filter("Profil BIMedit", &["json"]).pick_file() else { return };
        let resultat = std::fs::read_to_string(&chemin).map_err(|e| e.to_string()).and_then(|s| FichierProfil::lire(&s));
        match resultat {
            Ok(mut profil) => {
                profil.id.clear();
                if let Some(mut document) = profil.document_famille.take() {
                    document.nom = profil.nom;
                    self.ouvrir_famille_document_projet(*document, None);
                } else {
                    self.ouvrir_famille_plane(FamillePlane::Profil { profil, original: None });
                }
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(super) fn enregistrer_profil(&mut self, profil: &mut Profil, original: Option<&str>, copie: bool) -> Result<(), String> {
        let (projet, _) = self.natif.as_ref().ok_or("Aucun projet ouvert.")?;
        let mut projet = projet.clone();
        profil.nom = profil.nom.trim().to_string();
        profil.valider()?;
        if projet.profils.iter().any(|p| p.nom.eq_ignore_ascii_case(&profil.nom) && (copie || Some(p.id.as_str()) != original)) {
            return Err("Un profil porte déjà ce nom. Choisissez un nom distinct.".into());
        }
        if copie || original.is_none() {
            profil.id = noyau::uuid::uuid();
            projet.profils.push(profil.clone());
        } else {
            let p = projet.profils.iter_mut().find(|p| Some(p.id.as_str()) == original).ok_or("Profil introuvable dans le projet.")?;
            *p = profil.clone();
        }
        self.appliquer_projet(projet)
    }
}

pub(super) fn exporter_profil(profil: &Profil) -> Result<(), String> {
    profil.valider()?;
    if let Some(chemin) = rfd::FileDialog::new().set_file_name("profil.profil.json").add_filter("Profil BIMedit", &["json"]).save_file() {
        let f = FichierProfil { format: "bimedit.profil".into(), profil: profil.clone() };
        let texte = serde_json::to_string_pretty(&f).map_err(|e| e.to_string())?;
        std::fs::write(chemin, texte).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(super) fn proprietes_profil(ui: &mut egui::Ui, profil: &mut Profil) {
        ui.heading("Paramètres");
        egui::ScrollArea::vertical().id_salt("profil_parametres").max_height(180.0).show(ui, |ui| {
            let mut retirer = None;
            for (i, p) in profil.parametres.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut p.nom).desired_width(95.0));
                    ui.label("=");
                    ui.add(egui::TextEdit::singleline(&mut p.formule).desired_width(80.0));
                    if ui.small_button("−").on_hover_text("Supprimer le paramètre").clicked() { retirer = Some(i); }
                });
            }
            if let Some(i) = retirer { profil.parametres.remove(i); }
            if ui.small_button("+ Paramètre").clicked() {
                let nom = (1..).map(|n| format!("Parametre{n}")).find(|nom| !profil.parametres.iter().any(|p| p.nom == *nom)).unwrap();
                profil.parametres.push(ParametreProfil { nom, formule: "10".into() });
            }
        });
        ui.separator();
        ui.heading("Sommets du contour");
        ui.label(RichText::new("X et Y, dans l'ordre du contour fermé").small().weak());
        egui::ScrollArea::vertical().id_salt("profil_sommets").max_height(230.0).show(ui, |ui| {
            let mut retirer = None;
            for (i, p) in profil.contour.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}", i + 1));
                    ui.add(egui::TextEdit::singleline(&mut p.x).desired_width(90.0));
                    ui.add(egui::TextEdit::singleline(&mut p.y).desired_width(90.0));
                    if ui.small_button("−").on_hover_text("Supprimer le sommet").clicked() { retirer = Some(i); }
                });
            }
            if let Some(i) = retirer { profil.contour.remove(i); }
            if ui.small_button("+ Sommet").clicked() { profil.contour.push(PointProfil { x: "0".into(), y: "0".into() }); }
        });
}

pub(super) fn dessiner_profil(ui: &mut egui::Ui, profil: &mut Profil) {
        ui.heading("Aperçu");
        let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(100.0), 300.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(r, 2.0, ui.visuals().extreme_bg_color);
        match profil.points() {
            Ok(points) => {
                let minx = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
                let maxx = points.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
                let miny = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
                let maxy = points.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
                let s = ((r.width()-30.0) as f64 / (maxx-minx).max(1e-6)).min((r.height()-30.0) as f64 / (maxy-miny).max(1e-6));
                let pts: Vec<_> = points.iter().map(|p| egui::pos2(r.center().x + ((p[0]-(minx+maxx)/2.0)*s) as f32, r.center().y - ((p[1]-(miny+maxy)/2.0)*s) as f32)).collect();
                painter.add(egui::Shape::closed_line(pts.clone(), egui::Stroke::new(2.0, Color32::from_rgb(40, 120, 200))));
                for p in pts { painter.circle_filled(p, 3.0, Color32::from_rgb(40, 120, 200)); }
            }
            Err(e) => { ui.colored_label(ui.visuals().error_fg_color, e); }
        }
}
