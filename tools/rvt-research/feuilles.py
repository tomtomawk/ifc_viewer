"""Feuilles d'un fichier Revit : fenêtres de vue et cartouches (lus sans Revit).

Usage : python -I feuilles.py <fichier> [--json sortie.json]

Une feuille est un DBViewDrafting (m_viewName, m_sheetNumber). Ses fenêtres de vue sont
des Viewport (m_ownerDBViewId = la feuille) : vue placée m_dbViewId, échelle m_scale,
m_offset = position sur la feuille de l'origine de la vue (m_origin), titre m_labelOrigin
(sur la feuille) et m_labelLength. Un point p du modèle se place sur la feuille en
    m_offset + m_scale · ((p − origine)·droite, (p − origine)·haut).
Contour de la vue (View.Outline) : son cadrage (Viewer.m_boundedSpace : origine m_orig,
axes m_basis, bornes [max, min] m_boundOffset sur chaque axe) quand il est actif
(m_boundActive), projeté comme ci-dessus ; boîte de la fenêtre de vue (GetBoxOutline) :
ce contour élargi de 0,01 pied (3,048 mm) de chaque côté. Les annotations qui sortent du
cadrage (cadrage d'annotation inactif) élargissent le contour de Revit : non calculé ici.

Cartouches et autres instances propres à la feuille : FamilyInstance dont
m_ownerDBViewId = la feuille ; repère m_pInstanceInfo.m_Trf (comme GetTransform, la
colonne j de m_3x3 étant l'axe j ; échelle de la vue comprise pour une annotation).
--args f.json : prédictions pour le script RiveTT rvt-compare-feuilles (fenêtres de vue,
repères de toutes les instances propres à une vue, traits des cartouches avec --traits).

Longueurs en mm sur le papier (Revit stocke des pieds).
"""
import json, math, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from champs import FT
from elements import Modele, eid

MARGE = 0.01 * FT   # marge de la boîte d'une fenêtre de vue autour du contour


def corps(p):
    return p['corps'] if isinstance(p, dict) and 'corps' in p else None


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def cadrage(r):
    """Cadrage d'un Viewer : origine, axes, bornes [(min, max)] en pieds, actif."""
    b = r.get('m_boundedSpace')
    if not b:
        return None
    bornes = [(o[1], o[0]) for o in b['m_boundOffset']]
    actif = [a[0] == 1 and a[1] == 1 for a in b['m_boundActive']]
    return {'orig': b['m_orig'], 'axes': b['m_basis'], 'bornes': bornes, 'actif': actif,
            'drapeaux': r.get('m_viewerFlags'),
            'annotation': (corps(r.get('m_pAnnotationCropOffsets')) or {}).get('m_leftRightTopBottomOffsets')}


def contour(v, c):
    """Contour (umin, vmin, umax, vmax) en mm du papier, relatif à l'origine de la vue,
    d'après les huit coins du cadrage ; None si le cadrage n'est pas actif."""
    if not c or not (c['actif'][0] and c['actif'][1]):
        return None
    s = v['m_scale']
    us, vs = [], []
    (a0, b0), (a1, b1), (a2, b2) = c['bornes']
    for x in (a0, b0):
        for y in (a1, b1):
            for z in (a2, b2):
                p = [c['orig'][i] + x * c['axes'][0][i] + y * c['axes'][1][i] + z * c['axes'][2][i] - v['m_origin'][i] for i in range(3)]
                us.append(dot(p, v['m_horzDir']) * s * FT)
                vs.append(dot(p, v['m_vertDir']) * s * FT)
    return [min(us), min(vs), max(us), max(vs)]


def feuilles(m):
    vues = {e: r for e, r in m.de('DBView', True)}
    cadrages = {}
    for e, r in m.de('Viewer', True):
        if eid(r.get('m_dbViewId', -1)) != -1:
            cadrages[eid(r['m_dbViewId'])] = cadrage(r)
    symboles = {}
    for e, r in m.de('Symbol', True):
        si = corps(r.get('m_symbolInfo'))
        symboles[e] = (si or {}).get('m_name'), eid(r.get('m_familyId', -1))
    familles = {e: r.get('m_name') for e, r in m.de('Family', True)}
    out = {e: {'id': e, 'nom': r.get('m_viewName'), 'numero': r.get('m_sheetNumber'), 'fenetres': [], 'instances': []}
           for e, r in vues.items() if r['_classe'] == 'DBViewDrafting' and not r.get('m_isTemplate')}
    for e, r in m.de('Viewport', True):
        f = eid(r.get('m_ownerDBViewId', -1))
        if f not in out:
            continue
        vid = eid(r['m_dbViewId'])
        v = vues.get(vid)
        o = {'id': e, 'vue': vid, 'nom': v.get('m_viewName') if v else None, 'echelle': round(1 / r['m_scale']),
             'decalage': [x * FT for x in r['m_offset'][:2]], 'titre': [x * FT for x in r['m_labelOrigin'][:2]],
             'longueur_titre': (r.get('m_labelLength') or 0) * FT}
        c = cadrages.get(vid)
        if v and c:
            o['cadrage_actif'] = c['actif'][0] and c['actif'][1]
            o['drapeaux'] = c['drapeaux']
            k = contour(v, c)
            if k:
                d = o['decalage']
                o['contour'] = k
                o['centre'] = [d[0] + (k[0] + k[2]) / 2, d[1] + (k[1] + k[3]) / 2]
                o['boite'] = [d[0] + k[0] - MARGE, d[1] + k[1] - MARGE, d[0] + k[2] + MARGE, d[1] + k[3] + MARGE]
        out[f]['fenetres'].append(o)
    autres = []
    for e, r in m.de('FamilyInstance', True):
        f = eid(r.get('m_ownerDBViewId', -1))
        if f == -1:
            continue
        t = eid(r['m_masterSymbolId'])
        nom, fam = symboles.get(t, (None, -1))
        trf = corps(r.get('m_pInstanceInfo'))['m_Trf']
        i = {'id': e, 'vue': f, 'type': t, 'famille': familles.get(fam), 'nom_type': nom,
             'origine': [x * FT for x in trf['m_or']], 'axes': trf['m_3x3'],
             'point': [x * FT for x in r.get('m_instOrigin') or [0, 0, 0]]}
        # instances des feuilles avec leur feuille, les autres (vues) à part
        (out[f]['instances'] if f in out else autres).append(i)
    m.instances_vues = autres
    return [out[k] for k in sorted(out)]


