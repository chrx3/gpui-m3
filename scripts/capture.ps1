# Photographs a gallery section for the release notes, in light and dark, into
# changelog/<Version>/<Name>-<mode>.png. Windows only; it opens the gallery, takes the picture and
# closes only that window. Build the gallery first: cargo build --example gallery
#
#   .\scripts\capture.ps1 -Version 0.2.0 -Section 0 -Name buttons
#   .\scripts\capture.ps1 -Version 0.2.0 -Section 4 -Name shapes -Modes dark -Width 1100 -Height 760
#
# -Section is the gallery section number (M3_SECTION), -Seed one of the nine seed colors (M3_SEED),
# -Variant 0-8 a dynamic color variant (M3_VARIANT) and -Contrast -1 to 1 (M3_CONTRAST).

param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][int]$Section,
    [Parameter(Mandatory)][string]$Name,
    [string[]]$Modes = @("light", "dark"),
    [int]$Width = 1280,
    [int]$Height = 860,
    [int]$Seed = 0,
    [int]$Variant = 0,
    [double]$Contrast = 0,
    [int]$DelayMs = 3000,
    [string]$Exe
)

$ErrorActionPreference = "Stop"
# "dark,light" arrives as one string when the script runs with -File.
$Modes = @($Modes | ForEach-Object { $_ -split "," })
$repo = Split-Path -Parent $PSScriptRoot

if (-not $Exe) {
    $target = (cargo metadata --no-deps --format-version 1 --manifest-path (Join-Path $repo "Cargo.toml") | ConvertFrom-Json).target_directory
    $Exe = Join-Path $target "debug\examples\gallery.exe"
}
if (-not (Test-Path $Exe)) { throw "No gallery at $Exe. Build it: cargo build --example gallery" }

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
}
"@
# Without this Windows scales the coordinates (125 % screens) and the picture comes out cropped.
[Win]::SetProcessDPIAware() | Out-Null

$out = Join-Path $repo "changelog\$Version"
New-Item -ItemType Directory -Force $out | Out-Null

foreach ($mode in $Modes) {
    $env:M3_SECTION = "$Section"
    $env:M3_MODE = $mode
    $env:M3_SEED = "$Seed"
    $env:M3_VARIANT = "$Variant"
    $env:M3_CONTRAST = [string]::Format([cultureinfo]::InvariantCulture, "{0}", $Contrast)
    $env:M3_SIZE = "${Width}x${Height}"
    $process = Start-Process -FilePath $Exe -PassThru
    try {
        $deadline = (Get-Date).AddSeconds(30)
        while ($process.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $process.Refresh()
        }
        if ($process.MainWindowHandle -eq 0) { throw "The gallery opened no window" }
        Start-Sleep -Milliseconds $DelayMs
        $hwnd = $process.MainWindowHandle

        $window = New-Object Win+RECT
        $client = New-Object Win+RECT
        [Win]::GetWindowRect($hwnd, [ref]$window) | Out-Null
        [Win]::GetClientRect($hwnd, [ref]$client) | Out-Null
        $origin = New-Object Win+POINT
        [Win]::ClientToScreen($hwnd, [ref]$origin) | Out-Null

        $full = New-Object System.Drawing.Bitmap ($window.Right - $window.Left), ($window.Bottom - $window.Top)
        $graphics = [System.Drawing.Graphics]::FromImage($full)
        $dc = $graphics.GetHdc()
        [Win]::PrintWindow($hwnd, $dc, 2) | Out-Null
        $graphics.ReleaseHdc($dc)
        $graphics.Dispose()

        # Only the client area: no title bar or border.
        $area = New-Object System.Drawing.Rectangle ($origin.X - $window.Left), ($origin.Y - $window.Top), ($client.Right - $client.Left), ($client.Bottom - $client.Top)
        $picture = $full.Clone($area, $full.PixelFormat)
        $file = Join-Path $out "$Name-$mode.png"
        $picture.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
        $picture.Dispose()
        $full.Dispose()
        Write-Output $file
    }
    finally {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
}
