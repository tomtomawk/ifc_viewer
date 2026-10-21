"""Lecture des champs d'un objet Revit sérialisé, pilotée par le schéma Formats/Latest.

Usage : python -I champs.py <fichier> <ElementId> [--section 65|66|67]
        python -I champs.py <fichier> --couverture [--section 65|66|67]
        python -I champs.py <fichier> --murs

Encodage (docs/LISEZMOI-RVT.md § 7) : champs dans l'ordre du schéma, classe de base
d'abord ; entiers et doubles little-endian sans alignement ; chaîne = u32 nombre de
caractères + UTF-16LE ; conteneur = u32 nombre + éléments ; classe incorporée = ses
champs ; pointeur possédé = u32 (0 nul) + u16 id de classe, corps écrit plus loin
dans l'enregistrement ; référence faible = u32 numéro d'objet. Après la racine, les
corps des pointeurs suivent dans l'ordre où les pointeurs ont été rencontrés
(largeur d'abord). Les champs marqués non enregistrés (bit 0x02 de « ex ») sont
absents des données.
"""
import math, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from objets import CFB, ClassTable, id_size, inflate, records, sections, unpad

# Bit du u16 « ex » d'un champ (schéma 2027) : champ non enregistré
# (m_pTempExclusionMgr, m_oaNonSavedDrawFilters, GeomTable.m_cacheIsCurrent…).
NON_ENREGISTRE = 0x02

# Codes de type établis par les classes std::pair< T1, T2 > du schéma :
# bool, char, short, int / unsigned int, unsigned long, float, double, int64_t.
SIMPLE = {0x01: ('<B', 1), 0x02: ('<b', 1), 0x03: ('<h', 2), 0x04: ('<i', 4), 0x05: ('<I', 4),
          0x06: ('<f', 4), 0x07: ('<d', 8), 0x0b: ('<q', 8)}


class Fin(Exception):
    """Lecture incohérente : sortie des données ou valeur impossible."""


class Lecteur:
    def __init__(s, sch, b, trace=False, positions=None):
        s.sch, s.b, s.o, s.trace, s.depth = sch, b, 0, trace, 0
        s.pending = []          # pointeurs rencontrés, dans l'ordre : {num, classe, chemin, corps}
        s.positions = positions  # {chemin: (position, format)} des valeurs simples, si demandé

    def take(s, fmt, n):
        if s.o + n > len(s.b):
            raise Fin('fin des données à %d' % s.o)
        v = struct.unpack_from(fmt, s.b, s.o)[0]; s.o += n
        return v

    def chain(s, cid):
        out = []
        while cid:
            c = s.sch.byid.get(cid)
            if c is None:
                raise Fin('classe %d inconnue' % cid)
            out.append(c); cid = c['parent']
        return out[::-1]

    def obj(s, cid, path=''):
        res = {'_classe': s.sch.byid[cid]['name']}
        for c in s.chain(cid):
            for f in c['fields']:
                if f['ex'] & NON_ENREGISTRE:
                    continue
                start = s.o
                res[f['name']] = s.value(f, path + '.' + f['name'])
                if s.trace:
                    print('%s%6d %s.%s = %s' % ('  ' * s.depth, start, c['name'], f['name'], repr(res[f['name']])[:90]))
        return res

    def ptr(s, f, path):
        tag = s.take('<I', 4)
        if f['sub'] & 0x0F == 0x3:
            return ('réf', tag)
        if tag == 0:
            return None
        cid = s.take('<H', 2)
        if cid not in s.sch.byid:
            raise Fin('pointeur vers une classe inconnue %d à %d' % (cid, s.o - 2))
        p = {'num': tag, 'classe': cid, 'chemin': path, 'corps': None}
        s.pending.append(p)
        return p

    def one(s, f, path):
        t, sub = f['type'], f['sub'] & 0x0F
        if t in SIMPLE:
            if s.positions is not None:
                s.positions[path] = (s.o, SIMPLE[t][0])
            v = s.take(*SIMPLE[t])
            if t == 0x01 and v > 1:
                raise Fin('booléen %d à %d' % (v, s.o - 1))
            return v
        if t == 0x08:
            n = s.take('<I', 4)
            if n > 100000 or s.o + 2 * n > len(s.b):
                raise Fin('chaîne de %d caractères à %d' % (n, s.o - 4))
            v = s.b[s.o:s.o + 2 * n].decode('utf-16-le', 'replace'); s.o += 2 * n
            return v
        if t == 0x09:
            if s.o + 16 > len(s.b):
                raise Fin('GUID hors données')
            v = s.b[s.o:s.o + 16].hex(); s.o += 16
            return v
        if t == 0x0a:
            return s.take('<H', 2)
        if t == 0x0d:
            return s.value(f['elem'], path)
        if t == 0x0e:
            if sub == 0:
                s.depth += 1
                try:
                    return s.obj(f['ref'], path)
                finally:
                    s.depth -= 1
            return s.ptr(f, path)
        raise Fin('type 0x%x inconnu à %d' % (t, s.o))

    def value(s, f, path):
        hi = f['sub'] & 0xF0
        if hi == 0x10:
            return [s.one(f, '%s[%d]' % (path, i)) for i in range(f['count'])]
        if hi == 0x50:
            n = s.take('<I', 4)
            if n > len(s.b) - s.o:
                raise Fin('conteneur de %d éléments à %d' % (n, s.o - 4))
            return [s.one(f, '%s[%d]' % (path, i)) for i in range(n)]
        return s.one(f, path)

    def element(s):
        """Lit un enregistrement entier : id de classe, champs de la racine, puis les
        corps des pointeurs dans l'ordre où ils ont été rencontrés (file unique,
        largeur d'abord). Chaque corps est rattaché à son pointeur ('corps')."""
        s.o = 0
        cid = s.take('<H', 2)
        if cid not in s.sch.byid:
            raise Fin('classe %d inconnue' % cid)
        root = s.obj(cid)
        i = 0
        while i < len(s.pending):
            p = s.pending[i]; i += 1
            if s.trace:
                print('--- %6d corps de %s (%s)' % (s.o, s.sch.byid[p['classe']]['name'], p['chemin']))
            p['corps'] = s.obj(p['classe'], p['chemin'])
        if s.o != len(s.b):
            raise Fin('lecture arrêtée à %d sur %d octets' % (s.o, len(s.b)))
        return root


