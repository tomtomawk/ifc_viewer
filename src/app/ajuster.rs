//! Ajuster/prolonger graphique : une limite, puis les extrémités visées.
use super::*;
use crate::modele::{Projet, commandes::Commande};

#[derive(Default)]
pub(super) struct EtatAjustement {
    pub reference: Option<String>,
    fin_reference: bool,
    survol: Option<(String, bool)>,
    cache: Option<(u64, String, String, bool, bool, Option<bool>)>,
    apercu: Option<Result<Apercu, String>>,
}
struct Apercu {
    id: String,
    fin: bool,
    a: [f64; 2],
    b: [f64; 2],
    ancien: [f64; 2],
    point: [f64; 2],
    z: f64,
    contours: Vec<Vec<[f64; 2]>>,
    angle: Option<(bool, [f64; 2], [f64; 2], f64)>,
}

fn preparer(
    p: &Projet,
    id: &str,
    reference: &str,
    fin: bool,
    prolonger: bool,
    angle: Option<bool>,
) -> Result<Apercu, String> {
    let (a, b, z) = p
        .axe_element(id)
        .ok_or("Choisissez un mur droit ou un assemblage droit.")?;
    let point = p.cible_ajustement(id, reference, fin)?;
    let ancien = if fin { b } else { a };
    let fixe = if fin { a } else { b };
    let direction = glam::DVec2::from_array(ancien) - glam::DVec2::from_array(fixe);
    if prolonger
        && (glam::DVec2::from_array(point) - glam::DVec2::from_array(ancien)).dot(direction) <= 1e-6
    {
        return Err(
            "Cette limite raccourcit le mur. Choisissez le mode « Ajuster ou prolonger ».".into(),
        );
    }
    // La prévisualisation utilise la même transaction validée que le clic final.
    let commande=commande_ajuster(id,reference,fin,angle);
    if prolonger && angle.is_some() {
        let (ra,rb,_)=p.axe_element(reference).ok_or("Référence absente")?;
        let (ancien,fixe)=if angle==Some(true){(rb,ra)}else{(ra,rb)};
        if (glam::DVec2::from_array(point)-glam::DVec2::from_array(ancien))
            .dot(glam::DVec2::from_array(ancien)-glam::DVec2::from_array(fixe)) < -1e-6 {
            return Err("La limite serait raccourcie : choisissez « Ajuster ou prolonger ».".into());
        }
    }
    let (q, _) = p.executer(commande)?;
    let (a, b, _) = q.axe_element(id).ok_or("Axe ajusté absent")?;
    let coupes = crate::modele::jonctions::coupes(&q);
    let contours = q
        .murs
        .iter()
        .filter(|m| m.id == id || m.assemblage.as_deref() == Some(id) || (angle.is_some() && (m.id==reference || m.assemblage.as_deref()==Some(reference))))
        .filter_map(|m| crate::modele::jonctions::contour(&q, m, &coupes).map(|c| c.to_vec()))
        .collect();
    Ok(Apercu {
        id: id.into(),
        fin,
        a,
        b,
        ancien,
        point,
        z,
        contours,
        angle: angle.map(|fin| {let (a,b,z)=q.axe_element(reference).unwrap();(fin,a,b,z)}),
    })
}

fn commande_ajuster(id:&str,reference:&str,fin:bool,angle:Option<bool>)->Commande {
    match angle {
        Some(fin_reference)=>Commande::AjusterAngle{id:id.into(),reference:reference.into(),fin,fin_reference},
        None=>Commande::Ajuster{id:id.into(),reference:reference.into(),fin}
    }
}

