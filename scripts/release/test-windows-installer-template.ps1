$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$scriptPath = Join-Path $PSScriptRoot "windows-installer-template.ps1"
& $scriptPath -SelfTest
& $scriptPath -Check

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
$template = [IO.File]::ReadAllText((Join-Path $repoRoot "crates/copypaste-ui/src-tauri/windows/installer.nsi"))

function Assert-ExactlyOneGuard([string]$Name, [string]$Pattern) {
    $count = [regex]::Matches($template, $Pattern, [Text.RegularExpressions.RegexOptions]::Singleline).Count
    if ($count -ne 1) { throw "$Name must have exactly one immediate fail-closed guard; found $count" }
}

$pluginRegistration = $template.IndexOf('!addplugindir "${ADDITIONALPLUGINSPATH}"', [StringComparison]::Ordinal)
$helperDefinition = $template.IndexOf('!macro ExtractInstallerHelper', [StringComparison]::Ordinal)
if ($pluginRegistration -lt 0 -or $helperDefinition -le $pluginRegistration) {
    throw "installer helper definitions must follow Tauri's maintained plugin registration"
}
if (-not $template.Contains("!insertmacro DefineCliDrain un.StopCopyPasteForInstaller uninstaller_stop`n; Uninstaller signing command")) {
    throw "installer helper macro expansion must end before the upstream uninstaller signing command"
}
$uninstallNamespace = [regex]::Match($template, '(?s)Section Uninstall\r?\n(.*?)\r?\n  ClearErrors').Groups[1].Value
if ([string]::IsNullOrWhiteSpace($uninstallNamespace) -or
    -not $uninstallNamespace.Contains('Call un.RefuseIfCurrentUserAppIsRunning') -or
    -not $uninstallNamespace.Contains('Call un.StopCopyPasteForInstaller') -or
    $uninstallNamespace.Contains("`n  Call RefuseIfCurrentUserAppIsRunning") -or
    $uninstallNamespace.Contains("`n  Call StopCopyPasteForInstaller")) {
    throw "uninstaller must call only un-prefixed helper functions"
}
foreach ($required in @(
    "!insertmacro ExtractInstallerHelper",
    "FindProcessCurrentUser",
    "shutdown --wait-for-exit",
    "SetOverwrite try",
    "!insertmacro DefineGuiRefusal un.RefuseIfCurrentUserAppIsRunning uninstaller_gui",
    "!insertmacro DefineCliDrain un.StopCopyPasteForInstaller uninstaller_stop",
    "A Windows Installer version of CopyPaste must be removed manually",
    "The installed CopyPaste layout cannot be verified",
    "The installed CopyPaste version cannot be verified",
    "A newer version of CopyPaste is already installed",
    "Delete `"`$INSTDIR\`${MAINBINARYNAME}.exe`"",
    "`${If} `$UpdateMode <> 1"
)) {
    if (-not $template.Contains($required)) { throw "installer contract missing $required" }
}
foreach ($forbidden in @("KillProcess", "CheckIfAppIsRunning", "RMDir /REBOOTOK", "uninstallBeforeInstalling", 'ExecWait ''$R1''')) {
    if ($template.Contains($forbidden)) { throw "installer contract retained $forbidden" }
}
foreach ($legacy in @('Delete "$INSTDIR\$OldMainBinaryName"', '${If} $0 <> 0')) {
    if ($template.Contains($legacy)) { throw "installer contract retained unsafe control flow $legacy" }
}
if ([regex]::Matches($template, '(?s)Abort "CopyPaste (?:is running|could not).+?"').Count -ne
    [regex]::Matches($template, '(?s)SetErrorLevel 1\s+Abort "CopyPaste (?:is running|could not).+?"').Count) {
    throw "installer failure paths must set a nonzero exit level before Abort"
}
Assert-ExactlyOneGuard "GUI presence refusal" '(?s)!macro DefineGuiRefusal FunctionName LabelPrefix.*?ClearErrors.*?FindProcessCurrentUser.*?\$\{If\} \$\{Errors\}.*?Goto \$\{LabelPrefix\}_refuse.*?Pop \$0.*?\$\{If\} \$\{Errors\}.*?Goto \$\{LabelPrefix\}_refuse.*?StrCmp \$0 "1" \$\{LabelPrefix\}_absent.*?\$\{LabelPrefix\}_refuse:.*?SetErrorLevel 1.*?Abort "CopyPaste is running\. Close it and try again\.".*?!macroend'
Assert-ExactlyOneGuard "CLI drain refusal" '(?s)!macro DefineCliDrain FunctionName LabelPrefix.*?ClearErrors.*?nsExec::ExecToStack.*?\$\{If\} \$\{Errors\}.*?Goto \$\{LabelPrefix\}_refuse.*?Pop \$0.*?\$\{If\} \$\{Errors\}.*?Goto \$\{LabelPrefix\}_refuse.*?Pop \$1.*?\$\{If\} \$\{Errors\}.*?Goto \$\{LabelPrefix\}_refuse.*?StrCmp \$0 "0" \$\{LabelPrefix\}_done.*?\$\{LabelPrefix\}_refuse:.*?SetErrorLevel 1.*?Abort "CopyPaste could not stop safely\. Close it and try again\.".*?!macroend'