def load(path):
    c = CFB(path); by = {p: d for p, d in c.tree()}
    sch = ClassTable(inflate(unpad(c.read(by['/Formats/Latest'])))); sch.run()
    sch.byname = {k['name']: k for k in sch.order}
    parts = sorted((p for p in by if p.startswith('/Partitions/')), key=lambda p: int(p.split('/')[-1]))
    return c, by, sch, parts


def record(path, eid, h6=0x66):
    c, by, sch, parts = load(path)
    found = None
    for p in parts:
        raw = c.read(by[p])
        for e, body in records(sections(raw).get(h6, b''), h6 != 0x65, id_size(sch)):
            if e == eid:
                found = body
    return sch, found


def couverture(path, h6=0x66, limite=None):
    """Part des enregistrements d'une section lus exactement jusqu'à leur fin, par classe."""
    from collections import Counter
    c, by, sch, parts = load(path)
    ok, ko, err = Counter(), Counter(), {}
    n = 0
    for p in parts:
        raw = c.read(by[p])
        for e, body in records(sections(raw).get(h6, b''), h6 != 0x65, id_size(sch)):
            if len(body) < 2:
                continue
            name = sch.byid.get(struct.unpack_from('<H', body)[0], {}).get('name', '?')
            try:
                Lecteur(sch, body).element(); ok[name] += 1
            except (Fin, KeyError, IndexError, struct.error, RecursionError) as x:
                ko[name] += 1; err.setdefault(name, '%s (id %d)' % (x, e))
            n += 1
            if limite and n >= limite:
                break
    return ok, ko, err


FT = 304.8                      # mm par pied (unité interne de Revit)
WALL_USER_HEIGHT_PARAM, WALL_BASE_OFFSET, WALL_TOP_OFFSET = -1001105, -1001108, -1001109


def herite(sch, cid, nom):
    """La classe cid est-elle nom ou une classe dérivée de nom ?"""
    while cid:
        c = sch.byid[cid]
        if c['name'] == nom:
            return True
        cid = c['parent']
    return False


def objets_courants(path, classe=None):
    """(schéma, {id: octets}) des objets des éléments de ElemTable, état le plus
    récent ; classe : ne garder que cette classe et ses classes dérivées."""
    from objets import elem_table
    c, by, sch, parts = load(path)
    table = elem_table(c.read(by['/Global/ElemTable']))
    cur = {}
    for p in parts:
        raw = c.read(by[p])
        for e, body in records(sections(raw).get(0x66, b''), True, id_size(sch)):
            if e in table and len(body) >= 2:
                if classe is None or herite(sch, struct.unpack_from('<H', body)[0], classe):
                    cur[e] = body
    return sch, cur


def _id(v):
    """ElementId décodé -> entier (Identifier.m_id64 depuis 2024, Identifier.m_id avant)."""
    i = v['m_id']
    return i['m_id64'] if 'm_id64' in i else i['m_id']


def altitudes(sch, cur):
    """{id de niveau: altitude en mm} : Z de l'origine du plan de chaque Level."""
    out = {}
    for e, b in cur.items():
        if herite(sch, struct.unpack_from('<H', b)[0], 'Level'):
            out[e] = Lecteur(sch, b).element()['m_pSurface']['corps']['m_origin'][2] * FT
    return out


