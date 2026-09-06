#!/usr/bin/env pwsh
# build-wasm.ps1 — port PowerShell 7 de build-wasm.sh (le .sh reste la référence sur le VPS).
#
# Build reproductible de la surface WebAssembly (nie-wasm) pour azalee.
#
# Étapes (best practices wasm-bindgen) :
#   1. cargo build --release --target wasm32-unknown-unknown
#   2. wasm-bindgen --target web            (glue ESM + .d.ts)
#   3. wasm-opt -O3                          (taille -~15 % + vitesse runtime)
#   4. patch Turbopack : `new URL(..., import.meta.url)` -> throw (force module_or_path)
#   5. copie vers azalee (lib/nie-wasm-web/ + public/wasm/)
#
# La version du CLI wasm-bindgen DOIT égaler le pin du workspace (cf. Cargo.toml).

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# Chaque appel natif est testé explicitement sur $LASTEXITCODE.
$PSNativeCommandUseErrorActionPreference = $false

# Chemins dérivés (portables) : Root = racine du dépôt (parent de scripts/),
# Azalee surchargeable par env. Défaut = dépôt `rg` côte-à-côte du $HOME.
$Root = if ($env:NIE_ROOT) { $env:NIE_ROOT } else { (Resolve-Path (Join-Path $PSScriptRoot '..')).Path }
$MaisonDefaut = if ($env:HOME) { $env:HOME } else { $env:USERPROFILE }
$Azalee = if ($env:AZALEE_DIR) { $env:AZALEE_DIR } else { Join-Path $MaisonDefaut 'rg/apps/azalee' }
$Pkg = Join-Path $Root 'crates/engine/nie-wasm/pkg'
$Wasm = Join-Path $Root 'target/wasm32-unknown-unknown/release/nie_wasm.wasm'

Set-Location -LiteralPath $Root

function Assert-Exit([string] $Etape) {
    if ($LASTEXITCODE -ne 0) {
        [Console]::Error.WriteLine("ERREUR: $Etape a échoué (code $LASTEXITCODE).")
        exit $LASTEXITCODE
    }
}

# 0. Garde-fou : versions alignées.
$cargoToml = Get-Content -LiteralPath (Join-Path $Root 'Cargo.toml') -Raw
$pinMatch = [regex]::Match($cargoToml, 'wasm-bindgen = \{ version = "=([0-9.]+)"')
$pin = if ($pinMatch.Success) { $pinMatch.Groups[1].Value } else { '' }

if (-not (Get-Command wasm-bindgen -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine('ERREUR: wasm-bindgen CLI introuvable sur le PATH.')
    [Console]::Error.WriteLine("  Installe la version épinglée : cargo install wasm-bindgen-cli --version $pin")
    exit 1
}
$sortieCli = & wasm-bindgen --version 2>&1
$codeCli = $LASTEXITCODE
if ($codeCli -ne 0) {
    [Console]::Error.WriteLine("ERREUR: wasm-bindgen --version a échoué (code $codeCli).")
    exit 1
}
$cliMatch = [regex]::Match(($sortieCli -join "`n"), '[0-9.]+')
$cli = if ($cliMatch.Success) { $cliMatch.Value } else { '' }

if ($pin -ne $cli) {
    Write-Host "ERREUR: wasm-bindgen CLI $cli != pin $pin"
    exit 1
}

Write-Host '[1/5] cargo build release wasm32…'
& cargo build -p nie-wasm --target wasm32-unknown-unknown --release
Assert-Exit 'cargo build'

Write-Host '[2/5] wasm-bindgen --target web…'
& wasm-bindgen $Wasm --out-dir $Pkg --target web
Assert-Exit 'wasm-bindgen'

Write-Host '[3/5] wasm-opt -O3…'
$bgWasm = Join-Path $Pkg 'nie_wasm_bg.wasm'
$before = (Get-Item -LiteralPath $bgWasm -Force).Length
& wasm-opt -O3 $bgWasm -o $bgWasm
Assert-Exit 'wasm-opt'
$after = (Get-Item -LiteralPath $bgWasm -Force).Length
$gain = [math]::Truncate((($before - $after) * 100) / $before)   # division entière, comme en bash
Write-Host "      $before -> $after octets ($gain % en moins)"

Write-Host '[4/5] patch Turbopack…'
# Équivalent du `sed -i` : substitution littérale, sans réencodage ni changement de fins de ligne
# (lecture/écriture en UTF-8 sans BOM, contenu brut).
$jsPath = Join-Path $Pkg 'nie_wasm.js'
$avant = "module_or_path = new URL('nie_wasm_bg.wasm', import.meta.url);"
$apres = 'throw new Error("nie-wasm: module_or_path requis");'
$contenu = [System.IO.File]::ReadAllText($jsPath)
$contenu = $contenu.Replace($avant, $apres)
[System.IO.File]::WriteAllText($jsPath, $contenu, (New-Object System.Text.UTF8Encoding($false)))

Write-Host '[5/5] copie vers azalee…'
Copy-Item -LiteralPath $jsPath -Destination (Join-Path $Azalee 'lib/nie-wasm-web/') -Force
Copy-Item -LiteralPath (Join-Path $Pkg 'nie_wasm.d.ts') -Destination (Join-Path $Azalee 'lib/nie-wasm-web/') -Force
Copy-Item -LiteralPath $bgWasm -Destination (Join-Path $Azalee 'public/wasm/') -Force

Write-Host "OK — wasm déployable ($after octets). Rebuild azalee + deploy ensuite."
