# Desktop et mobile — Tauri et React/Vite

> **Gelé le 2026-09-05.** La partie desktop est exécutée cette semaine (J4–J5 de
> [`/PLAN.md`](../../PLAN.md)) ; la partie mobile est **hors semaine** et ne change que par
> amendement daté de l'ADR.

## Décision

L'explorateur s'appelle **Inacord** (`apps/inacord`, ex `apps/inacord`) et reste une
application React/Vite. Son interface est extraite dans `packages/inacord-ui`, montée par
deux hôtes : `Tauri 2` pour le bureau (et plus tard Android/iOS), `apps/nie-web` pour le
site **Aphrody** (`aphrody.com`) servi par `nie-site`. Les deux implémentent le même contrat
`packages/asset-source` et portent la même DA, celle du vrai jeu. `productName` devient
`Inacord` ; l'identifiant `dev.niers.explorer` et les URL de l'updater ne changent pas.
**Leptos n'est pas retenu** : une seconde pile d'UI ne partagerait rien avec Inacord.

Tauri utilise les webviews système : WKWebView sur macOS/iOS, WebView2 sur
Windows, WebKitGTK sur Linux et WebView système sur Android. Cette propriété
est adaptée à un studio/outillage, pas à la boucle de rendu du jeu.

## Matrice de livraison

- **Web studio** — `apps/nie-web`, bundle React/Vite partagé servi par `nie-site`;
  API `/api/v1` HTTPS; cache explicite.
- **Inacord desktop** — React/Vite dans Tauri; API distante et fonctions
  locales; offline selon l'écran.
- **Inacord Android/iOS** — même frontend et capabilities mobiles; API
  distante; cache limité et sécurisé; hors semaine.
- **Jeu desktop/mobile** — wgpu/winit natif et runtime local; cœur offline.

## Règles de sécurité Tauri

- capabilities minimales par fenêtre et par plateforme;
- CSP et origines explicites; pas de `allow-all` en production;
- commandes Tauri limitées à des DTO validés, sans exécuter un chemin fourni
  par l'UI;
- tokens dans le keychain/Keystore/Keychain via stockage sécurisé, jamais dans
  `localStorage` exportable si un secret long terme est en jeu;
- aucune clé Supabase service-role, mot de passe PostgreSQL ou clé Steamworks
  dans le frontend;
- sidecars et accès fichiers autorisés uniquement aux répertoires nécessaires;
- téléchargement de CPK/asset avec contrôle de taille, chemin canonique et
  destination non traversable.

## Commandes de validation Tauri

Les commandes officielles à activer dans le package de l'explorer sont :

```bash
bun tauri dev
bun tauri android dev
bun tauri ios dev
```

Elles constituent des gates à exécuter sur les toolchains natives présentes;
ce dossier ne prétend pas qu'une build Android/iOS est déjà verte. Ajouter
ensuite une build release par architecture et un smoke test sur appareil réel.

## Relation avec `nie-site`

Le serveur Rust expose une API stable et des assets contrôlés. Il ne force pas le
studio à réimplémenter ses composants React : le contrat de partage
(`packages/asset-source`) est prioritaire. Les seules pages rendues côté serveur
(`askama`) sont des coquilles — `index.html` enrichi de ses balises `og:`, erreurs,
`robots.txt`, `sitemap.xml` — jamais des écrans.

Les uploads, previews 3D et gros fichiers doivent être streamés/paginés; une
page mobile ne doit pas charger le catalogue complet des 250 000+ fichiers.
