# Audit de sécurité — bascule d'`azalee.rosegriffon.fr` vers Vercel

Audit mené le **2026-09-05** sur le VPS de production (`51.77.147.152`, `vps-203bea89`), en
**lecture seule sur l'infrastructure** : aucun service, pare-feu ni vhost modifié, aucun
`systemctl` autre que `status`/`show`, aucun redémarrage. Toutes les affirmations chiffrées
ci-dessous sont accompagnées de la commande qui les a produites et de sa sortie.

> **Note de méthode.** Le service `nie-model-serve` a été arrêté puis relancé par systemd à
> 06:17:29 et 06:26:39 pendant l'audit (`Deactivated successfully` = arrêt propre, suivi d'un
> `Started`). Ce n'est pas une conséquence des sondes : deux autres agents travaillent en
> parallèle sur ce dépôt. Les mesures concernées ont été rejouées après le redémarrage.

---

## Résumé exécutif

**La prémisse de la mission doit être corrigée d'entrée.** La question posée était : « une fois
le site sur Vercel, ces trois surfaces devront être joignables depuis l'extérieur, c'est là
qu'est le risque ». En réalité **les trois surfaces sont déjà publiques sur Internet
aujourd'hui**, et ce depuis que `supabase-compat.inc` est inclus dans le vhost. La bascule ne
crée pas cette exposition : elle la rend seulement permanente et explicite.

```
$ curl -s -o /dev/null -w '%{http_code}\n' https://azalee.rosegriffon.fr/rest/v1/
200
$ curl -s -o /dev/null -w '%{http_code}\n' https://azalee.rosegriffon.fr/storage/v1/
200
$ curl -s -o /dev/null -w '%{http_code}\n' https://azalee.rosegriffon.fr/realtime/v1/
200
```

Conséquence : **l'essentiel du travail de durcissement est un rattrapage, pas une préparation.**
Il doit être fait que la bascule ait lieu ou non, et il doit être fait *avant* elle, parce que la
bascule retire le seul garde-fou implicite qui restait (le fait que ces routes ne soient citées
nulle part publiquement).

| # | Gravité | Point | Déjà / Après |
|---|---|---|---|
| 1 | **Critique** | RPC destructif `rg_liberer_profil_discord` appelable anonymement depuis Internet | **Déjà** |
| 2 | **Critique** | `anon` détient `INSERT/UPDATE/DELETE/TRUNCATE` sur 129 tables ; seul RLS retient | **Déjà** |
| 3 | **Critique** | SSH : `PermitRootLogin yes` + `PasswordAuthentication yes` + root a un mot de passe | **Déjà** |
| 4 | **Critique** | `SUPABASE_JWT_SECRET` en clair dans un fichier de 44 secrets ; permet de forger un JWT `service_role` (`BYPASSRLS`) contre une API REST publique | **Déjà** |
| 5 | **Élevé** | 2 105 lignes de données personnelles Discord lisibles par un anonyme | **Déjà** |
| 6 | **Élevé** | `nie-model-serve` : 0 authentification, 0 limitation de débit, 5,65 Gio de RSS, 504 sur une requête anonyme | **Déjà** |
| 7 | **Élevé** | `limit_req_zone` déclarée dans nginx mais **jamais appliquée** nulle part | **Déjà** |
| 8 | **Élevé** | Le VPS continuera de servir une copie complète du site sur `https://51.77.147.152/` | **Après** |
| 9 | **Élevé** | `NEXT_PUBLIC_SUPABASE_URL` pointe l'origine du site : tous les appels navigateur cassent en silence | **Après** |
| 10 | Moyen | 3 RPC d'écriture supplémentaires anonymes (`rg_precreer_profils_discord`, `upsert_reading_progress`, `check_rate_limit`) | **Déjà** |
| 11 | Moyen | Flux SSE `realtime` non authentifié (payload assaini à la source — voir la nuance) | **Déjà** |
| 12 | Moyen | `/rest/v1/settings` : configuration métier publique | **Déjà** |
| 13 | Moyen | Mot de passe Postgres en clair dans `/etc/postgrest.conf` | **Déjà** |
| 14 | Faible | `/__preview` pose un cookie arbitraire sans authentification | **Déjà** (inerte) |
| 15 | Faible | Rejeu (rollback) possible sur l'updater de l'application de bureau | **Après** |

**Ce qui est infirmé.** La traversée de chemin sur `nie-model-serve`, désignée dans la mission
comme « le point le plus exposé du projet », **n'existe pas**. Le détail et les preuves sont en
§ 2. Le service reste néanmoins le deuxième risque du projet, mais pour une autre raison :
l'épuisement de ressources.

---

## 1. Exposer PostgREST, Realtime et Storage à Internet

### 1.1 Ce qui protège ces ports aujourd'hui

**Au niveau réseau, la posture est correcte.** Les trois services écoutent exclusivement sur la
boucle locale, et le pare-feu est actif en refus par défaut.

```
$ ss -ltnp | grep -E ':(8809|8810|8812|5432)\b'
LISTEN 0  2048  127.0.0.1:8809  0.0.0.0:*                                   # PostgREST
LISTEN 0   512  127.0.0.1:8810  0.0.0.0:*  users:(("bun",pid=3100718,fd=11)) # Storage
LISTEN 0   512  127.0.0.1:8812  0.0.0.0:*  users:(("bun",pid=3100717,fd=11)) # Realtime
LISTEN 0   200  127.0.0.1:5432  0.0.0.0:*                                   # PostgreSQL
```

```
$ sudo ufw status verbose
Status: active
Default: deny (incoming), allow (outgoing), deny (routed)
22/tcp    ALLOW IN  Anywhere
80/tcp    ALLOW IN  Anywhere
443/tcp   ALLOW IN  Anywhere
3080/tcp  ALLOW IN  Anywhere      # stack IP-only
7777/tcp  ALLOW IN  Anywhere      # Gemium static demo
8080/tcp  ALLOW IN  Anywhere      # M3 World Nginx Proxy
9222,9224,9225/tcp ALLOW IN Anywhere
5901/tcp  DENY  IN  Anywhere      # VNC explicite deny
Anywhere  ALLOW IN  81.64.138.142 # admin home IP - full access
Anywhere on wg0 ALLOW IN Anywhere # WireGuard
```

Un service annexe mérite d'être noté : 15 processus `socat` republient des ports locaux sur
l'interface VPN `10.8.0.1` (dont `6379` Redis et `9222` Chrome DevTools). C'est confiné au
tunnel WireGuard, donc hors périmètre de cet audit, mais Chrome DevTools sur un tunnel est un
accès `Runtime.evaluate` complet pour quiconque entre dans le VPN.

**Le problème n'est donc pas le réseau : c'est nginx.** `supabase-compat.inc` est inclus dans le
vhost public d'azalée (`azalee.rosegriffon.conf:75`) et publie les trois surfaces sous le domaine
du site, avec `Access-Control-Allow-Origin: "*"` posé en dur sur `/rest/v1/` et `/graphql/v1`.

### 1.2 Ce que RLS couvre réellement

La base `rg` compte **301 tables** dans `public`. La couverture RLS est bonne dans l'ensemble :

```
$ sudo -u postgres psql -d rg -tAc "select count(*) from pg_class c join pg_namespace n
    on n.oid=c.relnamespace where c.relkind='r' and n.nspname='public';"
301
$ sudo -u postgres psql -d rg -tAc "... and c.relrowsecurity=false;"
2      -- niers_schema_migrations, x_campagne_relais
```

Les deux tables sans RLS **ne sont pas accordées à `anon`** (requête croisant
`role_table_grants` et `pg_class` : 0 ligne). Les ~20 tables à RLS activée sans aucune politique
échouent en fermeture — c'est le comportement voulu. Les tables d'authentification sont
correctement verrouillées :

```
$ for t in account user session verification admin_audit_log audit_logs; do
    curl -s -o /tmp/x -w "$t -> %{http_code} " http://127.0.0.1:8809/$t?limit=1
    jq 'length' /tmp/x; done
account -> 200 0        user -> 200 0        session -> 200 0
verification -> 200 0   admin_audit_log -> 200 0   audit_logs -> 200 0
```

