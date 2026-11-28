//! Création et sauvegarde du document natif sur poste et dans le navigateur.

use crate::*;
use echanges::enregistrement::EXTENSION_PROJET;

impl Application {
    pub(crate) fn creer_projet(&mut self) {
        if self.partage_en_cours {
            self.error = Some("Attendre la fin de la création du partage".into());
            return;
        }
        #[cfg(target_arch = "wasm32")]
        let chemin = PathBuf::from(format!("Nouveau projet.{EXTENSION_PROJET}"));
        #[cfg(not(target_arch = "wasm32"))]
        let Some(chemin) = crate::socle::dialogue::Fichier::new()
            .set_title("Nouveau projet natif v0")
            .add_filter("Projet BIMedit", &[EXTENSION_PROJET])
            .set_file_name("Nouveau projet.bem")
            .save_file()
        else {
            return;
        };
        #[cfg(not(target_arch = "wasm32"))]
        let chemin = if chemin.extension().is_none() {
            chemin.with_extension(EXTENSION_PROJET)
        } else {
            chemin
        };
        let nom = chemin
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Nouveau projet".into());
        let projet = modele::Projet::depuis_gabarit(&nom, &modele::gabarit::actif());
        let resultat = maillage::depuis_natif(&projet, &chemin);
        #[cfg(not(target_arch = "wasm32"))]
        let resultat = resultat.and_then(|scene| {
            echanges::enregistrement::enregistrer_natif(&projet, &chemin)?;
            Ok(scene)
        });
        match resultat {
            Ok(scene) => {
                self.install_scene(scene);
                self.status = format!("{} — nouveau projet natif v0", projet.nom);
                #[cfg(not(target_arch = "wasm32"))]
                let _ = self.historique.enregistrer(&projet);
                self.natif_modifie = cfg!(target_arch = "wasm32");
                self.natif = Some((projet, chemin));
                self.avant_saisie = None;
                self.error = None;
                self.ouvrir_plan_initial_natif();
            }
            Err(e) => self.error = Some(e),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn enregistrer_projet(&mut self, ctx: &egui::Context) {
        if self.loading.is_some() || self.tache_en_cours() {
            if self.tache.is_some() {
                self.error =
                    Some("Attendre la fin de l'enregistrement ou de l'export en cours.".into());
            }
            return;
        }
        if self.natif.is_none() {
            self.enregistrer_rvt_dialogue(ctx);
            return;
        }
        self.enregistrer_rendu_vue();
        let Some((projet, chemin)) = &self.natif else {
            return;
        };
        match echanges::enregistrement::enregistrer_natif(projet, chemin) {
            Ok(()) => {
                self.natif_modifie = false;
                let _ = self.historique.enregistrer(projet);
                echanges::enregistrement::effacer_recuperation(chemin);
                self.status = format!("{} — enregistré", chemin.display());
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn enregistrer_projet(&mut self, _ctx: &egui::Context) {
        if self.loading.is_some() {
            return;
        }
        self.enregistrer_rendu_vue();
        let Some((projet, chemin)) = &self.natif else {
            return;
        };
        let nom = chemin
            .file_name()
            .and_then(|nom| nom.to_str())
            .unwrap_or("Nouveau projet.bem");
        let resultat = (|| {
            projet.verifier()?;
            let json = projet.json()?;
            modele::Projet::lire_json(&json)?;
            telecharger(nom, &json)
        })();
        match resultat {
            Ok(()) => {
                let _ = self.historique.enregistrer(projet);
                self.natif_modifie = false;
                self.status = format!("{nom} — téléchargement demandé");
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn telecharger(nom: &str, json: &str) -> Result<(), String> {
    use eframe::wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use eframe::web_sys::{self, Blob, BlobPropertyBag, HtmlAnchorElement, Url, js_sys::Array};

    let fenetre = web_sys::window().ok_or("Fenêtre du navigateur inaccessible.")?;
    let document = fenetre
        .document()
        .ok_or("Page du navigateur inaccessible.")?;
    let corps = document.body().ok_or("Corps de la page inaccessible.")?;
    let erreur = |e: JsValue| format!("Téléchargement de {nom} impossible : {e:?}");
    let lien: HtmlAnchorElement = document
        .create_element("a")
        .map_err(erreur)?
        .dyn_into()
        .map_err(|_| "Lien de téléchargement inaccessible.")?;
    let parties = Array::of1(&JsValue::from_str(json));
    let options = BlobPropertyBag::new();
    options.set_type("application/json;charset=utf-8");
    let fichier = Blob::new_with_str_sequence_and_options(&parties, &options).map_err(erreur)?;

    lien.set_download(nom);
    lien.set_hidden(true);
    corps.append_child(&lien).map_err(erreur)?;
    let adresse = Url::create_object_url_with_blob(&fichier);
    if let Ok(adresse) = &adresse {
        lien.set_href(adresse);
        lien.click();
    }
    lien.remove();
    let adresse = adresse.map_err(erreur)?;

    // Laisser au navigateur le temps de prendre en charge le Blob.
    let a_liberer = adresse.clone();
    let liberer = Closure::once_into_js(move || {
        let _ = Url::revoke_object_url(&a_liberer);
    });
    if fenetre
        .set_timeout_with_callback_and_timeout_and_arguments_0(liberer.unchecked_ref(), 1000)
        .is_err()
    {
        let _ = Url::revoke_object_url(&adresse);
    }
    Ok(())
}
