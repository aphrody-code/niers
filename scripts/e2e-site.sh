#!/usr/bin/env bash
# e2e-site.sh — suite de bout en bout d'Aphrody : le VRAI binaire, le VRAI bundle, le VRAI VFS.
#
# ## Pourquoi cette suite existe
#
# `cargo test -p nie-site` exerce le routeur en mémoire, avec un index injecté et un bundle
# fictif. C'est nécessaire et ça ne suffit pas : le 2026-09-05, cette suite-là était verte
# pendant que le site servait une page d'accueil SANS UNE LIGNE de JavaScript — la recherche du
# point d'entrée regardait `dist/assets/` quand Vite écrit dans `dist/static/`. Aucun test en
# mémoire ne pouvait le voir, parce qu'aucun ne partait du binaire ni du bundle réels.
#
# Ici, tout est réel : le bundle est construit, le binaire est lancé, et chaque assertion COMPTE
# (nombre d'entrées, code HTTP, octets, en-têtes). Un contrôle qui ne compte pas ne prouve rien.
#
# ## Usage
#
#   scripts/e2e-site.sh                 # construit ce qu'il faut, teste, rend un rapport chiffré
#   scripts/e2e-site.sh --no-build      # réutilise le bundle et le binaire déjà construits
#   scripts/e2e-site.sh --vfs 500       # tire 500 chemins VFS au lieu de 200
#
# Sortie non nulle dès qu'un compte passe sous son seuil. Le rapport final liste chaque
# vérification avec sa valeur mesurée, pour qu'un « vert » soit lisible et rejouable.

set -Eeuo pipefail

RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RACINE"

BUILD=1
ECHANTILLON_VFS=200
for arg in "$@"; do
	case "$arg" in
	--no-build) BUILD=0 ;;
	--vfs)
		shift
		ECHANTILLON_VFS="${1:-200}"
		;;
	--vfs=*) ECHANTILLON_VFS="${arg#*=}" ;;
	esac
done

# Port éphémère : la suite doit pouvoir tourner pendant qu'un `nie-site` de développement écoute.
PORT="$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')"
BASE="http://127.0.0.1:$PORT"
BUNDLE="apps/nie-web/dist"
BINAIRE="target/release/nie-site"
JOURNAL="$(mktemp -t nie-site-e2e-XXXXXX.log)"
PID=""

# Compteurs du rapport. `ECHECS` pilote le code de sortie.
declare -i VERIFS=0 ECHECS=0
declare -a LIGNES=()

couleur() { [ -t 1 ] && printf '\033[%sm%s\033[0m' "$1" "$2" || printf '%s' "$2"; }

# `verifier <intitulé> <attendu> <obtenu>` — égalité stricte, et la valeur obtenue est TOUJOURS
# rapportée, y compris au vert : c'est elle qui rend la preuve relisible six mois plus tard.
verifier() {
	local intitule="$1" attendu="$2" obtenu="$3"
	VERIFS+=1
	if [ "$attendu" = "$obtenu" ]; then
		LIGNES+=("  $(couleur '32' 'ok')    $intitule = $obtenu")
	else
		ECHECS+=1
		LIGNES+=("  $(couleur '31' 'ECHEC') $intitule : attendu $attendu, obtenu $obtenu")
	fi
}

# `au_moins <intitulé> <seuil> <obtenu>` — pour les comptes qui dépendent des données présentes.
au_moins() {
	local intitule="$1" seuil="$2" obtenu="$3"
	VERIFS+=1
	if [ "${obtenu:-0}" -ge "$seuil" ] 2>/dev/null; then
		LIGNES+=("  $(couleur '32' 'ok')    $intitule = $obtenu (seuil $seuil)")
	else
		ECHECS+=1
		LIGNES+=("  $(couleur '31' 'ECHEC') $intitule = ${obtenu:-<vide>}, seuil $seuil")
	fi
}

