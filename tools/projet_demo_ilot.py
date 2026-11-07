#!/usr/bin/env python3
"""Génère le projet de démonstration « Îlot de démonstration » (samples/ilot_demo) :
rue (chaussée, trottoirs, alignement d'arbres), logements R+3 en brique avec vitrine au
rez-de-chaussée, bureaux R+4 à mur-rideau sur parvis, cour-jardin, trois maisons
enduit/bois à toiture tuiles avec jardins ; textures (procédurales et pelouse CC0
d'ambientCG), réglages de rendu et trois vues caméra (rue, cour, aérienne).

Les arbres sont contrôlés : aucun houppier ne touche un bâtiment ni la chaussée.

Usage : python3 tools/projet_demo_ilot.py [dossier de sortie]
Nécessite numpy et Pillow (textures).
"""
import json
import os
import sys

import numpy as np
from PIL import Image

SORTIE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "samples", "ilot_demo")
TEX = os.path.join(SORTIE, "textures")
N = 512
rng = np.random.default_rng(7)


# ---------------------------------------------------------------- textures
def bruit(echelle, graine):
    """Bruit de valeurs raccordable (somme d'octaves), 0..1."""
    g = np.random.default_rng(graine)
    total = np.zeros((N, N))
    amp, poids = 1.0, 0.0
    for o in range(5):
        k = max(2, echelle * 2 ** o)
        grille = g.random((k, k))
        x = np.linspace(0, k, N, endpoint=False)
        i0 = np.floor(x).astype(int) % k
        i1 = (i0 + 1) % k
        f = x - np.floor(x)
        f = f * f * (3 - 2 * f)
        a = grille[np.ix_(i0, i0)] * (1 - f)[None, :] + grille[np.ix_(i0, i1)] * f[None, :]
        b = grille[np.ix_(i1, i0)] * (1 - f)[None, :] + grille[np.ix_(i1, i1)] * f[None, :]
        total += amp * (a * (1 - f)[:, None] + b * f[:, None])
        poids += amp
        amp *= 0.5
    return total / poids


def enregistrer(nom, rgb):
    Image.fromarray(np.clip(rgb, 0, 255).astype(np.uint8)).save(os.path.join(TEX, nom))


def gris(v):
    return np.repeat(np.clip(v, 0, 1)[:, :, None] * 255, 3, axis=2)


def teinte(base, variation, champ):
    return np.array(base)[None, None, :] * (1 + variation * (champ[:, :, None] - 0.5))


