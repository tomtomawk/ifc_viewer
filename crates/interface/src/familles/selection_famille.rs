//! Sélection des familles du projet et accès aux éditeurs.
use crate::*;
use modele::{GenreOuverture, Projet};

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum FamilleCatalogue {
    Ouverture(String, GenreOuverture),
    Composant(String),
    Profil(String),
    Etiquette(String),
    Cartouche(String),
    GardeCorps(String),
    /// Symbole d'annotation : genre et famille (vide : la standard).
    Symbole(modele::documentation::GenreSymbole, String),
}

pub(crate) enum CibleFamille {
    Catalogue(FamilleCatalogue),
    /// Cartouche posé : feuille, rang.
    CartoucheFeuille(String, usize),
    Ouverture(String),
    EtiquettePiece(String, String),
    GardeCorps(String),
}

impl FamilleCatalogue {
    pub(crate) fn liste(p: &Projet) -> Vec<(Self, String)> {
        let mut familles = vec![
            (Self::Cartouche(String::new()), "Cartouche standard"),
            (Self::Etiquette(String::new()), "Étiquette standard"),
        ];
        familles.extend(p.types_ouvertures.iter()
            .map(|t| (Self::Ouverture(t.id.clone(), t.genre), t.nom.as_str())));
        familles.extend(p.types_composants.iter().map(|t| (Self::Composant(t.id.clone()), t.nom.as_str())));
        familles.extend(p.profils.iter().map(|t| (Self::Profil(t.id.clone()), t.nom.as_str())));
        familles.extend(p.familles_etiquettes.iter().map(|t| (Self::Etiquette(t.id.clone()), t.nom.as_str())));
        familles.extend(p.familles_cartouches.iter().map(|t| (Self::Cartouche(t.id.clone()), t.nom.as_str())));
        familles.extend(p.types_garde_corps.iter().map(|t| (Self::GardeCorps(t.id.clone()), t.nom.as_str())));
        let mut liste: Vec<(Self, String)> = familles.into_iter().map(|(f, n)| (f, n.to_string())).collect();
        liste.extend(modele::documentation::GenreSymbole::TOUS.iter().map(|g| (Self::Symbole(*g, String::new()), g.nom_standard())));
        liste.extend(p.familles_symboles.iter().filter_map(|f| Some((Self::Symbole(f.genre()?, f.id.clone()), f.nom.clone()))));
        liste.sort_by(|a, b| a.1.cmp(&b.1));
        liste
    }

    /// Types de la famille affichés dans l'arborescence : dimensions et nombre d'occurrences.
    pub(crate) fn types(&self, p: &Projet) -> Vec<String> {
        let occurrences = |n: usize| match n { 0 => "non utilisé".to_string(), 1 => "1 occurrence".to_string(), n => format!("{n} occurrences") };
        match self {
            Self::Ouverture(id, _) => p.types_ouvertures.iter().find(|t| &t.id == id).map(|t| {
                let n = p.ouvertures.iter().filter(|o| o.type_ouverture.as_deref() == Some(id.as_str())).count();
                vec![format!("{:.0} × {:.0} · {}", t.largeur, t.hauteur, occurrences(n))]
            }).unwrap_or_default(),
            Self::Composant(id) => p.types_composants.iter().find(|t| &t.id == id).map(|t| vec![t.nom.clone()]).unwrap_or_default(),
            Self::Profil(id) => p.profils.iter().find(|t| &t.id == id).map(|t| vec![t.nom.clone()]).unwrap_or_default(),
            Self::Etiquette(id) => vec![p.familles_etiquettes.iter().find(|t| &t.id == id).map_or("Standard".into(), |t| t.nom.clone())],
            Self::Cartouche(id) => vec![p.familles_cartouches.iter().find(|t| &t.id == id).map_or("Standard".into(), |t| t.nom.clone())],
            Self::GardeCorps(id) => p.types_garde_corps.iter().find(|t| &t.id == id).map(|t| vec![t.nom.clone()]).unwrap_or_default(),
            Self::Symbole(g, id) => vec![if p.symboles.choix(*g).map_or(id.is_empty(), |c| c == id) { "utilisée dans le projet".into() } else { "non utilisée".into() }],
        }
    }

    pub(crate) fn categorie(&self) -> &'static str {
        match self {
            Self::Ouverture(_, GenreOuverture::Porte) => "Portes",
            Self::Ouverture(..) => "Fenêtres",
            Self::Composant(_) => "Composants",
            Self::Profil(_) => "Profils",
            Self::Etiquette(_) => "Étiquettes de pièce",
            Self::Cartouche(_) => "Cartouches",
            Self::GardeCorps(_) => "Garde-corps",
            Self::Symbole(..) => "Symboles d'annotation",
        }
    }
}

impl Application {
    pub(crate) fn mode_famille(&self) -> bool {
        self.famille_active && self.famille.is_some()
    }

    /// Ouvre un profil du projet (`id`) ou un nouveau profil, copie du premier du gabarit.
    pub(crate) fn ouvrir_profil(&mut self, id: Option<&str>) {
        if let Some(p) = id.and_then(|id| self.natif.as_ref()?.0.profils.iter().find(|p| p.id == id)) {
            let (fichier, id) = ((*p.document_famille).clone(), p.id.clone());
            self.ouvrir_famille_document_projet(fichier, Some(id));
            return;
        }
        // nouveau profil : d'après le premier profil du projet, sinon rectangle du code
        let profil = self.natif.as_ref().and_then(|(p, _)| p.profils.first().cloned())
            .unwrap_or_else(|| modele::famille::profil::Profil::depuis_contour("Profil", vec![[-50.0, -50.0], [50.0, -50.0], [50.0, 50.0], [-50.0, 50.0]]));
        let noms: Vec<String> = self.natif.as_ref().map(|(p, _)| p.profils.iter().map(|x| x.nom.clone()).collect()).unwrap_or_default();
        let mut document = *profil.document_famille;
        document.nom = (1..).map(|n| format!("Profil {n}")).find(|nom| !noms.iter().any(|x| x.eq_ignore_ascii_case(nom))).unwrap_or_default();
        self.ouvrir_famille_document_projet(document, None);
    }

