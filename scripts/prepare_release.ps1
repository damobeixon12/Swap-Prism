# Prepara una versión para publicar: sincroniza la versión de app/Cargo.toml en el instalador,
# compila todo y deja outputs/Swap Prism-Update.zip (instalador + update.json). No publica nada.
param([string]$Repository='damobeixon12/Swap-Prism',[string]$Notes='Mejoras y correcciones de Swap Prism.',[switch]$SkipBuild)
$ErrorActionPreference='Stop'
$projectRoot=[IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
Set-Location -LiteralPath $projectRoot
$Repository=$Repository.Trim().Replace('https://github.com/','').TrimEnd('/');if($Repository.EndsWith('.git')){$Repository=$Repository.Substring(0,$Repository.Length-4)}
if($Repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$'){throw 'Usa propietario/repositorio o el enlace HTTPS de GitHub.'}
$versionLine=Get-Content -LiteralPath 'app/Cargo.toml' | Where-Object {$_ -match '^version\s*='} | Select-Object -First 1
$releaseVersion=($versionLine -split '"')[1]
if($releaseVersion -notmatch '^\d+\.\d+\.\d+$'){throw 'Pon una versión estable x.y.z en app/Cargo.toml.'}
$channel='https://github.com/'+$Repository+'/releases/latest/download/update.json'
$settingsPath=Join-Path $projectRoot 'app/assets/settings.json'
$settings=Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
$settings | Add-Member -NotePropertyName update_manifest_url -NotePropertyValue $channel -Force
$settings | Add-Member -NotePropertyName release_repository -NotePropertyValue $Repository -Force
$utf8=New-Object System.Text.UTF8Encoding($false)
$newSettings=$settings | ConvertTo-Json
if($SkipBuild -and (Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json).update_manifest_url -ne $channel){throw 'El canal cambió. Ejecuta sin -SkipBuild para incorporarlo al instalador.'}
[IO.File]::WriteAllText($settingsPath,$newSettings,$utf8)
$installerScript=Join-Path $projectRoot 'app/installer.nsi'
$nsi=Get-Content -LiteralPath $installerScript -Raw -Encoding UTF8
$nsi=$nsi -replace 'VIProductVersion "[^"]+"',('VIProductVersion "'+$releaseVersion+'.0"')
$nsi=$nsi -replace '(VIAddVersionKey "FileVersion" ")[^"]+',('${1}'+$releaseVersion)
$nsi=$nsi -replace '("DisplayVersion" ")[^"]+',('${1}'+$releaseVersion)
[IO.File]::WriteAllText($installerScript,$nsi,(New-Object System.Text.UTF8Encoding($true))) # NSIS needs the BOM to read UTF-8
$lockPath=Join-Path $projectRoot 'app/Cargo.lock'
$lock=Get-Content -LiteralPath $lockPath -Raw
$lock=$lock -replace '(?m)(name = "swapprism"\r?\nversion = ")[^"]+',('${1}'+$releaseVersion)
[IO.File]::WriteAllText($lockPath,$lock,$utf8)
if(-not $SkipBuild){& (Join-Path $PSScriptRoot 'package_swapprism.ps1');if($LASTEXITCODE -ne 0){throw 'Falló la compilación.'}}
$setup=Join-Path $projectRoot 'outputs/Swap Prism-Setup.exe'
if(-not(Test-Path -LiteralPath $setup)){throw 'Falta el instalador.'}
$binaryVersion=(Get-Item -LiteralPath $setup).VersionInfo.FileVersion
if($binaryVersion -ne $releaseVersion){throw ('El instalador es '+$binaryVersion+' y el proyecto '+$releaseVersion+'. Vuelve a compilar.')}
$assetName='Swap-Prism-Setup.exe'
$manifest=[ordered]@{version=$releaseVersion;url=('https://github.com/'+$Repository+'/releases/download/v'+$releaseVersion+'/'+$assetName);sha256=(Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant();notes=$Notes}
$manifestPath=Join-Path $projectRoot 'outputs/update.json'
[IO.File]::WriteAllText($manifestPath,($manifest|ConvertTo-Json),$utf8)
$instructions=@"
SWAP PRISM $releaseVersion - ARCHIVOS PREPARADOS, SIN PUBLICAR

1. Extrae Swap Prism-Update.zip fuera de la carpeta del proyecto.
2. Abre https://github.com/$Repository/releases/new
3. Etiqueta: v$releaseVersion. Titulo: Swap Prism $releaseVersion.
4. Sube los dos archivos extraidos: $assetName y update.json.
5. Publica como version estable y marca Set as latest release.

El canal de las apps es $channel
Actualizar el codigo del repo NO publica un instalador: repite este proceso.
Para la siguiente version cambia primero version en app/Cargo.toml y ejecuta
Preparar update.cmd. No subas outputs/LocalSkins/data ni backups al repositorio.
"@
[IO.File]::WriteAllText((Join-Path $projectRoot 'outputs/PUBLICAR-ACTUALIZACION.txt'),$instructions,$utf8)
$bundle=Join-Path $projectRoot 'work/release-bundle'
if(Test-Path -LiteralPath $bundle){throw 'La carpeta temporal release-bundle ya existe.'}
New-Item -ItemType Directory -Path $bundle | Out-Null
try{
 Copy-Item -LiteralPath $setup -Destination (Join-Path $bundle $assetName)
 Copy-Item -LiteralPath $manifestPath -Destination (Join-Path $bundle 'update.json')
 Compress-Archive -Path (Join-Path $bundle '*') -DestinationPath (Join-Path $projectRoot 'outputs/Swap Prism-Update.zip') -Force
}finally{
 $resolved=[IO.Path]::GetFullPath($bundle)
 if($resolved -ne [IO.Path]::Combine($projectRoot,'work','release-bundle') -or (Get-Item -LiteralPath $bundle).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Carpeta temporal no válida; no se elimina.'}
 Remove-Item -LiteralPath $bundle -Recurse -Force
}
Write-Host ('Listo: outputs/Swap Prism-Update.zip · '+$releaseVersion)
Write-Host 'No se ha subido ni publicado nada.'
