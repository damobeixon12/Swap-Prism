# Compilación completa: pruebas, app Rust (interfaz y motor) e instalador NSIS.
# Resultado: outputs/Swap Prism-Setup.exe
# Uso: scripts/package_swapprism.ps1 [-SkipBuild] [-SkipTests]
param([switch]$SkipBuild,[switch]$SkipTests)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $root
$pythonPath=& (Join-Path $PSScriptRoot 'python_path.ps1')
$version=((Get-Content -LiteralPath 'app/Cargo.toml' | Where-Object {$_ -match '^version\s*='} | Select-Object -First 1) -split '"')[1]
if(-not $SkipBuild){
  Write-Host '== Rust'
  if(-not $SkipTests){& (Join-Path $PSScriptRoot 'build_rust.ps1') -Tests}
  & (Join-Path $PSScriptRoot 'build_rust.ps1')
}
Write-Host '== Miniaturas'
& $pythonPath (Join-Path $PSScriptRoot 'build_thumbnails.py')
if($LASTEXITCODE -ne 0){throw 'No se pudieron preparar las miniaturas.'}
Write-Host '== Distribución'
$dist=Join-Path $root 'work/distribution'
New-Item -ItemType Directory -Path $dist -Force | Out-Null
Copy-Item -LiteralPath app/target/release/SWAPPRISM.exe -Destination (Join-Path $dist 'Swap Prism.exe') -Force
Copy-Item -LiteralPath app/target/release/WebView2Loader.dll -Destination (Join-Path $dist 'WebView2Loader.dll') -Force
$thirdParty=(Get-Content -LiteralPath app/assets/THIRD_PARTY.txt -Raw -Encoding UTF8).Replace('{VERSION}',$version)
[IO.File]::WriteAllText((Join-Path $dist 'THIRD_PARTY.txt'),$thirdParty,(New-Object System.Text.UTF8Encoding($false)))
& $pythonPath (Join-Path $PSScriptRoot 'embed_brand.py') (Join-Path $dist 'Swap Prism.exe') app/assets/swapprism.ico
if($LASTEXITCODE -ne 0){throw 'No se pudo incrustar el icono.'}
# _engine keeps only the native updater and the licences (the engine runs inside Swap Prism.exe).
$engineDist=Join-Path $dist '_engine'
if(Test-Path -LiteralPath $engineDist){Remove-Item -LiteralPath $engineDist -Recurse -Force}
$licenseDir=Join-Path $engineDist 'licenses'
New-Item -ItemType Directory -Path $licenseDir -Force | Out-Null
Copy-Item -LiteralPath app/target/release/SWAPPRISMUpdate.exe -Destination (Join-Path $engineDist 'SWAPPRISMUpdate.exe') -Force
Copy-Item -Path app/assets/licenses/* -Destination $licenseDir -Force
Copy-Item -LiteralPath app/assets/icons/LICENSE -Destination (Join-Path $licenseDir 'Lucide-LICENSE.txt') -Force
Copy-Item -LiteralPath app/assets/fonts/Nunito-OFL.txt -Destination (Join-Path $licenseDir 'Nunito-OFL.txt') -Force
$cargoHome=if($env:CARGO_HOME){$env:CARGO_HOME}elseif(Test-Path -LiteralPath 'work/toolchain/cargo'){'work/toolchain/cargo'}else{Join-Path $HOME '.cargo'}
$wryLicense=Get-ChildItem -Path (Join-Path $cargoHome 'registry/src/*/wry-0.57.3/LICENSE-MIT') -ErrorAction SilentlyContinue | Select-Object -First 1
if(-not $wryLicense){throw 'Falta la licencia de Wry.'}
Copy-Item -LiteralPath $wryLicense.FullName -Destination (Join-Path $licenseDir 'Wry-MIT.txt') -Force
& $pythonPath (Join-Path $PSScriptRoot 'build_installer_payload.py')
if($LASTEXITCODE -ne 0){throw 'No se pudo generar el progreso del instalador.'}
Write-Host '== Instalador'
& work/toolchain/nsis-3.11/makensis.exe /V2 app/installer.nsi
if($LASTEXITCODE -ne 0){throw 'No se pudo generar el instalador.'}
Get-FileHash -LiteralPath 'outputs/Swap Prism-Setup.exe' -Algorithm SHA256
