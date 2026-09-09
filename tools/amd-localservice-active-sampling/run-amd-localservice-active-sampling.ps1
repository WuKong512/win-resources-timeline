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
. (Join-Path $ToolRoot '..\amd-privilege-qualification\i2e-runtime-library.ps1')

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
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)]$Contract
    )

    Test-HarnessSourceIdentity -Root $Root -Contract $Contract
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
    $harness = Test-HarnessSourceIdentity -Root $ToolRoot -Contract $Contract
    $account = Get-LocalServiceSid
    $admin = Test-Administrator
    $residual = Get-ResidualEvidence -Contract $Contract -RunRoot $RunRoot
    $rootGate = Test-ControlledRunRoot -RunRoot $RunRoot -OutputBase $Contract.output_base
    $platform = Get-PlatformSnapshot
    $failures = New-Object System.Collections.Generic.List[string]
    if (-not $admin.is_administrator) { [void]$failures.Add('administrator preflight failed') }
    if (-not $git.valid) { [void]$failures.Add('git baseline/working-tree preflight failed') }
    if (-not $harness.valid) { [void]$failures.Add('reviewed harness source identity preflight failed') }
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
        harness_identity = $harness
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
        [Parameter(Mandatory = $true)]$Contract,
        [AllowNull()][string]$ServiceSid,
        [switch]$Seal
    )

    if ($Seal -and -not [string]::IsNullOrWhiteSpace($ServiceSid)) {
        throw 'sealed output ACL cannot grant the Q1 Service SID write access'
    }
    if (-not [string]::IsNullOrWhiteSpace($ServiceSid) -and $ServiceSid -notmatch '^S-1-5-80-') {
        throw "refusing output ACL mutation for a non-Service SID: $ServiceSid"
    }
    $phase = if ($Seal) { 'SEALED' } elseif ([string]::IsNullOrWhiteSpace($ServiceSid)) { 'STAGING' } else { 'AUTHORIZED' }
    $inherit = [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
        [System.Security.AccessControl.InheritanceFlags]::ObjectInherit
    $allow = [System.Security.AccessControl.AccessControlType]::Allow
    $targets = @(
        Get-Item -LiteralPath $Path -Force
        Get-ChildItem -LiteralPath $Path -Force -Recurse -ErrorAction Stop
    )
    foreach ($target in $targets) {
        $isDirectory = $target -is [System.IO.DirectoryInfo]
        $security = if ($isDirectory) {
            New-Object System.Security.AccessControl.DirectorySecurity
        }
        else {
            New-Object System.Security.AccessControl.FileSecurity
        }
        $security.SetAccessRuleProtection($true, $false)
        foreach ($entry in @(
            @('S-1-5-18', [System.Security.AccessControl.FileSystemRights]::FullControl),
            @('S-1-5-32-544', [System.Security.AccessControl.FileSystemRights]::FullControl)
        )) {
            $sid = [System.Security.Principal.SecurityIdentifier]::new([string]$entry[0])
            $inheritance = if ($isDirectory) { $inherit } else { [System.Security.AccessControl.InheritanceFlags]::None }
            $security.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new(
                $sid, $entry[1], $inheritance, [System.Security.AccessControl.PropagationFlags]::None, $allow))
        }
        if ($phase -eq 'AUTHORIZED') {
            $serviceSidObject = [System.Security.Principal.SecurityIdentifier]::new($ServiceSid)
            $inheritance = if ($isDirectory) { $inherit } else { [System.Security.AccessControl.InheritanceFlags]::None }
            $security.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new(
                $serviceSidObject,
                [System.Security.AccessControl.FileSystemRights]::Modify,
                $inheritance,
                [System.Security.AccessControl.PropagationFlags]::None,
                $allow))
        }
        Set-Acl -LiteralPath $target.FullName -AclObject $security
    }
    $inspection = Get-Q1OutputTreeAclInspection -Path $Path -ServiceSid $ServiceSid
    $expected = Get-Q1OutputAclModel -Phase $phase -ServiceSid $ServiceSid
    $modelGate = Test-Q1OutputAclModel -Model $expected -ExpectedPhase $phase -ExpectedServiceSid $ServiceSid
    if (-not $modelGate.valid -or
        ($phase -eq 'AUTHORIZED' -and -not $inspection.service_sid_has_write) -or
        ($phase -ne 'AUTHORIZED' -and $inspection.service_sid_has_write) -or
        $inspection.localservice_account_wide_write) {
        throw "output ACL verification failed for phase $phase"
    }
    $inspection
}

