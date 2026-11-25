//! Modifier le profil d'un mur (Modifier → Modifier tracé sur un mur droit, en 3D, en
//! coupe ou en élévation) : le contour d'élévation se modifie avec la DAO 2D commune
//! (`edition_contour`), dans le plan vertical du mur. Repère : origine au début de l'axe
//! de référence et à la base du mur, `s` le long du mur, `z` vers le haut (mm).
use super::*;

impl Application {
    /// Commence la modification du profil du mur ou de l'assemblage `id` (une couche
    /// désigne son assemblage). Faux si la vue est un plan (le message de l'appelant le signale).
    pub(crate) fn commencer_profil_mur(&mut self, id: &str) -> bool {
        let Some((p, _)) = &self.natif else { return false };
        let cle = p.murs.iter().find(|m| m.id == id).and_then(|m| m.assemblage.clone()).unwrap_or_else(|| id.to_string());
        if self.vue_en_plan() { return false; }
        let profil = match p.profils_murs.get(&cle) {
            Some(x) => Ok(x.clone()),
            None => p.profil_rectangle(&cle),
        };
        let (profil, (_, u, _)) = match profil.and_then(|x| Ok((x, p.repere_profil(&cle)?))) {
            Ok(x) => x,
            Err(e) => { self.error = Some(e); return true; }
        };
        // mur vu par la tranche : son plan ne se dessine pas
        let n = u.perp();
        if (self.camera.forward().as_dvec3().truncate().dot(n)).abs() < 0.15 {
            self.error = Some("Le mur est vu par la tranche : tournez la vue pour modifier son profil.".into());
            return true;
        }
        let fermees = vec![true; profil.contours.len()];
        self.dessin2d.courbures = Vec::new();
        self.dessin2d.dalle = profil.contours;
        self.dessin2d.geoms = None;
        self.dessin2d.dernier_usage = Some(Usage::Surface);
        self.dessin2d.edition_contour = Some(EditionContour { id: cle, creation: None, altitude: 0.0, dessin: false, selection: None, segment: None, sommet: None, point_topo: None, commande: None, depart: None, repere: None, glisse: None, annotation: None, fermees, raccord: None, element: Some(ElementTrace::ProfilMur) });
        self.dessin2d.trace = Some(Trace::new(OutilDessin::Polyligne));
        self.ruban_edition.forme_dalle = OutilDessin::Polyligne;
        self.onglet_ruban = OngletRuban::Modifier;
        self.ruban_edition.annuler();
        self.visibility_dirty = true;
        self.coupe_dirty = true;
        self.status = "Profil du mur : glisser les sommets et les côtés, tracer des contours intérieurs pour les vides ; Valider (Entrée) ou Réinitialiser le profil.".into();
        true
    }

    /// Mur (ou assemblage) dont le profil est en cours de modification.
    pub(crate) fn profil_mur_en_cours(&self) -> Option<&str> {
        self.dessin2d.edition_contour.as_ref().filter(|e| e.element == Some(ElementTrace::ProfilMur)).map(|e| e.id.as_str())
    }

    /// Repère du tracé pendant la modification d'un profil : plan vertical du mur.
    pub(super) fn repere_profil_edition(&self) -> Option<(DVec3, DVec3, DVec3)> {
        let cle = self.profil_mur_en_cours()?;
        let (o, u, base) = self.natif.as_ref()?.0.repere_profil(cle).ok()?;
        Some((DVec3::new(o.x, o.y, base), u.extend(0.0), DVec3::Z))
    }

    /// Accrochages du profil : base et haut d'origine, bouts du mur, niveaux du projet.
    pub(super) fn geoms_profil_mur(&self) -> Option<Vec<Geom>> {
        let cle = self.profil_mur_en_cours()?;
        let p = &self.natif.as_ref()?.0;
        let (_, _, base) = p.repere_profil(cle).ok()?;
        let r = p.profil_rectangle(cle).ok()?;
        let c = r.contours.first()?;
        let (s0, s1, h) = (c[0][0], c[1][0], c[2][1]);
        let marge = (s1 - s0).max(1000.0);
        let mut g: Vec<Geom> = (0..4).map(|i| Geom::Segment([DVec2::from_array(c[i]), DVec2::from_array(c[(i + 1) % 4])])).collect();
        for n in &p.niveaux {
            let z = n.altitude - base;
            if z > -h && z < 3.0 * h { g.push(Geom::Segment([DVec2::new(s0 - marge, z), DVec2::new(s1 + marge, z)])); }
        }
        Some(g)
    }

    /// Valide le profil tracé (`Commande::ProfilMur`).
    pub(super) fn valider_profil_mur(&mut self) -> Result<(), String> {
        let cle = self.profil_mur_en_cours().ok_or("Aucun profil en cours.")?.to_string();
        if self.dessin2d.courbures.iter().flatten().any(|c| *c != 0.0) {
            return Err("Profil du mur : côtés droits seulement.".into());
        }
        // contour extérieur d'abord : le plus grand
        let mut contours = self.dessin2d.dalle.clone();
        let aire = |c: &Vec<[f64; 2]>| (0..c.len()).map(|i| { let (a, b) = (c[i], c[(i + 1) % c.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>().abs();
        contours.sort_by(|a, b| aire(b).total_cmp(&aire(a)));
        let profil = modele::murs::profil::ProfilMur { contours };
        self.executer_bim(Commande::ProfilMur { id: cle, profil: Some(profil) })?;
        self.status = "Profil du mur modifié — Ctrl+S pour enregistrer.".into();
        Ok(())
    }

    /// Rend au mur en cours son rectangle d'origine et termine la modification.
    pub(crate) fn reinitialiser_profil_mur(&mut self) {
        let Some(cle) = self.profil_mur_en_cours().map(str::to_string) else { return };
        match self.executer_bim(Commande::ProfilMur { id: cle, profil: None }) {
            Ok(_) => {
                self.dessin2d.edition_contour = None;
                self.dessin2d.trace = None;
                self.dessin2d.dalle.clear();
                self.status = "Profil du mur réinitialisé.".into();
            }
            Err(e) => self.error = Some(e),
        }
    }
}
