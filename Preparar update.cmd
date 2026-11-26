@echo off
cd /d "%~dp0"
call "%~dp0scripts\swapprism-build.cmd" release
pause