# Un saut est ANNONCÉ, jamais silencieux : une suite qui se tait sur ce qu'elle n'a pas exécuté
# rend un faux vert. Les sauts n'échouent pas, mais ils sont comptés et affichés.
declare -i SAUTS=0
sauter() {
	SAUTS+=1
	LIGNES+=("  $(couleur '33' 'saute') $1")
}

nettoyer() {
	local code=$?
	if [ -n "$PID" ] && kill -0 "$PID" 2>/dev/null; then
		# PID explicite : `pkill -f` tuerait la session Claude sur cette machine.
		kill "$PID" 2>/dev/null || true
		wait "$PID" 2>/dev/null || true
	fi
	[ $code -ne 0 ] && [ -s "$JOURNAL" ] && {
		echo "--- journal du serveur ---" >&2
		tail -30 "$JOURNAL" >&2
	}
	rm -f "$JOURNAL"
	return $code
}
trap nettoyer EXIT

# `req <chemin> [entêtes curl...]` — rend « <code> <octets> » et écrit le corps dans $CORPS.
CORPS="$(mktemp -t nie-site-corps-XXXXXX)"
trap 'rm -f "$CORPS"' EXIT
req() {
	local chemin="$1"
	shift
	curl -sS -o "$CORPS" -w '%{http_code} %{size_download}' "$@" "$BASE$chemin"
}
entete() {
	local chemin="$1" nom="$2"
	shift 2
	curl -sS -o /dev/null -D - "$@" "$BASE$chemin" | tr -d '\r' | awk -F': ' -v n="$(echo "$nom" | tr 'A-Z' 'a-z')" \
		'tolower($1) == n { sub(/^[^:]*: /, ""); print; exit }'
}

echo "▸ Aphrody — suite de bout en bout (port $PORT)"

# --- 1. Construire ce qui sera servi ---------------------------------------------------------
if [ "$BUILD" -eq 1 ]; then
	echo "  [1/4] bundle apps/nie-web…"
	(cd apps/nie-web && bun run build) >"$JOURNAL" 2>&1 ||
		{ echo "ERREUR: le bundle n'a pas été construit." >&2; exit 1; }
	echo "  [2/4] binaire nie-site (release)…"
	cargo build --release -p nie-site >>"$JOURNAL" 2>&1 ||
		{ echo "ERREUR: le binaire n'a pas été construit." >&2; exit 1; }
else
	echo "  [1-2/4] --no-build : bundle et binaire réutilisés"
fi

[ -x "$BINAIRE" ] || { echo "ERREUR: $BINAIRE absent. Lancer sans --no-build." >&2; exit 1; }
[ -f "$BUNDLE/index.html" ] || { echo "ERREUR: $BUNDLE/index.html absent." >&2; exit 1; }

# --- 2. Démarrer le serveur ------------------------------------------------------------------
echo "  [3/4] démarrage…"
NIE_SITE_ADDR="127.0.0.1:$PORT" NIE_GAME_DIR="${NIE_GAME_DIR:-$RACINE}" \
	"$BINAIRE" >>"$JOURNAL" 2>&1 &
PID=$!

# Le VFS s'indexe en tâche de fond ; `/healthz` doit répondre AVANT lui, c'est une propriété
# du service et le premier point vérifié.
declare -i attente=0
until curl -sS -o /dev/null "$BASE/healthz" 2>/dev/null; do
	attente+=1
	[ "$attente" -gt 100 ] && { echo "ERREUR: pas de réponse sur /healthz après 10 s." >&2; exit 1; }
	kill -0 "$PID" 2>/dev/null || { echo "ERREUR: le serveur s'est arrêté au démarrage." >&2; exit 1; }
	sleep 0.1
done
LIGNES+=("  $(couleur '32' 'ok')    /healthz répond après ${attente}00 ms, VFS encore en indexation")

echo "  [4/4] vérifications…"

