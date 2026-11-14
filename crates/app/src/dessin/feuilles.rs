//! Dessin d'une vue native et de ses feuilles : scène du projet coupée et projetée,
//! mise en page sur feuille, export SVG et PDF.
use super::plans::{fond_plan, limite_haute_plan, pointilles_plage_haute};
use super::{Poche, cadrer_primitives, coupe, vu};
use modele::documentation::*;
use crate::rendu::scene::{depuis_natif, id_natif, numero_vue};
use glam::{DVec2, DVec3};
use noyau::trace2d::{Dessin, Forme, Primitive};

pub(super) fn ids_masques_vue(p: &Projet, v: &Vue) -> std::collections::HashSet<noyau::Id> {
    p.murs.iter().map(|x| ("mur", &x.id))
        .chain(p.assemblages.iter().map(|x| ("assemblage", &x.id)))
        .chain(p.dalles.iter().map(|x| ("dalle", &x.id)))
        .chain(p.barres.iter().map(|x| ("barre", &x.id))).chain(p.composants.iter().map(|x| ("composant", &x.id))).chain(p.arbres.iter().map(|x| ("arbre", &x.id)))
        .chain(p.escaliers.iter().map(|x| ("escalier", &x.id)))
        .chain(p.garde_corps.iter().map(|x| ("garde_corps", &x.id)))
        .chain(p.ouvertures.iter().map(|x| ("ouverture", &x.id)))
        .chain(p.importes.iter().map(|x| ("importe", &x.id)))
        .chain(p.terrains.iter().map(|x| ("terrain", &x.id)))
        .chain(p.limites_propriete.iter().map(|x| ("limite_propriete", &x.id)))
        .filter(|(_, id)| v.elements_masques.contains(id))
        .map(|(genre, id)| id_natif(genre, id)).collect::<std::collections::HashSet<_>>()
}

