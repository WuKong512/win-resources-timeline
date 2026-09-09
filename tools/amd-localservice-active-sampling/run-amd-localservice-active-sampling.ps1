[CmdletBinding()]
param(
    [ValidateSet('DryRun', 'Live')]
    [string]$Mode = 'DryRun',
    [switch]$AuthorizeLiveRun,
    [string]$AuthorizationToken,
    [string]$ReportPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ToolRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $ToolRoot 'contract.ps1')
. (Join-Path $ToolRoot '..\amd-uprof-cli-spike\postprocess.ps1')

function ConvertTo-JsonText {
    param([Parameter(Mandatory = $true)]$Value)
    $Value | ConvertTo-Json -Depth 50
}

function Write-JsonAtomic {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value,
        [switch]$AllowReplace
    )

    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    if ((Test-Path -LiteralPath $Path) -and -not $AllowReplace) {
        throw "refusing to replace immutable evidence file: $Path"
    }
    $temp = Join-Path $parent ('.pending-{0}.json' -f ([Guid]::NewGuid().ToString('N')))
    try {
        $encoding = New-Object System.Text.UTF8Encoding($false)
        [IO.File]::WriteAllText($temp, (ConvertTo-JsonText -Value $Value), $encoding)
        if ($AllowReplace) {
            Move-Item -LiteralPath $temp -Destination $Path -Force
        }
        else {
            Move-Item -LiteralPath $temp -Destination $Path
        }
    }
    finally {
        if (Test-Path -LiteralPath $temp) {
            Remove-Item -LiteralPath $temp -Force -ErrorAction SilentlyContinue
        }
    }
}

function Get-PeArchitecture {
    param([Parameter(Mandatory = $true)][string]$Path)

    try {
        $bytes = [IO.File]::ReadAllBytes($Path)
        if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
            return 'UNKNOWN'
        }
        $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
        if ($peOffset -lt 0 -or ($peOffset + 6) -gt $bytes.Length) {
            return 'UNKNOWN'
        }
        if ($bytes[$peOffset] -ne 0x50 -or $bytes[$peOffset + 1] -ne 0x45 -or
            $bytes[$peOffset + 2] -ne 0 -or $bytes[$peOffset + 3] -ne 0) {
            return 'UNKNOWN'
        }
        $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
        if ($machine -eq 0x8664) { return 'x64' }
        if ($machine -eq 0x014C) { return 'x86' }
        '0x{0:X4}' -f $machine
    }
    catch {
        'UNKNOWN'
    }
}

function Get-HostBinaryIdentity {
    param([Parameter(Mandatory = $true)]$Contract)

    $path = $Contract.amd_cli_path
    $identity = [ordered]@{
        path = $path
        exists = $false
        sha256 = $null
        file_version = $null
        product_version = $null
        architecture = $null
        signature_status = $null
        signer = $null
        error = $null
    }
    try {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            $identity.error = 'AMD CLI file does not exist'
            return [pscustomobject]$identity
        }
        $identity.exists = $true
        $identity.sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToUpperInvariant()
        $version = [Diagnostics.FileVersionInfo]::GetVersionInfo($path)
        $identity.file_version = $version.FileVersion
        $identity.product_version = $version.ProductVersion
        $identity.architecture = Get-PeArchitecture -Path $path
        $signature = Get-AuthenticodeSignature -FilePath $path
        $identity.signature_status = [string]$signature.Status
        if ($null -ne $signature.SignerCertificate) {
            $identity.signer = $signature.SignerCertificate.Subject
        }
    }
    catch {
        $identity.error = $_.Exception.Message
    }
    [pscustomobject]$identity
}

function Get-HostDriverEvidence {
    param([Parameter(Mandatory = $true)]$Contract)

    $records = New-Object System.Collections.Generic.List[object]
    foreach ($property in $Contract.driver_versions.PSObject.Properties) {
        $expected = $property.Value
        $record = [ordered]@{
            name = $property.Name
            path = $expected.path
            exists = $false
            file_version = $null
            product_version = $null
            sha256 = $null
            signature_status = $null
            signer = $null
            state = $null
            error = $null
        }
        try {
            $record.exists = Test-Path -LiteralPath $expected.path -PathType Leaf
            if ($record.exists) {
                $version = [Diagnostics.FileVersionInfo]::GetVersionInfo($expected.path)
                $record.file_version = $version.FileVersion
                $record.product_version = $version.ProductVersion
                $record.sha256 = (Get-FileHash -LiteralPath $expected.path -Algorithm SHA256).Hash.ToUpperInvariant()
                $signature = Get-AuthenticodeSignature -FilePath $expected.path
                $record.signature_status = [string]$signature.Status
                if ($null -ne $signature.SignerCertificate) {
                    $record.signer = $signature.SignerCertificate.Subject
                }
            }
            $service = Get-Service -Name $property.Name -ErrorAction Stop
            $record.state = [string]$service.Status
        }
        catch {
            $record.error = $_.Exception.Message
        }
        [void]$records.Add([pscustomobject]$record)
    }
    @($records)
}

