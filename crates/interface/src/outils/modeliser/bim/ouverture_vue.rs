//! Visée et aperçu de la baie avant la pose d’une ouverture.
use super::*;
use commandes::Commande;
use modele::volumes::solides;

pub(crate) struct PoseOuverture {
    commande: Commande,
    geometrie: GeometriePose,
}

struct GeometriePose {
    contour: Vec<[f64; 2]>,
    bas: f64,
    haut: f64,
    tient: bool,
}

impl GeometriePose {
    fn depuis_commande(p: &modele::Projet, commande: &Commande) -> Result<Self, String> {
        let Commande::Ouverture {
            mur,
            abscisse,
            type_ouverture,
            largeur,
            hauteur,
            allege,
            ..
        } = commande
        else {
            unreachable!("Géométrie d’une commande de pose d’ouverture");
        };
        let (axe, epaisseur, niveau) = solides::hote(p, mur)?;
        let type_choisi = type_ouverture
            .as_ref()
            .map(|id| {
                p.types_ouvertures
                    .iter()
                    .find(|t| &t.id == id)
                    .ok_or("Type d’ouverture absent.")
            })
            .transpose()?;
        let (largeur, hauteur) =
            type_choisi.map_or((*largeur, *hauteur), |t| (t.largeur, t.hauteur));
        let debut = abscisse - largeur / 2.0;
        let fin = abscisse + largeur / 2.0;
        let bas = p
            .niveau(&niveau)
            .ok_or("Niveau de l’ouverture absent.")?
            .altitude
            + allege;
        let haut = bas + hauteur;
        if !haut.is_finite() || !bas.is_finite() || haut <= bas {
            return Err("Hauteur d’ouverture non valide.".into());
        }
        Ok(Self {
            contour: solides::bande(&axe, debut, fin, epaisseur)?,
            bas,
            haut,
            tient: debut >= 0.0 && fin <= axe.longueur(),
        })
    }
}

impl Application {
    pub(crate) fn interaction_ouverture(
        &mut self,
        resp: &egui::Response,
        pick: impl FnOnce(&Self, egui::Pos2) -> Option<(u32, Vec3)>,
    ) -> Option<PoseOuverture> {
        if self.vol.actif {
            return None;
        }
        let clic = resp.clicked_by(PointerButton::Primary);
        let point = if clic {
            resp.interact_pointer_pos()
        } else {
            resp.hover_pos()
        }?;
        let (objet, q) = pick(self, point)?;
        let s = self.scene.as_ref()?;
        let monde = q.as_dvec3() + s.origin;
        let objet = self.cible_clic(s.objects.get(objet as usize)?.id);
        let pose = self.viser_ouverture(objet, [monde.x * 1000.0, monde.y * 1000.0]);
        if clic {
            self.point_clique = Some(monde);
            if let Err(e) = pose.and_then(|p| self.executer_bim(p.commande)) {
                self.error = Some(e);
            }
            None
        } else {
            pose.ok()
        }
    }

    /// Même abscisse et même commande pour l’aperçu et le clic, y compris si
    /// la souris vise une couche d’un mur composé.
    fn viser_ouverture(&self, objet: Id, point: [f64; 2]) -> Result<PoseOuverture, String> {
        let mut id = self.cle_native(objet).ok_or("Sélectionnez un mur.")?;
        let p = &self.natif.as_ref().ok_or("Aucun projet ouvert.")?.0;
        if let Some(a) = p
            .murs
            .iter()
            .find(|m| m.id == id)
            .and_then(|m| m.assemblage.clone())
        {
            id = a;
        }
        let (axe, _, _) = solides::hote(p, &id)?;
        self.verifier_type_ouverture()?;
        let commande = self.commande_ouverture(id, axe.abscisse(point));
        let geometrie = GeometriePose::depuis_commande(p, &commande)?;
        Ok(PoseOuverture {
            commande,
            geometrie,
        })
    }

    pub(crate) fn apercu_ouverture(&self, ui: &egui::Ui, rect: egui::Rect, pose: &PoseOuverture) {
        let g = &pose.geometrie;
        let couleur = if g.tient {
            Color32::from_rgb(38, 164, 230)
        } else {
            Color32::from_rgb(236, 79, 68)
        };
        let painter = ui.painter().with_clip_rect(rect);
        let contour = egui::Stroke::new(2.0, couleur);
        let projeter = |z| {
            g.contour
                .iter()
                .map(|&xy| self.ecran_plan(rect, xy, z))
                .collect::<Option<Vec<_>>>()
        };
        let Some(bas) = projeter(g.bas) else {
            return;
        };
        let n = bas.len();
        if self.vue_en_plan() {
            let remplissage = couleur.gamma_multiply(0.28);
            for i in 0..n / 2 - 1 {
                let coins = vec![bas[i], bas[i + 1], bas[n - 2 - i], bas[n - 1 - i]];
                painter.add(egui::Shape::convex_polygon(
                    coins,
                    remplissage,
                    egui::Stroke::NONE,
                ));
            }
        } else {
            let Some(haut) = projeter(g.haut) else {
                return;
            };
            for i in [0, n / 2 - 1, n / 2, n - 1] {
                painter.line_segment([bas[i], haut[i]], contour);
            }
            painter.add(egui::Shape::closed_line(haut, contour));
        }
        painter.add(egui::Shape::closed_line(bas, contour));
    }
}
