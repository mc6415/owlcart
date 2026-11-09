# Downloads the official static aria2c.exe and places it where Tauri expects
# the sidecar binary. Skips the download if it is already there.
$ErrorActionPreference = "Stop"
$version = "1.37.0"
$root = Split-Path -Parent $PSScriptRoot
$target = Join-Path $root "src-tauri\binaries\aria2c-x86_64-pc-windows-msvc.exe"
if (Test-Path $target) { exit 0 }

$url = "https://github.com/aria2/aria2/releases/download/release-$version/aria2-$version-win-64bit-build1.zip"
$tmp = Join-Path ([IO.Path]::GetTempPath()) "aria2-$version"
$zip = "$tmp.zip"
Write-Host "Downloading aria2c $version..."
Invoke-WebRequest -Uri $url -OutFile $zip
Expand-Archive -Path $zip -DestinationPath $tmp -Force
New-Item -ItemType Directory -Force -Path (Split-Path $target) | Out-Null
Copy-Item (Get-ChildItem -Path $tmp -Recurse -Filter aria2c.exe | Select-Object -First 1).FullName $target
Remove-Item -Recurse -Force $tmp, $zip
Write-Host "aria2c placed at $target"
