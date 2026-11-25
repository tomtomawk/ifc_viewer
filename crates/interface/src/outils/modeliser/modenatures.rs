//! Outil Profilé de mur (Modéliser → Construire) : sélecteur de type commun, paramètres
//! de pose, clic sur un mur. La face est celle du côté cliqué ; en 3D, en coupe ou en
//! élévation, la hauteur est celle du point cliqué, en plan celle des paramètres.
use crate::*;
use modele::elements::modenature::{Modenature, TypeModenature};
use modele::Axe;

/// Réglages des prochains profilés.
pub(crate) struct ReglagesModenature {
    pub type_id: Option<String>,
    /// Hauteur de pose depuis la base (ou depuis le haut), mm : en plan.
    pub hauteur: f64,
    pub depuis_haut: bool,
}

impl Default for ReglagesModenature {
    fn default() -> Self { Self { type_id: None, hauteur: 1000.0, depuis_haut: false } }
}

/// Types proposés quand le projet n'en a pas encore.
fn type_par_defaut(creux: bool) -> TypeModenature {
    let (nom, largeur, hauteur) = if creux { ("Engravure 20 × 20", 20.0, 20.0) } else { ("Bandeau 200 × 50", 50.0, 200.0) };
    TypeModenature { id: String::new(), nom: nom.into(), creux, profil: None, largeur, hauteur, materiau: None, decalage: 0.0, retrait: 0.0 }
}

/// Distance d'un point (plan, mm) à la ligne d'un axe.
fn distance_axe(axe: &Axe, q: glam::DVec2) -> f64 {
    match *axe {
        Axe::Droit { debut, fin } => {
            let (a, b) = (glam::DVec2::from_array(debut), glam::DVec2::from_array(fin));
            (b - a).normalize_or_zero().perp_dot(q - a).abs()
        }
        Axe::Arc { debut, centre, .. } => {
            let c = glam::DVec2::from_array(centre);
            (q.distance(c) - c.distance(glam::DVec2::from_array(debut))).abs()
        }
    }
}

impl Application {
    /// Emplacement « sélecteur » : types de profilé du projet, Dupliquer, nouveau type.
    pub(crate) fn selecteur_modenature(&mut self, ui: &mut egui::Ui) {
        use crate::zones::ruban::selecteur;
        let Some((p, _)) = &self.natif else { return };
        let types = p.types_modenatures.clone();
        let r = &mut self.bim.modenature;
        if r.type_id.as_ref().is_none_or(|id| !types.iter().any(|t| &t.id == id)) {
            r.type_id = types.first().map(|t| t.id.clone());
        }
        let u = modele::unite::unite();
        let details: Vec<String> = types.iter().map(|t| {
            let genre = if t.creux { "creux" } else { "relief" };
            match &t.profil {
                Some(id) => format!("{genre} · {}", p.profils.iter().find(|x| &x.id == id).map_or("profil absent", |x| x.nom.as_str())),
                None => format!("{genre} · {} × {}", u.afficher(t.largeur), u.afficher(t.hauteur)),
            }
        }).collect();
        let actions: &[(selecteur::ActionType, &str)] = &[(selecteur::ActionType::Dupliquer, "Dupliquer : nouveau type d'après celui-ci")];
        let lignes: Vec<selecteur::Ligne> = types.iter().zip(&details).map(|(t, d)| selecteur::Ligne { nom: &t.nom, courant: r.type_id.as_ref() == Some(&t.id), etiquette: None, sous_titre: None, detail: Some(d.clone()), actions }).collect();
        let nom = r.type_id.as_ref().and_then(|id| types.iter().find(|t| &t.id == id)).map_or("aucun type", |t| t.nom.as_str());
        let titre = format!("Profilé · {nom}");
        let choix = selecteur::simple(ui, &titre, None, &format!("{titre}\nType de profilé : profil, relief ou creux, décalage, retrait.\nSes propriétés se modifient dans Propriétés du type (double-clic)."), &lignes, Some("+ Nouveau type de profilé"), None);
        let mut nouveau = None;
        if let Some(k) = choix.ligne { r.type_id = Some(types[k].id.clone()); }
        if let Some((k, _)) = choix.action {
            let base = types[k].clone();
            let nom = (2..).map(|n| format!("{} {n}", base.nom)).find(|n| types.iter().all(|t| &t.nom != n)).unwrap_or_default();
            nouveau = Some(TypeModenature { id: String::new(), nom, ..base });
        }
        if choix.ajout { nouveau = Some(type_par_defaut(false)); }
        if types.is_empty() {
            ui.horizontal(|ui| {
                if ui.button("+ Bandeau (relief)").clicked() { nouveau = Some(type_par_defaut(false)); }
                if ui.button("+ Engravure (creux)").clicked() { nouveau = Some(type_par_defaut(true)); }
            });
        }
        if let Some(t) = nouveau {
            match self.executer_bim(commandes::Commande::TypeModenature { type_modenature: t }) {
                Ok(ids) => self.bim.modenature.type_id = ids.first().cloned(),
                Err(e) => self.error = Some(e),
            }
        }
    }