function Get-PlatformSnapshot {
    $snapshot = [ordered]@{
        captured_at_utc = [DateTime]::UtcNow.ToString('o')
        computer_name = $env:COMPUTERNAME
        powershell = $PSVersionTable.PSVersion.ToString()
        os = $null
        computer = $null
        bios = $null
        device_guard = $null
        errors = @()
    }
    try {
        $snapshot.os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber, OSArchitecture
    }
    catch {
        $snapshot.errors += 'Win32_OperatingSystem read failed'
    }
    try {
        $snapshot.computer = Get-CimInstance Win32_ComputerSystem |
            Select-Object Manufacturer, Model, SystemType, HypervisorPresent
    }
    catch {
        $snapshot.errors += 'Win32_ComputerSystem read failed'
    }
    try {
        $snapshot.bios = Get-CimInstance Win32_BIOS | Select-Object Manufacturer, SMBIOSBIOSVersion, Version
    }
    catch {
        $snapshot.errors += 'Win32_BIOS read failed'
    }
    try {
        $snapshot.device_guard = Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\DeviceGuard' -ErrorAction Stop |
            Select-Object EnableVirtualizationBasedSecurity, RequirePlatformSecurityFeatures, Locked
    }
    catch {
        $snapshot.errors += 'DeviceGuard registry read failed'
    }
    [pscustomobject]$snapshot
}

function Get-GitBaselineEvidence {
    param([Parameter(Mandatory = $true)]$Contract)

    $repoRoot = $null
    $head = $null
    $mainAncestor = $false
    $designAncestor = $false
    $statusEntries = @()
    $statusExitCode = $null
    $errors = New-Object System.Collections.Generic.List[string]
    try {
        $repoPath = (Resolve-Path (Join-Path $ToolRoot '..\..')).Path
        $repoRoot = (& git.exe -C $repoPath rev-parse --show-toplevel 2>&1 | Out-String).Trim()
        $repoExitCode = $LASTEXITCODE
        $head = (& git.exe -C $repoPath rev-parse HEAD 2>&1 | Out-String).Trim()
        $headExitCode = $LASTEXITCODE
        if ($repoExitCode -ne 0 -or $headExitCode -ne 0) {
            [void]$errors.Add('git repository/head resolution failed')
        }
        $statusEntries = @(& git.exe -C $repoPath status --porcelain --untracked-files=all 2>&1 |
            ForEach-Object { [string]$_ } |
            Where-Object { $_ -match '^(?:\?\?|[ MADRCU?!]{2})\s' })
        $statusExitCode = $LASTEXITCODE
        $mainProbe = & git.exe -C $repoPath merge-base --is-ancestor $Contract.expected_main_pin HEAD 2>&1
        $mainAncestor = ($LASTEXITCODE -eq 0)
        $designProbe = & git.exe -C $repoPath merge-base --is-ancestor $Contract.design_checkpoint HEAD 2>&1
        $designAncestor = ($LASTEXITCODE -eq 0)
        if (-not $mainAncestor) { [void]$errors.Add('expected origin/main pin is not an ancestor of HEAD') }
        if (-not $designAncestor) { [void]$errors.Add('design checkpoint is not an ancestor of HEAD') }
        if ($statusExitCode -ne 0) { [void]$errors.Add('working-tree status query failed') }
        if ($statusEntries.Count -ne 0) { [void]$errors.Add('working tree is not clean') }
    }
    catch {
        [void]$errors.Add($_.Exception.Message)
    }
    [pscustomobject]@{
        repository = $repoRoot
        head = $head
        expected_main_pin = $Contract.expected_main_pin
        design_checkpoint = $Contract.design_checkpoint
        main_pin_is_ancestor = $mainAncestor
        design_checkpoint_is_ancestor = $designAncestor
        status_entries = @($statusEntries)
        status_exit_code = $statusExitCode
        clean = ($statusEntries.Count -eq 0 -and $statusExitCode -eq 0)
        valid = ($errors.Count -eq 0)
        errors = @($errors)
    }
}

function Get-ResidualEvidence {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [string]$RunRoot
    )

    $serviceNames = @(
        $Contract.service_name,
        'ResourceTimelineAmdProfileSingleProcessQualification',
        'ResourceTimelineAmdQualification'
    )
    $queryErrors = New-Object System.Collections.Generic.List[string]
    $services = @()
    try {
        $services = @(Get-CimInstance Win32_Service -ErrorAction Stop |
            Where-Object { $serviceNames -contains $_.Name } |
            Select-Object Name, State, StartName, ProcessId, PathName)
    }
    catch {
        [void]$queryErrors.Add('qualification service residue query failed')
    }
    $processes = @()
    try {
        $processes = @(Get-Process -ErrorAction Stop |
            Where-Object { $_.ProcessName -in @('AMDuProfCLI', 'AMDuProf') } |
            Select-Object Id, ProcessName, Path)
    }
    catch {
        [void]$queryErrors.Add('AMD process residue query failed')
    }
    $harnessProcesses = @()
    try {
        $harnessProcesses = @(Get-CimInstance Win32_Process -ErrorAction Stop |
            Where-Object {
                $commandLine = [string]$_.CommandLine
                $commandLine -match '(?i)service-host\.ps1' -and
                $commandLine -match '(?i)-ManifestPath' -and
                ([string]::IsNullOrWhiteSpace($RunRoot) -or
                    $commandLine.IndexOf($RunRoot, [StringComparison]::OrdinalIgnoreCase) -ge 0)
            } |
            Select-Object ProcessId, Name, ExecutablePath, CommandLine)
    }
    catch {
        [void]$queryErrors.Add('harness process residue query failed')
    }
    [pscustomobject]@{
        qualification_services = $services
        amd_processes = $processes
        harness_processes = $harnessProcesses
        query_errors = @($queryErrors)
        clean = ($queryErrors.Count -eq 0 -and $services.Count -eq 0 -and
            $processes.Count -eq 0 -and $harnessProcesses.Count -eq 0)
    }
}