function Get-Q1OutputAclInspection {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [AllowNull()][string]$ServiceSid
    )

    $acl = Get-Acl -LiteralPath $Path -ErrorAction Stop
    $records = New-Object System.Collections.Generic.List[object]
    foreach ($rule in @($acl.Access)) {
        $identitySid = [string]$rule.IdentityReference.Value
        try {
            if ($rule.IdentityReference -is [System.Security.Principal.SecurityIdentifier]) {
                $identitySid = $rule.IdentityReference.Value
            }
            else {
                $identitySid = $rule.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value
            }
        }
        catch {}
        $rights = [string]$rule.FileSystemRights
        $write = $rights -match '(?i)Modify|FullControl|Write|Delete|ChangePermissions|TakeOwnership'
        [void]$records.Add([pscustomobject]@{
            identity = [string]$rule.IdentityReference.Value
            identity_sid = $identitySid
            access_type = [string]$rule.AccessControlType
            rights = $rights
            write_capable = [bool]$write
        })
    }
    $serviceRules = @($records | Where-Object {
        -not [string]::IsNullOrWhiteSpace($ServiceSid) -and
        [string]$_.identity_sid -ieq $ServiceSid -and
        $_.access_type -ieq 'Allow' -and
        $_.write_capable
    })
    $localServiceRules = @($records | Where-Object {
        [string]$_.identity_sid -ieq 'S-1-5-19' -and
        $_.access_type -ieq 'Allow' -and
        $_.write_capable
    })
    [pscustomobject]@{
        path = $Path
        phase = if ($serviceRules.Count -gt 0) { 'AUTHORIZED' } else { 'STAGING_OR_SEALED' }
        rules = $records.ToArray()
        service_sid = $ServiceSid
        service_sid_has_write = ($serviceRules.Count -gt 0)
        localservice_account_wide_write = ($localServiceRules.Count -gt 0)
        service_sid_write_rules = @($serviceRules)
        localservice_write_rules = @($localServiceRules)
    }
}

function Get-Q1OutputTreeAclInspection {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [AllowNull()][string]$ServiceSid
    )

    $targets = @(
        Get-Item -LiteralPath $Path -Force
        Get-ChildItem -LiteralPath $Path -Force -Recurse -ErrorAction Stop
    )
    $inspections = @($targets | ForEach-Object {
        Get-Q1OutputAclInspection -Path $_.FullName -ServiceSid $ServiceSid
    })
    [pscustomobject]@{
        path = $Path
        service_sid = $ServiceSid
        service_sid_has_write = [bool]($inspections | Where-Object { $_.service_sid_has_write })
        localservice_account_wide_write = [bool]($inspections | Where-Object { $_.localservice_account_wide_write })
        inspections = $inspections
    }
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
    foreach ($directory in @('raw', 'raw\timechart-output', 'summary')) {
        New-Item -ItemType Directory -Path (Join-Path $RunRoot $directory) | Out-Null
    }
    Write-JsonAtomic -Path (Join-Path $RunRoot 'raw\output-acl-mutation-intent.json') -Value ([ordered]@{
        schema = 'amd-localservice-active-sampling-q1/output-acl-mutation-intent/v1'
        state = 'STAGING_ACL_ATTEMPTED'
        run_root = $RunRoot
        phase = 'STAGING'
        service_identity = 'SYSTEM_AND_ADMINISTRATORS_ONLY'
        scope = 'EXACT_Q1_RUN_ROOT_ONLY'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })
    $inspection = Set-IsolatedOutputAcl -Path $RunRoot -Contract $Contract
    Write-JsonAtomic -Path (Join-Path $RunRoot 'raw\output-acl-mutation-complete.json') -Value ([ordered]@{
        schema = 'amd-localservice-active-sampling-q1/output-acl-mutation-complete/v1'
        state = 'STAGING_ACL_COMPLETE'
        run_root = $RunRoot
        phase = 'STAGING'
        localservice_account_wide_write = $inspection.localservice_account_wide_write
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })
    [pscustomobject]@{
        run_root = $RunRoot
        phase = 'STAGING'
        acl_mutation_attempted = $true
        acl_mutation_verified = $true
        service_sid = $null
        service_sid_write_access = $false
        localservice_account_wide_write = $inspection.localservice_account_wide_write
    }
}

function Grant-Q1ServiceSidOutputAccess {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$ServiceSid
    )

    if ($ServiceSid -notmatch '^S-1-5-80-') {
        throw "refusing output ACL grant for a non-Service SID: $ServiceSid"
    }
    $rawRoot = Join-Path $RunRoot 'raw'
    Write-JsonAtomic -Path (Join-Path $rawRoot 'output-acl-service-sid-intent.json') -Value ([ordered]@{
        schema = 'amd-localservice-active-sampling-q1/output-acl-service-sid-intent/v1'
        state = 'AUTHORIZED_SID_ACL_ATTEMPTED'
        service_sid = $ServiceSid
        right = 'Modify'
        target = 'EXACT_Q1_RUN_ROOT_ONLY'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })
    $inspection = Set-IsolatedOutputAcl -Path $RunRoot -Contract $Contract -ServiceSid $ServiceSid
    if (-not $inspection.service_sid_has_write -or $inspection.localservice_account_wide_write) {
        throw 'exact Q1 Service SID output write access was not verified'
    }
    Write-JsonAtomic -Path (Join-Path $rawRoot 'output-acl-service-sid-complete.json') -Value ([ordered]@{
        schema = 'amd-localservice-active-sampling-q1/output-acl-service-sid-complete/v1'
        state = 'AUTHORIZED_SID_ACL_COMPLETE'
        service_sid = $ServiceSid
        right = 'Modify'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })
    [pscustomobject]@{
        run_root = $RunRoot
        phase = 'AUTHORIZED'
        service_sid = $ServiceSid
        acl_mutation_attempted = $true
        acl_mutation_verified = $true
        service_sid_write_access = $true
        localservice_account_wide_write = $false
        inspection = $inspection
    }
}

