#!/usr/bin/env bash
# SessionStart — injecte l'etat MESURE du depot dans le contexte de la session.
# Regle : aucun chiffre cite de memoire. Ce que ce script n'arrive pas a mesurer,
# il l'annonce « indisponible » plutot que de le taire.
# Doit rester sous ~3 s et ne jamais echouer (exit 0 quoi qu'il arrive).

set -u
# La racine se DEDUIT, elle ne se suppose pas. Ce hook se repliait sur /home/ubuntu/niers
# en dur : sous Codex, ou CLAUDE_PROJECT_DIR n'existe pas, le `cd` echouait et le `|| exit 0`
# rendait 0 SANS UNE LIGNE de sortie — le hook d'etat se taisait a chaque session, et rien
# ne le disait. Mesure le 2026-09-06 sur le poste Windows : 0 ligne, exit 0.
RACINE="${CLAUDE_PROJECT_DIR:-${CODEX_PROJECT_DIR:-}}"
[ -n "$RACINE" ] || RACINE="$(git rev-parse --show-toplevel 2>/dev/null)"
[ -n "$RACINE" ] || RACINE="$PWD"
if ! cd "$RACINE" 2>/dev/null; then
  echo "=== etat du depot : INDISPONIBLE — racine introuvable ($RACINE) ==="
  exit 0
fi
q() { timeout 5 sqlite3 -noheader -separator ' ' "$@" 2>/dev/null; }
KB=var/niers.sqlite

echo "=== etat mesure du depot (hook SessionStart, $(date '+%F %H:%M')) ==="

# --- plateforme -------------------------------------------------------------
# `free` n'existe pas sous MSYS : sans repli, la ligne affichait «  Gio libres », un trou muet
# la ou ce hook s'interdit justement de taire ce qu'il ne mesure pas.
MEM="$(free -g 2>/dev/null | awk '/^Mem:/{print $7" Gio libres"}')"
[ -n "$MEM" ] || MEM="RAM libre indisponible"
echo "machine   $(uname -sm) — $(nproc) coeurs, $MEM, disque $(df -h --output=pcent . | tail -1 | tr -d ' ') plein"
# La plateforme se MESURE : ce hook a longtemps affirme « VPS Linux » en dur, y compris sous
# Git Bash (uname rend MINGW64_NT), ou il annoncait l'inverse de la verite a chaque session.
# Troisieme cas, ajoute le 2026-09-06 : WSL. Depuis PowerShell — donc depuis Codex, dont
# c'est le shell par defaut sous Windows — `bash` ne resout PAS Git Bash mais
# C:\Windows\system32\bash.exe, c'est-a-dire WSL. `uname -s` y rend « Linux » et ce hook
# annoncait « CETTE machine est le VPS Linux » sur le poste Windows : le mensonge exact que
# CLAUDE.md § Deux machines reproche a la version en dur. Mesure : `bash -c pwd` rend
# /mnt/c/Users/aphro/niers, et non C:\Users\aphro\niers.
if [ "$(uname -s)" = "Linux" ] && grep -qi microsoft /proc/version 2>/dev/null; then
  echo "          CETTE machine est le poste Windows, vu au travers de WSL (pas le VPS)."
  echo "          Le depot est monte sur /mnt/c : les binaires .exe, MSVC et le jeu Steam sont"
  echo "          ceux de Windows, mais ce shell ne les voit pas comme Windows les voit."
  echo "          Pour mesurer le poste tel qu'il est, utiliser PowerShell ou Git Bash."
elif [ "$(uname -s)" = "Linux" ]; then
  echo "          CETTE machine est le VPS Linux. Les sections Windows de CLAUDE.md (MSVC, Git Bash,"
  echo "          MSYS, UAC, .exe, sed -i, cargo fmt --all) ne s'appliquent PAS ici."
else
  echo "          CETTE machine est le poste Windows (Git Bash/MSYS). Les pieges Windows de"
  echo "          CLAUDE.md (MSVC, UAC, .exe, sed -i, cargo fmt --all, verrou DLL) s'appliquent TOUS."
fi

