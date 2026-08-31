param(
    [switch]$Check,
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$expectedTauriTag = "tauri-cli-v2.11.4"
$expectedTauriCliVersion = "2.11.4"
$expectedInstallerHash = "20f4ecc730defb71f1342eaeaec4021df13be3d843abba0effe88ea5835fa079"
$expectedLicenseHash = "9dd42ea92cff2ede5cd477cbfcce051b2d0115c0ac7f368ee88cb545055dff1d"
$helperSource = "binaries/copypaste-x86_64-pc-windows-msvc.exe"
$helperTarget = "copypaste-installer-helper.exe"

function Assert-ExactlyOnce([string]$Text, [string]$Pattern, [string]$Name) {
    $matches = [regex]::Matches($Text, $Pattern, [Text.RegularExpressions.RegexOptions]::Singleline)
    if ($matches.Count -ne 1) { throw "installer template drift: $Name matched $($matches.Count) regions" }
    return $matches[0]
}

function Replace-ExactlyOnce([string]$Text, [string]$Pattern, [string]$Replacement, [string]$Name) {
    $match = Assert-ExactlyOnce $Text $Pattern $Name
    return $Text.Substring(0, $match.Index) + $Replacement + $Text.Substring($match.Index + $match.Length)
}

function Assert-HelperResource([object]$Resources, [string]$Name) {
    $properties = @($Resources.PSObject.Properties)
    $matches = @($properties | Where-Object { $_.Value -ceq $helperTarget })
    if ($matches.Count -ne 1 -or $matches[0].Name -cne $helperSource) {
        throw "$Name must map exactly one signed CLI resource to the installer helper"
    }
}

function Get-RepositoryRoot {
    return [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
}

function Get-GeneratedTemplate([string]$Root) {
    $tauriRoot = Join-Path $Root "crates/copypaste-ui/src-tauri"
    $upstreamPath = Join-Path $tauriRoot "windows/installer.upstream.nsi"
    $licensePath = Join-Path $tauriRoot "windows/installer.upstream.LICENSE-MIT"
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $upstreamPath).Hash.ToLowerInvariant() -cne $expectedInstallerHash) {
        throw "installer template drift: pinned upstream installer hash changed"
    }
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $licensePath).Hash.ToLowerInvariant() -cne $expectedLicenseHash) {
        throw "installer template drift: pinned upstream license hash changed"
    }

    $lock = Get-Content -Raw -LiteralPath (Join-Path $Root "crates/copypaste-ui/package-lock.json") | ConvertFrom-Json -AsHashtable
    if ($lock.packages["node_modules/@tauri-apps/cli"].version -cne $expectedTauriCliVersion) {
        throw "installer template drift: locked Tauri CLI version is not $expectedTauriCliVersion"
    }
    foreach ($configName in @("tauri.windows.release.conf.json", "tauri.windows.signed.conf.template.json")) {
        $config = Get-Content -Raw -LiteralPath (Join-Path $tauriRoot $configName) | ConvertFrom-Json
        Assert-HelperResource $config.bundle.resources $configName
    }

    $template = [IO.File]::ReadAllText($upstreamPath)
    $helperPreamble = @'