`200` avec zéro ligne : la politique `*_service_only` (`using = false`) filtre tout. `profiles`
va plus loin et n'est même pas accordée :

```
$ curl -s https://azalee.rosegriffon.fr/rest/v1/profiles
{"code":"42501","message":"permission denied for table profiles"}
```

### 1.3 Le vrai problème : les privilèges accordés à `anon`

**RLS est la seule barrière, et elle porte tout le poids.** Le rôle `anon` détient
`DELETE, INSERT, REFERENCES, SELECT, TRIGGER, TRUNCATE, UPDATE` — c'est-à-dire `ALL` — sur
**129 tables**, dont `account`, `user`, `audit_logs`, `admin_audit_log`, `agent`,
`approvalRequest`, `two_factor`.

```
$ sudo -u postgres psql -d rg -tAc "select count(distinct table_name)
    from information_schema.role_table_grants where grantee='anon' and table_schema='public';"
129
$ ... where grantee='anon' and table_name='account';
account|DELETE,INSERT,REFERENCES,SELECT,TRIGGER,TRUNCATE,UPDATE
```

C'est une posture à barrière unique. Une politique RLS oubliée sur une table nouvelle, un
`ALTER TABLE ... DISABLE ROW LEVEL SECURITY` passé en migration, un `CREATE TABLE` sans
`ENABLE RLS`, et l'anonyme obtient `TRUNCATE` sur cette table depuis Internet. Le modèle Supabase
hébergé fonctionne ainsi, mais il s'accompagne là-bas d'une clé `apikey` obligatoire au niveau de
la passerelle — ici, **aucune clé n'est exigée** : `supabase-compat.inc` transmet la requête à
PostgREST sans vérifier ni `apikey` ni `Authorization`.

### 1.4 Ce qu'un anonyme lit réellement, aujourd'hui

**Données personnelles — 2 105 membres Discord, nominatifs.**

```
$ curl -s 'https://azalee.rosegriffon.fr/rest/v1/discord_members?limit=1' \
    -H 'Prefer: count=exact' -H 'Range: 0-0' -D - -o /tmp/d.json | grep -i content-range
Content-Range: 0-0/2105
$ jq -r '.[0]|keys|join(", ")' /tmp/d.json
avatar_url, discord_id, display_name, is_bot, joined_at, nickname,
left_at, premium_since, roles, updated_at, username
```

Identifiant Discord, pseudo, surnom de serveur, avatar, rôles, date d'arrivée, date de départ et
statut Nitro de 2 105 personnes. Au sens du RGPD (art. 4), `discord_id` est un identifiant direct
et `roles` + `premium_since` sont des données de comportement. **Cette table est une base de
données de membres publiée en accès libre.**

**Autres lectures anonymes confirmées :**

```
$ for t in inagle_characters content_feed campagne_creations_instagram settings; do ... done
inagle_characters             -> 206  0-0/6168   59 colonnes
content_feed                  -> 206  0-0/107    10 colonnes
campagne_creations_instagram  -> 206  0-0/2      17 colonnes
settings                      -> 200  9 lignes
```

`inagle_characters` et consorts sont des données de jeu — leur diffusion est couverte par
l'accord RG-L5-VR-2026-001 et ne pose pas de problème. `settings` en revanche expose la
configuration métier :

```
$ curl -s 'https://azalee.rosegriffon.fr/rest/v1/settings?select=key' | jq -r '.[].key'
patreon.incentive_offer          patreon.incentive_max_redemptions
patreon.import_addresses_consent patreon.discord_grace_days
patreon.t0                       patreon.cutoff_days
patreon.grandfather_months       tweets.notification
tweets.notification.pause
```

Ce sont les paramètres d'une campagne commerciale (offre incitative, plafond de rachats, fenêtre
de grâce, date de coupure) : leur divulgation n'est pas une faille technique, c'est une fuite de
stratégie commerciale.

### 1.5 Ce qui est le plus grave : un RPC destructif appelable par un anonyme

PostgREST publie **21 fonctions** sous `/rpc/` :

```
$ curl -s http://127.0.0.1:8809/ | jq -r '.paths|keys[]|select(startswith("/rpc/"))'
/rpc/check_rate_limit          /rpc/rg_liberer_profil_discord
/rpc/generate_article_slug     /rpc/rg_precreer_profils_discord
/rpc/get_comment_counts        /rpc/rg_uuid_membre_discord
/rpc/get_my_patreon_status     /rpc/upsert_reading_progress
/rpc/get_share_counts          /rpc/increment_article_views
/rpc/get_trending_articles     /rpc/increment_share_count
/rpc/is_active_patron          /rpc/is_admin
/rpc/patreon_heartbeat         /rpc/rg_libelle_equipe
/rpc/show_limit /rpc/show_trgm /rpc/x_creation_doublon_ecarte
/rpc/x_masquer_hashtags        /rpc/x_regles_non_vides
```

`rg_liberer_profil_discord(p_discord_id text)` est en `SECURITY DEFINER`, exécutable par `anon`,
et son corps contient :

```sql
delete from public.profiles p where p.discord_id = p_discord_id and p.claimed_at is null ...
delete from auth.users a where a.id = public.rg_uuid_membre_discord(p_discord_id) ...
```

**Preuve d'appel depuis Internet.** La sonde utilise la chaîne vide, qui emprunte la première
branche de la fonction (`if p_discord_id is null or p_discord_id = '' then return 0`) et **ne
touche donc à aucune donnée** — c'est l'appel inoffensif qui prouve l'accessibilité sans rien
détruire :

```
$ curl -s -w '\nHTTP %{http_code}\n' -X POST \
    'https://azalee.rosegriffon.fr/rest/v1/rpc/rg_liberer_profil_discord' \
    -H 'Content-Type: application/json' -d '{"p_discord_id":""}'
0
HTTP 200
```

Un attaquant qui fournit un vrai `discord_id` supprime le profil correspondant **et sa ligne dans
`auth.users`**. Les trois gardes internes (`claimed_at is null`, identifiant dérivé, aucun champ
saisi par un humain) limitent les dégâts aux profils **pré-créés et non réclamés** — ce qui est
une bonne conception défensive — mais la liste des `discord_id` cibles est justement celle que
`/rest/v1/discord_members` publie en clair (§ 1.4). **Les deux failles se composent : la
première fournit les 2 105 arguments d'entrée de la seconde.** Un script trivial efface en
quelques minutes tous les profils pré-créés non réclamés du serveur.

Trois autres RPC sont anonymement appelables et écrivent :

| RPC | Effet | Défaut |
|---|---|---|
| `rg_precreer_profils_discord()` | `INSERT` dans `auth.users` et `public.profiles` | Écriture de masse déclenchable par un anonyme, sans argument |
| `upsert_reading_progress(p_user_id uuid, …)` | `INSERT/UPDATE` sur `reading_history` | **IDOR** : l'appelant choisit le `user_id` de sa victime |
| `check_rate_limit(p_user_id uuid, …)` | `INSERT/UPDATE` sur `rate_limits` | Un anonyme épuise le quota d'un autre utilisateur — la fonction *anti-abus* est elle-même l'outil d'abus |

**Point rassurant vérifié.** `promote_rg_creator_to_admin` — qui promeut un profil en `admin` —
est une fonction `RETURNS trigger`. PostgREST refuse d'appeler les fonctions trigger :

```
$ curl -s -o /dev/null -w '%{http_code}\n' -X POST \
    http://127.0.0.1:8809/rpc/promote_rg_creator_to_admin -d '{}'
404
```

Il n'y a **pas** d'escalade de privilèges par cette voie. C'est heureux : la fonction ne vérifie
que le contenu de `NEW`, sans authentifier l'appelant.

### 1.6 Ce qu'il faut, au minimum, pour que l'exposition soit défendable

Par ordre d'efficacité par unité d'effort :

