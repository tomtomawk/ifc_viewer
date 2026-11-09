//! Feuilles : jeux, cartouches, vues et nomenclatures placées, export SVG et PDF.
use super::*;

const TAILLE_CARTOUCHE_STANDARD: [f64; 2] = [180.0, 40.0];

impl Default for FamilleCartouche {
    fn default() -> Self {
        Self {
            id: crate::modele::collaboration::uuid(),
            nom: "Cartouche standard".into(), largeur: TAILLE_CARTOUCHE_STANDARD[0], hauteur: TAILLE_CARTOUCHE_STANDARD[1],
            traits: vec![[[0.0,0.0],[180.0,0.0]], [[180.0,0.0],[180.0,40.0]], [[180.0,40.0],[0.0,40.0]], [[0.0,40.0],[0.0,0.0]], [[140.0,0.0],[140.0,40.0]], [[0.0,14.0],[140.0,14.0]], [[0.0,30.0],[140.0,30.0]]],
            textes: vec![
                TexteCartouche { contenu: "{projet}".into(), position: [3.0,33.0], hauteur: 3.5 },
                TexteCartouche { contenu: "{feuille}".into(), position: [3.0,20.0], hauteur: 5.0 },
                TexteCartouche { contenu: "{cartouche}".into(), position: [3.0,9.0], hauteur: 2.5 },
                TexteCartouche { contenu: "{echelles}".into(), position: [3.0,3.5], hauteur: 2.5 },
                TexteCartouche { contenu: "{numero}".into(), position: [144.0,15.0], hauteur: 9.0 },
            ],
            formes: Vec::new(),
        }
    }
}

/// Jeux de feuilles du projet, dans l'ordre de leur première feuille.
pub fn jeux(p: &Projet) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for f in &p.feuilles {
        if !v.contains(&f.jeu) {
            v.push(f.jeu.clone());
        }
    }
    v
}

/// Déplace la feuille `id` dans le jeu `jeu`, juste avant la feuille `avant` ; sans
/// `avant` (ou s'il n'est pas dans ce jeu), après la dernière feuille du jeu.
pub fn deplacer_feuille(p: &mut Projet, id: &str, jeu: &str, avant: Option<&str>) -> Result<(), String> {
    let k = p.feuilles.iter().position(|f| f.id == id).ok_or("Feuille absente.")?;
    let mut f = p.feuilles.remove(k);
    f.jeu = jeu.into();
    let rang = avant
        .filter(|a| *a != id)
        .and_then(|a| p.feuilles.iter().position(|g| g.id == a && g.jeu == jeu))
        .or_else(|| p.feuilles.iter().rposition(|g| g.jeu == jeu).map(|r| r + 1))
        .unwrap_or(p.feuilles.len());
    p.feuilles.insert(rang, f);
    Ok(())
}

pub(super) fn pt_papier(x: f64, y: f64) -> DVec3 {
    DVec3::new(x / 1000.0, y / 1000.0, 0.0)
}

pub(super) fn trait_papier(points: Vec<DVec3>, fermee: bool, epaisseur: f64) -> Primitive {
    Primitive { forme: Forme::Polyligne { points, fermee }, style: Style { couleur: [20, 20, 20, 255], epaisseur }, role: Role::Annotation, element: None }
}

pub fn famille_cartouche<'a>(p: &'a Projet, feuille: &str) -> Option<&'a FamilleCartouche> {
    let nom = p.cartouches_feuilles.get(feuille)?;
    p.familles_cartouches.iter().find(|f| &f.nom == nom)
}

pub fn cadre_cartouche(p: &Projet, f: &Feuille) -> Option<[f64; 4]> {
    let [l, h] = famille_cartouche(p, &f.id).map_or(TAILLE_CARTOUCHE_STANDARD, |c| [c.largeur, c.hauteur]);
    let [x, y] = [f.largeur - MARGE - l, MARGE + h];
    (x > MARGE && y < f.hauteur - MARGE).then_some([x, MARGE, f.largeur - MARGE, y])
}

pub fn trait_cartouche(t: [[f64; 2]; 2]) -> Primitive {
    trait_papier(t.map(|[x, y]| pt_papier(x, y)).to_vec(), false, 0.25)
}

pub fn texte_cartouche(t: &TexteCartouche, contenu: &str) -> Primitive {
    texte(contenu, pt_papier(t.position[0], t.position[1]), DVec3::X, DVec3::Y, t.hauteur / 1000.0, [20, 20, 20, 255])
}

