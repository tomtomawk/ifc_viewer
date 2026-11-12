//! Onglets d'édition des familles 2D du projet.
use super::*;
use crate::modele::{documentation as doc, famille::profil::{ParametreProfil, PointProfil, Profil}};

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) enum FamillePlane {
    Cartouche { famille: doc::FamilleCartouche, feuille: Option<String>, original: String },
    Etiquette { famille: doc::FamilleEtiquette, piece: Option<String>, original: String },
    Profil { profil: Profil, original: Option<String> },
}

impl FamillePlane {
    fn json_fichier(&self) -> Result<String, String> {
        let valeur = match self {
            Self::Profil { profil, .. } => {
                profil.valider()?;
                serde_json::json!({"format":"bimedit.profil", "profil":profil})
            }
            Self::Cartouche { famille, .. } => {
                if famille.nom.trim().is_empty() || !famille.largeur.is_finite() || !famille.hauteur.is_finite() || famille.largeur <= 0.0 || famille.hauteur <= 0.0 { return Err("Nom et dimensions valides requis pour le cartouche.".into()); }
                serde_json::json!({"format":"bimedit.cartouche", "famille":famille})
            }
            Self::Etiquette { famille, .. } => {
                if famille.nom.trim().is_empty() { return Err("Nommer la famille d'étiquette.".into()); }
                serde_json::json!({"format":"bimedit.etiquette", "famille":famille})
            }
        };
        serde_json::to_string_pretty(&valeur).map_err(|e| e.to_string())
    }

    fn lire_fichier(texte: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(texte).map_err(|e| format!("Fichier de famille illisible : {e}"))?;
        match v.get("format").and_then(|x| x.as_str()) {
            Some("bimedit.profil") => Ok(Self::Profil { profil: crate::modele::famille::profil::FichierProfil::lire(texte)?, original: None }),
            Some("bimedit.cartouche") => {
                let mut famille: doc::FamilleCartouche = serde_json::from_value(v.get("famille").cloned().ok_or("Cartouche absent")?).map_err(|e| e.to_string())?;
                famille.id = crate::modele::collaboration::uuid();
                let contenu = Self::Cartouche { famille, feuille: None, original: String::new() };
                contenu.json_fichier()?;
                Ok(contenu)
            }
            Some("bimedit.etiquette") => {
                let mut famille: doc::FamilleEtiquette = serde_json::from_value(v.get("famille").cloned().ok_or("Étiquette absente")?).map_err(|e| e.to_string())?;
                famille.id.clear();
                let contenu = Self::Etiquette { famille, piece: None, original: String::new() };
                contenu.json_fichier()?;
                Ok(contenu)
            }
            _ => Err("Format de famille plane inconnu.".into()),
        }
    }

    fn extension_fichier(&self) -> &'static str {
        match self { Self::Profil { .. } => "profil", Self::Cartouche { .. } => "cartouche", Self::Etiquette { .. } => "etiquette" }
    }
    fn nom(&self) -> &str {
        match self {
            Self::Cartouche { famille, .. } => &famille.nom,
            Self::Etiquette { famille, .. } => &famille.nom,
            Self::Profil { profil, .. } => &profil.nom,
        }
    }

    fn categorie(&self) -> &'static str {
        match self {
            Self::Cartouche { .. } => "Cartouche",
            Self::Etiquette { .. } => "Étiquette de pièce",
            Self::Profil { .. } => "Profil",
        }
    }

    pub(super) fn id(&self) -> Option<String> {
        match self {
            Self::Cartouche { famille, original, .. } if !original.is_empty() => Some(famille.id.clone()),
            Self::Etiquette { original, .. } if !original.is_empty() => Some(original.clone()),
            Self::Profil { original, .. } => original.clone(),
            _ => None,
        }
    }
}

impl crate::modele::edition::historique::Instantane for FamillePlane {
    fn instantane(&self) -> Result<String, String> { serde_json::to_string(self).map_err(|e| e.to_string()) }
    fn restaurer(json: &str) -> Result<Self, String> { serde_json::from_str(json).map_err(|e| e.to_string()) }
    fn libelle(_: &str, _: &str) -> String { "Modifier la famille".into() }
}

