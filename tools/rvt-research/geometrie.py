"""Géométrie calculée d'un élément Revit : le GElement de la section 0x67.

Usage : python -I geometrie.py <fichier> <ElementId> [--obj sortie.obj] [--tout]

Méthode (docs/LISEZMOI-RVT.md § 7, « Géométrie calculée ») : le GElement est un
arbre de GGroup ; chaque Geometry (GBRep) est un B-rep. Une face (Face) porte une
surface (Plane, CylSurf) et une chaîne de boucles (EdgeLoop.m_nextLoop). Une boucle
et ses arêtes forment une liste circulaire : EdgeLoop.m_next → arête, puis
Edge.m_next[s] jusqu'au retour à la boucle, s étant le côté de l'arête qui touche
la face (Edge.m_pFace[s]). Une arête ne porte pas de courbe 3D : ses points
(m_firstAndLastEdgePnts, avec m_interiorEdgePnts entre les deux pour une arête
courbe) sont donnés en (u, v) sur chacune de ses deux faces. Le côté s parcourt
l'arête du premier au dernier point si s = m_flags & 1, en sens inverse sinon.
Parcourue ainsi, une boucle extérieure tourne dans le sens direct autour de la
normale sortante, un trou en sens inverse ; le bit 0x02 de GFace.m_faceFlags_v9
indique une normale sortante opposée à celle de la surface (x × y pour un Plane).
Unités internes : pieds ; les sommets rendus sont en mm.

--obj : écrit le maillage au format Wavefront OBJ (mm) ; --tout : garde aussi les
feuilles (Geometry ouvertes, comme les plans de référence d'un mur).
"""
import math, os, sys
from functools import lru_cache

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from champs import FT, Lecteur, load
from objets import id_size, records, sections


class Inconnu(Exception):
    """Donnée de géométrie non décodée ou incohérente."""


@lru_cache(maxsize=4)
def complements(path):
    """(schéma, {id: octets}) de la section 0x67, état le plus récent (une
    partition plus récente remplace la précédente)."""
    c, by, sch, parts = load(path)
    cur = {}
    for p in parts:
        raw = c.read(by[p])
        for e, body in records(sections(raw).get(0x67, b''), True, id_size(sch)):
            cur[e] = body
    return sch, cur


def gelement(path, eid):
    """(racine GElement, {numéro d'objet: corps}) du complément 0x67 de l'élément."""
    sch, cur = complements(path)
    body = cur.get(eid)
    if body is None:
        raise Inconnu('élément %d sans enregistrement 0x67' % eid)
    r = Lecteur(sch, body)
    root = r.element()
    if root['_classe'] != 'GElement':
        raise Inconnu('enregistrement 0x67 de classe %s, pas de géométrie' % root['_classe'])
    return root, {p['num']: p['corps'] for p in r.pending}


def _eid(v):
    """Valeur d'un ElementId lu par champs.py (m_id64 en 2024+, m_id avant)."""
    return next(x for k, x in v['m_id'].items() if k != '_classe')


def _cat(n):
    return _eid(n['m_GInfo']['m_categoryId'])


def noeuds(n, chemin='', cat=-1):
    """(chemin, catégorie, nœud) en profondeur ; catégorie = GInfo.m_categoryId du
    GGroup le plus proche qui en porte une (id d'un GStyleElem, −1 sinon)."""
    cat = _cat(n) if _cat(n) != -1 else cat
    yield chemin, cat, n
    for i, p in enumerate(n.get('m_subNodes') or []):
        if p:
            yield from noeuds(p['corps'], '%s/%d' % (chemin, i), cat)


def courbe(c, t):
    """Point (pieds) d'une GLine, d'un GArc ou d'une GHermiteSpline à son paramètre propre t.
    GHermiteSpline : nœuds (m_iParametr = abscisse curviligne, m_iPoint, m_iTangent unitaire),
    Hermite cubique de pas h entre deux nœuds, tangentes × h."""
    k = c['_classe']
    if k == 'GLine':
        return [o + t * d for o, d in zip(c['m_origin'], c['m_dirVec'])]
    if k == 'GArc':
        return [ci + c['m_radius'] * (math.cos(t) * x + math.sin(t) * y) for ci, x, y in zip(c['m_center'], c['m_xVec'], c['m_yVec'])]
    if k == 'GHermiteSpline':
        ns, i = c['m_NodeArray'], 0
        while i + 2 < len(ns) and t > ns[i + 1]['m_iParametr']:
            i += 1
        a, b = ns[i], ns[i + 1]
        h = b['m_iParametr'] - a['m_iParametr']
        x = (t - a['m_iParametr']) / h
        h00, h10, h01, h11 = 2 * x ** 3 - 3 * x ** 2 + 1, x ** 3 - 2 * x ** 2 + x, -2 * x ** 3 + 3 * x ** 2, x ** 3 - x ** 2
        return [h00 * p0 + h10 * h * m0 + h01 * p1 + h11 * h * m1
                for p0, m0, p1, m1 in zip(a['m_iPoint'], a['m_iTangent'], b['m_iPoint'], b['m_iTangent'])]
    raise Inconnu('courbe %s non décodée' % k)


