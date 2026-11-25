Unicode True
!include "MUI2.nsh"
!include "FileFunc.nsh"
Var TestMode
Name "Swap Prism"
OutFile "..\outputs\Swap Prism-Setup.exe"
InstallDir "$LOCALAPPDATA\Programs\SWAPPRISM"
InstallDirRegKey HKCU "Software\SWAPPRISM" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
Icon "assets\swapprism.ico"
UninstallIcon "assets\swapprism.ico"
VIProductVersion "0.30.0.0"
VIAddVersionKey "ProductName" "Swap Prism"
VIAddVersionKey "FileDescription" "Instalador Swap Prism"
VIAddVersionKey "FileVersion" "0.30.0"
VIAddVersionKey "LegalCopyright" "Swap Prism"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "Instala Swap Prism para tu usuario de Windows.$\r$\n$\r$\nIncluye el motor necesario: no necesitas instalar Python ni Rust.$\r$\n$\r$\nAplicaciÃ³n experimental sin firma digital. No instala controladores ni modifica Rocket League durante la instalaciÃ³n."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\Swap Prism.exe"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Spanish"

!macro UpdateProgress VALUE
  FileOpen $9 "$INSTDIR\data\updates\install-progress.txt" w
  FileWrite $9 "${VALUE}"
  FileClose $9
!macroend

!macro RequireClosed PREFIX
${PREFIX}retry:
  System::Call 'kernel32::OpenMutexW(i 0x100000, i 0, w "Local\SWAPPRISM.SingleInstance") p.r0'
  StrCmp $0 0 ${PREFIX}done
  System::Call 'kernel32::CloseHandle(p r0)'
  IfSilent ${PREFIX}abort
  MessageBox MB_RETRYCANCEL|MB_ICONINFORMATION "Cierra Swap Prism con Salir o Ctrl+Q antes de continuar." IDRETRY ${PREFIX}retry
${PREFIX}abort:
  SetErrorLevel 2
  Abort
${PREFIX}done:
!macroend
Function .onInit
  !insertmacro RequireClosed "install"
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/TESTMODE" $1
  IfErrors +2
  StrCpy $TestMode "1"
FunctionEnd
Function un.onInit
  !insertmacro RequireClosed "uninstall"
FunctionEnd

Section "Swap Prism" SecMain
  SetShellVarContext current
  Delete "$INSTDIR\SWAPPRISM.exe"
  Delete "$DESKTOP\SWAPPRISM.lnk"
  Delete "$SMPROGRAMS\SWAPPRISM\SWAPPRISM.lnk"
  Delete "$SMPROGRAMS\SWAPPRISM\Desinstalar.lnk"
  RMDir "$SMPROGRAMS\SWAPPRISM"
  CreateDirectory "$INSTDIR\data\updates"
  !insertmacro UpdateProgress 0
  Delete "$INSTDIR\install-update.ps1"
  RMDir /r "$INSTDIR\_engine"
  !include "..\work\installer-payload.nsh"
  StrCmp $TestMode "1" test_done
  WriteRegStr HKCU "Software\SWAPPRISM" "InstallDir" "$INSTDIR"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SWAPPRISM" "DisplayName" "Swap Prism"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SWAPPRISM" "DisplayVersion" "0.30.0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SWAPPRISM" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SWAPPRISM" "DisplayIcon" "$INSTDIR\swapprism.ico"
  CreateShortcut "$DESKTOP\Swap Prism.lnk" "$INSTDIR\Swap Prism.exe" "" "$INSTDIR\swapprism.ico"
  CreateDirectory "$SMPROGRAMS\Swap Prism"
  CreateShortcut "$SMPROGRAMS\Swap Prism\Swap Prism.lnk" "$INSTDIR\Swap Prism.exe"
  CreateShortcut "$SMPROGRAMS\Swap Prism\Desinstalar.lnk" "$INSTDIR\Uninstall.exe"
test_done:
  !insertmacro UpdateProgress 100
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$DESKTOP\Swap Prism.lnk"
  Delete "$SMPROGRAMS\Swap Prism\Swap Prism.lnk"
  Delete "$SMPROGRAMS\Swap Prism\Desinstalar.lnk"
  RMDir "$SMPROGRAMS\Swap Prism"
  Delete "$INSTDIR\Swap Prism.exe"
  Delete "$INSTDIR\WebView2Loader.dll"
  Delete "$INSTDIR\swapprism.ico"
  Delete "$INSTDIR\install-update.ps1"
  Delete "$INSTDIR\THIRD_PARTY.txt"
  RMDir /r "$INSTDIR\_engine"
  RMDir /r "$INSTDIR\_telemetry"
  Delete "$INSTDIR\Uninstall.exe"
  DeleteRegKey HKCU "Software\SWAPPRISM"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SWAPPRISM"
  ; Never delete data/backups or touch game files while uninstalling.
  IfSilent +2
  MessageBox MB_OK "AplicaciÃ³n desinstalada. Se conserva $INSTDIR\data con tus copias originales. Los cambios del juego se restauran desde Swap Prism antes de desinstalar."
SectionEnd