!define INSTALLER_HELPER_TARGET "copypaste-installer-helper.exe"
{{#each resources}}
!if "{{this.[1]}}" == "${INSTALLER_HELPER_TARGET}"
  !ifdef INSTALLER_HELPER_RESOURCE
    !error "installer helper resource is ambiguous"
  !endif
  !define INSTALLER_HELPER_RESOURCE
!endif
{{/each}}
!ifndef INSTALLER_HELPER_RESOURCE
  !error "installer helper resource is missing"
!endif

!macro ExtractInstallerHelper
  InitPluginsDir
{{#each resources}}
!if "{{this.[1]}}" == "${INSTALLER_HELPER_TARGET}"
  ClearErrors
  File "/oname=$PLUGINSDIR\${INSTALLER_HELPER_TARGET}" "{{no-escape @key}}"
  ${If} ${Errors}
    Abort "CopyPaste could not prepare the installer. Try again."
  ${EndIf}
!endif
{{/each}}
!macroend

!macro DefineGuiRefusal FunctionName LabelPrefix
Function ${FunctionName}
  ClearErrors
  nsis_tauri_utils::FindProcessCurrentUser "${MAINBINARYNAME}.exe"
  ${If} ${Errors}
    Goto ${LabelPrefix}_refuse
  ${EndIf}
  Pop $0
  ${If} ${Errors}
    Goto ${LabelPrefix}_refuse
  ${EndIf}
  StrCmp $0 "1" ${LabelPrefix}_absent
  ${LabelPrefix}_refuse:
    Abort "CopyPaste is running. Close it and try again."
  ${LabelPrefix}_absent:
FunctionEnd
!macroend

!macro DefineCliDrain FunctionName LabelPrefix
Function ${FunctionName}
  ClearErrors
  nsExec::ExecToStack '"$PLUGINSDIR\${INSTALLER_HELPER_TARGET}" shutdown --wait-for-exit'
  ${If} ${Errors}
    Goto ${LabelPrefix}_refuse
  ${EndIf}
  Pop $0
  ${If} ${Errors}
    Goto ${LabelPrefix}_refuse
  ${EndIf}
  Pop $1
  ${If} ${Errors}
    Goto ${LabelPrefix}_refuse
  ${EndIf}
  StrCmp $0 "0" ${LabelPrefix}_done
  ${LabelPrefix}_refuse:
    Abort "CopyPaste could not stop safely. Close it and try again."
  ${LabelPrefix}_done:
FunctionEnd
!macroend

!insertmacro DefineGuiRefusal RefuseIfCurrentUserAppIsRunning installer_gui
!insertmacro DefineGuiRefusal un.RefuseIfCurrentUserAppIsRunning uninstaller_gui
!insertmacro DefineCliDrain StopCopyPasteForInstaller installer_stop
!insertmacro DefineCliDrain un.StopCopyPasteForInstaller uninstaller_stop
'@
    $template = Replace-ExactlyOnce $template "Var PassiveMode\r?\nVar UpdateMode\r?\nVar NoShortcutMode\r?\nVar WixMode\r?\nVar OldMainBinaryName" "Var PassiveMode`nVar UpdateMode`nVar NoShortcutMode`nVar OldMainBinaryName" "installer variables"
    $pluginRegistration = @'
# additional plugins
!addplugindir "${ADDITIONALPLUGINSPATH}"

'@
    $template = Replace-ExactlyOnce $template '# additional plugins\r?\n!addplugindir "\$\{ADDITIONALPLUGINSPATH\}"\r?\n\r?\n' ($pluginRegistration + $helperPreamble + "`n") "plugin registration"

    $detectInstallation = @'
; 4. Detect existing NSIS or Windows Installer products without replacing them.
Function DetectExistingInstallation
  StrCpy $R0 0
  StrCpy $0 0
  wix_loop:
    EnumRegKey $1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall" $0
    StrCmp $1 "" wix_loop_done
    IntOp $0 $0 + 1
    ReadRegStr $R1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "DisplayName"
    ReadRegStr $R2 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "Publisher"
    StrCmp "$R1$R2" "${PRODUCTNAME}${MANUFACTURER}" 0 wix_loop
    ReadRegStr $R1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "UninstallString"
    ${StrCase} $R2 $R1 "L"
    ${StrLoc} $R1 $R2 "msiexec" ">"
    StrCmp $R1 0 wix_loop
    Abort "A Windows Installer version of CopyPaste must be removed manually before installing this version."
  wix_loop_done:

  ReadRegStr $R1 SHCTX "${UNINSTKEY}" ""
  ReadRegStr $R2 SHCTX "${UNINSTKEY}" "UninstallString"
  StrCmp "$R1$R2" "" existing_install_done
  ReadRegStr $OldMainBinaryName SHCTX "${UNINSTKEY}" "MainBinaryName"
  ${If} $OldMainBinaryName != ""
  ${AndIf} $OldMainBinaryName != "${MAINBINARYNAME}.exe"
    Abort "The installed CopyPaste layout cannot be verified. This installer cannot continue."
  ${EndIf}
  ReadRegStr $R1 SHCTX "${UNINSTKEY}" "DisplayVersion"
  ${If} $R1 == ""
    Abort "The installed CopyPaste version cannot be verified. This installer cannot continue."
  ${EndIf}
  nsis_tauri_utils::SemverCompare "${VERSION}" $R1
  Pop $R0
  ${If} $R0 <> -1
  ${AndIf} $R0 <> 0
  ${AndIf} $R0 <> 1
    Abort "The installed CopyPaste version cannot be verified. This installer cannot continue."
  ${EndIf}
  !if "${ALLOWDOWNGRADES}" == "false"
    ${If} $R0 = -1
      Abort "A newer version of CopyPaste is already installed. This installer cannot continue."
    ${EndIf}
  !endif
  existing_install_done:
FunctionEnd

'@
    $template = Replace-ExactlyOnce $template "(?s); 4\. Custom page to ask user if he wants to reinstall/uninstall.*?FunctionEnd\r?\n\r?\n; 5\. Choose install directory page" ($detectInstallation + "; 5. Choose install directory page") "maintenance page"
    $template = Replace-ExactlyOnce $template "(?s)Section EarlyChecks.*?SectionEnd" "Section EarlyChecks`nSectionEnd" "silent downgrade section"

    $installStart = @'
Section Install
  Call RefuseIfCurrentUserAppIsRunning
  Call StopCopyPasteForInstaller

  ClearErrors
  SetOutPath $INSTDIR
  ${If} ${Errors}
    Abort "CopyPaste could not prepare the installation. Try again."
  ${EndIf}
  SetOverwrite try

  ; Copy main executable
  ClearErrors
  File "${MAINBINARYSRCPATH}"
  ${If} ${Errors}
    Abort "CopyPaste could not update safely. Try again."
  ${EndIf}
'@
    $template = Replace-ExactlyOnce $template '(?s)Section Install\r?\n.*?  ; Copy main executable\r?\n  File "\$\{MAINBINARYSRCPATH\}"\r?\n' $installStart "install handoff and main payload"

    $payload = @'
  ; Copy resources except the helper, which remains private to the installer.
  {{#each resources_dirs}}
    ClearErrors
    CreateDirectory "$INSTDIR\\{{this}}"
    ${If} ${Errors}
      Abort "CopyPaste could not update safely. Try again."
    ${EndIf}
  {{/each}}
  {{#each resources}}
    !if "{{this.[1]}}" != "${INSTALLER_HELPER_TARGET}"
      ClearErrors
      File /a "/oname={{this.[1]}}" "{{no-escape @key}}"
      ${If} ${Errors}
        Abort "CopyPaste could not update safely. Try again."
      ${EndIf}
    !endif
  {{/each}}

  ; Copy external binaries only after the main executable succeeded.
  {{#each binaries}}
    ClearErrors
    File /a "/oname={{this}}" "{{no-escape @key}}"
    ${If} ${Errors}
      Abort "CopyPaste could not update safely. Try again."
    ${EndIf}
  {{/each}}

  ClearErrors
  WriteUninstaller "$INSTDIR\uninstall.exe"
  ${If} ${Errors}
    Abort "CopyPaste could not update safely. Try again."
  ${EndIf}
'@
    $template = Replace-ExactlyOnce $template "(?s)  ; Copy resources\r?\n.*?  ; Create file associations" ($payload + "`n  ; Create file associations") "payload files"
    $template = Replace-ExactlyOnce $template '  ; Create uninstaller\r?\n  WriteUninstaller "\$INSTDIR\\uninstall\.exe"\r?\n\r?\n' "" "late uninstaller creation"
    $template = Replace-ExactlyOnce $template '(?s)  ; Remove old main binary if it doesn''t match new main binary name\r?\n  ReadRegStr \$OldMainBinaryName SHCTX "\$\{UNINSTKEY\}" "MainBinaryName"\r?\n  \$\{If\} \$OldMainBinaryName != ""\r?\n  \$\{AndIf\} \$OldMainBinaryName != "\$\{MAINBINARYNAME\}\.exe"\r?\n    Delete "\$INSTDIR\\\$OldMainBinaryName"\r?\n  \$\{EndIf\}\r?\n\r?\n' "" "unsafe old main deletion"

    $installerInit = @'
  !if "${INSTALLMODE}" == "both"
    !insertmacro MULTIUSER_INIT
  !endif

  !insertmacro ExtractInstallerHelper
  Call DetectExistingInstallation
FunctionEnd
'@
    $template = Replace-ExactlyOnce $template '  !if "\$\{INSTALLMODE\}" == "both"\r?\n    !insertmacro MULTIUSER_INIT\r?\n  !endif\r?\nFunctionEnd' $installerInit "installer initialization"
    $uninstallerInit = @'
Function un.onInit
  !insertmacro SetContext

  !if "${INSTALLMODE}" == "both"
    !insertmacro MULTIUSER_UNINIT
  !endif

  !insertmacro MUI_UNGETLANGUAGE

  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}

  !insertmacro ExtractInstallerHelper
FunctionEnd
'@
    $template = Replace-ExactlyOnce $template "(?s)Function un\.onInit\r?\n.*?FunctionEnd" $uninstallerInit "uninstaller initialization"

    $uninstallStart = @'
Section Uninstall
  Call un.RefuseIfCurrentUserAppIsRunning
  Call un.StopCopyPasteForInstaller

  ClearErrors
  Delete "$INSTDIR\${MAINBINARYNAME}.exe"
  ${If} ${Errors}
    Abort "CopyPaste could not remove safely. Close it and try again."
  ${EndIf}

  {{#each resources}}
    !if "{{this.[1]}}" != "${INSTALLER_HELPER_TARGET}"
      ClearErrors
      Delete "$INSTDIR\\{{this.[1]}}"
      ${If} ${Errors}
        Abort "CopyPaste could not remove safely. Close it and try again."
      ${EndIf}
    !endif
  {{/each}}

  {{#each binaries}}
    ClearErrors
    Delete "$INSTDIR\\{{this}}"
    ${If} ${Errors}
      Abort "CopyPaste could not remove safely. Close it and try again."
    ${EndIf}
  {{/each}}

  ClearErrors
  Delete "$INSTDIR\uninstall.exe"
  ${If} ${Errors}
    Abort "CopyPaste could not remove safely. Close it and try again."
  ${EndIf}

  ; Delete app associations

'@
    $template = Replace-ExactlyOnce $template "(?s)Section Uninstall\r?\n.*?  ; Delete app associations\r?\n" $uninstallStart "uninstall handoff and payload"
    $template = Replace-ExactlyOnce $template '  ; Delete uninstaller\r?\n  Delete "\$INSTDIR\\uninstall\.exe"\r?\n\r?\n' "" "late uninstaller deletion"
    $template = $template.Replace("RMDir /REBOOTOK", "RMDir")
    $startMenuMigration = @'
  ; Skip creating shortcuts in updater and no-shortcut modes.
  ${If} $UpdateMode = 1
  ${OrIf} $NoShortcutMode = 1
    Return
  ${EndIf}

  !if "${STARTMENUFOLDER}" != ""
'@
    $template = Replace-ExactlyOnce $template '(?s)  ; Skip creating shortcut if in update mode or no shortcut mode\r?\n  ; but always create if migrating from wix\r?\n  \$\{If\} \$WixMode = 0\r?\n    \$\{If\} \$UpdateMode = 1\r?\n    \$\{OrIf\} \$NoShortcutMode = 1\r?\n      Return\r?\n    \$\{EndIf\}\r?\n  \$\{EndIf\}\r?\n\r?\n  !if "\$\{STARTMENUFOLDER\}" != ""' $startMenuMigration "start-menu Wix migration"
    $desktopMigration = @'
  ; Skip creating shortcuts in updater and no-shortcut modes.
  ${If} $UpdateMode = 1
  ${OrIf} $NoShortcutMode = 1
    Return
  ${EndIf}

  CreateShortcut "$DESKTOP
'@
    $template = Replace-ExactlyOnce $template '(?s)  ; Skip creating shortcut if in update mode or no shortcut mode\r?\n  ; but always create if migrating from wix\r?\n  \$\{If\} \$WixMode = 0\r?\n    \$\{If\} \$UpdateMode = 1\r?\n    \$\{OrIf\} \$NoShortcutMode = 1\r?\n      Return\r?\n    \$\{EndIf\}\r?\n  \$\{EndIf\}\r?\n\r?\n  CreateShortcut "\$DESKTOP' $desktopMigration "desktop Wix migration"

    foreach ($forbidden in @("CheckIfAppIsRunning", "KillProcess", "WixMode", "ReinstallPageCheck", "uninstallBeforeInstalling", 'ExecWait ''$R1''')) {
        if ($template.Contains($forbidden)) { throw "installer template drift: forbidden legacy control flow $forbidden remains" }
    }
    foreach ($message in @(
        "CopyPaste could not prepare the installer. Try again.",
        "CopyPaste is running. Close it and try again.",
        "CopyPaste could not stop safely. Close it and try again.",
        "CopyPaste could not prepare the installation. Try again.",
        "CopyPaste could not update safely. Try again.",
        "CopyPaste could not remove safely. Close it and try again.",
        "A Windows Installer version of CopyPaste must be removed manually before installing this version.",
        "The installed CopyPaste layout cannot be verified. This installer cannot continue.",
        "The installed CopyPaste version cannot be verified. This installer cannot continue.",
        "A newer version of CopyPaste is already installed. This installer cannot continue."
    )) {
        $template = $template.Replace("Abort `"$message`"", "SetErrorLevel 1`n    Abort `"$message`"")
    }
    $header = "; Generated by scripts/release/windows-installer-template.ps1; do not edit.`n; Source: https://github.com/tauri-apps/tauri/blob/$expectedTauriTag/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi`n; Tauri tag: $expectedTauriTag; SHA-256: $expectedInstallerHash`n; License: installer.upstream.LICENSE-MIT; SHA-256: $expectedLicenseHash`n`n"
    return ($header + $template).Replace("`r`n", "`n")
}

function Invoke-SelfTest([string]$Root) {
    $template = Get-GeneratedTemplate $Root
    if ($template.Contains("`r`n")) { throw "installer template must use LF line endings" }
    foreach ($needle in @('FindProcessCurrentUser', 'shutdown --wait-for-exit', 'File "${MAINBINARYSRCPATH}"', 'SetOverwrite try', 'Abort "CopyPaste could not update safely. Try again."', 'Abort "A newer version of CopyPaste is already installed.')) {
        if (-not $template.Contains($needle)) { throw "installer template self-test missing $needle" }
    }
    foreach ($forbidden in @("KillProcess", "CheckIfAppIsRunning", "RMDir /REBOOTOK", "uninstallBeforeInstalling", 'ExecWait ''$R1''')) {
        if ($template.Contains($forbidden)) { throw "installer template self-test retained $forbidden" }
    }
    $main = $template.IndexOf('File "${MAINBINARYSRCPATH}"', [StringComparison]::Ordinal)
    $resources = $template.IndexOf('; Copy resources except the helper', [StringComparison]::Ordinal)
    $associations = $template.IndexOf('; Create file associations', [StringComparison]::Ordinal)
    if ($main -lt 0 -or $resources -lt $main -or $associations -lt $resources) { throw "installer payload order self-test failed" }
    $validResourceSet = [pscustomobject]@{ $helperSource = $helperTarget }
    Assert-HelperResource $validResourceSet "synthetic helper resource"
    foreach ($badResourceSet in @(
        [pscustomobject]@{},
        [pscustomobject]@{ wrong = $helperTarget },
        [pscustomobject]@{ $helperSource = $helperTarget; duplicate = $helperTarget }
    )) {
        $accepted = $true
        try { Assert-HelperResource $badResourceSet "synthetic helper resource" } catch { $accepted = $false }
        if ($accepted) { throw "unexpected helper acceptance" }
    }
    Write-Host "PASS: Windows installer template contract"
}

$repoRoot = Get-RepositoryRoot
if ($SelfTest) {
    Invoke-SelfTest $repoRoot
    exit 0
}

$generated = Get-GeneratedTemplate $repoRoot
$destination = Join-Path $repoRoot "crates/copypaste-ui/src-tauri/windows/installer.nsi"
if ($Check) {
    if (-not (Test-Path -LiteralPath $destination -PathType Leaf) -or [IO.File]::ReadAllText($destination) -cne $generated) {
        throw "installer template is stale; run scripts/release/windows-installer-template.ps1"
    }
    Write-Host "PASS: Windows installer template is current"
    exit 0
}

[IO.File]::WriteAllText($destination, $generated, [Text.UTF8Encoding]::new($false))
Write-Host "Generated $destination"
