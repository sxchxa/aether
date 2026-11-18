# Rassemble l'application compilée dans un dossier clair sur le Bureau.
# À relancer après chaque « npx tauri build ».
param(
  [string]$Destination = (Join-Path ([Environment]::GetFolderPath("Desktop")) "Aether - Application")
)

$ErrorActionPreference = "Stop"
$release = Join-Path $PSScriptRoot "..\src-tauri\target\release" | Resolve-Path
$version = (Get-Content (Join-Path $PSScriptRoot "..\src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json).version

$exe = Join-Path $release "aether.exe"
$setup = Get-ChildItem (Join-Path $release "bundle\nsis") -Filter "Aether_${version}_x64-setup.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not (Test-Path $exe) -or -not $setup) { throw "Compilation introuvable : lancez d'abord « npx tauri build »." }

# Un exe en cours d'utilisation ne peut pas être supprimé, mais il peut être déplacé :
# il part dans le dossier temporaire et le dossier reste propre.
if (Test-Path $Destination) {
  $parking = Join-Path $env:TEMP "aether-anciennes-versions"
  New-Item -ItemType Directory -Force $parking | Out-Null
  foreach ($f in Get-ChildItem $Destination -Recurse -File) {
    try { Remove-Item $f.FullName -Force -ErrorAction Stop }
    catch { Move-Item $f.FullName (Join-Path $parking ("{0}-{1}" -f (Get-Date -Format "yyyyMMddHHmmss"), $f.Name)) -Force }
  }
  Remove-Item $Destination -Recurse -Force -ErrorAction SilentlyContinue
}
New-Item -ItemType Directory -Force $Destination | Out-Null

Copy-Item $setup.FullName (Join-Path $Destination "Installer Aether $version.exe")
Copy-Item $exe (Join-Path $Destination "Aether $version (portable).exe")

@"
AETHER $version
===============

Installer Aether $version.exe
    L'installateur à utiliser et à partager. Il installe Aether,
    crée les raccourcis et permet la désinstallation depuis Windows.

Aether $version (portable).exe
    L'application seule, sans installation : double-clic pour la lancer.
    Pratique pour tester. Nécessite WebView2, déjà présent sur Windows 11.

Ce dossier est recréé à chaque compilation par scripts\collect-release.ps1.
Le code source prêt pour GitHub se trouve dans le dossier « aether » du Bureau.
"@ | Out-File (Join-Path $Destination "LISEZ-MOI.txt") -Encoding utf8

Write-Host "Dossier prêt : $Destination"
