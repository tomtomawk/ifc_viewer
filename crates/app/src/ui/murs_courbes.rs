//! Ligne de contrôle et poignées des murs courbes, sur leur implantation réelle.
use super::*;
use glam::DVec2;
use modele::Axe;

fn points_controle(axe:Axe)->Option<[DVec2;3]> {
    let Axe::Arc{debut,fin,centre,direct}=axe else {return None};
    let (a,b,c)=(DVec2::from_array(debut),DVec2::from_array(fin),DVec2::from_array(centre));
    let depart=(a-c).to_angle();let fin=(b-c).to_angle();
    let balayage=if direct {(fin-depart).rem_euclid(std::f64::consts::TAU)}else{-(depart-fin).rem_euclid(std::f64::consts::TAU)};
    Some([a,c+DVec2::from_angle(depart+balayage/2.)*a.distance(c),b])
}

/// Déplace une poignée autour du centre existant. La poignée centrale modifie le
/// rayon ; les extrémités modifient le balayage en conservant le rayon.
fn ajuster_arc(axe: Axe, poignee: usize, curseur: DVec2, valeur: Option<f64>) -> Result<[DVec2; 3], String> {
    let Axe::Arc { debut, fin, centre, direct } = axe else { return Err("Mur courbe requis.".into()) };
    let centre = DVec2::from_array(centre);
    let debut = DVec2::from_array(debut) - centre;
    let fin = DVec2::from_array(fin) - centre;
    let rayon = debut.length();
    if rayon <= 1e-6 || poignee > 2 { return Err("Arc non valide.".into()); }
    let sens = if direct { 1.0 } else { -1.0 };
    let mut depart = debut.to_angle();
    let fin_angle = fin.to_angle();
    let mut balayage = (sens * (fin_angle - depart)).rem_euclid(std::f64::consts::TAU);
    let mut nouveau_rayon = rayon;
    if poignee == 1 {
        nouveau_rayon = valeur.unwrap_or_else(|| curseur.distance(centre));
    } else if let Some(longueur) = valeur {
        balayage = longueur / rayon;
        if poignee == 0 { depart = fin_angle - sens * balayage; }
    } else {
        let direction = curseur - centre;
        if direction.length_squared() < 1e-12 { return Err("Choisissez un point éloigné du centre.".into()); }
        if poignee == 0 { depart = direction.to_angle(); }
        let fin = if poignee == 2 { direction.to_angle() } else { fin_angle };
        balayage = (sens * (fin - depart)).rem_euclid(std::f64::consts::TAU);
    }
    if !nouveau_rayon.is_finite() || nouveau_rayon <= 0.5 || !balayage.is_finite() || !(1e-6..std::f64::consts::TAU - 1e-6).contains(&balayage) {
        return Err("Rayon ou longueur d'arc non valide.".into());
    }
    Ok([depart, depart + sens * balayage * 0.5, depart + sens * balayage].map(|a| centre + DVec2::from_angle(a) * nouveau_rayon))
}