def briques():
    # carreau de 4 briques de 230 mm sur 8 assises de 75 mm (920 x 600 mm)
    y, x = np.mgrid[0:N, 0:N]
    assise = (y * 8 // N)
    xx = (x + (assise % 2) * (N // 8)) % N
    brique = xx * 4 // N
    joint = ((y * 8 % N) < N * 0.13) | ((xx * 4 % N) < N * 0.045)
    h = rng.random((8, 4))
    ton = h[assise, brique]
    n = bruit(16, 1)
    coul = np.array([150, 62, 40])[None, None, :] * (0.78 + 0.4 * ton[:, :, None]) * (0.9 + 0.2 * n[:, :, None])
    coul[joint] = np.array([196, 188, 172]) * (0.92 + 0.12 * n[joint][:, None])
    enregistrer("brique.png", coul)
    relief = np.where(joint, 0.15, 0.85 + 0.1 * n)
    enregistrer("brique_relief.png", gris(relief))


def beton():
    # banches 2,50 x 1,25 m : joints et trous de banche
    n = bruit(6, 2)
    v = 0.74 + 0.12 * (n - 0.5) + 0.04 * (bruit(48, 3) - 0.5)
    y, x = np.mgrid[0:N, 0:N]
    joint = (x % N < 3) | (y % (N // 2) < 3)
    v[joint] -= 0.12
    for cx in (N // 8, 3 * N // 8, 5 * N // 8, 7 * N // 8):
        for cy in (N // 8, 3 * N // 8, 5 * N // 8, 7 * N // 8):
            v[(x - cx) ** 2 + (y - cy) ** 2 < 25] -= 0.25
    enregistrer("beton.png", gris(v) * np.array([1.0, 1.0, 0.97]))
    enregistrer("beton_relief.png", gris(np.where(joint, 0.3, 0.6 + 0.2 * n)))


def enduit():
    v = 0.92 + 0.06 * (bruit(64, 4) - 0.5) + 0.03 * (bruit(8, 5) - 0.5)
    enregistrer("enduit.png", gris(v) * np.array([1.0, 0.99, 0.95]))
    enregistrer("enduit_relief.png", gris(0.5 + 0.5 * (bruit(96, 6) - 0.5)))


def bois():
    # lames verticales de 120 mm (8 lames sur 960 mm), fil du bois
    y, x = np.mgrid[0:N, 0:N]
    lame = x * 8 // N
    h = rng.random(8)
    fil = np.sin((x * 0.9 + 18 * bruit(4, 7)) * 0.35 + lame * 3) * 0.5 + 0.5
    coul = np.array([150, 104, 66])[None, None, :] * (0.8 + 0.35 * h[lame][:, :, None]) * (0.88 + 0.18 * fil[:, :, None])
    joint = (x * 8 % N) < 5
    coul[joint] *= 0.45
    enregistrer("bois.png", coul)
    enregistrer("bois_relief.png", gris(np.where(joint, 0.1, 0.7 + 0.2 * fil)))


def tuiles():
    # tuiles de 200 x 200 mm à recouvrement (carreau 800 mm)
    y, x = np.mgrid[0:N, 0:N]
    rang = y * 4 // N
    xx = (x + (rang % 2) * (N // 8)) % N
    t = (y * 4 % N) / (N / 4)
    col = xx * 4 // N
    h = rng.random((4, 4))
    ombre = 0.55 + 0.45 * t
    bord = (xx * 4 % N) < 4
    coul = np.array([158, 70, 44])[None, None, :] * (0.8 + 0.3 * h[rang, col][:, :, None]) * ombre[:, :, None]
    coul[bord] *= 0.6
    enregistrer("tuiles.png", coul)
    enregistrer("tuiles_relief.png", gris(np.where(bord, 0.2, t)))


def zinc():
    # bardage zinc à joints debout tous les 500 mm (carreau 1000 mm)
    y, x = np.mgrid[0:N, 0:N]
    joint = np.abs((x % (N // 2)) - 4) < 4
    n = bruit(12, 8)
    v = 0.62 + 0.08 * (n - 0.5)
    v[joint] = 0.78
    enregistrer("zinc.png", gris(v) * np.array([0.96, 0.98, 1.0]))
    enregistrer("zinc_relief.png", gris(np.where(joint, 1.0, 0.3)))


def feuillage():
    # touffes de feuilles : bruit fin contrasté, reflets clairs
    n = bruit(48, 13)
    m = bruit(8, 14)
    v = np.clip((n - 0.35) * 1.8, 0, 1)
    coul = np.array([62, 96, 40])[None, None, :] * (0.55 + 0.75 * v[:, :, None]) * (0.85 + 0.3 * m[:, :, None])
    enregistrer("feuillage.png", coul)
    enregistrer("feuillage_relief.png", gris(v))


def feuilles():
    """Feuilles détourées raccordables (carreau de 700 mm) : couleur et masque de
    découpe (blanc : feuille), pour les plans de feuillage des arbres."""
    from PIL import ImageDraw
    n = 1024
    coul = Image.new("RGB", (n, n), (40, 60, 30))
    masque = Image.new("L", (n, n), 0)
    dc, dm = ImageDraw.Draw(coul), ImageDraw.Draw(masque)
    g = np.random.default_rng(21)
    for _ in range(620):
        cx, cy = g.random() * n, g.random() * n
        long, larg = g.uniform(38, 70), g.uniform(16, 30)
        a = g.uniform(0, np.pi)
        ton = g.uniform(0.65, 1.2)
        base = np.array([70, 112, 45]) * ton + g.uniform(-12, 12, 3)
        t = np.linspace(0, 2 * np.pi, 18)
        # feuille : ellipse pointue
        x, y = np.cos(t) * long / 2, np.sin(t) * larg / 2 * (1 - 0.35 * np.cos(t) ** 2)
        pts = np.stack([x * np.cos(a) - y * np.sin(a), x * np.sin(a) + y * np.cos(a)], 1)
        for ox in (-n, 0, n):
            for oy in (-n, 0, n):
                poly = [(cx + px + ox, cy + py + oy) for px, py in pts]
                dc.polygon(poly, fill=tuple(int(v) for v in np.clip(base, 0, 255)))
                dm.polygon(poly, fill=255)
                # nervure claire
                dc.line([poly[0], poly[9]], fill=tuple(int(v) for v in np.clip(base * 1.25, 0, 255)), width=1)
    coul.save(os.path.join(TEX, "feuilles.png"))
    masque.save(os.path.join(TEX, "feuilles_masque.png"))


def ecorce():
    y, x = np.mgrid[0:N, 0:N]
    n = bruit(6, 15)
    sillons = np.abs(np.sin((x + 40 * n) * 0.12)) ** 0.5
    coul = np.array([96, 80, 64])[None, None, :] * (0.6 + 0.45 * sillons[:, :, None])
    enregistrer("ecorce.png", coul)
    enregistrer("ecorce_relief.png", gris(sillons))


def pelouse():
    n = bruit(32, 9)
    m = bruit(4, 10)
    coul = teinte([84, 122, 52], 0.5, n) * (0.85 + 0.3 * m[:, :, None])
    enregistrer("pelouse.png", coul)


def enrobe():
    n = bruit(96, 11)
    g = rng.random((N, N))
    v = 0.28 + 0.08 * (n - 0.5) + np.where(g > 0.985, 0.25, 0) - np.where(g < 0.01, 0.08, 0)
    enregistrer("enrobe.png", gris(v))


def paves():
    # pavés 200 x 100 mm à joints décalés (carreau 800 mm)
    y, x = np.mgrid[0:N, 0:N]
    rang = y * 8 // N
    xx = (x + (rang % 2) * (N // 8)) % N
    joint = ((y * 8 % N) < 5) | ((xx * 4 % N) < 5)
    h = rng.random((8, 4))
    v = 0.6 + 0.15 * (h[rang, xx * 4 // N] - 0.5) + 0.05 * (bruit(48, 12) - 0.5)
    v[joint] = 0.35
    enregistrer("paves.png", gris(v) * np.array([1.0, 0.97, 0.92]))
    enregistrer("paves_relief.png", gris(np.where(joint, 0.1, 0.8)))


# ---------------------------------------------------------------- projet
def materiau(nom, couleur, motif="diagonales", apparence=None):
    m = {"nom": nom, "couleur": couleur, "motif": motif}
    if apparence:
        m["apparence"] = apparence
    return m


def tex(fichier, largeur, hauteur, **x):
    return {"chemin": "textures/" + fichier, "largeur": largeur, "hauteur": hauteur, **x}


MATERIAUX = [
    materiau("Brique", [150, 70, 48], apparence={"rugosite": 0.8, "textures": {"Couleur": tex("brique.png", 920, 600), "Relief": tex("brique_relief.png", 920, 600, intensite=0.8, profondeur=12)}}),
    materiau("Béton banché", [190, 190, 184], apparence={"rugosite": 0.75, "textures": {"Couleur": tex("beton.png", 2500, 2500), "Relief": tex("beton_relief.png", 2500, 2500, intensite=0.3)}}),
    materiau("Enduit blanc", [236, 233, 222], apparence={"rugosite": 0.85, "textures": {"Couleur": tex("enduit.png", 1000, 1000), "Relief": tex("enduit_relief.png", 1000, 1000, intensite=0.15)}}),
    materiau("Bardage bois", [150, 104, 66], apparence={"rugosite": 0.7, "textures": {"Couleur": tex("bois.png", 960, 960), "Relief": tex("bois_relief.png", 960, 960, intensite=0.6)}}),
    materiau("Tuiles", [158, 72, 46], apparence={"rugosite": 0.6, "textures": {"Couleur": tex("tuiles.png", 800, 800), "Relief": tex("tuiles_relief.png", 800, 800, intensite=1.0)}}),
    materiau("Zinc", [150, 156, 162], apparence={"rugosite": 0.35, "metallique": 0.8, "textures": {"Couleur": tex("zinc.png", 1000, 1000), "Relief": tex("zinc_relief.png", 1000, 1000, intensite=0.5)}}),
    materiau("Verre", [160, 190, 200], motif="uni", apparence={"teinte": [150, 185, 195], "rugosite": 0.03, "transparence": 0.6}),
    # feuillage des arbres (style Réaliste) : feuilles détourées, découpe
    materiau("Feuillage", [70, 105, 45], apparence={"rugosite": 0.7, "textures": {"Couleur": tex("feuilles.png", 700, 700), "Decoupe": tex("feuilles_masque.png", 700, 700)}}),
    materiau("Écorce", [96, 80, 64], apparence={"rugosite": 0.9, "textures": {"Couleur": tex("ecorce.png", 600, 600), "Relief": tex("ecorce_relief.png", 600, 600, intensite=0.8)}}),
    # pelouse : « Grass004 » d'ambientCG (CC0, ambientcg.com), carreau de 1,5 m,
    # anti-répétition, parallaxe, brins de 60 mm
    materiau("Pelouse", [84, 122, 52], apparence={"rugosite": 0.95, "herbe": 60, "textures": {
        "Couleur": tex("herbe_diff.jpg", 1500, 1500, anti_repetition=True),
        "Normale": tex("herbe_nor_gl.jpg", 1500, 1500, anti_repetition=True, intensite=0.8),
        "Relief": tex("herbe_disp.jpg", 1500, 1500, anti_repetition=True, profondeur=15, intensite=1.0),
        "Occlusion": tex("herbe_ao.jpg", 1500, 1500, anti_repetition=True)}}),
    materiau("Enrobé", [70, 70, 70], apparence={"rugosite": 0.9, "textures": {"Couleur": tex("enrobe.png", 1000, 1000)}}),
    materiau("Pavés", [160, 155, 145], apparence={"rugosite": 0.8, "textures": {"Couleur": tex("paves.png", 800, 800), "Relief": tex("paves_relief.png", 800, 800, intensite=0.8, profondeur=10)}}),
]


def type_mur(id, nom, materiau, epaisseur):
    return {"id": id, "nom": nom, "couches": [{"materiau": materiau, "epaisseur": epaisseur, "fonction": "structure"}], "couches_detaillees": True, "origine": None}


def type_dalle(id, nom, materiau, epaisseur):
    return {"id": id, "nom": nom, "couches": [{"materiau": materiau, "epaisseur": epaisseur, "fonction": "structure"}], "couches_detaillees": True, "origine": None}


NIVEAUX = [{"id": f"n{k}", "nom": "RDC" if k == 0 else f"R+{k}", "altitude": 3000 * k, "origine": None} for k in range(5)]

murs, ouvertures, dalles = [], [], []
# murs prolongés jusqu'à la sous-face de toiture (pignons)
attaches = {}


def rectangle(x0, y0, x1, y1):
    return [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]


def boite(prefixe, x0, y0, x1, y1, niveau, hauteur, types, nom):
    """Quatre murs (sud, est, nord, ouest) ; types : un type ou un par face."""
    if isinstance(types, str):
        types = [types] * 4
    pts = rectangle(x0, y0, x1, y1)
    ids = []
    for k in range(4):
        a, b = pts[k], pts[(k + 1) % 4]
        id = f"{prefixe}-{niveau}-{k}"
        murs.append({"id": id, "nom": f"{nom} {['sud', 'est', 'nord', 'ouest'][k]}", "type_mur": types[k], "niveau": niveau, "decalage_bas": 0, "haut": {"contrainte": "libre", "hauteur": hauteur}, "axe": {"forme": "droit", "debut": a, "fin": b}, "origine": None})
        ids.append(id)
    return ids


def baies(mur, longueur, pas, largeur, hauteur, allege, genre="fenetre", marge=1500):
    n = int((longueur - 2 * marge) // pas) + 1
    debut = (longueur - (n - 1) * pas) / 2
    for i in range(n):
        ouvertures.append({"id": f"{mur}-b{i}", "nom": "Baie", "genre": genre, "mur": mur, "type_ouverture": None, "abscisse": debut + i * pas, "largeur": largeur, "hauteur": hauteur, "allege": allege, "representation": None, "origine": None})


def dalle(id, nom, type_dalle, niveau, decalage, contour, sorte="dalle", pentes=None):
    d = {"id": id, "nom": nom, "sorte": sorte, "type_dalle": type_dalle, "niveau": niveau, "decalage": decalage, "contours": [contour], "origine": None}
    if pentes:
        d["pentes"] = [pentes]
    dalles.append(d)


# ---------------------------------------------------------------- plan masse
# Repère : x vers l'est, y vers le nord (mm). Rue au sud, bâtiments en retrait, cour au
# nord, maisons à l'est. Chaque emprise est déclarée ici et sert au contrôle des arbres.
CHAUSSEE = (-60000, 2000, 180000, 9000)
TROTTOIR_NORD = (-60000, 9000, 180000, 12000)
TROTTOIR_SUD = (-60000, -1000, 180000, 2000)
LOGEMENTS = (6000, 16000, 42000, 29000)
BUREAUX = (54000, 18000, 84000, 34000)
MAISONS = [(x, 48000, x + 8000, 57000) for x in (90000, 100000, 110000)]
EMPRISES = [LOGEMENTS, BUREAUX] + MAISONS


def points_terrain():
    """Grille de points cotés : terrain plat à 29,90 NGF (10 cm sous les sols)."""
    pts = []
    for x in range(-50000, 180000, 20000):
        for y in range(-30000, 140000, 20000):
            pts.append([x, y, 29900])
    return pts


def arbres():
    """Alignement sur le trottoir nord, feuillus dans la cour, fastigiés le long de
    l'allée des maisons, petits arbres dans les jardins, bosquet de pins au nord-ouest.
    Contrôle : aucun houppier ne touche un bâtiment ni la chaussée."""
    out = []

    def arbre(id, x, y, essence, hauteur, diametre, nom):
        r = diametre / 2 + 500
        for (x0, y0, x1, y1) in EMPRISES:
            # distance du centre au rectangle
            dx, dy = max(x0 - x, 0, x - x1), max(y0 - y, 0, y - y1)
            assert dx * dx + dy * dy > r * r, f"arbre {id} trop près d'un bâtiment"
        assert not (CHAUSSEE[1] - 500 < y < CHAUSSEE[3] + 500), f"arbre {id} sur la chaussée"
        out.append({"id": id, "nom": nom, "niveau": "n0", "position": [x, y], "decalage": -100,
                    "essence": essence, "hauteur": hauteur, "diametre": diametre})
    for i, x in enumerate(range(3000, 120000, 9000)):
        arbre(f"rue-{i + 1}", x, 10500, "feuillu", 8000 + (i % 3) * 800, 5500, "Tilleul")
    for i, (x, y) in enumerate([(14000, 44000), (30000, 50000), (62000, 46000), (76000, 52000), (40000, 62000)]):
        arbre(f"cour-{i + 1}", x, y, "feuillu", 10000 + i * 500, 7000, "Érable")
    for i, y in enumerate(range(40000, 62000, 6000)):
        arbre(f"allee-{i + 1}", 85000, y, "fastigie", 12000, 2800, "Charme fastigié")
    for i, (x0, _, x1, _) in enumerate(MAISONS):
        arbre(f"jardin-{i + 1}", (x0 + x1) // 2, 66000, "feuillu", 6000, 4500, "Pommier")
    g = np.random.default_rng(3)
    for i in range(6):
        arbre(f"pin-{i + 1}", int(4000 + g.random() * 14000), int(64000 + g.random() * 12000), "conifere", int(11000 + g.random() * 3000), 4500, "Pin")
    return out


def plancher(id, nom, type_dalle, niveau, x0, y0, x1, y1, retrait, debords=None):
    """Plancher au nu intérieur des murs (retrait : demi-épaisseurs par face, sud, est,
    nord, ouest) et débords filants en bandes extérieures (mm par face)."""
    s, e, n, o = retrait
    dalle(id, nom, type_dalle, niveau, 0, rectangle(x0 + o, y0 + s, x1 - e, y1 - n))
    if debords:
        ds, de, dn, do = debords
        if ds:
            dalle(f"{id}-sud", f"{nom}, débord sud", type_dalle, niveau, 0, rectangle(x0 - do, y0 - ds, x1 + de, y0 - s))
        if dn:
            dalle(f"{id}-nord", f"{nom}, débord nord", type_dalle, niveau, 0, rectangle(x0 - do, y1 + n, x1 + de, y1 + dn))
        if do:
            dalle(f"{id}-ouest", f"{nom}, débord ouest", type_dalle, niveau, 0, rectangle(x0 - do, y0 - s, x0 - o, y1 + n))
        if de:
            dalle(f"{id}-est", f"{nom}, débord est", type_dalle, niveau, 0, rectangle(x1 + e, y0 - s, x1 + de, y1 + n))


def camera(id, nom, oeil, cible, champ=55.0):
    return {"id": id, "nom": nom, "type_vue": "Vue piéton" if oeil[2] < 3000 else "Vue caméra", "echelle": 100, "genre": "camera", "oeil": oeil, "cible": cible, "champ": champ}


def rendu(projection, heure=16.0):
    return {"style": "realiste", "projection": projection, "ombres_portees": True, "ombre_ambiante": True, "jour": 21, "mois": 6, "heure": heure,
            "ciel": {"ambiance": "Clair", "brume": 0.15, "nuages": 0.0, "intensite_soleil": 1.0, "intensite_ciel": 1.0, "taille_soleil": 1.5, "sol": [110, 120, 90], "exposition_auto": True, "exposition": 0.0}}


def projet():
    # logements R+3 : brique, vitrine vitrée au rez-de-chaussée sur rue, toiture terrasse
    ax0, ay0, ax1, ay1 = LOGEMENTS
    for k in range(4):
        types = ["vitrage" if k == 0 else "brique300", "brique300", "brique300", "brique300"]
        sud, est, nord, ouest = boite("A", ax0, ay0, ax1, ay1, f"n{k}", 3000, types, "Logements")
        if k:
            baies(sud, 36000, 3000, 1400, 1500, 900)
        baies(nord, 36000, 3000, 1000, 1400, 1000)
        baies(est, 13000, 4000, 1000, 1400, 1000)
        baies(ouest, 13000, 4000, 1000, 1400, 1000)
        # vitrage au sud (30 de l'axe), brique 300 ailleurs (150)
        plancher(f"A-p{k}", f"Plancher logements {k}", "dalle250", f"n{k}", ax0, ay0, ax1, ay1, (30 if k == 0 else 150, 150, 150, 150))
    plancher("A-toit", "Toiture terrasse logements", "dalle250", "n4", ax0, ay0, ax1, ay1, (150, 150, 150, 150))
    boite("A-acr", ax0, ay0, ax1, ay1, "n4", 900, "brique300", "Acrotère logements")

    # bureaux R+4 : mur-rideau au sud et à l'ouest, béton banché au nord et à l'est,
    # débords filants (brise-soleil) au sud
    bx0, by0, bx1, by1 = BUREAUX
    for k in range(5):
        sud, est, nord, ouest = boite("B", bx0, by0, bx1, by1, f"n{k}", 3000, ["vitrage", "beton200", "beton200", "vitrage"], "Bureaux")
        baies(nord, 30000, 3000, 1800, 1800, 900)
        baies(est, 16000, 4000, 1800, 1800, 900)
        plancher(f"B-p{k}", f"Plancher bureaux {k}", "dalle300", f"n{k}", bx0, by0, bx1, by1, (30, 100, 100, 30), (1000, 0, 0, 0) if k else None)
    plancher("B-toit", "Toiture bureaux", "dalle300", "n5", bx0, by0, bx1, by1, (30, 100, 100, 30), (1000, 0, 0, 0))
    boite("B-acr", bx0, by0, bx1, by1, "n5", 1000, "beton200", "Acrotère bureaux")

    # maisons R+1 : enduit au rez-de-chaussée, bois à l'étage, tuiles à 35°
    attaches = {}
    for i, (x0, y0, x1, y1) in enumerate(MAISONS):
        p = f"C{i + 1}"
        sud, est, nord, ouest = boite(p, x0, y0, x1, y1, "n0", 2800, "enduit200", f"Maison {i + 1}")
        ouvertures.append({"id": f"{p}-porte", "nom": "Entrée", "genre": "porte", "mur": sud, "type_ouverture": None, "abscisse": 2000, "largeur": 1000, "hauteur": 2200, "allege": 0, "representation": None, "origine": None})
        ouvertures.append({"id": f"{p}-baie", "nom": "Baie séjour", "genre": "fenetre", "mur": sud, "type_ouverture": None, "abscisse": 5500, "largeur": 3000, "hauteur": 2200, "allege": 0, "representation": None, "origine": None})
        baies(nord, 8000, 3000, 1200, 1200, 1000)
        sud1, est1, nord1, ouest1 = boite(p, x0, y0, x1, y1, "n1", 2800, "bois200", f"Maison {i + 1} étage")
        baies(sud1, 8000, 3000, 1200, 1400, 900)
        baies(nord1, 8000, 3000, 1000, 1200, 1000)
        for m in (sud1, est1, nord1, ouest1):
            attaches[m] = f"{p}-toit"
        plancher(f"{p}-p0", f"Plancher maison {i + 1}", "dalle250", "n0", x0, y0, x1, y1, (100, 100, 100, 100))
        plancher(f"{p}-p1", f"Plancher étage maison {i + 1}", "dalle250", "n1", x0, y0, x1, y1, (100, 100, 100, 100))
        dalle(f"{p}-toit", f"Toiture maison {i + 1}", "tuiles200", "n1", 2800, rectangle(x0 - 500, y0 - 600, x1 + 500, y1 + 600), sorte="toiture", pentes=[35, None, 35, None])

    # rue, trottoirs, parvis, allées
    dalle("chaussee", "Chaussée", "enrobe120", "n0", -20, rectangle(*CHAUSSEE), sorte="sol")
    dalle("trottoir-nord", "Trottoir nord", "paves120", "n0", 0, rectangle(*TROTTOIR_NORD), sorte="sol")
    dalle("trottoir-sud", "Trottoir sud", "paves120", "n0", 0, rectangle(*TROTTOIR_SUD), sorte="sol")
    dalle("parvis-bureaux", "Parvis des bureaux", "paves120", "n0", 0, rectangle(46000, 12000, 92000, 17000), sorte="sol")
    dalle("seuil-logements", "Seuil des logements", "paves120", "n0", 0, rectangle(6000, 12000, 42000, 15950), sorte="sol")
    dalle("allee-cour", "Allée de la cour", "paves120", "n0", 0, rectangle(46000, 17000, 50000, 60000), sorte="sol")
    dalle("allee-maisons", "Allée des maisons", "paves120", "n0", 0, rectangle(87000, 17000, 89500, 60000), sorte="sol")
    dalle("allee-jardins", "Desserte des maisons", "paves120", "n0", 0, rectangle(89500, 44000, 120000, 47500), sorte="sol")

    return {
        "format": "bimedit-modele",
        "version": 0,
        "nom": "Îlot de démonstration",
        "sources": [], "phases": [], "sous_projets": [], "variantes": [],
        "lieu": {"nom": "Rennes", "latitude": 48.1173, "longitude": -1.6778, "fuseau": 1.0, "heure_ete": True},
        "coordonnees_origine": [0, 0, 30000],
        "niveaux": [{"id": f"n{k}", "nom": "RDC" if k == 0 else f"R+{k}", "altitude": 3000 * k, "origine": None} for k in range(6)],
        "types_murs": [
            type_mur("brique300", "Brique 300", "Brique", 300),
            type_mur("beton200", "Béton banché 200", "Béton banché", 200),
            type_mur("vitrage", "Mur-rideau", "Verre", 60),
            type_mur("enduit200", "Enduit 200", "Enduit blanc", 200),
            type_mur("bois200", "Bardage bois 200", "Bardage bois", 200),
        ],
        "types_dalles": [
            type_dalle("dalle250", "Dalle béton 250", "Béton banché", 250),
            type_dalle("dalle300", "Dalle béton 300", "Béton banché", 300),
            type_dalle("tuiles200", "Toiture tuiles 200", "Tuiles", 200),
            type_dalle("enrobe120", "Enrobé 120", "Enrobé", 120),
            type_dalle("paves120", "Pavés 120", "Pavés", 120),
        ],
        "types_ouvertures": [],
        "types_terrains": [{"id": "pelouse", "nom": "Pelouse", "epaisseur": 1000, "materiau": "Pelouse"}],
        "terrains": [{
            "id": "terrain", "nom": "Terrain",
            "contour": [[-60000, -40000, 29900], [180000, -40000, 29900], [180000, 140000, 29900], [-60000, 140000, 29900]],
            "points": points_terrain(),
            "type_terrain": "pelouse",
        }],
        "arbres": arbres(),
        "murs": murs,
        "assemblages": [],
        "materiaux": [m for m in MATERIAUX if m["nom"] != "Zinc"],
        "ouvertures": ouvertures,
        "dalles": dalles,
        "importes": [], "ignores": {},
        "vues": [
            {"id": "plan-rdc", "nom": "Plan RDC", "echelle": 200, "genre": "plan", "niveau": "n0", "coupe": 1200, "bas_niveau": "n0", "bas_decalage": 0},
            {"id": "plan-r1", "nom": "Plan R+1", "echelle": 200, "genre": "plan", "niveau": "n1", "coupe": 1200, "bas_niveau": "n1", "bas_decalage": 0},
            {"id": "coupe-ns", "nom": "Coupe nord-sud", "echelle": 200, "genre": "coupe", "debut": [69000, -5000], "fin": [69000, 75000], "profondeur": 20000, "elevation": False},
            camera("vue-rue", "Vue piéton depuis la rue", [22000, 500, 1500], [62000, 24000, 6000]),
            camera("vue-cour", "Vue piéton dans la cour", [22000, 62000, 1500], [62000, 30000, 6000]),
            camera("vue-aerienne", "Vue aérienne", [150000, -45000, 55000], [55000, 38000, 0], 45.0),
        ],
        "rendus_vues": {
            "3d": rendu("perspective"),
            "vue-rue": rendu("deux_points", 11.0),
            "vue-cour": rendu("deux_points", 17.0),
            "vue-aerienne": rendu("perspective", 15.0),
        },
        "quadrillages": [], "annotations": [], "feuilles": [], "nomenclatures": [],
        "attaches_toitures": attaches,
        "implantations": {},
    }


if __name__ == "__main__":
    os.makedirs(TEX, exist_ok=True)
    for f in (briques, beton, enduit, bois, tuiles, feuilles, ecorce, enrobe, paves):
        f()
    with open(os.path.join(SORTIE, "ilot_demo.modele.json"), "w", encoding="utf-8") as fichier:
        json.dump(projet(), fichier, ensure_ascii=False, indent=1)
    print("Projet écrit dans", os.path.abspath(SORTIE))