def murs(path):
    """Murs (SWall droits, ArcWall courbes ; Railing, qui dérive aussi de VWall, est
    exclu) : type, niveaux, ligne de positionnement en mm, hauteur et décalages.
    Les éléments d'un groupe non placé (m_unplacedOwnerId) sont ignorés, comme dans
    les collecteurs de Revit. Hauteur : celle que donne Revit, calculée depuis les
    niveaux si le mur est contraint en haut ; hauteur_libre : valeur enregistrée."""
    sch, tout = objets_courants(path)
    alt = altitudes(sch, tout)
    cur = {e: b for e, b in tout.items() if sch.byid[struct.unpack_from('<H', b)[0]]['name'] in ('SWall', 'ArcWall')}
    out = []
    for e, body in sorted(cur.items()):
        root = Lecteur(sch, body).element()
        if _id(root['m_unplacedOwnerId']) != -1:
            continue
        params = {}
        pd = root['m_pParamValueSetDouble']
        if pd:
            params = {_id(x['m_paramId']): x['m_value'] for x in pd['corps']['m_paramSet']}
        crv = root['m_pCurveDriver']['corps']['m_pCrv']['corps']
        m = {'id': e, 'classe': root['_classe'], 'type': _id(root['m_WallAttributesId']), 'niveau': _id(root['m_assocLevelId']),
             'niveau_haut': _id(root['m_upToLevelId']), 'courbe': crv['_classe'],
             'hauteur_libre': params.get(WALL_USER_HEIGHT_PARAM, 0) * FT,
             'decalage_bas': params.get(WALL_BASE_OFFSET, 0) * FT, 'decalage_haut': params.get(WALL_TOP_OFFSET, 0) * FT}
        m['hauteur'] = m['hauteur_libre']
        if m['niveau_haut'] != -1 and m['niveau_haut'] in alt and m['niveau'] in alt:
            m['hauteur'] = alt[m['niveau_haut']] + m['decalage_haut'] - alt[m['niveau']] - m['decalage_bas']
        if crv['_classe'] == 'GLine':
            t0, t1 = crv['m_endParams']
            m['debut'] = [(o + t0 * d) * FT for o, d in zip(crv['m_origin'], crv['m_dirVec'])]
            m['fin'] = [(o + t1 * d) * FT for o, d in zip(crv['m_origin'], crv['m_dirVec'])]
        elif crv['_classe'] == 'GArc':
            # point(t) = centre + rayon × (cos t · xVec + sin t · yVec), t en radians
            c, r = crv['m_center'], crv['m_radius']
            pt = lambda t: [(ci + r * (math.cos(t) * x + math.sin(t) * y)) * FT for ci, x, y in zip(c, crv['m_xVec'], crv['m_yVec'])]
            m['debut'], m['fin'] = pt(crv['m_endParams'][0]), pt(crv['m_endParams'][1])
            m['centre'], m['rayon'] = [ci * FT for ci in c], r * FT
        out.append(m)
    return out


def main():
    args = sys.argv[1:]
    h6 = int(args[args.index('--section') + 1], 16) if '--section' in args else 0x66
    if '--murs' in args:
        for m in murs(args[0]):
            pts = ('(%.1f ; %.1f ; %.1f) -> (%.1f ; %.1f ; %.1f)' % tuple(m['debut'] + m['fin'])) if 'debut' in m else m['courbe']
            if 'rayon' in m:
                pts += '  arc : centre (%.1f ; %.1f), rayon %.1f' % (m['centre'][0], m['centre'][1], m['rayon'])
            print('%8d  type %6d  niveau %6d  haut %6d  h %8.1f  bas %+7.1f  haut %+7.1f  %s' % (
                m['id'], m['type'], m['niveau'], m['niveau_haut'], m['hauteur'], m['decalage_bas'], m['decalage_haut'], pts))
        return
    if '--couverture' in args:
        ok, ko, err = couverture(args[0], h6)
        t_ok, t_ko = sum(ok.values()), sum(ko.values())
        print('enregistrements lus en entier : %d / %d (%.1f %%) ; classes entièrement lues %d / %d' % (
            t_ok, t_ok + t_ko, 100 * t_ok / max(1, t_ok + t_ko), len(set(ok) - set(ko)), len(set(ok) | set(ko))))
        for k, v in ko.most_common(25):
            print('  %6d échecs  %-36s (%d lus)  %s' % (v, k, ok[k], err[k][:90]))
        return
    sch, b = record(args[0], int(args[1]), h6)
    if b is None:
        print('élément introuvable'); return
    r = Lecteur(sch, b, trace=True)
    try:
        r.element(); print('lu en entier : %d octets, %d corps différés' % (len(b), len(r.pending)))
    except Fin as x:
        print('ARRÊT :', x)


if __name__ == '__main__':
    main()