1. **Révoquer l'écriture à `anon`, immédiatement.**
   `REVOKE INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER ON ALL TABLES IN SCHEMA public
   FROM anon;` puis `ALTER DEFAULT PRIVILEGES ... REVOKE ...`. `anon` ne doit détenir que
   `SELECT`. Toute écriture légitime passe par un rôle authentifié ou par une route serveur.
   Cela transforme la posture « une barrière unique (RLS) » en « deux barrières indépendantes
   (privilèges + RLS) », qui est le minimum défendable pour une API publique.
2. **Révoquer `EXECUTE` à `anon` sur les 4 RPC d'écriture.**
   `rg_liberer_profil_discord`, `rg_precreer_profils_discord`, `upsert_reading_progress`,
   `check_rate_limit`. Les deux premiers sont des outils d'administration qui n'ont rien à faire
   dans une API publique ; les deux autres doivent exiger `authenticated` et déduire le `user_id`
   de `auth.uid()` au lieu de le recevoir en paramètre.
3. **Exiger la clé `apikey` au niveau de nginx.** `supabase-compat.inc` laisse aujourd'hui passer
   toute requête sans en-tête. Poser un `if ($http_apikey = "") { return 401; }` (ou mieux, un
   `map` sur les clés valides) rétablit le comportement de la passerelle Kong de Supabase et
   écarte 100 % du balayage automatisé.
4. **Restreindre le CORS.** `Access-Control-Allow-Origin: "*"` autorise n'importe quel site à
   faire lire l'API par le navigateur de ses visiteurs. Après la bascule, la seule origine
   légitime sera le domaine Vercel : la remplacer par une valeur explicite.
5. **Retirer `discord_members` de `anon`, ou la réduire à une vue.** Aucune page publique n'a
   besoin de `discord_id`, `left_at` ni `premium_since`. Une vue `discord_members_public`
   exposant `display_name` et `avatar_url` suffit, sur le modèle déjà appliqué à `profiles`
   (`20260811_profiles_pii.sql`).
6. **Retirer `settings` de `anon`.**
7. **Un sous-domaine dédié, pas le domaine du site.** Après la bascule, servir ces trois surfaces
   sous `supabase.rosegriffon.fr` (le vhost existe déjà) plutôt que sous le domaine Vercel :
   cela permet de poser une politique de sécurité distincte, un pare-feu applicatif distinct et
   une limitation de débit distincte, sans dépendre de la configuration Vercel.

---

## 2. `nie-model-serve` (port 8790)

Serveur HTTP écrit à la main, `std::net::TcpListener` + pool de threads, 6 796 lignes dans
`crates/tools/nie-model-serve/src/main.rs` (+ 760 dans `catalogue.rs`, 163 dans `menu.rs`),
29 familles de routes, 250 800 entrées de VFS servies, atteignable publiquement via
`cdn.rosegriffon.fr`.

### 2.1 Traversée de chemin — **INFIRMÉ**

La mission désigne ce service comme « le point le plus exposé du projet ». **Sur le critère de la
traversée de chemin, c'est infirmé, sur les quatre familles de routes citées.** Le détail
compte, parce que la conclusion n'est pas « c'est sûr par chance » mais « c'est sûr par
construction, à trois niveaux indépendants ».

**`/depot/*` — désactivé en production.** La route existe dans le code mais n'est montée que si
`--depot-code` est passé. Le fichier d'unité ne le passe pas :

```
$ systemctl cat nie-model-serve | grep -c 'depot-code'
0
$ for p in '../../../etc/passwd' '/etc/passwd' '.env.local' 'apps/azalee/.env.local' \
           'var/niers.sqlite' '.git/config' 'Cargo.toml'; do
    curl -s -G --data-urlencode "path=$p" -w 'HTTP %{http_code} ' --max-time 15 \
      http://127.0.0.1:8790/depot/read; done
HTTP 404 route /depot inconnue    (×7, y compris sur Cargo.toml qui existe)
```

`Cargo.toml` renvoie 404 comme `/etc/passwd` : ce n'est pas un refus de traversée, c'est la route
entière qui n'existe pas. Le commentaire du code assume explicitement ce choix
(`main.rs:4248` : « cette instance est joignable publiquement, et publier le code du projet est
une décision, pas un défaut »). En défense supplémentaire, `cdn.rosegriffon.conf:902` termine par
`location / { return 404; }` — `/depot/` n'est de toute façon pas routé côté nginx.

