@echo off
rem Compila y ejecuta la herramienta de compilación (app/src/build_tool.rs) con la toolchain de work\toolchain.
rem Uso: scripts\swapprism-build.cmd package [--skip-build] [--skip-tests] ^| release [--repo o/r] [--notes "..."] ^| check-installer [--keep]
setlocal
set "ROOT=%~dp0.."
set "TC=%ROOT%\work\toolchain"
if not exist "%TC%\cargo\bin" goto build
set "RUSTUP_HOME=%TC%\rustup"
set "CARGO_HOME=%TC%\cargo"
set "RB=%TC%\rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin"
set "PATH=%TC%\cargo\bin;%RB%;%RB%\self-contained;%PATH%"
:build
rem app\.cargo\config.toml (enlazador rust-lld) solo se aplica si cargo se ejecuta desde app\.
cd /d "%ROOT%\app"
rem --offline en vez de --locked: subir la versión en Cargo.toml solo cambia nuestra propia entrada de Cargo.lock.
cargo build --release --offline --bin swapprism-build
if errorlevel 1 exit /b 1
"%ROOT%\app\target\release\swapprism-build.exe" %*