pub fn dessin_vue(p: &Projet, id: &str) -> Result<Dessin, String> {
    let mut vue = p.vues.iter().find(|v| v.id == id).ok_or("Vue absente.")?.clone();
    vue.elements_masques.extend(p.elements_masques_contextes(&p.contextes_vues.get(id).cloned().unwrap_or_default()));
    let v = &vue;
    let s = depuis_natif(p, std::path::Path::new("projet.modele.json"))?;
    let (o, mut u, mut h) = repere(p, v);
    if matches!(v.genre, GenreVue::Plan { .. }) {
        let (sin, cos) = v.orientation_plan.sin_cos();
        u = DVec3::new(cos, sin, 0.0);
        h = DVec3::new(-sin, cos, 0.0);
    }
    let oeil = u.cross(h);
    let normale = -oeil;
    let plan = normale.extend(-normale.dot(o * 0.001 - s.origin)).as_vec4();
    let elevation = matches!(
        v.genre,
        GenreVue::Coupe {
            elevation: true,
            ..
        }
    );
    let mut visible = vec![true; s.objects.len()];
    let masques = ids_masques_vue(p, v);
    for (i, objet) in s.objects.iter().enumerate() {
        visible[i] = !masques.contains(&objet.id) && !s.model.parent.get(&objet.id).is_some_and(|id| masques.contains(id));
    }
    if let GenreVue::Plan { niveau, bas_niveau, bas_decalage, .. } = &v.genre {
        let etage = id_natif("niveau", niveau);
        let contient_etage = s.objects.iter().any(|o| o.storey == Some(etage) || &*o.class == "IFCGEOGRAPHICELEMENT");
        if contient_etage {
            for (i, obj) in s.objects.iter().enumerate() {
                visible[i] &= obj.storey == Some(etage) || &*obj.class == "IFCGEOGRAPHICELEMENT";
            }
        }
        if v.coupe_active.unwrap_or(true) && let Some(niveau) = p.niveau(bas_niveau) {
            let bas = (niveau.altitude + bas_decalage) * 0.001;
            for (i, obj) in s.objects.iter().enumerate() {
                let haut = obj.max.z as f64 + s.origin.z;
                visible[i] &= if matches!(&*obj.class, "IFCSLAB" | "IFCCOVERING" | "IFCROOF" | "IFCANNOTATION" | "IFCGEOGRAPHICELEMENT") { haut >= bas - 0.002 } else { haut > bas + 0.002 };
            }
        }
    }
    if let GenreVue::Coupe {
        debut,
        fin,
        profondeur,
        ..
    } = &v.genre
    {
        let a = DVec2::from_array(*debut);
        let d = DVec2::from_array(*fin) - a;
        let u = d.normalize();
        let n = u.perp();
        for (i, obj) in s.objects.iter().enumerate() {
            let corners = (0..8).map(|k| {
                DVec3::new(
                    if k & 1 == 0 { obj.min.x } else { obj.max.x } as f64,
                    if k & 2 == 0 { obj.min.y } else { obj.max.y } as f64,
                    if k & 4 == 0 { obj.min.z } else { obj.max.z } as f64,
                )
            });
            let points: Vec<_> = corners
                .map(|q| (q + s.origin).truncate() * 1000.0 - a)
                .collect();
            visible[i] &= points.iter().any(|q| q.dot(u) >= 0.0)
                && points.iter().any(|q| q.dot(u) <= d.length())
                && points.iter().any(|q| q.dot(n) >= 0.0)
                && points.iter().any(|q| q.dot(n) <= *profondeur);
        }
    }
    let actif = v.coupe_active.unwrap_or(true);
    let poche = if p.rendus_vues.get(id).is_some_and(|r| r.style.as_deref() == Some("realiste")) {
        Poche::Materiau
    } else { match v.poche_vue.as_deref() {
        Some("plein") => Poche::Plein,
        Some("vide") => Poche::Vide,
        Some("materiau") => Poche::Materiau,
        _ => Poche::Hachure,
    }};
    let mut primitives = Vec::new();
    if actif && !elevation {
        primitives.extend(fond_plan(p, &s, v, plan, v.echelle));
        primitives.extend(coupe(&s, &visible, plan, v.echelle, poche));
        if let Some(limite) = limite_haute_plan(p, &s, v, plan.w)
            && let GenreVue::Plan { niveau, .. } = &v.genre
        {
            let niveau = id_natif("niveau", niveau);
            let au_dessus: Vec<_> = s.objects.iter().zip(&visible).map(|(obj, visible)| {
                *visible && obj.storey == Some(niveau) && obj.min.z > plan.w + 1e-5 && obj.min.z < limite
            }).collect();
            let aretes = vu(&s, &au_dessus, u, h, oeil, Some(glam::Vec3::NEG_Z.extend(limite)), 1024);
            primitives.extend(pointilles_plage_haute(aretes, v.echelle, plan.w as f64));
        }
    }
    primitives.extend(vu(
        &s,
        &visible,
        u,
        h,
        oeil,
        actif.then_some(plan),
        4096,
    ));
    if let Some(nav) = s.navigateur.as_ref().and_then(|n| n.vues.iter().find(|nv| nv.revit_id == numero_vue(id))) {
        let plans: Vec<_> = nav.cadrage.iter().map(|plan| {
            let n = plan.truncate();
            n.extend(plan.w + n.dot(s.origin)).as_vec4()
        }).collect();
        primitives = cadrer_primitives(primitives, &plans, oeil);
    }
    // Revenir au repère absolu, comme les annotations.
    let mut primitives: Vec<_> = primitives
        .into_iter()
        .map(|pr| pr.translater(s.origin))
        .collect();
    primitives.extend(annotations(p, v));
    let primitives = modele::elements::cadrage::decouper(primitives, &v.contour_cadrage)?;
    Ok(Dessin {
        droite: u,
        haut: h,
        echelle: v.echelle,
        primitives,
    })
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
