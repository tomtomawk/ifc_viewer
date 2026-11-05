//! Palettes natives partageant le même état de projet que la fenêtre principale.
use super::*;

#[derive(Default)]
pub(super) struct FenetreVolet {
    pub natif: bool,
    pub interaction: bool,
    pub rect: Option<egui::Rect>,
    pub entete: Option<egui::Rect>,
    pub position: Option<egui::Pos2>,
    pub taille: Option<egui::Vec2>,
    pub cible: Option<Ancrage>,
}

/// Coordonnées globales de fenêtre et coordonnées de dépôt dans la fenêtre principale.
pub(super) fn suivre_pointeur(ctx:&egui::Context,source:egui::ViewportId,decalage:egui::Vec2)->Option<(egui::Pos2,egui::Pos2,bool)> {
    let (q,enfonce,origine)=ctx.input_for(source,|i| (
        i.pointer.latest_pos(),i.pointer.primary_down(),
        i.viewport().inner_rect.map_or(egui::Pos2::ZERO,|r|r.min)));
    let principale=ctx.input_for(egui::ViewportId::ROOT,|i|
        i.viewport().inner_rect.map_or(egui::Pos2::ZERO,|r|r.min));
    let global=origine+q?.to_vec2();
    Some((global-decalage,egui::pos2(global.x-principale.x,global.y-principale.y),enfonce))
}

pub(super) fn cible_volet(zone:egui::Rect,q:egui::Pos2)->Option<Ancrage> {
    if !zone.is_positive() || !zone.contains(q) {None}
    else if q.x<=zone.left()+64.0 {Some(Ancrage::Gauche)}
    else if q.x>=zone.right()-64.0 {Some(Ancrage::Droite)}
    else {None}
}

impl ViewerApp {
    pub(super) fn menu_volet(&mut self,resp:&egui::Response,p:Panneau) {
        resp.context_menu(|ui| {
            for (a,nom) in [(Ancrage::Externe,"Détacher dans une fenêtre"),(Ancrage::Gauche,"Ancrer à gauche"),(Ancrage::Droite,"Ancrer à droite"),(Ancrage::Flottant,"Flotter dans l'application")] {
                if ui.selectable_label(self.ancrages[p as usize]==a,nom).clicked() {
                    self.ancrer(p,a);
                    self.deplacement_panneau=None;
                    ui.close();
                }
            }
        });
    }

    pub(super) fn retablir_volets(&mut self) {
        self.ancrer(Panneau::Proprietes,Ancrage::Gauche);
        self.ancrer(Panneau::Arborescence,Ancrage::Droite);
        self.deplacement_panneau=None;
    }

    pub(super) fn fenetres_volets(&mut self,ctx:&egui::Context) {
        for p in Panneau::TOUS {
            let i=p as usize;
            if self.ancrages[i]!=Ancrage::Externe {self.volets[i].natif=false;self.volets[i].interaction=false;continue;}
            let titre=match p {Panneau::Proprietes=>"BIMedit — Propriétés",Panneau::Arborescence=>"BIMedit — Arborescence"};
            let glissement=self.deplacement_panneau.is_some_and(|(q,_,source)|q==p && source==egui::ViewportId::ROOT);
            let mut builder=egui::ViewportBuilder::default().with_title(titre)
                .with_inner_size(self.volets[i].taille.unwrap_or(egui::vec2(self.largeurs_panneaux[i].max(280.0),600.0)))
                .with_decorations(false)
                .with_min_inner_size([160.0,160.0]).with_taskbar(true)
                .with_active(false).with_mouse_passthrough(glissement);
            if let Some(position)=self.volets[i].position {builder=builder.with_position(position);}
            ctx.show_viewport_immediate(egui::ViewportId::from_hash_of(("volet_externe",p.nom())),builder,|ui,classe| {
                let ctx=ui.ctx().clone();
                // Une fenêtre native n'hérite pas du fond peint par le panneau ancré.
                // Couvrir toute sa surface, y compris sous le contenu défilant.
                ui.painter().rect_filled(ui.max_rect(),0.0,ui.visuals().panel_fill);
                self.volets[i].natif=classe==egui::ViewportClass::Immediate;
                self.volets[i].interaction=ctx.input(|i|i.pointer.any_down());
                self.volets[i].rect=ctx.input(|i|i.viewport().outer_rect);
                self.volets[i].taille=ctx.input(|i|i.viewport().inner_rect.map(|r|r.size()));
                if ctx.input(|i|i.viewport().close_requested()) {
                    // Fermer une palette la replace dans le projet, sans fermer le projet.
                    self.ancrer(p,if p==Panneau::Proprietes {Ancrage::Gauche}else{Ancrage::Droite});
                } else {
                    self.panneau(ui,p);
                    self.clavier_raccourcis(&ctx);
                }
            });
        }
    }

    pub(super) fn etat_volets(&self)->serde_json::Value {
        serde_json::json!({"volets":Panneau::TOUS.map(|p| {
            let v=&self.volets[p as usize];
            serde_json::json!({"nom":p.nom(),"ancrage":self.ancrages[p as usize].nom(),"fenetre_native":v.natif,"entete":v.entete.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"cible":v.cible.map(|c|c.nom()),"rectangle":v.rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y])})
        })})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn volet_deja_flottant_revient_depuis_un_autre_ecran() {
        let ctx=egui::Context::default();
        let source=egui::ViewportId::from_hash_of("palette");
        let mut viewports=egui::ViewportIdMap::default();
        viewports.insert(egui::ViewportId::ROOT,egui::ViewportInfo{inner_rect:Some(egui::Rect::from_min_size(egui::pos2(100.,60.),egui::vec2(900.,700.))),..Default::default()});
        viewports.insert(source,egui::ViewportInfo{inner_rect:Some(egui::Rect::from_min_size(egui::pos2(1200.,-200.),egui::vec2(300.,600.))),..Default::default()});
        let mut sortie=ctx.run_ui(egui::RawInput{viewports:viewports.clone(),..Default::default()},|_|{});sortie.textures_delta.clear();
        let zone=egui::Rect::from_min_max(egui::pos2(0.,80.),egui::pos2(900.,700.));
        for (q,appuye,attendu) in [(egui::pos2(50.,20.),true,None),(egui::pos2(-1090.,400.),false,Some(Ancrage::Gauche))] {
            let mut sortie=ctx.run_ui(egui::RawInput{viewport_id:source,viewports:viewports.clone(),events:vec![egui::Event::PointerMoved(q),egui::Event::PointerButton{pos:q,button:PointerButton::Primary,pressed:appuye,modifiers:egui::Modifiers::NONE}],..Default::default()},|_|{});sortie.textures_delta.clear();
            // Le suivi s'exécute depuis la fenêtre principale, pas depuis la palette.
            let mut sortie=ctx.run_ui(egui::RawInput{viewports:viewports.clone(),..Default::default()},|ui| {
                let (position,local,enfonce)=suivre_pointeur(ui.ctx(),source,egui::vec2(20.,10.)).unwrap();
                assert_eq!(enfonce,appuye);
                assert_eq!(position,egui::pos2(1200.+q.x-20.,-200.+q.y-10.));
                assert_eq!(cible_volet(zone,local),attendu);
            });sortie.textures_delta.clear();
        }
    }
}
