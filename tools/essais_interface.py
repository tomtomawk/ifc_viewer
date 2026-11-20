"""Lance les essais d'interface par clics simulés (`tools/test*.py`) sur un exécutable,
un par un, avec un délai par essai ; résumé et code de sortie non nul si un essai échoue.

Sous Linux sans écran : xvfb-run -a -s "-screen 0 3200x1000x24" python3 tools/essais_interface.py --exe target/debug/bimedit
Les essais tournent fenêtre visible (BIMEDIT_ARRIERE_PLAN=0) sauf --arriere-plan.
"""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import time

DOSSIER = Path(__file__).resolve().parent


def arguments(script: Path, exe: Path) -> list[str]:
    """Les essais prennent l'exécutable par --exe (argparse) ou en premier argument."""
    texte = script.read_text(encoding="utf-8")
    return ["--exe", str(exe)] if '"--exe"' in texte else [str(exe)]


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--exe", type=Path, required=True)
    p.add_argument("--delai", type=int, default=600, help="secondes par essai")
    p.add_argument("--arriere-plan", action="store_true", help="fenêtre hors écran (BIMEDIT_ARRIERE_PLAN=1)")
    p.add_argument("essais", nargs="*", help="noms d'essais (défaut : tous)")
    a = p.parse_args()
    exe = a.exe.resolve()
    scripts = sorted(DOSSIER.glob("test*.py"))
    if a.essais:
        scripts = [s for s in scripts if any(e in s.stem for e in a.essais)]
    env = dict(os.environ, BIMEDIT_ARRIERE_PLAN="1" if a.arriere_plan else "0")
    resultats = []
    for s in scripts:
        t = time.monotonic()
        try:
            r = subprocess.run([sys.executable, str(s), *arguments(s, exe)], env=env, timeout=a.delai, capture_output=True, text=True)
            ok, sortie = r.returncode == 0, (r.stdout + r.stderr)[-3000:]
        except subprocess.TimeoutExpired as e:
            ok, sortie = False, f"délai de {a.delai} s dépassé\n{(e.stdout or b'')[-2000:]!r}"
        duree = time.monotonic() - t
        resultats.append((s.stem, ok, duree))
        print(f"{'OK  ' if ok else 'ÉCHEC'} {s.stem} ({duree:.0f} s)", flush=True)
        if not ok:
            print("    " + sortie.replace("\n", "\n    "), flush=True)
    echecs = [n for n, ok, _ in resultats if not ok]
    print(f"\n{len(resultats) - len(echecs)}/{len(resultats)} essais réussis" + (f" ; échecs : {', '.join(echecs)}" if echecs else ""))
    return 1 if echecs else 0


if __name__ == "__main__":
    sys.exit(main())
