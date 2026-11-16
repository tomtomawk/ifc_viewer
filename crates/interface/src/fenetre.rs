//! Fenêtre native : création de la fenêtre eframe, mode arrière-plan, cadre du système.

use std::path::PathBuf;

/// Ouvre la fenêtre de l'application (projet ou fichier optionnel) et bloque jusqu'à sa
/// fermeture.
pub fn lancer(path: Option<PathBuf>) -> eframe::Result<()> {
    // l'anticrénelage de la vue 3D est celui du rendu (BIMEDIT_MSAA), pas de la fenêtre
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("BIMedit")
        .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icone.png")).expect("Icône intégrée invalide"))
        .with_decorations(cadre_natif())
        .with_min_inner_size([800.0, 520.0])
        .with_inner_size([1500.0, 920.0])
        .with_drag_and_drop(true);
    // Mode arrière-plan (tests, pilotage) : fenêtre créée sans activation, hors de l'écran et
    // hors de la barre des tâches ; elle reste rendue et pilotable (saisies egui simulées).
    let arriere_plan = arriere_plan();
    if arriere_plan {
        viewport = viewport.with_active(false).with_position([-20000.0, -20000.0]).with_taskbar(false);
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: !arriere_plan,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: limites_gpu(),
        ..Default::default()
    };
    eframe::run_native("BIMedit", options, Box::new(move |cc| Ok(Box::new(crate::ViewerApp::new(cc, path)))))
}

/// `BIMEDIT_ARRIERE_PLAN=1` : la fenêtre ne passe pas devant celles de l'utilisateur.
pub fn arriere_plan() -> bool {
    std::env::var("BIMEDIT_ARRIERE_PLAN").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Périphérique GPU aux limites de tampon de la carte (souvent plusieurs Go) au lieu des
/// 256 Mo par défaut de wgpu : sommets et traits d'un gros modèle Revit tiennent en un
/// tampon. Le reste des réglages d'egui est conservé.
fn limites_gpu() -> eframe::egui_wgpu::WgpuConfiguration {
    let mut config = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(creation) = &mut config.wgpu_setup {
        let defaut = creation.device_descriptor.clone();
        creation.device_descriptor = std::sync::Arc::new(move |adaptateur| {
            let mut d = defaut(adaptateur);
            let carte = adaptateur.limits();
            d.required_limits.max_buffer_size = d.required_limits.max_buffer_size.max(carte.max_buffer_size);
            d
        });
    }
    config
}

/// Cadre de fenêtre du système (barre de titre, bords) plutôt que la barre dessinée par
/// l'application. Par défaut sous Windows 10 seulement : une fenêtre sans décoration y
/// garde une bande en haut de la zone cliente, qui décale l'affichage par rapport aux
/// clics (Windows 11 n'a pas ce défaut). `BIMEDIT_CADRE=natif` ou `perso` force l'un
/// ou l'autre.
pub fn cadre_natif() -> bool {
    match std::env::var("BIMEDIT_CADRE").as_deref() {
        Ok("natif") => true,
        Ok("perso") => false,
        _ => windows_10(),
    }
}

/// Windows antérieur à Windows 11 (build < 22000), lu dans le registre ; faux ailleurs
/// ou si le build est illisible.
#[cfg(windows)]
fn windows_10() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    static BUILD: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    let build = *BUILD.get_or_init(|| {
        let large = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (cle, valeur) = (large("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"), large("CurrentBuildNumber"));
        let mut tampon = [0u16; 32];
        let mut taille = std::mem::size_of_val(&tampon) as u32;
        // SAFETY : chaînes terminées par zéro, tampon et taille cohérents
        let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, cle.as_ptr(), valeur.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), tampon.as_mut_ptr().cast(), &mut taille) };
        if r != 0 {
            return None;
        }
        let n = tampon.iter().position(|&c| c == 0).unwrap_or(tampon.len());
        String::from_utf16_lossy(&tampon[..n]).trim().parse().ok()
    });
    build.is_some_and(|b| b < 22000)
}

#[cfg(not(windows))]
fn windows_10() -> bool {
    false
}
