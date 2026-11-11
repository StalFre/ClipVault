# Genera los dos instaladores (con y sin administrador) y el zip de la extension en .\dist
# Uso: .\build_installer.ps1
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$ver = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
Write-Host "ClipVault $ver"

cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build fallo' }

$iscc = @(
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw 'No se encontro Inno Setup 6 (ISCC.exe)' }

New-Item -ItemType Directory -Force dist | Out-Null
& $iscc "/DMyAppVersion=$ver" /DADMIN installer\clipvault.iss
if ($LASTEXITCODE -ne 0) { throw 'ISCC (Admin) fallo' }
& $iscc "/DMyAppVersion=$ver" installer\clipvault.iss
if ($LASTEXITCODE -ne 0) { throw 'ISCC (User) fallo' }

$zip = "dist\ClipVault-extension-$ver.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path ClipVault-extension\* -DestinationPath $zip
Get-ChildItem dist | Select-Object Name, @{n='MB';e={[math]::Round($_.Length/1MB,1)}}