function Acquire-OneShotGate {
    param(
        [Parameter(Mandatory = $true)][string]$OutputBase,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)]$Contract
    )

    if ([int]$Contract.max_runs -ne 1 -or [int]$Contract.retries -ne 0) {
        throw 'one-shot gate contract is not MAX_RUNS=1 / RETRIES=0'
    }
    $gatePath = Join-Path $OutputBase $Contract.gate_file_name
    $gateRecord = New-OneShotGateRecord -TaskId $Contract.task_id -RunId $RunId -MaxRuns $Contract.max_runs -Retries $Contract.retries -RealExecutionAllowed $true
    Acquire-OneShotGateFile -GatePath $gatePath -GateRecord $gateRecord
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
        [Parameter(Mandatory = $true)][string]$ServiceHostPath,
        [string]$RunRoot
    )

    $binPath = Get-ServiceHostBinaryPath -ServiceHostPath $ServiceHostPath -ManifestPath $ManifestPath
    $created = $false
    try {
        if (-not [string]::IsNullOrWhiteSpace($RunRoot)) {
            Write-JsonAtomic -Path (Join-Path $RunRoot 'raw\service-mutation-started.json') -Value ([ordered]@{
                schema = 'amd-localservice-active-sampling-q1/service-mutation-started/v1'
                state = 'CREATE_ATTEMPTED'
                service_name = $Contract.service_name
                service_account = $Contract.account
                service_display_name = $Contract.service_display_name
                recorded_at_utc = [DateTime]::UtcNow.ToString('o')
            })
        }
        $createArguments = New-QualificationServiceCreateArguments -ServiceName $Contract.service_name -BinPath $binPath -ServiceAccount $Contract.account -DisplayName $Contract.service_display_name
        $create = Invoke-Sc -Arguments $createArguments
        if ($create.exit_code -ne 0) {
            throw "service creation failed: $($create.output)"
        }
        $created = $true
        if (-not [string]::IsNullOrWhiteSpace($RunRoot)) {
            Write-JsonAtomic -Path (Join-Path $RunRoot 'raw\service-mutation-complete.json') -Value ([ordered]@{
                schema = 'amd-localservice-active-sampling-q1/service-mutation-complete/v1'
                state = 'CREATE_COMPLETE'
                service_name = $Contract.service_name
                recorded_at_utc = [DateTime]::UtcNow.ToString('o')
            })
        }
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
            create_arguments = @($createArguments)
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

function Get-Q1LsaSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)]$Contract
    )

    [pscustomobject]@{
        schema = 'amd-localservice-active-sampling-q1/lsa-snapshot/v1'
        label = $Label
        service_sid = $ServiceSid
        right = $Contract.allowed_lsa_right
        direct = Get-I2eDirectAccountRightsSnapshot -Label $Label -Sid $ServiceSid
        assignment = Get-I2eUserRightAssignmentSnapshot -Right $Contract.allowed_lsa_right
        captured_at_utc = [DateTime]::UtcNow.ToString('o')
    }
}

