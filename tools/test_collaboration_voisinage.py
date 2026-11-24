"""Refus spatiaux de la collaboration (C4) : deux instances graphiques, un projet partagé.

Projet : maison_native plus un niveau à 12 m portant un mur de même tracé que rdc-m0.
Chaque cas modifie un mur sur chaque poste avant de synchroniser A, B puis A.
  a : hauteur de rdc-m0 / nom du mur 12 m plus haut      -> accepté, postes identiques
  c : nom de rdc-m0 / nom de rdc-m1 (murs joints)         -> accepté, postes identiques
  d : hauteur de rdc-m0 / hauteur de rdc-m1 (murs joints) -> conflit attendu (protection)
  b : hauteur de rdc-m0 / hauteur du mur 12 m plus haut   -> conflit relevé (reste de C4)

Usage (Linux sans écran) : xvfb-run -a -s "-screen 0 3200x1200x24" python3 tools/test_collaboration_voisinage.py [exe]
"""
import copy, json, os, socket, subprocess, sys, tempfile, time
from pathlib import Path

racine = Path(__file__).resolve().parent.parent
exe = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else racine / ("target/release/bimedit" + (".exe" if os.name == "nt" else ""))
essai = Path(tempfile.mkdtemp(prefix="bimedit-voisinage-"))
ports = [47981, 47982]
processus = []


def rpc(port, c):
    with socket.create_connection(("127.0.0.1", port), timeout=5) as s:
        s.settimeout(60)
        s.sendall((c + "\n").encode("utf-8"))
        with s.makefile("r", encoding="utf-8") as f:
            return json.loads(f.readline())


def attendre(port, predicat, delai=90):
    limite = time.monotonic() + delai
    while time.monotonic() < limite:
        try:
            r = rpc(port, "etat")
            if predicat(r):
                return r
        except (OSError, ValueError):
            pass
        time.sleep(0.1)
    raise RuntimeError(f"Délai dépassé : port {port} ; journaux : {essai}")


def synchroniser(port):
    rpc(port, "collaboration_sync")
    time.sleep(0.3)
    return attendre(port, lambda r: r.get("collaboration", {}).get("etat") != "Synchronisation…")["collaboration"]


def modifier(port, chemin, valeur):
    r = rpc(port, "projet_natif")
    p = r["projet"]
    mur, champ = chemin
    i = next(k for k, m in enumerate(p["murs"]) if m["id"] == mur)
    ops = [{"op": "replace", "path": f"/murs/{i}/{champ}", "value": valeur}]
    r = rpc(port, "modifier_projet " + json.dumps({"empreinte": r["empreinte"], "operations": ops}, ensure_ascii=False))
    assert r.get("ok"), r


def murs(port):
    return {m["id"]: (m["nom"], m["haut"].get("hauteur")) for m in rpc(port, "projet_natif")["projet"]["murs"]}


p = json.loads((racine / "samples/maison_native.bem").read_text(encoding="utf-8"))
p["niveaux"].append({"id": "haut", "nom": "Haut", "altitude": 12000, "origine": None})
m = copy.deepcopy(next(x for x in p["murs"] if x["id"] == "rdc-m0"))
m.update(id="haut-m0", nom="Mur haut", niveau="haut")
p["murs"].append(m)
H, N = "haut/hauteur", "nom"
cas = [
    ("a", (("rdc-m0", H), 3200), (("haut-m0", N), "Mur haut renommé"), False),
    ("c", (("rdc-m0", N), "Mur A"), (("rdc-m1", N), "Mur B"), False),
    ("d", (("rdc-m0", H), 3100), (("rdc-m1", H), 3150), True),
    ("b", (("rdc-m0", H), 3400), (("haut-m0", H), 2800), None),
]
bilan = {}
try:
    for n, port in enumerate(ports):
        copie = essai / f"poste-{n}.bem"
        copie.write_text(json.dumps(p, ensure_ascii=False), encoding="utf-8")
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN=os.environ.get("BIMEDIT_ARRIERE_PLAN", "1"))
        processus.append(subprocess.Popen([str(exe), str(copie)], env=env, stdout=open(essai / f"poste-{n}.log", "w"), stderr=subprocess.STDOUT))
        attendre(port, lambda r: not r.get("chargement", True) and "selection_native" in r)
        assert rpc(port, "collaboration_associer " + str(essai / "central")).get("ok")
        attendre(port, lambda r: r.get("collaboration", {}).get("etat") == "Synchronisé")
    for port in ports + ports:
        synchroniser(port)
    for nom, (ca, va), (cb, vb), conflit_attendu in cas:
        modifier(ports[0], ca, va)
        modifier(ports[1], cb, vb)
        etats = [synchroniser(ports[0]), synchroniser(ports[1]), synchroniser(ports[0])]
        conflit = any(e.get("conflits") for e in etats)
        bilan[nom] = "conflit" if conflit else "accepté"
        if conflit_attendu is not None:
            assert conflit == conflit_attendu, (nom, etats)
        if not conflit:
            assert murs(ports[0]) == murs(ports[1]), nom
        for port in ports:
            for t in rpc(port, "collaboration_etat").get("conflits") or {}:
                assert rpc(port, "collaboration_resoudre " + json.dumps({"transaction": t, "choix": "acceptee"})).get("ok")
        for port in ports + ports:
            synchroniser(port)
    print(json.dumps({"essai": str(essai), "bilan": bilan}, ensure_ascii=False))
finally:
    for x in processus:
        x.terminate()
        x.wait(timeout=15)
