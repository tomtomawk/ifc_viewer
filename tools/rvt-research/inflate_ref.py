"""Décodeur deflate de référence en Python pur, instrumentable.

Sert à tester des variantes d'interprétation du flux (Deflate64, tables
modifiées…) et à relever les symboles (littéraux, copies) d'un membre gzip.
Lent : réservé aux membres de quelques centaines de Ko.

Usage : python -I inflate_ref.py <fichier Revit> <flux, ex. Formats/Latest>
"""
import os, struct, sys, zlib

LBASE = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258]
LEXT = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0]
DBASE = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097,
         6145, 8193, 12289, 16385, 24577, 32769, 49153]
DEXT = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14]
CL_ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]


class Bits:
    def __init__(s, b):
        s.b = b; s.p = 0; s.v = 0; s.n = 0

    def get(s, n):
        while s.n < n:
            s.v |= s.b[s.p] << s.n; s.p += 1; s.n += 8
        r = s.v & ((1 << n) - 1); s.v >>= n; s.n -= n
        return r

    def align(s):
        s.v = 0; s.n = 0


def huff(lengths):
    """Code canonique : dictionnaire (longueur, code) -> symbole."""
    maxl = max(lengths) if lengths else 0
    bl = [0] * (maxl + 1)
    for l in lengths:
        if l:
            bl[l] += 1
    code = 0; nxt = [0] * (maxl + 2)
    for i in range(1, maxl + 1):
        code = (code + bl[i - 1]) << 1; nxt[i] = code
    t = {}
    for sym, l in enumerate(lengths):
        if l:
            t[(l, nxt[l])] = sym; nxt[l] += 1
    return t, maxl


def dec(bits, h):
    t, maxl = h; code = 0
    for l in range(1, maxl + 1):
        code = (code << 1) | bits.get(1)
        s = t.get((l, code))
        if s is not None:
            return s
    raise ValueError('code de Huffman invalide')


FIXED_L = huff([8] * 144 + [9] * 112 + [7] * 24 + [8] * 8)
FIXED_D = huff([5] * 32)


def tokenize(data):
    """Symboles du flux : int (littéral), ('blk', type, octet), ('raw', octets) ou (code_long, extra, code_dist, extra)."""
    bits = Bits(data); toks = []
    while True:
        final = bits.get(1); typ = bits.get(2)
        toks.append(('blk', typ, bits.p))
        if typ == 0:
            bits.align(); ln = struct.unpack_from('<H', data, bits.p)[0]; bits.p += 4
            toks.append(('raw', data[bits.p:bits.p + ln])); bits.p += ln
        else:
            if typ == 1:
                hl, hd = FIXED_L, FIXED_D
            else:
                hlit = bits.get(5) + 257; hdist = bits.get(5) + 1; hclen = bits.get(4) + 4
                cl = [0] * 19
                for i in range(hclen):
                    cl[CL_ORDER[i]] = bits.get(3)
                hc = huff(cl); ls = []
                while len(ls) < hlit + hdist:
                    sym = dec(bits, hc)
                    if sym < 16: ls.append(sym)
                    elif sym == 16: ls += [ls[-1]] * (3 + bits.get(2))
                    elif sym == 17: ls += [0] * (3 + bits.get(3))
                    else: ls += [0] * (11 + bits.get(7))
                hl = huff(ls[:hlit]); hd = huff(ls[hlit:])
            while True:
                sym = dec(bits, hl)
                if sym < 256:
                    toks.append(sym)
                elif sym == 256:
                    break
                else:
                    i = sym - 257; le = bits.get(LEXT[i]); ds = dec(bits, hd); de = bits.get(DEXT[ds])
                    toks.append((i, le, ds, de))
        if final:
            break
    return toks, bits.p


def render(toks, lbase=LBASE, dbase=DBASE):
    """Reconstruit les octets à partir des symboles ; None si une copie remonte avant le début."""
    out = bytearray()
    for t in toks:
        if isinstance(t, int):
            out.append(t)
        elif t[0] == 'raw':
            out += t[1]
        elif t[0] == 'blk':
            continue
        else:
            i, le, ds, de = t; ln = lbase[i] + le; dist = dbase[ds] + de
            if dist > len(out):
                return None
            st = len(out) - dist
            for k in range(ln):
                out.append(out[st + k])
    return bytes(out)


if __name__ == '__main__':
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from cfb import CFB
    c = CFB(sys.argv[1]); by = {p: d for p, d in c.tree()}
    b = c.read(by['/' + sys.argv[2].strip('/')])
    h = b.find(b'\x1f\x8b\x08') + 10
    toks, p = tokenize(b[h:])
    crc, isz = struct.unpack_from('<II', b, h + p)
    r = render(toks)
    blocks = [t for t in toks if isinstance(t, tuple) and t[0] == 'blk']
    print('symboles %d, blocs %d, fin du flux à l\'octet %d' % (len(toks), len(blocks), p))
    print('décodé %s o, ISIZE %d, CRC %s' % (None if r is None else len(r), isz, 'OK' if r is not None and zlib.crc32(r) == crc else 'différent'))