def point(s, u, v):
    """Point (pieds) de la surface s au paramètre (u, v). RuledSurf : u ∈ [0, 1] le long des
    deux profils (entre leurs m_endParams), v ∈ [0, 1] de l'un à l'autre. SurfRev : profil
    au paramètre v dans le repère local, tourné de u autour de m_zVec."""
    k = s['_classe']
    if k == 'Plane':
        return [o + u * x + v * y for o, x, y in zip(s['m_origin'], s['m_xVec'], s['m_yVec'])]
    if k == 'CylSurf':
        r, cu, su = s['m_radius'], math.cos(u), math.sin(u)
        return [c + r * (cu * x + su * y) + v * z
                for c, x, y, z in zip(s['m_center'], s['m_xVec'], s['m_yVec'], s['m_zVec'])]
    if k == 'RuledSurf':
        def profil(nom, pt):
            c = s[nom]
            if not c:
                return s[pt]
            t0, t1 = c['corps']['m_endParams']
            return courbe(c['corps'], t0 + u * (t1 - t0))
        a, b = profil('m_pProfileCurve1', 'm_Point1'), profil('m_pProfileCurve2', 'm_Point2')
        return [x + v * (y - x) for x, y in zip(a, b)]
    if k == 'SurfRev':
        a, b, h = courbe(s['m_pProfileCurve']['corps'], v)
        A, B = a * math.cos(u) - b * math.sin(u), a * math.sin(u) + b * math.cos(u)
        return [o + A * x + B * y + h * z for o, x, y, z in zip(s['m_center'], s['m_xVec'], s['m_yVec'], s['m_zVec'])]
    raise Inconnu('surface %s non décodée (champs : %s)' % (k, ', '.join(f for f in s if f != '_classe')))


def _ref(v):
    return v[1] if isinstance(v, tuple) else None


def boucles(fnum, face, objs):
    """Boucles de la face : listes de (numéro d'arête, [(u, v)…]) dans l'ordre de
    parcours de la face, chaque arête dans le sens de ce parcours."""
    out, lp = [], face['m_pFirstLoop']
    while lp:
        loop, lnum = lp['corps'], lp['num']
        pts, prev, cur = [], lnum, _ref(loop['m_next'])
        while cur != lnum:
            e = objs[cur]
            cotes = [s for s in (0, 1) if _ref(e['m_pFace'][s]) == fnum and _ref(e['m_prev'][s]) == prev]
            if len(cotes) != 1:
                raise Inconnu('arête %d : côté introuvable sur la face %d' % (cur, fnum))
            s = cotes[0]
            seq = [e['m_firstAndLastEdgePnts'][0]] + e['m_interiorEdgePnts'] + [e['m_firstAndLastEdgePnts'][1]]
            uv = [tuple(p['uv'][s]) for p in seq]
            if s != e['m_flags'] & 1:
                uv.reverse()
            pts.append((cur, uv))
            prev, cur = cur, _ref(e['m_next'][s])
            if len(pts) > len(objs):
                raise Inconnu('boucle %d non refermée' % lnum)
        out.append(pts)
        lp = loop['m_nextLoop']
    return out


def _dist(a, b):
    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))


def _cross(a, b, c):
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def _aire(poly):
    return sum(p[0] * q[1] - q[0] * p[1] for p, q in zip(poly, poly[1:] + poly[:1])) / 2


def _dans(p, poly):
    """p strictement dans le polygone (règle pair-impair)."""
    c = False
    for a, b in zip(poly, poly[1:] + poly[:1]):
        if (a[1] > p[1]) != (b[1] > p[1]) and p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]):
            c = not c
    return c


