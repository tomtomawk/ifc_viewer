//! Finalisation et propriétés communes des formes à profil, quel que soit le canevas.
use super::*;
use modele::famille::modelee::{FormeGuidee, Revolution};

impl OutilFamille {
    pub(super) fn est_forme(self) -> bool {
        matches!(self, Self::Extrusion | Self::ExtrusionVide | Self::Revolution | Self::RevolutionVide | Self::ExtrusionChemin | Self::Raccordement | Self::RaccordementChemin)
    }
}

impl EditeurFamille {
    /// Faces d'un volume sur l'axe que la vue ne montre pas : celles du dernier volume
    /// du même genre, sinon les faces du mur (± 5 mm pour un vide), sinon les plans
    /// extrêmes.
    pub(super) fn faces_profondeur(&self, r: &Resolution, axe: AxePlan, vide: bool) -> [Face; 2] {
        let k = axe as usize;
        if let Some(e) = self.famille.extrusions.iter().rev().find(|e| e.vide == vide && e.vue.axes().2 == axe) {
            return e.profondeur.clone();
        }
        if let Some(v) = self.famille.volumes.iter().rev().find(|v| v.vide == vide) {
            return [v.min[k].clone(), v.max[k].clone()];
        }
        let marge = if vide { 5.0 } else { 0.0 };
        if axe == AxePlan::Y {
            let hote = |h| self.famille.plans.iter().find(|p| p.hote == Some(h)).map(|p| p.nom.clone());
            if let (Some(i), Some(e)) = (hote(Hote::Interieur), hote(Hote::Exterieur)) {
                return [Face { plan: i, decalage: -marge }, Face { plan: e, decalage: marge }];
            }
        }
        let mut plans: Vec<&PlanRef> = self.famille.plans.iter().filter(|p| p.axe == axe).collect();
        plans.sort_by(|a, b| self.position(r, &a.nom).total_cmp(&self.position(r, &b.nom)));
        let nom = |p: Option<&&PlanRef>| p.map_or_else(String::new, |p| p.nom.clone());
        [Face { plan: nom(plans.first()), decalage: 0.0 }, Face { plan: nom(plans.last()), decalage: 0.0 }]
    }

    /// Ne consomme le tracé qu'après validation ; une erreur reste corrigeable.
    /// `support` : plan de dessin actif de la vue 3D ; None : vue d'esquisse plane.
    pub(super) fn terminer_forme(&mut self, support: Option<SupportForme>) -> Result<(), String> {
        let resolution = self.resoudre();
        let contour: Vec<_> = self.esquisse.iter().map(|(p,_)| {
            let (h,v) = self.famille.point_esquisse(&resolution,p); [h,v]
        }).collect();
        modele::famille::profil::contour_simple(&contour)?;
        let revolution = if self.outil.est_revolution() {
            let r = Revolution::nouvelle(self.axe_revolution.ok_or("Définir l'axe avant le profil.")?);
            r.valider(&contour)?;
            Some(r)
        } else { None };
        let hauteur = 1000.0;
        let chemin = vec![[0.0;3],[0.0,0.0,hauteur]];
        let guidage = match self.outil {
            OutilFamille::ExtrusionChemin => Some(FormeGuidee::extrusion(chemin)),
            OutilFamille::Raccordement => Some(FormeGuidee::raccordement(contour.clone(), hauteur)),
            OutilFamille::RaccordementChemin => Some(FormeGuidee::raccordement_chemin(contour.clone(), chemin)),
            _ => None,
        };
        if let Some(g) = &guidage { g.valider(&contour)?; }
        let vide = matches!(self.outil, OutilFamille::ExtrusionVide | OutilFamille::RevolutionVide);
        let base = guidage.as_ref().map(FormeGuidee::nom).unwrap_or(if revolution.is_some() { "Révolution" } else { "Extrusion" });
        let base = if vide { format!("{base} vide") } else { base.into() };
        let nom = (1..).map(|n| format!("{base} {n}")).find(|nom| self.famille.extrusions.iter().all(|e| &e.nom != nom)).unwrap();
        let vue = self.vue.esquisse();
        let profondeur = if support.is_some() {
            [Face {plan:String::new(),decalage:0.0},Face {plan:String::new(),decalage:100.0}]
        } else if let Some(plan) = self.plan_actif_parallele() {
            [Face {plan:plan.clone(),decalage:0.0},Face {plan,decalage:100.0}]
        } else { self.faces_profondeur(&resolution,vue.axes().2,vide) };
        let profil = std::mem::take(&mut self.esquisse).into_iter().map(|(p,_)| p).collect();
        let operation = match (guidage, revolution) {
            (Some(guidage), None) => OperationForme::Guidage { guidage },
            (None, Some(revolution)) => OperationForme::Revolution { revolution },
            (None, None) => OperationForme::Extrusion,
            (Some(_), Some(_)) => return Err("Opérations de construction incompatibles.".into()),
        };
        self.famille.extrusions.push(FormeProfil { visibilite: Default::default(),operation,support,nom,vide,materiau:Materiau::Menuiserie,vue,profil,profondeur,visible:None,materiau_parametre:None});
        self.choisir_outil(OutilFamille::Selection);
        self.selectionner(vec![SelectionFamille::Extrusion(self.famille.extrusions.len()-1)]);
        self.message=Some("Forme créée : régler le chemin, le second profil ou les dimensions dans Propriétés. Plein/Vide est disponible pour toutes les formes.".into());
        Ok(())
    }
}

