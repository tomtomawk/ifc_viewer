//! Travail de fond. Sur poste : un fil nommé, à pile de 64 Mio (booléens et
//! reconstruction de scène sont récursifs). En wasm32, où un fil exige l'isolation
//! d'origine (en-têtes COOP et COEP, ROADMAP §10.3) : le travail s'exécute tout de suite
//! sur le fil appelant ; ses messages (canaux `mpsc`) sont déjà là au retour.

/// Taille de pile d'un fil de travail (poste).
pub const PILE: usize = 64 << 20;

/// Lance `travail` en arrière-plan (poste) ou l'exécute aussitôt (wasm32). L'erreur est
/// celle du système quand le fil ne peut être créé.
pub fn lancer(nom: &str, travail: impl FnOnce() + Send + 'static) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::Builder::new().name(nom.into()).stack_size(PILE).spawn(travail).map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = nom;
        travail();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn le_travail_lance_rend_son_resultat() {
        let (tx, rx) = std::sync::mpsc::channel();
        super::lancer("essai", move || { let _ = tx.send(6 * 7); }).unwrap();
        assert_eq!(rx.recv().unwrap(), 42);
    }
}
