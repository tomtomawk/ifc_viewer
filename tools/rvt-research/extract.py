"""Extrait les flux d'un fichier Revit et décompresse leurs membres gzip.

Usage : python -I extract.py <fichier.rvt|.rfa|.rte> <dossier_sortie> [--quiet]

Pour chaque flux : copie brute (<flux>.raw) et, s'il contient des membres gzip,
leur concaténation décompressée (<flux>.bin). Avant décompression, les blocs de
353 octets que Revit intercale tous les 64 896 octets de données sont retirés
(voir unpad). Chaque membre est décodé en deflate brut puis comparé au CRC32 et
à ISIZE de son trailer gzip. Un membre « KO » ne correspond pas à son trailer.

--brut : ne retire pas les blocs intercalés (comportement d'avant le 20/10/2026).
"""
import os, re, struct, sys, zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cfb import CFB

GZIP = re.compile(rb'\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\x0b')
DATA, PAD = 64896, 353


def unpad(b):
    """Retire les blocs intercalés : le flux est une suite de tranches de 65 249
    octets, 64 896 de données puis 353 d'un bloc commençant par 00 x0."""
    return b''.join(b[i:i + DATA] for i in range(0, len(b), DATA + PAD))


def members(b):
    """(offset, données, crc_ok, isize, erreur) pour chaque membre gzip du flux."""
    heads = [m.start() for m in GZIP.finditer(b)]
    out = []
    for i, h in enumerate(heads):
        end = heads[i + 1] if i + 1 < len(heads) else len(b)
        d = zlib.decompressobj(-15)
        try:
            r = d.decompress(b[h + 10:end])
            u = d.unused_data
            crc, isz = struct.unpack_from('<II', u, 0) if len(u) >= 8 else (None, None)
            out.append((h, r, crc == zlib.crc32(r) and isz == len(r) & 0xFFFFFFFF, isz, None))
        except zlib.error as e:
            out.append((h, b'', False, None, str(e)))
    return out


def main():
    src, dst = sys.argv[1], sys.argv[2]
    quiet = '--quiet' in sys.argv
    brut = '--brut' in sys.argv
    os.makedirs(dst, exist_ok=True)
    c = CFB(src)
    print('CFB v%d, secteur %d, mini-secteur %d, seuil %d' % (c.major, c.ss, c.mss, c.mini_cutoff))
    for path, d in c.tree():
        if d['type'] != 2:
            continue
        b = c.read(d)
        name = path.strip('/').replace('/', '_')
        open(os.path.join(dst, name + '.raw'), 'wb').write(b)
        ms = members(b if brut else unpad(b))
        if not ms:
            print('%-34s %12d o  (pas de gzip)' % (path, len(b)))
            continue
        ok = sum(1 for m in ms if m[2])
        print('%-34s %12d o  gzip %5d  OK %5d  KO %5d  décompressé %d o' % (path, len(b), len(ms), ok, len(ms) - ok, sum(len(m[1]) for m in ms)))
        if not quiet:
            for h, r, good, isz, err in ms:
                if not good and len(ms) <= 20:
                    print('    membre @%d : %s' % (h, err or 'décodé %d o, ISIZE %s' % (len(r), isz)))
        open(os.path.join(dst, name + '.bin'), 'wb').write(b''.join(m[1] for m in ms))


if __name__ == '__main__':
    main()
