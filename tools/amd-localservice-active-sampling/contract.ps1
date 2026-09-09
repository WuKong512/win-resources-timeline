Set-StrictMode -Version Latest

function Get-AmdLocalServiceSamplingContract {
    [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/v1'
        task_id = 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1'
        harness_task_id = 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-HARNESS-I1'
        account = 'NT AUTHORITY\LocalService'
        account_sid = 'S-1-5-19'
        service_name = 'ResourceTimelineAmdLocalServiceActiveSamplingQualification'
        service_sid_type = 'unrestricted'
        session_id = 0
        interactive = $false
        architecture = 'x64'
        amd_cli_path = 'D:\apps\AMDuProf\bin\AMDuProfCLI.exe'
        amd_cli_sha256 = 'D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC'
        amd_cli_file_version = '5.3.521.0'
        amd_cli_product_version = '5.3.521.0'
        amd_cli_signer = 'Advanced Micro Devices'
        amd_driver_signer_pattern = '(?i)Advanced Micro Devices|\bAMD\b'
        amd_cli_event = 'power'
        amd_cli_interval_ms = 1000
        amd_cli_duration_seconds = 10
        amd_cli_format = 'csv'
        max_runs = 1
        retries = 0
        cli_timeout_ms = 30000
        service_timeout_ms = 45000
        output_base = 'C:\ProgramData\ResourceTimeline\qualification\amd-localservice-active-sampling-q1'
        system_profile_privilege = 'SeSystemProfilePrivilege'
        forbidden_privileges = @(
            'SeProfileSingleProcessPrivilege',
            'SeDebugPrivilege',
            'SeTcbPrivilege'
        )
        required_enabled_privileges = @(
            'SeChangeNotifyPrivilege',
            'SeCreateGlobalPrivilege',
            'SeImpersonatePrivilege',
            'SeSystemProfilePrivilege'
        )
        forbidden_group_sids = @('S-1-5-32-544')
        expected_main_pin = 'e74153e74a416a1c8c542498b8b87cc6d146f5cf'
        design_checkpoint = 'd2a2e536181924f0dededd8b15f2964cffbe8101'
        authorization_token = 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1'
        authorization_environment = 'AMD_LOCALSERVICE_ACTIVE_SAMPLING_AUTHORIZATION'
        authorization_environment_value = 'GRANTED_FOR_THIS_TASK_ONLY'
        gate_file_name = 'Q1-LIVE-GATE.json'
        driver_versions = [ordered]@{
            AMDPowerProfiler = [ordered]@{
                file_name = 'AMDPowerProfiler.sys'
                path = 'C:\Windows\System32\drivers\AMDPowerProfiler.sys'
                file_version = '10.6.3.0'
                product_version = '5.3.481.0'
                signer_pattern = '(?i)Advanced Micro Devices|\bAMD\b'
            }
            AMDCpuProfiler = [ordered]@{
                file_name = 'AMDCpuProfiler.sys'
                path = 'C:\Windows\System32\drivers\AMDCpuProfiler.sys'
                file_version = '4.4.1.0'
                product_version = '5.3.481.0'
                signer_pattern = '(?i)Advanced Micro Devices|\bAMD\b'
            }
        }
    }
}

function Get-FixedAmdCliArguments {
    param(
        [Parameter(Mandatory = $true)][string]$OutputDirectory
    )

    @(
        'timechart'
        '--event'
        'power'
        '--interval'
        '1000'
        '--duration'
        '10'
        '--format'
        'csv'
        '--output-dir'
        $OutputDirectory
    )
}

function New-QualificationRunId {
    $timestamp = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ')
    'q1-{0}-{1}' -f $timestamp, ([Guid]::NewGuid().ToString('N'))
}

function Get-QualificationRunRoot {
    param(
        [Parameter(Mandatory = $true)][string]$OutputBase,
        [Parameter(Mandatory = $true)][string]$RunId
    )

    Join-Path $OutputBase $RunId
}

