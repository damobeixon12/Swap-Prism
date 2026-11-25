# Empaqueta el motor Python (engine/) con PyInstaller en work/staging/SWAPPRISMEngine.
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$work=Join-Path $root 'work'
Set-Location -LiteralPath $root
$pythonPath=& (Join-Path $PSScriptRoot 'python_path.ps1')
$env:PYTHONPATH=Join-Path $work 'builddeps'
$env:PYINSTALLER_CONFIG_DIR=Join-Path $work 'pyinstaller-cache'
$buildArgs=@('--noconfirm','--onedir','--console','--name','SWAPPRISMEngine',
  '--distpath','work/staging','--workpath','work/pyinstaller-engine','--specpath','work',
  '--exclude-module','tkinter','--exclude-module','numpy','--exclude-module','scipy',
  '--exclude-module','PIL.AvifImagePlugin','--exclude-module','PIL._avif',
  '--collect-all','cryptography','--collect-all','miniaudio',
  '--icon',(Join-Path $root 'app/assets/swapprism.ico'))
foreach($file in @('rl_upk_editor.py','items.json','keys.txt','keys_map.json','catalog_enrichment.json')){
  $buildArgs+=@('--add-data',((Join-Path $root ('engine/motor/'+$file))+';motor'))
}
foreach($file in @('engine/avm2_operands.json','engine/AVM2_NOTICE.txt','app/assets/audio_manifest.json','app/assets/logo.png')){
  $buildArgs+=@('--add-data',((Join-Path $root $file)+';.'))
}
# Certificados del runtime de Python usado para compilar (necesario para HTTPS, ver docs/historial/FIX_0231.md).
$runtimeTrust=Join-Path (Join-Path (Split-Path $pythonPath) 'Lib') 'codex-rejected-cas.pem'
if(Test-Path -LiteralPath $runtimeTrust){$buildArgs+=@('--add-data',($runtimeTrust+';.'))}
$buildArgs+='engine/swapprism_engine.py'
& $pythonPath -m PyInstaller @buildArgs
if($LASTEXITCODE -ne 0){throw 'No se pudo compilar el motor.'}
$licenseDir=Join-Path $work 'staging/SWAPPRISMEngine/licenses'
New-Item -ItemType Directory -Path $licenseDir -Force | Out-Null
foreach($dependency in @('miniaudio-1.73','cffi-2.1.1','pycparser-3.12')){
  Copy-Item -LiteralPath (Join-Path $work ('builddeps/'+$dependency+'.dist-info/licenses/LICENSE')) -Destination (Join-Path $licenseDir ($dependency+'-LICENSE.txt')) -Force
}
Copy-Item -LiteralPath (Join-Path $root 'app/assets/icons/LICENSE') -Destination (Join-Path $licenseDir 'Lucide-LICENSE.txt') -Force
Copy-Item -LiteralPath (Join-Path $root 'app/assets/fonts/Nunito-OFL.txt') -Destination (Join-Path $licenseDir 'Nunito-OFL.txt') -Force
$cargoHome=if($env:CARGO_HOME){$env:CARGO_HOME}elseif(Test-Path -LiteralPath (Join-Path $work 'toolchain/cargo')){Join-Path $work 'toolchain/cargo'}else{Join-Path $HOME '.cargo'}
$wryLicense=Get-ChildItem -Path (Join-Path $cargoHome 'registry/src/*/wry-0.57.3/LICENSE-MIT') -ErrorAction SilentlyContinue | Select-Object -First 1
if(-not $wryLicense){throw 'Falta la licencia de Wry (compila primero la parte Rust).'}
Copy-Item -LiteralPath $wryLicense.FullName -Destination (Join-Path $licenseDir 'Wry-MIT.txt') -Force
