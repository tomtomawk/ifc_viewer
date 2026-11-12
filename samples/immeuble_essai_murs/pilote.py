"""Client minimal du pilote BIMedit (instance isolée sur un port fixe), avec chronométrage."""
import json
import socket
import time

PORT = 47955


def commande(nom, argument=None, timeout=600):
    """Renvoie (réponse JSON, durée en secondes)."""
    if argument is not None and not isinstance(argument, str):
        argument = json.dumps(argument, ensure_ascii=False, separators=(",", ":"))
    ligne = nom + (" " + argument if argument is not None else "")
    debut = time.perf_counter()
    with socket.create_connection(("127.0.0.1", PORT), timeout=timeout) as s:
        s.sendall((ligne + "\n").encode())
        reponse = json.loads(s.makefile("r", encoding="utf-8").readline())
    return reponse, time.perf_counter() - debut


def attendre_repos(timeout=600):
    """Attend la fin du chargement et des saisies ; renvoie la durée d'attente."""
    debut = time.perf_counter()
    while time.perf_counter() - debut < timeout:
        e, _ = commande("etat")
        if not e.get("chargement") and not e.get("saisies_en_attente"):
            return time.perf_counter() - debut
        time.sleep(0.02)
    raise TimeoutError("instance occupée")
