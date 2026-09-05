#!/usr/bin/env bash
# Porte vers Supabase Cloud les tables du domaine EDITORIAL du wiki.
#
# POURQUOI CE SCRIPT EXISTE
# La migration vers Cloud avait porte les 224 tables `inagle_*` — les donnees du jeu — et le
# PLAN annoncait « 65 tables / 165 277 lignes, 0 ecart ». Mesure le 2026-09-05 au soir :
#
#     select count(*) from information_schema.tables where table_schema='public'
#       -> 224, dont 224 `inagle_*` et ZERO autre
#
# Les tables que le domaine editorial interroge etaient donc TOUTES absentes. Le build de
# `apps/azalee` s'arretait au prerendu de `/` sur « Connection terminated due to connection
# timeout », et l'on croyait a un probleme de `DATABASE_URL` : c'etait un schema a moitie
# migre. Sur le VPS rien ne se voyait, puisque `127.0.0.1` repondait.
#
# CE QU'IL MIGRE, ET CE QU'IL NE MIGRE PAS
# Le wiki n'interroge que CINQ tables hors `inagle_*` sur son chemin Postgres direct
# (mesure en lisant les requetes SQL, pas les imports) : `articles`, `tweets`, `profiles`,
# `article_series`, `patch_notes`.
#
#   * quatre sont migrees avec leurs donnees ;
#   * `profiles` est creee VIDE, deliberement. Elle porte 1 821 profils d'utilisateurs :
#     deplacer des donnees personnelles vers une autre base n'est pas une decision technique.
#     La table vide suffit — `is_admin()` la lit, rend `false`, donc l'administration reste
#     fermee et la lecture publique fonctionne. C'est le bon defaut, pas un contournement.
#
# TROIS PIEGES PAYES ICI
#   1. Les FK vers `auth.users` font echouer l'application entiere du schema : `auth.users`
#      n'est pas migre (decision du PLAN), la contrainte pointerait dans le vide.
#   2. `profiles` porte un trigger vers `rg_realtime_notify()`, la fonction du realtime
#      AUTO-HEBERGE du VPS. Supabase Cloud a le sien : le trigger n'a pas d'objet.
#   3. Une table en RLS activee SANS policy refuse tout, y compris au service. Les tables
#      arrivent dans cet etat et restent inutilisables tant que les policies ne sont pas
#      posees — sans le moindre message.
#
# Idempotent : les tables existantes ne sont pas recreees, les donnees sont remplacees.
#
#   scripts/ops/migrer-editorial-vers-cloud.sh            # migre puis compte
#   scripts/ops/migrer-editorial-vers-cloud.sh --compter  # compte seulement
set -uo pipefail

racine="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
travail="$racine/var/tmp"
mkdir -p "$travail"

PROJET="${SUPABASE_PROJET:-kvnlbhatjqqmhhxaxlbi}"
HOTE="${SUPABASE_POOLER:-aws-1-eu-west-3.pooler.supabase.com}"
# Port 5432 = mode SESSION. Le 6543 est le mode transaction, fait pour des requetes courtes
# de fonctions serverless : le DDL n'y passe pas.
PORT_DDL=5432
UTIL="postgres.$PROJET"

# shellcheck source=/dev/null
[ -f "$HOME/.config/niers/supabase.env" ] && { set -a; . "$HOME/.config/niers/supabase.env"; set +a; }
: "${SUPABASE_DB_PASSWORD:?SUPABASE_DB_PASSWORD absent (~/.config/niers/supabase.env)}"
export PGPASSWORD="$SUPABASE_DB_PASSWORD"

LOCALE="${DATABASE_URL_LOCAL:-$(grep -oE '^DATABASE_URL=.*' "$racine/apps/azalee/.env.local" 2>/dev/null | head -1 | sed 's/^DATABASE_URL=//' | tr -d '"'"'"'')}"
[ -n "$LOCALE" ] || { echo "DATABASE_URL locale introuvable" >&2; exit 2; }

AVEC_DONNEES=(article_series articles patch_notes tweets)
SANS_DONNEES=(profiles)
TOUTES=("${AVEC_DONNEES[@]}" "${SANS_DONNEES[@]}")