Assert-ExactlyOneGuard "main executable write" 'ClearErrors\s+File "\$\{MAINBINARYSRCPATH\}"\s+\$\{If\} \$\{Errors\}\s+SetErrorLevel 1\s+Abort "CopyPaste could not update safely\. Try again\."\s+\$\{EndIf\}'
Assert-ExactlyOneGuard "resource directory creation" '\{\{#each resources_dirs\}\}\s+ClearErrors\s+CreateDirectory.*?\s+\$\{If\} \$\{Errors\}\s+SetErrorLevel 1\s+Abort "CopyPaste could not update safely\. Try again\."\s+\$\{EndIf\}\s+\{\{/each\}\}'
Assert-ExactlyOneGuard "resource write" '\{\{#each resources\}\}\s+!if.*?\s+ClearErrors\s+File /a.*?\s+\$\{If\} \$\{Errors\}\s+SetErrorLevel 1\s+Abort "CopyPaste could not update safely\. Try again\."\s+\$\{EndIf\}\s+!endif\s+\{\{/each\}\}'
Assert-ExactlyOneGuard "external binary write" '\{\{#each binaries\}\}\s+ClearErrors\s+File /a.*?\s+\$\{If\} \$\{Errors\}\s+SetErrorLevel 1\s+Abort "CopyPaste could not update safely\. Try again\."\s+\$\{EndIf\}\s+\{\{/each\}\}\s+ClearErrors\s+WriteUninstaller'
Assert-ExactlyOneGuard "uninstaller write" 'ClearErrors\s+WriteUninstaller "\$INSTDIR\\uninstall\.exe"\s+\$\{If\} \$\{Errors\}\s+SetErrorLevel 1\s+Abort "CopyPaste could not update safely\. Try again\."\s+\$\{EndIf\}\s+; Create file associations'
Assert-ExactlyOneGuard "main executable deletion" 'ClearErrors\s+Delete "\$INSTDIR\\\$\{MAINBINARYNAME\}\.exe"\s+\$\{If\} \$\{Errors\}\s+SetErrorLevel 1\s+Abort "CopyPaste could not remove safely\. Close it and try again\."\s+\$\{EndIf\}'
Assert-ExactlyOneGuard "resource deletion" '\{\{#each resources\}\}\s+!if.*?\s+ClearErrors\s+Delete.*?\s+\$\{If\} \$\{Errors\}\s+SetErrorLevel 1\s+Abort "CopyPaste could not remove safely\. Close it and try again\."\s+\$\{EndIf\}\s+!endif\s+\{\{/each\}\}'
Assert-ExactlyOneGuard "external binary deletion" '\{\{#each binaries\}\}\s+ClearErrors\s+Delete.*?\s+\$\{If\} \$\{Errors\}\s+SetErrorLevel 1\s+Abort "CopyPaste could not remove safely\. Close it and try again\."\s+\$\{EndIf\}\s+\{\{/each\}\}\s+ClearErrors\s+Delete "\$INSTDIR\\uninstall\.exe"'
Assert-ExactlyOneGuard "uninstaller deletion" 'ClearErrors\s+Delete "\$INSTDIR\\uninstall\.exe"\s+\$\{If\} \$\{Errors\}\s+SetErrorLevel 1\s+Abort "CopyPaste could not remove safely\. Close it and try again\."\s+\$\{EndIf\}\s+; Delete app associations'
$main = $template.IndexOf('File "${MAINBINARYSRCPATH}"', [StringComparison]::Ordinal)
$resources = $template.IndexOf('; Copy resources except the helper', [StringComparison]::Ordinal)
$external = $template.IndexOf('; Copy external binaries only after the main executable succeeded.', [StringComparison]::Ordinal)
$writeUninstaller = $template.IndexOf('WriteUninstaller "$INSTDIR\uninstall.exe"', [StringComparison]::Ordinal)
$registry = $template.IndexOf('; Registry information for add/remove programs', [StringComparison]::Ordinal)
if ($main -lt 0 -or $resources -lt $main -or $external -lt $resources -or $writeUninstaller -lt $external -or $registry -lt $writeUninstaller) {
    throw "installer payload and registry ordering contract failed"
}
$uninstall = $template.IndexOf('Section Uninstall', [StringComparison]::Ordinal)
$deleteMain = $template.IndexOf('Delete "$INSTDIR\${MAINBINARYNAME}.exe"', [StringComparison]::Ordinal)
$deleteUninstaller = $template.IndexOf('Delete "$INSTDIR\uninstall.exe"', [StringComparison]::Ordinal)
$uninstallAssociations = $template.IndexOf('; Delete app associations', [StringComparison]::Ordinal)
if ($uninstall -lt 0 -or $deleteMain -lt $uninstall -or $deleteUninstaller -lt $deleteMain -or $uninstallAssociations -lt $deleteUninstaller) {
    throw "uninstaller deletion ordering contract failed"
}
$layoutProbe = $template.IndexOf('ReadRegStr $OldMainBinaryName', [StringComparison]::Ordinal)
if ($layoutProbe -lt 0 -or $layoutProbe -gt $main -or
    -not $template.Contains('Abort "The installed CopyPaste layout cannot be verified. This installer cannot continue."')) {
    throw "old-main layout refusal must precede installer mutation"
}
& (Join-Path $PSScriptRoot "build-windows.ps1") -SelfTest
Write-Host "PASS: Windows installer template regression checks"
