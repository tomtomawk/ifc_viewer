//! Sélection des familles du projet et accès aux éditeurs.
use super::*;
use crate::modele::{GenreOuverture, Projet};

#[derive(Clone, PartialEq, Eq)]
pub(super) enum FamilleCatalogue {
    Ouverture(String, GenreOuverture),
    Composant(String),
    Profil(String),
    Etiquette(String),
    Cartouche(String),
    GardeCorps(String),
}

pub(super) enum CibleFamille {
    Catalogue(FamilleCatalogue),
    CartoucheFeuille(String),
    Ouverture(String),
    EtiquettePiece(String, String),
    GardeCorps(String),
}

impl FamilleCatalogue {
    pub(super) fn liste(p: &Projet) -> Vec<(Self, &str)> {
        let mut familles = vec![
            (Self::Cartouche(String::new()), "Cartouche standard"),
            (Self::Etiquette(String::new()), "Étiquette standard"),
        ];
        familles.extend(p.types_ouvertures.iter().filter(|t| t.genre != GenreOuverture::Libre)
            .map(|t| (Self::Ouverture(t.id.clone(), t.genre), t.nom.as_str())));
        familles.extend(p.types_composants.iter().map(|t| (Self::Composant(t.id.clone()), t.nom.as_str())));
        familles.extend(p.profils.iter().map(|t| (Self::Profil(t.id.clone()), t.nom.as_str())));
        familles.extend(p.familles_etiquettes.iter().map(|t| (Self::Etiquette(t.id.clone()), t.nom.as_str())));
        familles.extend(p.familles_cartouches.iter().map(|t| (Self::Cartouche(t.id.clone()), t.nom.as_str())));
        familles.extend(p.types_garde_corps.iter().map(|t| (Self::GardeCorps(t.id.clone()), t.nom.as_str())));
        familles.sort_by_key(|(_, nom)| *nom);
        familles
    }

    /// Types de la famille affichés dans l'arborescence : dimensions et nombre d'occurrences.
    pub(super) fn types(&self, p: &Projet) -> Vec<String> {
        let occurrences = |n: usize| match n { 0 => "non utilisé".to_string(), 1 => "1 occurrence".to_string(), n => format!("{n} occurrences") };
        match self {
            Self::Ouverture(id, _) => p.types_ouvertures.iter().find(|t| &t.id == id).map(|t| {
                let n = p.ouvertures.iter().filter(|o| o.type_ouverture.as_deref() == Some(id.as_str())).count();
                vec![format!("{:.0} × {:.0} — {}", t.largeur, t.hauteur, occurrences(n))]
            }).unwrap_or_default(),
            Self::Composant(id) => p.types_composants.iter().find(|t| &t.id == id).map(|t| vec![t.nom.clone()]).unwrap_or_default(),
            Self::Profil(id) => p.profils.iter().find(|t| &t.id == id).map(|t| vec![t.nom.clone()]).unwrap_or_default(),
            Self::Etiquette(id) => vec![p.familles_etiquettes.iter().find(|t| &t.id == id).map_or("Standard".into(), |t| t.nom.clone())],
            Self::Cartouche(id) => vec![p.familles_cartouches.iter().find(|t| &t.id == id).map_or("Standard".into(), |t| t.nom.clone())],
            Self::GardeCorps(id) => p.types_garde_corps.iter().find(|t| &t.id == id).map(|t| vec![t.nom.clone()]).unwrap_or_default(),
        }
    }

    pub(super) fn categorie(&self) -> &'static str {
        match self {
            Self::Ouverture(_, GenreOuverture::Porte) => "Portes",
            Self::Ouverture(..) => "Fenêtres",
            Self::Composant(_) => "Composants",
            Self::Profil(_) => "Profils",
            Self::Etiquette(_) => "Étiquettes de pièce",
            Self::Cartouche(_) => "Cartouches",
            Self::GardeCorps(_) => "Garde-corps",
        }
    }
}

