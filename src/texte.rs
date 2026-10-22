//! Textes tracés en triangles, pour qu'ils fassent partie de la scène (captures hors
//! écran comprises) : contours des glyphes lus par skrifa, aplatis puis triangulés
//! (earcut). Police du type de texte prise dans les polices du système (Windows, macOS, Linux)
//! quand elle y est (Arial, Trebuchet MS…, ou l'équivalent Liberation), sinon Arial,
//! sinon la police Ubuntu d'egui.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Glyphe en unités de la police : triangles et avance.
struct Glyphe {
    triangles: Vec<[[f32; 2]; 3]>,
    avance: f32,
}

struct Police {
    donnees: Vec<u8>,
    /// Unités par cadratin et hauteur des capitales (unités de la police).
    unites: f32,
    capitales: f32,
    ascendante: f32,
    descendante: f32,
    glyphes: Mutex<HashMap<char, Arc<Glyphe>>>,
}

/// Ligne de texte en triangles (mm) : ligne de base à y = 0, début à x = 0.
pub struct Ligne {
    pub triangles: Vec<[[f64; 2]; 3]>,
    pub largeur: f64,
}

fn charger(donnees: Vec<u8>) -> Option<Police> {
    let f = FontRef::new(&donnees).ok()?;
    let m = f.metrics(Size::unscaled(), LocationRef::default());
    let unites = m.units_per_em as f32;
    let capitales = m.cap_height.unwrap_or(0.7 * unites);
    Some(Police { donnees, unites, capitales, ascendante: m.ascent, descendante: m.descent, glyphes: Mutex::new(HashMap::new()) })
}

/// Fichiers (normal, gras) des polices courantes : noms Windows, puis équivalents macOS
/// et Linux (Liberation, métriquement compatibles avec Arial, Times et Courier).
const FICHIERS: &[(&str, &[&str], &[&str])] = &[
    ("arial", &["arial.ttf", "LiberationSans-Regular.ttf", "DejaVuSans.ttf"], &["arialbd.ttf", "Arial Bold.ttf", "LiberationSans-Bold.ttf", "DejaVuSans-Bold.ttf"]),
    ("arial narrow", &["ARIALN.TTF", "Arial Narrow.ttf", "LiberationSansNarrow-Regular.ttf"], &["ARIALNB.TTF", "Arial Narrow Bold.ttf", "LiberationSansNarrow-Bold.ttf"]),
    ("trebuchet ms", &["trebuc.ttf", "Trebuchet MS.ttf"], &["trebucbd.ttf", "Trebuchet MS Bold.ttf"]),
    ("calibri", &["calibri.ttf", "Carlito-Regular.ttf"], &["calibrib.ttf", "Carlito-Bold.ttf"]),
    ("verdana", &["verdana.ttf", "Verdana.ttf", "DejaVuSans.ttf"], &["verdanab.ttf", "Verdana Bold.ttf", "DejaVuSans-Bold.ttf"]),
    ("tahoma", &["tahoma.ttf", "Tahoma.ttf"], &["tahomabd.ttf", "Tahoma Bold.ttf"]),
    ("times new roman", &["times.ttf", "Times New Roman.ttf", "LiberationSerif-Regular.ttf"], &["timesbd.ttf", "Times New Roman Bold.ttf", "LiberationSerif-Bold.ttf"]),
    ("segoe ui", &["segoeui.ttf", "DejaVuSans.ttf"], &["segoeuib.ttf", "DejaVuSans-Bold.ttf"]),
    ("century gothic", &["GOTHIC.TTF", "Century Gothic.ttf"], &["GOTHICB.TTF", "Century Gothic Bold.ttf"]),
    ("courier new", &["cour.ttf", "Courier New.ttf", "LiberationMono-Regular.ttf"], &["courbd.ttf", "Courier New Bold.ttf", "LiberationMono-Bold.ttf"]),
];

/// Dossiers de polices du système.
fn dossiers_polices() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut d = Vec::new();
    if cfg!(windows) {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        d.push(PathBuf::from(windir).join("Fonts"));
        if let Ok(l) = std::env::var("LOCALAPPDATA") {
            d.push(PathBuf::from(l).join("Microsoft").join("Windows").join("Fonts"));
        }
    } else {
        let home = std::env::var("HOME").map(PathBuf::from).ok();
        if cfg!(target_os = "macos") {
            d.extend(["/System/Library/Fonts", "/Library/Fonts"].map(PathBuf::from));
            d.extend(home.iter().map(|h| h.join("Library/Fonts")));
        } else {
            d.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
            d.extend(home.iter().map(|h| h.join(".local/share/fonts")));
            d.extend(home.iter().map(|h| h.join(".fonts")));
        }
    }
    d
}

