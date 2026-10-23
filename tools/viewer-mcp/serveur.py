"""Serveur MCP du viewer IFC/Revit (stdio, JSON-RPC 2.0, sans dépendance).

Il pilote le viewer par son canal local (127.0.0.1:47811, voir src/pilote.rs) : une
ligne « commande argument » envoyée, une ligne JSON reçue. Si aucun viewer n'écoute, il
en lance un (IFC_VIEWER_EXE, sinon target/release puis target/alt/release).

Outils : ouvrir un fichier, capturer la vue ou la fenêtre (image PNG rendue au client),
lister et ouvrir les vues Revit, régler la caméra, lire les propriétés, désigner
l'objet sous un point, et une commande brute pour le reste.
"""

import base64
import json
import os
import socket
import subprocess
import sys
import tempfile
import time

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PORT = int(os.environ.get("IFC_VIEWER_PORT", "47811"))
CAPTURES = os.path.join(tempfile.gettempdir(), "ifc-viewer-mcp")

COMMANDES = """Commandes du viewer (outil viewer_commande) :
etat ; ouvrir <chemin> ; capture vue|fenetre [LxH] <chemin.png> ; taille <largeur> <hauteur> ;
reduire ; restaurer ; quitter ;
vues ; vue <indice|nom> ; camera dessus|face|iso|cadrer|ortho|perspective ;
camera <lacet°> <tangage°> [distance] ; zoom <facteur> ; afficher <CLASSES> ;
masquer <CLASSES> ; tout_afficher ; classes ; elements [CLASSE|*] [texte] ;
selectionner <id> ; isoler <id> ; cadrer <id> ; proprietes [id] ; objet <x> <y>
(0..1 depuis le coin haut gauche de la vue) ; fiche oui|non (fiche Revit) ;
onglet tout|vues|feuilles|nomenclatures|familles|materiaux|groupes|liens|modele|classes ;
dessin oui|non (rendu 2D des plans et élévations) ; coupe non | coupe <hauteur m> [niveau]
(coupe horizontale au-dessus du niveau, mode dessin) ; niveaux ; poche hachure|plein|vide ;
echelle <dénominateur> (épaisseur des contours coupés, pas des hachures) ;
exporter <chemin.svg> (dessin vectoriel de la vue orthographique : coupe, arêtes vues
sans parties cachées, éléments 2D de la vue Revit ouverte)."""


NOM_EXE = "ifc_viewer.exe" if os.name == "nt" else "ifc_viewer"


def executable():
    candidats = [os.environ.get("IFC_VIEWER_EXE", "")] + [
        os.path.join(RACINE, "target", d, NOM_EXE) for d in ("release", os.path.join("alt", "release"))
    ]
    existants = [c for c in candidats if c and os.path.isfile(c)]
    if not existants:
        raise RuntimeError("viewer introuvable : construire avec cargo build --release ou fixer IFC_VIEWER_EXE")
    # variable explicite d'abord, sinon le plus récent
    if os.environ.get("IFC_VIEWER_EXE") in existants:
        return os.environ["IFC_VIEWER_EXE"]
    return max(existants, key=os.path.getmtime)


def envoyer(ligne, delai=600.0):
    with socket.create_connection(("127.0.0.1", PORT), timeout=delai) as s:
        s.sendall((ligne + "\n").encode("utf-8"))
        tampon = b""
        while not tampon.endswith(b"\n"):
            bloc = s.recv(65536)
            if not bloc:
                break
            tampon += bloc
    return json.loads(tampon.decode("utf-8"))


def lancer():
    exe = executable()
    options = {}
    if os.name == "nt":
        options["creationflags"] = subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        options["start_new_session"] = True
    subprocess.Popen([exe], cwd=RACINE, stdin=subprocess.DEVNULL,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, close_fds=True, **options)
    fin = time.time() + 30
    while time.time() < fin:
        try:
            return envoyer("etat", 5)
        except OSError:
            time.sleep(0.3)
    raise RuntimeError(f"le viewer lancé ({exe}) ne répond pas sur le port {PORT}")


def commande(ligne, delai=600.0):
    try:
        return envoyer(ligne, delai)
    except ConnectionRefusedError:
        lancer()
        return envoyer(ligne, delai)


def texte(r):
    return {"type": "text", "text": json.dumps(r, ensure_ascii=False, indent=1)}


def capture(zone, largeur=None, hauteur=None):
    taille = ""
    if largeur and hauteur:
        if zone == "fenetre":
            commande(f"taille {largeur} {hauteur}")
            time.sleep(0.4)  # le redimensionnement arrive à l'image suivante
        else:
            taille = f"{largeur}x{hauteur} "  # rendu hors écran à cette taille
    os.makedirs(CAPTURES, exist_ok=True)
    chemin = os.path.join(CAPTURES, time.strftime("capture-%Y%m%d-%H%M%S") + f"-{int(time.time() * 1000) % 1000:03d}.png")
    r = commande(f"capture {zone} {taille}{chemin}", 60)
    if not r.get("ok"):
        return [texte(r)], True
    with open(chemin, "rb") as f:
        donnees = base64.b64encode(f.read()).decode("ascii")
    return [texte(r), {"type": "image", "data": donnees, "mimeType": "image/png"}], False


def _obj(props, requis=()):
    return {"type": "object", "properties": props, "required": list(requis)}