function Initialize-Q1LsaMaterialization {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$ServiceSid
    )

    if ([string]$Contract.allowed_lsa_right -cne 'SeSystemProfilePrivilege' -or
        [string]$Contract.allowed_lsa_target -cne 'EXACT_Q1_SERVICE_SID_ONLY') {
        throw 'Q1 LSA contract is not the exact SeSystemProfilePrivilege / dedicated Service SID contract'
    }
    $rawRoot = Join-Path $RunRoot 'raw'
    $before = Get-Q1LsaSnapshot -Label 'BEFORE_Q1_MATERIALIZATION' -ServiceSid $ServiceSid -Contract $Contract
    Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-before.json') -Value $before
    $beforeGate = Test-Q1LsaBeforeMaterialization -Snapshot $before -ServiceSid $ServiceSid -Right $Contract.allowed_lsa_right
    if (-not $beforeGate.valid) {
        Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-materialization-blocked.json') -Value $beforeGate
        throw ('BLOCKED_LSA_CONTROL_BASELINE_PRECONDITION: {0}' -f ($beforeGate.failures -join '; '))
    }
    $intent = [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/lsa-mutation-intent/v2'
        task_id = $Contract.task_id
        service_name = $Contract.service_name
        state = 'PLANNED'
        service_sid = $ServiceSid
        right = $Contract.allowed_lsa_right
        target = $Contract.allowed_lsa_target
        mutation_intent = 'ADD_EXACT_RIGHT'
        ownership_before = 'ABSENT'
        right_absent_before = [bool]$beforeGate.right_absent_before
        cleanup_if_ambiguous = 'REQUIRED'
        permitted_mutation = 'NEW_Q1_SERVICE_SID_PLUS_SeSystemProfilePrivilege_ONLY'
        forbidden_targets = @('S-1-5-19', 'LocalSystem', 'Administrators', 'device ACLs', 'drivers', 'platform security')
        timestamp = [DateTime]::UtcNow.ToString('o')
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-mutation-intent.json') -Value $intent
    $started = [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/lsa-mutation-started/v2'
        state = 'MUTATION_ATTEMPTED'
        task_id = $Contract.task_id
        service_name = $Contract.service_name
        service_sid = $ServiceSid
        right = $Contract.allowed_lsa_right
        target = $Contract.allowed_lsa_target
        mutation_attempted = $true
        precondition_proved = 'RIGHT_ABSENT_BEFORE'
        started_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-mutation-started.json') -Value $started
    try {
        Add-I2eExactServiceProfileRight -ServiceSid $ServiceSid
    }
    catch {
        try {
            Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-mutation-failure.json') -Value ([ordered]@{
                schema = 'amd-localservice-active-sampling-q1/lsa-mutation-failure/v1'
                state = 'MUTATION_FAILED_OR_AMBIGUOUS'
                service_sid = $ServiceSid
                right = $Contract.allowed_lsa_right
                error = $_.Exception.Message
                recorded_at_utc = [DateTime]::UtcNow.ToString('o')
            })
        }
        catch {}
        throw
    }
    $after = Get-Q1LsaSnapshot -Label 'AFTER_Q1_MATERIALIZATION' -ServiceSid $ServiceSid -Contract $Contract
    Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-after.json') -Value $after
    $decision = Get-Q1LsaMaterializationDecision -Before $before -After $after -ServiceSid $ServiceSid -Right $Contract.allowed_lsa_right -MutationAttempted $true
    Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-ownership.json') -Value ([ordered]@{
        schema = 'amd-localservice-active-sampling-q1/lsa-ownership/v1'
        added_by_run = $decision.added_by_run
        cleanup_allowed = $decision.cleanup_allowed
        service_sid = $ServiceSid
        right = $Contract.allowed_lsa_right
        decision = $decision
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
    })
    if (-not $decision.valid) {
        throw ('LSA right readback failed closed: {0}' -f ($decision.failures -join '; '))
    }
    [pscustomobject]@{
        service_sid = $ServiceSid
        right = $Contract.allowed_lsa_right
        before = $before
        after = $after
        decision = $decision
        mutation_attempted = $true
        added_by_run = [bool]$decision.added_by_run
    }
}