# --- git --------------------------------------------------------------------
if git rev-parse --git-dir >/dev/null 2>&1; then
  echo "git       $(git branch --show-current) @ $(git log -1 --format='%h %s' | cut -c1-72)"
  echo "          $(git status --porcelain | wc -l) fichiers modifies, $(git stash list | wc -l) remises"
fi

# --- binaire de reference ---------------------------------------------------
if [ -e nie.exe ]; then
  taille=$(stat -Lc%s nie.exe 2>/dev/null)
  sha=$(timeout 5 sha256sum -b nie.exe 2>/dev/null | cut -c1-12)
  echo "cible RE  nie.exe $taille o, sha $sha…"
  [ "$taille" = "33918464" ] && echo "          = la cible documentee (b1fa04ea3658…)." \
    || echo "          ATTENTION : ce n'est PAS la cible documentee (33 918 464 o) — toute mesure citee est suspecte."
else
  echo "cible RE  nie.exe indisponible"
fi

# --- base de connaissance ---------------------------------------------------
if [ -f "$KB" ]; then
  anc=$(q "$KB" "select substr(sha256,1,12) from binary where id=2;")
  cov=$(q "$KB" "select total_funcs||' fonctions, '||named||' nommees ('||round(named*100.0/total_funcs,2)||'%), '||classified||' classifiees ('||round(pct,2)||'%)' from coverage where binary_id=2 order by id desc limit 1;")
  roots=$(q "$KB" "select count(*) from pdata_func;")
  echo "KB        ${cov:-indisponible}"
  echo "          racines .pdata=${roots:-?} — ancrage binaire id=2 : ${anc:-?}…"
  [ "${anc:0:8}" = "4c2b91fb" ] && echo "          CONTRADICTION : la KB est ancree sur le build TRANSITOIRE (31 468 032 o), pas sur nie.exe local. Ne pas citer ses chiffres comme mesures de la cible."
else
  echo "KB        $KB absent"
fi

# --- forge ------------------------------------------------------------------
if [ -x target/release/nie-forge ]; then
  if [ -f var/forge/cover.json ]; then
    r=$(timeout 20 target/release/nie-forge report 2>/dev/null | grep -Ei '%' | head -2 | tr '\n' ' ')
    echo "forge     ${r:-report muet}"
  else
    echo "forge     var/forge/ absent — aucune mesure ici. 'just forge' (split→lift→cc→build→verify→report) la reconstruit."
  fi
  [ -f dist/nie.exe ] && echo "          dist/nie.exe present ($(stat -Lc%s dist/nie.exe) o)"
else
  echo "forge     target/release/nie-forge non construit"
fi

# --- gisements --------------------------------------------------------------
g=""
for f in var/mirror.sqlite data/anime/episodes.db data/cpk_list.cfg.bin; do
  if [ -e "$f" ]; then g="$g $(basename "$f")=$(du -Lsh "$f" 2>/dev/null | cut -f1)"; else g="$g $(basename "$f")=absent"; fi
done
echo "gisements$g   (facade : packages/nie-catalog/src/cli.ts etat)"
echo "VFS       NIE_GAME_DIR=${NIE_GAME_DIR:-non posee}"

# --- services ---------------------------------------------------------------
s=$(timeout 5 systemctl list-units --type=service --state=running --no-legend --no-pager 2>/dev/null \
    | awk '{print $1}' | grep -E '(azalee|niers|nie-|rg-|bxc|cdn)' | sed 's/\.service$//' | tr '\n' ' ')
echo "services  ${s:-aucun service du projet en cours}"
f=$(timeout 5 systemctl list-units --type=service --state=failed --no-legend --no-pager 2>/dev/null \
    | awk '{print $1}' | sed 's/\.service$//' | tr '\n' ' ')
[ -n "$f" ] && echo "EN ECHEC  $f"

# --- binaires deja construits ----------------------------------------------
b=$(ls target/release 2>/dev/null | grep -Ev '\.(d|rlib|so|a)$|^(build|deps|examples|incremental|\.)' | tr '\n' ' ')
echo "binaires  ${b:-aucun}"
echo "          les lancer plutot que rebuild ; verification = cargo clippy (jamais build --workspace --all-targets)."
exit 0