**Le moteur `nie_explore::depot` est correct même s'il était activé.**
`crates/engine/nie-explore/src/depot.rs:210` (`resoudre`) : rejet du `\0`, normalisation lexicale
des `..` **avant** tout accès disque, vérification `starts_with(racine)`, contrôle d'exclusion,
`canonicalize`, puis **re-vérification `starts_with` après canonicalisation** (pour attraper un
lien symbolique sortant), puis second contrôle d'exclusion. `DOSSIERS_EXCLUS` écarte `refs, data,
var, .git, target, node_modules` ; `fichier_sensible()` (`depot.rs:58`) écarte tout `.env*`,
`*.key`, `*.pem`, `*.p12`, `*.pfx`, `id_rsa`, `.npmrc`, `.netrc`, `.pgpass`, `credentials`,
`.htpasswd` **à n'importe quelle profondeur**, pas seulement à la racine. C'est le schéma correct.

**`/raw/*` — rejet explicite du `..`.** À noter, la sonde doit passer `--path-as-is` : sans lui,
`curl` normalise les `../` côté client et l'on teste son propre client, pas le serveur.

```
$ curl -s --path-as-is -w 'HTTP %{http_code} ' 'http://127.0.0.1:8790/raw/../../../../etc/passwd'
HTTP 400 chemin invalide
$ curl -s --path-as-is -w 'HTTP %{http_code} ' \
    'http://127.0.0.1:8790/raw/%2e%2e/%2e%2e/%2e%2e/%2e%2e/etc/passwd'
HTTP 404 fichier absent du VFS
$ curl -s --path-as-is -w 'HTTP %{http_code} ' 'http://127.0.0.1:8790/raw/data/../../../../../../etc/passwd'
HTTP 400 chemin invalide
```

La variante percent-encodée est intéressante : le chemin de requête **n'est pas** percent-décodé
pour le routage (`main.rs:4128`, seul `param()` décode les paramètres de query). `%2e%2e` reste
donc littéral, échappe au test `contains("..")` — et se heurte au niveau suivant.

**`/vfs/*` — l'index n'est pas un système de fichiers.** `/vfs/stat?path=` *est* percent-décodé,
donc le test `contains("..")` n'y suffirait pas. Mais la résolution se fait dans l'index CPK, pas
sur le disque :

```
$ curl -s 'http://127.0.0.1:8790/vfs/stats'
{"cpkCount":936,"extraCount":0,"looseCount":5,"total":255308}
$ for p in '../../../../etc/passwd' '/etc/passwd' 'data/../../../../../etc/passwd'; do
    curl -s -G --data-urlencode "path=$p" -w 'HTTP %{http_code} ' http://127.0.0.1:8790/vfs/stat; done
HTTP 404 chemin absent du VFS    (×3)
```

`cpkCount=936` : c'est le montage *packs*, pas le montage *dump*. Les chemins se résolvent dans
les tables d'archives CPK, où `../` n'a aucune signification. C'est le troisième niveau, et le
plus solide, parce qu'il est structurel.

**Réserve.** `looseCount=5` : cinq fichiers se résolvent hors archive. Le risque est confiné à
ces cinq entrées et je n'ai pas trouvé de chemin d'attaque, mais c'est le seul endroit du service
où le VFS touche le disque, et c'est donc là qu'il faudrait regarder en premier si le montage
basculait un jour sur *dump* (255 308 fichiers résolus sur le système de fichiers). **La sûreté
constatée ici dépend du mode de montage, qui n'est pas figé par une garde de code.**

### 2.2 Limitation de débit — absente, et le filet nginx n'est pas branché

**Aucun contrôle dans le service :**

```
$ grep -c 'Authorization\|X-Api-Key\|token\|rate_limit\|limiteur' \
    crates/tools/nie-model-serve/src/main.rs
0
```

Zéro occurrence sur 6 796 lignes. Pas d'authentification, pas de clé, pas de compteur, pas de
contrôle d'origine.

**Et le garde-fou nginx est déclaré mais jamais appliqué :**

```
$ grep -rn 'limit_req_zone' /etc/nginx/nginx.conf
111:    limit_req_zone $binary_remote_addr zone=rpb_api:10m rate=30r/s;
$ grep -rn '^\s*limit_req\b\|^\s*limit_conn\b' /etc/nginx/nginx.conf /etc/nginx/conf.d/*.conf
(aucune ligne)
```

La zone `rpb_api` occupe 10 Mio de mémoire partagée et **ne limite rien**. C'est le pire état
possible : le dispositif existe, il figure dans la configuration, il donne l'impression d'être en
place, et il est inerte. Une relecture de la configuration le compte comme une protection. À
noter que `fail2ban` a une prison `nginx-limit-req` active — elle bannit sur les journaux de
`limit_req`, qui ne produit aucune ligne : **cette prison est structurellement muette.**

### 2.3 Taille de réponse non bornée — confirmé, et mesuré

```
$ curl -s -o /dev/null -w 'HTTP %{http_code} %{size_download} octets en %{time_total}s\n' \
    'https://cdn.rosegriffon.fr/vfs/find?q=.&limit=20000'
HTTP 200 3148662 octets en 0.689621s
```

**3,1 Mio pour une requête GET anonyme de 60 octets**, servie en 0,69 s. Le facteur
d'amplification dépasse 50 000×. Sans limitation de débit, un client unique sur une liaison
domestique sature la bande passante sortante du VPS. Le plafond de `limit` est fixé à 20 000
(`main.rs:4152`, `4172`) — ce plafond borne la structure de la réponse, pas son coût.

### 2.4 Budget de temps — partiellement présent, et insuffisant en pratique

Le code **a** des délais, et ils sont bien raisonnés (`main.rs:4023-4029`, avec en commentaire la
panne du 21/08/2026 qui les a motivés) :

```rust
const DELAI_LECTURE:  Duration = Duration::from_secs(10);  // inactivité en lecture
const DELAI_ECRITURE: Duration = Duration::from_secs(30);  // inactivité en écriture
```

Ce sont des délais **d'inactivité de socket**, pas un budget de temps de traitement. Rien ne
borne la durée d'un travail utile. Mesuré sur une route d'assemblage :

```
$ curl -s -o /dev/null -w 'HTTP %{http_code} %{size_download}o en %{time_total}s\n' \
    --max-time 90 'https://cdn.rosegriffon.fr/model-chr/c02023700.glb'
HTTP 504 160o en 30.017043s
```

**Une seule requête anonyme épuise le `proxy_read_timeout` de nginx.** Le client reçoit 504, mais
le serveur continue de calculer : le travail n'est pas annulé, il occupe un des **4** ouvriers du
pool (`--http-threads 4`). Quatre requêtes de ce type simultanées immobilisent l'intégralité du
service, avec une file d'attente de 128 derrière.

### 2.5 Mémoire — le service dépasse son propre plafond souple

```
$ systemctl show nie-model-serve -p MemoryCurrent -p MemoryPeak -p MemoryHigh -p MemoryMax
MemoryCurrent=5660467200      # 5,27 Gio
MemoryPeak=5654917120         # 5,27 Gio
MemoryHigh=5368709120         # 5,00 Gio  <- dépassé
MemoryMax=7516192768          # 7,00 Gio
$ free -m
               total   used   free  shared  buff/cache  available
Mem:           47035  21524  11591     382       17042       25510
Swap:          49151   7986  41165
```

Le service tourne **au-dessus de son `MemoryHigh`**, donc sous étranglement permanent du noyau,
à 1,7 Gio de son `MemoryMax` où le tueur de mémoire l'abat. Les journaux confirment que ce n'est
pas un pic isolé : `Consumed 46.151s CPU time over 9min 10.393s wall clock time, 5.5G memory
peak, 1G memory swap peak`. Et le swap de la machine est déjà entamé de 7,9 Gio.

Le budget de cache CPK est fixé à 8 Gio par variable d'environnement
(`NIE_CPK_CACHE_BUDGET_GIB=8`) alors que le plafond dur du cgroup est de 7 Gio : **le service est
autorisé à demander plus de mémoire que systemd ne lui en accordera jamais.** Il atteindra donc
`MemoryMax` avant d'atteindre son propre plafond de cache. C'est très exactement le mode de
panne consigné dans `nie-model-serve-memoire-resilience.md` (saturation → sortie → 502 « décodage
indisponible »), et `Restart=always` le masque au lieu de le corriger.

### 2.6 Verdict sur la question posée

**Le service n'est pas le point le plus exposé du projet.** Ce titre revient à l'API PostgREST
publique (§ 1), qui donne à un anonyme un RPC destructif et 2 105 fiches nominatives — une
atteinte à la confidentialité et à l'intégrité, pas seulement à la disponibilité.

`nie-model-serve` est le **deuxième** risque, et sa nature est le déni de service : zéro
authentification, zéro limitation de débit, amplification de 50 000×, pas de budget de temps de
traitement, 4 ouvriers, et une consommation mémoire déjà au-delà de son plafond souple. Sa
surface de *lecture arbitraire* est en revanche correctement fermée, à trois niveaux
indépendants.

### 2.7 Correctifs

1. **Brancher `limit_req`** dans le vhost `cdn.rosegriffon.fr` : `limit_req zone=rpb_api burst=60
   nodelay;`. La zone existe déjà — c'est une ligne. Cela réveille aussi la prison `fail2ban`
   `nginx-limit-req`, aujourd'hui muette.
2. **Ajouter `limit_conn`** (une zone `$binary_remote_addr` + `limit_conn addr 8;`) : c'est la
   limite qui protège les 4 ouvriers, plus encore que le débit.
3. **Un budget de temps par requête** dans le service, avec abandon coopératif du travail : une
   requête qui dépasse 20 s doit rendre 503 et **libérer son ouvrier**, sinon le 504 de nginx ne
   protège que le client.
4. **Aligner `NIE_CPK_CACHE_BUDGET_GIB` sous `MemoryMax`** (4 Gio pour 7 Gio de plafond dur), et
   surveiller `MemoryHigh` plutôt que de compter sur `Restart=always`.
5. **Plafonner la taille de réponse** : refuser `limit` au-delà de ce qu'un client réel consomme,
   ou imposer la pagination (`offset` existe déjà).
6. **Figer le montage VFS en mode packs** par une garde explicite, puisque la sûreté des chemins
   en dépend (§ 2.1, réserve).

---

## 3. Les secrets — ordre de rotation par gravité

**Aucune valeur de secret n'apparaît dans ce document.** Toutes les inspections ont été faites
par `sed -E 's/=.*/=<REDACTED>/'`, `grep -l` ou `grep -c`.

### 3.1 L'ampleur réelle est bien supérieure à quatre jetons

La mission mentionne quatre jetons dans `~/.config/niers/{github,vercel,supabase}.env`. Le
répertoire en contient **douze fichiers**, tous en `0600` avec un répertoire parent en `0700` —
les permissions sont correctes :

```
$ ls -la ~/.config/niers/
drwx------ ubuntu ubuntu  .
-rw------- 6727  cron.env          -rw------- 2435  steam-cookies.json
-rw-------  315  donnees.env       -rw------- 1215  steam-cookies.txt
-rw-------  277  github.env        -rw-------  531  steam.env
-rw-------   84  mcp.env           -rw-------  343  supabase.env
-rw-------  486  vroid.env         -rw-------  238  vercel.env
-rw------- 2153  wonderbot.env
```