function Read-Q1EvidenceJson {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $null
    }
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Recover-Q1LsaMutationIfNecessary {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    $rawRoot = Join-Path $RunRoot 'raw'
    $startedPath = Join-Path $rawRoot 'lsa-mutation-started.json'
    if (-not (Test-Path -LiteralPath $startedPath -PathType Leaf)) {
        return [pscustomobject]@{
            attempted = $false
            cleanup_verified = $true
            added_by_run = $false
            recovery_state = 'NO_MUTATION_ATTEMPTED'
            residual_state = 'NONE'
            reason = 'Q1 run did not durably record an LSA mutation attempt'
        }
    }
    $intentPath = Join-Path $rawRoot 'lsa-mutation-intent.json'
    $beforePath = Join-Path $rawRoot 'lsa-before.json'
    $intent = $null
    $before = $null
    $started = $null
    try {
        $intent = Read-Q1EvidenceJson -Path $intentPath
        $before = Read-Q1EvidenceJson -Path $beforePath
        $started = Read-Q1EvidenceJson -Path $startedPath
    }
    catch {
        try {
            Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-recovery.json') -Value ([ordered]@{
                schema = 'amd-localservice-active-sampling-q1/lsa-recovery/v2'
                recovery_state = 'FAILED_CLOSED'
                residual_state = 'UNKNOWN'
                cleanup_verified = $false
                error = $_.Exception.Message
                recorded_at_utc = [DateTime]::UtcNow.ToString('o')
            })
        }
        catch {}
        [pscustomobject]@{
            attempted = $true
            cleanup_verified = $false
            added_by_run = $true
            recovery_state = 'FAILED_CLOSED'
            residual_state = 'UNKNOWN'
            error = $_.Exception.Message
        }
        return
    }
    $serviceSid = [string](Get-ContractPropertyValue -Object $intent -Name 'service_sid')
    $plan = $null
    try {
        $plan = Get-Q1LsaRecoveryPlan -Intent $intent -MutationStarted $started -Before $before -Current $null -ExpectedServiceName $Contract.service_name -ExpectedServiceSid $serviceSid -ExpectedRight $Contract.allowed_lsa_right
        $current = Get-Q1LsaSnapshot -Label 'CURRENT_Q1_RECOVERY_READBACK' -ServiceSid $serviceSid -Contract $Contract
        $plan = Get-Q1LsaRecoveryPlan -Intent $intent -MutationStarted $started -Before $before -Current $current -ExpectedServiceName $Contract.service_name -ExpectedServiceSid $serviceSid -ExpectedRight $Contract.allowed_lsa_right
    }
    catch {
        $plan = [pscustomobject]@{
            valid = $false
            action = 'FAILED_CLOSED'
            recovery_state = 'FAILED_CLOSED'
            residual_state = 'UNKNOWN'
            failures = @($_.Exception.Message)
        }
    }
    if (-not $plan.valid) {
        try {
            Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-recovery.json') -Value ([ordered]@{
                schema = 'amd-localservice-active-sampling-q1/lsa-recovery/v2'
                recovery_state = 'FAILED_CLOSED'
                residual_state = if ($null -eq $plan.residual_state) { 'UNKNOWN' } else { $plan.residual_state }
                cleanup_verified = $false
                service_sid = $serviceSid
                right = $Contract.allowed_lsa_right
                plan = $plan
                recorded_at_utc = [DateTime]::UtcNow.ToString('o')
            })
        }
        catch {}
        return [pscustomobject]@{
            attempted = $true
            cleanup_verified = $false
            added_by_run = $true
            recovery_state = 'FAILED_CLOSED'
            residual_state = if ($null -eq $plan.residual_state) { 'UNKNOWN' } else { $plan.residual_state }
            plan = $plan
        }
    }
    if ($plan.action -eq 'NONE') {
        $cleanupGate = Test-Q1LsaCleanupEvidence -Snapshot $current -ServiceSid $serviceSid -Right $Contract.allowed_lsa_right
        try {
            Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-cleanup-after.json') -Value $current
            Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-recovery.json') -Value ([ordered]@{
                schema = 'amd-localservice-active-sampling-q1/lsa-recovery/v2'
                recovery_state = 'RIGHT_ALREADY_ABSENT'
                residual_state = 'ABSENT'
                cleanup_verified = [bool]$cleanupGate.valid
                service_sid = $serviceSid
                right = $Contract.allowed_lsa_right
                plan = $plan
                validation = $cleanupGate
                recorded_at_utc = [DateTime]::UtcNow.ToString('o')
            })
        }
        catch {
            return [pscustomobject]@{
                attempted = $true
                cleanup_verified = $false
                added_by_run = $true
                recovery_state = 'FAILED_CLOSED'
                residual_state = 'ABSENT'
                error = $_.Exception.Message
            }
        }
        return [pscustomobject]@{
            attempted = $true
            cleanup_verified = [bool]$cleanupGate.valid
            added_by_run = $true
            recovery_state = 'RIGHT_ALREADY_ABSENT'
            residual_state = 'ABSENT'
            final = $current
            validation = $cleanupGate
        }
    }
    try {
        Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-cleanup-intent.json') -Value ([ordered]@{
            schema = 'amd-localservice-active-sampling-q1/lsa-cleanup-intent/v2'
            state = 'PLANNED'
            service_sid = $serviceSid
            right = $Contract.allowed_lsa_right
            exact_owner_required = $true
            recovery_from_ambiguous_mutation = $true
            recorded_at_utc = [DateTime]::UtcNow.ToString('o')
        })
        Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-cleanup-started.json') -Value ([ordered]@{
            schema = 'amd-localservice-active-sampling-q1/lsa-cleanup-started/v2'
            state = 'CLEANUP_ATTEMPTED'
            service_sid = $serviceSid
            right = $Contract.allowed_lsa_right
            started_at_utc = [DateTime]::UtcNow.ToString('o')
        })
        Remove-I2eExactServiceProfileRight -ServiceSid $serviceSid
        $final = Get-Q1LsaSnapshot -Label 'AFTER_Q1_CLEANUP' -ServiceSid $serviceSid -Contract $Contract
        $gate = Test-Q1LsaCleanupEvidence -Snapshot $final -ServiceSid $serviceSid -Right $Contract.allowed_lsa_right
        if (-not $gate.valid) {
            throw ('LSA final readback failed closed: {0}' -f ($gate.failures -join '; '))
        }
        Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-cleanup-after.json') -Value $final
        Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-recovery.json') -Value ([ordered]@{
            schema = 'amd-localservice-active-sampling-q1/lsa-recovery/v2'
            recovery_state = 'OWNED_RIGHT_REMOVED'
            residual_state = 'ABSENT'
            cleanup_verified = $true
            service_sid = $serviceSid
            right = $Contract.allowed_lsa_right
            plan = $plan
            validation = $gate
            recorded_at_utc = [DateTime]::UtcNow.ToString('o')
        })
        [pscustomobject]@{
            attempted = $true
            cleanup_verified = $true
            added_by_run = $true
            recovery_state = 'OWNED_RIGHT_REMOVED'
            residual_state = 'ABSENT'
            final = $final
            validation = $gate
        }
    }
    catch {
        try {
            Write-JsonAtomic -Path (Join-Path $rawRoot 'lsa-recovery.json') -Value ([ordered]@{
                schema = 'amd-localservice-active-sampling-q1/lsa-recovery/v2'
                recovery_state = 'FAILED_CLOSED'
                residual_state = 'UNKNOWN'
                cleanup_verified = $false
                service_sid = $serviceSid
                right = $Contract.allowed_lsa_right
                error = $_.Exception.Message
                recorded_at_utc = [DateTime]::UtcNow.ToString('o')
            })
        }
        catch {}
        [pscustomobject]@{
            attempted = $true
            cleanup_verified = $false
            added_by_run = $true
            recovery_state = 'FAILED_CLOSED'
            residual_state = 'UNKNOWN'
            error = $_.Exception.Message
        }
    }
}

