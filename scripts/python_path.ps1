# Devuelve la ruta de Python 3.12 para compilar el motor.
# Orden: variable SWAPPRISM_PYTHON, runtime de Codex, lanzador py -3.12, python del PATH.
# El Python elegido necesita Pillow (el cifrado usa Windows; PyInstaller y miniaudio vienen en work/builddeps).
$ErrorActionPreference='Stop'
if($env:SWAPPRISM_PYTHON -and (Test-Path -LiteralPath $env:SWAPPRISM_PYTHON)){$env:SWAPPRISM_PYTHON;return}
$poppler=Get-Command pdftoppm -ErrorAction SilentlyContinue
if($poppler){$candidate=[IO.Path]::GetFullPath((Join-Path (Split-Path $poppler.Source) '../../../../python/python.exe'));if(Test-Path -LiteralPath $candidate){$candidate;return}}
if(Get-Command Get-AppxPackage -ErrorAction SilentlyContinue){
  foreach($package in Get-AppxPackage -Name 'OpenAI.CodexPrimaryRuntime*' -ErrorAction SilentlyContinue){
    $candidate=Join-Path $package.InstallLocation 'dependencies/python/python.exe'
    if(Test-Path -LiteralPath $candidate){$candidate;return}
  }
}
if(Get-Command py -ErrorAction SilentlyContinue){
  $candidate=& py -3.12 -c 'import sys;print(sys.executable)' 2>$null
  if($LASTEXITCODE -eq 0 -and $candidate){$candidate.Trim();return}
}
$python=Get-Command python -ErrorAction SilentlyContinue
if($python){
  $version=& $python.Source -c 'import sys;print("%d.%d"%sys.version_info[:2])' 2>$null
  if($version -eq '3.12'){$python.Source;return}
}
throw 'No se encuentra Python 3.12. Instálalo (con Pillow) o define SWAPPRISM_PYTHON con su ruta.'