    /// Ouvre un fichier de famille de profil (`.bef`) dans l'éditeur commun.
    pub(crate) fn charger_profil_fichier(&mut self) {
        let Some(chemin) = rfd::FileDialog::new().add_filter("Famille BIMedit", &[echanges::enregistrement::EXTENSION_FAMILLE]).pick_file() else { return };
        if let Err(e) = self.ouvrir_famille_fichier(&chemin) { self.error = Some(e); }
    }

    pub(crate) fn enregistrer_profil(&mut self, profil: &mut modele::famille::profil::Profil, original: Option<&str>, copie: bool) -> Result<(), String> {
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

    pub(crate) fn selectionner_famille(&mut self, famille: FamilleCatalogue) {
        self.select(None, false);
        self.feuilles.selection = Default::default();
        self.famille_selectionnee = Some(famille);
        self.onglet_ruban = OngletRuban::Modifier;
    }

    pub(crate) fn proprietes_famille_selectionnee(&mut self, ui: &mut egui::Ui) -> bool {
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
            .selected_text(nom.clone())
            .show_ui(ui, |ui| {
                for (f, nom) in liste.into_iter().filter(|(f, _)| f.categorie() == categorie) {
                    ui.selectable_value(&mut choix, f, nom);
                }
            });
        self.famille_selectionnee = Some(choix);
        // famille d'un type de porte ou de fenêtre : regroupement dans l'arborescence
        if let (Some(FamilleCatalogue::Ouverture(id, _)), Some((p, _))) = (&self.famille_selectionnee, &self.natif) {
            let actuel = p.types_ouvertures.iter().find(|t| &t.id == id).map(|t| t.nom_famille().to_string()).unwrap_or_default();
            let cle = ui.make_persistent_id(("famille_nom_type", id.as_str()));
            let mut saisie = ui.data(|d| d.get_temp::<String>(cle)).unwrap_or_else(|| actuel.clone());
            let r = ui.horizontal(|ui| { ui.label("Famille"); ui.text_edit_singleline(&mut saisie) }).inner;
            if r.lost_focus() && saisie.trim() != actuel {
                let mut q = p.clone();
                if let Some(t) = q.types_ouvertures.iter_mut().find(|t| &t.id == id) {
                    t.famille_nom = Some(saisie.trim().to_string()).filter(|n| !n.is_empty());
                }
                if let Err(e) = self.appliquer_projet(q) { self.error = Some(e); }
                ui.data_mut(|d| d.remove::<String>(cle));
            } else if r.has_focus() {
                ui.data_mut(|d| d.insert_temp(cle, saisie));
            } else {
                ui.data_mut(|d| d.remove::<String>(cle));
            }
        }
        true
    }

    pub(crate) fn cible_famille(&self) -> Option<CibleFamille> {
        use FamilleCatalogue as F;
        if let Some(famille) = self.famille_selectionnee.clone() {
            return Some(CibleFamille::Catalogue(famille));
        }
        if self.jeu_actif().is_some() {
            if let crate::documenter::feuilles::SelectionFeuille::Cartouche(id, i) = &self.feuilles.selection {
                return Some(CibleFamille::CartoucheFeuille(id.clone(), *i));
            }
            return None;
        }
        let ids = self.selection_bim();
        let ([id], Some((p, _))) = (ids.as_slice(), &self.natif) else { return None };
        if p.ouvertures.iter().any(|o| &o.id == id && o.type_ouverture.is_some()) {
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

    pub(crate) fn modifier_famille_selectionnee(&mut self) {
        use FamilleCatalogue as F;
        use CibleFamille as C;
        match self.cible_famille() {
            Some(C::Catalogue(F::Ouverture(id, genre))) => self.ouvrir_editeur_famille_type(Some(id), genre),
            Some(C::Catalogue(F::Composant(id))) => self.ouvrir_editeur_composant(Some(id), false),
            Some(C::Catalogue(F::Profil(id))) => self.ouvrir_profil(Some(&id)),
            Some(C::Catalogue(F::Etiquette(id))) => self.editer_famille_etiquette(Some(id), None),
            Some(C::Catalogue(F::Cartouche(id))) => self.editer_cartouche_catalogue(&id),
            Some(C::Catalogue(F::GardeCorps(id))) => self.ouvrir_type_garde_corps(None, Some(&id), false),
            Some(C::Catalogue(F::Symbole(g, id))) => self.editer_famille_symbole(g, &id),
            Some(C::CartoucheFeuille(id, i)) => self.editer_cartouche_feuille(&id, i),
            Some(C::Ouverture(id)) => self.ouvrir_editeur_famille(&id),
            Some(C::EtiquettePiece(famille, piece)) => self.editer_famille_etiquette(Some(famille), Some(piece)),
            Some(C::GardeCorps(id)) => self.ouvrir_type_garde_corps(Some(&id), None, false),
            None => {}
        }
    }
}