def _coupe(p, q, a, b):
    """Les segments pq et ab se coupent-ils en dehors de leurs extrémités communes ? Un sommet
    de ab posé à l'intérieur de pq compte aussi (pont passant par le coin d'un trou aligné)."""
    l2 = (q[0] - p[0]) ** 2 + (q[1] - p[1]) ** 2
    for x in (a, b):
        if x not in (p, q) and l2 > 0:
            t = ((x[0] - p[0]) * (q[0] - p[0]) + (x[1] - p[1]) * (q[1] - p[1])) / l2
            if 0 < t < 1 and abs(_cross(p, q, x)) <= 1e-9 * l2:
                return True
    if p in (a, b) or q in (a, b):
        return False
    d1, d2, d3, d4 = _cross(p, q, a), _cross(p, q, b), _cross(a, b, p), _cross(a, b, q)
    return d1 * d2 < 0 and d3 * d4 < 0


def trianguler(uv, xyz, polys, P=None):
    """Triangles (indices) d'une face : polys = boucles en indices de uv / xyz,
    extérieures dans un sens, trous dans l'autre. Oreilles coupées dans le plan (u, v) ;
    d'abord la diagonale de plus petite flèche sur la surface P(u, v) (arrondie à 1e-6 pi :
    nulle sur un plan et le long des génératrices), puis la plus courte en 3D ; chaque
    triangle garde le sens de parcours des boucles (donc de la normale sortante). Décisions
    sur des (u, v) arrondis à 1e-9 pi (sommets alignés à 1e-14 près)"""
    uv = [(round(u, 9), round(v, 9)) for u, v in uv]
    aires = [_aire([uv[i] for i in p]) for p in polys]
    sg = 1 if aires[max(range(len(polys)), key=lambda k: abs(aires[k]))] > 0 else -1
    ext = [p for p, a in zip(polys, aires) if a * sg > 0]
    trous = [p for p, a in zip(polys, aires) if a * sg < 0]
    tris = []
    for o in ext:
        poly = list(o)
        mes = [t for t in trous if _dans(uv[t[0]], [uv[i] for i in o])]
        for t in sorted(mes, key=lambda t: -max(uv[i][0] for i in t)):
            # pont entre le sommet le plus à droite du trou et le sommet visible le plus proche
            k = max(range(len(t)), key=lambda j: uv[t[j]][0])
            m = uv[t[k]]
            bords = [(uv[a], uv[b]) for c in [poly] + mes for a, b in zip(c, c[1:] + c[:1])]
            vis = [j for j in range(len(poly)) if not any(_coupe(m, uv[poly[j]], a, b) for a, b in bords)]
            if not vis:
                raise Inconnu('trou sans pont visible')
            j = min(vis, key=lambda j: _dist(m, uv[poly[j]]))
            poly = poly[:j + 1] + t[k:] + t[:k + 1] + poly[j:]
        while len(poly) > 3:
            best = None
            n = len(poly)
            for i in range(n):
                a, b, c = poly[i - 1], poly[i], poly[(i + 1) % n]
                if _cross(uv[a], uv[b], uv[c]) * sg <= 1e-12:
                    continue
                tri = (uv[a], uv[b], uv[c])
                if any(uv[p] not in tri and all(_cross(tri[q], tri[(q + 1) % 3], uv[p]) * sg >= -1e-12 for q in range(3))
                       for p in poly):
                    continue
                d = _dist(xyz[a], xyz[c])
                if P is not None:
                    pm = P((uv[a][0] + uv[c][0]) / 2, (uv[a][1] + uv[c][1]) / 2)
                    d = (round(_dist(pm, [(x + y) / 2 for x, y in zip(xyz[a], xyz[c])]) * 1e6), d)
                if best is None or d < best[0]:
                    best = (d, i)
            if best is None:
                # sommets alignés restants : on retire le moins saillant sans triangle
                i = min(range(n), key=lambda i: abs(_cross(uv[poly[i - 1]], uv[poly[i]], uv[poly[(i + 1) % n]])))
                poly.pop(i)
                continue
            i = best[1]
            tris.append((poly[i - 1], poly[i], poly[(i + 1) % n]))
            poly.pop(i)
        if len(poly) == 3 and abs(_cross(*[uv[i] for i in poly])) > 1e-12:
            tris.append(tuple(poly))
    return tris


TOLERANCE = 2 / FT          # flèche maximale des faces courbes densifiées (pieds)
TOLERANCE_SOMMET = 5e-4     # écart toléré entre arêtes consécutives d'une boucle (pieds)
GRILLE_MAX = 64


