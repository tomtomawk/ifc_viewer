//! Ligne de contrôle et poignées des murs courbes, sur leur implantation réelle.
use super::*;
use glam::DVec2;
use crate::modele::Axe;

fn points_controle(axe:Axe)->Option<[DVec2;3]> {
    let Axe::Arc{debut,fin,centre,direct}=axe else {return None};
    let (a,b,c)=(DVec2::from_array(debut),DVec2::from_array(fin),DVec2::from_array(centre));
    let depart=(a-c).to_angle();let fin=(b-c).to_angle();
    let balayage=if direct {(fin-depart).rem_euclid(std::f64::consts::TAU)}else{-(depart-fin).rem_euclid(std::f64::consts::TAU)};
    Some([a,c+DVec2::from_angle(depart+balayage/2.)*a.distance(c),b])
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
                if let Some((i,_))=poignee {self.dessin2d.arc_glisse=Some((id,i,points));}
            }
        }
        let actif=self.dessin2d.arc_glisse.is_some();
        if ui.input(|i|i.key_pressed(egui::Key::Escape)) || resp.clicked_by(PointerButton::Secondary) {
            self.dessin2d.arc_glisse=None;self.terminer_edition_axe();return actif;
        }
        if let (Some((_,i,pts)),Some(q))=(self.dessin2d.arc_glisse.clone(),resp.interact_pointer_pos().or(resp.hover_pos())) {
            let fixe=pts[if i==0 {2}else{0}].to_array();
            if let Some(p)=self.point_edition_axe(ui,rect,q,ui.input(|i|i.modifiers.shift),fixe) {
                if let Some((_,i,pts))=&mut self.dessin2d.arc_glisse {pts[*i]=DVec2::from_array(p);}
            }
        }
        if resp.drag_stopped_by(PointerButton::Primary) {
            if let Some((id,_,pts))=self.dessin2d.arc_glisse.take() {
                self.terminer_edition_axe();
                match self.executer_bim(crate::modele::edition::commandes::Commande::ModifierTrajectoireMur{id,points:pts.map(|p|p.to_array())}) {
                    Ok(_)=>self.status="Trajectoire courbe modifiée — Ctrl+Z pour annuler.".into(),Err(e)=>self.error=Some(e)
                }
            }
        }
        actif
    }
    pub(super) fn dessiner_axes_courbes(&self,ui:&egui::Ui,rect:egui::Rect,survol:Option<egui::Pos2>) {
        let Some((p,_))=&self.natif else{return};
        let ids=self.selection_bim();
        let painter=ui.painter().with_clip_rect(rect);let bleu=Color32::from_rgb(40,120,210);
        for id in &ids {
            let Some((axe,z))=p.trajectoire_justification(id) else {continue};
            let Some(mut points)=points_controle(axe) else {continue};
            if let Some((gid,_,pts))=&self.dessin2d.arc_glisse {if gid==id {points=*pts;}}
            let Some((o,r,d,s))=crate::dessin::dao::arc_3_points(points[0],points[1],points[2]) else {continue};
            let ligne:Vec<_>=crate::dessin::dao::points_arc(o,r,d,s).into_iter().filter_map(|p|self.ecran_plan(rect,p.to_array(),z)).collect();
            painter.add(egui::Shape::dashed_line(&ligne,egui::Stroke::new(1.5,bleu),8.,4.));
            if ids.len()==1 {
                for p in points {
                    if let Some(e)=self.ecran_plan(rect,p.to_array(),z) {
                        let hover=survol.is_some_and(|q|q.distance(e)<=9.);
                        painter.circle(e,if hover{6.5}else{5.},Color32::WHITE,egui::Stroke::new(2.,bleu));
                        if hover {ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modification_arc_conserve_implantation_couches_et_transaction_atomique() {
        use crate::modele::{Projet,edition::{ParametresMur,Implantation,commandes::Commande}};
        let mut p=Projet::depuis_gabarit("Arc",&crate::config::gabarit::Gabarit::defaut());
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