pub(super) struct EditeurPlan {
    pub(super) document_famille: super::document_famille::DocumentFamille<FamillePlane>,
    pub contenu: FamillePlane,
    reference: FamillePlane,
    tracer: bool,
    debut: Option<[f64; 2]>,
    cartouche: cartouche::EditionCartouche,
}

impl EditeurPlan {
    pub(super) fn suivre_historique(&mut self, geste: bool) -> Result<(), String> {
        self.document_famille.suivre(&self.contenu, geste)
    }

    pub(super) fn libelles_historique(&self, retablir: bool) -> Vec<String> { self.document_famille.historique.libelles(retablir) }

    pub(super) fn parcourir_historique(&mut self, retablir: bool, nombre: usize) -> Result<(), String> {
        self.contenu = self.document_famille.cible(&self.contenu, retablir, nombre)?;
        self.tracer = false;
        self.debut = None;
        self.cartouche = Default::default();
        Ok(())
    }

    pub(super) fn element_selectionne(&self) -> bool {
        matches!(self.contenu, FamillePlane::Cartouche { .. }) && self.cartouche.element_selectionne()
    }
    pub(super) fn modifie(&self) -> bool { (self.document_famille.autonome && self.document_famille.chemin_fichier.is_none()) || self.contenu_modifie() }
    pub(super) fn contenu_modifie(&self) -> bool { self.contenu != self.reference }

    pub(super) fn titre(&self) -> String {
        format!("{} · {}{}", self.contenu.categorie(), self.contenu.nom(), if self.modifie() { " *" } else { "" })
    }
}

