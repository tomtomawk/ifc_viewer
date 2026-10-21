"""Géométrie des instances de familles : GInstance → GElement du type, vue 3D en détail fin.

Usage : python -I familles.py <fichier> <ElementId>...           volume, triangles, boîte (mm)
        python -I familles.py <fichier> --trace <ElementId>       solides retenus ou écartés, et pourquoi

Règles (docs/LISEZMOI-RVT.md § 7, « Familles ») :
- le GElement d'une instance contient un GInstance : repère m_instanceInfo.m_Trf
  (p' = m_or + R·p, la colonne j de m_3x3 étant l'axe j) et type m_instanceInfo.m_symbolId,
  dont le GElement (section 0x67 du Symbol) porte la géométrie ; ou une géométrie incorporée
  m_oEmbeddedSymbolGRep. Les GInstance s'imbriquent (familles imbriquées) ;
- un GFilter n'est parcouru que si toutes ses conditions sont vraies pour une vue 3D en
  détail fin : GConditionDir et GConditionCut vraies si niées (m_negateDirCondition = 1),
  GConditionInt de paramètre 3 = niveau de détail (m_comp 0 : différent, 3 : égal) ;
- un nœud dont GInfo.m_flags porte les bits 0xE0 est invisible (ouverture d'une fenêtre…) ;
- un solide fermé de volume négatif est un vide (réservation).
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import geometrie as G
from elements import eid

INVISIBLE = 0xE0
DETAIL_FIN = 3
ID = ([[1, 0, 0], [0, 1, 0], [0, 0, 1]], [0, 0, 0])


def condition(c):
    k = c['_classe']
    if k in ('GConditionDir', 'GConditionCut'):
        return c.get('m_negateDirCondition') == 1
    if k == 'GConditionInt' and c.get('m_param') == 3:
        return {0: DETAIL_FIN != c['m_value'], 3: DETAIL_FIN == c['m_value']}.get(c['m_comp'], True)
    return True


def appliquer(T, p):
    R, o = T
    return [o[i] + R[i][0] * p[0] + R[i][1] * p[1] + R[i][2] * p[2] for i in range(3)]


def composer(T1, T2):
    """T1 après T2."""
    R1, R2 = T1[0], T2[0]
    return [[sum(R1[i][k] * R2[k][j] for k in range(3)) for j in range(3)] for i in range(3)], appliquer(T1, T2[1])


def det(R):
    return (R[0][0] * (R[1][1] * R[2][2] - R[1][2] * R[2][1]) - R[0][1] * (R[1][0] * R[2][2] - R[1][2] * R[2][0])
            + R[0][2] * (R[1][0] * R[2][1] - R[1][1] * R[2][0]))


def triangles(n, objs):
    """Triangles (pieds, repère du nœud) d'une Geometry si c'est un solide fermé de volume ≥ 0."""
    tris, aretes = [], set()
    for fp in n['m_pFaces']:
        f, s = fp['corps'], fp['corps']['m_pSurf']['corps']
        if not f['m_pFirstLoop']:
            continue
        uv, polys = [], []
        for b in G.boucles(fp['num'], f, objs):
            polys.append([])
            for (k, ar), (_, suiv) in zip(b, b[1:] + b[:1]):
                if G._dist(G.point(s, *ar[-1]), G.point(s, *suiv[0])) > G.TOLERANCE_SOMMET:
                    raise G.Inconnu('boucle de la face %d discontinue' % fp['num'])
                aretes.add(k)
                for q in ar[:-1]:
                    polys[-1].append(len(uv))
                    uv.append(q)
        xyz = [G.point(s, u, v) for u, v in uv]
        P = lambda u, v, s=s: G.point(s, u, v)
        tt = G.trianguler(uv, xyz, polys, P)
        if s['_classe'] != 'Plane':
            uv, xyz, tt = G.densifier(uv, xyz, polys, tt, P)
        tris += [tuple(xyz[i] for i in t) for t in tt]
    if not all(G._ref(objs[k]['m_pFace'][1]) for k in aretes) or volume(tris) < 0:
        return []
    return tris


def volume(tris, echelle=1.0):
    return sum(a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
               for a, b, c in tris) / 6 * echelle ** 3


class Fichier:
    def __init__(s, path):
        s.path = path
        s.cache = {}

    def gelement(s, e):
        if e not in s.cache:
            try:
                s.cache[e] = G.gelement(s.path, e)
            except G.Inconnu:
                s.cache[e] = None
        return s.cache[e]

    def parcourir(s, n, objs, T, out, trace=None, chemin='', prof=0):
        k = n['_classe']
        if n['m_GInfo']['m_flags'] & INVISIBLE:
            return trace and trace(chemin, k, 'invisible (GInfo.m_flags %#x)' % n['m_GInfo']['m_flags'])
        if k == 'GFilter' and not all(condition(x['corps']) for x in n.get('m_oConditions') or []):
            return trace and trace(chemin, k, 'filtre : ' + ' & '.join(x['corps']['_classe'] for x in n['m_oConditions']))
        if k == 'Geometry':
            try:
                t = triangles(n, objs)
            except G.Inconnu as x:
                return trace and trace(chemin, k, 'non décodé : %s' % x)
            out += [tuple(appliquer(T, p) for p in tr)[::-1 if det(T[0]) < 0 else 1] for tr in t]
            if trace:
                trace(chemin, k, 'retenu, %.6f m³' % volume(t, G.FT / 1000) if t else 'feuille ou vide')
        if k == 'GInstance' and prof < 16:
            ii = n['m_instanceInfo']['corps']
            T2 = composer(T, ([list(r) for r in ii['m_Trf']['m_3x3']], ii['m_Trf']['m_or']))
            emb = n.get('m_oEmbeddedSymbolGRep')
            if emb and emb.get('corps'):
                s.parcourir(emb['corps'], objs, T2, out, trace, chemin + '>incorporé', prof + 1)
            else:
                g = s.gelement(eid(ii['m_symbolId']))
                if trace:
                    trace(chemin, k, 'type %d%s' % (eid(ii['m_symbolId']), '' if g else ' (géométrie absente)'))
                if g:
                    s.parcourir(g[0], g[1], T2, out, trace, chemin + '>%d' % eid(ii['m_symbolId']), prof + 1)
        for i, sp in enumerate(n.get('m_subNodes') or []):
            if sp:
                s.parcourir(sp['corps'], objs, T, out, trace, chemin + '/%d' % i, prof)

    def instance(s, e, trace=None):
        """Triangles (mm) de l'élément e, familles comprises."""
        g = s.gelement(e)
        out = []
        if g:
            s.parcourir(g[0], g[1], ID, out, trace)
        return [tuple(tuple(c * G.FT for c in p) for p in t) for t in out]


def main():
    args = sys.argv[1:]
    F = Fichier(args[0])
    if '--trace' in args:
        e = int(args[args.index('--trace') + 1])
        F.instance(e, lambda ch, k, m: print('%-40s %-10s %s' % (ch or '/', k, m)))
        return
    for e in map(int, args[1:]):
        t = F.instance(e)
        pts = [p for tr in t for p in tr]
        if not pts:
            print('%d : pas de géométrie' % e)
            continue
        lo = [min(p[i] for p in pts) for i in range(3)]
        hi = [max(p[i] for p in pts) for i in range(3)]
        print('%d : %.6f m³, %d triangles, boîte (%.1f ; %.1f ; %.1f) -> (%.1f ; %.1f ; %.1f) mm' % ((e, volume(t) / 1e9, len(t)) + tuple(lo + hi)))


if __name__ == '__main__':
    main()
