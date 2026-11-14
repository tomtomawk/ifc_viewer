//! Identifiants uuid v4 des objets du projet.

pub fn uuid() -> String {
    ::uuid::Uuid::new_v4().to_string()
}
