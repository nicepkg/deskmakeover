# Builds the shippable v1.0 release: one self-contained single-file DeskMakeover.App.exe
# that runs on any Windows 10/11 machine with NO .NET install required.
#
#   pwsh scripts/dev/publish.ps1
#
# Output: publish/DeskMakeover-v<version>-win-x64/DeskMakeover.App.exe (~63 MB).
param([string]$OutDir = "")

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$dotnet = Join-Path $repo ".dotnet\dotnet.exe"
$env:DOTNET_ROOT = Join-Path $repo ".dotnet"

$version = (Select-String -Path (Join-Path $repo "Directory.Build.props") -Pattern '<Version>(.*?)</Version>').Matches[0].Groups[1].Value
if (-not $OutDir) { $OutDir = Join-Path $repo "publish\DeskMakeover-v$version-win-x64" }

Write-Host "Publishing DeskMakeover v$version (self-contained, single file) -> $OutDir"
& $dotnet publish (Join-Path $repo "src\DeskMakeover.App\DeskMakeover.App.csproj") `
    -p:PublishProfile=win-x64 -o $OutDir

# Ship exactly one file — strip the debug symbols the referenced projects leave behind.
Get-ChildItem -Path $OutDir -Filter *.pdb -ErrorAction SilentlyContinue | Remove-Item -Force

$exe = Join-Path $OutDir "DeskMakeover.App.exe"
if (Test-Path $exe) {
    $sizeMb = [math]::Round((Get-Item $exe).Length / 1MB, 1)
    Write-Host "OK -> $exe  ($sizeMb MB, no .NET install required)" -ForegroundColor Green
} else {
    throw "Publish did not produce $exe"
}