function Get-LocalServiceSid {
    $account = New-Object System.Security.Principal.NTAccount('NT AUTHORITY', 'LocalService')
    $sid = $account.Translate([System.Security.Principal.SecurityIdentifier])
    [pscustomobject]@{
        account = $account.Value
        sid = $sid.Value
        matches = ($sid.Value -ceq 'S-1-5-19')
    }
}

function Test-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    $isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    [pscustomobject]@{
        is_administrator = [bool]$isAdmin
        user = $identity.Name
        sid = if ($null -ne $identity.User) { $identity.User.Value } else { $null }
    }
}

function Get-HarnessIdentity {
    param([Parameter(Mandatory = $true)][string]$Root)

    $files = @(
        (Join-Path $Root 'contract.ps1'),
        (Join-Path $Root 'run-amd-localservice-active-sampling.ps1'),
        (Join-Path $Root 'service-host.ps1')
    )
    $records = foreach ($path in $files) {
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            [pscustomobject]@{
                path = $path
                sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToUpperInvariant()
            }
        }
        else {
            [pscustomobject]@{ path = $path; sha256 = $null; missing = $true }
        }
    }
    [pscustomobject]@{
        source_files = @($records)
        source_root = $Root
    }
}

function Get-LivePreflight {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    $binary = Get-HostBinaryIdentity -Contract $Contract
    $binaryGate = Test-BinaryIdentityEvidence -Identity $binary -Contract $Contract
    $drivers = Get-HostDriverEvidence -Contract $Contract
    $driverGate = Test-DriverVersionEvidence -Drivers $drivers -Contract $Contract
    $git = Get-GitBaselineEvidence -Contract $Contract
    $account = Get-LocalServiceSid
    $admin = Test-Administrator
    $residual = Get-ResidualEvidence -Contract $Contract -RunRoot $RunRoot
    $rootGate = Test-ControlledRunRoot -RunRoot $RunRoot -OutputBase $Contract.output_base
    $platform = Get-PlatformSnapshot
    $failures = New-Object System.Collections.Generic.List[string]
    if (-not $admin.is_administrator) { [void]$failures.Add('administrator preflight failed') }
    if (-not $git.valid) { [void]$failures.Add('git baseline/working-tree preflight failed') }
    if (-not $account.matches) { [void]$failures.Add('LocalService SID preflight failed') }
    if (-not $binaryGate.valid) { [void]$failures.Add('AMD binary identity preflight failed') }
    if (-not $driverGate.valid) { [void]$failures.Add('AMD driver identity preflight failed') }
    if (-not $residual.clean) { [void]$failures.Add('existing qualification service/process residue found') }
    if (-not $rootGate.valid) { $failures.AddRange(@($rootGate.failures)) }
    if ($platform.errors.Count -gt 0) { [void]$failures.Add('platform snapshot is incomplete') }
    if (-not (Test-Path -LiteralPath (Join-Path $ToolRoot 'service-host.ps1') -PathType Leaf)) {
        [void]$failures.Add('dedicated service host is missing')
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
        git = $git
        administrator = $admin
        account = $account
        binary = $binary
        binary_gate = $binaryGate
        drivers = @($drivers)
        driver_gate = $driverGate
        residual = $residual
        output_root = $rootGate
        platform = $platform
        token_contract = Get-ExpectedTokenContract -Contract $Contract
        token_validation = 'DEFERRED_TO_SESSION_0_SERVICE_PROCESS'
    }
}

function Set-IsolatedOutputAcl {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Contract
    )

    $security = New-Object System.Security.AccessControl.DirectorySecurity
    $security.SetAccessRuleProtection($true, $false)
    $rules = @(
        New-Object System.Security.AccessControl.FileSystemAccessRule(
            'NT AUTHORITY\SYSTEM',
            [System.Security.AccessControl.FileSystemRights]::FullControl,
            [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
            [System.Security.AccessControl.InheritanceFlags]::ObjectInherit,
            [System.Security.AccessControl.PropagationFlags]::None,
            [System.Security.AccessControl.AccessControlType]::Allow
        )
        New-Object System.Security.AccessControl.FileSystemAccessRule(
            'BUILTIN\Administrators',
            [System.Security.AccessControl.FileSystemRights]::FullControl,
            [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
            [System.Security.AccessControl.InheritanceFlags]::ObjectInherit,
            [System.Security.AccessControl.PropagationFlags]::None,
            [System.Security.AccessControl.AccessControlType]::Allow
        )
        New-Object System.Security.AccessControl.FileSystemAccessRule(
            $Contract.account,
            [System.Security.AccessControl.FileSystemRights]::Modify,
            [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
            [System.Security.AccessControl.InheritanceFlags]::ObjectInherit,
            [System.Security.AccessControl.PropagationFlags]::None,
            [System.Security.AccessControl.AccessControlType]::Allow
        )
    )
    foreach ($rule in $rules) {
        $security.AddAccessRule($rule)
    }
    $directory = New-Object IO.DirectoryInfo($Path)
    $directory.SetAccessControl($security)
}

function New-IsolatedOutputRoot {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)]$Contract
    )

    $rootGate = Test-ControlledRunRoot -RunRoot $RunRoot -OutputBase $Contract.output_base
    if (-not $rootGate.valid) {
        throw ('output root gate failed: {0}' -f ($rootGate.failures -join '; '))
    }
    if (-not (Test-Path -LiteralPath $Contract.output_base -PathType Container)) {
        New-Item -ItemType Directory -Path $Contract.output_base -Force | Out-Null
    }
    New-Item -ItemType Directory -Path $RunRoot | Out-Null
    Set-IsolatedOutputAcl -Path $RunRoot -Contract $Contract
    foreach ($directory in @('raw', 'raw\timechart-output', 'summary')) {
        New-Item -ItemType Directory -Path (Join-Path $RunRoot $directory) | Out-Null
    }
}