function Remove-Q1LsaRightIfOwned {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [AllowNull()]$Materialization
    )

    Recover-Q1LsaMutationIfNecessary -Contract $Contract -RunRoot $RunRoot
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
            display_name = $null
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
        display_name = $service.DisplayName
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

    Test-Q1ServiceConfigurationEvidence -Evidence $Evidence -Contract $Contract -ExpectedBinPath $ExpectedBinPath -ServiceSidEvidence $ServiceSidEvidence
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
        [string]$RunRoot,
        [switch]$KeepRegistration
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
            service_deleted = $false
            registration_kept = [bool]$KeepRegistration
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
        if ($stopVerified -and -not $KeepRegistration) {
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
        elseif ($stopVerified -and $KeepRegistration) {
            $delete = [pscustomobject]@{
                exit_code = $null
                output = 'delete deferred until exact LSA recovery and evidence sealing complete'
                arguments = @('delete', $Contract.service_name)
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
        service_deleted = (-not $after.query_failed -and -not $after.present)
        registration_kept = [bool]$KeepRegistration
        residue = $residue
        cleanup_verified = if ($KeepRegistration) {
            $stopVerified
        }
        else {
            (-not $after.query_failed -and -not $after.present -and $residue.clean)
        }
    }
}

function Remove-QualificationServiceRegistration {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [string]$RunRoot
    )

    $before = Get-ServiceSnapshot -ServiceName $Contract.service_name
    if ($before.query_failed) {
        return [pscustomobject]@{
            attempted = $false
            removed = $false
            cleanup_verified = $false
            before = $before
            after = $before
            error = 'service state could not be read before deferred deletion'
        }
    }
    if (-not $before.present) {
        return [pscustomobject]@{
            attempted = $false
            removed = $false
            cleanup_verified = $true
            before = $before
            after = $before
        }
    }
    if ([string]$before.state -ine 'Stopped' -or [int]$before.process_id -ne 0) {
        return [pscustomobject]@{
            attempted = $false
            removed = $false
            cleanup_verified = $false
            before = $before
            after = $before
            error = 'refusing deferred deletion while qualification service is not stopped'
        }
    }
    $delete = Invoke-Sc -Arguments @('delete', $Contract.service_name)
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    $after = $null
    while ([DateTime]::UtcNow -lt $deadline) {
        $after = Get-ServiceSnapshot -ServiceName $Contract.service_name
        if (-not $after.query_failed -and -not $after.present) { break }
        Start-Sleep -Milliseconds 250
    }
    if ($null -eq $after) { $after = Get-ServiceSnapshot -ServiceName $Contract.service_name }
    $residue = Get-ResidualEvidence -Contract $Contract -RunRoot $RunRoot
    [pscustomobject]@{
        attempted = $true
        removed = (-not $after.query_failed -and -not $after.present)
        cleanup_verified = (-not $after.query_failed -and -not $after.present -and $residue.clean)
        before = $before
        delete = $delete
        after = $after
        residue = $residue
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
        harness_identity = Get-HarnessIdentity -Root $ToolRoot -Contract $Contract
        service_name = $Contract.service_name
        service_display_name = $Contract.service_display_name
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
        lsa_contract = [ordered]@{
            supported = [bool]$Contract.live_control_baseline_lsa_mutation_supported
            right = $Contract.allowed_lsa_right
            target = $Contract.allowed_lsa_target
            ownership_cleanup_required = $true
        }
        gate = $Gate
        preflight = $Preflight
        created_at_utc = [DateTime]::UtcNow.ToString('o')
    }
}

function Write-Q1EvidenceManifest {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$ServiceSid
    )

    $entries = @(Get-Q1EvidenceManifestEntries -Root $RunRoot)
    if ($entries.Count -eq 0) {
        throw 'cannot seal a qualification run with no evidence files'
    }
    $manifest = [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/evidence-manifest/v2'
        run_root = $RunRoot
        service_sid = $ServiceSid
        seal_status = 'SEALED_EXPECTED'
        raw_evidence_immutable = $true
        entries = $entries
        created_at_utc = [DateTime]::UtcNow.ToString('o')
    }
    $path = Join-Path $RunRoot 'raw\evidence-manifest.json'
    Write-JsonAtomic -Path $path -Value $manifest
    [pscustomobject]@{
        path = $path
        manifest = [pscustomobject]$manifest
        entries = $entries
    }
}

