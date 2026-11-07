#!/usr/bin/env python3
"""Génère le projet de démonstration « Îlot de démonstration » (samples/ilot_demo) :
terrain avec butte, voirie, parvis, logements collectifs R+3 en brique, bureaux R+2
à façade vitrée, gymnase en bardage zinc, trois maisons enduit/bois à toiture tuiles,
textures procédurales (raccordables) et réglages de rendu de la vue 3D.

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
    materiau("Brique", [150, 70, 48], apparence={"rugosite": 0.8, "textures": {"Couleur": tex("brique.png", 920, 600), "Relief": tex("brique_relief.png", 920, 600, intensite=0.8)}}),
    materiau("Béton banché", [190, 190, 184], apparence={"rugosite": 0.75, "textures": {"Couleur": tex("beton.png", 2500, 2500), "Relief": tex("beton_relief.png", 2500, 2500, intensite=0.3)}}),
    materiau("Enduit blanc", [236, 233, 222], apparence={"rugosite": 0.85, "textures": {"Couleur": tex("enduit.png", 1000, 1000), "Relief": tex("enduit_relief.png", 1000, 1000, intensite=0.15)}}),
    materiau("Bardage bois", [150, 104, 66], apparence={"rugosite": 0.7, "textures": {"Couleur": tex("bois.png", 960, 960), "Relief": tex("bois_relief.png", 960, 960, intensite=0.6)}}),
    materiau("Tuiles", [158, 72, 46], apparence={"rugosite": 0.6, "textures": {"Couleur": tex("tuiles.png", 800, 800), "Relief": tex("tuiles_relief.png", 800, 800, intensite=1.0)}}),
    materiau("Zinc", [150, 156, 162], apparence={"rugosite": 0.35, "metallique": 0.8, "textures": {"Couleur": tex("zinc.png", 1000, 1000), "Relief": tex("zinc_relief.png", 1000, 1000, intensite=0.5)}}),
    materiau("Verre", [160, 190, 200], motif="uni", apparence={"teinte": [150, 185, 195], "rugosite": 0.03, "transparence": 0.6}),
    materiau("Pelouse", [84, 122, 52], apparence={"rugosite": 0.95, "textures": {"Couleur": tex("pelouse.png", 2000, 2000)}}),
    materiau("Enrobé", [70, 70, 70], apparence={"rugosite": 0.9, "textures": {"Couleur": tex("enrobe.png", 1000, 1000)}}),
    materiau("Pavés", [160, 155, 145], apparence={"rugosite": 0.8, "textures": {"Couleur": tex("paves.png", 800, 800), "Relief": tex("paves_relief.png", 800, 800, intensite=0.5)}}),
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


def points_terrain():
    """Grille de points cotés tous les 10 m : terrain plat à 29,90 NGF, butte de 2,5 m
    (profil en cloche, 18 m de rayon) au nord, entre le gymnase et les maisons."""
    pts = []
    # au-delà de la parcelle : grille lâche
    for x in range(-50000, 210000, 30000):
        for y in range(-40000, 160000, 30000):
            if not (-10000 <= x <= 155000 and -10000 <= y <= 105000):
                pts.append([x, y, 29900])
    for x in range(-5000, 150000, 10000):
        for y in range(-5000, 100000, 10000):
            d2 = ((x - 80000) ** 2 + (y - 85000) ** 2) / 18000.0 ** 2
            pts.append([x, y, round(29900 + 2500 * np.exp(-2.0 * d2)) if d2 < 1.5 else 29900])
    return pts


def projet():
    # A : logements collectifs R+3 en brique, toiture terrasse et acrotère
    ax0, ay0, ax1, ay1 = 10000, 22000, 40000, 34000
    for k in range(4):
        sud, est, nord, ouest = boite("A", ax0, ay0, ax1, ay1, f"n{k}", 3000, "brique300", "Logements")
        baies(sud, 30000, 3000, 1400, 1500 if k else 2200, 900 if k else 0, "fenetre" if k else "porte")
        baies(nord, 30000, 3000, 1000, 1400, 1000)
        baies(est, 12000, 4000, 1000, 1400, 1000)
        baies(ouest, 12000, 4000, 1000, 1400, 1000)
        dalle(f"A-p{k}", f"Plancher logements {k}", "dalle250", f"n{k}", 0, rectangle(ax0 - 150, ay0 - 150, ax1 + 150, ay1 + 150))
    dalle("A-toit", "Toiture terrasse logements", "dalle250", "n4", 0, rectangle(ax0 - 150, ay0 - 150, ax1 + 150, ay1 + 150))
    boite("A-acr", ax0, ay0, ax1, ay1, "n4", 900, "brique300", "Acrotère logements")

    # B : bureaux R+2, façade sud vitrée, autres façades en béton banché
    bx0, by0, bx1, by1 = 55000, 24000, 95000, 40000
    for k in range(3):
        sud, est, nord, ouest = boite("B", bx0, by0, bx1, by1, f"n{k}", 3000, ["vitrage", "beton200", "beton200", "beton200"], "Bureaux")
        baies(nord, 40000, 4000, 2400, 1800, 900)
        baies(est, 16000, 4000, 1800, 1800, 900)
        dalle(f"B-p{k}", f"Plancher bureaux {k}", "dalle300", f"n{k}", 0, rectangle(bx0 - 400, by0 - 1200, bx1 + 400, by1 + 400))
    dalle("B-toit", "Toiture bureaux", "dalle300", "n3", 0, rectangle(bx0 - 400, by0 - 1200, bx1 + 400, by1 + 400))
    boite("B-acr", bx0 - 400, by0 - 1200, bx1 + 400, by1 + 400, "n3", 1000, "beton200", "Acrotère bureaux")

    # D : gymnase, bardage zinc, bande vitrée haute, toiture zinc deux pans à 8°
    dx0, dy0, dx1, dy1 = 15000, 50000, 55000, 75000
    sud, est, nord, ouest = boite("D", dx0, dy0, dx1, dy1, "n0", 9000, "zinc200", "Gymnase")
    for m in (sud, est, nord, ouest):
        attaches[m] = "D-toit"
    baies(sud, 40000, 5000, 4200, 1500, 6800, marge=2500)
    baies(nord, 40000, 5000, 4200, 1500, 6800, marge=2500)
    ouvertures.append({"id": "D-entree", "nom": "Entrée gymnase", "genre": "porte", "mur": sud, "type_ouverture": None, "abscisse": 20000, "largeur": 4000, "hauteur": 3000, "allege": 0, "representation": None, "origine": None})
    dalle("D-sol", "Sol gymnase", "dalle250", "n0", 0, rectangle(dx0 - 150, dy0 - 150, dx1 + 150, dy1 + 150))
    dalle("D-toit", "Toiture gymnase", "zinc150", "n3", 0, rectangle(dx0 - 600, dy0 - 600, dx1 + 600, dy1 + 600), sorte="toiture", pentes=[8, None, 8, None])

    # C : trois maisons R+1, enduit au rez-de-chaussée, bois à l'étage, tuiles à 35°
    for i, x0 in enumerate((105000, 117000, 129000)):
        y0 = 52000
        x1, y1 = x0 + 9000, y0 + 8000
        p = f"C{i + 1}"
        sud, est, nord, ouest = boite(p, x0, y0, x1, y1, "n0", 2800, "enduit200", f"Maison {i + 1}")
        ouvertures.append({"id": f"{p}-porte", "nom": "Entrée", "genre": "porte", "mur": sud, "type_ouverture": None, "abscisse": 2200, "largeur": 1000, "hauteur": 2200, "allege": 0, "representation": None, "origine": None})
        ouvertures.append({"id": f"{p}-baie", "nom": "Baie séjour", "genre": "fenetre", "mur": sud, "type_ouverture": None, "abscisse": 6200, "largeur": 3200, "hauteur": 2200, "allege": 0, "representation": None, "origine": None})
        baies(nord, 9000, 3000, 1000, 1200, 1000)
        sud1, est1, nord1, ouest1 = boite(p, x0, y0, x1, y1, "n1", 2800, "bois200", f"Maison {i + 1} étage")
        for m in (sud1, est1, nord1, ouest1):
            attaches[m] = f"{p}-toit"
        baies(sud1, 9000, 3000, 1200, 1400, 900)
        baies(est1, 8000, 4000, 900, 1200, 1000, marge=2000)
        dalle(f"{p}-p0", f"Plancher maison {i + 1}", "dalle250", "n0", 0, rectangle(x0 - 100, y0 - 100, x1 + 100, y1 + 100))
        dalle(f"{p}-p1", f"Plancher étage maison {i + 1}", "dalle250", "n1", 0, rectangle(x0 - 100, y0 - 100, x1 + 100, y1 + 100))
        dalle(f"{p}-toit", f"Toiture maison {i + 1}", "tuiles200", "n1", 2800, rectangle(x0 - 500, y0 - 600, x1 + 500, y1 + 600), sorte="toiture", pentes=[35, None, 35, None])

    # aménagements : voirie au sud, parvis entre logements et bureaux, allée des maisons
    dalle("voirie", "Voirie", "enrobe120", "n0", 0, rectangle(-10000, 3000, 150000, 11000), sorte="sol")
    dalle("parvis", "Parvis", "paves120", "n0", 0, rectangle(8000, 11000, 100000, 21000), sorte="sol")
    dalle("allee", "Allée des maisons", "paves120", "n0", 0, rectangle(100000, 11000, 106000, 62000), sorte="sol")

    return {
        "format": "bimedit-modele",
        "version": 0,
        "nom": "Îlot de démonstration",
        "sources": [], "phases": [], "sous_projets": [], "variantes": [],
        "lieu": {"nom": "Rennes", "latitude": 48.1173, "longitude": -1.6778, "fuseau": 1.0, "heure_ete": True},
        "coordonnees_origine": [0, 0, 30000],
        "niveaux": NIVEAUX,
        "types_murs": [
            type_mur("brique300", "Brique 300", "Brique", 300),
            type_mur("beton200", "Béton banché 200", "Béton banché", 200),
            type_mur("vitrage", "Mur-rideau", "Verre", 60),
            type_mur("zinc200", "Bardage zinc 200", "Zinc", 200),
            type_mur("enduit200", "Enduit 200", "Enduit blanc", 200),
            type_mur("bois200", "Bardage bois 200", "Bardage bois", 200),
        ],
        "types_dalles": [
            type_dalle("dalle250", "Dalle béton 250", "Béton banché", 250),
            type_dalle("dalle300", "Dalle béton 300", "Béton banché", 300),
            type_dalle("zinc150", "Toiture zinc 150", "Zinc", 150),
            type_dalle("tuiles200", "Toiture tuiles 200", "Tuiles", 200),
            type_dalle("enrobe120", "Enrobé 120", "Enrobé", 120),
            type_dalle("paves120", "Pavés 120", "Pavés", 120),
        ],
        "types_ouvertures": [],
        "types_terrains": [{"id": "pelouse", "nom": "Pelouse", "epaisseur": 1000, "materiau": "Pelouse"}],
        "terrains": [{
            "id": "terrain", "nom": "Terrain",
            # cote NGF : niveau 0 à 30,00 ; terrain 10 cm sous les sols (précision de profondeur de loin), butte au nord-est
            "contour": [[-60000, -50000, 29900], [210000, -50000, 29900], [210000, 160000, 29900], [-60000, 160000, 29900]],
            "points": points_terrain(),
            "type_terrain": "pelouse",
        }],
        "murs": murs,
        "assemblages": [],
        "materiaux": MATERIAUX,
        "ouvertures": ouvertures,
        "dalles": dalles,
        "importes": [], "ignores": {},
        "vues": [
            {"id": "plan-rdc", "nom": "Plan RDC", "echelle": 200, "genre": "plan", "niveau": "n0", "coupe": 1200, "bas_niveau": "n0", "bas_decalage": 0},
            {"id": "plan-r1", "nom": "Plan R+1", "echelle": 200, "genre": "plan", "niveau": "n1", "coupe": 1200, "bas_niveau": "n1", "bas_decalage": 0},
            {"id": "coupe-ns", "nom": "Coupe nord-sud", "echelle": 200, "genre": "coupe", "debut": [30000, -5000], "fin": [30000, 95000], "profondeur": 20000, "elevation": False},
        ],
        "rendus_vues": {"3d": {"style": "realiste", "ombres_portees": True, "ombre_ambiante": True, "jour": 21, "mois": 6, "heure": 17.0,
                               "ciel": {"ambiance": "Clair", "brume": 0.15, "nuages": 0.0, "intensite_soleil": 1.0, "intensite_ciel": 1.0, "taille_soleil": 1.5, "sol": [110, 120, 90], "exposition_auto": True, "exposition": 0.0}}},
        "quadrillages": [], "annotations": [], "feuilles": [], "nomenclatures": [],
        "attaches_toitures": attaches,
        "implantations": {},
    }


if __name__ == "__main__":
    os.makedirs(TEX, exist_ok=True)
    for f in (briques, beton, enduit, bois, tuiles, zinc, pelouse, enrobe, paves):
        f()
    with open(os.path.join(SORTIE, "ilot_demo.modele.json"), "w", encoding="utf-8") as fichier:
        json.dump(projet(), fichier, ensure_ascii=False, indent=1)
    print("Projet écrit dans", os.path.abspath(SORTIE))
