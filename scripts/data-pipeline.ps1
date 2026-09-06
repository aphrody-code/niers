#!/usr/bin/env pwsh
# Chaîne de bout en bout les données du jeu, en n'appelant QUE les commandes publiées dans le PATH
# (`just installer`) et les CLI des paquets — jamais `./target/release/...` ni `bun run` en dur.
# Portage PowerShell 7 de scripts/data-pipeline.sh, qui reste la référence sur le VPS Linux.
#
# Pourquoi le PATH : un chemin `target/release/x` en dur est faux dès qu'on lance le script d'un
# autre répertoire, et il masque le fait qu'une commande n'a jamais été publiée. Le PATH échoue
# franchement, tout de suite, avec le nom manquant.
#
# Usage : pwsh -NoProfile -File scripts/data-pipeline.ps1 [--verif-seule]

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$sec = if ($args.Count -ge 1) { [string]$args[0] } else { '' }
$racine = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not $env:NIE_GAME_DIR) { $env:NIE_GAME_DIR = $racine }

$manquants = [System.Collections.Generic.List[string]]::new()
$echecs = [System.Collections.Generic.List[string]]::new()

# Sans cela, PowerShell 7 retire les séquences ANSI dès que la sortie est redirigée.
if ($PSStyle) { $PSStyle.OutputRendering = 'Ansi' }
# Les binaires du dépôt écrivent en UTF-8 : sans cela PowerShell les décode en page OEM.
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

$ESC = [char]27
function titre([string]$m) { Write-Host ''; Write-Host "$ESC[1m$m$ESC[0m" }

titre '1. Les commandes attendues sont-elles publiées ?'
foreach ($c in @('niers', 'nie-catalog', 'export_skills', 'export_passives', 'export_formations', 'export_aphrody')) {
    $cmd = Get-Command -Name $c -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cmd) {
        Write-Host ('  ✓ {0,-20} {1}' -f $c, $cmd.Source)
    } else {
        Write-Host ('  ✗ {0,-20} ABSENT du PATH' -f $c)
        $manquants.Add($c)
    }
}
if ($manquants.Count -gt 0) {
    Write-Host ''
    Write-Host "→ $($manquants.Count) commande(s) absente(s). Lance : just installer"
    exit 1
}

titre '2. Les quatre gisements répondent-ils ? (paquet @niers/catalog)'
# `nie-catalog etat` MESURE le contenu : un gisement présent peut être vide. Le lanceur se place
# à la racine du dépôt, sans quoi `extrait` et `re` sont annoncés vides — faux négatif vécu.
# Code de sortie du natif lu immédiatement, sans pipe intermédiaire. $LASTEXITCODE est
# pré-armé : sous Set-StrictMode, le lire alors qu'aucun natif n'a encore tourné dans la
# session fait planter le script au lieu de compter un échec.
$global:LASTEXITCODE = 0
& (Get-Command nie-catalog -CommandType Application | Select-Object -First 1).Source etat
if ($LASTEXITCODE -ne 0) { $echecs.Add('nie-catalog etat') }

if ($sec -eq '--verif-seule') {
    Write-Host ''
    Write-Host 'vérification seule : rien exporté.'
    exit 0
}

titre '3. Exports (binaires nie-data du PATH)'
# Ces binaires n'ont pas de --help et résolvent le jeu par resolve_game_dir() : sans NIE_GAME_DIR
# ils échouent hors du dépôt. Elle est posée en tête de script.
foreach ($e in @('export_skills', 'export_passives', 'export_formations', 'export_aphrody')) {
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $exe = (Get-Command -Name $e -CommandType Application | Select-Object -First 1).Source
    $global:LASTEXITCODE = 0
    $sortie = & $exe 2>&1
    $rc = $LASTEXITCODE
    $chrono.Stop()
    $secs = [int]$chrono.Elapsed.TotalSeconds
    $lignes = @($sortie | ForEach-Object { [string]$_ })
    $derniere = if ($lignes.Count -gt 0) { $lignes[-1] } else { '' }
    if ($derniere.Length -gt 88) { $derniere = $derniere.Substring(0, 88) }
    if ($rc -eq 0) {
        Write-Host ('  ✓ {0,-20} {1,3}s  {2}' -f $e, $secs, $derniere)
    } else {
        Write-Host ('  ✗ {0,-20} {1,3}s  {2}' -f $e, $secs, $derniere)
        $echecs.Add($e)
    }
}

titre '4. Ce qui a été écrit'
$exportDir = Join-Path $racine 'export'
if (Test-Path -LiteralPath $exportDir -PathType Container) {
    $limite = (Get-Date).AddMinutes(-10)
    $lignes = Get-ChildItem -LiteralPath $exportDir -File |
        Where-Object { $_.LastWriteTime -gt $limite } |
        ForEach-Object { '  {0,-42} {1,10} o' -f $_.Name, $_.Length }
    foreach ($l in ($lignes | Sort-Object)) { Write-Host $l }
} else {
    Write-Host '  (aucun répertoire export/)'
}

Write-Host ''
if ($echecs.Count -gt 0) {
    Write-Host ('ÉCHECS ({0}) : {1}' -f $echecs.Count, ($echecs -join ' '))
    exit 1
}
Write-Host 'pipeline complet — 4 exports, 4 gisements.'
exit 0
