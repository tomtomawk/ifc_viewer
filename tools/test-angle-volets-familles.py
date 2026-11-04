"""Régression du raccord en angle, des volets natifs et de la navigation des familles."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe, captures):
    captures.mkdir(parents=True, exist_ok=True)
    captures=captures.resolve()
    with tempfile.TemporaryDirectory(prefix="bimedit-interface-") as dossier:
        root=Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1",0));port=s.getsockname()[1]
        env=dict(os.environ,BIMEDIT_PORT=str(port),APPDATA=str(root/"configuration"),BIMEDIT_CACHE_COLLABORATION=str(root/"cache"))
        log=(root/"instance.log").open("w",encoding="utf-8")
        p=None
        nombre=0

        def commande(nom,argument=None,ok=True):
            nonlocal nombre
            if argument is not None and not isinstance(argument,str):
                argument=json.dumps(argument,ensure_ascii=False,separators=(",",":"))
            ligne=nom+(" "+argument if argument is not None else "")
            with socket.create_connection(("127.0.0.1",port),timeout=30) as s:
                s.sendall((ligne+"\n").encode());r=json.loads(s.makefile("r",encoding="utf-8").readline())
            assert r.get("ok")==ok,(nom,r)
            nombre+=1
            return r

        def attendre(f,timeout=15):
            fin=time.monotonic()+timeout
            while time.monotonic()<fin:
                r=f()
                if r:return r
                time.sleep(.05)
            raise AssertionError("Délai dépassé")

        def demarrer():
            nonlocal p
            p=subprocess.Popen([str(exe.resolve())],env=env,stdout=log,stderr=log)
            def pret():
                try:return commande("etat")["ok"]
                except OSError:return False
            attendre(pret,45)

        def saisie(entrees):
            commande("saisie",entrees)
            attendre(lambda:commande("etat")["saisies_en_attente"]==0)

        def capture(nom,zone="fenetre"):
            commande("capture",f"{zone} {captures/nom}")

        try:
            demarrer()
            commande("nouveau",{"chemin":str(root/"Angle.modele.json")})
            ref=commande("mur","4000 -3000 4000 -1000 2800 300")["mur"]
            id=commande("mur","0 0 2000 0 2800 300")["mur"]
            commande("camera","cadrer")
            def point(id,t):
                commande("selection_native",[id]);a=commande("axe_selection")
                return [a["debut_ecran"][i]*(1-t)+a["fin_ecran"][i]*t for i in range(2)]
            rp,pt=point(ref,.85),point(id,.85)
            commande("outil","ajuster");commande("ajuster_angle","oui")
            def clic(p):saisie([{"action":"clic","x":p[0],"y":p[1]}])
            avant=commande("projet_natif")
            clic(rp)
            saisie([{"action":"pointer","x":pt[0],"y":pt[1]}])
            a=commande("ajuster_etat")
            assert a["apercu"] and a["angle"],a
            assert commande("projet_natif")["empreinte"]==avant["empreinte"]
            clic(pt)
            apres=commande("projet_natif")
            murs=lambda p:{m["id"]:m for m in p["projet"]["murs"]}
            m0,m1=murs(avant),murs(apres)
            assert {i for i in m0 if m0[i]!=m1[i]}=={ref,id},(m0,m1)
            assert m1[ref]["axe"]["fin"]==m1[id]["axe"]["fin"],m1
            commande("annuler")
            assert commande("projet_natif")["empreinte"]==avant["empreinte"]
            commande("retablir")
            assert commande("projet_natif")["empreinte"]==apres["empreinte"]
            commande("outil","aucun")
            # Les deux palettes doivent être de vraies fenêtres natives.
            commande("panneau","proprietes externe")
            commande("panneau","arborescence externe")
            attendre(lambda:all(v["fenetre_native"] and v["rectangle"] for v in commande("volets_etat")["volets"]))
            assert commande("projet_natif")["empreinte"]==apres["empreinte"]
            commande("panneau","proprietes fermer")
            attendre(lambda:commande("volets_etat")["volets"][0]["ancrage"]=="gauche")
            assert commande("projet_natif")["empreinte"]==apres["empreinte"]
            commande("volets_retablir")
            attendre(lambda:all(not v["fenetre_native"] for v in commande("volets_etat")["volets"]))
            # Glisser le titre suffit : le volet suit le pointeur hors de l'application.
            h=commande("volets_etat")["volets"][0]["entete"]
            x,y=(h[0]+h[2])/2,(h[1]+h[3])/2
            saisie([{"action":"bouton","x":x,"y":y,"bouton":"gauche","appuye":True},{"action":"pointer","x":x+35,"y":y+15}])
            attendre(lambda:commande("volets_etat")["volets"][0]["fenetre_native"])
            saisie([{"action":"pointer","x":-90,"y":y+80},{"action":"bouton","x":-90,"y":y+80,"bouton":"gauche","appuye":False}])
            v=commande("volets_etat")["volets"][0]
            assert v["ancrage"]=="externe" and v["fenetre_native"],v
            assert commande("projet_natif")["empreinte"]==apres["empreinte"]
            commande("volets_retablir")
            attendre(lambda:all(not v["fenetre_native"] for v in commande("volets_etat")["volets"]))
            # Relâcher sur un bord intérieur réancre, sans menu.
            h=commande("volets_etat")["volets"][0]["entete"]
            x,y=(h[0]+h[2])/2,(h[1]+h[3])/2
            saisie([{"action":"bouton","x":x,"y":y,"bouton":"gauche","appuye":True},{"action":"pointer","x":x+35,"y":y+15},{"action":"pointer","x":20,"y":y+80},{"action":"bouton","x":20,"y":y+80,"bouton":"gauche","appuye":False}])
            attendre(lambda:commande("volets_etat")["volets"][0]["ancrage"]=="gauche")
            # Double-clic central : toute la vue, même sur un objet et avec un outil actif.
            commande("camera","cadrer")
            cadrage=commande("etat")["camera"]
            x,y=point(id,.4)
            for outil in ["aucun","mur"]:
                commande("outil",outil)
                commande("zoom","3")
                assert commande("etat")["camera"]["distance"]!=cadrage["distance"]
                time.sleep(.4)
                saisie([{"action":"clic","x":x,"y":y,"bouton":"milieu"},{"action":"clic","x":x,"y":y,"bouton":"milieu"}])
                assert commande("etat")["camera"]==cadrage
            commande("outil","aucun")
            # Navigation réelle à la souris dans chacune des vues de menuiserie.
            commande("famille_nouvelle","fenetre")
            f0=commande("famille_document")["fichier"]
            etats={}
            for vue in ["elevation","plan","coupe"]:
                commande("famille_navigation",vue)
                n=commande("famille_navigation")
                assert n["zoom"]==1.0 and n["decalage"]==[0.0,0.0],n
                x0,y0,x1,y1=n["rectangle"];x,y=(x0+x1)/2,(y0+y1)/2
                saisie([{"action":"defiler","x":x+40,"y":y,"dx":0,"dy":200}])
                attendre(lambda:commande("famille_navigation")["zoom"]>1.05)
                time.sleep(.3)
                n=commande("famille_navigation")
                saisie([{"action":"bouton","x":x,"y":y,"bouton":"milieu","appuye":True},{"action":"pointer","x":x+50,"y":y+30},{"action":"bouton","x":x+50,"y":y+30,"bouton":"milieu","appuye":False}])
                suivant=commande("famille_navigation")
                assert suivant["decalage"]!=n["decalage"],(n,suivant)
                etats[vue]=suivant
                assert commande("famille_document")["fichier"]==f0,"La navigation a modifié la famille"
            for vue,n in etats.items():
                commande("famille_navigation",vue)
                actuel=commande("famille_navigation")
                assert actuel["zoom"]==n["zoom"] and actuel["decalage"]==n["decalage"]
            saisie([{"action":"touche","touche":"F"}])
            n=commande("famille_navigation")
            assert n["zoom"]==1.0 and n["decalage"]==[0.,0.],n
            # Le même geste réinitialise aussi le cadrage des vues de menuiserie.
            for vue in ["elevation","plan","coupe"]:
                commande("famille_navigation",vue)
                n=commande("famille_navigation")
                x0,y0,x1,y1=n["rectangle"];x,y=(x0+x1)/2,(y0+y1)/2
                saisie([{"action":"defiler","x":x,"y":y,"dx":0,"dy":200}])
                attendre(lambda:commande("famille_navigation")["zoom"]>1.05)
                time.sleep(.3)
                time.sleep(.4)
                saisie([{"action":"clic","x":x,"y":y,"bouton":"milieu"},{"action":"clic","x":x,"y":y,"bouton":"milieu"}])
                n=commande("famille_navigation")
                assert n["zoom"]==1.0 and n["decalage"]==[0.,0.],n
            assert commande("famille_document")["fichier"]==f0
            commande("famille_fermer","abandonner")
            commande("quitter","abandonner");p.wait(timeout=15)
            print(f"Angle, fenêtres natives et navigation menuiseries : {nombre} requêtes validées")
        except Exception:
            log.flush()
            print((root/"instance.log").read_text(encoding="utf-8")[-3500:])
            raise
        finally:
            if p is not None and p.poll() is None:p.terminate();p.wait(timeout=10)
            log.close()

if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--exe",type=Path,default=Path("target/ajuster-prolonger/release/bimedit.exe"))
    args=parser.parse_args();run(args.exe,Path("exports/verification-angle-volets"))