function Seal-Q1Evidence {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$ServiceSid
    )

    try {
        $manifestEvidence = Write-Q1EvidenceManifest -RunRoot $RunRoot -ServiceSid $ServiceSid
        $sealedAcl = Set-IsolatedOutputAcl -Path $RunRoot -Contract $Contract -Seal
        $manifestGate = Test-Q1EvidenceManifest -Root $RunRoot -Manifest $manifestEvidence.manifest
        if (-not $manifestGate.valid) {
            throw ('post-seal evidence hash verification failed: {0}' -f ($manifestGate.failures -join '; '))
        }
        if ($sealedAcl.service_sid_has_write -or $sealedAcl.localservice_account_wide_write) {
            throw 'post-seal ACL still grants qualification write access'
        }
        [pscustomobject]@{
            valid = $true
            manifest_path = $manifestEvidence.path
            manifest = $manifestEvidence.manifest
            manifest_validation = $manifestGate
            acl = $sealedAcl
            post_seal_service_sid_write_access = $false
            hashes_match = $true
            evidence_preserved = $true
        }
    }
    catch {
        [pscustomobject]@{
            valid = $false
            manifest_path = Join-Path $RunRoot 'raw\evidence-manifest.json'
            manifest = $null
            manifest_validation = $null
            acl = $null
            post_seal_service_sid_write_access = $null
            hashes_match = $false
            evidence_preserved = (Test-Path -LiteralPath $RunRoot -PathType Container)
            error = $_.Exception.Message
        }
    }
}