cloud() { psql -h "$HOTE" -p "$PORT_DDL" -U "$UTIL" -d postgres "$@"; }

compter() {
	echo
	printf '  %-16s %8s %8s %8s   %s\n' TABLE LOCAL CLOUD ECART POLICIES
	local souci=0
	for t in "${TOUTES[@]}"; do
		local l c p ecart
		l=$(psql "$LOCALE" -t -A -c "select count(*) from \"$t\"" 2>/dev/null || echo "?")
		c=$(cloud -t -A -c "select count(*) from \"$t\"" 2>/dev/null || echo "?")
		p=$(cloud -t -A -c "select count(*) from pg_policy p join pg_class k on k.oid=p.polrelid
			join pg_namespace n on n.oid=k.relnamespace where n.nspname='public' and k.relname='$t'" 2>/dev/null || echo "?")
		ecart=$(( ${c:-0} - ${l:-0} ))
		printf '  %-16s %8s %8s %8s   %s\n' "$t" "$l" "$c" "$ecart" "$p"
		# `profiles` DOIT diverger : c'est le seul ecart attendu de cette migration.
		if [ "$t" != profiles ] && [ "$ecart" != 0 ]; then souci=$((souci + 1)); fi
		if [ "${p:-0}" = 0 ]; then
			echo "      ATTENTION : 0 policy, et RLS active refuse alors TOUT, service compris"
			souci=$((souci + 1))
		fi
	done
	return "$souci"
}

if [ "${1:-}" = "--compter" ]; then compter; exit $?; fi

echo "== schema des quatre tables a donnees"
pg_dump "$LOCALE" --schema-only --no-owner --no-acl --no-privileges \
	"${AVEC_DONNEES[@]/#/-t public.}" > "$travail/schema-editorial.sql"

echo "== schema de profiles (sans ses donnees)"
pg_dump "$LOCALE" --schema-only --no-owner --no-acl --no-privileges \
	-t public.profiles > "$travail/schema-profiles.sql"

echo "== retrait des FK vers auth.users et du trigger realtime du VPS"
"$racine/scripts/ops/_nettoyer-dump.py" "$travail/schema-editorial.sql" "$travail/schema-profiles.sql"

echo "== application du schema (les objets deja presents sont ignores)"
for f in schema-editorial schema-profiles; do
	cloud -q -f "$travail/$f.sql" 2>&1 | grep -iE '^psql.*error' | grep -viE 'already exists|unrestrict' | head -3
done

echo "== is_admin(), dont dependent les policies"
psql "$LOCALE" -t -A -c "select pg_get_functiondef(p.oid) from pg_proc p
	join pg_namespace n on n.oid=p.pronamespace where n.nspname='public' and p.proname='is_admin'" \
	> "$travail/is_admin.sql"
cloud -q -f "$travail/is_admin.sql" 2>&1 | grep -iE '^psql.*error' | head -3

echo "== policies"
debut=$(grep -n '^CREATE POLICY' "$travail/schema-editorial.sql" | head -1 | cut -d: -f1)
if [ -n "$debut" ]; then
	sed -n "${debut},\$p" "$travail/schema-editorial.sql" > "$travail/policies.sql"
	cloud -q -f "$travail/policies.sql" 2>&1 | grep -iE '^psql.*error' | grep -vi 'already exists\|unrestrict' | head -3
fi

echo "== donnees (profiles exclue, deliberement)"
pg_dump "$LOCALE" --data-only --no-owner --no-acl \
	"${AVEC_DONNEES[@]/#/-t public.}" > "$travail/donnees-editorial.sql"
cloud -q -f "$travail/donnees-editorial.sql" 2>&1 | grep -iE '^psql.*error' | grep -vi unrestrict | head -5

if compter; then
	echo
	echo "MIGRATION OK — ecart 0 partout, sauf profiles qui reste vide par decision."
	exit 0
fi
echo
echo "MIGRATION INCOMPLETE — voir les lignes ci-dessus."
exit 1