    /// Emplacement « paramètres » : hauteur de pose (en plan), depuis la base ou le haut.
    pub(crate) fn parametres_modenature(&mut self, ui: &mut egui::Ui) {
        crate::zones::palette_creation::titre_parametres(ui, "Profilé");
        let r = &mut self.bim.modenature;
        ui.horizontal(|ui| {
            ui.label("Hauteur");
            ui.add(egui::DragValue::new(&mut r.hauteur).speed(10.0).dimensions()).on_hover_text("Hauteur de l'origine du profil, posé depuis un plan");
        });
        ui.checkbox(&mut r.depuis_haut, "Depuis le haut du mur");
        ui.label(RichText::new("Face : celle du côté cliqué. En 3D, en coupe ou en élévation, la hauteur est celle du point cliqué.").weak().small());
    }

    /// Clic sur un mur : pose un profilé sur toute sa longueur.
    pub(crate) fn interaction_modenature(&mut self, resp: &egui::Response, pick: impl FnOnce(&Self, egui::Pos2) -> Option<(u32, glam::Vec3)>) {
        if resp.clicked_by(egui::PointerButton::Secondary) { self.ruban_edition.annuler(); return; }
        if !resp.clicked() { return; }
        let Some(q) = resp.interact_pointer_pos() else { return };
        let Some((objet, point)) = pick(self, q) else { self.status = "Profilé : cliquez un mur.".into(); return };
        let Some(s) = self.scene.as_ref() else { return };
        let monde = (point.as_dvec3() + s.origin) * 1000.0;
        let Some(id) = s.objects.get(objet as usize).map(|o| o.id).and_then(|id| self.cle_native(self.cible_clic(id))) else { return };
        if let Err(e) = self.poser_modenature(&id, monde) { self.error = Some(e); }
    }

    /// Pose un profilé sur le mur `id` cliqué au point `monde` (mm).
    pub(crate) fn poser_modenature(&mut self, id: &str, monde: glam::DVec3) -> Result<(), String> {
        let p = &self.natif.as_ref().ok_or("Aucun projet natif.")?.0;
        // une ouverture ou un profilé cliqué désigne leur mur
        let id = p.ouvertures.iter().find(|o| o.id == id).map(|o| o.mur.as_str())
            .or_else(|| p.modenatures.iter().find(|m| m.id == id).map(|m| m.mur.as_str())).unwrap_or(id);
        let mur = p.murs.iter().find(|m| m.id == id).map(|m| m.assemblage.clone().unwrap_or_else(|| m.id.clone()))
            .or_else(|| p.assemblage(id).map(|a| a.id.clone())).ok_or("Profilé : cliquez un mur.")?;
        let r = &self.bim.modenature;
        let mut m = Modenature { id: String::new(), nom: "Profilé".into(), mur, type_modenature: r.type_id.clone().unwrap_or_default(), interieur: false, hauteur: r.hauteur, depuis_haut: r.depuis_haut, debut: None, fin: None };
        // face du côté cliqué
        let q = glam::DVec2::new(monde.x, monde.y);
        let ext = p.face_modenature(&m)?;
        let int = p.face_modenature(&Modenature { interieur: true, ..m.clone() })?;
        m.interieur = distance_axe(&int.0, q) < distance_axe(&ext.0, q);
        if !self.vue_en_plan() {
            let (_, _, bas, haut) = if m.interieur { int } else { ext };
            m.hauteur = ((if m.depuis_haut { haut - monde.z } else { monde.z - bas }) / 10.0).round() * 10.0;
        }
        if p.types_modenatures.is_empty() {
            self.executer_bim_suite(commandes::Commande::TypeModenature { type_modenature: type_par_defaut(false) }, |ids| {
                m.type_modenature = ids.first().cloned().unwrap_or_default();
                commandes::Commande::Modenature { modenature: m }
            })?;
        } else {
            let ids = self.executer_bim(commandes::Commande::Modenature { modenature: m })?;
            self.bim.selection = ids;
        }
        self.status = "Profilé posé — cliquez un autre mur, ou Échap pour terminer.".into();
        Ok(())
    }
}