function Get-RuntimeFailureCategory {
    param(
        [AllowNull()]$ProcessResult,
        [AllowNull()]$PowerEvidence,
        [string]$RunRoot
    )

    if (-not [string]::IsNullOrWhiteSpace($RunRoot)) {
        $accounting = Get-InvocationAccounting -ProcessResult $ProcessResult -RunRoot $RunRoot
        if ([string]$accounting.invocation_certainty -ceq 'AMBIGUOUS') { return 'INVOCATION_AMBIGUOUS' }
    }
    if ($null -eq $ProcessResult) { return 'HARNESS_FAILURE' }
    if ([string]$ProcessResult.invocation_certainty -ceq 'AMBIGUOUS') { return 'INVOCATION_AMBIGUOUS' }
    if (-not $ProcessResult.process_started) { return 'LAUNCH_FAILURE' }
    if ($ProcessResult.harness_failure_after_process_start) { return 'HARNESS_FAILURE_AFTER_PROCESS_START' }
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
        [Parameter(Mandatory = $true)]$PowerEvidence,
        [string]$RunRoot
    )

    $invocationAccounting = Get-ContractPropertyValue -Object $ServiceResult -Name 'invocation_accounting'
    if ($null -eq $invocationAccounting) {
        $invocationAccounting = Get-InvocationAccounting -ProcessResult $ServiceResult.process_result -RunRoot $RunRoot
    }
    if ([string]$invocationAccounting.invocation_certainty -ceq 'AMBIGUOUS') {
        return 'INVOCATION_AMBIGUOUS'
    }
    if ([string]$ServiceResult.result -like 'BLOCKED_*') {
        return [string]$ServiceResult.result
    }
    if ($ServiceResult.token_validation.valid -eq $false) { return 'BLOCKED_TOKEN_CONTRACT' }
    if ($ServiceResult.process_result.process_started -and
        $ServiceResult.process_result.cleanup_succeeded -eq $false) {
        return 'HARNESS_CLEANUP_FAILED'
    }
    if ($ServiceResult.process_result.timeout) { return 'TIMEOUT' }
    if ($ServiceResult.process_result.harness_failure_after_process_start) { return 'HARNESS_FAILURE_AFTER_PROCESS_START' }
    if ($ServiceResult.process_result.harness_failed) { return 'HARNESS_FAILED' }
    if (-not $ServiceResult.process_result.process_started) { return 'LAUNCH_FAILURE' }
    if ($ServiceResult.process_result.target_exit_signed -ne 0) {
        return Get-RuntimeFailureCategory -ProcessResult $ServiceResult.process_result -PowerEvidence $PowerEvidence -RunRoot $RunRoot
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
         live_service_mutation_supported = [bool]$Contract.live_service_mutation_supported
        live_output_acl_mutation_supported = [bool]$Contract.live_output_acl_mutation_supported
        live_control_baseline_lsa_mutation_supported = [bool]$Contract.live_control_baseline_lsa_mutation_supported
        harness_live_contract_allows_control_baseline_lsa_mutation = 'YES'
        allowed_lsa_right = $Contract.allowed_lsa_right
        allowed_lsa_target = $Contract.allowed_lsa_target
        current_task_service_mutations = 0
        current_task_lsa_mutations = 0
        current_task_token_mutations = 0
        current_task_acl_mutations = 0
        current_task_device_mutations = 0
        current_task_driver_mutations = 0
        current_task_platform_security_mutations = 0
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
    $lsaMaterialization = $null
    $lsaCleanup = $null
    $outputRootEvidence = $null
    $outputServiceSidEvidence = $null
    $evidenceSeal = $null
    $registrationCleanup = $null
    $preServiceCleanup = $null
    $wrapperError = $null
    try {
        $preflight = Get-LivePreflight -Contract $Contract -RunRoot $runRoot
        if (-not $preflight.valid) {
            throw ('BLOCKED_PREFLIGHT: {0}' -f ($preflight.failures -join '; '))
        }
        $outputRootEvidence = New-IsolatedOutputRoot -RunRoot $runRoot -Contract $Contract
        $gate = Acquire-OneShotGate -OutputBase $Contract.output_base -RunId $runId -Contract $Contract
        $manifestPath = Join-Path $runRoot 'manifest.json'
        $serviceHostPath = Join-Path $ToolRoot 'service-host.ps1'
        $serviceDefinition = New-QualificationService -Contract $Contract -ManifestPath $manifestPath -ServiceHostPath $serviceHostPath -RunRoot $runRoot
        $serviceCreated = $true
        $outputServiceSidEvidence = Grant-Q1ServiceSidOutputAccess -RunRoot $runRoot -Contract $Contract -ServiceSid ([string]$serviceDefinition.service_sid.sid)
        $lsaMaterialization = Initialize-Q1LsaMaterialization -Contract $Contract -RunRoot $runRoot -ServiceSid ([string]$serviceDefinition.service_sid.sid)
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
            $cleanup = Stop-And-RemoveQualificationService -Contract $Contract -RunRoot $runRoot -KeepRegistration
            $lsaCleanup = Recover-Q1LsaMutationIfNecessary -Contract $Contract -RunRoot $runRoot
            if ($cleanup.stop_verified -and $lsaCleanup.cleanup_verified -and $null -ne $serviceDefinition) {
                $evidenceSeal = Seal-Q1Evidence -Contract $Contract -RunRoot $runRoot -ServiceSid ([string]$serviceDefinition.service_sid.sid)
            }
            if ($cleanup.stop_verified -and $lsaCleanup.cleanup_verified -and
                $null -ne $evidenceSeal -and $evidenceSeal.valid) {
                $registrationCleanup = Remove-QualificationServiceRegistration -Contract $Contract -RunRoot $runRoot
                $cleanup = [pscustomobject]@{
                    stop = $cleanup
                    delete = Get-ContractPropertyValue -Object $registrationCleanup -Name 'delete'
                    before = $cleanup.before
                    after = $registrationCleanup.after
                    stop_verified = [bool]$cleanup.stop_verified
                    service_deleted = [bool]$registrationCleanup.removed
                    registration_kept = -not [bool]$registrationCleanup.removed
                    residue = $registrationCleanup.residue
                    cleanup_verified = [bool]$registrationCleanup.cleanup_verified
                }
            }
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
    if (($null -ne $cleanup -and -not $cleanup.cleanup_verified) -or
        ($null -ne $lsaCleanup -and -not $lsaCleanup.cleanup_verified) -or
        ($null -ne $evidenceSeal -and -not $evidenceSeal.valid) -or
        ($null -ne $registrationCleanup -and -not $registrationCleanup.cleanup_verified)) {
        $classification = 'HARNESS_CLEANUP_FAILED'
    }
    elseif ($null -ne $serviceResult) {
        $classification = Get-ResultClassification -ServiceResult $serviceResult -PowerEvidence $powerEvidence -RunRoot $runRoot
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
    $invocationAccounting = Get-InvocationAccounting -ProcessResult $runtimeProcessResult -RunRoot $runRoot
    $lsaAccounting = Get-Q1LsaMutationAccounting -RunRoot $runRoot
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
        runtime_failure_category = Get-RuntimeFailureCategory -ProcessResult $runtimeProcessResult -PowerEvidence $powerEvidence -RunRoot $runRoot
        csv_validation = $parsedCsv
        power_evidence = $powerEvidence
        pre_service_cleanup = $preServiceCleanup
        output_inventory = $finalInventory
        cleanup = $cleanup
        output_root_mutation = $outputRootEvidence
        output_service_sid_acl = $outputServiceSidEvidence
        evidence_seal = $evidenceSeal
        registration_cleanup = $registrationCleanup
        lsa_materialization = $lsaMaterialization
        lsa_cleanup = $lsaCleanup
        lsa_accounting = $lsaAccounting
        invocation_accounting = $invocationAccounting
        residue = $residue
        wrapper_error = $wrapperError
        gate = $gate
        exact_command = Get-CanonicalCommandRecord -Contract $Contract -OutputDirectory (Join-Path $runRoot 'raw\timechart-output')
        max_runs = $Contract.max_runs
        retries = $Contract.retries
        amd_cli_real_invocations = $invocationAccounting.amd_cli_real_invocations
        amd_api_real_invocations = 0
        power_sampling_runs = $invocationAccounting.power_sampling_runs
        service_mutations = if (Test-Path -LiteralPath (Join-Path $runRoot 'raw\service-mutation-started.json') -PathType Leaf) { 1 } else { 0 }
        lsa_mutations = $lsaAccounting.lsa_mutations
        token_mutations = 0
        acl_mutations = if (Test-Path -LiteralPath (Join-Path $runRoot 'raw\output-acl-mutation-intent.json') -PathType Leaf) { 1 } else { 0 }
        driver_mutations = 0
        platform_security_mutations = 0
        live_service_mutation_supported = [bool]$Contract.live_service_mutation_supported
        live_output_acl_mutation_supported = [bool]$Contract.live_output_acl_mutation_supported
        live_control_baseline_lsa_mutation_supported = [bool]$Contract.live_control_baseline_lsa_mutation_supported
        allowed_lsa_right = $Contract.allowed_lsa_right
        allowed_lsa_target = $Contract.allowed_lsa_target
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