fn indexer(dossier: &std::path::Path, profondeur: u32, index: &mut HashMap<String, std::path::PathBuf>) {
    let Ok(entrees) = std::fs::read_dir(dossier) else { return };
    for e in entrees.flatten() {
        let chemin = e.path();
        if chemin.is_dir() {
            if profondeur > 0 {
                indexer(&chemin, profondeur - 1, index);
            }
        } else if let Some(n) = chemin.file_name().and_then(|n| n.to_str()) {
            index.entry(n.to_lowercase()).or_insert(chemin);
        }
    }
}

/// Contenu du fichier de police `nom` (casse ignorée) dans les dossiers du système.
fn lire_fichier_police(nom: &str) -> Option<Vec<u8>> {
    static INDEX: OnceLock<HashMap<String, std::path::PathBuf>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        let mut index = HashMap::new();
        for d in dossiers_polices() {
            indexer(&d, 4, &mut index);
        }
        index
    });
    std::fs::read(index.get(&nom.to_lowercase())?).ok()
}

/// Police `nom` normale ou grasse, lue une fois ; repli sur Arial puis sur Ubuntu.
fn police(nom: &str, gras: bool) -> Arc<Police> {
    static POLICES: OnceLock<Mutex<HashMap<(String, bool), Arc<Police>>>> = OnceLock::new();
    let cle = (nom.to_lowercase(), gras);
    let cache = POLICES.get_or_init(Default::default);
    if let Some(p) = cache.lock().unwrap().get(&cle) {
        return p.clone();
    }
    let lire = |n: &str| {
        let (_, normal, gras_) = FICHIERS.iter().find(|f| f.0 == n)?;
        (if gras { gras_ } else { normal }).iter().find_map(|f| lire_fichier_police(f).and_then(charger))
    };
    let p = lire(&cle.0)
        .or_else(|| lire("arial"))
        .or_else(|| charger(epaint_default_fonts::UBUNTU_LIGHT.to_vec()))
        .expect("police de repli illisible");
    let p = Arc::new(p);
    cache.lock().unwrap().insert(cle, p.clone());
    p
}

/// Contours d'un glyphe, courbes aplaties.
struct Stylo {
    contours: Vec<Vec<[f32; 2]>>,
    courant: [f32; 2],
}

impl OutlinePen for Stylo {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contours.push(vec![[x, y]]);
        self.courant = [x, y];
    }
    fn line_to(&mut self, x: f32, y: f32) {
        if let Some(c) = self.contours.last_mut() {
            c.push([x, y]);
        }
        self.courant = [x, y];
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let p0 = self.courant;
        for k in 1..=4 {
            let t = k as f32 / 4.0;
            let u = 1.0 - t;
            self.line_to(u * u * p0[0] + 2.0 * u * t * cx + t * t * x, u * u * p0[1] + 2.0 * u * t * cy + t * t * y);
        }
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let p0 = self.courant;
        for k in 1..=6 {
            let t = k as f32 / 6.0;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.line_to(a * p0[0] + b * cx0 + c * cx1 + d * x, a * p0[1] + b * cy0 + c * cy1 + d * y);
        }
    }
    fn close(&mut self) {}
}

