//! Pilotage du viewer par un autre programme (serveur MCP `tools/viewer-mcp/`).
//!
//! Le viewer écoute sur 127.0.0.1 (port 47811, ou la variable `IFC_VIEWER_PORT`). Une
//! connexion envoie une ligne « commande argument » en UTF-8 et reçoit une ligne JSON.
//! Les commandes sont traitées par l'application entre deux images (`ViewerApp::piloter`).

use eframe::egui;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

pub const PORT: u16 = 47811;

/// Commande reçue, avec le canal de sa réponse (une ligne JSON).
pub struct Requete {
    pub commande: String,
    pub argument: String,
    pub reponse: Sender<String>,
}

/// Port d'écoute : `IFC_VIEWER_PORT` ou [`PORT`].
pub fn port() -> u16 {
    std::env::var("IFC_VIEWER_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(PORT)
}

/// Démarre l'écoute ; None si le port est déjà pris (autre viewer ouvert).
pub fn demarrer(ctx: egui::Context) -> Option<Receiver<Requete>> {
    let ecoute = TcpListener::bind(("127.0.0.1", port())).ok()?;
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("pilote".into())
        .spawn(move || {
            for flux in ecoute.incoming().flatten() {
                let (tx, ctx) = (tx.clone(), ctx.clone());
                std::thread::spawn(move || servir(flux, tx, ctx));
            }
        })
        .ok()?;
    Some(rx)
}

fn servir(flux: TcpStream, tx: Sender<Requete>, ctx: egui::Context) {
    let _ = flux.set_read_timeout(Some(Duration::from_secs(30)));
    let mut ligne = String::new();
    let Ok(copie) = flux.try_clone() else { return };
    if BufReader::new(copie).read_line(&mut ligne).is_err() {
        return;
    }
    let ligne = ligne.trim_end_matches(['\r', '\n']);
    let (commande, argument) = ligne.split_once(' ').unwrap_or((ligne, ""));
    let (rtx, rrx) = channel();
    if tx.send(Requete { commande: commande.to_string(), argument: argument.trim().to_string(), reponse: rtx }).is_err() {
        return;
    }
    ctx.request_repaint();
    // ouverture d'un gros fichier : jusqu'à 10 minutes
    let reponse = rrx.recv_timeout(Duration::from_secs(600)).unwrap_or_else(|_| erreur("délai dépassé"));
    let mut flux = flux;
    let _ = writeln!(flux, "{reponse}");
}

/// Chaîne JSON échappée.
pub fn texte(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Objet JSON à partir de paires (clé, valeur JSON déjà écrite).
pub fn objet(paires: &[(&str, String)]) -> String {
    let corps: Vec<String> = paires.iter().map(|(k, v)| format!("{}:{v}", texte(k))).collect();
    format!("{{{}}}", corps.join(","))
}

/// Liste JSON de valeurs déjà écrites.
pub fn liste(valeurs: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", valeurs.into_iter().collect::<Vec<_>>().join(","))
}

pub fn erreur(message: &str) -> String {
    objet(&[("ok", "false".into()), ("erreur", texte(message))])
}

pub fn succes(paires: &[(&str, String)]) -> String {
    let mut p = vec![("ok", "true".to_string())];
    p.extend(paires.iter().cloned());
    objet(&p)
}

/// Encode une image RGBA en PNG.
pub fn png(largeur: usize, hauteur: usize, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, largeur as u32, hauteur as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}
