# Construit l'installateur de Retro EQ avec Inno Setup.
#
# Utilisé à la main (voir installateur/README.md) et par l'intégration continue :
# une seule recette, donc le même résultat aux deux endroits.
#
#   .\installateur\construire.ps1                  # exécutable : target\release\retro-eq.exe
#   .\installateur\construire.ps1 -Exe C:\autre\retro-eq.exe
#
# La version est lue dans Cargo.toml (workspace.package.version) : c'est la seule source.
# Résultat : installateur\sortie\retro-eq-<version>-installateur-x64.exe
param(
    [string]$Exe = "target\release\retro-eq.exe"
)
$ErrorActionPreference = 'Stop'

$racine = Split-Path -Parent $PSScriptRoot
$script = Join-Path $PSScriptRoot 'retro-eq.iss'
$sortie = Join-Path $PSScriptRoot 'sortie'

# 1. La version, depuis Cargo.toml. La première ligne « version = "..." » est celle de
#    [workspace.package] : il n'y en a pas d'autre dans ce fichier.
$ligne = Select-String -Path (Join-Path $racine 'Cargo.toml') -Pattern '^\s*version\s*=\s*"([^"]+)"' |
    Select-Object -First 1
if (-not $ligne) { throw "Version introuvable dans Cargo.toml" }
$version = $ligne.Matches[0].Groups[1].Value

# 2. L'exécutable doit déjà être compilé : cargo build --release -p retro-app
if (-not [System.IO.Path]::IsPathRooted($Exe)) { $Exe = Join-Path $racine $Exe }
if (-not (Test-Path $Exe)) {
    throw "Executable introuvable : $Exe (lancer d'abord : cargo build --release -p retro-app)"
}

# 3. Le compilateur d'Inno Setup (ISCC.exe) : sur le PATH, ou dans un dossier d'installation
#    habituel. Sur une machine d'intégration continue, on l'installe avec Chocolatey s'il manque.
function Trouver-Iscc {
    $commande = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($commande) { return $commande.Source }
    foreach ($base in @(${env:ProgramFiles(x86)}, $env:ProgramFiles, "$env:LOCALAPPDATA\Programs")) {
        if (-not $base) { continue }
        $trouve = Get-ChildItem -Path (Join-Path $base 'Inno Setup*') -Filter ISCC.exe -ErrorAction SilentlyContinue |
            Sort-Object FullName -Descending | Select-Object -First 1
        if ($trouve) { return $trouve.FullName }
    }
    return $null
}
$iscc = Trouver-Iscc
if (-not $iscc -and $env:CI) {
    Write-Host "ISCC.exe absent : installation d'Inno Setup avec Chocolatey"
    choco install innosetup -y --no-progress
    if ($LASTEXITCODE -ne 0) { throw "choco install innosetup a echoue (code $LASTEXITCODE)" }
    $iscc = Trouver-Iscc
}
if (-not $iscc) {
    throw "Inno Setup 6.3 ou plus est introuvable. Installe-le : https://jrsoftware.org/isdl.php"
}
Write-Host "Inno Setup : $iscc"
Write-Host "Version    : $version"
Write-Host "Executable : $Exe"

# 4. Compilation.
& $iscc "/DAppVersion=$version" "/DExeSource=$Exe" "/DDossierSortie=$sortie" $script
if ($LASTEXITCODE -ne 0) { throw "ISCC.exe a echoue (code $LASTEXITCODE)" }

$resultat = Join-Path $sortie "retro-eq-$version-installateur-x64.exe"
if (-not (Test-Path $resultat)) { throw "Installateur attendu introuvable : $resultat" }
Write-Host "Installateur : $resultat"