function Acquire-OneShotGate {
    param(
        [Parameter(Mandatory = $true)][string]$OutputBase,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)]$Contract
    )

    if (-not (Test-Path -LiteralPath $OutputBase -PathType Container)) {
        New-Item -ItemType Directory -Path $OutputBase -Force | Out-Null
    }
    $gatePath = Join-Path $OutputBase $Contract.gate_file_name
    $gate = [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/gate/v1'
        task_id = $Contract.task_id
        run_id = $RunId
        max_runs = $Contract.max_runs
        retries = $Contract.retries
        state = 'CONSUMED'
        consumed_before_service_registration = $true
        consumed_at_utc = [DateTime]::UtcNow.ToString('o')
        real_execution_allowed = $true
    }
    $json = ConvertTo-JsonText -Value $gate
    $bytes = (New-Object System.Text.UTF8Encoding($false)).GetBytes($json)
    try {
        $stream = New-Object IO.FileStream(
            $gatePath,
            [IO.FileMode]::CreateNew,
            [IO.FileAccess]::Write,
            [IO.FileShare]::None
        )
        try {
            $stream.Write($bytes, 0, $bytes.Length)
        }
        finally {
            $stream.Dispose()
        }
    }
    catch {
        throw "one-shot gate unavailable or already consumed: $gatePath"
    }
    [pscustomobject]@{
        path = $gatePath
        state = 'CONSUMED'
        max_runs = 1
        retries = 0
    }
}

function Quote-WindowsArgument {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value)

    if ($Value -notmatch '[\s"]') {
        return $Value
    }
    $escaped = $Value -replace '(\\*)"', '$1$1\"'
    $escaped = $escaped -replace '(\\+)$', '$1$1'
    '"{0}"' -f $escaped
}

function Get-ServiceHostBinaryPath {
    param(
        [Parameter(Mandatory = $true)][string]$ServiceHostPath,
        [Parameter(Mandatory = $true)][string]$ManifestPath
    )

    $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    if (-not (Test-Path -LiteralPath $powershell -PathType Leaf)) {
        throw 'Windows PowerShell 5.1 is unavailable for the service host'
    }
    '{0} -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File {1} -ManifestPath {2}' -f (Quote-WindowsArgument -Value $powershell), (Quote-WindowsArgument -Value $ServiceHostPath), (Quote-WindowsArgument -Value $ManifestPath)
}

function Invoke-Sc {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    $stdout = & sc.exe @Arguments 2>&1 | Out-String
    [pscustomobject]@{
        exit_code = $LASTEXITCODE
        output = $stdout.Trim()
        arguments = @($Arguments)
    }
}

function New-QualificationService {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$ServiceHostPath
    )

    $binPath = Get-ServiceHostBinaryPath -ServiceHostPath $ServiceHostPath -ManifestPath $ManifestPath
    $created = $false
    try {
        $create = Invoke-Sc -Arguments @(
            'create',
            $Contract.service_name,
            "binPath= $binPath",
            'type= own',
            'start= demand',
            'obj= NT AUTHORITY\LocalService',
            'DisplayName= Resource Timeline AMD LocalService active sampling qualification'
        )
        if ($create.exit_code -ne 0) {
            throw "service creation failed: $($create.output)"
        }
        $created = $true
        $sidType = Invoke-Sc -Arguments @('sidtype', $Contract.service_name, 'unrestricted')
        if ($sidType.exit_code -ne 0) {
            throw "service SID configuration failed: $($sidType.output)"
        }
        $serviceSid = Get-ServiceSidEvidence -ServiceName $Contract.service_name
        $configuration = Get-ServiceConfigurationEvidence -ServiceName $Contract.service_name
        $configurationValidation = Test-ServiceConfigurationEvidence -Evidence $configuration -Contract $Contract -ExpectedBinPath $binPath -ServiceSidEvidence $serviceSid
        if (-not $configurationValidation.valid) {
            throw ('service configuration validation failed: {0}' -f ($configurationValidation.failures -join '; '))
        }
        [pscustomobject]@{
            service_name = $Contract.service_name
            service_account = $Contract.account
            service_sid_type = $Contract.service_sid_type
            bin_path = $binPath
            create = $create
            sid_type = $sidType
            service_sid = $serviceSid
            configuration = $configuration
            configuration_validation = $configurationValidation
        }
    }
    catch {
        if ($created) {
            $rollback = Stop-And-RemoveQualificationService -Contract $Contract
            if (-not $rollback.cleanup_verified) {
                throw 'service setup failed and exact service cleanup was not verified'
            }
        }
        throw
    }
}

function Get-ServiceSnapshot {
    param([Parameter(Mandatory = $true)][string]$ServiceName)

    try {
        $service = Get-CimInstance Win32_Service -Filter ("Name='{0}'" -f $ServiceName) -ErrorAction Stop
    }
    catch {
        return [pscustomobject]@{
            present = $null
            query_failed = $true
            name = $ServiceName
            error = $_.Exception.Message
        }
    }
    if ($null -eq $service) {
        return [pscustomobject]@{
            present = $false
            query_failed = $false
            name = $ServiceName
        }
    }
    [pscustomobject]@{
        present = $true
        query_failed = $false
        name = $service.Name
        state = $service.State
        start_name = $service.StartName
        process_id = [int]$service.ProcessId
        path_name = $service.PathName
    }
}