def traits(path, type_id):
    """Traits (polylignes en mm, repère du type) du GElement d'un type 2D (cartouche,
    symbole), filtré comme une vue 3D en détail fin (familles.py) ; familles imbriquées
    (GInstance) comprises."""
    import math
    import familles as F
    out = []

    def courbe(c, T):
        t0, t1 = c['m_endParams']
        if c['_classe'] == 'GLine':
            pts = [[c['m_origin'][i] + t * c['m_dirVec'][i] for i in range(3)] for t in (t0, t1)]
        elif c['_classe'] == 'GArc':
            n = max(2, math.ceil(abs(t1 - t0) / math.radians(5)))
            pts = []
            for k in range(n + 1):
                t = t0 + (t1 - t0) * k / n
                pts.append([c['m_center'][i] + c['m_radius'] * (math.cos(t) * c['m_xVec'][i] + math.sin(t) * c['m_yVec'][i]) for i in range(3)])
        else:
            return
        out.append([[x * FT for x in F.appliquer(T, p)] for p in pts])

    def parcourir(n, T, prof=0):
        k = n['_classe']
        if n['m_GInfo']['m_flags'] & F.INVISIBLE:
            return
        if k == 'GFilter' and not all(F.condition(x['corps']) for x in n.get('m_oConditions') or []):
            return
        if k in ('GLine', 'GArc'):
            courbe(n, T)
        if k == 'GInstance' and prof < 16:
            ii = n['m_instanceInfo']['corps']
            T2 = F.composer(T, ([list(r) for r in ii['m_Trf']['m_3x3']], ii['m_Trf']['m_or']))
            emb = n.get('m_oEmbeddedSymbolGRep')
            g = (emb['corps'], None) if emb and emb.get('corps') else G.gelement(path, eid(ii['m_symbolId']))
            parcourir(g[0], T2, prof + 1)
        for sp in n.get('m_subNodes') or []:
            if sp:
                parcourir(sp['corps'], T, prof)

    import geometrie as G
    parcourir(G.gelement(path, type_id)[0], F.ID)
    return out


def main():
    a = sys.argv[1:]
    m = Modele(a[0])
    res = feuilles(m)
    if '--traits' in a:
        # traits des cartouches placés : polylignes sur la feuille (repère de l'instance)
        import familles as F
        for f in res:
            for i in f['instances']:
                T = ([list(r) for r in i['axes']], [x / FT for x in i['origine']])
                pl = [[[x * FT for x in F.appliquer(T, [c / FT for c in p])] for p in l] for l in traits(a[0], i['type'])]
                lg = sum(math.dist(p, q) for l in pl for p, q in zip(l, l[1:])) if pl else 0
                xs = [p[0] for l in pl for p in l]
                ys = [p[1] for l in pl for p in l]
                i['traits'] = {'n': len(pl), 'longueur': lg, 'boite': [min(xs), min(ys), max(xs), max(ys)] if xs else None}
    for f in res:
        print('%10d %-10s %s' % (f['id'], f['numero'], f['nom']))
        for o in f['fenetres']:
            print('   fenêtre %d vue %d %-28s 1:%-4d centre %s boîte %s' % (
                o['id'], o['vue'], o['nom'], o['echelle'],
                ['%.3f' % x for x in o['centre']] if 'centre' in o else '—',
                ['%.3f' % x for x in o['boite']] if 'boite' in o else '—'))
        for i in f['instances']:
            print('   instance %d %s / %s origine %s' % (i['id'], i['famille'], i['nom_type'], ['%.3f' % x for x in i['origine']]))
    if '--json' in a:
        json.dump(res, open(a[a.index('--json') + 1], 'w', encoding='utf-8'), ensure_ascii=False, indent=1)
    if '--args' in a:
        # arguments du script RiveTT rvt-compare-feuilles (prédictions à comparer à l'API)
        fen = [{k: o[k] for k in ('id', 'centre', 'boite') if k in o} for f in res for o in f['fenetres']]
        ins = [{'id': i['id'], 'origine': i['origine'], 'axes': i['axes'], 'traits': i.get('traits')} for f in res for i in f['instances']]
        ins += [{'id': i['id'], 'origine': i['origine'], 'axes': i['axes']} for i in getattr(m, 'instances_vues', [])]
        json.dump({'path': os.path.abspath(a[0]), 'fenetres': fen, 'instances': ins},
                  open(a[a.index('--args') + 1], 'w', encoding='utf-8'), ensure_ascii=False)


if __name__ == '__main__':
    main()
