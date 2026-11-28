//! BIMedit dans le navigateur : l'interface egui dans le canevas `#bimedit` de
//! `index.html`, rendue par wgpu en WebGPU seul (décision du 28/11/2026). Visionneuse : un
//! projet `.bem` déposé dans la page s'ouvre (ROADMAP §10.3).

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;
    wasm_bindgen_futures::spawn_local(async {
        let Some(canevas) = document()
            .and_then(|d| d.get_element_by_id("bimedit"))
            .and_then(|e| e.dyn_into::<eframe::web_sys::HtmlCanvasElement>().ok())
        else {
            return;
        };
        let mut options = eframe::WebOptions::default();
        if let eframe::egui_wgpu::WgpuSetup::CreateNew(creation) = &mut options.wgpu_options.wgpu_setup {
            creation.instance_descriptor.backends = eframe::wgpu::Backends::BROWSER_WEBGPU;
        }
        let lancement = eframe::WebRunner::new()
            .start(
                canevas,
                options,
                Box::new(|cc| {
                    // périphérique perdu (pilote, mémoire, onglet en arrière-plan…) : le dire
                    // plutôt que laisser une page figée
                    if let Some(rendu) = &cc.wgpu_render_state {
                        rendu.device.set_device_lost_callback(|raison, texte| {
                            message(&format!("Le périphérique WebGPU a été perdu ({raison:?} : {texte}). Rechargez la page."));
                        });
                    }
                    Ok(Box::new(interface::Application::new(cc, None)))
                }),
            )
            .await;
        match lancement {
            Ok(()) => masquer_message(),
            Err(e) => message(&format!("BIMedit demande un navigateur avec WebGPU (Chrome, Edge) : {e:?}")),
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn document() -> Option<eframe::web_sys::Document> {
    eframe::web_sys::window().and_then(|w| w.document())
}

/// Affiche `texte` dans le bandeau `#attente` d'`index.html`, par-dessus le canevas.
#[cfg(target_arch = "wasm32")]
fn message(texte: &str) {
    if let Some(e) = document().and_then(|d| d.get_element_by_id("attente")) {
        e.set_text_content(Some(texte));
        let _ = e.set_attribute("style", "display: flex");
    }
}

#[cfg(target_arch = "wasm32")]
fn masquer_message() {
    if let Some(e) = document().and_then(|d| d.get_element_by_id("attente")) {
        let _ = e.set_attribute("style", "display: none");
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("bimedit-web se construit pour le navigateur : `trunk build --release` dans crates/web.");
}