function Get-ServiceSidEvidence {
    param([Parameter(Mandatory = $true)][string]$ServiceName)

    $sidQuery = Invoke-Sc -Arguments @('showsid', $ServiceName)
    $typeQuery = Invoke-Sc -Arguments @('qsidtype', $ServiceName)
    $match = [regex]::Match(
        [string]$sidQuery.output,
        'S-1-5-80-(?:\d+-){4}\d+',
        [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
    )
    $sidType = $null
    if ([string]$typeQuery.output -match '(?i)UNRESTRICTED') {
        $sidType = 'unrestricted'
    }
    elseif ([string]$typeQuery.output -match '(?i)RESTRICTED') {
        $sidType = 'restricted'
    }
    [pscustomobject]@{
        service_name = $ServiceName
        sid = if ($match.Success) { $match.Value } else { $null }
        sid_type = $sidType
        showsid = $sidQuery
        qsidtype = $typeQuery
        valid = ($sidQuery.exit_code -eq 0 -and $match.Success -and
            $typeQuery.exit_code -eq 0 -and $sidType -ieq 'unrestricted')
    }
}

function Get-ServiceConfigurationEvidence {
    param([Parameter(Mandatory = $true)][string]$ServiceName)

    $service = Get-CimInstance Win32_Service -Filter ("Name='{0}'" -f $ServiceName) -ErrorAction SilentlyContinue
    if ($null -eq $service) {
        return [pscustomobject]@{
            present = $false
            name = $ServiceName
            start_name = $null
            start_mode = $null
            service_type = $null
            path_name = $null
            state = $null
            process_id = 0
        }
    }
    [pscustomobject]@{
        present = $true
        name = $service.Name
        start_name = $service.StartName
        start_mode = $service.StartMode
        service_type = $service.ServiceType
        path_name = $service.PathName
        state = $service.State
        process_id = [int]$service.ProcessId
    }
}

function Test-ServiceConfigurationEvidence {
    param(
        [Parameter(Mandatory = $true)]$Evidence,
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$ExpectedBinPath,
        [Parameter(Mandatory = $true)]$ServiceSidEvidence
    )

    $failures = New-Object System.Collections.Generic.List[string]
    if (-not $Evidence.present) {
        [void]$failures.Add('qualification service is absent after creation')
    }
    else {
        if ([string]$Evidence.name -cne $Contract.service_name) {
            [void]$failures.Add('service name mismatch')
        }
        if ([string]$Evidence.start_name -ine $Contract.account) {
            [void]$failures.Add('service account mismatch')
        }
        if ([string]$Evidence.start_mode -ine 'Manual') {
            [void]$failures.Add('service start mode is not demand/manual')
        }
        if ([string]$Evidence.service_type -notmatch '(?i)Own Process') {
            [void]$failures.Add('service type is not own-process')
        }
        if ([string]$Evidence.path_name -notmatch [regex]::Escape($ExpectedBinPath)) {
            [void]$failures.Add('service image path does not match the frozen host command')
        }
    }
    if (-not $ServiceSidEvidence.valid) {
        [void]$failures.Add('dedicated Service SID evidence is invalid')
    }
    if ([string]$ServiceSidEvidence.sid_type -ine $Contract.service_sid_type) {
        [void]$failures.Add('dedicated Service SID type is not unrestricted')
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
    }
}

function Wait-ForPath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][int]$TimeoutMs
    )

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            return $true
        }
        Start-Sleep -Milliseconds 250
    }
    $false
}

function Stop-And-RemoveQualificationService {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [string]$RunRoot
    )

    $before = Get-ServiceSnapshot -ServiceName $Contract.service_name
    $stop = $null
    $delete = $null
    $stopVerified = $false
    if ($before.query_failed) {
        return [pscustomobject]@{
            before = $before
            stop = $null
            delete = [pscustomobject]@{
                exit_code = $null
                output = 'stop/delete skipped because service state could not be read'
                arguments = @('stop', $Contract.service_name)
            }
            after = $before
            stop_verified = $false
            residue = Get-ResidualEvidence -Contract $Contract -RunRoot $RunRoot
            cleanup_verified = $false
        }
    }
    if ($before.present) {
        $stop = Invoke-Sc -Arguments @('stop', $Contract.service_name)
        $stopDeadline = [DateTime]::UtcNow.AddSeconds(10)
        $stopped = $null
        while ([DateTime]::UtcNow -lt $stopDeadline) {
            $stopped = Get-ServiceSnapshot -ServiceName $Contract.service_name
            if (-not $stopped.query_failed -and
                (-not $stopped.present -or $stopped.state -ieq 'Stopped')) {
                $stopVerified = $true
                break
            }
            Start-Sleep -Milliseconds 250
        }
        if ($stopVerified) {
            $delete = Invoke-Sc -Arguments @('delete', $Contract.service_name)
            $deleteDeadline = [DateTime]::UtcNow.AddSeconds(10)
            while ([DateTime]::UtcNow -lt $deleteDeadline) {
                $deleted = Get-ServiceSnapshot -ServiceName $Contract.service_name
                if (-not $deleted.query_failed -and -not $deleted.present) {
                    break
                }
                Start-Sleep -Milliseconds 250
            }
        }
        else {
            $delete = [pscustomobject]@{
                exit_code = $null
                output = 'delete skipped because service did not reach Stopped/Absent state'
                arguments = @('delete', $Contract.service_name)
            }
        }
    }
    $after = Get-ServiceSnapshot -ServiceName $Contract.service_name
    $residue = Get-ResidualEvidence -Contract $Contract -RunRoot $RunRoot
    [pscustomobject]@{
        before = $before
        stop = $stop
        delete = $delete
        after = $after
        stop_verified = $stopVerified
        residue = $residue
        cleanup_verified = (-not $after.query_failed -and -not $after.present -and
            $residue.clean)
    }
}

