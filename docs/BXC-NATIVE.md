# BXC natif dans niers

La couche métier `ietv`, `zukan`, `wonderbot` et `iecrawl` reste dans niers,
mais le moteur navigateur BXC reste consommé depuis le registre npm et par le
binaire natif standalone. Cette séparation évite de recopier le dépôt BXC,
ses données privées, ses cookies ou ses profils.

## Installation Windows vérifiée

Le tag BXC `v0.9.7` est actuellement une release source sans asset GitHub.
Le binaire a donc été construit depuis le checkout BXC tagué, puis installé
dans `%USERPROFILE%\.bxc\bin\bxc.exe`. Vérification effectuée :

```powershell
bxc --version # bxc 0.9.7
```

Le chemin `%USERPROFILE%\.bxc\bin` doit précéder les anciens chemins BXC dans
le PATH utilisateur. Les appels de recon peuvent être forcés explicitement :

```powershell
$env:BXC_BIN = "$env:USERPROFILE\.bxc\bin\bxc.exe"
$env:BXC_CWD = "$env:USERPROFILE\.bxc\bin"
```

`BXC_CWD` est important dans ce monorepo : le binaire standalone ne doit pas
hériter du `bunfig.toml` de niers, qui précharge le plugin de formats IEVR.

## Paquet TypeScript

Les imports `@aphrody/bxc` restent intentionnellement sur `0.9.6`, dernière
version effectivement publiée au registre au moment de cette migration. Ils
seront relevés vers `0.9.7` dès que le paquet npm correspondant sera publié ;
le CLI natif vérifié est déjà `0.9.7`.

Les secrets, cookies, profils CDP et bases BXC restent hors dépôt.
