//! Export d'un jeu de feuilles en plusieurs formats : une ligne par feuille, une colonne
//! par format, une case par couple ; un clic sur l'en-tête d'une colonne (ou d'une ligne)
//! la coche ou la décoche en entier.
//!
//! Signification des formats : PDF et DXF (lu par AutoCAD ; le DWG, format fermé, n'est
//! pas écrit) donnent un fichier par feuille cochée, au format du papier, cartouche
//! compris. L'IFC est la maquette du projet, commune à toutes les feuilles : il suffit
//! d'une feuille cochée en IFC pour l'écrire, une seule fois, nommée d'après le jeu.
use crate::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Format {
    Ifc,
    Pdf,
    Dxf,
}

impl Format {
    pub(crate) const TOUS: [Format; 3] = [Format::Ifc, Format::Pdf, Format::Dxf];

    fn libelle(self) -> &'static str {
        match self {
            Format::Ifc => "IFC",
            Format::Pdf => "PDF",
            Format::Dxf => "DWG (DXF)",
        }
    }

    fn aide(self) -> &'static str {
        match self {
            Format::Ifc => "Maquette du projet, commune aux feuilles : un seul fichier, nommé d'après le jeu",
            Format::Pdf => "Un PDF par feuille, au format du papier, cartouche compris",
            Format::Dxf => "Un DXF par feuille (mm du papier), ouvert par AutoCAD et DWG TrueView ; le DWG n'est pas écrit",
        }
    }
}

/// Fenêtre ouverte : jeu exporté et cases cochées (feuille, format).
pub(crate) struct ExportJeu {
    pub jeu: String,
    pub cases: BTreeSet<(String, Format)>,
}

/// Coche toute la colonne `format` si elle ne l'est pas entièrement, sinon la décoche.
pub(crate) fn basculer_colonne(cases: &mut BTreeSet<(String, Format)>, feuilles: &[String], format: Format) {
    let pleine = feuilles.iter().all(|f| cases.contains(&(f.clone(), format)));
    for f in feuilles {
        if pleine { cases.remove(&(f.clone(), format)); } else { cases.insert((f.clone(), format)); }
    }
}

/// Coche toute la ligne `feuille` si elle ne l'est pas entièrement, sinon la décoche.
fn basculer_ligne(cases: &mut BTreeSet<(String, Format)>, feuille: &str, formats: &[Format]) {
    let pleine = formats.iter().all(|&f| cases.contains(&(feuille.to_string(), f)));
    for &f in formats {
        if pleine { cases.remove(&(feuille.to_string(), f)); } else { cases.insert((feuille.to_string(), f)); }
    }
}

/// Fichiers à écrire : (nom de fichier, feuille ou None pour l'IFC, format).
pub(crate) fn fichiers(cases: &BTreeSet<(String, Format)>, feuilles: &[(String, String)], jeu: &str) -> Vec<(String, Option<String>, Format)> {
    let propre = |s: &str| -> String { s.chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect() };
    let mut v = Vec::new();
    if cases.iter().any(|(_, f)| *f == Format::Ifc) {
        v.push((format!("{}.ifc", propre(jeu)), None, Format::Ifc));
    }
    for (id, nom) in feuilles {
        for (format, ext) in [(Format::Pdf, "pdf"), (Format::Dxf, "dxf")] {
            if cases.contains(&(id.clone(), format)) {
                v.push((format!("{}.{ext}", propre(nom)), Some(id.clone()), format));
            }
        }
    }
    v
}

impl ViewerApp {
    /// Ouvre la fenêtre d'export du jeu `jeu`, toutes les feuilles cochées en PDF.
    pub(crate) fn exporter_jeu_dialogue(&mut self, jeu: &str) {
        let Some((p, _)) = &self.natif else { return };
        let cases = p.feuilles.iter().filter(|f| f.jeu == jeu).map(|f| (f.id.clone(), Format::Pdf)).collect();
        self.feuilles.export = Some(ExportJeu { jeu: jeu.to_string(), cases });
    }