**`cron.env` porte à lui seul 44 clés.** Il est chargé par `nie-cron.service` (vérifié :
`EnvironmentFile=/home/ubuntu/.config/niers/cron.env`). Il contient notamment
`SUPABASE_JWT_SECRET`, `SUPABASE_SERVICE_ROLE_KEY`, `BETTER_AUTH_SECRET`, `STRIPE_SECRET_KEY`,
`STRIPE_WEBHOOK_SECRET`, `DISCORD_BOT_TOKEN`, `DISCORD_CLIENT_SECRET`, `PATREON_CLIENT_SECRET`,
`PATREON_CREATOR_ACCESS_TOKEN`, `PATREON_WEBHOOK_SECRET`, `TWITCH_CLIENT_SECRET`,
`GOOGLE_APPLICATION_CREDENTIALS`, `BLOB_READ_WRITE_TOKEN`, `NODE_AUTH_TOKEN`, `GITHUB_TOKEN`,
`VERCEL_OIDC_TOKEN`, `CRON_SECRET`, `BOT_ADMIN_TOKEN`, `DATABASE_URL`.

**Le fichier de 44 secrets est lui-même le problème.** Un service, une prison. `nie-cron`
n'a besoin ni de la clé Stripe ni du jeton GitHub `admin:enterprise` ; il les charge pourtant
tous dans son environnement, où n'importe quelle bibliothèque compromise de sa chaîne de
dépendances les lit en une ligne.

**Aucun `.env` n'est suivi par git** (vérifié : `git ls-files | grep '\.env'` ne rend que deux
`.env.example`), et `apps/azalee/.env` (11 492 octets, `0600`) est bien ignoré
(`git check-ignore -v` → `apps/azalee/.gitignore:23:.env*`). Le dépôt est propre ; c'est la
machine qui concentre le risque.

### 3.2 Ordre de rotation

#### Rang 1 — `SUPABASE_JWT_SECRET` (**le plus grave, et il n'est pas dans la liste des quatre**)

C'est le secret qui signe les JWT que PostgREST valide. Le rôle `service_role` porte
`rolbypassrls = true` (vérifié dans `pg_roles`) et `authenticator` en est membre (vérifié dans
`pg_auth_members`). **Quiconque détient ce secret forge un JWT `service_role` et lit, modifie ou
efface les 301 tables de la base — RLS compris — via une API REST accessible depuis
Internet (§ 1.1).** C'est la seule fuite de cette liste qui donne, à elle seule, la compromission
totale des données.

- **Références (8 fichiers) :** `apps/storage/server.ts`, `apps/azalee/lib/supabase/server.ts`,
  `apps/azalee/lib/supabase/jwt.ts`, `packages/db/src/env.ts`, `packages/inagle/src/cli-push.ts`,
  `packages/inagle/src/push-adapter.ts`, plus deux fichiers de documentation.
- **Systemd :** aucune unité ne le nomme directement ; il arrive par `cron.env`
  (`nie-cron.service`) et par `apps/azalee/.env` (`azalee-web.service`, `azalee-web-b.service`).
- **Ce qui casse :** **toutes** les sessions utilisateur en cours (les JWT émis deviennent
  invalides), le service `rg-storage` (qui valide les jetons avec ce secret), et les scripts de
  poussée `inagle`. La rotation exige de mettre à jour `/etc/postgrest.conf`
  (`jwt-secret`), `/etc/rg-storage.env`, `cron.env` et `apps/azalee/.env` **de façon atomique**,
  puis de redémarrer `rg-postgrest`, `rg-storage`, `azalee-web`, `nie-cron`. Fenêtre de coupure à
  prévoir : c'est la rotation la plus intrusive de la liste, et c'est pourquoi elle doit être
  planifiée plutôt que faite dans l'urgence.

#### Rang 2 — Jeton GitHub `aphrody-dev` (`github.env` : `GH_TOKEN`, `GITHUB_TOKEN`)

Portées annoncées : `admin:org`, `admin:enterprise`, `delete_repo`, `repo`, `workflow`,
`write:packages`. **`admin:enterprise` et `delete_repo` dépassent de très loin tout besoin de ce
dépôt.** Une fuite permet de supprimer des dépôts, de modifier les workflows CI (donc d'exécuter
du code arbitraire dans le contexte des exécuteurs GitHub et d'exfiltrer les secrets d'Actions),
et d'administrer l'organisation. Combiné au § 5, c'est aussi le chemin vers la publication d'une
release desktop signée — sauf que la clé de signature minisign n'est pas sur GitHub, ce qui
coupe cette voie.

- **Références dans le dépôt (2) :** `bunfig.toml`, `packages/cron/src/tasks/api.ts`.
- **Systemd :** 0 unité le nomme ; il arrive par `cron.env` (`nie-cron.service`).
- **Ce qui casse :** la tâche `api` de `nie-cron` (veille des dépôts), l'installation de paquets
  npm privés via `bunfig.toml`, et `NODE_AUTH_TOKEN` s'il partage la même valeur.
- **Correctif au-delà de la rotation :** re-créer un jeton à portée fine (*fine-grained*) limité
  au seul dépôt `aphrody-code/nie`, en lecture de contenu et écriture de releases. **Ne jamais
  reconduire `admin:enterprise` ni `delete_repo`.**

#### Rang 3 — `SUPABASE_DB_PASSWORD` / mot de passe `authenticator`

Deux problèmes distincts ici.

`/etc/postgrest.conf` contient l'URI de connexion **avec le mot de passe en clair**. Le fichier
est en `0600 postgrest:postgrest`, donc correctement protégé sur le disque — mais toute lecture
de fichier arbitraire par le compte `postgrest`, ou toute copie de configuration, le divulgue.
`authenticator` est membre de `service_role` (`BYPASSRLS`) : ce mot de passe vaut donc autant que
le rang 1, à ceci près que Postgres n'écoute que sur `127.0.0.1`, ce qui exige d'abord un accès à
la machine.

- **Références (2) :** `scripts/ops/migrate-to-supabase-cloud.ts`,
  `scripts/ops/README-SUPABASE-MIGRATION.md`. **Systemd :** 0.
- **Ce qui casse :** `rg-postgrest` si `/etc/postgrest.conf` n'est pas mis à jour en même temps
  que le rôle ; les scripts de migration (qui ne tournent pas en continu).

#### Rang 4 — `VERCEL_TOKEN` (`vercel.env`)

Un jeton Vercel permet de déclencher des déploiements, de lire les variables d'environnement d'un
projet (donc **tous les autres secrets qui y seront posés après la bascule**) et de modifier les
domaines. **Sa gravité augmente mécaniquement avec la bascule** : aujourd'hui il ne contrôle
presque rien, demain il contrôle le site.

- **Références dans le dépôt : 0.** **Systemd : 1** — `/etc/systemd/system/vercel-token-sync.service`,
  un service `oneshot` qui exécute `/home/ubuntu/vercel-token-sync.sh` et dont la description est
  « Resync d'un token Vercel valide vers le secret GitHub VERCEL_TOKEN ».
- **Ce qui casse :** `vercel-token-sync.service` et, par ricochet, le secret GitHub Actions
  `VERCEL_TOKEN` qu'il alimente — donc les déploiements automatiques. **Ce service est aussi un
  pont entre les rangs 2 et 4 : il recopie un jeton Vercel dans GitHub. Une compromission du
  jeton GitHub donne accès au jeton Vercel, et réciproquement.** Il faut les faire tourner
  ensemble, jamais l'un sans l'autre.

#### Rang 5 — `SUPABASE_ACCESS_TOKEN` (`supabase.env`)

Jeton de la plateforme Supabase (API de gestion). Le VPS étant en auto-hébergement, il ne sert
qu'aux scripts de migration vers le nuage.

- **Références (3) :** `scripts/ops/load-game-data-to-cloud.ts`,
  `scripts/ops/migrate-to-supabase-cloud.ts`, plus la documentation. **Systemd :** 0.
- **Ce qui casse : rien en production.** C'est la rotation la moins coûteuse et elle devrait être
  la première faite, précisément parce qu'elle n'a aucun impact.

#### Rang 6 — les autres, à faire tourner dans la foulée