impl ViewerApp {
    pub(super) fn selectionner_famille(&mut self, famille: FamilleCatalogue) {
        self.select(None, false);
        self.feuilles.selection = Default::default();
        self.famille_selectionnee = Some(famille);
        self.onglet_ruban = OngletRuban::Modifier;
    }

    pub(super) fn proprietes_famille_selectionnee(&mut self, ui: &mut egui::Ui) -> bool {
        let (Some(famille), Some((p, _))) = (&self.famille_selectionnee, &self.natif) else { return false };
        let liste = FamilleCatalogue::liste(p);
        let Some((_, nom)) = liste.iter().find(|(f, _)| f == famille) else {
            self.famille_selectionnee = None;
            return false;
        };
        let categorie = famille.categorie();
        ui.heading(categorie);
        let mut choix = famille.clone();
        egui::ComboBox::from_id_salt("famille_catalogue")
            .width((ui.available_width() - 8.0).max(1.0))
            .truncate()
            .selected_text(*nom)
            .show_ui(ui, |ui| {
                for (f, nom) in liste.into_iter().filter(|(f, _)| f.categorie() == categorie) {
                    ui.selectable_value(&mut choix, f, nom);
                }
            });
        self.famille_selectionnee = Some(choix);
        true
    }

    pub(super) fn cible_famille(&self) -> Option<CibleFamille> {
        use FamilleCatalogue as F;
        if let Some(famille) = self.famille_selectionnee.clone() {
            return Some(CibleFamille::Catalogue(famille));
        }
        if self.jeu_actif().is_some() {
            if let feuilles::SelectionFeuille::Cartouche(id) = &self.feuilles.selection {
                return Some(CibleFamille::CartoucheFeuille(id.clone()));
            }
            return None;
        }
        let ids = self.selection_bim();
        let ([id], Some((p, _))) = (ids.as_slice(), &self.natif) else { return None };
        if p.ouvertures.iter().any(|o| &o.id == id && o.genre != GenreOuverture::Libre) {
            Some(CibleFamille::Ouverture(id.clone()))
        } else if let Some(c) = p.composants.iter().find(|c| &c.id == id) {
            Some(CibleFamille::Catalogue(F::Composant(c.type_composant.clone())))
        } else if let Some(piece) = p.pieces.iter().find(|piece| &piece.id == id) {
            Some(CibleFamille::EtiquettePiece(piece.etiquette.clone(), id.clone()))
        } else if p.garde_corps.iter().any(|g| &g.id == id) {
            Some(CibleFamille::GardeCorps(id.clone()))
        } else {
            None
        }
    }

    pub(super) fn modifier_famille_selectionnee(&mut self) {
        use FamilleCatalogue as F;
        use CibleFamille as C;
        match self.cible_famille() {
            Some(C::Catalogue(F::Ouverture(id, genre))) => self.ouvrir_editeur_famille_type(Some(id), genre),
            Some(C::Catalogue(F::Composant(id))) => self.ouvrir_editeur_composant(Some(id)),
            Some(C::Catalogue(F::Profil(id))) => self.ouvrir_profil(Some(&id)),
            Some(C::Catalogue(F::Etiquette(id))) => self.editer_famille_etiquette(Some(id), None),
            Some(C::Catalogue(F::Cartouche(id))) => self.editer_cartouche_catalogue(&id),
            Some(C::Catalogue(F::GardeCorps(id))) => self.ouvrir_type_garde_corps(None, Some(&id), false),
            Some(C::CartoucheFeuille(id)) => self.editer_cartouche_feuille(&id),
            Some(C::Ouverture(id)) => self.ouvrir_editeur_famille(&id),
            Some(C::EtiquettePiece(famille, piece)) => self.editer_famille_etiquette(Some(famille), Some(piece)),
            Some(C::GardeCorps(id)) => self.ouvrir_type_garde_corps(Some(&id), None, false),
            None => {}
        }
    }
}