def _incercle(a, b, c, d):
    a, b, c = [(q[0] - d[0], q[1] - d[1]) for q in (a, b, c)]
    la, lb, lc = [q[0] ** 2 + q[1] ** 2 for q in (a, b, c)]
    return a[0] * (b[1] * lc - lb * c[1]) - a[1] * (b[0] * lc - lb * c[0]) + la * (b[0] * c[1] - b[1] * c[0])


def densifier(uv, xyz, polys, tris, P):
    """Points intérieurs d'une face courbe (comme src/rvt/geometrie.rs) : grille (u, v) dont
    le pas garde la flèche sous TOLERANCE, chaque point inséré dans son triangle puis
    bascules de Delaunay dans un (u, v) mis à l'échelle 3D ; arêtes du contour fixes."""
    if not tris:
        return uv, xyz, tris
    us, vs = [q[0] for q in uv], [q[1] for q in uv]
    u0, u1, v0, v1 = min(us), max(us), min(vs), max(vs)
    if u1 - u0 < 1e-12 or v1 - v0 < 1e-12:
        return uv, xyz, tris

    def segments(axe_u):
        a, b, fixes = (u0, u1, (v0, (v0 + v1) / 2, v1)) if axe_u else (v0, v1, (u0, (u0 + u1) / 2, u1))
        at = (lambda t, f: (t, f)) if axe_u else (lambda t, f: (f, t))
        n = 1
        while n < GRILLE_MAX:
            if all(_dist(P(*at(a + (b - a) * (k + .5) / n, f)),
                         [(x + y) / 2 for x, y in zip(P(*at(a + (b - a) * k / n, f)), P(*at(a + (b - a) * (k + 1) / n, f)))]) <= TOLERANCE
                   for f in fixes for k in range(n)):
                return n
            n *= 2
        return GRILLE_MAX

    nu, nv = segments(True), segments(False)
    if nu < 2 or nv < 2:
        return uv, xyz, tris
    um, vm, h = (u0 + u1) / 2, (v0 + v1) / 2, 1e-4
    ku = max(_dist(P(um + h * (u1 - u0), vm), P(um - h * (u1 - u0), vm)) / (2 * h * (u1 - u0)), 1e-9)
    kv = max(_dist(P(um, vm + h * (v1 - v0)), P(um, vm - h * (v1 - v0))) / (2 * h * (v1 - v0)), 1e-9)
    uv, xyz, tris = list(uv), list(xyz), [list(t) for t in tris]

    def S(i):
        return (uv[i][0] * ku, uv[i][1] * kv)

    sens = 1 if sum(_cross(S(t[0]), S(t[1]), S(t[2])) for t in tris) > 0 else -1
    bord = {frozenset((q[i], q[(i + 1) % len(q)])) for q in polys for i in range(len(q))}
    aretes = {}

    def poser(ti, t):
        if ti < len(tris):
            for j in range(3):
                e = (tris[ti][j], tris[ti][(j + 1) % 3])
                if aretes.get(e) == ti:
                    del aretes[e]
            tris[ti] = t
        else:
            tris.append(t)
        for j in range(3):
            aretes[(t[j], t[(j + 1) % 3])] = ti

    for ti, t in enumerate(list(tris)):
        poser(ti, t)
    contours = [[uv[i] for i in q] for q in polys]
    du, dv = (u1 - u0) / nu, (v1 - v0) / nv
    for i in range(1, nu):
        for j in range(1, nv):
            q = (u0 + du * i, v0 + dv * j)
            if sum(_dans(q, c) for c in contours) % 2 == 0:
                continue
            if any(abs(c[0] - q[0]) < .3 * du and abs(c[1] - q[1]) < .3 * dv for cc in contours for c in cc):
                continue
            sq = (q[0] * ku, q[1] * kv)
            ti = next((ti for ti, t in enumerate(tris) if all(sens * _cross(S(t[m]), S(t[(m + 1) % 3]), sq) > 0 for m in range(3))), None)
            if ti is None:
                continue
            pi = len(uv)
            uv.append(q)
            xyz.append(P(*q))
            a, b, c = tris[ti]
            n = len(tris)
            poser(ti, [a, b, pi])
            poser(n, [b, c, pi])
            poser(n + 1, [c, a, pi])
            pile = [(a, b), (b, c), (c, a)]
            while pile:
                x, y = pile.pop()
                if frozenset((x, y)) in bord or (x, y) not in aretes or (y, x) not in aretes:
                    continue
                t1, t2 = aretes[(x, y)], aretes[(y, x)]
                o = next(z for z in tris[t2] if z not in (x, y))
                if sens * _incercle(S(x), S(y), S(pi), S(o)) > 0:
                    poser(t1, [x, o, pi])
                    poser(t2, [o, y, pi])
                    pile += [(x, o), (o, y)]
    return uv, xyz, tris