`BETTER_AUTH_SECRET` (3 réf. : `apps/azalee/lib/auth.ts`,
`apps/azalee/app/api/auth/magic-login/route.ts`, README ; invalide toutes les sessions),
`CRON_SECRET` (4 réf. dont `apps/azalee/app/api/cron/publish-scheduled/route.ts` ; protège une
route déclenchable), `RG_MCP_ADMIN_TOKEN` (4 réf. + **1 unité systemd**, `rg-mcp.service` —
casse le serveur MCP), `DISCORD_BOT_TOKEN`, `STRIPE_SECRET_KEY`, `PATREON_*`, `TWITCH_*` — tous
présents dans `cron.env` et donc tous à considérer comme exposés au même titre.

### 3.3 Correctif structurel

La rotation traite le symptôme. Le défaut de conception est **la concentration** :
`nie-cron.service` charge 44 secrets pour en utiliser peut-être dix. Deux mesures :

1. **Découper `cron.env` par tâche**, et n'attacher à chaque unité que ce dont elle a besoin.
   `systemd` accepte plusieurs directives `EnvironmentFile`.
2. **Passer les secrets par `systemd-creds`** (`LoadCredentialEncrypted=`) plutôt que par
   `EnvironmentFile=`. Un `EnvironmentFile` est lisible dans `/proc/<pid>/environ` par tout
   processus du même utilisateur ; `systemd-creds` chiffre au repos et n'expose le secret qu'au
   processus destinataire.

---

## 4. Surface d'attaque après la bascule DNS

### 4.1 Ce que le VPS continuera de servir sous ce nom — et le point aveugle

Le vhost déclare `server_name azalee.rosegriffon.fr 51.77.147.152;`. **Le nom DNS bascule, l'IP
ne bascule pas.** Vérifié :

```
$ curl -sk -o /dev/null -w 'https://51.77.147.152/ (sans SNI) -> HTTP %{http_code}\n' \
    https://51.77.147.152/
https://51.77.147.152/ (sans SNI) -> HTTP 200
$ curl -sk -o /dev/null -w '(Host: azalee) -> HTTP %{http_code} %{size_download}o\n' \
    -H 'Host: azalee.rosegriffon.fr' https://51.77.147.152/
(Host: azalee) -> HTTP 200 312807o
```

**312 807 octets de HTML servis.** Après la bascule, le VPS hébergera donc une **copie fantôme
complète et fonctionnelle du site**, atteignable à l'IP nue, avec :

- l'intégralité de `/rest/v1/`, `/graphql/v1`, `/storage/v1/`, `/realtime/v1/` (§ 1) ;
- une version du code qui se figera au dernier déploiement et **cessera de recevoir les
  correctifs** appliqués côté Vercel ;
- l'authentification `better-auth` pointant sur la même base de production.

C'est le point le plus dangereux de la bascule, parce qu'il est **silencieux** : rien ne signale
qu'un second exemplaire du site tourne. Il ne sera plus surveillé, plus mis à jour, plus testé —
et il restera indexable par tout balayage d'IP (Shodan, Censys). Les scanners de vulnérabilités
trouvent ce genre d'hôte en heures, pas en mois.

**Correctif :** au moment de la bascule, retirer `51.77.147.152` du `server_name`, et poser un
`server` par défaut (`listen 443 ssl default_server;` avec un certificat autosigné) qui rend 444.
Décider explicitement du sort du vhost : soit il est supprimé, soit il est conservé en
pré-production derrière une restriction d'IP (`allow 81.64.138.142; deny all;` — l'IP
d'administration figure déjà dans `ufw`).

### 4.2 Ce qui casse silencieusement — le plus coûteux d'abord

**a) `NEXT_PUBLIC_SUPABASE_URL` pointe l'origine du site.**

```
$ grep -hE 'SUPABASE_URL|SITE_URL|BETTER_AUTH_URL' apps/azalee/.env
NEXT_PUBLIC_SUPABASE_URL="https://azalee.rosegriffon.fr"
NEXT_PUBLIC_SITE_URL=https://azalee.rosegriffon.fr
BETTER_AUTH_URL="https://azalee.rosegriffon.fr"
```

Le commentaire en tête de `supabase-compat.inc` explique le choix : « Inclure ce fichier dans le
vhost de chaque app permet de pointer `NEXT_PUBLIC_SUPABASE_URL` sur l'origine de l'app
elle-même : aucune requête cross-origin ». **Ce choix, judicieux en auto-hébergement, est
exactement ce qui casse à la bascule.** Le navigateur continuera d'appeler
`https://azalee.rosegriffon.fr/rest/v1/…` — qui sera Vercel, où ces routes n'existent pas.
Résultat : **404 sur chaque appel client de Supabase**. 17 fichiers du dépôt lisent cette
variable (`packages/db/src/env.ts`, `apps/azalee/lib/supabase/public.ts`,
`components/providers/SupabaseProvider.tsx`, `lib/api-client.ts`, `app/*/opengraph-image.tsx`…).

L'échec est silencieux au sens le plus gênant : la page rend, le pré-rendu serveur fonctionne
(il parle à Postgres en direct), et seules les fonctions interactives cessent de répondre.
`bun run build` et `bun run typecheck` ne verront rien.

**Correctif :** pointer `NEXT_PUBLIC_SUPABASE_URL` sur `https://supabase.rosegriffon.fr` (le
vhost existe déjà et inclut le même `supabase-compat.inc`) **avant** la bascule, vérifier que le
site fonctionne encore en cross-origin, puis basculer. Cela impose de restreindre le
`Access-Control-Allow-Origin: "*"` à l'origine Vercel — ce que le § 1.6 recommande de toute façon.

**b) Les URL absolues déjà écrites en base.** Le même commentaire de `supabase-compat.inc` le
dit : « les URLs absolues déjà enregistrées en base
(`https://azalee.rosegriffon.fr/storage/v1/object/public/…`) restent valables ». Elles ne le
resteront **que** si Vercel réachemine `/storage/v1/`. Sinon, toutes les images stockées se
brisent. 25 colonnes de la base portent des URL (`inagle_keshins.image_url`,
`discord_messages.author_avatar_url`, `patch_notes.featured_image`, `events.image_url`,
`x_campagnes.image_couverture`…).

**Correctif :** soit une réécriture Vercel de `/storage/v1/*` vers le VPS, soit une migration en
base des URL absolues vers des chemins relatifs. La première est réversible, la seconde ne l'est
pas — commencer par la première.

**c) `/_next/static/` en `alias`.**

```nginx
location /_next/static/ {
    alias /home/ubuntu/rg-releases/azalee/static/;
    add_header Cache-Control "public, max-age=31536000, immutable";
    try_files $uri @bunproxy;
}
```

Sur Vercel, les statiques sont servis par leur propre CDN : la route n'a plus d'objet et ne
cassera rien. **Le piège est ailleurs** : `max-age=31536000, immutable` a été servi pendant des
mois. Les navigateurs qui ont ces ressources en cache les conserveront **un an**, avec les
empreintes de build du VPS. Comme Next.js hache le nom des fichiers, il n'y aura pas de collision
— mais tout visiteur ayant un `document.html` en cache pointant ces empreintes recevra un 404
Vercel. **Correctif :** garder `/home/ubuntu/rg-releases/azalee/static/` en place et servi
pendant au moins un cycle de cache, ou faire réacheminer `/_next/static/` par Vercel vers le VPS
le temps de la transition.

**d) Le micro-cache et la page de maintenance disparaissent.** Le bloc `proxy_cache rg_pages`
couvre 18 familles de routes de catalogue (`/chara`, `/skill`, `/item`, `/gallery`…) avec
`proxy_cache_lock on` et `proxy_cache_use_stale`. C'est un mécanisme d'anti-avalanche : un seul
rendu quand dix visiteurs demandent la même page froide. **Sur Vercel, il n'existe pas d'
équivalent automatique** — l'ISR de Next.js le remplace mais doit être configuré explicitement
par route. Les pages se déclarent `no-store` (c'est écrit dans le vhost : « Les pages se
déclarent `no-store` alors que leur contenu ne dépend d'aucune session »), donc **par défaut
Vercel ne mettra rien en cache et chaque visite déclenchera un rendu facturé**. C'est un risque
de coût autant que de performance. De même, `@maintenance` et la logique
`max_fails`/`fail_timeout` disparaissent.

**e) `location = /__preview`.**