function Test-ControlledRunRoot {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$OutputBase,
        [switch]$AllowExisting
    )

    $failures = New-Object System.Collections.Generic.List[string]
    try {
        $baseFull = [IO.Path]::GetFullPath($OutputBase).TrimEnd('\')
        $rootFull = [IO.Path]::GetFullPath($RunRoot).TrimEnd('\')
        $parent = [IO.DirectoryInfo]$rootFull
        if ($null -eq $parent.Parent -or
            $parent.Parent.FullName.TrimEnd('\') -ine $baseFull) {
            [void]$failures.Add('run root must be exactly one child of the controlled output base')
        }
        if ([string]::IsNullOrWhiteSpace($parent.Name) -or
            $parent.Name -match '[\\/:*?"<>|]') {
            [void]$failures.Add('run id is empty or contains an invalid path character')
        }
        if ($parent.Name -in @('.', '..')) {
            [void]$failures.Add('run id contains a traversal component')
        }
    }
    catch {
        [void]$failures.Add(('run root path is invalid: {0}' -f $_.Exception.Message))
    }
    if (-not $AllowExisting -and (Test-Path -LiteralPath $RunRoot)) {
        [void]$failures.Add('run root already exists')
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
        run_root = $RunRoot
        output_base = $OutputBase
        preexisting = (Test-Path -LiteralPath $RunRoot)
    }
}

function Get-ExpectedTokenContract {
    param(
        [Parameter(Mandatory = $true)]$Contract
    )

    [ordered]@{
        user_sid = $Contract.account_sid
        session_id = $Contract.session_id
        interactive = $Contract.interactive
        architecture = $Contract.architecture
        integrity_sid = 'S-1-16-16384'
        administrators_absent = $true
        service_sid_required = $true
        required_enabled_privileges = @($Contract.required_enabled_privileges)
        forbidden_privileges = @($Contract.forbidden_privileges)
        forbidden_group_sids = @($Contract.forbidden_group_sids)
    }
}

function Test-EffectiveTokenEvidence {
    param(
        [Parameter(Mandatory = $true)]$Evidence,
        [Parameter(Mandatory = $true)]$Contract,
        [AllowNull()][string]$ExpectedServiceSid
    )

    $failures = New-Object System.Collections.Generic.List[string]
    if ([string]$Evidence.user_sid -ine $Contract.account_sid) {
        [void]$failures.Add('token user SID does not match S-1-5-19')
    }
    if ([int]$Evidence.session_id -ne [int]$Contract.session_id) {
        [void]$failures.Add('service is not in Session 0')
    }
    if ([bool]$Evidence.interactive) {
        [void]$failures.Add('service context is interactive')
    }
    if ([string]$Evidence.architecture -ine $Contract.architecture) {
        [void]$failures.Add('service process is not x64')
    }
    if ([string]$Evidence.integrity_sid -ine 'S-1-16-16384') {
        [void]$failures.Add('service integrity level is not System')
    }
    $groups = @($Evidence.group_sids | ForEach-Object { [string]$_ })
    if ($groups -contains 'S-1-5-32-544') {
        [void]$failures.Add('Administrators membership is present')
    }
    if (-not ($groups | Where-Object { $_ -match '^S-1-5-80-' })) {
        [void]$failures.Add('dedicated Service SID is absent from the effective token')
    }
    if (-not [string]::IsNullOrWhiteSpace($ExpectedServiceSid) -and
        -not ($groups | Where-Object { $_ -ieq $ExpectedServiceSid })) {
        [void]$failures.Add('effective token does not contain the exact configured Service SID')
    }
    $privilegeMap = @{}
    foreach ($privilege in @($Evidence.privileges)) {
        if ($null -ne $privilege.name) {
            $privilegeMap[[string]$privilege.name] = [string]$privilege.state
        }
    }
    foreach ($required in @($Contract.required_enabled_privileges)) {
        if (-not $privilegeMap.ContainsKey($required)) {
            [void]$failures.Add(('{0} is absent from the effective token' -f $required))
        }
        elseif ($privilegeMap[$required] -ine 'Enabled') {
            [void]$failures.Add(('{0} is not enabled' -f $required))
        }
    }
    foreach ($forbidden in @($Contract.forbidden_privileges)) {
        if ($privilegeMap.ContainsKey($forbidden)) {
            [void]$failures.Add(('{0} is present in the effective token' -f $forbidden))
        }
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
    }
}

function Get-OfflineTokenFixture {
    param(
        [string]$ServiceSid = 'S-1-5-80-1234567890-1234567890-1234567890-1234567890-1234'
    )

    [pscustomobject]@{
        user = 'NT AUTHORITY\LocalService'
        user_sid = 'S-1-5-19'
        session_id = 0
        interactive = $false
        architecture = 'x64'
        integrity_sid = 'S-1-16-16384'
        group_sids = @('S-1-5-19', 'S-1-5-80-1234567890-1234567890-1234567890-1234567890-1234')
        privileges = @(
            [pscustomobject]@{ name = 'SeChangeNotifyPrivilege'; state = 'Enabled' }
            [pscustomobject]@{ name = 'SeCreateGlobalPrivilege'; state = 'Enabled' }
            [pscustomobject]@{ name = 'SeImpersonatePrivilege'; state = 'Enabled' }
            [pscustomobject]@{ name = 'SeSystemProfilePrivilege'; state = 'Enabled' }
        )
    }
}

function Get-OfflineBinaryFixture {
    param(
        [Parameter(Mandatory = $true)]$Contract
    )

    [pscustomobject]@{
        path = $Contract.amd_cli_path
        sha256 = $Contract.amd_cli_sha256
        file_version = $Contract.amd_cli_file_version
        product_version = $Contract.amd_cli_product_version
        architecture = $Contract.architecture
        signer = $Contract.amd_cli_signer
        signature_status = 'Valid'
    }
}

function Test-BinaryIdentityEvidence {
    param(
        [Parameter(Mandatory = $true)]$Identity,
        [Parameter(Mandatory = $true)]$Contract
    )

    $failures = New-Object System.Collections.Generic.List[string]
    if ([string]$Identity.path -cne $Contract.amd_cli_path) {
        [void]$failures.Add('AMD CLI path mismatch')
    }
    if ([string]$Identity.sha256 -cne $Contract.amd_cli_sha256) {
        [void]$failures.Add('AMD CLI SHA256 mismatch')
    }
    if ([string]$Identity.file_version -cne $Contract.amd_cli_file_version) {
        [void]$failures.Add('AMD CLI file version mismatch')
    }
    if ([string]$Identity.product_version -cne $Contract.amd_cli_product_version) {
        [void]$failures.Add('AMD CLI product version mismatch')
    }
    if ([string]$Identity.architecture -ine $Contract.architecture) {
        [void]$failures.Add('AMD CLI architecture mismatch')
    }
    if ([string]$Identity.signature_status -ine 'Valid') {
        [void]$failures.Add('AMD CLI Authenticode status is not Valid')
    }
    if ([string]$Identity.signer -notmatch [regex]::Escape($Contract.amd_cli_signer)) {
        [void]$failures.Add('AMD CLI signer mismatch')
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
    }
}

function Get-OfflineDriverFixture {
    param(
        [Parameter(Mandatory = $true)]$Contract
    )

    @(
        [pscustomobject]@{
            name = 'AMDPowerProfiler'
            path = $Contract.driver_versions.AMDPowerProfiler.path
            file_version = $Contract.driver_versions.AMDPowerProfiler.file_version
            product_version = $Contract.driver_versions.AMDPowerProfiler.product_version
            signature_status = 'Valid'
            signer = 'CN=Advanced Micro Devices, Inc.'
            state = 'Running'
        }
        [pscustomobject]@{
            name = 'AMDCpuProfiler'
            path = $Contract.driver_versions.AMDCpuProfiler.path
            file_version = $Contract.driver_versions.AMDCpuProfiler.file_version
            product_version = $Contract.driver_versions.AMDCpuProfiler.product_version
            signature_status = 'Valid'
            signer = 'CN=Advanced Micro Devices, Inc.'
            state = 'Running'
        }
    )
}

function Test-DriverVersionEvidence {
    param(
        [Parameter(Mandatory = $true)]$Drivers,
        [Parameter(Mandatory = $true)]$Contract
    )

    $failures = New-Object System.Collections.Generic.List[string]
    foreach ($name in @('AMDPowerProfiler', 'AMDCpuProfiler')) {
        $expected = $Contract.driver_versions.$name
        $actual = @($Drivers | Where-Object { [string]$_.name -ieq $name }) | Select-Object -First 1
        if ($null -eq $actual) {
            [void]$failures.Add(('{0} driver evidence is absent' -f $name))
            continue
        }
        if ([string]$actual.file_version -cne $expected.file_version) {
            [void]$failures.Add(('{0} file version mismatch' -f $name))
        }
        if ([string]$actual.product_version -cne $expected.product_version) {
            [void]$failures.Add(('{0} product version mismatch' -f $name))
        }
        if ([string]$actual.state -ine 'Running') {
            [void]$failures.Add(('{0} driver is not Running' -f $name))
        }
        if ([string]$actual.signature_status -ine 'Valid') {
            [void]$failures.Add(('{0} driver Authenticode status is not Valid' -f $name))
        }
        if ([string]$actual.signer -notmatch $expected.signer_pattern) {
            [void]$failures.Add(('{0} driver signer is not AMD' -f $name))
        }
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
    }
}

function Get-CanonicalCommandRecord {
    param(
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)][string]$OutputDirectory
    )

    [ordered]@{
        executable = $Contract.amd_cli_path
        arguments = @(Get-FixedAmdCliArguments -OutputDirectory $OutputDirectory)
        working_directory = (Split-Path -Parent $Contract.amd_cli_path)
        duration_seconds = $Contract.amd_cli_duration_seconds
        interval_ms = $Contract.amd_cli_interval_ms
        max_runs = $Contract.max_runs
        retries = $Contract.retries
    }
}

