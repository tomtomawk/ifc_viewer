"""Code correcteur des flux compressés de Revit (blocs de 353 octets, fin des flux).

Usage : python -I crcbloc.py <fichier> [...]   (réencode chaque flux et compare)

Chaque flux compressé est découpé en morceaux d'au plus 64 896 octets. Un morceau de
D octets devient un mot de N bits, N = longueur × n, avec un code choisi selon D
dans une table de huit CRC (largeur w, polynôme réfléchi, longueur maximale L d'un
mot, pas k du nombre de registres) :

    w   polynôme  L      k        w   polynôme  L    k
    11  0x500     2047   2        5   0x14      31   2
    9   0x110     511    2        4   0x0C      15   2
    7   0x60      127    2        3   0x5       7    2
    6   0x30      63     2        2   0x3       3    4

Disposition du mot (bits rangés poids faible d'abord dans chaque octet) :
  - zone de données de (longueur − w) × n bits : les D octets, des zéros, puis un
    champ de b bits (b = bits nécessaires pour compter ⌈max(L·k, 65) / 8⌉ octets)
    qui donne le nombre d'octets de zéros ;
  - n CRC de w bits : le bit i de la zone de données entre dans le registre i mod n
    (CRC réfléchi, valeur initiale 0, sans inversion finale) ; le bit j du registre
    r est rangé au bit (zone + r + j·n).
Nombre de registres : 65 au moins ; au-delà, ⌈bits / (L − w)⌉ arrondi pour que
n − 65 soit multiple de k, la longueur valant alors L. Morceau plein : CRC-11,
n = 255, 255 × 2 047 bits = 65 249 octets = 64 896 + 353.

Établi le 21/10/2026 par lecture du code machine de Utility.dll (Revit 2026,
classes CRCFile / CRCIO, à des fins d'interopérabilité ; aucun code repris) et
vérifié en réencodant tous les flux du corpus d'essai (LISEZMOI § 4).
"""
import math, os, struct, sys

CODES = [(11, 0x500, 2047, 2), (9, 0x110, 511, 2), (7, 0x60, 127, 2), (6, 0x30, 63, 2),
         (5, 0x14, 31, 2), (4, 0x0C, 15, 2), (3, 0x5, 7, 2), (2, 0x3, 3, 4)]
MORCEAU = 64896          # données d'un morceau plein
MIN_REG = 65             # nombre minimal de registres


def bits_champ(c):
    """Largeur du champ qui compte les octets de remplissage."""
    w, p, L, k = c
    n = (max(L * k, MIN_REG) + 7) >> 3
    b, v = 0, 1
    while v < n:
        v, b = v * 2, b + 1
    return b


def disposition_donnees(c, d):
    """(longueur d'un mot en bits, n) pour d octets de données."""
    w, p, L, k = c
    bits = bits_champ(c) + 8 * d
    if bits <= (L - w) * MIN_REG:
        return -(-bits // MIN_REG) + w, MIN_REG
    n = -(-bits // (L - w))
    if (n - MIN_REG) % k:
        n += k - (n - MIN_REG) % k
    return L, n


def disposition_total(c, t):
    """(longueur, n) pour un mot de t octets (lecture)."""
    w, p, L, k = c
    bits = 8 * t
    n = bits // L
    if n > MIN_REG:
        n -= (n - MIN_REG) % k
    if n < MIN_REG:
        return bits // MIN_REG, MIN_REG
    return L, n


def total(c, d):
    lg, n = disposition_donnees(c, d)
    return (lg * n + 7) >> 3


def bits_donnees(c, t):
    lg, n = disposition_total(c, t)
    return (lg - c[0]) * n if lg >= c[0] else 0


def donnees_max(c, t):
    b = bits_donnees(c, t)
    return (b - bits_champ(c)) >> 3 if b > bits_champ(c) else 0


def _seuils(arrondi):
    s_donnees, s_total = [], []
    for c0, c1 in zip(CODES, CODES[1:]):
        x = arrondi(c0[0] / (c1[0] / c1[2]))
        a = donnees_max(c0, ((x * MIN_REG + 7) >> 3) - 1)
        t1 = total(c1, a)
        s_total.append(t1)
        s_donnees.append(donnees_max(c1, t1))
    return s_donnees, s_total


SEUILS_DONNEES, SEUILS_TOTAL = _seuils(math.ceil)


def code_pour_donnees(d):
    """Code utilisé pour d octets de données (le plus petit dont le seuil couvre d)."""
    for i in range(len(SEUILS_DONNEES) - 1, -1, -1):
        if SEUILS_DONNEES[i] >= d:
            return CODES[i + 1]
    return CODES[0]


def encoder(donnees):
    """Mot de code d'un morceau (données suivies du remplissage et des CRC)."""
    d = len(donnees)
    c = code_pour_donnees(d)
    w, poly, L, k = c
    lg, n = disposition_donnees(c, d)
    zone = (lg - w) * n
    b = bits_champ(c)
    rempl = ((zone - b) - 8 * d) >> 3
    t = (lg * n + 7) >> 3
    mot = bytearray(donnees) + bytes(t - d)
    for j in range(b):                      # champ du remplissage
        if (rempl >> j) & 1:
            q = zone - b + j
            mot[q >> 3] |= 1 << (q & 7)
    bits = [(x >> j) & 1 for x in mot[:(zone + 7) >> 3] for j in range(8)][:zone]
    for r in range(n):                      # CRC entrelacés : bit i -> registre i mod n
        x = 0
        for e in bits[r::n]:
            x = (x >> 1) ^ (poly if (x ^ e) & 1 else 0)
        for j in range(w):
            if (x >> j) & 1:
                q = zone + r + j * n
                mot[q >> 3] ^= 1 << (q & 7)
    return bytes(mot)


def encoder_flux(donnees):
    """Flux complet : morceaux de 64 896 octets encodés à la suite."""
    out = bytearray()
    for i in range(0, max(len(donnees), 1), MORCEAU):
        out += encoder(donnees[i:i + MORCEAU])
    return bytes(out)


def decoder_flux(b):
    """Données d'un flux (sans contrôle ni correction)."""
    out, o = bytearray(), 0
    plein = total(CODES[0], MORCEAU)
    while o < len(b):
        t = min(plein, len(b) - o)
        # code d'un mot de t octets : plus petit code dont le seuil de total couvre t
        c = CODES[0]
        for i in range(len(SEUILS_TOTAL) - 1, -1, -1):
            if SEUILS_TOTAL[i] >= t:
                c = CODES[i + 1]
                break
        zone = bits_donnees(c, t)
        bc = bits_champ(c)
        q0 = zone - bc
        rempl = sum(((b[o + ((q0 + j) >> 3)] >> ((q0 + j) & 7)) & 1) << j for j in range(bc))
        d = (zone - bc - 8 * rempl) >> 3
        out += b[o:o + d]
        o += t
    return bytes(out)


def main():
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from cfb import CFB
    ok = ko = 0
    for path in sys.argv[1:]:
        c = CFB(path)
        for p, d in c.tree():
            if d['type'] != 2 or not (p.startswith('/Global/') or p.startswith('/Partitions/') or p in ('/Formats/Latest', '/Contents')):
                continue
            b = c.read(d)
            donnees = decoder_flux(b)
            if encoder_flux(donnees) == b:
                ok += 1
            else:
                ko += 1
                print('KO', os.path.basename(path), p, len(b), len(donnees))
    print('flux identiques %d, différents %d' % (ok, ko))


if __name__ == '__main__':
    main()
