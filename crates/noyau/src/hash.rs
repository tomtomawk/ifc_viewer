//! Empreintes SHA-256 de valeurs sérialisées (JSON canonique : dictionnaires triés).
use serde::Serialize;
use sha2::{Digest, Sha256};

pub fn empreinte(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn encoder<T: Serialize>(v: &T) -> Result<Vec<u8>, String> {
    // Value utilise des dictionnaires triés (serde_json sans preserve_order).
    serde_json::to_vec(&serde_json::to_value(v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn hash<T: Serialize>(v: &T) -> String {
    // valeur non sérialisable (clé de dictionnaire non textuelle) : empreinte du message d'erreur
    empreinte(&encoder(v).unwrap_or_else(String::into_bytes))
}