def polymaillage(n):
    """Face d'un maillage à facettes (GPolyMesh : surface topographique).
    m_pFacetedTopology est une FacetedTopologyN dont les bits de N donnent le format
    (schéma) : 1 indices i32 (i16 sinon), 2 points f32 décalés de m_offset, 4 points
    f64 (f32 sinon) ; 8 et suivants ajoutent visibilité des arêtes et (u, v), non lus.
    Facettes triangulaires (m_facetsArr, 3 indices)."""
    ft = n['m_pFacetedTopology']['corps']
    if 'm_pointsArr' not in ft:
        raise Inconnu('%s : points absents (format non décodé)' % ft['_classe'])
    off = ft.get('m_offset', [0.0, 0.0, 0.0])
    mm = [tuple((c + o) * FT for c, o in zip(p, off)) for p in ft['m_pointsArr']]
    tris = [tuple(mm[i] for i in t) for t in ft['m_facetsArr']]
    return {'surface': 'GPolyMesh', 'materiau': _eid(n['m_materialID']), 'boucles': [], 'triangles': tris}


def solides(path, eid, tout=False):
    """B-rep de l'élément : liste de {'chemin', 'categorie', 'ferme', 'faces'} ; une
    face = {'surface', 'materiau', 'boucles': [[(x, y, z) mm…]], 'triangles':
    [((x, y, z) mm ×3)]}. 'ferme' : chaque arête des boucles a deux faces (solide) ;
    sinon feuille (plans de référence…), rendue seulement avec tout=True. Les
    arêtes hors boucle de m_pEdges (murs joints) sont ignorées."""
    root, objs = gelement(path, eid)
    out = []
    for chemin, cat, n in noeuds(root):
        if n['_classe'] == 'GPolyMesh':
            # surface topographique : une feuille, rendue dans tous les cas
            out.append({'chemin': chemin, 'categorie': cat, 'ferme': False, 'faces': [polymaillage(n)]})
            continue
        if 'm_pFaces' not in n:
            continue
        faces, aretes = [], set()
        for fp in n['m_pFaces']:
            f, s = fp['corps'], fp['corps']['m_pSurf']['corps']
            if not f['m_pFirstLoop']:
                continue    # face sans boucle (E09 : faces de l'ouverture confondues avec celles du mur)
            uv, polys = [], []
            for b in boucles(fp['num'], f, objs):
                polys.append([])
                for (k, ar), (_, suiv) in zip(b, b[1:] + b[:1]):
                    # contrôle du sens de parcours : fin de chaque arête = début de la suivante
                    if _dist(point(s, *ar[-1]), point(s, *suiv[0])) > TOLERANCE_SOMMET:
                        raise Inconnu('boucle de la face %d discontinue' % fp['num'])
                    aretes.add(k)
                    for p in ar[:-1]:
                        polys[-1].append(len(uv)); uv.append(p)
            xyz = [point(s, u, v) for u, v in uv]
            P = lambda u, v, s=s: point(s, u, v)
            tris = trianguler(uv, xyz, polys, P)
            if s['_classe'] != 'Plane':
                uv, xyz, tris = densifier(uv, xyz, polys, tris, P)
            mm = [tuple(c * FT for c in q) for q in xyz]
            faces.append({'surface': s['_classe'], 'materiau': _eid(f['m_renderStyleId']),
                          'boucles': [[mm[i] for i in p] for p in polys],
                          'triangles': [tuple(mm[i] for i in t) for t in tris]})
        ferme = all(_ref(objs[k]['m_pFace'][1]) for k in aretes)
        # solide fermé de volume négatif : vide de famille (réservation), ignoré
        vol = sum(a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
                  for f in faces for a, b, c in f['triangles'])
        if (ferme and vol >= 0) or tout:
            out.append({'chemin': chemin, 'categorie': cat, 'ferme': ferme, 'faces': faces})
    return out


