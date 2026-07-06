# Windows-only visual-verification helper for the WPF app.
# Launches the built DeskMakeover.App exe (or attaches to a running one),
# waits for it to render, captures its window to a PNG, then (optionally) closes it.
# Visual parity verification is win-x64 only by design (plan P0/§Non-negotiables).
#
# Usage:
#   pwsh scripts/dev/capture-window.ps1 -Out shot.png [-Wait 2500] [-Keep] [-Exe path]
param(
  [string]$Out = "shot.png",
  [int]$Wait = 2600,
  [switch]$Keep,           # leave the app running after capture
  [switch]$Attach,         # attach to an already-running instance, don't launch
  [string]$Exe = "",
  [int]$Width = 0,         # optional: resize the window to WxH before capturing
  [int]$Height = 0,
  [string]$Click = ""      # optional: comma-separated UIA button Names to invoke before capture
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $Exe) {
  $Exe = Join-Path $root "src\DeskMakeover.App\bin\Debug\net10.0-windows\DeskMakeover.App.exe"
}

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool repaint);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$proc = $null
if (-not $Attach) {
  if (-not (Test-Path $Exe)) { throw "exe not found: $Exe (build first)" }
  $proc = Start-Process -FilePath $Exe -PassThru
  Start-Sleep -Milliseconds $Wait
} else {
  Start-Sleep -Milliseconds 200
}

# find the main window handle for the DeskMakeover process
$target = Get-Process -Name "DeskMakeover.App" -ErrorAction SilentlyContinue |
  Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $target) { throw "no DeskMakeover.App window found" }
$h = $target.MainWindowHandle

if ($Width -gt 0 -and $Height -gt 0) {
  [void][Win]::MoveWindow($h, 60, 40, $Width, $Height, $true)
  Start-Sleep -Milliseconds 500
}

[void][Win]::SetForegroundWindow($h)
Start-Sleep -Milliseconds 350

if ($Click) {
  Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
  $win = [System.Windows.Automation.AutomationElement]::FromHandle($h)
  foreach ($name in ($Click -split ',')) {
    $cond = New-Object System.Windows.Automation.PropertyCondition(
      [System.Windows.Automation.AutomationElement]::NameProperty, $name.Trim())
    $btn = $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
    if ($btn) {
      $inv = $btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
      $inv.Invoke()
      Start-Sleep -Milliseconds 600
    } else {
      Write-Output "warn: UIA button '$name' not found"
    }
  }
}
$r = New-Object Win+RECT
[void][Win]::GetWindowRect($h, [ref]$r)
$w = $r.Right - $r.Left
$ht = $r.Bottom - $r.Top
if ($w -le 0 -or $ht -le 0) { throw "bad window rect ${w}x${ht}" }

$bmp = New-Object System.Drawing.Bitmap $w, $ht
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $ht))
$outPath = if ([System.IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path (Get-Location) $Out }
$bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "captured ${w}x${ht} -> $outPath"

if (-not $Keep -and $proc) {
  try { $proc.CloseMainWindow() | Out-Null; Start-Sleep -Milliseconds 300 } catch {}
  try { if (-not $proc.HasExited) { $proc.Kill() } } catch {}
}
