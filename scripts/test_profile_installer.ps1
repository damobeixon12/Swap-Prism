# Prueba el instalador en una carpeta temporal (work/installer-check):
# arranque de la app, lector de paquetes del motor, HTTPS verificado y ausencia de datos privados.
# Uso: scripts/test_profile_installer.ps1 [-KeepForQA]
param([switch]$KeepForQA)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$work=Join-Path $root 'work'
$smoke=Join-Path $PSScriptRoot 'smoke'
$version=((Get-Content -LiteralPath (Join-Path $root 'app/Cargo.toml') | Where-Object {$_ -match '^version\s*='} | Select-Object -First 1) -split '"')[1]
$installRoot=[IO.Path]::GetFullPath((Join-Path $work 'installer-check'))
if(-not $installRoot.StartsWith($work+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Destino de prueba fuera del espacio previsto.'}
if(Test-Path -LiteralPath $installRoot){throw 'work/installer-check ya existe; bórrala antes de repetir la prueba.'}
$setup=Start-Process -FilePath (Join-Path $root 'outputs/Swap Prism-Setup.exe') -ArgumentList @('/S','/TESTMODE',('/D='+$installRoot)) -WindowStyle Hidden -PassThru -Wait
if($setup.ExitCode -ne 0){throw ('El instalador devolvió '+$setup.ExitCode)}
$exe=Join-Path $installRoot 'Swap Prism.exe'
if((Get-FileHash -LiteralPath $exe).Hash -ne (Get-FileHash -LiteralPath (Join-Path $work 'distribution/Swap Prism.exe')).Hash){throw 'El instalador contiene otro ejecutable.'}
foreach($private in @('data/profile.json','data/profile-draft.json','data/profile/images','data/backups','data/tracker.json','data/tracker-browser','data/rank-draft.json')){
  if(Test-Path -LiteralPath (Join-Path $installRoot $private)){throw ('El instalador contiene datos privados: '+$private)}
}
$smokeLog=Join-Path $work 'installer-smoke.json'
$testApp=Start-Process -FilePath $exe -ArgumentList @('--smoke',$smokeLog) -WindowStyle Hidden -PassThru
if(-not $testApp.WaitForExit(30000)){throw 'La prueba de arranque no terminó.'}
$result=Get-Content -LiteralPath $smokeLog -Raw | ConvertFrom-Json
if($result.version -ne $version -or $result.gui -ne 'ok'){throw ('Falló el arranque instalado (versión '+$result.version+', esperada '+$version+').')}
$env:SWAPPRISM_ROOT=$installRoot
$engine=Join-Path $installRoot '_engine/SWAPPRISMEngine.exe'
$codecLog=Join-Path $work 'installer-codec.json'
& $engine --run-worker (Join-Path $smoke 'profile_codec_smoke.py') $codecLog 2>$null
if($LASTEXITCODE -ne 0){throw 'Falló el lector del instalador.'}
$codec=Get-Content -LiteralPath $codecLog -Raw | ConvertFrom-Json
if(-not $codec.codec_loaded -or $codec.tkinter_loaded){throw 'El lector necesita la interfaz antigua.'}
$httpsLog=Join-Path $work 'installer-https.json'
& $engine --run-worker (Join-Path $smoke 'https_context_smoke.py') $httpsLog
if($LASTEXITCODE -ne 0){throw 'El motor instalado no puede crear conexiones HTTPS verificadas.'}
$https=Get-Content -LiteralPath $httpsLog -Raw | ConvertFrom-Json
if(-not $https.frozen -or -not $https.verified_https_context){throw 'No se verificó HTTPS en el motor instalado.'}
Remove-Item Env:SWAPPRISM_ROOT
$folder=Get-Item -LiteralPath $installRoot
if(($folder.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $folder.FullName -ne $installRoot){throw 'Destino temporal cambiado; no se borra.'}
if(-not $KeepForQA){Remove-Item -LiteralPath $installRoot -Recurse -Force}
[pscustomobject]@{Version=$result.version;InstallerStart='ok';BundledCodec='ok';VerifiedHttps='ok';PrivateDataIncluded=$false;TemporaryInstallRemoved=(-not $KeepForQA)} | ConvertTo-Json -Compress
