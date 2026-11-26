Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma
!include "LogicLib.nsh"

!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef OUTPUT_FILE
  !error "OUTPUT_FILE is required"
!endif

; With UNINSTALLER_OUTPUT the build is a generator: run once on the build
; machine, it writes the uninstaller to that path and exits. The release signs
; that file and the installer ships it, so no unsigned uninstaller is ever
; written on the user's computer.
!ifndef UNINSTALLER_OUTPUT
  !ifndef SOURCE_DIR
    !error "SOURCE_DIR is required"
  !endif
!endif

!define APP_WINDOW_CLASS "FLUTTER_RUNNER_WIN32_WINDOW"
; 30 seconds in 250 ms steps.
!define EXIT_WAIT_STEPS 120

Name "CopyPaste"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\CopyPaste"
InstallDirRegKey HKCU "Software\CopyPaste" "InstallDir"
VIProductVersion "${VERSION}.1"
VIAddVersionKey /LANG=1033 "ProductName" "CopyPaste"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}.1"
VIAddVersionKey /LANG=1033 "CompanyName" "CopyPaste"

!ifdef UNINSTALLER_OUTPUT

VIAddVersionKey /LANG=1033 "FileDescription" "CopyPaste uninstaller"
UninstPage uninstConfirm
UninstPage instfiles

Function .onInit
  WriteUninstaller "${UNINSTALLER_OUTPUT}"
  Quit
FunctionEnd

Section "CopyPaste" SEC_MAIN
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  FindWindow $0 "${APP_WINDOW_CLASS}" "CopyPaste"
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Quit CopyPaste before uninstalling it."
    Abort
  ${EndIf}
  Delete "$SMPROGRAMS\CopyPaste\CopyPaste.lnk"
  RMDir "$SMPROGRAMS\CopyPaste"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste"
  DeleteRegKey HKCU "Software\CopyPaste"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "CopyPaste"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "CopyPaste"
  ; Notification registration written by the app and by Windows for its AUMID.
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\com.copypaste.app"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\PushNotifications\Backup\com.copypaste.app"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\com.copypaste.app"
  ReadRegStr $0 HKCU "Software\Classes\copypaste\shell\open\command" ""
  ${If} $0 == '"$INSTDIR\CopyPaste.exe" "%1"'
    DeleteRegKey HKCU "Software\Classes\copypaste"
  ${EndIf}
  RMDir /r "$INSTDIR"

  ; Silent uninstalls (package managers, upgrades) keep history.
  MessageBox MB_YESNO|MB_ICONQUESTION "Also delete CopyPaste history, settings and encryption keys from this computer? This cannot be undone." /SD IDNO IDNO keep_data
    ; App data (path_provider) and the standalone CLI daemon's data.
    RMDir /r "$APPDATA\com.copypaste\CopyPaste"
    RMDir "$APPDATA\com.copypaste"
    RMDir /r "$APPDATA\copypaste\CopyPaste"
    RMDir "$APPDATA\copypaste"
    RMDir /r "$TEMP\copypaste-staging"
    RMDir /r "$TEMP\copypaste-updates"
  keep_data:
SectionEnd

!else

VIAddVersionKey /LANG=1033 "FileDescription" "CopyPaste installer"
Page directory
Page instfiles

; Sets $0 to 1 when FILE exists and a running process still holds it.
!macro MarkFileInUse FILE
  ${If} ${FileExists} "${FILE}"
    ClearErrors
    FileOpen $2 "${FILE}" a
    ${If} ${Errors}
      StrCpy $0 1
    ${Else}
      FileClose $2
    ${EndIf}
  ${EndIf}
!macroend

; Leaves $0 at 0 once CopyPaste and its daemon have exited. An update started
; from the running app reaches this while the app is still quitting, so wait a
; bounded time instead of failing at once.
Function WaitForApplicationExit
  StrCpy $1 0
  ${Do}
    FindWindow $0 "${APP_WINDOW_CLASS}" "CopyPaste"
    ${If} $0 == 0
      !insertmacro MarkFileInUse "$INSTDIR\CopyPaste.exe"
      !insertmacro MarkFileInUse "$INSTDIR\copypaste-daemon.exe"
    ${EndIf}
    ${If} $0 == 0
    ${OrIf} $1 >= ${EXIT_WAIT_STEPS}
      ${ExitDo}
    ${EndIf}
    ${If} $1 == 0
      DetailPrint "Waiting for CopyPaste to close..."
    ${EndIf}
    Sleep 250
    IntOp $1 $1 + 1
  ${Loop}
FunctionEnd

Section "CopyPaste" SEC_MAIN
  SetShellVarContext current
  Call WaitForApplicationExit
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Quit CopyPaste before installing this update."
    Abort
  ${EndIf}

  SetOutPath "$INSTDIR"
  File /r /x "Uninstall.exe" "${SOURCE_DIR}\*"
  ; Named on its own so a build without the signed uninstaller fails.
  File "${SOURCE_DIR}\Uninstall.exe"
  ; Earlier releases shipped an update helper and left copies of it in TEMP.
  Delete "$INSTDIR\copypaste-update-helper.exe"
  Delete "$TEMP\CopyPaste-update-helper-*.exe"
  WriteRegStr HKCU "Software\CopyPaste" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "DisplayName" "CopyPaste"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "Publisher" "CopyPaste"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  CreateDirectory "$SMPROGRAMS\CopyPaste"
  CreateShortcut "$SMPROGRAMS\CopyPaste\CopyPaste.lnk" "$INSTDIR\CopyPaste.exe"
SectionEnd

!endif
