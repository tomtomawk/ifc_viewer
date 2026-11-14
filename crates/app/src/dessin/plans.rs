//! Éléments complémentaires du plan, partagés par la vue et le dessin sur feuille.
use super::feuilles::ids_masques_vue;
use super::Poche;
use modele::documentation::*;
use crate::rendu::scene::{Scene, id_natif};
use glam::DVec3;
use noyau::trace2d::{Primitive, Role};

pub fn limite_haute_plan(p: &Projet, s: &Scene, v: &Vue, coupe: f32) -> Option<f32> {
    let GenreVue::Plan { niveau, .. } = &v.genre else { return None };
    let haut = p.niveau(v.limite_haute_niveau.as_deref().unwrap_or(niveau))?;
    let limite = ((haut.altitude + v.limite_haute_decalage) * 0.001 - s.origin.z) as f32;
    (limite > coupe).then_some(limite)
}

pub fn fond_plan(p: &Projet, s: &Scene, v: &Vue, plan: glam::Vec4, echelle: f64) -> Vec<Primitive> {
    if !matches!(v.genre, GenreVue::Plan { .. }) { return Vec::new(); }
    let Some(source) = v.fond_plan.as_ref().and_then(|id| p.vues.iter().find(|x| &x.id == id)) else { return Vec::new(); };
    let GenreVue::Plan { niveau, coupe, .. } = &source.genre else { return Vec::new(); };
    let Some(n) = p.niveau(niveau) else { return Vec::new(); };
    let z = (n.altitude + coupe) * 0.001 - s.origin.z;
    let niveau = id_natif("niveau", niveau);
    let mut source = source.clone();
    let filtre = p.contextes_vues.get(&v.id).cloned().unwrap_or_default().sur(&p.contextes_vues.get(&source.id).cloned().unwrap_or_default());
    source.elements_masques.extend(p.elements_masques_contextes(&filtre));
    let exclus = ids_masques_vue(p, &source);
    let visible: Vec<_> = s.objects.iter().map(|o| o.storey == Some(niveau)
        && !exclus.contains(&o.id) && !s.model.parent.get(&o.id).is_some_and(|id| exclus.contains(id))).collect();
    let mut prims = super::coupe(s, &visible, glam::Vec3::NEG_Z.extend(z as f32), echelle, Poche::Vide);
    prims.retain(|p| matches!(p.role, Role::Coupe | Role::Vu));
    prims.extend(annotations(p, &source).into_iter().filter(|p| p.role == Role::Annotation).map(|p| p.translater(-s.origin)));
    for pr in &mut prims {
        *pr = pr.translater(DVec3::Z * (plan.w as f64 - z - 0.001));
        pr.style.couleur = [165, 165, 165, 255];
        pr.style.epaisseur = 0.0;
        pr.role = Role::Poche; // derrière le poché de la vue courante
    }
    prims
}

pub fn pointilles_plage_haute(mut prims: Vec<Primitive>, echelle: f64, coupe: f64) -> Vec<Primitive> {
    // Pointillés calés sur une phase commune à chaque droite du plan : les arêtes
    // superposées en projection (dessus et dessous d'un élément, éléments voisins,
    // tronçons découpés par les parties cachées) tracent les mêmes tirets au lieu
    // de tirets décalés qui se chevauchent. Doublons retirés.
    let longueur=(echelle*0.003).max(0.01);let pas=longueur*2.5;
    let z = coupe - 0.0005;
    let mut vus=std::collections::HashSet::new();
    for p in &mut prims {
        let noyau::trace2d::Forme::Segments(segments)=&p.forme else{continue};
        let mut pointilles=Vec::new();
        for segment in segments {
            let (mut a,mut b)=(glam::DVec2::new(segment[0].x,segment[0].y),glam::DVec2::new(segment[1].x,segment[1].y));
            let l=a.distance(b);
            if l<=1e-6 {continue;}
            let mut u=(b-a)/l;
            if u.x< -1e-9 || (u.x.abs()<=1e-9 && u.y<0.0) {std::mem::swap(&mut a,&mut b);u=-u;}
            let cle=|x:f64|(x*1e4).round() as i64;
            let decalage=u.perp_dot(a);
            if !vus.insert((cle(u.x),cle(u.y),cle(decalage),cle(a.dot(u)),cle(b.dot(u)))) {continue;}
            let (s0,s1)=(a.dot(u),b.dot(u));
            if (s1-s0)/pas>20000.0 {continue;}
            let mut k=(s0/pas).floor();
            while k*pas<s1 {
                let (t0,t1)=((k*pas).max(s0),(k*pas+longueur).min(s1));
                if t1>t0+1e-9 {
                    let q=|t:f64|{let v=a+u*(t-s0);glam::DVec3::new(v.x,v.y,z)};
                    pointilles.push([q(t0),q(t1)]);
                }
                k+=1.0;
            }
        }
        p.forme=noyau::trace2d::Forme::Segments(pointilles);
    }
    prims
}