function Get-OfflineCsvFixtureText {
    @"
PROFILE RECORDS
Timestamp,Record ID,Socket0-Package-Power
2026-01-01T00:00:00Z,1,40.5
2026-01-01T00:00:01Z,2,41.25
2026-01-01T00:00:02Z,3,42.0
"@
}

function Test-PackagePowerEvidence {
    param(
        [Parameter(Mandatory = $true)]$Parsed
    )

    $samples = @($Parsed.samples)
    $values = @($samples | ForEach-Object { [double]$_.value_watts })
    $finite = $true
    foreach ($value in $values) {
        if ([double]::IsNaN($value) -or [double]::IsInfinity($value) -or $value -lt 0) {
            $finite = $false
        }
    }
    $nonConstant = $false
    if ($values.Count -gt 1) {
        $first = $values[0]
        $nonConstant = [bool]($values | Where-Object { $_ -ne $first } | Select-Object -First 1)
    }
    $reason = $null
    if ([string]$Parsed.status -ne 'PASS') {
        $reason = 'CSV parser did not return PASS'
    }
    elseif ($values.Count -lt 2) {
        $reason = 'at least two package-power samples are required to reject a constant placeholder'
    }
    elseif (-not $finite) {
        $reason = 'package-power values are not finite non-negative numbers'
    }
    elseif (-not $nonConstant) {
        $reason = 'package-power values are constant'
    }
    [pscustomobject]@{
        status = if ($null -eq $reason) { 'PASS' } else { 'FAIL' }
        parser_status = [string]$Parsed.status
        sample_count = $values.Count
        finite_non_negative = $finite
        non_constant = $nonConstant
        minimum_watts = if ($values.Count -gt 0) { ($values | Measure-Object -Minimum).Minimum } else { $null }
        maximum_watts = if ($values.Count -gt 0) { ($values | Measure-Object -Maximum).Maximum } else { $null }
        reason = $reason
        samples = $samples
    }
}

