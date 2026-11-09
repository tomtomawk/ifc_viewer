//! Édition des familles de profils plans.
use super::*;
use crate::modele::famille::profil::{FichierProfil, ParametreProfil, PointProfil, Profil};

pub(super) struct EditeurProfil { profil: Profil, original: Option<String>, erreur: Option<String> }

impl ViewerApp {
    pub(super) fn ouvrir_profil(&mut self, id: Option<&str>) {
        let Some((projet, _)) = &self.natif else { return };
        let profil = if let Some(id) = id {
            let Some(p) = projet.profils.iter().find(|p| p.id == id) else { return };
            p.clone()
        } else {
            let Some(mut p) = crate::config::gabarit::actif().profils.first().cloned().or_else(|| crate::config::gabarit::Gabarit::defaut().profils.first().cloned()) else { return };
            p.id.clear();
            p.nom = (1..).map(|n| format!("Profil {n}")).find(|nom| !projet.profils.iter().any(|x| x.nom.eq_ignore_ascii_case(nom))).unwrap();
            p
        };
        self.editeur_profil = Some(EditeurProfil { profil, original: id.map(str::to_string), erreur: None });
    }

    pub(super) fn charger_profil_fichier(&mut self) {
        let Some(chemin) = rfd::FileDialog::new().add_filter("Profil BIMedit", &["json"]).pick_file() else { return };
        let resultat = std::fs::read_to_string(&chemin).map_err(|e| e.to_string()).and_then(|s| FichierProfil::lire(&s));
        match resultat {
            Ok(mut profil) => { profil.id.clear(); self.editeur_profil = Some(EditeurProfil { profil, original: None, erreur: None }); }
            Err(e) => self.error = Some(e),
        }
    }

    pub(super) fn fenetre_profil(&mut self, ctx: &egui::Context) {
        let Some(mut ed) = self.editeur_profil.take() else { return };
        let mut ouvert = true;
        let (mut enregistrer, mut dupliquer, mut supprimer, mut exporter, mut annuler) = (false, false, false, false, false);
        egui::Window::new("Famille de profil").id(egui::Id::unique("famille_profil")).default_width(630.0).default_height(520.0).open(&mut ouvert).show(ctx, |ui| {
            ui.horizontal(|ui| { ui.label("Nom"); ui.text_edit_singleline(&mut ed.profil.nom); });
            ui.label(RichText::new("Les coordonnées et les paramètres sont exprimés en millimètres. Les formules peuvent référencer les paramètres.").small().weak());
            ui.columns(2, |ui| {
                ui[0].heading("Paramètres");
                egui::ScrollArea::vertical().id_salt("profil_parametres").max_height(180.0).show(&mut ui[0], |ui| {
                    let mut retirer = None;
                    for (i, p) in ed.profil.parametres.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut p.nom).desired_width(95.0));
                            ui.label("=");
                            ui.add(egui::TextEdit::singleline(&mut p.formule).desired_width(80.0));
                            if ui.small_button("−").on_hover_text("Supprimer le paramètre").clicked() { retirer = Some(i); }
                        });
                    }
                    if let Some(i) = retirer { ed.profil.parametres.remove(i); }
                    if ui.small_button("+ Paramètre").clicked() {
                        let nom = (1..).map(|n| format!("Parametre{n}")).find(|nom| !ed.profil.parametres.iter().any(|p| p.nom == *nom)).unwrap();
                        ed.profil.parametres.push(ParametreProfil { nom, formule: "10".into() });
                    }
                });
                ui[0].separator();
                ui[0].heading("Sommets du contour");
                ui[0].label(RichText::new("X et Y, dans l'ordre du contour fermé").small().weak());
                egui::ScrollArea::vertical().id_salt("profil_sommets").max_height(230.0).show(&mut ui[0], |ui| {
                    let mut retirer = None;
                    for (i, p) in ed.profil.contour.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}", i + 1));
                            ui.add(egui::TextEdit::singleline(&mut p.x).desired_width(90.0));
                            ui.add(egui::TextEdit::singleline(&mut p.y).desired_width(90.0));
                            if ui.small_button("−").on_hover_text("Supprimer le sommet").clicked() { retirer = Some(i); }
                        });
                    }
                    if let Some(i) = retirer { ed.profil.contour.remove(i); }
                    if ui.small_button("+ Sommet").clicked() { ed.profil.contour.push(PointProfil { x: "0".into(), y: "0".into() }); }
                });
                ui[1].heading("Aperçu");
                let (r, _) = ui[1].allocate_exact_size(egui::vec2(ui[1].available_width().max(100.0), 300.0), egui::Sense::hover());
                let painter = ui[1].painter();
                painter.rect_filled(r, 2.0, ui[1].visuals().extreme_bg_color);
                match ed.profil.points() {
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
                    Err(e) => { ui[1].colored_label(ui[1].visuals().error_fg_color, e); }
                }
            });
            if let Some(e) = &ed.erreur { ui.colored_label(ui.visuals().error_fg_color, e); }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui.button("Enregistrer dans le projet").clicked() { enregistrer = true; }
                if ed.original.is_some() && ui.button("Créer une copie").clicked() { dupliquer = true; }
                if ui.button("Exporter…").on_hover_text("Enregistrer un fichier de famille de profil").clicked() { exporter = true; }
                if ed.original.is_some() && ui.button("Supprimer").clicked() { supprimer = true; }
                if ui.button("Annuler").clicked() { annuler = true; }
            });
        });
        if exporter {
            match ed.profil.valider() {
                Err(e) => ed.erreur = Some(e),
                Ok(()) => if let Some(chemin) = rfd::FileDialog::new().set_file_name("profil.profil.json").add_filter("Profil BIMedit", &["json"]).save_file() {
                    let f = FichierProfil { format: "bimedit.profil".into(), profil: ed.profil.clone() };
                    if let Err(e) = serde_json::to_string_pretty(&f).map_err(|e| e.to_string()).and_then(|s| std::fs::write(chemin, s).map_err(|e| e.to_string())) { ed.erreur = Some(e); }
                },
            }
        }
        if enregistrer || dupliquer || supprimer {
            let resultat = (|| {
                let (projet, _) = self.natif.as_ref().ok_or("Aucun projet ouvert.")?;
                let mut copie = projet.clone();
                if supprimer {
                    let id = ed.original.as_deref().ok_or("Profil absent.")?;
                    copie.profils.retain(|p| p.id != id);
                } else {
                    ed.profil.nom = ed.profil.nom.trim().into();
                    ed.profil.valider()?;
                    if copie.profils.iter().any(|p| p.nom.eq_ignore_ascii_case(&ed.profil.nom) && (dupliquer || Some(&p.id) != ed.original.as_ref())) {
                        return Err("Un profil porte déjà ce nom ; donnez un nom distinct à la copie.".into());
                    }
                    if dupliquer || ed.original.is_none() { ed.profil.id = crate::modele::collaboration::uuid(); copie.profils.push(ed.profil.clone()); }
                    else if let Some(p) = copie.profils.iter_mut().find(|p| Some(&p.id) == ed.original.as_ref()) { *p = ed.profil.clone(); }
                    else { return Err("Profil introuvable dans le projet.".into()); }
                }
                self.appliquer_projet(copie)
            })();
            match resultat {
                Ok(()) => { self.status = if supprimer { "Profil supprimé." } else { "Profil enregistré dans le projet." }.into(); return; }
                Err(e) => ed.erreur = Some(e),
            }
        }
        if ouvert && !annuler { self.editeur_profil = Some(ed); }
    }
}
