Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma
!include "LogicLib.nsh"

!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef SOURCE_DIR
  !error "SOURCE_DIR is required"
!endif
!ifndef OUTPUT_FILE
  !error "OUTPUT_FILE is required"
!endif

Name "CopyPaste"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\CopyPaste"
InstallDirRegKey HKCU "Software\CopyPaste" "InstallDir"
VIProductVersion "${VERSION}.1"
VIAddVersionKey /LANG=1033 "ProductName" "CopyPaste"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}.1"
VIAddVersionKey /LANG=1033 "CompanyName" "CopyPaste"
VIAddVersionKey /LANG=1033 "FileDescription" "CopyPaste installer"

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "CopyPaste" SEC_MAIN
  SetShellVarContext current
  System::Call 'user32::FindWindowW(w "FLUTTER_RUNNER_WIN32_WINDOW", w "CopyPaste") p .r0'
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Quit CopyPaste before installing this update."
    Abort
  ${EndIf}

  SetOutPath "$INSTDIR"
  File /r "${SOURCE_DIR}\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\CopyPaste" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "DisplayName" "CopyPaste"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "Publisher" "CopyPaste"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CopyPaste" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  CreateDirectory "$SMPROGRAMS\CopyPaste"
  CreateShortcut "$SMPROGRAMS\CopyPaste\CopyPaste.lnk" "$INSTDIR\CopyPaste.exe"
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  System::Call 'user32::FindWindowW(w "FLUTTER_RUNNER_WIN32_WINDOW", w "CopyPaste") p .r0'
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