impl ViewerApp {
    pub(super) fn arc_mur_selection(&self)->Option<(String,[DVec2;3],f64)> {
        let ids=self.selection_bim();if ids.len()!=1 {return None;}
        let (axe,z)=self.natif.as_ref()?.0.trajectoire_justification(&ids[0])?;
        Some((ids[0].clone(),points_controle(axe)?,z))
    }
    pub(super) fn souris_arc_mur(&mut self,ui:&egui::Ui,rect:egui::Rect,resp:&egui::Response,autre_outil:bool)->bool {
        if autre_outil {self.dessin2d.arc_glisse=None;return false;}
        let Some((id,points,z))=self.arc_mur_selection() else {self.dessin2d.arc_glisse=None;return false};
        if resp.drag_started_by(PointerButton::Primary) {
            if let Some(q)=ui.input(|i|i.pointer.press_origin()) {
                let poignee=points.iter().enumerate().filter_map(|(i,p)|self.ecran_plan(rect,p.to_array(),z).map(|e|(i,e.distance(q))))
                    .filter(|(_,d)|*d<=9.).min_by(|a,b|a.1.total_cmp(&b.1));
                if let Some((i,_))=poignee {
                    self.cote_arc=None;
                    self.cote_axe=None;
                    self.dessin2d.saisie.clear();
                    self.dessin2d.arc_glisse=Some((id.clone(),i,points));
                }
            }
        }
        let actif=self.dessin2d.arc_glisse.is_some();
        if ui.input(|i|i.key_pressed(egui::Key::Escape)) || resp.clicked_by(PointerButton::Secondary) {
            self.dessin2d.arc_glisse=None;self.terminer_edition_axe();return actif;
        }
        if let (Some((_,i,_)),Some(q))=(self.dessin2d.arc_glisse.clone(),resp.interact_pointer_pos().or(resp.hover_pos())) {
            let saisie = std::mem::take(&mut self.dessin2d.saisie);
            let fixe=points[if i==0 {2}else{0}].to_array();
            let position=self.point_edition_axe(ui,rect,q,ui.input(|i|i.modifiers.shift),fixe,fixe);
            self.dessin2d.saisie = saisie;
            let valeur = self.dessin2d.saisie.replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0).map(|v| v * crate::config::options::unite().facteur());
            let axe = self.natif.as_ref().and_then(|(p,_)|p.trajectoire_justification(&id)).map(|(axe,_)|axe);
            if let (Some(axe), Some(position)) = (axe, position)
                && let Ok(nouveaux) = ajuster_arc(axe, i, DVec2::from_array(position), valeur)
                && let Some((_,_,pts))=&mut self.dessin2d.arc_glisse { *pts=nouveaux; }
        }
        if resp.drag_stopped_by(PointerButton::Primary) {
            if let Some((id,i,pts))=self.dessin2d.arc_glisse.take() {
                let valeur = if self.dessin2d.saisie.is_empty() { None } else { self.dessin2d.saisie.replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0).map(|v| v * crate::config::options::unite().facteur()) };
                let valide = if self.dessin2d.saisie.is_empty() { true } else if let (Some(valeur), Some((axe,_))) = (valeur, self.natif.as_ref().and_then(|(p,_)|p.trajectoire_justification(&id))) { ajuster_arc(axe,i,pts[i],Some(valeur)).is_ok() } else { false };
                self.terminer_edition_axe();
                if !valide {self.error=Some("Saisissez une valeur positive et finie.".into());return actif;}
                match self.executer_bim(crate::edition::commandes::Commande::ModifierTrajectoireMur{id:id.clone(),points:pts.map(|p|p.to_array())}) {
                    Ok(_)=>{
                        if let Some((_,r,_,s))=noyau::forme2d::arc_3_points(pts[0],pts[1],pts[2]) {
                            let facteur=crate::config::options::unite().facteur();
                            let formater=|v|noyau::nombres::format_real(v / facteur);
                            self.cote_arc=Some((id,i,formater(r),formater(r*s.abs())));
                        }
                        self.status="Mur courbe modifié : cliquer une cote pour la saisir.".into();
                    },Err(e)=>self.error=Some(e)
                }
            }
        }
        actif
    }
    pub(super) fn dessiner_axes_courbes(&mut self,ui:&mut egui::Ui,rect:egui::Rect,survol:Option<egui::Pos2>) {
        let Some((p,_))=&self.natif else{return};
        let ids=self.selection_bim();
        let painter=ui.painter().with_clip_rect(rect);let bleu=Color32::from_rgb(40,120,210);
        let mut modifier = None;
        for id in &ids {
            let Some((axe,z))=p.trajectoire_justification(id) else {continue};
            let Some(mut points)=points_controle(axe) else {continue};
            if let Some((gid,_,pts))=&self.dessin2d.arc_glisse {if gid==id {points=*pts;}}
            let Some((o,r,d,s))=noyau::forme2d::arc_3_points(points[0],points[1],points[2]) else {continue};
            let ligne:Vec<_>=noyau::forme2d::points_arc(o,r,d,s).into_iter().filter_map(|p|self.ecran_plan(rect,p.to_array(),z)).collect();
            painter.add(egui::Shape::dashed_line(&ligne,egui::Stroke::new(1.5,bleu),8.,4.));
            if ids.len()==1 {
                for p in points {
                    if let Some(e)=self.ecran_plan(rect,p.to_array(),z) {
                        let hover=survol.is_some_and(|q|q.distance(e)<=9.);
                        painter.circle(e,if hover{6.5}else{5.},Color32::WHITE,egui::Stroke::new(2.,bleu));
                        if hover {ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);}
                    }
                }
                if self.dessin2d.arc_glisse.is_none() {
                    let facteur = crate::config::options::unite().facteur();
                    let rayon_actuel = noyau::nombres::format_real(r / facteur);
                    let longueur_actuelle = noyau::nombres::format_real(r * s.abs() / facteur);
                    if self.cote_arc.as_ref().is_none_or(|(mur, _, _, _)| mur != id) {
                        self.cote_arc = Some((id.clone(), 2, rayon_actuel, longueur_actuelle));
                    } else if let Some((_, _, rayon, longueur)) = &mut self.cote_arc {
                        if !ui.memory(|m| m.has_focus(ui.make_persistent_id(("cote_arc", id, 0)))) { *rayon = rayon_actuel; }
                        if !ui.memory(|m| m.has_focus(ui.make_persistent_id(("cote_arc", id, 1)))) { *longueur = longueur_actuelle; }
                    }
                }
                if let Some((gid, poignee, _)) = &self.dessin2d.arc_glisse && gid == id {
                    if let Some(e) = self.ecran_plan(rect, points[*poignee].to_array(), z) {
                        let unite = crate::config::options::unite();
                        let (nom, valeur) = if *poignee == 1 { ("Rayon", r) } else { ("Longueur d'arc", r * s.abs()) };
                        let texte = if self.dessin2d.saisie.is_empty() { unite.afficher(valeur) } else { format!("{}▏{}", self.dessin2d.saisie, unite.suffixe()) };
                        let g = painter.layout_no_wrap(format!("{nom} : {texte}"), egui::FontId::proportional(13.0), Color32::from_gray(30));
                        let cadre = egui::Rect::from_min_size(e + egui::vec2(14.0, -28.0), g.size() + egui::vec2(10.0, 4.0));
                        painter.rect(cadre, 3.0, Color32::from_white_alpha(235), egui::Stroke::new(1.0, bleu), egui::StrokeKind::Inside);
                        painter.galley(cadre.min + egui::vec2(5.0, 2.0), g, Color32::from_gray(30));
                    }
                } else if self.cote_arc.as_ref().is_some_and(|(mur, _, _, _)| mur == id)
                    && let Some(e) = self.ecran_plan(rect, points[1].to_array(), z)
                {
                    let (_, poignee, rayon, longueur) = self.cote_arc.as_mut().unwrap();
                    let poignee = *poignee;
                    for (k, (nom, texte)) in [("Rayon", rayon), ("Longueur", longueur)].into_iter().enumerate() {
                        let zone = egui::Rect::from_min_size(e + egui::vec2(14.0, -48.0 + k as f32 * 24.0), egui::vec2(95.0, 22.0));
                        let reponse = ui.put(zone, egui::TextEdit::singleline(texte).id(ui.make_persistent_id(("cote_arc", id, k))).desired_width(95.0))
                            .on_hover_text(format!("{nom} ({}) : saisir une valeur puis valider", crate::config::options::unite().suffixe().trim()));
                        if reponse.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            modifier = Some((id.clone(), axe, if k == 0 { 1 } else if poignee == 0 { 0 } else { 2 }, texte.clone()));
                        }
                        painter.text(zone.right_center() + egui::vec2(4.0, 0.0), egui::Align2::LEFT_CENTER, format!("{nom} ({})", crate::config::options::unite().suffixe().trim()), egui::FontId::proportional(11.0), bleu);
                    }
                }
            }
        }
        if let Some((id, axe, poignee, texte)) = modifier {
            let valeur = texte.trim().replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0).map(|v| v * crate::config::options::unite().facteur());
            let resultat = valeur.ok_or_else(|| "Saisissez une valeur positive et finie.".to_string())
                .and_then(|valeur| ajuster_arc(axe, poignee, DVec2::ZERO, Some(valeur)))
                .and_then(|points| self.executer_bim(crate::edition::commandes::Commande::ModifierTrajectoireMur { id: id.clone(), points: points.map(|p|p.to_array()) }));
            match resultat {
                Ok(_) => {
                    if let Some((axe, _)) = self.natif.as_ref().and_then(|(p,_)|p.trajectoire_justification(&id))
                        && let Some(points) = points_controle(axe)
                        && let Some((_,r,_,s)) = noyau::forme2d::arc_3_points(points[0],points[1],points[2]) {
                        let facteur = crate::config::options::unite().facteur();
                        let derniere_extremite = self.cote_arc.as_ref().map_or(poignee, |(_, ancienne, _, _)| *ancienne);
                        self.cote_arc = Some((id, derniere_extremite, noyau::nombres::format_real(r/facteur), noyau::nombres::format_real(r*s.abs()/facteur)));
                    }
                    self.status="Mur courbe modifié.".into();
                }
                Err(e) => self.error=Some(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn poignees_conservent_le_centre_et_acceptent_les_cotes() {
        for direct in [true, false] {
            let axe=Axe::Arc{debut:[6000.,-2000.],fin:[1000.,3000.],centre:[1000.,-2000.],direct};
            let centre=DVec2::new(1000.,-2000.);
            let rayon=5000.0;
            let points=ajuster_arc(axe,2,DVec2::new(10000.,10000.),None).unwrap();
            let (o,r,_,_)=noyau::forme2d::arc_3_points(points[0],points[1],points[2]).unwrap();
            assert!(o.distance(centre)<1e-6 && (r-rayon).abs()<1e-6);
            let points=ajuster_arc(axe,1,DVec2::ZERO,Some(7000.)).unwrap();
            let (o,r,_,_)=noyau::forme2d::arc_3_points(points[0],points[1],points[2]).unwrap();
            assert!(o.distance(centre)<1e-6 && (r-7000.).abs()<1e-6);
            for poignee in [0,2] {
                let points=ajuster_arc(axe,poignee,DVec2::ZERO,Some(6000.)).unwrap();
                let (o,r,_,s)=noyau::forme2d::arc_3_points(points[0],points[1],points[2]).unwrap();
                assert!(o.distance(centre)<1e-6 && (r*s.abs()-6000.).abs()<1e-6);
                let fixe=if poignee==0 {points[2]}else{points[0]};
                let ancien=if poignee==0 {DVec2::from_array([1000.,3000.])}else{DVec2::from_array([6000.,-2000.])};
                assert!(fixe.distance(ancien)<1e-6);
            }
            assert!(ajuster_arc(axe,2,DVec2::ZERO,Some(rayon*std::f64::consts::TAU)).is_err());
        }
    }

    #[test]
    fn modification_arc_conserve_implantation_couches_et_transaction_atomique() {
        use {crate::edition::{Execution,commandes::Commande},modele::{Projet,creation::{ParametresMur,Implantation}}};
        let mut p=Projet::depuis_gabarit("Arc",&modele::gabarit::Gabarit::defaut());
        let type_mur=p.types_murs.iter().find(|t|t.couches.len()>1).unwrap().id.clone();
        let options=ParametresMur{type_mur:Some(type_mur),implantation:Implantation::Exterieure,..Default::default()};
        let id=p.ajouter_mur_axe(Axe::Arc{debut:[5000.,0.],fin:[0.,5000.],centre:[0.,0.],direct:true},&options).unwrap();
        let (axe,_)=p.trajectoire_justification(&id).unwrap();
        let mut points=points_controle(axe).unwrap();
        assert!((points[0]-DVec2::new(5000.,0.)).length()<0.01);
        points[1]=DVec2::new(4000.,4000.);
        let original=p.json().unwrap();
        let (q,_)=p.executer(Commande::ModifierTrajectoireMur{id:id.clone(),points:points.map(|p|p.to_array())}).unwrap();
        q.verifier().unwrap();
        assert_eq!(p.json().unwrap(),original);
        assert_eq!(p.couches_assemblage(&id).len(),q.couches_assemblage(&id).len());
        let (axe,_)=q.trajectoire_justification(&id).unwrap();
        let controle=points_controle(axe).unwrap();
        assert!((controle[0]-points[0]).length()<0.1 && (controle[2]-points[2]).length()<0.1);
        assert!(p.executer(Commande::ModifierTrajectoireMur{id,points:[[0.,0.],[1.,0.],[2.,0.]]}).is_err());
        assert_eq!(p.json().unwrap(),original);
    }
}