function Get-Manifest {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)]$Gate,
        [Parameter(Mandatory = $true)]$Preflight,
        [Parameter(Mandatory = $true)]$ServiceDefinition
    )

    $outputDirectory = Join-Path $RunRoot 'raw\timechart-output'
    [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/manifest/v1'
        task_id = $Contract.task_id
        harness_task_id = $Contract.harness_task_id
        run_id = $RunId
        run_root = $RunRoot
        output_directory = $outputDirectory
        harness_identity = Get-HarnessIdentity -Root $ToolRoot
        service_name = $Contract.service_name
        service_account = $Contract.account
        service_account_sid = $Contract.account_sid
        service_sid_type = $Contract.service_sid_type
        expected_service_sid = $ServiceDefinition.service_sid.sid
        service_sid_evidence = $ServiceDefinition.service_sid
        service_configuration = $ServiceDefinition.configuration
        service_configuration_validation = $ServiceDefinition.configuration_validation
        session_id = $Contract.session_id
        interactive = $Contract.interactive
        executable = $Contract.amd_cli_path
        expected_sha256 = $Contract.amd_cli_sha256
        expected_file_version = $Contract.amd_cli_file_version
        expected_product_version = $Contract.amd_cli_product_version
        expected_architecture = $Contract.architecture
        arguments = @(Get-FixedAmdCliArguments -OutputDirectory $outputDirectory)
        working_directory = Split-Path -Parent $Contract.amd_cli_path
        duration_seconds = $Contract.amd_cli_duration_seconds
        interval_ms = $Contract.amd_cli_interval_ms
        cli_timeout_ms = $Contract.cli_timeout_ms
        service_timeout_ms = $Contract.service_timeout_ms
        max_runs = $Contract.max_runs
        retries = $Contract.retries
        token_contract = Get-ExpectedTokenContract -Contract $Contract
        gate = $Gate
        preflight = $Preflight
        created_at_utc = [DateTime]::UtcNow.ToString('o')
    }
}

function Get-RuntimeFailureCategory {
    param(
        [AllowNull()]$ProcessResult,
        [AllowNull()]$PowerEvidence
    )

    if ($null -eq $ProcessResult) { return 'HARNESS_FAILURE' }
    if (-not $ProcessResult.process_started) { return 'LAUNCH_FAILURE' }
    if ($ProcessResult.harness_failed) { return 'HARNESS_FAILURE' }
    if ($ProcessResult.cleanup_succeeded -eq $false) { return 'HARNESS_CLEANUP_FAILED' }
    if ($ProcessResult.timeout) { return 'TIMEOUT' }
    $text = ''
    foreach ($path in @($ProcessResult.stdout_path, $ProcessResult.stderr_path)) {
        if (-not [string]::IsNullOrWhiteSpace([string]$path) -and
            (Test-Path -LiteralPath $path -PathType Leaf)) {
            $text += [Environment]::NewLine + (Get-Content -LiteralPath $path -Raw -ErrorAction SilentlyContinue)
        }
    }
    $patterns = @(
        [pscustomobject]@{ category = 'ACCESS_DENIED'; pattern = '(?i)access.?denied|accessdenied|ERROR_ACCESS_DENIED' }
        [pscustomobject]@{ category = 'LOADER_FAILURE'; pattern = '(?i)load(?:er|library)|cxl.*load|fatal.?exit' }
        [pscustomobject]@{ category = 'VERSION_MISMATCH'; pattern = '(?i)version.?mismatch|incompatible.*version|driver.*version' }
        [pscustomobject]@{ category = 'DRIVER_UNAVAILABLE'; pattern = '(?i)driver.*(unavailable|not found|missing)|no driver' }
        [pscustomobject]@{ category = 'BIOS_UNSUPPORTED'; pattern = '(?i)bios.*(unsupported|not supported)|smu.*unsupported' }
        [pscustomobject]@{ category = 'HYPERVISOR_UNSUPPORTED'; pattern = '(?i)hypervisor.*(unsupported|not supported)|virtualization.*unsupported' }
        [pscustomobject]@{ category = 'NO_COUNTER'; pattern = '(?i)no counter|counter.*unavailable|power.*unavailable|POWER_UNAVAILABLE' }
    )
    if ([int]$ProcessResult.target_exit_signed -ne 0) {
        foreach ($candidate in $patterns) {
            if ($text -match $candidate.pattern) {
                return $candidate.category
            }
        }
        return 'CLI_RUNTIME_FAILURE'
    }
    if ($null -ne $PowerEvidence -and [string]$PowerEvidence.parser_status -eq 'COUNTER_UNAVAILABLE') {
        return 'NO_COUNTER'
    }
    if ($null -ne $PowerEvidence -and [string]$PowerEvidence.parser_status -eq 'PARSE_FAILED') {
        return 'PARSE_FAILURE'
    }
    if ($null -ne $PowerEvidence -and [string]$PowerEvidence.status -ne 'PASS') {
        return 'OUTPUT_ARTIFACT_MISSING'
    }
    'NONE'
}