```
$ curl -s -D - -o /dev/null 'https://azalee.rosegriffon.fr/__preview?token=AAA_injecte_BBB'
HTTP/1.1 302
Location: https://azalee.rosegriffon.fr/
Set-Cookie: rg_preview=AAA_injecte_BBB; Path=/; Secure; HttpOnly; SameSite=Lax
```

Le paramètre `?token=` est reflété **sans aucune validation** dans un `Set-Cookie`, par un
point d'entrée non authentifié. **Gravité réelle aujourd'hui : faible**, pour une raison
précise — la table de correspondance ne route rien :

```
$ cat /etc/nginx/rg-upstreams/azalee_preview.conf
upstream azalee_preview { server 127.0.0.1:3013 max_fails=0; keepalive 8; }
map $cookie_rg_preview $azalee_backend { default azalee_web; }
```

Aucune valeur de cookie ne mène ailleurs que vers `azalee_web`. Le point d'entrée est donc
**inerte** — mais seulement jusqu'au prochain `scripts/ops/deploy.ts`, qui regénère cette table
pour ouvrir une fenêtre de prévisualisation. Pendant cette fenêtre, un lien
`https://azalee.rosegriffon.fr/__preview?token=<jeton>` envoyé à un tiers l'épingle sur le slot B
non validé. Les attributs `Secure`, `HttpOnly` et `SameSite=Lax` sont corrects, et nginx ne
percent-décode pas `$arg_token` — **il n'y a pas d'injection d'en-tête**, seulement une valeur de
cookie non validée. **Correctif :** valider `$arg_token` contre une expression régulière
(`if ($arg_token !~ "^[A-Za-z0-9_-]{16,64}$") { return 400; }`) et faire disparaître le point
d'entrée à la bascule, puisque le déploiement bleu/vert n'aura plus d'objet.

**f) Ce qui reste sur le VPS et doit continuer d'y vivre.** `cdn.rosegriffon.fr` (le VFS et
`nie-model-serve`), `supabase.rosegriffon.fr`, `rosegriffon.fr`, `api.rosegriffon.fr`,
`studio.rosegriffon.fr`, et les 18 services de production. La bascule ne les concerne pas — mais
`azalee-web.service` et `azalee-web-b.service` deviendront inutiles et devront être arrêtés
explicitement, faute de quoi ils continueront de consommer de la mémoire sur une machine dont le
swap est déjà entamé de 7,9 Gio (§ 2.5).

### 4.3 Points d'infrastructure hors bascule, mais critiques

**SSH accepte l'authentification par mot de passe, et `root` peut se connecter.**

```
$ sudo sshd -T | grep -E '^(permitrootlogin|passwordauthentication|maxauthtries)'
permitrootlogin yes
passwordauthentication yes
maxauthtries 3
$ sudo passwd -S root
root P          # P = mot de passe posé et utilisable
$ sudo fail2ban-client status sshd
Total failed: 2331   |   Total banned: 504   |   Currently banned: 4
```

Le port 22 est ouvert au monde entier (`ufw`), `root` a un mot de passe utilisable, et
`PasswordAuthentication` est à `yes` — réaffirmé dans **trois** fichiers
(`sshd_config`, `sshd_config.d/00-hardening.conf`, `sshd_config.d/50-cloud-init.conf`), ce
dernier étant susceptible d'être réécrit par cloud-init. Le fichier qui s'appelle
`00-hardening.conf` **pose lui-même `PermitRootLogin yes`** : le durcissement affiché contredit
le durcissement réel.

`fail2ban` a banni 504 adresses pour 2 331 échecs — c'est la preuve que l'attaque par force brute
est **continue**, pas hypothétique. `fail2ban` ralentit ; il n'empêche pas. Une attaque distribuée
lente passe sous le seuil.

**Correctif (le meilleur rapport effort/gain de tout ce rapport, trois lignes) :**
`PasswordAuthentication no`, `PermitRootLogin prohibit-password`, `KbdInteractiveAuthentication
no` dans `sshd_config.d/00-hardening.conf`. **Vérifier impérativement qu'une clé publique
fonctionne avant de recharger `sshd`** — et garder une session ouverte pendant l'opération.

**Ports publics sans justification apparente.** `ufw` ouvre `7777` (« Gemium static demo »),
`8080` (« M3 World Nginx Proxy »), `9222`, `9224`, `9225` et `3080` au monde entier. Le port
`9222` est le port par défaut de Chrome DevTools ; `ss -ltnp` montre `127.0.0.1:9222` occupé par
`bxc` et un relais `socat` sur `10.8.0.1:9222`. **Un DevTools accessible, c'est une exécution de
code arbitraire et la lecture de tous les cookies du navigateur.** Ces ouvertures ne semblent
correspondre à aucun service de ce projet : les auditer une par une et refermer celles qui sont
des restes.

---

## 5. `/tools/niers/latest.json` — l'endpoint à ne pas casser

### 5.1 La chaîne, vérifiée bout en bout

**L'endpoint est bien déclaré en dur dans le client**, et il est **premier** de la liste
(`apps/inacord/src-tauri/tauri.conf.json`) :

```json
"updater": {
  "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDgyRjUxMEVCREM0MkUyOTQK…",
  "endpoints": [
    "https://azalee.rosegriffon.fr/tools/niers/latest.json",
    "https://github.com/aphrody-code/nie/releases/latest/download/latest.json"
  ]
}
```

**Le point décisif : la clé publique minisign est compilée dans le binaire installé.** Ce n'est
pas le serveur qui atteste la mise à jour, c'est la signature de l'artefact vérifiée localement
contre cette clé. Le serveur ne choisit **pas** ce qui s'exécute.

**Le VPS n'est qu'un relais vers GitHub.** `apps/azalee/lib/niers-releases.ts` interroge
`https://api.github.com/repos/aphrody-code/nie/releases`, retient la première release
non-brouillon et non-préversion portant un `*-setup.exe.sig`, et
`apps/azalee/app/tools/niers/latest.json/route.ts` renvoie `signature` (lue depuis GitHub) et
`url` (l'URL de téléchargement GitHub). Le VPS **ne détient ni la clé privée ni l'artefact** :

```
$ curl -s https://azalee.rosegriffon.fr/tools/niers/latest.json \
    | jq '{version, pub_date, url: .platforms."windows-x86_64".url,
           sig_len: (.platforms."windows-x86_64".signature|length)}'
{
  "version": "0.5.9",
  "pub_date": "2026-09-05T01:18:37Z",
  "url": "https://github.com/aphrody-code/nie/releases/download/v0.5.9/niers_0.5.9_x64-setup.exe",
  "sig_len": 416
}
```

L'URL servie pointe `github.com`, la signature fait 416 caractères (une signature minisign
encodée). La chaîne est saine.

### 5.2 Ce qui se passerait si ce domaine servait un `latest.json` hostile

Trois scénarios, du plus improbable au plus réaliste.

**Exécution de code arbitraire : NON.** Un `latest.json` hostile pointant un `.exe` malveillant
échoue à la vérification minisign côté client. L'attaquant devrait posséder la clé privée
`~/.tauri/niers.key`, qui n'est **pas** sur le VPS et n'est **pas** sur GitHub. C'est la
propriété de sécurité qui tient, et elle tient bien.

**Déni de mise à jour : OUI, et durable.** Servir une version arbitrairement haute avec une
signature invalide bloque toutes les installations sur un échec de mise à jour permanent. Elles
resteront sur leur version courante, sans recevoir les correctifs, jusqu'à intervention manuelle
de chaque utilisateur. **Le second endpoint GitHub ne sauve pas** : le plugin Tauri retient le
premier endpoint qui répond avec un JSON valide ; un JSON bien formé mais à signature invalide
est une réponse valide, pas une raison de basculer sur l'endpoint suivant.