pub fn forme_cartouche(p: &Projet, forme: &AnnotationForme) -> Vec<Primitive> {
    let pt = |v: DVec2| pt_papier(v.x, v.y);
    // cotes : même géométrie que dans les vues, en mm du papier (texte de 2,5 mm)
    let longueur = |x: f64| format!("{x:.1} mm");
    let altitude = |q: [f64; 2], _| q[1];
    let style = StyleCote { oblique: 1.5, taille: 2.5, longueur: &longueur, altitude: &altitude };
    if let Some((traits, textes)) = GenreCote::de_forme(forme).and_then(|_| geometrie(forme, None, &style)) {
        let mut out: Vec<_> = traits.iter().map(|[x, y]| ligne(pt(*x), pt(*y))).collect();
        out.extend(textes.into_iter().map(|(pos, dir, s)| {
            let (du, dv) = (DVec3::new(dir.x, dir.y, 0.0), DVec3::new(-dir.y, dir.x, 0.0));
            let l = crate::noyau::texte::largeur(&s, "Arial", 0.0025, false);
            texte(&s, pt(pos) - du * (l / 2.0), du, dv, 0.0025, [20, 20, 20, 255])
        }));
        out
    } else {
        primitives_forme_dessin(p, forme, 1.0, &pt)
    }
}

/// Cadre et cartouche d'une feuille, en mètres du papier (origine en bas à gauche) :
/// bord, cadre intérieur, cartouche en bas à droite (projet, cartouche libre, titre,
/// numéro, échelles des vues placées).
pub fn cadre_feuille(p: &Projet, f: &Feuille) -> Vec<Primitive> {
    let (l, h, m) = (f.largeur, f.hauteur, MARGE);
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64, e: f64| trait_papier(vec![pt_papier(x0, y0), pt_papier(x1, y0), pt_papier(x1, y1), pt_papier(x0, y1)], true, e);
    let mut v = vec![rect(0.0, 0.0, l, h, 0.0), rect(m, m, l - m, h - m, 0.5)];
    if let Some([x0, y0, _, _]) = cadre_cartouche(p, f) {
        let standard;
        let famille = if let Some(famille) = famille_cartouche(p, &f.id) {
            famille
        } else {
            standard = FamilleCartouche::default();
            &standard
        };
        let origine = pt_papier(x0, y0);
        v.extend(famille.formes.iter().flat_map(|forme| forme_cartouche(p, forme)).map(|pr| pr.translater(origine)));
        v.extend(famille.traits.iter().map(|t| trait_cartouche(*t).translater(origine)));
        let mut echelles: Vec<String> = f.vues.iter().map(|x| format!("1:{}", x.echelle_sur_feuille(p))).collect();
        echelles.dedup();
        let ech = if echelles.is_empty() { String::new() } else { format!("Échelle {}", echelles.join(", ")) };
        for t in &famille.textes {
            let contenu = t.contenu.replace("{projet}", &p.nom).replace("{feuille}", &f.nom).replace("{numero}", &f.numero).replace("{cartouche}", &f.cartouche).replace("{echelles}", &ech);
            if !contenu.is_empty() {
                v.push(texte_cartouche(t, &contenu).translater(origine));
            }
        }
    }
    v
}