def ecart_aretes(path, eid):
    """Plus grand écart (mm) entre les points d'une arête calculés sur ses deux
    faces : contrôle du décodage des surfaces et des (u, v)."""
    root, objs = gelement(path, eid)
    surf = {}
    for _, _, n in noeuds(root):
        for fp in n.get('m_pFaces') or []:
            surf[fp['num']] = fp['corps']['m_pSurf']['corps']
    ecart = 0.0
    for _, _, n in noeuds(root):
        for ep in n.get('m_pEdges') or []:
            e = ep['corps']
            f0, f1 = _ref(e['m_pFace'][0]), _ref(e['m_pFace'][1])
            if not f1:
                continue
            for p in e['m_firstAndLastEdgePnts'] + e['m_interiorEdgePnts']:
                ecart = max(ecart, _dist(point(surf[f0], *p['uv'][0]), point(surf[f1], *p['uv'][1])) * FT)
    return ecart


def maillage(path, eid, tout=False):
    """(sommets [(x, y, z) mm], faces) ; une face = {'surface', 'materiau',
    'boucles': [[indices]], 'triangles': [(i, j, k)]}, triangles dans le sens
    direct autour de la normale sortante."""
    sommets, index, faces = [], {}, []

    def ix(p):
        k = tuple(round(c, 6) for c in p)
        if k not in index:
            index[k] = len(sommets); sommets.append(p)
        return index[k]

    for sol in solides(path, eid, tout):
        for f in sol['faces']:
            faces.append({'surface': f['surface'], 'materiau': f['materiau'], 'categorie': sol['categorie'],
                          'boucles': [[ix(p) for p in b] for b in f['boucles']],
                          'triangles': [tuple(ix(p) for p in t) for t in f['triangles']]})
    return sommets, faces


def controle(sommets, faces):
    """Boîte englobante (mm), volume (m³, théorème de la divergence), aire (m²) et
    nombre d'arêtes de triangles non appariées (0 : maillage fermé et orienté)."""
    tris = [t for f in faces for t in f['triangles']]
    vol = aire = 0.0
    for i, j, k in tris:
        a, b, c = sommets[i], sommets[j], sommets[k]
        vol += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6
        u, v = [b[q] - a[q] for q in range(3)], [c[q] - a[q] for q in range(3)]
        aire += math.sqrt((u[1] * v[2] - u[2] * v[1]) ** 2 + (u[2] * v[0] - u[0] * v[2]) ** 2 + (u[0] * v[1] - u[1] * v[0]) ** 2) / 2
    dirigees = {}
    for i, j, k in tris:
        for a, b in ((i, j), (j, k), (k, i)):
            dirigees[(a, b)] = dirigees.get((a, b), 0) + 1
    seules = sum(1 for (a, b), n in dirigees.items() if n != 1 or dirigees.get((b, a)) != 1)
    lo = [min(p[q] for p in sommets) for q in range(3)]
    hi = [max(p[q] for p in sommets) for q in range(3)]
    return {'min': lo, 'max': hi, 'dimensions': [h - l for h, l in zip(hi, lo)],
            'volume_m3': vol / 1e9, 'aire_m2': aire / 1e6, 'aretes_non_appariees': seules, 'triangles': len(tris)}


def obj(path, sommets, faces):
    with open(path, 'w') as f:
        for p in sommets:
            f.write('v %.6f %.6f %.6f\n' % p)
        for fa in faces:
            for t in fa['triangles']:
                f.write('f %d %d %d\n' % tuple(i + 1 for i in t))


def main():
    args = sys.argv[1:]
    path, eid, tout = args[0], int(args[1]), '--tout' in args
    for sol in solides(path, eid, tout):
        print('%-8s catégorie %6d  %s  %d faces' % (sol['chemin'] or '/', sol['categorie'],
              'solide ' if sol['ferme'] else 'feuille', len(sol['faces'])))
        for f in sol['faces']:
            print('    %-8s matériau %4d  boucles %s  triangles %d' % (
                f['surface'], f['materiau'], '+'.join(str(len(b)) for b in f['boucles']), len(f['triangles'])))
    sommets, faces = maillage(path, eid, tout)
    c = controle(sommets, faces)
    print('sommets %d, faces %d, triangles %d' % (len(sommets), len(faces), c['triangles']))
    print('boîte (%.1f ; %.1f ; %.1f) -> (%.1f ; %.1f ; %.1f) mm' % tuple(c['min'] + c['max']))
    print('dimensions %.1f x %.1f x %.1f mm' % tuple(c['dimensions']))
    print('volume %.6f m3, aire %.4f m2, arêtes non appariées %d, écart des arêtes %.2e mm' % (
        c['volume_m3'], c['aire_m2'], c['aretes_non_appariees'], ecart_aretes(path, eid)))
    if '--obj' in args:
        obj(args[args.index('--obj') + 1], sommets, faces)


if __name__ == '__main__':
    main()