function Get-ResultClassification {
    param(
        [Parameter(Mandatory = $true)]$ServiceResult,
        [Parameter(Mandatory = $true)]$PowerEvidence
    )

    if ([string]$ServiceResult.result -like 'BLOCKED_*') {
        return [string]$ServiceResult.result
    }
    if ($ServiceResult.token_validation.valid -eq $false) { return 'BLOCKED_TOKEN_CONTRACT' }
    if ($ServiceResult.process_result.process_started -and
        $ServiceResult.process_result.cleanup_succeeded -eq $false) {
        return 'HARNESS_CLEANUP_FAILED'
    }
    if ($ServiceResult.process_result.timeout) { return 'TIMEOUT' }
    if ($ServiceResult.process_result.harness_failed) { return 'HARNESS_FAILED' }
    if (-not $ServiceResult.process_result.process_started) { return 'LAUNCH_FAILURE' }
    if ($ServiceResult.process_result.target_exit_signed -ne 0) {
        return Get-RuntimeFailureCategory -ProcessResult $ServiceResult.process_result -PowerEvidence $PowerEvidence
    }
    if ($PowerEvidence.status -eq 'PASS') { return 'PASS_BOUNDED_PACKAGE_POWER' }
    if ($PowerEvidence.parser_status -eq 'COUNTER_UNAVAILABLE') { return 'POWER_UNAVAILABLE' }
    if ($PowerEvidence.parser_status -eq 'PARSE_FAILED') { return 'PARSE_FAILED' }
    'OUTPUT_ARTIFACT_MISSING'
}

function Invoke-OfflineDryRun {
    param([Parameter(Mandatory = $true)]$Contract)

    $plan = Get-OfflinePlan -Contract $Contract
    $tokenGate = Test-EffectiveTokenEvidence -Evidence (Get-OfflineTokenFixture) -Contract $Contract
    $binaryGate = Test-BinaryIdentityEvidence -Identity $plan.binary -Contract $Contract
    $driverGate = Test-DriverVersionEvidence -Drivers $plan.drivers -Contract $Contract
    $rootGate = Test-ControlledRunRoot -RunRoot $plan.run_root -OutputBase $Contract.output_base
    $fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ('amd-localservice-active-sampling-dry-' + [Guid]::NewGuid().ToString('N'))
    $fixturePath = Join-Path $fixtureRoot 'timechart.csv'
    $parsed = $null
    try {
        New-Item -ItemType Directory -Path $fixtureRoot | Out-Null
        [IO.File]::WriteAllText($fixturePath, (Get-OfflineCsvFixtureText))
        $parsed = Parse-PackagePowerCsv -Path $fixturePath
    }
    finally {
        if (Test-Path -LiteralPath $fixtureRoot) {
            Remove-Item -LiteralPath $fixtureRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    [pscustomobject]@{
        result = if ($tokenGate.valid -and $binaryGate.valid -and $driverGate.valid -and
            $rootGate.valid -and $parsed.status -eq 'PASS') { 'OFFLINE_VALIDATION_PASS' } else { 'OFFLINE_VALIDATION_FAIL' }
        mode = 'DRY_RUN'
        plan = $plan
        token_gate = $tokenGate
        binary_gate = $binaryGate
        driver_gate = $driverGate
        output_root_gate = $rootGate
        csv_validation = $parsed
        power_evidence = Test-PackagePowerEvidence -Parsed $parsed
        service_lifecycle = [ordered]@{
            materialized = $false
            started = $false
            stopped = $false
            deleted = $false
        }
        gate_lifecycle = [ordered]@{
            new_authorization_required = $true
            consumed = $false
            max_runs = $Contract.max_runs
            retries = $Contract.retries
        }
        amd_cli_real_invocations = 0
        amd_api_real_invocations = 0
        power_sampling_runs = 0
        service_mutations = 0
        lsa_mutations = 0
        token_mutations = 0
        acl_mutations = 0
        driver_mutations = 0
        platform_security_mutations = 0
        note = 'Dry run does not create a Windows service, inspect an effective service token, or invoke AMD CLI.'
    }
}

function Assert-LiveAuthorization {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][bool]$AuthorizationSwitch,
        [string]$Token
    )

    if (-not $AuthorizationSwitch) {
        throw 'new explicit -AuthorizeLiveRun switch is required'
    }
    if ($Token -cne $Contract.authorization_token) {
        throw 'dedicated LocalService active-sampling authorization token is required'
    }
    $marker = [Environment]::GetEnvironmentVariable([string]$Contract.authorization_environment)
    if ($marker -cne $Contract.authorization_environment_value) {
        throw 'dedicated LocalService active-sampling authorization environment marker is required'
    }
}