OUTILS = [
    ("viewer_etat", "État du viewer : fichier ouvert, vue active, caméra, classes masquées, sélection. Lance le viewer s'il ne tourne pas.", _obj({})),
    ("viewer_ouvrir", "Ouvre un fichier IFC ou Revit (.rvt, .rfa, .rte) et attend la fin du chargement.",
     _obj({"chemin": {"type": "string", "description": "Chemin absolu du fichier"}}, ["chemin"])),
    ("viewer_capture", "Capture du viewer, rendue en image PNG. zone « vue » (défaut) : vue 3D rendue hors écran, à la taille de la vue ou à largeur × hauteur, même fenêtre réduite. zone « fenetre » : interface entière ; largeur et hauteur redimensionnent alors la fenêtre.",
     _obj({"zone": {"type": "string", "enum": ["vue", "fenetre"]}, "largeur": {"type": "integer"}, "hauteur": {"type": "integer"}})),
    ("viewer_vues", "Liste des vues, feuilles et nomenclatures du fichier Revit ouvert (indice, genre, type, nom, échelle).", _obj({})),
    ("viewer_vue", "Ouvre une vue Revit comme un double-clic dans l'arborescence : par indice (voir viewer_vues) ou par nom (« Plan d'étage : Niveau 0 », ou une partie du nom).",
     _obj({"vue": {"type": "string"}}, ["vue"])),
    ("viewer_camera", "Règle la caméra : mode dessus|face|iso|cadrer|ortho|perspective, ou lacet et tangage en degrés (et distance en mètres).",
     _obj({"mode": {"type": "string"}, "lacet": {"type": "number"}, "tangage": {"type": "number"}, "distance": {"type": "number"}})),
    ("viewer_proprietes", "Attributs et jeux de propriétés d'une entité (id STEP, ou la sélection si absent).",
     _obj({"id": {"type": "integer"}})),
    ("viewer_objet", "Objet visible sous un point de la vue (x, y de 0 à 1 depuis le coin haut gauche) et point 3D touché.",
     _obj({"x": {"type": "number"}, "y": {"type": "number"}}, ["x", "y"])),
    ("viewer_commande", "Commande brute du viewer.\n" + COMMANDES,
     _obj({"commande": {"type": "string", "description": "Ligne « commande argument »"}}, ["commande"])),
]


def appeler(nom, a):
    if nom == "viewer_etat":
        return [texte(commande("etat"))], False
    if nom == "viewer_ouvrir":
        r = commande(f"ouvrir {a['chemin']}")
        return [texte(r)], not r.get("ok")
    if nom == "viewer_capture":
        return capture(a.get("zone", "vue"), a.get("largeur"), a.get("hauteur"))
    if nom == "viewer_vues":
        return [texte(commande("vues"))], False
    if nom == "viewer_vue":
        r = commande(f"vue {a['vue']}")
        return [texte(r)], not r.get("ok")
    if nom == "viewer_camera":
        if a.get("mode"):
            r = commande(f"camera {a['mode']}")
        else:
            ligne = f"camera {a.get('lacet', -90)} {a.get('tangage', 30)}"
            if a.get("distance"):
                ligne += f" {a['distance']}"
            r = commande(ligne)
        return [texte(r)], not r.get("ok")
    if nom == "viewer_proprietes":
        r = commande(f"proprietes {a.get('id', '')}".strip())
        return [texte(r)], not r.get("ok")
    if nom == "viewer_objet":
        r = commande(f"objet {a['x']} {a['y']}")
        return [texte(r)], not r.get("ok")
    if nom == "viewer_commande":
        ligne = a["commande"].strip()
        if ligne.startswith("capture"):
            mots = ligne.split() + ["vue"]
            zone = mots[1] if mots[1] in ("vue", "fenetre") else "vue"
            l, _, h = next((m for m in mots[2:] if "x" in m and m.replace("x", "").isdigit()), "x").partition("x")
            return capture(zone, l or None, h or None)
        r = commande(ligne)
        return [texte(r)], not r.get("ok")
    raise ValueError(f"outil inconnu : {nom}")


def repondre(id_, resultat=None, erreur=None):
    m = {"jsonrpc": "2.0", "id": id_}
    if erreur is not None:
        m["error"] = erreur
    else:
        m["result"] = resultat
    sys.stdout.buffer.write((json.dumps(m, ensure_ascii=False) + "\n").encode("utf-8"))
    sys.stdout.buffer.flush()


def main():
    for brut in sys.stdin.buffer:
        brut = brut.strip()
        if not brut:
            continue
        try:
            msg = json.loads(brut.decode("utf-8"))
        except ValueError:
            repondre(None, erreur={"code": -32700, "message": "JSON invalide"})
            continue
        methode, id_ = msg.get("method"), msg.get("id")
        if id_ is None:
            continue  # notification
        p = msg.get("params") or {}
        if methode == "initialize":
            repondre(id_, {
                "protocolVersion": p.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "ifc-viewer", "version": "1.0"},
                "instructions": "Pilote le viewer IFC/Revit du projet : viewer_ouvrir puis viewer_capture pour voir le rendu ; viewer_vue pour ouvrir une vue Revit.",
            })
        elif methode == "ping":
            repondre(id_, {})
        elif methode == "tools/list":
            repondre(id_, {"tools": [{"name": n, "description": d, "inputSchema": s} for n, d, s in OUTILS]})
        elif methode == "tools/call":
            try:
                contenu, est_erreur = appeler(p.get("name"), p.get("arguments") or {})
            except Exception as e:  # erreur rendue au client, le serveur continue
                contenu, est_erreur = [{"type": "text", "text": f"{type(e).__name__} : {e}"}], True
            repondre(id_, {"content": contenu, "isError": est_erreur})
        else:
            repondre(id_, erreur={"code": -32601, "message": f"méthode inconnue : {methode}"})


if __name__ == "__main__":
    main()
