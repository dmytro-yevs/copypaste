$script:WindowsUiaClientProviderReady = $false
$script:WindowsUiaClientProviderBootstrap = $null

function Get-WindowsUiaProviderAssemblyName([Reflection.AssemblyName]$ClientAssembly) {
    $provider = [Reflection.AssemblyName]::new()
    $provider.Name = "UIAutomationClientsideProviders"
    $provider.Version = $ClientAssembly.Version
    $provider.CultureInfo = $ClientAssembly.CultureInfo
    $provider.SetPublicKeyToken($ClientAssembly.GetPublicKeyToken())
    return $provider
}

function Format-WindowsUiaAssemblyIdentity([Reflection.AssemblyName]$AssemblyName) {
    $token = @($AssemblyName.GetPublicKeyToken() | ForEach-Object { $_.ToString("x2") }) -join ""
    return "$($AssemblyName.Name), Version=$($AssemblyName.Version), PublicKeyToken=$token"
}

function Get-LoadedWindowsUiaProviderIdentities {
    return @(
        [AppDomain]::CurrentDomain.GetAssemblies() |
            Where-Object { $_.GetName().Name -eq "UIAutomationClientsideProviders" } |
            ForEach-Object { Format-WindowsUiaAssemblyIdentity $_.GetName() } |
            Sort-Object -Unique
    )
}

function New-WindowsUiaCanaryExpectation(
    [string]$Role,
    [string]$ClassName,
    [string]$ControlType,
    [bool]$IsPassword,
    [string]$StyleFlags
) {
    return [ordered]@{
        role = $Role
        class_name = $ClassName
        control_type = $ControlType
        is_password = $IsPassword
        style_flags = $StyleFlags
    }
}

