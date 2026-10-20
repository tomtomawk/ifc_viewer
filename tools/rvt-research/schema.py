"""Analyse du schéma de classes `Formats/Latest` (après décompression).

Usage : python -I schema.py <Formats_Latest.bin> [sortie.json]

Le fichier .bin est produit par extract.py. La liste des classes se termine par
un nom vide ; une erreur avant cette fin signale un enregistrement non compris.
Lu en entier sur les formats 2016, 2026 et 2027.
"""
import json, struct, sys


class Schema:
    def __init__(s, b):
        s.b = b; s.o = 0; s.classes = {}; s.top = []

    def u8(s):
        v = s.b[s.o]; s.o += 1; return v

    def u16(s):
        v = struct.unpack_from('<H', s.b, s.o)[0]; s.o += 2; return v

    def u32(s):
        v = struct.unpack_from('<I', s.b, s.o)[0]; s.o += 4; return v

    def name16(s):
        n = s.u16(); v = s.b[s.o:s.o + n]; s.o += n
        if not all(32 <= c < 127 for c in v):
            raise ValueError('nom (u16) invalide %r à 0x%x' % (v[:40], s.o - n))
        return v.decode()

    def name32(s):
        n = s.u32()
        if n > 200:
            raise ValueError('longueur de nom (u32) %d invalide à 0x%x' % (n, s.o - 4))
        v = s.b[s.o:s.o + n]; s.o += n
        if not all(32 <= c < 127 for c in v):
            raise ValueError('nom (u32) invalide %r à 0x%x' % (v[:40], s.o - n))
        return v.decode()

    def ref(s):
        """Référence de classe : u16 ; bit 0x8000 = définition en ligne (u16 0, nom, corps)."""
        r = s.u16()
        if r & 0x8000:
            s.u16(); cid = r & 0x7FFF
            s.classes[cid] = {'name': s.name16()}
            s.body(s.classes[cid])
            return cid
        return r

    def body(s, c):
        c['parent'] = s.ref()
        c['version'] = s.u32(); n = s.u32()
        if n > 500:
            raise ValueError('nombre de champs %d invalide à 0x%x' % (n, s.o - 4))
        c['fields'] = [s.field() for _ in range(n)]
        ng = s.u32()
        if ng > 5000:
            raise ValueError('nombre de GUID %d invalide à 0x%x' % (ng, s.o - 4))
        c['guids'] = [s.b[s.o + 16 * i:s.o + 16 * i + 16].hex() for i in range(ng)]
        s.o += 16 * ng

    def tdesc(s):
        t = s.u8(); sub = s.u8(); ex = s.u16()
        f = {'type': t, 'sub': sub, 'ex': ex}
        if sub & 0xF0 == 0x10:
            f['count'] = s.u32()
        if t == 0x0D:
            f['a'] = s.u32(); f['b'] = s.u8(); f['elem'] = s.tdesc()
            if f['elem']['type'] == 0x0E:
                f['ref2'] = s.ref()
        if t == 0x0E and sub & 0x0F == 0:
            f['ref'] = s.ref()
        return f

    def field(s):
        nm = s.name32(); f = s.tdesc(); f['name'] = nm
        return f

    def run(s):
        s.u16()
        while s.o < len(s.b) - 4:
            start = s.o
            if struct.unpack_from('<H', s.b, s.o)[0] == 0:  # nom vide : fin de la liste
                break
            c = {'name': s.name16(), 'offset': start}
            s.body(c); c['extra'] = s.u16()
            s.top.append(c)


if __name__ == '__main__':
    sch = Schema(open(sys.argv[1], 'rb').read())
    try:
        sch.run()
        print('schéma lu en entier')
    except Exception as e:
        print('arrêt : %s (0x%x / 0x%x, %.0f %%)' % (e, sch.o, len(sch.b), 100 * sch.o / len(sch.b)))
    nf = sum(len(c.get('fields', [])) for c in sch.top) + sum(len(c.get('fields', [])) for c in sch.classes.values())
    print('classes de premier niveau %d, définies en ligne %d, champs %d' % (len(sch.top), len(sch.classes), nf))
    if len(sys.argv) > 2:
        json.dump({'top': sch.top, 'inline': {str(k): v for k, v in sch.classes.items()}}, open(sys.argv[2], 'w'), indent=1)