# --- 3. Santé et capacités -------------------------------------------------------------------
verifier "GET /healthz" "200 " "$(req /healthz | cut -d' ' -f1) "
read -r code _ <<<"$(req /api/v1/health)"
verifier "GET /api/v1/health" "200" "$code"
SANTE="$(cat "$CORPS")"
verifier "service annoncé" "nie-site" "$(jq -r '.service' <<<"$SANTE")"
verifier "version d'API" "v1" "$(jq -r '.api' <<<"$SANTE")"
verifier "bundle vu par le serveur" "true" "$(jq -r '.capacites.bundle' <<<"$SANTE")"

# L'index du VFS est asynchrone : on lui laisse le temps d'aboutir avant de compter les vues.
declare -i tours=0
while [ "$(jq -r '.capacites.vfs' <<<"$SANTE")" = "en_cours" ] && [ "$tours" -lt 300 ]; do
	sleep 0.2
	tours+=1
	req /api/v1/health >/dev/null
	SANTE="$(cat "$CORPS")"
done
ETAT_VFS="$(jq -r '.capacites.vfs' <<<"$SANTE")"
VFS_ENTREES="$(jq -r '.capacites.vfs_entrees' <<<"$SANTE")"

# --- 4. La coquille charge RÉELLEMENT le bundle -----------------------------------------------
# C'est le contrôle qui manquait : une page valide, en 200, et sans application.
req / >/dev/null
PAGE="$(cat "$CORPS")"
SCRIPT_SRC="$(grep -o 'src="/[^"]*\.js"' <<<"$PAGE" | head -1 | sed 's/src="//; s/"$//' || true)"
FEUILLE_SRC="$(grep -o 'href="/[^"]*\.css"' <<<"$PAGE" | head -1 | sed 's/href="//; s/"$//' || true)"
verifier "la coquille référence un point d'entrée JS" "1" "$([ -n "$SCRIPT_SRC" ] && echo 1 || echo 0)"

if [ -n "$SCRIPT_SRC" ]; then
	read -r code taille <<<"$(req "$SCRIPT_SRC")"
	verifier "le JS annoncé est servi ($SCRIPT_SRC)" "200" "$code"
	au_moins "poids du JS servi (octets)" 10000 "$taille"
	verifier "le JS empreinté est immuable" \
		"public, max-age=31536000, immutable" "$(entete "$SCRIPT_SRC" cache-control)"
	# Le fichier servi est bien celui du disque, pas une page d'erreur déguisée en 200.
	verifier "octets servis = octets du bundle" \
		"$(wc -c <"$BUNDLE${SCRIPT_SRC}")" "$taille"
fi
if [ -n "$FEUILLE_SRC" ]; then
	verifier "la feuille annoncée est servie" "200" "$(req "$FEUILLE_SRC" | cut -d' ' -f1)"
else
	sauter "aucune feuille de style dans le bundle (l'interface n'en produit pas encore)"
fi

verifier "l'index n'est PAS figé dans les caches" \
	"public, max-age=60, stale-while-revalidate=600" "$(entete / cache-control)"

# --- 5. En-têtes de sécurité et CSP -----------------------------------------------------------
verifier "CSP posée par la crate sur une page" "1" \
	"$([ -n "$(entete / content-security-policy)" ] && echo 1 || echo 0)"
verifier "CSP posée aussi sur une erreur" "1" \
	"$([ -n "$(entete /route-qui-nexiste-pas content-security-policy)" ] && echo 1 || echo 0)"
verifier "X-Content-Type-Options" "nosniff" "$(entete / x-content-type-options)"

# --- 6. Le service est en LECTURE SEULE --------------------------------------------------------
verifier "POST refusé" "405" "$(req /healthz -X POST | cut -d' ' -f1)"
verifier "PUT refusé" "405" "$(req /healthz -X PUT | cut -d' ' -f1)"
read -r code taille <<<"$(req /healthz -I)"
verifier "HEAD répond 200" "200" "$code"
verifier "HEAD ne rend aucun corps" "0" "$taille"

# --- 7. Aucune sortie de la racine -------------------------------------------------------------
# Quatre formes, dont l'encodée : c'est celle qu'on oublie.
for chemin in "/f/../../etc/passwd" "/f/data/../../../etc/passwd" "/assets/../../etc/passwd" "/f/%2e%2e%2f%2e%2e%2fetc%2fpasswd"; do
	code="$(req "$chemin" | cut -d' ' -f1)"
	VERIFS+=1
	if [ "$code" = "400" ] || [ "$code" = "404" ]; then
		LIGNES+=("  $(couleur '32' 'ok')    traversée refusée ($chemin) = $code")
	else
		ECHECS+=1
		LIGNES+=("  $(couleur '31' 'ECHEC') traversée NON refusée ($chemin) = $code")
	fi
done

# --- 8. Fichiers de référencement --------------------------------------------------------------
for chemin in /robots.txt /.well-known/security.txt /sitemap.xml; do
	verifier "GET $chemin" "200" "$(req "$chemin" | cut -d' ' -f1)"
done

# --- 9. L'API v1 : formes, bornes, comptes -----------------------------------------------------
if [ "$ETAT_VFS" = "pret" ]; then
	au_moins "entrées indexées dans le VFS" 1000 "$VFS_ENTREES"
	for vue in textures modeles sons videos; do
		req "/api/v1/$vue?page=1&per_page=5" >/dev/null
		au_moins "/api/v1/$vue : total" 1 "$(jq -r '.total' <"$CORPS")"
		verifier "/api/v1/$vue : éléments rendus" "5" "$(jq -r '.elements | length' <"$CORPS")"
		# Le chemin d'un élément est aussi son URL sous /f/ : c'est l'amendement A3, et c'est
		# vérifiable plutôt que déclaratif.
		premier="$(jq -r '.elements[0].chemin' <"$CORPS")"
		verifier "/f/<chemin de $vue> est servi" "200" "$(req "/f/$premier" | cut -d' ' -f1)"
	done

	# Pagination bornée : une demande déraisonnable est ramenée à la borne, pas refusée.
	req "/api/v1/textures?page=1&per_page=99999" >/dev/null
	verifier "per_page borné par le serveur" "200" "$(jq -r '.per_page' <"$CORPS")"

	# Un filtre inconnu se distingue d'une ressource absente.
	verifier "/api/v1/<vue inconnue> = 404" "404" "$(req "/api/v1/pas-une-vue" | cut -d' ' -f1)"

	# --- 10. Échantillon VFS : la gate A3 ------------------------------------------------------
	# Des chemins TIRÉS de l'index, sous leur forme exacte, extension du jeu conservée. Un
	# chemin cité de mémoire est presque toujours faux — d'où le tirage plutôt qu'une liste.
	req "/api/v1/textures?page=1&per_page=200" >/dev/null
	mapfile -t CHEMINS < <(jq -r '.elements[].chemin' <"$CORPS" | head -n "$ECHANTILLON_VFS")
	declare -i ok200=0
	for c in "${CHEMINS[@]}"; do
		[ "$(req "/f/$c" -o /dev/null -I | cut -d' ' -f1)" = "200" ] && ok200+=1
	done
	verifier "chemins VFS répondant 200 sur /f/" "${#CHEMINS[@]}" "$ok200"

	# --- 11. ETag et 304 -----------------------------------------------------------------------
	if [ "${#CHEMINS[@]}" -gt 0 ]; then
		etag="$(entete "/f/${CHEMINS[0]}" etag)"
		verifier "ETag présent sur une ressource" "1" "$([ -n "$etag" ] && echo 1 || echo 0)"
		verifier "If-None-Match rend 304" "304" \
			"$(req "/f/${CHEMINS[0]}" -H "If-None-Match: $etag" | cut -d' ' -f1)"
	fi

	# --- 12. Parcours d'un dossier -------------------------------------------------------------
	req "/b/data" >/dev/null
	verifier "/b/<préfixe> rend du JSON" "200" "$(req "/b/data" | cut -d' ' -f1)"
	au_moins "/b/data : sous-dossiers listés" 1 "$(jq -r '(.dossiers // []) | length' <"$CORPS")"
else
	sauter "VFS non monté ($ETAT_VFS) : /f, /b et les quatre vues ne sont pas éprouvés ici"
	sauter "gate A3 (échantillon de $ECHANTILLON_VFS chemins) non exécutée"
fi

# --- 13. Le gisement -------------------------------------------------------------------------
if [ "$(jq -r '.capacites.gisement' <<<"$SANTE")" = "true" ]; then
	req "/api/v1/chara?page=1&per_page=5" >/dev/null
	verifier "/api/v1/chara" "200" "$(req '/api/v1/chara?page=1&per_page=5' | cut -d' ' -f1)"
	verifier "/api/v1/chara : éléments rendus" "5" "$(jq -r '.elements | length' <"$CORPS")"
	au_moins "/api/v1/chara : total" 1000 "$(jq -r '.total' <"$CORPS")"
	# `internal_code` est l'identifiant stable et le seul adressable ; `base_slug` ne l'est pas
	# (6 168 lignes pour 5 199 valeurs distinctes). On vérifie que le premier est bien là.
	verifier "chara : internal_code présent" "5" \
		"$(jq -r '[.elements[] | select(.internal_code != null)] | length' <"$CORPS")"
else
	sauter "miroir SQLite absent : /api/v1/chara n'est pas éprouvé ici"
fi

# --- 14. Distinction des espaces sur une route inconnue ---------------------------------------
verifier "route inconnue sous /api = JSON" "application/json" \
	"$(entete /api/v1/inconnu content-type | cut -d';' -f1)"
verifier "route de navigation inconnue = coquille HTML" "text/html" \
	"$(entete /une/page/inconnue content-type | cut -d';' -f1)"

# --- 15. Le proxy d'assets borne son amont ----------------------------------------------------
# `nie-model-serve` peut être injoignable ou figé ; ce qui se vérifie ici, c'est que le proxy ne
# pend pas indéfiniment et n'expose aucune adresse interne.
debut="$(date +%s)"
code_assets="$(curl -sS -o "$CORPS" -w '%{http_code}' --max-time 15 "$BASE/assets/inexistant.png" || echo 000)"
duree=$(($(date +%s) - debut))
VERIFS+=1
if [ "$duree" -le 12 ]; then
	LIGNES+=("  $(couleur '32' 'ok')    le proxy borne son amont = ${duree} s (code $code_assets)")
else
	ECHECS+=1
	LIGNES+=("  $(couleur '31' 'ECHEC') le proxy a mis ${duree} s, borne attendue 10 s")
fi
VERIFS+=1
if grep -q '127\.0\.0\.1\|8790' "$CORPS" 2>/dev/null; then
	ECHECS+=1
	LIGNES+=("  $(couleur '31' 'ECHEC') le message d'erreur du proxy divulgue l'adresse de l'amont")
else
	LIGNES+=("  $(couleur '32' 'ok')    le proxy ne divulgue pas son amont")
fi

# --- Rapport ----------------------------------------------------------------------------------
echo
echo "── Aphrody, bout en bout ──────────────────────────────────────────"
printf '%s\n' "${LIGNES[@]}"
echo "───────────────────────────────────────────────────────────────────"
echo "  VFS $ETAT_VFS ($VFS_ENTREES entrées) · bundle $(du -sh "$BUNDLE" | cut -f1)"
echo "  $VERIFS vérifications, $ECHECS échec(s), $SAUTS saut(s) annoncé(s)"

[ "$ECHECS" -eq 0 ] || exit 1
[ "$VERIFS" -gt 0 ] || { echo "ERREUR: aucune vérification exécutée — une suite muette n'est pas une suite verte." >&2; exit 1; }