    pub(crate) fn fenetre_export_jeu(&mut self, ctx: &egui::Context) {
        let Some(mut etat) = self.feuilles.export.take() else { return };
        let Some((p, _)) = &self.natif else { return };
        let feuilles: Vec<(String, String)> = p.feuilles.iter().filter(|f| f.jeu == etat.jeu).map(|f| (f.id.clone(), format!("{} {}", f.numero, f.nom))).collect();
        let ids: Vec<String> = feuilles.iter().map(|f| f.0.clone()).collect();
        let (mut ouvert, mut exporter) = (true, false);
        egui::Window::new(format!("Exporter le jeu « {} »", etat.jeu)).open(&mut ouvert).collapsible(false).resizable(true).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                egui::Grid::new("matrice_export").striped(true).show(ui, |ui| {
                    if ui.button("Feuille").on_hover_text("Tout cocher ou tout décocher").clicked() {
                        let tout = Format::TOUS.iter().all(|&f| ids.iter().all(|i| etat.cases.contains(&(i.clone(), f))));
                        for f in Format::TOUS {
                            if tout == ids.iter().all(|i| etat.cases.contains(&(i.clone(), f))) { basculer_colonne(&mut etat.cases, &ids, f); }
                        }
                    }
                    for f in Format::TOUS {
                        if ui.button(f.libelle()).on_hover_text(format!("{}\nClic : cocher ou décocher la colonne", f.aide())).clicked() {
                            basculer_colonne(&mut etat.cases, &ids, f);
                        }
                    }
                    ui.end_row();
                    for (id, nom) in &feuilles {
                        if ui.selectable_label(false, nom).on_hover_text("Cocher ou décocher la ligne").clicked() {
                            basculer_ligne(&mut etat.cases, id, &Format::TOUS);
                        }
                        for f in Format::TOUS {
                            let mut coche = etat.cases.contains(&(id.clone(), f));
                            if ui.checkbox(&mut coche, "").changed() {
                                if coche { etat.cases.insert((id.clone(), f)); } else { etat.cases.remove(&(id.clone(), f)); }
                            }
                        }
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            let n = fichiers(&etat.cases, &feuilles, &etat.jeu).len();
            exporter = ui.add_enabled(n > 0, egui::Button::new(format!("Exporter {n} fichier(s)…"))).clicked();
        });
        if exporter && let Some(dossier) = rfd::FileDialog::new().pick_folder() {
            match self.ecrire_export_jeu(&etat, &feuilles, &dossier) {
                Ok(n) => self.informer(format!("Jeu « {} » : {n} fichier(s) écrit(s) dans {}", etat.jeu, dossier.display())),
                Err(e) => self.error = Some(e),
            }
            return;
        }
        if ouvert { self.feuilles.export = Some(etat); }
    }

    fn ecrire_export_jeu(&self, etat: &ExportJeu, feuilles: &[(String, String)], dossier: &std::path::Path) -> Result<usize, String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif ouvert.")?;
        let a_ecrire = fichiers(&etat.cases, feuilles, &etat.jeu);
        for (nom, feuille, format) in &a_ecrire {
            let f = feuille.as_ref().and_then(|id| p.feuilles.iter().find(|f| &f.id == id));
            let octets = match (format, f) {
                (Format::Ifc, _) => echanges::export_ifc::exporter(p)?.into_bytes(),
                (Format::Pdf, Some(f)) => moteur2d::pdf(&moteur2d::svg_feuille(p, f)?)?,
                (Format::Dxf, Some(f)) => moteur2d::dxf_feuille(p, f)?.into_bytes(),
                _ => continue,
            };
            echanges::enregistrement::ecrire_atomique(&dossier.join(nom), &octets)?;
        }
        Ok(a_ecrire.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// N° 13 : l'en-tête d'une colonne la coche puis la décoche ; un seul IFC quelles
    /// que soient les feuilles cochées ; un fichier par feuille et par format sinon.
    #[test]
    fn matrice_et_fichiers() {
        let ids = vec!["a".to_string(), "b".to_string()];
        let mut cases = BTreeSet::new();
        basculer_colonne(&mut cases, &ids, Format::Pdf);
        assert_eq!(cases.len(), 2);
        basculer_colonne(&mut cases, &ids, Format::Pdf);
        assert!(cases.is_empty());
        basculer_ligne(&mut cases, "a", &Format::TOUS);
        assert_eq!(cases.len(), 3);
        cases.remove(&("a".to_string(), Format::Pdf));
        cases.remove(&("a".to_string(), Format::Dxf));
        cases.insert(("b".into(), Format::Ifc));
        cases.insert(("b".into(), Format::Dxf));
        let feuilles = vec![("a".into(), "A01 Plan".into()), ("b".into(), "A02 Coupe/Façade".into())];
        let f = fichiers(&cases, &feuilles, "Permis");
        assert_eq!(f.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["Permis.ifc", "A02 Coupe_Façade.dxf"]);
    }
}