impl ViewerApp {
    pub(super) fn ouvrir_famille_plane_fichier(&mut self, chemin: &std::path::Path) -> Result<(), String> {
        if let Some(ordre) = self.familles_planes.iter().find(|(_, ed)| ed.document_famille.autonome && ed.document_famille.chemin_fichier.as_deref() == Some(chemin)).map(|(&ordre, _)| ordre) {
            self.activer_famille(ordre);
            return Ok(());
        }
        let texte = std::fs::read_to_string(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        let contenu = FamillePlane::lire_fichier(&texte)?;
        self.creer_editeur_famille_plane(contenu, true, Some(chemin.to_path_buf()));
        Ok(())
    }

    /// Renvoie faux si le dialogue a été annulé ; aucun état n'est alors modifié.
    pub(super) fn enregistrer_famille_plane_dialogue(&mut self, sous: bool) -> Result<bool, String> {
        let ordre = self.famille_plane_active.ok_or("Éditeur fermé")?;
        let ed = self.familles_planes.get(&ordre).ok_or("Famille absente")?;
        let texte = ed.contenu.json_fichier()?;
        let chemin = if sous { None } else { ed.document_famille.chemin_fichier.clone() }.or_else(|| {
            rfd::FileDialog::new().add_filter("Famille BIMedit", &["json"])
                .set_file_name(format!("{}.{}.json", ed.contenu.nom(), ed.contenu.extension_fichier())).save_file()
        });
        let Some(chemin) = chemin else { return Ok(false) };
        crate::modele::echanges::enregistrement::ecrire_atomique(&chemin, texte.as_bytes())?;
        let ed = self.familles_planes.get_mut(&ordre).unwrap();
        ed.document_famille.chemin_fichier = Some(chemin.clone());
        ed.reference = ed.contenu.clone();
        self.status = format!("Famille enregistrée : {}", chemin.display());
        Ok(true)
    }

    pub(super) fn mode_famille(&self) -> bool {
        self.famille_plane_active.is_some() || (self.famille_active && self.famille.is_some())
    }

    pub(super) fn ouvrir_famille_plane(&mut self, contenu: FamillePlane) {
        let id = contenu.id();
        let existant = self.familles_planes.iter().find(|(_, ed)| {
            !ed.document_famille.autonome && ed.document_famille.document == self.document_actif && (ed.reference == contenu
                || (id.is_some() && ed.contenu.categorie() == contenu.categorie() && ed.contenu.id() == id)
                || matches!((&ed.reference, &contenu),
                    (FamillePlane::Cartouche { feuille: a, original: x, .. }, FamillePlane::Cartouche { feuille: b, original: y, .. })
                    if (a == b && x.is_empty() && y.is_empty()) || (x == doc::NOM_CARTOUCHE_STANDARD && y == x)))
        }).map(|(&ordre, _)| ordre);
        if let Some(ordre) = existant {
            self.activer_famille(ordre);
        } else {
            self.creer_editeur_famille_plane(contenu, self.natif.is_none(), None);
        }
        self.onglet_ruban = OngletRuban::FamilleCreer;
    }

    fn creer_editeur_famille_plane(&mut self, contenu: FamillePlane, autonome: bool, chemin_fichier: Option<std::path::PathBuf>) {
        self.prochain_ordre_famille += 1;
        let ordre = self.prochain_ordre_famille;
        self.familles_planes.insert(ordre, EditeurPlan {
            document_famille: super::document_famille::DocumentFamille::nouveau(ordre, self.document_actif, autonome, chemin_fichier, contenu.clone()),
            reference: contenu.clone(), contenu, tracer: false, debut: None,
            cartouche: Default::default(),
        });
        self.activer_famille(ordre);
        self.onglet_ruban = OngletRuban::FamilleCreer;
    }

    pub(super) fn fermer_famille_plane(&mut self) {
        if let Some(ordre) = self.famille_plane_active.take() {
            self.familles_planes.remove(&ordre);
        }
    }

    pub(super) fn edition_plane_modifiee(&self, document: Option<usize>) -> Option<usize> {
        self.familles_planes.iter().find(|(_, ed)| document.is_none_or(|id| !ed.document_famille.autonome && ed.document_famille.document == id) && ed.modifie())
            .map(|(&ordre, _)| ordre)
    }

    pub(super) fn vue_famille_plane(&mut self, ui: &mut egui::Ui) {
        let Some(ed) = self.famille_plane_active.and_then(|i| self.familles_planes.get_mut(&i)) else { return };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !ui.ctx().egui_wants_keyboard_input() {
            ed.tracer = false;
            ed.debut = None;
        }
        let rect = ui.available_rect_before_wrap();
        if rect.is_positive() { self.rect_vue = rect; }
        if let FamillePlane::Cartouche { famille, .. } = &mut ed.contenu {
            let projet_vide = crate::modele::Projet::nouveau("Famille");
            let p = self.natif.as_ref().map(|(p, _)| p).unwrap_or(&projet_vide);
            let rect = ed.cartouche.canevas(ui, p, famille);
            if rect.is_positive() { self.rect_vue = rect; }
            return;
        }
        egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(12.0).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::vertical().id_salt(self.famille_plane_active).show(ui, |ui| match &mut ed.contenu {
                FamillePlane::Etiquette { famille, piece, .. } => {
                    let exemple = self.natif.as_ref().and_then(|(p, _)| {
                        let id = piece.as_ref()?;
                        p.pieces.iter().find(|x| &x.id == id).map(|x| (p, x))
                    });
                    feuilles::dessiner_etiquette(ui, famille, exemple, ed.tracer, &mut ed.debut);
                }
                FamillePlane::Profil { profil, .. } => profils::dessiner_profil(ui, profil),
                FamillePlane::Cartouche { .. } => unreachable!(),
            });
        });
    }

    pub(super) fn cartouche_actif(&mut self) -> Option<&mut cartouche::EditionCartouche> {
        let ed = self.familles_planes.get_mut(&self.famille_plane_active?)?;
        matches!(ed.contenu, FamillePlane::Cartouche { .. }).then_some(&mut ed.cartouche)
    }

    pub(super) fn panneau_famille_plane(&mut self, ui: &mut egui::Ui, panneau: Panneau) {
        self.entete_panneau(ui, panneau, if panneau == Panneau::Proprietes { "Propriétés de la famille" } else { "Arborescence de la famille" });
        let Some(ed) = self.famille_plane_active.and_then(|i| self.familles_planes.get_mut(&i)) else { return };
        if panneau == Panneau::Arborescence {
            ui.label(RichText::new(ed.contenu.categorie()).strong());
            ui.label(ed.contenu.nom());
            match &ed.contenu {
                FamillePlane::Cartouche { .. } => ed.cartouche.arborescence(ui),
                FamillePlane::Etiquette { famille, .. } => { ui.label(format!("{} traits · {} textes", famille.traits.len(), famille.textes.len())); }
                FamillePlane::Profil { profil, .. } => { ui.label(format!("{} sommets · {} paramètres", profil.contour.len(), profil.parametres.len())); }
            }
            return;
        }
        let standard = matches!(&ed.contenu, FamillePlane::Cartouche { original, .. } if original == doc::NOM_CARTOUCHE_STANDARD);
        let nom = match &mut ed.contenu {
            FamillePlane::Cartouche { famille, .. } => &mut famille.nom,
            FamillePlane::Etiquette { famille, .. } => &mut famille.nom,
            FamillePlane::Profil { profil, .. } => &mut profil.nom,
        };
        ui.label("Nom de la famille");
        ui.add_enabled(!standard, egui::TextEdit::singleline(nom));
        if let FamillePlane::Profil { profil, .. } = &mut ed.contenu { profils::proprietes_profil(ui, profil); }
        if let FamillePlane::Cartouche { famille, .. } = &mut ed.contenu {
            ui.horizontal(|ui| {
                ui.label("Largeur");
                ui.add(egui::DragValue::new(&mut famille.largeur).range(10.0..=2000.0).suffix(" mm"));
            });
            ui.horizontal(|ui| {
                ui.label("Hauteur");
                ui.add(egui::DragValue::new(&mut famille.hauteur).range(10.0..=2000.0).suffix(" mm"));
            });
            ed.cartouche.proprietes(ui, famille);
        }
    }

    pub(super) fn ruban_famille_plane(&mut self, ui: &mut egui::Ui, sortie: bool) {
        use crate::ui::ruban::{bandeau, Commande as C, Icone as I};
        let Some(ordre) = self.famille_plane_active else { return };
        let Some(ed) = self.familles_planes.get_mut(&ordre) else { return };
        if !sortie && matches!(ed.contenu, FamillePlane::Cartouche { .. }) {
            let projet_vide = crate::modele::Projet::nouveau("Famille");
            ed.cartouche.ruban(ui, Some(self.natif.as_ref().map(|(p, _)| p).unwrap_or(&projet_vide)));
            return;
        }
        let profil = matches!(ed.contenu, FamillePlane::Profil { .. });
        let existant = ed.contenu.id().is_some();
        let commandes = if sortie {
            let mut commandes = vec![
                C::new("charger", "Charger dans le projet", I::Charger, "Enregistrer la famille dans le projet ouvert", self.natif.is_some()),
                C::new("fichier", "Enregistrer sous", I::EnregistrerSous, "Enregistrer la famille dans un fichier", true),
                C::new("ouvrir", "Ouvrir", I::Ouvrir, "Ouvrir un fichier de famille", true),
            ];
            if profil {
                commandes.extend([
                    C::new("copier", "Créer une copie", I::Nouveau, "Enregistrer un autre profil sous un nom distinct", existant),
                    C::new("exporter", "Exporter", I::EnregistrerSous, "Enregistrer un fichier de famille de profil", true),
                    C::new("supprimer", "Supprimer", I::Fermer, "Supprimer ce profil du projet", existant),
                ]);
            }
            commandes
        } else if profil {
            vec![
                C::new("sommet", "Sommet", I::Famille("ligne"), "Ajouter un sommet au contour", true),
                C::new("parametre", "Paramètre", I::Nouveau, "Ajouter un paramètre au profil", true),
            ]
        } else {
            vec![
                C { enfonce: ed.tracer, ..C::new("trait", "Trait", I::Famille("ligne"), "Cliquer deux points pour dessiner un trait", true) },
                C::new("texte", "Texte", I::Nouveau, "Ajouter un texte à la famille", true),
            ]
        };
        let groupes = if sortie { vec![("Sortie", commandes)] } else { vec![("Formes", crate::ui::ruban::formes_famille_planes()), ("Créer", commandes)] };
        let action = bandeau(ui, &groupes)
            .into_iter().find_map(|(cle, r)| r.clicked().then_some(cle));
        let resultat = match action {
            Some("charger") => self.enregistrer_famille_plane(false),
            Some("fichier") => self.enregistrer_famille_plane_dialogue(true).map(|_| ()),
            Some("ouvrir") => {
                if let Some(chemin) = rfd::FileDialog::new().add_filter("Famille BIMedit", &["json"]).pick_file() { self.open(chemin, ui.ctx()); }
                return;
            }
            Some("copier") => self.enregistrer_famille_plane(true),
            Some("exporter") => match &ed.contenu { FamillePlane::Profil { profil, .. } => profils::exporter_profil(profil), _ => Ok(()) },
            Some("supprimer") => self.supprimer_profil_actif(),
            Some("trait") => { ed.tracer = !ed.tracer; ed.debut = None; Ok(()) }
            Some("texte") => {
                match &mut ed.contenu {
                    FamillePlane::Etiquette { famille, .. } => famille.textes.push(doc::TexteCartouche { contenu: "{numero}".into(), position: [0.0, -6.0], hauteur: 2.0 }),
                    _ => {}
                }
                Ok(())
            }
            Some("sommet") => {
                if let FamillePlane::Profil { profil, .. } = &mut ed.contenu { profil.contour.push(PointProfil { x: "0".into(), y: "0".into() }); }
                Ok(())
            }
            Some("parametre") => {
                if let FamillePlane::Profil { profil, .. } = &mut ed.contenu {
                    let nom = (1..).map(|n| format!("Parametre{n}")).find(|nom| !profil.parametres.iter().any(|p| p.nom == *nom)).unwrap();
                    profil.parametres.push(ParametreProfil { nom, formule: "10".into() });
                }
                Ok(())
            }
            _ => return,
        };
        self.error = resultat.err();
    }

    pub(super) fn navigation_cartouche(&mut self, commande: &str) -> Result<serde_json::Value, String> {
        let ed = self.famille_plane_active.and_then(|i| self.familles_planes.get_mut(&i)).ok_or("Éditeur fermé.")?;
        if !matches!(ed.contenu, FamillePlane::Cartouche { .. }) { return Err("Aucune vue de cartouche ouverte.".into()); }
        ed.cartouche.navigation_pilote(commande)
    }

    pub(super) fn supprimer_selection_famille_plane(&mut self) {
        if let Some(ed) = self.famille_plane_active.and_then(|i| self.familles_planes.get_mut(&i))
            && let FamillePlane::Cartouche { famille, .. } = &mut ed.contenu
        {
            ed.cartouche.supprimer(famille);
        }
    }

    pub(super) fn enregistrer_famille_plane(&mut self, copie: bool) -> Result<(), String> {
        let ordre = self.famille_plane_active.ok_or("Éditeur de famille fermé.")?;
        let ed = self.familles_planes.get(&ordre).ok_or("Famille absente.")?;
        if self.natif.is_none() { return Err("Ouvrir un projet pour y charger la famille.".into()); }
        if !ed.document_famille.autonome && ed.document_famille.document != self.document_actif { return Err("Le projet de la famille doit être actif.".into()); }
        let mut contenu = ed.contenu.clone();
        match &mut contenu {
            FamillePlane::Cartouche { famille, feuille, original } => {
                famille.nom = famille.nom.trim().to_string();
                if original.is_empty() && famille.nom == doc::NOM_CARTOUCHE_STANDARD {
                    let p = &self.natif.as_ref().unwrap().0;
                    famille.nom = (1..).map(|n| if n == 1 { "Cartouche standard (copie)".into() } else { format!("Cartouche standard (copie {n})") })
                        .find(|nom| p.familles_cartouches.iter().all(|f| &f.nom != nom)).unwrap();
                }
                self.enregistrer_cartouche(famille, feuille.as_deref(), original)?;
                *original = famille.nom.clone();
            }
            FamillePlane::Etiquette { famille, piece, original } => {
                famille.nom = famille.nom.trim().to_string();
                if original.is_empty() && famille.nom == "Étiquette standard" {
                    let p = &self.natif.as_ref().unwrap().0;
                    famille.nom = (1..).map(|n| if n == 1 { "Étiquette standard (copie)".into() } else { format!("Étiquette standard (copie {n})") })
                        .find(|nom| p.familles_etiquettes.iter().all(|f| &f.nom != nom)).unwrap();
                }
                famille.id = self.enregistrer_famille_etiquette(famille.clone(), piece.clone(), original)?;
                *original = famille.id.clone();
            }
            FamillePlane::Profil { profil, original } => {
                self.enregistrer_profil(profil, original.as_deref(), copie)?;
                *original = Some(profil.id.clone());
            }
        }
        let ed = self.familles_planes.get_mut(&ordre).unwrap();
        ed.document_famille.document = self.document_actif;
        ed.document_famille.autonome = false;
        ed.reference = contenu.clone();
        ed.contenu = contenu.clone();
        ed.document_famille.instantane = contenu;
        ed.document_famille.historique = Default::default();
        self.status = "Famille enregistrée dans le projet.".into();
        Ok(())
    }

    fn supprimer_profil_actif(&mut self) -> Result<(), String> {
        let id = self.famille_plane_active.and_then(|ordre| self.familles_planes.get(&ordre))
            .and_then(|ed| ed.contenu.id()).ok_or("Profil absent.")?;
        let (projet, _) = self.natif.as_ref().ok_or("Aucun projet ouvert.")?;
        let mut projet = projet.clone();
        projet.profils.retain(|p| p.id != id);
        self.appliquer_projet(projet)?;
        self.fermer_famille_plane();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historique_des_trois_categories_annule_et_retablit_un_geste() {
        let familles = [
            FamillePlane::Profil { profil: crate::config::gabarit::Gabarit::defaut().profils[0].clone(), original: None },
            FamillePlane::Cartouche { famille: doc::FamilleCartouche::default(), feuille: None, original: String::new() },
            FamillePlane::Etiquette { famille: doc::FamilleEtiquette::standard(), piece: None, original: String::new() },
        ];
        for contenu in familles {
            let nom = contenu.nom().to_owned();
            let mut ed = EditeurPlan { document_famille: super::super::document_famille::DocumentFamille::nouveau(0, 0, false, None, contenu.clone()), reference: contenu.clone(), contenu, tracer: false, debut: None, cartouche: Default::default() };
            for suffixe in ["a", "b"] {
                match &mut ed.contenu {
                    FamillePlane::Profil { profil, .. } => profil.nom = format!("{nom}{suffixe}"),
                    FamillePlane::Cartouche { famille, .. } => famille.nom = format!("{nom}{suffixe}"),
                    FamillePlane::Etiquette { famille, .. } => famille.nom = format!("{nom}{suffixe}"),
                }
                ed.suivre_historique(true).unwrap();
            }
            ed.suivre_historique(false).unwrap();
            assert_eq!(ed.libelles_historique(false).len(), 1);
            ed.parcourir_historique(false, 1).unwrap();
            assert_eq!(ed.contenu.nom(), nom);
            assert!(!ed.contenu_modifie());
            ed.parcourir_historique(true, 1).unwrap();
            assert_eq!(ed.contenu.nom(), format!("{nom}b"));
            assert!(ed.contenu_modifie());
        }
    }

    #[test]
    fn fichiers_de_familles_planes_se_rouvrent_sans_projet() {
        let profil = crate::config::gabarit::Gabarit::defaut().profils[0].clone();
        let familles = [
            FamillePlane::Profil { profil, original: None },
            FamillePlane::Cartouche { famille: doc::FamilleCartouche::default(), feuille: None, original: String::new() },
            FamillePlane::Etiquette { famille: doc::FamilleEtiquette::standard(), piece: None, original: String::new() },
        ];
        for famille in familles {
            let texte = famille.json_fichier().unwrap();
            let relue = FamillePlane::lire_fichier(&texte).unwrap();
            assert_eq!(famille.nom(), relue.nom());
            assert_eq!(famille.categorie(), relue.categorie());
            match (&famille, &relue) {
                (FamillePlane::Cartouche { famille: avant, .. }, FamillePlane::Cartouche { famille: apres, .. }) => {
                    assert_eq!(avant.formes, apres.formes);
                    assert_eq!(avant.traits, apres.traits);
                    assert_eq!(avant.textes, apres.textes);
                }
                (FamillePlane::Profil { profil: avant, .. }, FamillePlane::Profil { profil: apres, .. }) => assert_eq!(avant, apres),
                (FamillePlane::Etiquette { famille: avant, .. }, FamillePlane::Etiquette { famille: apres, .. }) => {
                    assert_eq!(avant.traits, apres.traits);
                    assert_eq!(avant.textes, apres.textes);
                }
                _ => panic!("Catégorie de famille différente après réouverture"),
            }
        }
    }
}
