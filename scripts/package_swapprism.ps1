# Compilación completa: pruebas, interfaz Rust, motor Python e instalador NSIS.
# Resultado: outputs/Swap Prism-Setup.exe
# Uso: scripts/package_swapprism.ps1 [-SkipBuild] [-SkipTests]
param([switch]$SkipBuild,[switch]$SkipTests)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $root
$pythonPath=& (Join-Path $PSScriptRoot 'python_path.ps1')
$version=((Get-Content -LiteralPath 'app/Cargo.toml' | Where-Object {$_ -match '^version\s*='} | Select-Object -First 1) -split '"')[1]
if(-not $SkipTests){
  Write-Host '== Pruebas del motor Python'
  $env:PYTHONPATH=Join-Path $root 'work/builddeps'
  & $pythonPath -m unittest discover -s engine/tests -t engine
  if($LASTEXITCODE -ne 0){throw 'Fallan las pruebas del motor Python.'}
  Remove-Item Env:PYTHONPATH
}
if(-not $SkipBuild){
  Write-Host '== Rust'
  if(-not $SkipTests){& (Join-Path $PSScriptRoot 'build_rust.ps1') -Tests}
  & (Join-Path $PSScriptRoot 'build_rust.ps1')
  Write-Host '== Motor'
  & (Join-Path $PSScriptRoot 'build_engine.ps1')
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
$engineDist=Join-Path $dist '_engine'
if(Test-Path -LiteralPath $engineDist){Remove-Item -LiteralPath $engineDist -Recurse -Force}
New-Item -ItemType Directory -Path $engineDist -Force | Out-Null
Copy-Item -Path work/staging/SWAPPRISMEngine/* -Destination $engineDist -Recurse -Force
Copy-Item -LiteralPath app/target/release/SWAPPRISMUpdate.exe -Destination (Join-Path $engineDist 'SWAPPRISMUpdate.exe') -Force
Copy-Item -LiteralPath engine/RL-Swapper_LICENSE.txt -Destination (Join-Path $engineDist 'RL-Swapper_LICENSE.txt') -Force
& $pythonPath (Join-Path $PSScriptRoot 'build_installer_payload.py')
if($LASTEXITCODE -ne 0){throw 'No se pudo generar el progreso del instalador.'}
Write-Host '== Instalador'
& work/toolchain/nsis-3.11/makensis.exe /V2 app/installer.nsi
if($LASTEXITCODE -ne 0){throw 'No se pudo generar el instalador.'}
Get-FileHash -LiteralPath 'outputs/Swap Prism-Setup.exe' -Algorithm SHA256