impl ViewerApp {
    pub(super) fn etat_ajustement(&self)->serde_json::Value {
        let a=self.ajustement.as_ref();
        let apercu=a.and_then(|a|a.apercu.as_ref());
        serde_json::json!({"actif":self.ruban_edition.outil()==Some(crate::ruban::Outil::Ajuster),"angle":self.ruban_edition.prolonger_angle,"reference":a.and_then(|a|a.reference.as_ref()),"survol":a.and_then(|a|a.survol.as_ref()),
            "apercu":apercu.and_then(|r|r.as_ref().ok()).map(|p|serde_json::json!({"id":p.id,"fin":p.fin,"point":p.point})),
            "erreur":apercu.and_then(|r|r.as_ref().err())})
    }
    pub(super) fn preparer_outil_ajuster(&mut self) -> bool {
        let actif = self.ruban_edition.outil() == Some(crate::ruban::Outil::Ajuster)
            && self.natif.is_some()
            && self.loading.is_none();
        if actif {
            let etat = self.ajustement.get_or_insert_with(Default::default);
            if etat
                .reference
                .as_ref()
                .is_some_and(|id| self.natif.as_ref().unwrap().0.axe_element(id).is_none())
            {
                *etat = Default::default();
            }
        } else {
            self.ajustement = None;
        }
        actif
    }