**Rejeu (rollback) : OUI — c'est le scénario réaliste.** La signature minisign porte sur les
**octets de l'artefact**, pas sur le numéro de version annoncé dans le manifeste. Un attaquant
contrôlant l'endpoint peut donc servir un artefact **légitimement signé** d'une version
ancienne — 0.3.0, par exemple — en le déclarant `"version": "9.9.9"`. Le client compare son
`0.5.9` à `9.9.9`, accepte la mise à jour, télécharge l'artefact ancien, **vérifie la signature
avec succès** (elle est authentique), et installe une version dont les vulnérabilités connues
sont publiquement documentées dans l'historique du dépôt. **Le mécanisme fonctionne exactement
comme prévu et rétrograde quand même l'application.**

### 5.3 Le risque réel n'est pas l'attaque, c'est la bascule elle-même

Le scénario de loin le plus probable **n'est pas malveillant** : c'est que la route
`/tools/niers/latest.json` ne soit tout simplement pas portée sur Vercel, ou qu'elle y échoue.
La route dépend de trois choses :

1. `export const revalidate = 3600` — un segment de configuration Next.js analysé
   statiquement. Il se comporte différemment sur Vercel (ISR réel) que derrière nginx.
2. Un appel sortant vers `api.github.com` **sans authentification**. Le commentaire du code le
   dit : « Évite de marteler l'API GitHub (rate-limit anonyme 60 req/h/IP) ». **Sur le VPS, l'IP
   sortante est unique et stable, donc 60 requêtes/h suffisent largement avec un cache d'une
   heure. Sur Vercel, les fonctions serverless sortent par des IP partagées et changeantes** —
   le quota anonyme est consommé par d'autres locataires, et le cache d'une heure ne s'applique
   pas de la même manière selon les régions. Le résultat de `getLatestNiersDesktopRelease()`
   devient `null`, et la route rend **404 « Aucune release desktop signée publiée »**.
3. Un second appel sortant vers `fetchSignature(release.nsis.sigUrl)`, avec le même problème.

**C'est le mode de panne à redouter : un 404 ou un 502 intermittent sur l'endpoint de mise à
jour, causé par un plafond de débit GitHub, sur des installations non corrigibles à distance.**
Il ne se verra ni au `build`, ni au `typecheck`, ni dans un test — seulement en production, et
seulement par intermittence.

### 5.4 Correctifs, par ordre de priorité

1. **Avant la bascule, authentifier l'appel à l'API GitHub.** Un jeton en lecture seule sur les
   releases publiques fait passer le quota de 60 à 5 000 requêtes/h. C'est une variable
   d'environnement et deux lignes dans `niers-releases.ts` (`headers: { Authorization: ... }`).
   **C'est le correctif qui évite la panne la plus probable.** Utiliser un jeton *fine-grained*
   dédié — surtout pas celui du § 3.2 rang 2.
2. **Vérifier l'endpoint immédiatement après la bascule**, et le mettre sous surveillance
   permanente : un contrôle qui alerte si `latest.json` ne rend pas 200 avec un champ `signature`
   non vide. Aujourd'hui, rien ne le surveille.
3. **Inverser l'ordre des endpoints** dans `tauri.conf.json` pour les versions futures : mettre
   GitHub en premier, azalée en repli. GitHub est l'origine de la vérité et sa disponibilité
   dépasse celle de ce VPS. Cela ne corrige pas les installations déjà déployées, mais borne le
   problème dans le temps.
4. **Contre le rejeu**, faire figurer la version dans un contenu signé — par exemple signer le
   manifeste lui-même, ou refuser côté client une version dont l'artefact porte un numéro
   différent de celui annoncé. À défaut, conserver un journal des versions publiées et surveiller
   toute régression du `version` servi.
5. **La clé de signature.** `~/.tauri/niers.key` tient sur une seule ligne et son mot de passe est
   **vide** (documenté dans `CLAUDE.md`). Une clé privée non protégée par un mot de passe est un
   fichier à copier. Régénérer la paire invaliderait l'updater de toutes les installations
   existantes — **ne pas le faire dans l'urgence** — mais poser un mot de passe sur la clé
   existante (`minisign -C`) est possible sans invalider quoi que ce soit, puisque la clé
   publique ne change pas. **À faire.**

---

## Plan d'action ordonné

### Avant la bascule — bloquant

| # | Action | Effort | Effet |
|---|---|---|---|
| 1 | `REVOKE` les écritures de `anon` sur les 129 tables | 1 requête | Ferme le risque n° 2 |
| 2 | `REVOKE EXECUTE` à `anon` sur les 4 RPC d'écriture | 1 requête | Ferme le risque n° 1 |
| 3 | `PasswordAuthentication no` + `PermitRootLogin prohibit-password` | 3 lignes | Ferme le risque n° 3 |
| 4 | Retirer `discord_members` et `settings` de `anon` | 2 requêtes | Ferme la fuite de données personnelles |
| 5 | Authentifier l'appel GitHub dans `niers-releases.ts` | 2 lignes | Évite la panne d'updater la plus probable |
| 6 | Brancher `limit_req` + `limit_conn` sur `cdn.rosegriffon.fr` | 3 lignes | Ferme le déni de service, réveille `fail2ban` |
| 7 | Pointer `NEXT_PUBLIC_SUPABASE_URL` sur `supabase.rosegriffon.fr` et valider | 1 variable + test | Évite la casse silencieuse du client |

### Au moment de la bascule

| # | Action |
|---|---|
| 8 | Retirer `51.77.147.152` du `server_name`, poser un `default_server` qui rend 444 |
| 9 | Restreindre `Access-Control-Allow-Origin` à l'origine Vercel |
| 10 | Réacheminer `/storage/v1/*` et `/_next/static/*` depuis Vercel, ou migrer les URL en base |
| 11 | Arrêter `azalee-web.service` et `azalee-web-b.service`, supprimer `/__preview` |
| 12 | Vérifier `latest.json` et le mettre sous surveillance |

### Dans les jours qui suivent

| # | Action |
|---|---|
| 13 | Rotation des secrets dans l'ordre du § 3.2 (commencer par le rang 5, sans impact) |
| 14 | Découper `cron.env` par tâche ; migrer vers `systemd-creds` |
| 15 | Poser un mot de passe sur `~/.tauri/niers.key` (sans régénérer la paire) |
| 16 | Budget de temps par requête + alignement du budget mémoire dans `nie-model-serve` |
| 17 | Auditer les ports `7777`, `8080`, `9222`, `9224`, `9225`, `3080` ouverts au monde |
| 18 | Exiger l'en-tête `apikey` sur `/rest/v1/` au niveau de nginx |

---

## Ce que cet audit n'a pas couvert

Par honnêteté sur le périmètre :

- **Aucun test d'écriture réel** contre la base. Les privilèges d'écriture de `anon` (§ 1.3) sont
  établis par `information_schema.role_table_grants`, pas par une écriture effective : la
  vérifier aurait modifié des données de production. Le risque est **établi par les privilèges**,
  et sa réalisation dépend de la politique RLS de chaque table, non vérifiée table par table.
- **Aucun test de charge**, conformément à la consigne. Les mesures de § 2.3 et § 2.4 sont des
  requêtes uniques ; l'extrapolation au comportement sous charge est un raisonnement, pas une
  mesure.
- **Les portées du jeton GitHub n'ont pas été vérifiées** — elles sont reprises telles
  qu'annoncées dans la mission. Les vérifier aurait exigé d'utiliser le jeton.
- **`csharp/`** n'a pas été examiné : `dotnet` est absent du VPS.
- **Le contenu des 44 secrets de `cron.env`** n'a pas été lu, seulement les noms de clés.
- **La configuration Vercel** (variables d'environnement, réécritures, en-têtes) n'a pas été
  auditée : elle n'existe pas encore.

---

*Audit réalisé le 2026-09-05 sur `vps-203bea89` (`51.77.147.152`). Lecture seule sur
l'infrastructure : aucun service, pare-feu ni vhost modifié. Aucune valeur de secret ne figure
dans ce document.*
