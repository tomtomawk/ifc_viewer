//! Écriture des nombres.

/// Réel écrit sans zéros inutiles : entier si la valeur l'est, sinon quatre décimales au
/// plus.
pub fn format_real(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
