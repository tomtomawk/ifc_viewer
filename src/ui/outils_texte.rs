//! Propriétés communes des textes placés dans un document graphique.
use crate::config::options::Dimensions;
use eframe::egui;

/// Champs sélectionnables selon le document cible.
pub(super) const CHAMPS_PROJET: &[(&str, &str)] = &[("{projet}", "Projet"), ("{vue}", "Nom de la vue")];
pub(super) const CHAMPS_FEUILLE: &[(&str, &str)] = &[("{projet}", "Projet"), ("{feuille}", "Nom de la feuille"), ("{numero}", "Numéro"), ("{cartouche}", "Texte du cartouche"), ("{echelles}", "Échelles")];
pub(super) const CHAMPS_PIECE: &[(&str, &str)] = &[("{nom}", "Nom"), ("{numero}", "Numéro"), ("{surface}", "Surface"), ("{perimetre}", "Périmètre"), ("{niveau}", "Niveau")];

/// Les champs disponibles et l'unité du point dépendent du document.
pub(super) fn proprietes_texte(ui: &mut egui::Ui, contenu: &mut String, position: &mut [f64; 2], papier: bool, champs: &[(&str, &str)]) -> bool {
    let mut change = false;
    ui.label("Contenu");
    change |= ui.text_edit_singleline(contenu).changed();
    if !champs.is_empty() {
        egui::ComboBox::from_id_salt("champ_texte_commun").selected_text("Insérer un champ").show_ui(ui, |ui| {
            for &(cle, nom) in champs {
                if ui.button(nom).clicked() { contenu.push_str(cle); change = true; ui.close(); }
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("Position");
        for valeur in position {
            if papier { change |= ui.add(egui::DragValue::new(valeur).speed(0.25).suffix(" mm")).changed(); }
            else { change |= ui.add(egui::DragValue::new(valeur).speed(10.0).dimensions()).changed(); }
        }
    });
    change
}

/// Hauteur imprimée d'un texte, en millimètres de papier.
pub(super) fn hauteur_texte(ui: &mut egui::Ui, hauteur: &mut f64) -> bool {
    let mut change = false;
    ui.horizontal(|ui| {
        ui.label("Hauteur du texte");
        change = ui.add(egui::DragValue::new(hauteur).range(0.5..=100.0).speed(0.1).suffix(" mm")).changed();
    });
    change
}

/// Aperçu fondé sur le contexte réellement associé à la famille ; les champs sans
/// contexte restent visibles pour être identifiables pendant l'édition autonome.
pub(super) fn apercu_famille(contenu: &str, projet: &crate::modele::Projet, vue: &str, categorie: crate::modele::famille::modelee::CategorieFamille, feuille: Option<&str>, piece: Option<&str>) -> String {
    use crate::modele::famille::modelee::CategorieFamille as C;
    use crate::modele::documentation::{remplir_champs, FamilleEtiquette};
    match categorie {
        C::EtiquettePiece => piece.and_then(|id| projet.pieces.iter().find(|p| p.id == id))
            .map_or_else(|| contenu.to_owned(), |p| FamilleEtiquette::remplir(contenu, projet, p)),
        C::Cartouche => {
            let Some(f) = feuille.and_then(|id| projet.feuilles.iter().find(|f| f.id == id)) else { return contenu.to_owned() };
            let mut echelles: Vec<String> = f.vues.iter().map(|v| format!("1:{}", v.echelle_sur_feuille(projet))).collect();
            echelles.dedup();
            let echelles = if echelles.is_empty() { String::new() } else { format!("Échelle {}", echelles.join(", ")) };
            remplir_champs(contenu, &[("{projet}", &projet.nom), ("{feuille}", &f.nom), ("{numero}", &f.numero), ("{cartouche}", &f.cartouche), ("{echelles}", &echelles)])
        }
        _ if projet.nom.is_empty() => remplir_champs(contenu, &[("{vue}", vue)]),
        _ => remplir_champs(contenu, &[("{projet}", &projet.nom), ("{vue}", vue)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::famille::modelee::CategorieFamille as C;
    #[test]
    fn apercu_emploie_le_contexte_associe_et_conserve_les_champs_inconnus() {
        let mut projet = crate::modele::Projet::nouveau("Agence");
        projet.feuilles.push(crate::modele::documentation::Feuille { id: "f".into(), numero: "A01".into(), nom: "Plan".into(), jeu: "Plans".into(), largeur: 420.0, hauteur: 297.0, cartouche: "Titre".into(), vues: vec![], nomenclatures: vec![] });
        let texte = "{projet} - {numero} - {feuille}";
        assert_eq!(apercu_famille(texte, &projet, "Plan", C::Cartouche, Some("f"), None), "Agence - A01 - Plan");
        assert_eq!(apercu_famille(texte, &projet, "Plan", C::Cartouche, None, None), texte);
        assert_eq!(apercu_famille("{projet} / {vue}", &projet, "Coupe", C::Composant, None, None), "Agence / Coupe");
        assert_eq!(apercu_famille("{projet} / {vue}", &Default::default(), "Coupe", C::Composant, None, None), "{projet} / Coupe");
        projet.pieces.push(crate::modele::Piece { id: "p".into(), nom: "Bureau".into(), numero: "101".into(), niveau: String::new(), point: [0.0, 0.0], contour: vec![], etiquette: String::new() });
        assert_eq!(apercu_famille("{nom} {numero}", &projet, "Plan", C::EtiquettePiece, None, Some("p")), "Bureau 101");
    }
}