/// Vue placée sur une feuille : dessin à l'échelle de sa vue source, en mètres du
/// papier (origine en bas à gauche), suivi de son titre (nom, échelle) souligné ; et
/// l'étendue du dessin seul, en mm du papier [x0, y0, x1, y1].
pub fn vue_placee(p: &Projet, v: &VuePlacee) -> Result<(Vec<Primitive>, [f64; 4]), String> {
    let vue = p.vues.iter().find(|x| x.id == v.vue).ok_or("Vue absente.")?;
    let dessin = dessin_vue(p, &v.vue)?;
    let echelle = dessin.echelle;
    let (origine, _, _) = repere(p, vue);
    let proj = |q: DVec3| {
        let q = q - origine * 0.001;
        pt_papier(v.position[0] + q.dot(dessin.droite) * 1000.0 / echelle, v.position[1] + q.dot(dessin.haut) * 1000.0 / echelle)
    };
    let mut primitives = Vec::new();
    let mut e = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for mut pr in dessin.primitives {
        pr.forme = match pr.forme {
            Forme::Polyligne { points, fermee } => Forme::Polyligne { points: points.into_iter().map(proj).collect(), fermee },
            Forme::Segments(s) => Forme::Segments(s.into_iter().map(|s| s.map(proj)).collect()),
            Forme::Triangles(t) => Forme::Triangles(t.into_iter().map(|t| t.map(proj)).collect()),
            Forme::Surface(c) => Forme::Surface(c.into_iter().map(|c| c.into_iter().map(proj).collect()).collect()),
        };
        let points: Box<dyn Iterator<Item = &DVec3> + '_> = match &pr.forme {
            Forme::Polyligne { points, .. } => Box::new(points.iter()),
            Forme::Segments(s) => Box::new(s.iter().flatten()),
            Forme::Triangles(t) => Box::new(t.iter().flatten()),
            Forme::Surface(c) => Box::new(c.iter().flatten()),
        };
        for q in points {
            e = [e[0].min(q.x * 1000.0), e[1].min(q.y * 1000.0), e[2].max(q.x * 1000.0), e[3].max(q.y * 1000.0)];
        }
        primitives.push(pr);
    }
    if e[0] > e[2] {
        e = [v.position[0], v.position[1], v.position[0] + 10.0, v.position[1] + 10.0];
    }
    // titre sous le dessin : nom, échelle, trait
    let y = e[1] - 8.0;
    let noir = [20, 20, 20, 255];
    primitives.push(texte(&vue.nom, pt_papier(e[0], y), DVec3::X, DVec3::Y, 0.004, noir));
    primitives.push(texte(&format!("1:{echelle}"), pt_papier(e[0], y - 5.0), DVec3::X, DVec3::Y, 0.0025, noir));
    primitives.push(trait_papier(vec![pt_papier(e[0], y - 1.2), pt_papier(e[0] + 60.0_f64.min(e[2] - e[0]).max(30.0), y - 1.2)], false, 0.35));
    Ok((primitives, e))
}

/// Tableau d'une nomenclature posée sur une feuille, en mètres du papier : titre
/// (nom), ligne d'en-tête, lignes ; colonnes à la largeur de leur plus long texte. Et
/// son étendue en mm du papier [x0, y0, x1, y1].
pub fn nomenclature_placee(p: &Projet, n: &NomenclaturePlacee) -> Result<(Vec<Primitive>, [f64; 4]), String> {
    let nom = p.nomenclatures.iter().find(|x| x.id == n.nomenclature).ok_or("Nomenclature absente.")?;
    let lignes = tableau(p, nom)?;
    // mm du papier : texte 2,5, en-tête et titre plus grands
    let (texte_h, ligne_h, marge, titre_h) = (2.5, 6.0, 2.0, 3.5);
    let largeur = |s: &str, h: f64| crate::noyau::texte::ligne(s, "Arial", h, false).largeur;
    let ncol = lignes.iter().map(Vec::len).max().unwrap_or(0);
    let mut colonnes = vec![12.0_f64; ncol];
    for l in &lignes {
        for (i, c) in l.iter().enumerate() {
            colonnes[i] = colonnes[i].max(largeur(c, texte_h) + 2.0 * marge);
        }
    }
    let total: f64 = colonnes.iter().sum::<f64>().max(largeur(&nom.nom, titre_h) + 2.0 * marge);
    let [x0, y0] = n.position;
    let noir = [20, 20, 20, 255];
    let mut prims = Vec::new();
    prims.push(texte(&nom.nom, pt_papier(x0 + marge, y0 - titre_h - 1.5), DVec3::X, DVec3::Y, titre_h / 1000.0, noir));
    let haut = y0 - titre_h - 4.0;
    let bas = haut - ligne_h * lignes.len() as f64;
    // cadre, lignes, colonnes ; en-tête souligné plus fort
    prims.push(trait_papier(vec![pt_papier(x0, haut), pt_papier(x0 + total, haut), pt_papier(x0 + total, bas), pt_papier(x0, bas)], true, 0.35));
    for k in 1..lignes.len() {
        let y = haut - ligne_h * k as f64;
        prims.push(trait_papier(vec![pt_papier(x0, y), pt_papier(x0 + total, y)], false, if k == 1 { 0.35 } else { 0.13 }));
    }
    let mut x = x0;
    for w in colonnes.iter().take(ncol.saturating_sub(1)) {
        x += w;
        prims.push(trait_papier(vec![pt_papier(x, haut), pt_papier(x, bas)], false, 0.13));
    }
    for (k, l) in lignes.iter().enumerate() {
        let y = haut - ligne_h * (k as f64 + 1.0) + (ligne_h - texte_h) * 0.5;
        let mut x = x0;
        for (i, c) in l.iter().enumerate() {
            if !c.is_empty() {
                prims.push(texte(c, pt_papier(x + marge, y), DVec3::X, DVec3::Y, texte_h / 1000.0, noir));
            }
            x += colonnes[i];
        }
    }
    Ok((prims, [x0, bas, x0 + total, y0]))
}