    fn mur_ajuster_vise(
        &self,
        rect: egui::Rect,
        q: egui::Pos2,
        hit: Option<u32>,
    ) -> Option<(String, bool)> {
        let p = &self.natif.as_ref()?.0;
        let s = self.scene.as_ref()?;
        let candidat = |id: String| -> Option<(String, bool)> {
            let (a, b, z) = p.axe_element(&id)?;
            let (a, b) = (self.ecran_plan(rect, a, z)?, self.ecran_plan(rect, b, z)?);
            Some((id, q.distance(b) <= q.distance(a)))
        };
        if let Some(id) = hit
            .and_then(|i| s.objects.get(i as usize))
            .and_then(|o| self.cle_native(o.id))
        {
            if let Some(c) = candidat(id) {
                return Some(c);
            }
        }
        if !self.point_dans_cadrage(rect, q) {
            return None;
        }
        // Tolérance sur l'axe pour les vues au trait et les murs très fins.
        p.murs
            .iter()
            .filter(|m| m.assemblage.is_none())
            .map(|m| (&m.id, "mur"))
            .chain(p.assemblages.iter().map(|a| (&a.id, "assemblage")))
            .filter_map(|(id, genre)| {
                if !self
                    .objects_under(scene::id_natif(genre, id))
                    .iter()
                    .any(|i| self.visible.get(*i as usize).copied().unwrap_or(false))
                {
                    return None;
                }
                let (a, b, z) = p.axe_element(id)?;
                let (a, b) = (self.ecran_plan(rect, a, z)?, self.ecran_plan(rect, b, z)?);
                let d = b - a;
                let t = if d.length_sq() > 0.0 {
                    ((q - a).dot(d) / d.length_sq()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let distance = q.distance(a + t * d);
                (distance <= 12.0).then_some((distance, id.clone()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .and_then(|(_, id)| candidat(id))
    }

    pub(super) fn souris_ajuster(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        resp: &egui::Response,
        hit: Option<u32>,
    ) {
        if resp.clicked_by(PointerButton::Secondary) {
            if self
                .ajustement
                .as_ref()
                .is_some_and(|a| a.reference.is_some())
            {
                self.ajustement = Some(Default::default());
            } else {
                self.ruban_edition.annuler();
                self.ajustement = None;
            }
            return;
        }
        let survol = resp
            .hover_pos()
            .or(resp.interact_pointer_pos())
            .and_then(|q| self.mur_ajuster_vise(rect, q, hit));
        let prolonger = self
            .ruban_edition
            .texte_option(crate::ruban::Outil::Ajuster, 0)
            == "Prolonger seulement";
        let angle=self.ruban_edition.prolonger_angle;
        let Some(etat) = self.ajustement.as_mut() else {
            return;
        };
        etat.survol = survol.clone();
        if let (Some(reference), Some((id, fin))) = (&etat.reference, &survol) {
            let cle = (
                self.bim.revision,
                reference.clone(),
                id.clone(),
                *fin,
                prolonger,
                angle.then_some(etat.fin_reference),
            );
            if etat.cache.as_ref() != Some(&cle) {
                etat.apercu = Some(preparer(
                    &self.natif.as_ref().unwrap().0,
                    id,
                    reference,
                    *fin,
                    prolonger,
                    angle.then_some(etat.fin_reference),
                ));
                etat.cache = Some(cle);
            }
        } else {
            etat.apercu = None;
            etat.cache = None;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        if !resp.clicked_by(PointerButton::Primary) {
            return;
        }
        if etat.reference.is_none() {
            if let Some((id, fin)) = survol {
                etat.fin_reference = fin;
                etat.reference = Some(id);
                etat.cache = None;
                self.status = if angle {
                    "Première extrémité choisie : cliquez près de la seconde pour raccorder les deux murs."
                } else {
                    "Limite choisie : cliquez près de l'extrémité du mur à ajuster ou prolonger."
                }.into();
            } else {
                self.status =
                    "Ajuster / prolonger : cliquez un mur droit pour choisir son axe comme limite."
                        .into();
            }
            return;
        }
        let commande = match &etat.apercu {
            Some(Ok(a)) => Some(commande_ajuster(&a.id,etat.reference.as_deref().unwrap(),a.fin,a.angle.map(|a|a.0))),
            Some(Err(e)) => {
                self.status = e.clone();
                None
            }
            None => {
                self.status = "Cliquez le mur à ajuster, près de l'extrémité à déplacer.".into();
                None
            }
        };
        if let Some(c) = commande {
            match self.executer_bim(c) {
                Ok(_) => {
                    self.status=if angle {"Angle créé — cliquez près de la première extrémité du prochain raccord."}else{"Mur ajusté — choisissez une autre extrémité, ou Échap pour changer de limite."}.into();
                    if angle {self.ajustement=Some(Default::default());}
                }
                Err(e) => self.error = Some(e),
            }
            if let Some(e) = &mut self.ajustement {
                e.cache = None;
                e.apercu = None;
            }
        }
    }

    pub(super) fn dessiner_ajustement(&self, ui: &egui::Ui, rect: egui::Rect) {
        let Some(e) = &self.ajustement else { return };
        let Some((p, _)) = &self.natif else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let orange = Color32::from_rgb(220, 125, 10);
        let bleu = Color32::from_rgb(0, 125, 215);
        if self.ruban_edition.prolonger_angle {
            if let Some((a,b,z))=e.reference.as_deref().and_then(|id|p.axe_element(id)) {
                if let Some(q)=self.ecran_plan(rect,if e.fin_reference {b}else{a},z) {
                    painter.circle_stroke(q,7.0,egui::Stroke::new(2.5,orange));
                }
            }
        }
        let axe = |id: &str, couleur: Color32, infini: bool| {
            if let Some((a, b, z)) = p.axe_element(id) {
                if let (Some(a), Some(b)) =
                    (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z))
                {
                    if infini {
                        let u = (b - a).normalized();
                        painter.add(egui::Shape::dashed_line(
                            &[a - u * rect.size().length(), b + u * rect.size().length()],
                            egui::Stroke::new(1.5, couleur),
                            7.0,
                            4.0,
                        ));
                    }
                    painter.line_segment([a, b], egui::Stroke::new(3.0, couleur));
                }
            }
        };
        if let Some(id) = &e.reference {
            axe(id, orange, true);
        }
        if let Some((id, _)) = &e.survol {
            if e.reference.as_ref() != Some(id) {
                axe(id, if e.reference.is_none() { orange } else { bleu }, false);
            }
        }
        match &e.apercu {
            Some(Ok(a)) => {
                if let Some((_,x,y,z))=a.angle {
                    if let (Some(x),Some(y))=(self.ecran_plan(rect,x,z),self.ecran_plan(rect,y,z)) {
                        painter.line_segment([x,y],egui::Stroke::new(2.5,bleu));
                    }
                }
                for c in &a.contours {
                    let pts: Vec<_> = c
                        .iter()
                        .filter_map(|p| self.ecran_plan(rect, *p, a.z))
                        .collect();
                    if pts.len() >= 3 {
                        painter.add(egui::Shape::convex_polygon(
                            pts,
                            bleu.gamma_multiply(0.2),
                            egui::Stroke::new(1.5, bleu),
                        ));
                    }
                }
                if let (Some(x), Some(y), Some(ancien), Some(point)) = (
                    self.ecran_plan(rect, a.a, a.z),
                    self.ecran_plan(rect, a.b, a.z),
                    self.ecran_plan(rect, a.ancien, a.z),
                    self.ecran_plan(rect, a.point, a.z),
                ) {
                    painter.line_segment([x, y], egui::Stroke::new(2.5, bleu));
                    painter.add(egui::Shape::dashed_line(
                        &[ancien, point],
                        egui::Stroke::new(2.0, Color32::from_rgb(210, 70, 55)),
                        5.0,
                        3.0,
                    ));
                    painter.circle(point, 6.0, Color32::WHITE, egui::Stroke::new(2.0, bleu));
                }
            }
            Some(Err(erreur)) => {
                if let Some(q) = ui
                    .input(|i| i.pointer.hover_pos())
                    .filter(|q| rect.contains(*q))
                {
                    painter.text(
                        q + egui::vec2(15.0, 20.0),
                        egui::Align2::LEFT_TOP,
                        erreur,
                        egui::FontId::proportional(12.0),
                        Color32::from_rgb(185, 55, 40),
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn angle_raccorde_deux_extremites_en_une_transaction() {
        let mut p=Projet::depuis_gabarit("Angle", &crate::gabarit::Gabarit::defaut());
        let id=p.ajouter_mur([0.,0.],[2000.,0.],&Default::default()).unwrap();
        let reference=p.ajouter_mur([4000.,-3000.],[4000.,-1000.],&Default::default()).unwrap();
        let apercu=preparer(&p,&id,&reference,true,true,Some(true)).unwrap();
        assert_eq!(apercu.point,[4000.,0.]);
        assert_eq!(apercu.angle.unwrap().2,[4000.,0.]);
        assert_eq!(p.axe_element(&reference).unwrap().1,[4000.,-1000.]);
        let (q,ids)=p.executer(commande_ajuster(&id,&reference,true,Some(true))).unwrap();
        assert_eq!(ids,vec![id.clone(),reference.clone()]);
        assert_eq!(q.axe_element(&id).unwrap().1,q.axe_element(&reference).unwrap().1);
        assert!(preparer(&p,&id,&reference,true,false,Some(false)).is_err());
        assert_eq!(p.axe_element(&id).unwrap().1,[2000.,0.]);
    }
    #[test]
    fn apercu_valide_prolongement_raccourcissement_et_rejets_sans_mutation() {
        let mut p = Projet::depuis_gabarit("Ajuster", &crate::gabarit::Gabarit::defaut());
        let cible = p
            .ajouter_mur([0.0, 0.0], [2000.0, 0.0], &Default::default())
            .unwrap();
        let limite = p
            .ajouter_mur([4000.0, -2000.0], [4000.0, 2000.0], &Default::default())
            .unwrap();
        let a = preparer(&p, &cible, &limite, true, false, None).unwrap();
        assert_eq!(a.point, [4000.0, 0.0]);
        assert_eq!(p.axe_element(&cible).unwrap().1, [2000.0, 0.0]);
        assert!(preparer(&p, &cible, &limite, false, false, None).is_err());
        let (q, _) = p
            .executer(Commande::Ajuster {
                id: cible.clone(),
                reference: limite.clone(),
                fin: true,
            })
            .unwrap();
        assert_eq!(q.axe_element(&cible).unwrap().1, a.point);
        let proche = p
            .ajouter_mur([1000.0, -2000.0], [1000.0, 2000.0], &Default::default())
            .unwrap();
        assert_eq!(
            preparer(&p, &cible, &proche, true, false, None).unwrap().point,
            [1000.0, 0.0]
        );
        assert!(preparer(&p, &cible, &proche, true, true, None).is_err());
        assert!(preparer(&p, &cible, &cible, true, false, None).is_err());
        let parallele = p
            .ajouter_mur([0.0, 3000.0], [3000.0, 3000.0], &Default::default())
            .unwrap();
        assert!(preparer(&p, &cible, &parallele, true, false, None).is_err());
    }
}
