//! Export d'un jeu de feuilles en plusieurs formats : une ligne par feuille, une colonne
//! par format, une case par couple ; un clic sur l'en-tête d'une colonne (ou d'une ligne)
//! la coche ou la décoche en entier.
//!
//! Signification des formats : DWG (2004) et DXF donnent un fichier par feuille cochée,
//! au format du papier, cartouche compris. Le PDF aussi, ou un seul PDF pour le jeu
//! (feuilles cochées dans l'ordre du jeu) si « PDF groupé » est coché. L'IFC est la
//! maquette du projet, commune à toutes les feuilles : il suffit d'une feuille cochée en
//! IFC pour l'écrire, une seule fois, nommée d'après le jeu.
use crate::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Format {
    Ifc,
    Pdf,
    Dwg,
    Dxf,
}

impl Format {
    pub(crate) const TOUS: [Format; 4] = [Format::Ifc, Format::Pdf, Format::Dwg, Format::Dxf];

    fn libelle(self) -> &'static str {
        match self {
            Format::Ifc => "IFC",
            Format::Pdf => "PDF",
            Format::Dwg => "DWG",
            Format::Dxf => "DXF",
        }
    }

    fn aide(self) -> &'static str {
        match self {
            Format::Ifc => "Maquette du projet, commune aux feuilles : un seul fichier, nommé d'après le jeu",
            Format::Pdf => "Un PDF par feuille, au format du papier, cartouche compris ; un seul pour le jeu si « PDF groupé » est coché",
            Format::Dwg => "Un DWG 2004 par feuille (mm du papier, un calque par rôle : coupe, vu, poché, hachure, dessin)",
            Format::Dxf => "Un DXF R12 par feuille (mm du papier), mêmes calques que le DWG",
        }
    }
}

/// Fenêtre ouverte : jeu exporté et cases cochées (feuille, format).
pub(crate) struct ExportJeu {
    pub jeu: String,
    pub cases: BTreeSet<(String, Format)>,
}

/// Fichier à écrire : nom, feuilles (aucune pour l'IFC, plusieurs pour un PDF groupé),
/// format.
pub(crate) type Fichier = (String, Vec<String>, Format);

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

/// Fichiers à écrire ; `pdf_groupe` : un seul PDF pour le jeu (feuilles cochées dans
/// l'ordre de `feuilles`).
pub(crate) fn fichiers(cases: &BTreeSet<(String, Format)>, feuilles: &[(String, String)], jeu: &str, pdf_groupe: bool) -> Vec<Fichier> {
    let propre = |s: &str| -> String { s.chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect() };
    let mut v = Vec::new();
    if cases.iter().any(|(_, f)| *f == Format::Ifc) {
        v.push((format!("{}.ifc", propre(jeu)), Vec::new(), Format::Ifc));
    }
    let pdf: Vec<String> = feuilles.iter().filter(|(id, _)| cases.contains(&(id.clone(), Format::Pdf))).map(|(id, _)| id.clone()).collect();
    if pdf_groupe && !pdf.is_empty() {
        v.push((format!("{}.pdf", propre(jeu)), pdf, Format::Pdf));
    }
    for (id, nom) in feuilles {
        for (format, ext) in [(Format::Pdf, "pdf"), (Format::Dwg, "dwg"), (Format::Dxf, "dxf")] {
            if (format != Format::Pdf || !pdf_groupe) && cases.contains(&(id.clone(), format)) {
                v.push((format!("{}.{ext}", propre(nom)), vec![id.clone()], format));
            }
        }
    }
    v
}

impl Application {
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
            ui.checkbox(&mut self.feuilles.pdf_groupe, "PDF groupé").on_hover_text("Coché : un seul PDF pour le jeu, feuilles dans l'ordre du jeu. Décoché : un PDF par feuille.");
            let n = fichiers(&etat.cases, &feuilles, &etat.jeu, self.feuilles.pdf_groupe).len();
            exporter = ui.add_enabled(n > 0, egui::Button::new(format!("Exporter {n} fichier(s)…"))).clicked();
        });
        if exporter && let Some(dossier) = rfd::FileDialog::new().pick_folder() {
            // écrit en arrière-plan, un fichier après l'autre (`socle::taches`)
            let a_ecrire = fichiers(&etat.cases, &feuilles, &etat.jeu, self.feuilles.pdf_groupe);
            self.exporter_jeu_arriere_plan(ctx, etat.jeu, a_ecrire, dossier);
            return;
        }
        if ouvert { self.feuilles.export = Some(etat); }
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
        assert_eq!(cases.len(), 4);
        cases.remove(&("a".to_string(), Format::Pdf));
        cases.remove(&("a".to_string(), Format::Dxf));
        cases.remove(&("a".to_string(), Format::Dwg));
        cases.insert(("b".into(), Format::Ifc));
        cases.insert(("b".into(), Format::Dxf));
        cases.insert(("b".into(), Format::Dwg));
        let feuilles = vec![("a".into(), "A01 Plan".into()), ("b".into(), "A02 Coupe/Façade".into())];
        let f = fichiers(&cases, &feuilles, "Permis", false);
        assert_eq!(f.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["Permis.ifc", "A02 Coupe_Façade.dwg", "A02 Coupe_Façade.dxf"]);
    }

    /// PDF groupé : un seul PDF nommé d'après le jeu, feuilles cochées dans l'ordre du
    /// jeu ; séparé : un PDF par feuille.
    #[test]
    fn pdf_groupe_ou_separe() {
        let feuilles: Vec<(String, String)> = vec![("a".into(), "A01".into()), ("b".into(), "A02".into()), ("c".into(), "A03".into())];
        let cases: BTreeSet<(String, Format)> = [("c".to_string(), Format::Pdf), ("a".to_string(), Format::Pdf), ("a".to_string(), Format::Dwg)].into_iter().collect();
        let g = fichiers(&cases, &feuilles, "DCE", true);
        assert_eq!(g[0], ("DCE.pdf".to_string(), vec!["a".to_string(), "c".to_string()], Format::Pdf));
        assert_eq!(g.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["DCE.pdf", "A01.dwg"]);
        let s = fichiers(&cases, &feuilles, "DCE", false);
        assert_eq!(s.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["A01.pdf", "A01.dwg", "A03.pdf"]);
    }
}