pub fn feuille(p: &Projet, f: &Feuille) -> Result<Dessin, String> {
    let mut primitives = cadre_feuille(p, f);
    for v in &f.vues {
        primitives.extend(vue_placee(p, v)?.0);
    }
    for n in &f.nomenclatures {
        primitives.extend(nomenclature_placee(p, n)?.0);
    }
    Ok(Dessin { droite: DVec3::X, haut: DVec3::Y, echelle: 1.0, primitives })
}

pub fn csv(lignes: &[Vec<String>]) -> String {
    lignes
        .iter()
        .map(|l| {
            l.iter()
                .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(";")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n"
}

/// Feuille au format papier exact, sans la marge automatique des exports de vues.
pub fn svg_feuille(p: &Projet, f: &Feuille) -> Result<String, String> {
    let mut svg = feuille(p, f)?.svg(&format!("{} — {}", f.numero, f.nom));
    let a = svg.find("<svg ").ok_or("SVG absent")?;
    let b = a + svg[a..].find('>').ok_or("SVG invalide")? + 1;
    svg.replace_range(a..b,&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 -{} {} {}\">",f.largeur,f.hauteur,f.hauteur,f.largeur,f.hauteur));
    Ok(svg)
}

pub fn pdf(svg: &str) -> Result<Vec<u8>, String> {
    let tree = svg2pdf::usvg::Tree::from_str(svg, &svg2pdf::usvg::Options::default())
        .map_err(|e| format!("SVG : {e}"))?;
    // usvg interprète les mm avec 96 px/pouce ; conserver la taille physique du papier.
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions { dpi: 96.0 },
    )
    .map_err(|e| format!("PDF : {e}"))
}

/// PDF de plusieurs pages, une par SVG (chacune à son format, comme `pdf`).
pub fn pdf_pages(svgs: &[String]) -> Result<Vec<u8>, String> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
    if svgs.is_empty() {
        return Err("Aucune page à exporter.".into());
    }
    let mut alloc = Ref::new(1);
    let (catalogue, arbre) = (alloc.bump(), alloc.bump());
    let mut pages = Vec::new();
    let mut morceaux = Vec::new();
    for svg in svgs {
        let tree = svg2pdf::usvg::Tree::from_str(svg, &svg2pdf::usvg::Options::default()).map_err(|e| format!("SVG : {e}"))?;
        let (morceau, x) = svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default()).map_err(|e| format!("PDF : {e}"))?;
        let mut renum = std::collections::HashMap::new();
        let morceau = morceau.renumber(|ancien| *renum.entry(ancien).or_insert_with(|| alloc.bump()));
        let x = *renum.get(&x).ok_or("PDF : objet de page absent")?;
        // usvg lit les mm à 96 px/pouce ; points PDF à 72 par pouce
        let (l, h) = (tree.size().width() * 0.75, tree.size().height() * 0.75);
        pages.push((alloc.bump(), alloc.bump(), x, l, h));
        morceaux.push(morceau);
    }
    let mut pdf = Pdf::new();
    pdf.catalog(catalogue).pages(arbre);
    pdf.pages(arbre).kids(pages.iter().map(|p| p.0)).count(pages.len() as i32);
    for &(page, contenu, x, l, h) in &pages {
        let mut pg = pdf.page(page);
        pg.media_box(Rect::new(0.0, 0.0, l, h));
        pg.parent(arbre);
        pg.contents(contenu);
        let mut ressources = pg.resources();
        ressources.x_objects().pair(Name(b"F"), x);
        ressources.finish();
        pg.finish();
        let mut c = Content::new();
        c.transform([l, 0.0, 0.0, h, 0.0, 0.0]).x_object(Name(b"F"));
        pdf.stream(contenu, &c.finish());
    }
    for m in &morceaux {
        pdf.extend(m);
    }
    Ok(pdf.finish())
}

/// PDF d'un jeu de feuilles : ses feuilles dans l'ordre, une page chacune.
pub fn pdf_jeu(p: &Projet, jeu: &str) -> Result<Vec<u8>, String> {
    let svgs = p.feuilles.iter().filter(|f| f.jeu == jeu).map(|f| svg_feuille(p, f)).collect::<Result<Vec<_>, _>>()?;
    pdf_pages(&svgs)
}
