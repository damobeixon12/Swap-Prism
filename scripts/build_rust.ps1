# Compila la interfaz Rust (app/) con la toolchain local de work/toolchain.
# Uso: scripts/build_rust.ps1 [-Tests]
param([switch]$Tests)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$work=Join-Path $root 'work'
$toolchain=Join-Path $work 'toolchain'
if(Test-Path -LiteralPath (Join-Path $toolchain 'cargo/bin')){
  $env:RUSTUP_HOME=Join-Path $toolchain 'rustup'
  $env:CARGO_HOME=Join-Path $toolchain 'cargo'
  $compilerBin=Join-Path $env:RUSTUP_HOME 'toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin'
  $env:PATH=(Join-Path $env:CARGO_HOME 'bin')+';'+$compilerBin+';'+(Join-Path $compilerBin 'self-contained')+';'+$env:PATH
  $llvmRoot=Get-ChildItem -LiteralPath $toolchain -Directory -Filter 'llvm-mingw-*-ucrt-x86_64' -ErrorAction SilentlyContinue | Select-Object -First 1
  if($llvmRoot){$env:PATH=(Join-Path $llvmRoot.FullName 'bin')+';'+$env:PATH}
}elseif(-not (Get-Command cargo -ErrorAction SilentlyContinue)){
  throw 'No hay toolchain de Rust: falta work/toolchain y cargo no está en el PATH. Instala rustup con stable-x86_64-pc-windows-gnu.'
}else{
  Write-Host 'work/toolchain no existe: se usa cargo del sistema.'
}
$cargoHome=if($env:CARGO_HOME){$env:CARGO_HOME}else{Join-Path $HOME '.cargo'}
$sdkPattern=Join-Path $cargoHome 'registry/src/*/webview2-com-sys-*/x64/WebView2Loader.dll'
$webviewSDK=Get-ChildItem -Path $sdkPattern -ErrorAction SilentlyContinue | Select-Object -Last 1
Push-Location (Join-Path $root 'app')
try{
  if(-not $webviewSDK){
    cargo fetch --locked --target x86_64-pc-windows-gnu
    if($LASTEXITCODE -ne 0){throw 'No se pudieron descargar las dependencias de Windows.'}
    $webviewSDK=Get-ChildItem -Path $sdkPattern -ErrorAction SilentlyContinue | Select-Object -Last 1
  }
  if($webviewSDK){$env:PATH=$webviewSDK.DirectoryName+';'+$env:PATH}
  if($Tests){cargo test --release --locked}else{cargo build --release --locked}
  if($LASTEXITCODE -ne 0){throw 'La compilación Rust no se completó.'}
}finally{Pop-Location}
if($webviewSDK -and -not $Tests){Copy-Item -LiteralPath $webviewSDK.FullName -Destination (Join-Path $root 'app/target/release/WebView2Loader.dll') -Force}