function Get-WindowsUiaCanaryExpectations {
    return @(
        New-WindowsUiaCanaryExpectation "password-edit" "Edit" "ControlType.Edit" $true `
            "WS_CHILD|WS_VISIBLE|WS_BORDER|ES_PASSWORD"
        New-WindowsUiaCanaryExpectation "button" "Button" "ControlType.Button" $false `
            "WS_CHILD|WS_VISIBLE|WS_BORDER"
        New-WindowsUiaCanaryExpectation "static" "Static" "ControlType.Text" $false `
            "WS_CHILD|WS_VISIBLE|WS_BORDER"
    )
}

function Test-WindowsUiaCanaryMappings([object[]]$Facts) {
    $expected = @(Get-WindowsUiaCanaryExpectations)
    if (@($Facts).Count -ne $expected.Count) { return $false }
    foreach ($requirement in $expected) {
        $matches = @($Facts | Where-Object {
            $_ -is [Collections.IDictionary] -and
            $_["role"] -eq $requirement["role"] -and
            $_["class_name"] -eq $requirement["class_name"] -and
            $_["control_type"] -eq $requirement["control_type"] -and
            $_["is_password"] -eq $requirement["is_password"] -and
            $_["style_flags"] -eq $requirement["style_flags"]
        })
        if ($matches.Count -ne 1) { return $false }
    }
    return $true
}

function Add-WindowsUiaCanaryFixtureType {
    if ($null -ne ("CopyPasteUiaCanarySession" -as [type])) { return }
    Add-Type -TypeDefinition @'
using System;
using System.Threading;
using System.Windows.Forms;

public sealed class CopyPasteUiaCanaryWindow : NativeWindow
{
    public void Create(string className, int style, IntPtr parent, int x, int y)
    {
        CreateParams parameters = new CreateParams();
        parameters.ClassName = className;
        parameters.Caption = String.Empty;
        parameters.Style = style;
        parameters.Parent = parent;
        parameters.X = x;
        parameters.Y = y;
        parameters.Width = 40;
        parameters.Height = 22;
        CreateHandle(parameters);
    }

    public void Close()
    {
        if (Handle != IntPtr.Zero)
            DestroyHandle();
    }
}

public sealed class CopyPasteUiaCanarySession : IDisposable
{
    private const int ChildVisibleBorder = 0x50800000;
    private const int PasswordEditStyle = ChildVisibleBorder | 0x0020;
    private readonly ManualResetEvent ready = new ManualResetEvent(false);
    private readonly ManualResetEvent closed = new ManualResetEvent(false);
    private Thread thread;
    private Form form;
    private Exception failure;
    private CopyPasteUiaCanaryWindow passwordEdit;
    private CopyPasteUiaCanaryWindow button;
    private CopyPasteUiaCanaryWindow staticText;

    public IntPtr PasswordEditHandle { get; private set; }
    public IntPtr ButtonHandle { get; private set; }
    public IntPtr StaticHandle { get; private set; }

    public static CopyPasteUiaCanarySession Start()
    {
        CopyPasteUiaCanarySession session = new CopyPasteUiaCanarySession();
        session.StartCore();
        return session;
    }

    private void StartCore()
    {
        thread = new Thread(new ThreadStart(ThreadMain));
        thread.IsBackground = true;
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        if (!ready.WaitOne(5000) || failure != null)
        {
            Dispose();
            throw new InvalidOperationException("UIA canary fixture could not start.");
        }
    }

    private void ThreadMain()
    {
        try
        {
            form = new Form();
            form.Text = String.Empty;
            form.ShowInTaskbar = false;
            form.StartPosition = FormStartPosition.Manual;
            form.Left = -32000;
            form.Top = -32000;
            form.Width = 160;
            form.Height = 80;
            IntPtr parent = form.Handle;
            passwordEdit = new CopyPasteUiaCanaryWindow();
            button = new CopyPasteUiaCanaryWindow();
            staticText = new CopyPasteUiaCanaryWindow();
            passwordEdit.Create("Edit", PasswordEditStyle, parent, 4, 4);
            button.Create("Button", ChildVisibleBorder, parent, 48, 4);
            staticText.Create("Static", ChildVisibleBorder, parent, 92, 4);
            PasswordEditHandle = passwordEdit.Handle;
            ButtonHandle = button.Handle;
            StaticHandle = staticText.Handle;
            form.Show();
            ready.Set();
            Application.Run(form);
        }
        catch (Exception error)
        {
            failure = error;
        }
        finally
        {
            if (passwordEdit != null) passwordEdit.Close();
            if (button != null) button.Close();
            if (staticText != null) staticText.Close();
            if (form != null) form.Dispose();
            ready.Set();
            closed.Set();
        }
    }

    public void Dispose()
    {
        Form current = form;
        if (current != null && current.IsHandleCreated)
        {
            try { current.BeginInvoke(new MethodInvoker(current.Close)); }
            catch (InvalidOperationException) { }
        }
        if (thread != null && thread.IsAlive)
            closed.WaitOne(5000);
    }
}
'@ -ReferencedAssemblies "System.Windows.Forms"
}

function Read-WindowsUiaCanaryControl(
    [IntPtr]$Handle,
    [Collections.IDictionary]$Expectation
) {
    $element = [Windows.Automation.AutomationElement]::FromHandle($Handle)
    return [ordered]@{
        role = $Expectation["role"]
        class_name = $element.Current.ClassName
        control_type = $element.Current.ControlType.ProgrammaticName
        is_password = $element.Current.IsPassword
        style_flags = $Expectation["style_flags"]
    }
}

function Invoke-WindowsUiaCanaryProbe(
    [scriptblock]$Start,
    [scriptblock]$Read,
    [scriptblock]$Close
) {
    $session = $null
    $facts = $null
    $failure = $null
    try {
        $session = & $Start
        $facts = @(& $Read $session)
        if (-not (Test-WindowsUiaCanaryMappings $facts)) {
            throw "UIA canary provider mapping mismatch"
        }
    } catch {
        $failure = $_
    } finally {
        if ($null -ne $session) {
            try { & $Close $session } catch { }
        }
    }
    if ($null -ne $failure) {
        throw "Windows UI Automation client provider canary failed. The native control observer is unavailable."
    }
    return $facts
}

function Invoke-WindowsUiaClientProviderCanary {
    Add-WindowsUiaCanaryFixtureType
    $probe = @{
        Start = { [CopyPasteUiaCanarySession]::Start() }
        Read = {
            param($session)
            $expected = @(Get-WindowsUiaCanaryExpectations)
            @(
                Read-WindowsUiaCanaryControl $session.PasswordEditHandle $expected[0]
                Read-WindowsUiaCanaryControl $session.ButtonHandle $expected[1]
                Read-WindowsUiaCanaryControl $session.StaticHandle $expected[2]
            )
        }
        Close = { param($session) $session.Dispose() }
    }
    return Invoke-WindowsUiaCanaryProbe @probe
}

function Initialize-WindowsUiaClientProviders {
    if ($script:WindowsUiaClientProviderReady) { return $script:WindowsUiaClientProviderBootstrap }
    $client = [Windows.Automation.ClientSettings].Assembly.GetName()
    $provider = Get-WindowsUiaProviderAssemblyName $client
    $before = @(Get-LoadedWindowsUiaProviderIdentities)
    try {
        [Windows.Automation.ClientSettings]::RegisterClientSideProviderAssembly($provider)
        $after = @(Get-LoadedWindowsUiaProviderIdentities)
        if ($after.Count -eq 0) { throw "client-side provider assembly is not loaded" }
        $canary = @(Invoke-WindowsUiaClientProviderCanary)
        if (-not (Test-WindowsUiaCanaryMappings $canary)) { throw "client-side provider canary mismatch" }
    } catch {
        throw "Windows UI Automation environment bootstrap failed: explicit client-side provider registration and the native-control canary must succeed before product evidence runs."
    }
    $script:WindowsUiaClientProviderReady = $true
    $script:WindowsUiaClientProviderBootstrap = [ordered]@{
        client_assembly = Format-WindowsUiaAssemblyIdentity $client
        requested_provider_assembly = Format-WindowsUiaAssemblyIdentity $provider
        provider_assemblies_before = $before
        provider_assemblies_after = $after
        controls = $canary
    }
    return $script:WindowsUiaClientProviderBootstrap
}

function Test-WindowsUiaClientBootstrapHelpers {
    $client = [Reflection.AssemblyName]::new("UIAutomationClient, Version=4.0.0.0, Culture=neutral, PublicKeyToken=31bf3856ad364e35")
    $provider = Get-WindowsUiaProviderAssemblyName $client
    if ($provider.Name -ne "UIAutomationClientsideProviders") {
        throw "UIA provider identity did not use the default provider assembly"
    }
    if ($provider.Version -ne $client.Version) { throw "UIA provider identity did not retain the client version" }
    if ((Format-WindowsUiaAssemblyIdentity $provider) -notmatch "UIAutomationClientsideProviders, Version=4.0.0.0") {
        throw "UIA provider report omitted its allowlisted assembly identity"
    }
    $facts = @(Get-WindowsUiaCanaryExpectations)
    if (-not (Test-WindowsUiaCanaryMappings $facts)) { throw "the native UIA canary expectations were rejected" }
    $facts[0]["is_password"] = $false
    if (Test-WindowsUiaCanaryMappings $facts) { throw "the native UIA canary accepted an unprotected Edit" }
    $cleanup = [ordered]@{ called = $false }
    $failed = $null
    try {
        $probe = @{
            Start = { [pscustomobject]@{ fixture = $true } }
            Read = { param($session) throw "C:\\private\\canary-failure" }
            Close = { param($session) $cleanup["called"] = $true }
        }
        Invoke-WindowsUiaCanaryProbe @probe | Out-Null
    } catch {
        $failed = $_.Exception.Message
    }
    if (-not $cleanup["called"] -or $failed -ne "Windows UI Automation client provider canary failed. The native control observer is unavailable.") {
        throw "the UIA canary did not fail closed and clean up"
    }
    $source = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "windows-native-ui-evidence.ps1"))
    $bootstrap = $source.IndexOf("Initialize-WindowsUiaClientProviders")
    $productElement = $source.IndexOf("[Windows.Automation.AutomationElement]::FromHandle")
    if ($bootstrap -lt 0 -or $bootstrap -ge $productElement) {
        throw "UIA provider bootstrap did not precede the first product AutomationElement"
    }
    if (${function:Invoke-WindowsUiaClientProviderCanary}.ToString() -match "ValuePattern|Exception.Message|Screenshot") {
        throw "UIA provider bootstrap exposed a protected value, raw exception, or screenshot path"
    }
}
