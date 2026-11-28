//! Fichier déposé dans la fenêtre. Poste : ouvert par son chemin (`open`), un IFC est lié
//! au projet ouvert. Navigateur (wasm32) : pas de chemin local, le contenu est lu de façon
//! asynchrone ; seul un projet `.bem` s'ouvre (visionneuse, ROADMAP §10.3).
use crate::*;

impl Application {
    pub(crate) fn deposer(&mut self, fichier: egui::DroppedFileHandle, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let p = fichier.path().to_path_buf();
            // IFC déposé : lié au projet natif ouvert (il ne s'ouvre pas seul)
            if crate::outils::modeliser::liens::est_ifc(&p) && self.natif.is_some() { self.inserer_lien(ctx, p); } else { self.open(p, ctx); }
        }
        #[cfg(target_arch = "wasm32")]
        self.deposer_contenu(fichier, ctx);
    }

    #[cfg(target_arch = "wasm32")]
    fn deposer_contenu(&mut self, fichier: egui::DroppedFileHandle, ctx: &egui::Context) {
        let nom = fichier.path().to_path_buf();
        if !echanges::enregistrement::extension_projet(&nom) {
            self.error = Some(format!("{} : dans le navigateur, seul un projet .bem s'ouvre.", nom.display()));
            return;
        }
        let (tx, rx) = channel();
        self.abandonner_chargement();
        self.partage_en_cours = false;
        self.loading = Some(rx);
        self.error = None;
        self.status = format!("Chargement de {}…", nom.display());
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let res = fichier.bytes_async().await
                .and_then(|octets| String::from_utf8(octets).map_err(|e| format!("Lecture impossible : {e}")))
                .and_then(|json| modele::Projet::lire_json(&json))
                .and_then(|projet| maillage::depuis_natif(&projet, &nom).map(|scene| (Box::new(scene), projet, nom)));
            let _ = tx.send(LoadMsg::Natif(res));
            ctx.request_repaint();
        });
    }
}
