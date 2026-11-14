//! Chronométrage des étapes de calcul (variable d'environnement `BIMEDIT_CHRONO`).

/// Chronométrage des étapes de calcul, écrit sur la sortie d'erreur quand la variable
/// d'environnement `BIMEDIT_CHRONO` est définie ; remet le compteur à zéro.
pub fn chrono(nom: &str, depuis: &mut std::time::Instant) {
    static ACTIF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ACTIF.get_or_init(|| std::env::var_os("BIMEDIT_CHRONO").is_some()) {
        eprintln!("chrono {nom} : {:.1} ms", depuis.elapsed().as_secs_f64() * 1000.0);
    }
    *depuis = std::time::Instant::now();
}

/// `chrono` limité aux étapes de plus de 5 ms (étapes appelées à chaque image).
pub fn chrono_lent(nom: &str, depuis: &mut std::time::Instant) {
    if depuis.elapsed().as_millis() >= 5 { chrono(nom, depuis); } else { *depuis = std::time::Instant::now(); }
}