function Get-OfflinePlan {
    param(
        [Parameter(Mandatory = $true)]$Contract
    )

    $runId = New-QualificationRunId
    $runRoot = Get-QualificationRunRoot -OutputBase $Contract.output_base -RunId $runId
    $outputDirectory = Join-Path $runRoot 'raw\timechart-output'
    [ordered]@{
        schema = $Contract.schema
        task_id = $Contract.task_id
        harness_task_id = $Contract.harness_task_id
        mode = 'OFFLINE_DRY_RUN'
        run_id = $runId
        run_root = $runRoot
        output_directory = $outputDirectory
        service_name = $Contract.service_name
        account = $Contract.account
        account_sid = $Contract.account_sid
        session_id = $Contract.session_id
        interactive = $Contract.interactive
        token_contract = Get-ExpectedTokenContract -Contract $Contract
        binary = Get-OfflineBinaryFixture -Contract $Contract
        drivers = @(Get-OfflineDriverFixture -Contract $Contract)
        command = Get-CanonicalCommandRecord -Contract $Contract -OutputDirectory $outputDirectory
        gate = [ordered]@{
            max_runs = $Contract.max_runs
            retries = $Contract.retries
            consumed = $false
            real_execution_allowed = $false
        }
        amd_cli_real_invocations = 0
        power_sampling_runs = 0
        service_mutations = 0
        lsa_mutations = 0
        token_mutations = 0
        acl_mutations = 0
        driver_mutations = 0
        platform_security_mutations = 0
    }
}
