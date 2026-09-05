"""Retire d'un dump `pg_dump` ce qui ne peut pas s'appliquer sur Supabase Cloud.

Appelé par `migrer-editorial-vers-cloud.sh`. Deux catégories, et une seule raison commune :
elles pointent vers des objets qui n'existent que sur le VPS.

  1. **Les clés étrangères vers `auth.users`.** `auth.users` n'est pas migré — c'est une
     décision du PLAN, pas un oubli. La contrainte pointerait dans le vide, et `psql` avec
     `ON_ERROR_STOP` abandonne alors le fichier entier : une seule ligne fait perdre le schéma.
     La colonne, elle, est conservée : c'est la contrainte qui saute, pas la donnée.

  2. **Les déclencheurs vers `rg_realtime_notify()`.** C'est la fonction du realtime
     auto-hébergé du VPS (`rg-realtime.service`). Supabase Cloud a le sien ; le déclencheur
     n'a aucun objet ici et sa fonction n'existe pas.

Le script réécrit les fichiers en place et dit ce qu'il a retiré. Il est idempotent : rejoué
sur un fichier déjà nettoyé, il ne retire rien et le signale.

    scripts/ops/_nettoyer-dump.py var/tmp/schema-editorial.sql var/tmp/schema-profiles.sql
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


def nettoyer(chemin: Path) -> tuple[int, int]:
    """Retire les FK vers `auth.users` et les déclencheurs realtime. Rend `(fk, triggers)`."""
    texte = chemin.read_text(encoding="utf-8")

    # Le déclencheur s'écrit sur plusieurs lignes jusqu'au `;` — le lire ligne à ligne en
    # laisserait la queue, qui deviendrait une instruction orpheline.
    texte, triggers = re.subn(
        r"CREATE TRIGGER \w*rg_realtime\w*[^;]*;\n?", "", texte, flags=re.S
    )

    # La FK s'écrit en DEUX lignes : `ALTER TABLE ONLY x` puis `    ADD CONSTRAINT … ;`.
    # Retirer la seconde seule laisse un `ALTER TABLE ONLY` sans verbe, donc une erreur de
    # syntaxe — c'est le « syntax error at or near ADD » payé le 2026-09-05, dans l'autre sens.
    lignes = texte.splitlines(keepends=True)
    sortie: list[str] = []
    fk = 0
    for ligne in lignes:
        if "REFERENCES auth.users" in ligne:
            if sortie and sortie[-1].lstrip().startswith("ALTER TABLE ONLY"):
                sortie.pop()
            fk += 1
            continue
        sortie.append(ligne)

    chemin.write_text("".join(sortie), encoding="utf-8")
    return fk, triggers


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    for arg in argv[1:]:
        p = Path(arg)
        if not p.is_file():
            print(f"  {arg} : absent", file=sys.stderr)
            return 2
        fk, tr = nettoyer(p)
        detail = []
        if fk:
            detail.append(f"{fk} FK vers auth.users")
        if tr:
            detail.append(f"{tr} déclencheur(s) realtime")
        print(f"  {p.name} : {', '.join(detail) if detail else 'rien à retirer'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
