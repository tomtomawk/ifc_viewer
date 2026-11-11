//! Finalisation et propriétés communes des formes à profil, quel que soit le canevas.
use super::*;
use crate::modele::famille::modelee::{FormeGuidee, Revolution};

impl OutilFamille {
    pub(super) fn est_forme(self) -> bool {
        matches!(self, Self::Extrusion | Self::ExtrusionVide | Self::Revolution | Self::RevolutionVide | Self::ExtrusionChemin | Self::Raccordement | Self::RaccordementChemin)
    }
}

impl EditeurFamille {
    /// Ne consomme le tracé qu'après validation ; une erreur reste corrigeable.
    pub(super) fn terminer_forme(&mut self, construction: Option<PlanConstruction>) -> Result<(), String> {
        let resolution = self.resoudre();
        let contour: Vec<_> = self.esquisse.iter().map(|(p,_)| {
            let (h,v) = self.famille.point_esquisse(&resolution,p); [h,v]
        }).collect();
        crate::modele::famille::profil::contour_simple(&contour)?;
        let revolution = if self.outil.est_revolution() {
            let r = Revolution::nouvelle(self.axe_revolution.ok_or("Définir l'axe avant le profil.")?);
            r.valider(&contour)?;
            Some(r)
        } else { None };
        let hauteur = 1000.0;
        let chemin = vec![[0.0;3],[0.0,0.0,hauteur]];
        let guidage = match self.outil {
            OutilFamille::ExtrusionChemin => Some(FormeGuidee::ExtrusionChemin { chemin }),
            OutilFamille::Raccordement => Some(FormeGuidee::Raccordement { profil_fin: contour.clone(), hauteur }),
            OutilFamille::RaccordementChemin => Some(FormeGuidee::RaccordementChemin { profil_fin: contour.clone(), chemin }),
            _ => None,
        };
        if let Some(g) = &guidage { g.valider(&contour)?; }
        let vide = matches!(self.outil, OutilFamille::ExtrusionVide | OutilFamille::RevolutionVide);
        let base = guidage.as_ref().map(FormeGuidee::nom).unwrap_or(if revolution.is_some() { "Révolution" } else { "Extrusion" });
        let base = if vide { format!("{base} vide") } else { base.into() };
        let nom = (1..).map(|n| format!("{base} {n}")).find(|nom| self.famille.extrusions.iter().all(|e| &e.nom != nom)).unwrap();
        let vue = self.vue.esquisse();
        let profondeur = if construction.is_some() {
            [Face {plan:String::new(),decalage:0.0},Face {plan:String::new(),decalage:100.0}]
        } else { self.faces_profondeur(&resolution,vue.axes().2,vide) };
        let profil = std::mem::take(&mut self.esquisse).into_iter().map(|(p,_)| p).collect();
        self.famille.extrusions.push(Extrusion {guidage,revolution,construction,nom,vide,materiau:Materiau::Menuiserie,vue,profil,profondeur,visible:None,materiau_parametre:None});
        self.choisir_outil(OutilFamille::Selection);
        self.selectionner(vec![SelectionFamille::Extrusion(self.famille.extrusions.len()-1)]);
        self.message=Some("Forme créée : régler le chemin, le second profil ou les dimensions dans Propriétés. Plein/Vide est disponible pour toutes les formes.".into());
        Ok(())
    }
}

pub(super) fn guidage_ui(ui: &mut egui::Ui, guidage: &mut FormeGuidee) {
    ui.label(RichText::new(guidage.nom()).strong());
    ui.label("Coordonnées locales en mm : H et V dans le profil, D en profondeur. Le premier point du chemin est l'origine de la section.");
    let (profil_fin, chemin) = match guidage {
        FormeGuidee::ExtrusionChemin { chemin } => (None,Some(chemin)),
        FormeGuidee::Raccordement { profil_fin, hauteur } => {
            ui.horizontal(|ui| { ui.label("Distance entre profils"); ui.add(egui::DragValue::new(hauteur).speed(10.0).range(0.1..=1e6).dimensions()); });
            (Some(profil_fin),None)
        }
        FormeGuidee::RaccordementChemin { profil_fin, chemin } => (Some(profil_fin),Some(chemin)),
    };
    if let Some(chemin) = chemin {
        egui::CollapsingHeader::new("Chemin polygonal").default_open(true).show(ui, |ui| {
            let mut retirer = None;
            let suppression = chemin.len()>2;
            for (i,point) in chemin.iter_mut().enumerate() {
                ui.push_id(("chemin",i), |ui| {
                    ui.label(format!("Point {}",i+1));
                    ui.horizontal(|ui| {
                        for (label,valeur) in ["H","V","D"].into_iter().zip(point) {
                            ui.label(label); ui.add(egui::DragValue::new(valeur).speed(10.0).dimensions());
                        }
                        if ui.add_enabled(suppression,egui::Button::new("−")).clicked() { retirer=Some(i); }
                    });
                });
            }
            if let Some(i)=retirer { chemin.remove(i); }
            if ui.button("Ajouter un point").clicked() {
                let dernier = chemin.last().copied().unwrap_or([0.0;3]);
                chemin.push([dernier[0],dernier[1],dernier[2]+1000.0]);
            }
        });
    }
    if let Some(fin) = profil_fin {
        egui::CollapsingHeader::new("Profil d'arrivée").default_open(true).show(ui, |ui| {
            ui.label("Chaque sommet correspond au sommet de même numéro du profil initial.");
            for (i,point) in fin.iter_mut().enumerate() {
                ui.push_id(("profil_fin",i), |ui| ui.horizontal(|ui| {
                    ui.label(format!("Sommet {}",i+1));
                    ui.label("H"); ui.add(egui::DragValue::new(&mut point[0]).speed(1.0).dimensions());
                    ui.label("V"); ui.add(egui::DragValue::new(&mut point[1]).speed(1.0).dimensions());
                }));
            }
        });
    }
}
