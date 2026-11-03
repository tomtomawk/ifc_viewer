//! Raccordement du document, des saisies et des décisions explicites de l'utilisateur.
use super::*;
use crate::modele::collaboration::{self as collab, session::Session};

impl ViewerApp {
    pub(super) fn associer_collaboration(&mut self,central:PathBuf)->Result<(),String>{
        let (p,path)=self.natif.as_ref().ok_or("Projet natif requis")?;
        let c=Session::ouvrir(p,path.clone(),central)?;let travail=c.travail()?;
        self.collaboration=Some(c);self.appliquer_projet_interne(travail,true)?;
        self.collaboration.as_mut().unwrap().synchroniser(None,false)
    }
    pub(super) fn rouvrir_collaboration(&mut self) {
        self.collaboration=None;
        let Some((p,path))=&self.natif else{return;};
        let sidecar=path.with_extension("collaboration.json");
        if !sidecar.exists(){return;}
        let resultat=(||{
            let copie=Session::lire_copie(&sidecar)?;
            Session::ouvrir(p,path.clone(),copie.central)
        })();
        match resultat {Ok(c)=>{let travail=c.travail();self.collaboration=Some(c);if let Err(e)=travail.and_then(|p|self.appliquer_projet_interne(p,true)){self.error=Some(e);}},Err(e)=>self.error=Some(e)}
    }
    pub(super) fn recevoir_collaboration(&mut self) {
        if self.saisie_active || self.avant_saisie.is_some(){return;}
        if let Some(c)=&mut self.collaboration {
            match c.recevoir(){Ok(Some(p))=>{if let Err(e)=self.appliquer_projet_interne(p,true){self.error=Some(e);}},Ok(None)=>(),Err(e)=>self.error=Some(e)}
        }
    }
    fn cles_collaboration(&self)->Vec<String>{
        let ids=self.selection_bim();
        self.natif.as_ref().and_then(|(p,_)|collab::objets(p).ok()).into_iter().flat_map(|o|o.into_iter().filter(|(_,v)|v.get("id").and_then(serde_json::Value::as_str).is_some_and(|id|ids.iter().any(|i|i==id))).map(|(k,_)|k)).collect()
    }
    pub(super) fn barre_collaboration(&mut self, ui:&mut egui::Ui){
        if self.natif.is_none(){return;}
        if self.collaboration.is_none(){
            if ui.button("Collaboration : associer cette copie locale…").clicked(){
                if let Some(central)=rfd::FileDialog::new().set_title("Dépôt partagé BIMedit (dossier dédié)").pick_folder(){
                    if let Err(e)=self.associer_collaboration(central){self.error=Some(e);}
                }
            } return;
        }
        let mut sync=false;let mut reservation=None;let mut instantane=false;let cles=self.cles_collaboration();
        ui.horizontal_wrapped(|ui|{
            let c=self.collaboration.as_ref().unwrap();ui.label(&c.etat);
            ui.label(format!("r{} · {} en attente",c.copie.base.revision,c.copie.attente.len()));
            if ui.button("Synchroniser").clicked(){sync=true;}
            if ui.button("Instantané partagé").clicked(){sync=true;instantane=true;}
            if !cles.is_empty(){
                if ui.button("Réserver sélection").clicked(){reservation=Some((cles.clone(),false,false));sync=true;}
                if ui.button("Libérer sélection").clicked(){reservation=Some((cles.clone(),false,true));sync=true;}
                ui.menu_button("Reprise explicite",|ui|{ui.label("Révoque l'ancien détenteur. Prévenir le collègue concerné.");if ui.button("Reprendre la sélection").clicked(){reservation=Some((cles.clone(),true,false));sync=true;ui.close();}});
                for k in &cles {if let Some(b)=c.copie.base.baux.get(k).filter(|b|b.actif){ui.label(format!("Réservé : {} · génération {}",b.proprietaire,b.generation));}}
            }
            ui.label(format!("validation {:.1} ms · sync {:.1} ms",c.mesure_validation_ms,c.mesure_sync_ms));
        });
        if sync {if let Err(e)=self.collaboration.as_mut().unwrap().synchroniser(reservation,instantane){self.error=Some(e);}}
        let mut abandon=None;let mut locale=None;
        let c=self.collaboration.as_ref().unwrap();
        if !c.copie.conflits.is_empty(){
            egui::Window::new("Conflits collaboratifs — propositions conservées").show(ui.ctx(),|ui|{
                egui::ScrollArea::vertical().show(ui,|ui|{
                    for (id,conflits) in &c.copie.conflits{
                        ui.label(format!("Transaction {id}"));
                        for conflit in conflits {ui.label(format!("{} : {}",conflit.cle,conflit.raison));
                            ui.columns(3,|cols|{for (col,(titre,v)) in cols.iter_mut().zip([("Base",&conflit.base),("Proposition locale",&conflit.locale),("État accepté",&conflit.acceptee)]){col.label(titre);col.label(v.as_ref().map(|v|serde_json::to_string_pretty(v).unwrap_or_default()).unwrap_or_else(||"∅".into()));}});
                        }
                        ui.collapsing("Transaction complète récupérable",|ui|{if let Some(t)=c.copie.attente.iter().find(|t|&t.id==id){ui.label(serde_json::to_string_pretty(t).unwrap_or_default());}});
                        if ui.button("Conserver l'état accepté et archiver cette proposition").clicked(){abandon=Some(id.clone());}
                        if ui.button("Choisir la proposition locale : nouvelle transaction validée").clicked(){locale=Some(id.clone());}
                    }
                });
            });
        }
        ui.collapsing("Textes d'annotations supprimées récupérables",|ui|{
            for(id,t)in &c.copie.base.textes{if t.supprime{ui.label(id);if let Ok(texte)=t.contenu(){ui.label(&texte);if ui.button(format!("Copier le texte {id}")).clicked(){ui.ctx().copy_text(texte);}}}}
        });
        if let Some(id)=abandon{let r=self.collaboration.as_mut().unwrap().abandonner(&id).and_then(|p|self.appliquer_projet_interne(p,true));if let Err(e)=r{self.error=Some(e);}}
        if let Some(id)=locale{let r=self.collaboration.as_mut().unwrap().resoudre_locale(&id).and_then(|p|self.appliquer_projet_interne(p,true));if let Err(e)=r{self.error=Some(e);}}
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    }
}
