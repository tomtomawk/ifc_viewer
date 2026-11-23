"""Vue de dessin : sans le modèle, annotations tracées, déplacées et supprimées par le
ruban Modifier ; Échap ne fait pas réapparaître le modèle."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe):
    with tempfile.TemporaryDirectory(prefix="bimedit-dessin-") as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN="1", APPDATA=str(root / "configuration"))
        with (root / "instance.log").open("w", encoding="utf-8") as log:
            processus = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)

            def commande(nom, argument=None):
                ligne = nom if argument is None else nom + " " + (argument if isinstance(argument, str) else json.dumps(argument, ensure_ascii=False))
                with socket.create_connection(("127.0.0.1", port), timeout=30) as connexion:
                    connexion.sendall((ligne + "\n").encode("utf-8"))
                    resultat = json.loads(connexion.makefile(encoding="utf-8").readline())
                assert resultat.get("ok"), (nom, resultat)
                return resultat

            def saisie(evenements):
                commande("saisie", evenements)
                limite = time.monotonic() + 15
                while commande("etat")["saisies_en_attente"]:
                    if time.monotonic() > limite:
                        raise TimeoutError("Saisie graphique non terminée")
                    time.sleep(0.05)
                time.sleep(0.15)

            def clic(x, y):
                # survol d'abord : l'élément visé est celui sous le pointeur
                saisie([{"action": "pointer", "x": x, "y": y}])
                saisie([{"action": "clic", "x": x, "y": y}])

            def traits(vue):
                return [a for a in commande("projet_natif")["projet"].get("annotations", []) if a["vue"] == vue]

            try:
                limite = time.monotonic() + 45
                while True:
                    try:
                        commande("etat")
                        break
                    except OSError:
                        if processus.poll() is not None or time.monotonic() > limite:
                            raise RuntimeError("Instance de test indisponible")
                        time.sleep(0.2)
                commande("nouveau", {"chemin": str(root / "Projet.modele.json")})
                commande("mur", "-4000 -4000 4000 -4000 2800 200")
                commande("vue_dessin", "20")
                vue = [v["id"] for v in commande("projet_natif")["projet"]["vues"] if v["genre"] == "dessin"][-1]
                assert commande("etat")["objets_visibles"] == 0
                commande("outil", "polyligne")
                a, b, c = (commande("ecran", f"{x} {y}")["ecran"] for x, y in ((-1000, 0), (1000, 0), (1000, 800)))
                for q in (a, b, c):
                    clic(*q)
                saisie([{"action": "touche", "touche": "Enter"}, {"action": "touche", "touche": "Escape"}, {"action": "touche", "touche": "Escape"}])
                avant = traits(vue)
                assert len(avant) == 1 and abs(avant[0]["points"][0][0] + 1000) < 1, avant
                assert commande("etat")["objets_visibles"] == 0, "Échap a fait réapparaître le modèle"
                clic((a[0] + 3 * b[0]) / 4, (a[1] + 3 * b[1]) / 4)
                assert commande("etat")["selection_native"] == [avant[0]["id"]], commande("etat")["selection_native"]
                # S5 : la polyligne passe par l'origine du projet ; un clic sur l'origine la
                # sélectionne (l'origine invisible ne passe qu'en l'absence d'autre candidat)
                saisie([{"action": "touche", "touche": "Escape"}])
                clic(*commande("ecran", "0 0")["ecran"])
                assert commande("etat")["selection_native"] == [avant[0]["id"]], commande("etat")["selection_native"]
                commande("outil", "deplacer")
                d = commande("ecran", "1000 500")["ecran"]
                clic(*b)
                clic(*d)
                apres = traits(vue)
                assert abs(apres[0]["points"][0][1] - 500) < 1, apres
                saisie([{"action": "touche", "touche": "Escape"}, {"action": "touche", "touche": "Escape"}])
                m = commande("ecran", "0 500")["ecran"]
                clic(*m)
                saisie([{"action": "touche", "touche": "Delete"}])
                assert traits(vue) == []
                print("Vue de dessin : OK")
            finally:
                processus.terminate()
                processus.wait(timeout=15)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/debug/bimedit.exe"))
    run(parser.parse_args().exe)
