//! Onglets d'édition des familles 2D du projet.
use super::*;
use crate::modele::{documentation as doc, famille::profil::{ParametreProfil, PointProfil, Profil}};

#[derive(Clone, PartialEq)]
pub(super) enum FamillePlane {
    Cartouche { famille: doc::FamilleCartouche, feuille: Option<String>, original: String },
    Etiquette { famille: doc::FamilleEtiquette, piece: Option<String>, original: String },
    Profil { profil: Profil, original: Option<String> },
}

impl FamillePlane {
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

pub(super) struct EditeurPlan {
    pub document: usize,
    pub contenu: FamillePlane,
    reference: FamillePlane,
    tracer: bool,
    debut: Option<[f64; 2]>,
    cartouche: cartouche::EditionCartouche,
}

impl EditeurPlan {
    pub(super) fn modifie(&self) -> bool { self.contenu != self.reference }

    pub(super) fn titre(&self) -> String {
        format!("{} · {}{}", self.contenu.categorie(), self.contenu.nom(), if self.modifie() { " *" } else { "" })
    }
}

impl ViewerApp {
    pub(super) fn mode_famille(&self) -> bool {
        self.famille_plane_active.is_some() || (self.famille_active && self.famille.is_some())
    }

    pub(super) fn ouvrir_famille_plane(&mut self, contenu: FamillePlane) {
        let id = contenu.id();
        let existant = self.familles_planes.iter().find(|(_, ed)| {
            ed.document == self.document_actif && (ed.reference == contenu
                || (id.is_some() && ed.contenu.categorie() == contenu.categorie() && ed.contenu.id() == id)
                || matches!((&ed.reference, &contenu),
                    (FamillePlane::Cartouche { feuille: a, original: x, .. }, FamillePlane::Cartouche { feuille: b, original: y, .. })
                    if a == b && x.is_empty() && y.is_empty()))
        }).map(|(&ordre, _)| ordre);
        let ordre = existant.unwrap_or_else(|| {
            self.prochain_ordre_famille += 1;
            let ordre = self.prochain_ordre_famille;
            self.familles_planes.insert(ordre, EditeurPlan {
                document: self.document_actif,
                reference: contenu.clone(),
                contenu,
                tracer: false,
                debut: None,
                cartouche: Default::default(),
            });
            ordre
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
        self.familles_planes.iter().find(|(_, ed)| document.is_none_or(|id| ed.document == id) && ed.modifie())
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
            if let Some((p, _)) = &self.natif {
                let rect = ed.cartouche.canevas(ui, p, famille);
                if rect.is_positive() { self.rect_vue = rect; }
            }
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
        let nom = match &mut ed.contenu {
            FamillePlane::Cartouche { famille, .. } => &mut famille.nom,
            FamillePlane::Etiquette { famille, .. } => &mut famille.nom,
            FamillePlane::Profil { profil, .. } => &mut profil.nom,
        };
        ui.label("Nom de la famille");
        ui.text_edit_singleline(nom);
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
            ed.cartouche.ruban(ui, self.natif.as_ref().map(|(p, _)| p));
            return;
        }
        let profil = matches!(ed.contenu, FamillePlane::Profil { .. });
        let existant = ed.contenu.id().is_some();
        let commandes = if sortie {
            let mut commandes = vec![C::new("charger", "Charger dans le projet", I::Charger, "Enregistrer la famille dans son projet (Ctrl+S)", true)];
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
        let action = bandeau(ui, &[(if sortie { "Sortie" } else { "Créer" }, commandes)])
            .into_iter().find_map(|(cle, r)| r.clicked().then_some(cle));
        let resultat = match action {
            Some("charger") => self.enregistrer_famille_plane(false),
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
        if ed.document != self.document_actif { return Err("Le projet de la famille doit être actif.".into()); }
        let mut contenu = ed.contenu.clone();
        match &mut contenu {
            FamillePlane::Cartouche { famille, feuille, original } => {
                famille.nom = famille.nom.trim().to_string();
                self.enregistrer_cartouche(famille, feuille.as_deref(), original)?;
                *original = famille.nom.clone();
            }
            FamillePlane::Etiquette { famille, piece, original } => {
                famille.nom = famille.nom.trim().to_string();
                famille.id = self.enregistrer_famille_etiquette(famille.clone(), piece.clone(), original)?;
                *original = famille.id.clone();
            }
            FamillePlane::Profil { profil, original } => {
                self.enregistrer_profil(profil, original.as_deref(), copie)?;
                *original = Some(profil.id.clone());
            }
        }
        let ed = self.familles_planes.get_mut(&ordre).unwrap();
        ed.reference = contenu.clone();
        ed.contenu = contenu;
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
