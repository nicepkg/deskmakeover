# Signs ONE Windows artifact with the Certum code-signing certificate that SimplySign Desktop
# exposes to the CurrentUser\My store. Invoked by Tauri's bundle.windows.signCommand (release
# config) once per artifact — Tauri calls it for the app .exe AND for the NSIS installer, so both
# ship signed. Runs on the SELF-HOSTED signing runner only; see docs/signing-setup.md.
#
# Requires:
#   - SimplySign Desktop LOGGED IN (so the cloud cert is present in CurrentUser\My)
#   - DM_SIGN_THUMBPRINT env var = the SHA-1 thumbprint of that cert (repo variable in CI)
#   - The Windows 10/11 SDK (signtool.exe)
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$File)

$ErrorActionPreference = 'Stop'

$thumb = $env:DM_SIGN_THUMBPRINT
if (-not $thumb) {
    throw 'DM_SIGN_THUMBPRINT is not set — cannot select the signing certificate.'
}

# Newest signtool.exe from any installed Windows SDK (x64).
$signtool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe' -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $signtool) {
    throw 'signtool.exe not found — install the Windows 10/11 SDK on this runner.'
}

# /sha1 selects the exact cert; RFC-3161 timestamp via Certum so signatures outlive the cert.
& $signtool sign /sha1 $thumb /fd sha256 /tr http://time.certum.pl /td sha256 /v "$File"
if ($LASTEXITCODE -ne 0) {
    throw "signtool failed on $File (exit $LASTEXITCODE) — is SimplySign Desktop logged in?"
}