pub(super) fn guidage_ui(ui: &mut egui::Ui, guidage: &mut FormeGuidee) {
    ui.label(RichText::new(guidage.nom()).strong());
    ui.label("Coordonnées locales en mm : H et V dans le profil, D en profondeur. Le premier point du chemin est l'origine de la section.");
    if let FormeGuidee::Raccordement { hauteur, .. } = guidage {
        ui.horizontal(|ui| { ui.label("Distance entre profils"); ui.add(egui::DragValue::new(hauteur).speed(10.0).range(0.1..=1e6).dimensions()); });
    }
    let ancre = guidage.ancrage_fin().is_some();
    if let Some((chemin, arcs, ferme)) = guidage.trace_mut() {
        egui::CollapsingHeader::new("Chemin").default_open(true).show(ui, |ui| {
            if let Some(ferme) = ferme { ui.checkbox(ferme, "Chemin fermé (le dernier point rejoint le premier)"); }
            let mut retirer = None;
            let suppression = chemin.len()>2;
            for (i,point) in chemin.iter_mut().enumerate() {
                ui.push_id(("chemin",i), |ui| {
                    ui.label(format!("Point {}",i+1));
                    ui.horizontal(|ui| {
                        for (label,valeur) in ["H","V","D"].into_iter().zip(point) {
                            ui.label(label); ui.add(egui::DragValue::new(valeur).speed(10.0).dimensions());
                        }
                        let mut passage = arcs.contains(&i);
                        if ui.checkbox(&mut passage, "arc").on_hover_text("Point de passage d'un arc par trois points (avec ses deux voisins)").changed() {
                            if passage { arcs.push(i); arcs.sort_unstable(); } else { arcs.retain(|&j| j != i); }
                        }
                        if ui.add_enabled(suppression,egui::Button::new("−")).clicked() { retirer=Some(i); }
                    });
                });
            }
            if let Some(i)=retirer {
                chemin.remove(i);
                *arcs = arcs.iter().filter(|&&j| j != i).map(|&j| if j > i { j - 1 } else { j }).collect();
            }
            if ui.button("Ajouter un point").clicked() {
                let dernier = chemin.last().copied().unwrap_or([0.0;3]);
                chemin.push([dernier[0],dernier[1],dernier[2]+1000.0]);
            }
        });
    }
    let profil_fin = match guidage {
        FormeGuidee::Raccordement { profil_fin, .. } | FormeGuidee::RaccordementChemin { profil_fin, .. } if !ancre => Some(profil_fin),
        _ => None,
    };
    if let Some(fin) = profil_fin {
        egui::CollapsingHeader::new("Profil d'arrivée").default_open(true).show(ui, |ui| {
            ui.label("Même nombre de sommets : correspondance sommet à sommet ; sinon, correspondance automatique (centres alignés, premier sommet le plus proche, abscisse curviligne).");
            let (suppression, mut retirer) = (fin.len() > 3, None);
            for (i,point) in fin.iter_mut().enumerate() {
                ui.push_id(("profil_fin",i), |ui| ui.horizontal(|ui| {
                    ui.label(format!("Sommet {}",i+1));
                    ui.label("H"); ui.add(egui::DragValue::new(&mut point[0]).speed(1.0).dimensions());
                    ui.label("V"); ui.add(egui::DragValue::new(&mut point[1]).speed(1.0).dimensions());
                    if ui.add_enabled(suppression, egui::Button::new("−")).clicked() { retirer = Some(i); }
                }));
            }
            if let Some(i) = retirer { fin.remove(i); }
            if ui.button("Ajouter un sommet").clicked() && let (Some(a), Some(b)) = (fin.last().copied(), fin.first().copied()) {
                fin.push([(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]);
            }
        });
    }
}