function Invoke-LiveRun {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][bool]$AuthorizationSwitch,
        [string]$Token
    )

    Assert-LiveAuthorization -Contract $Contract -AuthorizationSwitch $AuthorizationSwitch -Token $Token
    $runId = New-QualificationRunId
    $runRoot = Get-QualificationRunRoot -OutputBase $Contract.output_base -RunId $runId
    $gate = $null
    $serviceCreated = $false
    $serviceStarted = $false
    $serviceDefinition = $null
    $preflight = $null
    $serviceResult = $null
    $cleanup = $null
    $preServiceCleanup = $null
    $wrapperError = $null
    try {
        $preflight = Get-LivePreflight -Contract $Contract -RunRoot $runRoot
        if (-not $preflight.valid) {
            throw ('BLOCKED_PREFLIGHT: {0}' -f ($preflight.failures -join '; '))
        }
        New-IsolatedOutputRoot -RunRoot $runRoot -Contract $Contract
        $gate = Acquire-OneShotGate -OutputBase $Contract.output_base -RunId $runId -Contract $Contract
        $manifestPath = Join-Path $runRoot 'manifest.json'
        $serviceHostPath = Join-Path $ToolRoot 'service-host.ps1'
        $serviceDefinition = New-QualificationService -Contract $Contract -ManifestPath $manifestPath -ServiceHostPath $serviceHostPath
        $serviceCreated = $true
        $manifest = Get-Manifest -Contract $Contract -RunId $runId -RunRoot $runRoot -Gate $gate -Preflight $preflight -ServiceDefinition $serviceDefinition
        Write-JsonAtomic -Path $manifestPath -Value $manifest
        Start-Service -Name $Contract.service_name
        $serviceStarted = $true
        $resultPath = Join-Path $runRoot 'raw\service-result.json'
        if (-not (Wait-ForPath -Path $resultPath -TimeoutMs $Contract.service_timeout_ms)) {
            throw 'service result timeout; no retry is permitted'
        }
        $serviceResult = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
    }
    catch {
        $wrapperError = $_.Exception.Message
    }
    finally {
        if ($serviceCreated) {
            $cleanup = Stop-And-RemoveQualificationService -Contract $Contract -RunRoot $runRoot
        }
        elseif ($null -eq $gate -and (Test-Path -LiteralPath $runRoot -PathType Container)) {
            Remove-Item -LiteralPath $runRoot -Recurse -Force
            $preServiceCleanup = [pscustomobject]@{
                attempted = $true
                path = $runRoot
                verified_absent = (-not (Test-Path -LiteralPath $runRoot))
            }
        }
    }
    if ($null -eq $preflight) {
        $preflight = [pscustomobject]@{ valid = $false; failures = @($wrapperError) }
    }
    $parsedCsv = [pscustomobject]@{ status = 'NOT_FOUND'; error = 'service did not produce a parsed CSV'; sample_count = 0; samples = @() }
    if (Test-Path -LiteralPath $runRoot -PathType Container) {
        $csv = Get-ChildItem -LiteralPath $runRoot -File -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -ieq 'timechart.csv' } | Select-Object -First 1
        if ($null -ne $csv) {
            $parsedCsv = Parse-PackagePowerCsv -Path $csv.FullName
        }
    }
    $powerEvidence = Test-PackagePowerEvidence -Parsed $parsedCsv
    $classification = 'BLOCKED'
    if ($null -ne $cleanup -and -not $cleanup.cleanup_verified) {
        $classification = 'HARNESS_CLEANUP_FAILED'
    }
    elseif ($null -ne $serviceResult) {
        $classification = Get-ResultClassification -ServiceResult $serviceResult -PowerEvidence $powerEvidence
    }
    $residue = Get-ResidualEvidence -Contract $Contract -RunRoot $runRoot
    $finalInventory = @()
    if (Test-Path -LiteralPath $runRoot -PathType Container) {
        $finalInventory = @(Get-OutputInventory -Root $runRoot)
    }
    $runtimeProcessResult = $null
    if ($null -ne $serviceResult) {
        $runtimeProcessResult = $serviceResult.process_result
    }
    $summary = [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/summary/v1'
        result = $classification
        task_id = $Contract.task_id
        harness_task_id = $Contract.harness_task_id
        run_id = $runId
        run_root = $runRoot
        preflight = $preflight
        service_definition = $serviceDefinition
        service_started = $serviceStarted
        service_result = $serviceResult
        runtime_failure_category = Get-RuntimeFailureCategory -ProcessResult $runtimeProcessResult -PowerEvidence $powerEvidence
        csv_validation = $parsedCsv
        power_evidence = $powerEvidence
        pre_service_cleanup = $preServiceCleanup
        output_inventory = $finalInventory
        cleanup = $cleanup
        residue = $residue
        wrapper_error = $wrapperError
        gate = $gate
        exact_command = Get-CanonicalCommandRecord -Contract $Contract -OutputDirectory (Join-Path $runRoot 'raw\timechart-output')
        max_runs = $Contract.max_runs
        retries = $Contract.retries
        amd_cli_real_invocations = if ($null -ne $serviceResult -and $serviceResult.process_result.process_started) { 1 } else { 0 }
        amd_api_real_invocations = 0
        power_sampling_runs = if ($null -ne $serviceResult -and $serviceResult.process_result.process_started) { 1 } else { 0 }
        service_mutations = if ($serviceCreated) { 1 } else { 0 }
        lsa_mutations = 0
        token_mutations = 0
        acl_mutations = if ($serviceCreated) { 1 } else { 0 }
        driver_mutations = 0
        platform_security_mutations = 0
        production_admission = 'DEFER'
        next_gate = 'HUMAN_REVIEW_LOCALSERVICE_ACTIVE_SAMPLING_RESULT'
    }
    if (Test-Path -LiteralPath $runRoot -PathType Container) {
        $summaryPath = Join-Path $runRoot 'summary\qualification-summary.json'
        Write-JsonAtomic -Path $summaryPath -Value $summary
    }
    [pscustomobject]$summary
}

$contract = Get-AmdLocalServiceSamplingContract
if ($Mode -eq 'DryRun') {
    $result = Invoke-OfflineDryRun -Contract $contract
    $text = ConvertTo-JsonText -Value $result
    if ([string]::IsNullOrWhiteSpace($ReportPath)) {
        Write-Output $text
    }
    else {
        Write-JsonAtomic -Path $ReportPath -Value $result -AllowReplace
        Write-Output $ReportPath
    }
    exit 0
}

$liveResult = Invoke-LiveRun -Contract $contract -AuthorizationSwitch $AuthorizeLiveRun -Token $AuthorizationToken
if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    Write-Output (ConvertTo-JsonText -Value $liveResult)
}
else {
    Write-JsonAtomic -Path $ReportPath -Value $liveResult -AllowReplace
    Write-Output $ReportPath
}
if ([string]$liveResult.result -eq 'PASS_BOUNDED_PACKAGE_POWER') {
    exit 0
}
exit 1
