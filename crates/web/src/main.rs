//! BIMedit dans le navigateur : l'interface egui dans le canevas `#bimedit` de
//! `index.html`, rendue par wgpu (WebGPU, ou WebGL2 en repli). Visionneuse : un projet
//! `.bem` déposé dans la page s'ouvre (ROADMAP §10.3).

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;
    wasm_bindgen_futures::spawn_local(async {
        let document = eframe::web_sys::window().and_then(|w| w.document());
        let Some(canevas) = document
            .as_ref()
            .and_then(|d| d.get_element_by_id("bimedit"))
            .and_then(|e| e.dyn_into::<eframe::web_sys::HtmlCanvasElement>().ok())
        else {
            return;
        };
        let lancement = eframe::WebRunner::new()
            .start(canevas, eframe::WebOptions::default(), Box::new(|cc| Ok(Box::new(interface::Application::new(cc, None)))))
            .await;
        // démarré : le message d'attente disparaît ; échec (ni WebGPU ni WebGL2…) : il dit pourquoi
        if let Some(attente) = document.and_then(|d| d.get_element_by_id("attente")) {
            match lancement {
                Ok(()) => attente.remove(),
                Err(e) => attente.set_text_content(Some(&format!("BIMedit ne peut démarrer dans ce navigateur : {e:?}"))),
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("bimedit-web se construit pour le navigateur : `trunk build --release` dans crates/web.");
}
