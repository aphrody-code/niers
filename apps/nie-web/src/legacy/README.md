# `legacy/` — ce qui vient du wiki et n'a pas encore ete porte

Ce dossier accueille, **tel quel**, le code deplace d'`apps/azalee` par le lot
J2 : les pages et bibliotheques qui lisaient un fichier local (`/cpk`,
`/textures`, `/modeles`, `/mode`, `/sons`, `/videos`, `/avatar`, `/demo`,
`/save`, `/vroid`, `lib/cpk`, `lib/cutin`, `api/cpk`, `api/mode-tex`).

Il est **exclu du `tsconfig.json`** : rien ici n'est compile ni servi. C'est un
sas, pas une bibliotheque.

**Pourquoi un sas plutot qu'une suppression.** Ces pages marchent aujourd'hui en
production ; les jeter pour tenir une gate ferait passer une regression pour un
progres. Elles seront reecrites contre les routes de `nie-site` (`/f/<chemin
VFS>`, `/b/<prefixe>`, `/api/v1/<vue>`, `/assets/<chemin>`), qui rendent depuis
le serveur Rust ce qu'elles lisaient depuis le disque.

**Ce qui change en les portant** : plus aucun acces disque cote client, le
chemin VFS voyage en segment et verbatim (amendement A3), et les vues nommees
sont des filtres enregistres — jamais des fichiers.

TODO date du 2026-09-05 : porter, puis vider ce dossier.
