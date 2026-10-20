"""Inventaire des éléments d'un fichier Revit, lu directement dans les partitions.

Usage : python -I objets.py <fichier.rvt|.rfa|.rte> [--liste] [--classe NOM] [--tout]

Par défaut, seuls les éléments listés dans Global/ElemTable (les éléments du
document) sont comptés ; --tout ajoute les autres objets des partitions
(probablement le contenu des familles chargées, non vérifié).

Chaque partition est une suite de membres gzip ; le champ h6 de leur en-tête de
32 octets donne la section :
  0x65  un ElementHeader par élément : [id][u32 n][n octets][u32 n]
  0x66  l'objet de l'élément :         [id][u32 ?][u32 n][n octets][u32 n]
  0x67  un complément par élément :    même cadrage que 0x66
L'id est un i64 (formats 2024 et suivants) ou un i32 (2016 à 2022) : voir id_size.
Un enregistrement peut déborder sur le membre suivant de la même section. Les n
octets commencent par l'id de classe (u16) ; les ids de classe sont le rang de
définition dans Formats/Latest, plus 12 (voir docs/LISEZMOI-RVT.md).
"""
import os, struct, sys, zlib
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cfb import CFB
from extract import GZIP, unpad
from schema import Schema


class ClassTable(Schema):
    """Schéma lu en notant l'id de chaque classe (rang de définition + 12)."""

    def __init__(s, b):
        super().__init__(b); s.order = []

    def ref(s):
        r = s.u16()
        if r & 0x8000:
            s.u16(); cid = r & 0x7FFF
            c = {'name': s.name16(), 'id': cid}
            s.order.append(c); s.classes[cid] = c; s.body(c)
            return cid
        return r

    def run(s):
        s.u16()
        while s.o < len(s.b) - 4 and struct.unpack_from('<H', s.b, s.o)[0]:
            c = {'name': s.name16(), 'id': 12 + len(s.order)}
            s.order.append(c); s.body(c); c['extra'] = s.u16(); s.top.append(c)
        s.byid = {c['id']: c for c in s.order}


def inflate(b):
    o = zlib.decompressobj(31)
    return o.decompress(b)


def sections(raw):
    """{h6: données décompressées concaténées} pour un flux de partition."""
    b = unpad(raw)
    heads = [m.start() for m in GZIP.finditer(b)]
    out = {}
    for i, h in enumerate(heads):
        end = heads[i + 1] - 32 if i + 1 < len(heads) else len(b)
        h6 = struct.unpack_from('<8I', b, h - 32)[6]
        out[h6] = out.get(h6, b'') + inflate(b[h:end])
    return out


def id_size(sch):
    """Taille des ElementId : 8 octets si la classe Identifier du schéma a un champ
    int64_t (m_id64, formats 2024 et suivants), 4 sinon (m_id en int, 2016 à 2022)."""
    ident = next((c for c in sch.order if c['name'] == 'Identifier'), None)
    return 8 if ident and ident['fields'] and ident['fields'][0]['type'] == 0x0b else 4


def records(data, stamp, ids=8):
    """(id, octets) ; stamp : présence du u32 non identifié avant la longueur ;
    ids : taille de l'ElementId en tête d'enregistrement (voir id_size)."""
    o, hdr = 0, ids + (8 if stamp else 4)
    while o + hdr <= len(data):
        eid = struct.unpack_from('<q' if ids == 8 else '<i', data, o)[0]
        n = struct.unpack_from('<I', data, o + hdr - 4)[0]
        if o + hdr + n + 4 > len(data) or struct.unpack_from('<I', data, o + hdr + n)[0] != n:
            raise ValueError('cadrage perdu à %d (id %d, longueur %d)' % (o, eid, n))
        yield eid, data[o + hdr:o + hdr + n]
        o += hdr + n + 4


def elem_table(raw):
    """Ids de Global/ElemTable : 8 octets de préfixe, puis gzip ; u16, u32 nombre,
    enregistrements commençant par l'id : 40 octets et id i64 (2024 et suivants),
    28 octets et id i32 (2016 à 2022). Quelques octets de fin, non étudiés."""
    t = inflate(unpad(raw)[8:])
    n = struct.unpack_from('<I', t, 2)[0]
    size = (len(t) - 6) // n if n else 40
    fmt = '<q' if size == 40 else '<i'
    return {struct.unpack_from(fmt, t, 6 + size * k)[0] for k in range(n)}


def main():
    args = sys.argv[1:]
    c = CFB(args[0]); by = {p: d for p, d in c.tree()}
    sch = ClassTable(inflate(unpad(c.read(by['/Formats/Latest'])))); sch.run()
    table = elem_table(c.read(by['/Global/ElemTable']))
    name = lambda b: sch.byid.get(struct.unpack_from('<H', b)[0], {}).get('name', '?') if len(b) >= 2 else '-'
    elems = {}
    for p in sorted((p for p in by if p.startswith('/Partitions/')), key=lambda p: int(p.split('/')[-1])):
        raw = c.read(by[p]); sec = sections(raw); ids = id_size(sch)
        objs = list(records(sec.get(0x66, b''), True, ids))
        for eid, body in objs:
            elems[eid] = (p, name(body), len(body))  # une partition plus récente remplace la précédente
        print('%-16s ElementHeader %5d  objets %5d  compléments %5d' % (
            p, sum(1 for _ in records(sec.get(0x65, b''), False, ids)), len(objs), sum(1 for _ in records(sec.get(0x67, b''), True, ids))))
    hors = len(set(elems) - table)
    print('\nElemTable %d ids, tous présents : %s ; objets hors ElemTable : %d' % (len(table), table <= set(elems), hors))
    if '--tout' not in args:
        elems = {k: v for k, v in elems.items() if k in table}
    cnt = Counter(v[1] for v in elems.values())
    print('%d éléments, %d classes' % (len(elems), len(cnt)))
    if '--classe' in args:
        k = args[args.index('--classe') + 1]
        for eid, v in sorted(elems.items()):
            if v[1] == k:
                print('  %8d  %6d o  %s' % (eid, v[2], v[0]))
    elif '--liste' in args:
        for eid, v in sorted(elems.items()):
            print('  %8d  %-40s %7d o  %s' % (eid, v[1], v[2], v[0]))
    else:
        for k, n in cnt.most_common(40):
            print('  %6d  %s' % (n, k))


if __name__ == '__main__':
    main()