fn aire(c: &[[f32; 2]]) -> f32 {
    (0..c.len()).map(|i| {
        let (a, b) = (c[i], c[(i + 1) % c.len()]);
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f32>() / 2.0
}

fn dedans(p: [f32; 2], c: &[[f32; 2]]) -> bool {
    let mut d = false;
    for i in 0..c.len() {
        let (a, b) = (c[i], c[(i + 1) % c.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            d = !d;
        }
    }
    d
}

/// Triangles d'un glyphe : chaque contour est extérieur ou trou selon la parité du
/// nombre de contours qui le contiennent ; un trou est rattaché au plus petit contour
/// extérieur qui le contient.
pub(crate) fn trianguler(mut contours: Vec<Vec<[f32; 2]>>) -> Vec<[[f32; 2]; 3]> {
    for c in &mut contours {
        if c.len() > 1 && c.first() == c.last() {
            c.pop();
        }
    }
    contours.retain(|c| c.len() >= 3 && aire(c).abs() > 1e-6);
    let n = contours.len();
    let profondeur: Vec<usize> = (0..n).map(|i| (0..n).filter(|&j| j != i && aire(&contours[j]).abs() > aire(&contours[i]).abs() && dedans(contours[i][0], &contours[j])).count()).collect();
    let mut out = Vec::new();
    for i in (0..n).filter(|&i| profondeur[i] % 2 == 0) {
        let trous: Vec<usize> = (0..n)
            .filter(|&j| profondeur[j] == profondeur[i] + 1 && dedans(contours[j][0], &contours[i]))
            .collect();
        let mut coords: Vec<f64> = Vec::new();
        let mut indices_trous = Vec::new();
        for (k, &c) in std::iter::once(&i).chain(&trous).enumerate() {
            if k > 0 {
                indices_trous.push(coords.len() / 2);
            }
            coords.extend(contours[c].iter().flat_map(|p| [p[0] as f64, p[1] as f64]));
        }
        if let Ok(idx) = earcutr::earcut(&coords, &indices_trous, 2) {
            let pt = |k: usize| [coords[2 * k] as f32, coords[2 * k + 1] as f32];
            out.extend(idx.chunks_exact(3).map(|t| [pt(t[0]), pt(t[1]), pt(t[2])]));
        }
    }
    out
}

impl Police {
    fn glyphe(&self, c: char) -> Arc<Glyphe> {
        if let Some(g) = self.glyphes.lock().unwrap().get(&c) {
            return g.clone();
        }
        let f = FontRef::new(&self.donnees).expect("police relue");
        let id = f.charmap().map(c).or_else(|| f.charmap().map('?')).unwrap_or_default();
        let avance = f.glyph_metrics(Size::unscaled(), LocationRef::default()).advance_width(id).unwrap_or(0.5 * self.unites);
        let mut stylo = Stylo { contours: Vec::new(), courant: [0.0; 2] };
        if let Some(g) = f.outline_glyphs().get(id) {
            let _ = g.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut stylo);
        }
        let g = Arc::new(Glyphe { triangles: trianguler(stylo.contours), avance });
        self.glyphes.lock().unwrap().insert(c, g.clone());
        g
    }
}

/// Largeur (mm) d'une ligne de texte dont les capitales mesurent `taille` mm.
pub fn largeur(texte: &str, nom: &str, taille: f64, gras: bool) -> f64 {
    let p = police(nom, gras);
    let k = taille / p.capitales as f64;
    texte.chars().map(|c| p.glyphe(c).avance as f64 * k).sum()
}

/// Ligne de texte dont les capitales mesurent `taille` mm (taille de texte de Revit).
pub fn ligne(texte: &str, nom: &str, taille: f64, gras: bool) -> Ligne {
    let p = police(nom, gras);
    let k = taille / p.capitales as f64;
    let (mut triangles, mut x) = (Vec::new(), 0.0);
    for c in texte.chars() {
        let g = p.glyphe(c);
        triangles.extend(g.triangles.iter().map(|t| t.map(|q| [x + q[0] as f64 * k, q[1] as f64 * k])));
        x += g.avance as f64 * k;
    }
    Ligne { triangles, largeur: x }
}

/// Ascendante et descendante (mm, positives) de la police d'un texte dont les capitales
/// mesurent `taille` mm.
pub fn metriques(nom: &str, taille: f64, gras: bool) -> (f64, f64) {
    let p = police(nom, gras);
    let k = taille / p.capitales as f64;
    (p.ascendante as f64 * k, p.descendante.abs() as f64 * k)
}

/// Coupe un paragraphe en lignes de largeur au plus `max` mm (aux espaces) ; un mot plus
/// long que `max` reste entier.
pub fn couper(texte: &str, nom: &str, taille: f64, gras: bool, max: f64) -> Vec<String> {
    let mut out = Vec::new();
    for para in texte.split('\n') {
        let mut l = String::new();
        for mot in para.split(' ') {
            let essai = if l.is_empty() { mot.to_string() } else { format!("{l} {mot}") };
            if !l.is_empty() && max > 0.0 && largeur(&essai, nom, taille, gras) > max {
                out.push(std::mem::take(&mut l));
                l = mot.to_string();
            } else {
                l = essai;
            }
        }
        out.push(l);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les polices système sont retrouvées par nom de fichier (Windows, macOS, Linux) ;
    /// sans aucune police installée, le repli reste utilisable.
    #[test]
    fn polices_systeme() {
        let trouvee = FICHIERS[0].1.iter().any(|f| lire_fichier_police(f).is_some());
        eprintln!("Arial ou équivalent trouvé : {trouvee}");
        assert!(!ligne("Test", "Arial", 2.5, true).triangles.is_empty());
    }

    /// Un « H » de 2,5 mm de capitales : triangles dans [0, 2,5] en hauteur, aire > 0 ;
    /// un « o » a un trou (aire inférieure à celle de son disque).
    #[test]
    fn glyphes() {
        let h = ligne("H", "Arial", 2.5, false);
        let ys: Vec<f64> = h.triangles.iter().flatten().map(|p| p[1]).collect();
        let (y0, y1) = ys.iter().fold((f64::MAX, f64::MIN), |(a, b), &y| (a.min(y), b.max(y)));
        assert!(y0.abs() < 0.05 && (y1 - 2.5).abs() < 0.05, "{y0} {y1}");
        let aire = |l: &Ligne| l.triangles.iter().map(|[a, b, c]| ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0).sum::<f64>();
        assert!(aire(&h) > 1.0);
        let o = ligne("o", "Arial", 2.5, false);
        assert!(aire(&o) > 0.5 && aire(&o) < 0.7 * o.largeur * o.largeur, "{}", aire(&o));
        assert!(largeur("HH", "Arial", 2.5, false) > 1.9 * largeur("H", "Arial", 2.5, false));
        assert_eq!(couper("un deux trois", "Arial", 2.5, false, largeur("un deux", "Arial", 2.5, false) + 0.01), vec!["un deux", "trois"]);
    }
}
