Set-StrictMode -Version Latest

function Get-AmdLocalServiceSamplingContract {
    [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/v1'
        task_id = 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1'
        harness_task_id = 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-HARNESS-I1'
        account = 'NT AUTHORITY\LocalService'
        account_sid = 'S-1-5-19'
        service_name = 'ResourceTimelineAmdLocalServiceActiveSamplingQualification'
        service_display_name = 'Resource Timeline AMD LocalService active sampling qualification'
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
        harness_identity_mode = 'SOURCE_SHA256_PINNED'
        contract_canonical_sha256 = '7B7D43CA282DB77940C93D49BC5DFEE36B773A0DA71C8A20F591B4F62B2052B1'
        harness_source_sha256 = [ordered]@{
            contract = '7B7D43CA282DB77940C93D49BC5DFEE36B773A0DA71C8A20F591B4F62B2052B1'
            runner = '87DFC3903DAD61A069CC4276C2EBE340F5B5AD4A0FC38FBE9093D3FDBCEEF4BA'
            service_host = 'E4F1F8AE2F25C91E7B2EF43C1A47C5251BF7BFCE8AF203445CDF41A086456C3E'
            sc_argument_contract = 'A238266DF382BFE2870E11ED40A14468EF7BCB58807D0F235D17C5A3C3F5E5FA'
            i2e_runtime_library = 'BC22E7599A64D61BC3B93351328B546656D1393EFABC87106630A86F43A71F08'
            i2e_service_profile_contract = '75CFE997C4F89E2162ECA28AA96655F1210E10574E2A33B3B1A25E26748F9468'
            cleanup_state_contract = 'AB812C8393448AF17DD89518A4B07FD793AD2CB5E444B0DE52B7B9BA5A62A157'
            postprocess = 'BAB1C3505B1A0E1ABC6AF58E85687B0097FEC1B27CE768F8A7AE2FA4FF1EC338'
        }
        live_service_mutation_supported = $true
        live_output_acl_mutation_supported = $true
        live_control_baseline_lsa_mutation_supported = $true
        allowed_lsa_right = 'SeSystemProfilePrivilege'
        allowed_lsa_target = 'EXACT_Q1_SERVICE_SID_ONLY'
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

function Get-ContractPropertyValue {
    param(
        [AllowNull()][object]$Object,
        [Parameter(Mandatory = $true)][string]$Name,
        [AllowNull()][object]$Default = $null
    )

    if ($null -eq $Object) {
        return $Default
    }
    $property = @($Object.PSObject.Properties | Where-Object Name -eq $Name | Select-Object -First 1)
    if ($property.Count -eq 1) {
        return $property[0].Value
    }
    $Default
}

function Get-HarnessSourcePaths {
    param([Parameter(Mandatory = $true)][string]$Root)

    [ordered]@{
        contract = Join-Path $Root 'contract.ps1'
        runner = Join-Path $Root 'run-amd-localservice-active-sampling.ps1'
        service_host = Join-Path $Root 'service-host.ps1'
        sc_argument_contract = Join-Path $Root '..\amd-privilege-qualification\sc-argument-contract.ps1'
        i2e_runtime_library = Join-Path $Root '..\amd-privilege-qualification\i2e-runtime-library.ps1'
        i2e_service_profile_contract = Join-Path $Root '..\amd-privilege-qualification\i2e-service-profile-contract.ps1'
        cleanup_state_contract = Join-Path $Root '..\amd-privilege-qualification\cleanup-state-contract.ps1'
        postprocess = Join-Path $Root '..\amd-uprof-cli-spike\postprocess.ps1'
    }
}

function Get-CanonicalFileSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)

    $text = [IO.File]::ReadAllText($Path, (New-Object System.Text.UTF8Encoding($false)))
    $canonical = $text.Replace(
        ([string][char]13 + [string][char]10),
        [string][char]10
    ).Replace([string][char]13, [string][char]10)
    if ([IO.Path]::GetFileName($Path) -ieq 'contract.ps1') {
        $canonical = [regex]::Replace(
            $canonical,
            "(?m)(contract_canonical_sha256\s*=\s*')[^']*(')",
            '$1__SELF_CANONICAL_SHA256__$2',
            1
        )
        $canonical = [regex]::Replace(
            $canonical,
            "(?m)^\s*contract\s*=\s*'[^']*'",
            "            contract = '__SELF_CANONICAL_SHA256__'",
            1
        )
    }
    $bytes = (New-Object System.Text.UTF8Encoding($false)).GetBytes($canonical)
    $sha = New-Object System.Security.Cryptography.SHA256Managed
    try {
        ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToUpperInvariant()
    }
    finally {
        $sha.Dispose()
    }
}

function Get-HarnessSourceIdentity {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)]$Contract
    )

    $paths = Get-HarnessSourcePaths -Root $Root
    $records = New-Object System.Collections.Generic.List[object]
    $failures = New-Object System.Collections.Generic.List[string]
    foreach ($entry in $paths.GetEnumerator()) {
        $key = [string]$entry.Key
        $path = [string]$entry.Value
        $expected = [string]$Contract.harness_source_sha256[$key]
        $exists = Test-Path -LiteralPath $path -PathType Leaf
        $actual = $null
        if ($exists) {
            try {
                $actual = Get-CanonicalFileSha256 -Path $path
            }
            catch {
                [void]$failures.Add(('{0} hash failed: {1}' -f $key, $_.Exception.Message))
            }
        }
        else {
            [void]$failures.Add(('{0} source file is missing' -f $key))
        }
        $matches = $exists -and
            $expected -notmatch '^__PENDING_' -and
            $actual -cne $null -and
            $actual -ceq $expected
        if (-not $matches) {
            [void]$failures.Add(('{0} SHA256 does not match the reviewed source contract' -f $key))
        }
        [void]$records.Add([pscustomobject]@{
            key = $key
            path = $path
            expected_sha256 = $expected
            sha256 = $actual
            exists = $exists
            matches = $matches
        })
    }
    [pscustomobject]@{
        mode = [string]$Contract.harness_identity_mode
        source_root = $Root
        source_files = $records.ToArray()
        valid = ($failures.Count -eq 0)
        failures = $failures.ToArray()
    }
}

function Test-HarnessSourceIdentity {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)]$Contract
    )

    Get-HarnessSourceIdentity -Root $Root -Contract $Contract
}

function Get-InvocationAccounting {
    param(
        [AllowNull()]$ProcessResult,
        [string]$RunRoot
    )

    $launchIntentPath = $null
    $launchStartedPath = $null
    $launchFailedPath = $null
    $launchIntent = $null
    if (-not [string]::IsNullOrWhiteSpace($RunRoot)) {
        $rawRoot = Join-Path $RunRoot 'raw'
        $launchIntentPath = Join-Path $rawRoot 'cli-launch-intent.json'
        $launchStartedPath = Join-Path $rawRoot 'cli-launch-started.json'
        $launchFailedPath = Join-Path $rawRoot 'cli-launch-start-failed.json'
        if (Test-Path -LiteralPath $launchIntentPath -PathType Leaf) {
            try { $launchIntent = Get-Content -LiteralPath $launchIntentPath -Raw | ConvertFrom-Json } catch { $launchIntent = $null }
        }
    }
    $durableLaunchIntent = -not [string]::IsNullOrWhiteSpace($launchIntentPath) -and
        (Test-Path -LiteralPath $launchIntentPath -PathType Leaf)
    $durableLaunchStarted = -not [string]::IsNullOrWhiteSpace($launchStartedPath) -and
        (Test-Path -LiteralPath $launchStartedPath -PathType Leaf)
    $durableLaunchFailed = -not [string]::IsNullOrWhiteSpace($launchFailedPath) -and
        (Test-Path -LiteralPath $launchFailedPath -PathType Leaf)
    $processStarted = [bool](Get-ContractPropertyValue -Object $ProcessResult -Name 'process_started' -Default $false)
    $invocationAttempted = [bool](Get-ContractPropertyValue -Object $ProcessResult -Name 'invocation_attempted' -Default $false)
    $samplingRuns = [int](Get-ContractPropertyValue -Object $ProcessResult -Name 'power_sampling_runs' -Default 0)
    $gateConsumed = [bool](Get-ContractPropertyValue -Object $launchIntent -Name 'gate_consumed' -Default $false)
    $state = [string](Get-ContractPropertyValue -Object $ProcessResult -Name 'state' -Default $null)
    if ($durableLaunchStarted -or $processStarted -or $invocationAttempted -or ($samplingRuns -gt 0)) {
        $certainty = 'CONFIRMED_ONE'
        $accountingState = 'CONFIRMED_STARTED'
        $started = $true
        $count = 1
    }
    elseif ($durableLaunchIntent -and -not $durableLaunchFailed) {
        $certainty = 'AMBIGUOUS'
        $accountingState = 'AMBIGUOUS_AFTER_LAUNCH_INTENT'
        $started = $false
        $count = 'UNKNOWN_0_OR_1'
    }
    else {
        $certainty = 'CONFIRMED_ZERO'
        $accountingState = if ($durableLaunchFailed) { 'LAUNCH_FAILED' } else { 'NOT_ATTEMPTED' }
        $started = $false
        $count = 0
    }
    if ($accountingState -eq 'CONFIRMED_STARTED' -and [string]::IsNullOrWhiteSpace($state)) {
        $state = if ($durableLaunchStarted) { 'PROCESS_STARTED_EVIDENCE_PRESENT' } else { 'PROCESS_STARTED' }
    }
    [pscustomobject]@{
        durable_launch_intent = $durableLaunchIntent
        durable_launch_started = $durableLaunchStarted
        durable_launch_failed = $durableLaunchFailed
        launch_intent_path = $launchIntentPath
        launch_started_path = $launchStartedPath
        launch_failed_path = $launchFailedPath
        gate_consumed = $gateConsumed -or $durableLaunchIntent
        second_run_forbidden = ($gateConsumed -or $durableLaunchIntent)
        process_started = $started
        invocation_attempted = $count
        amd_cli_real_invocations = $count
        power_sampling_runs = $count
        invocation_certainty = $certainty
        accounting_state = $accountingState
        state = $state
        irreversible = ($certainty -ne 'CONFIRMED_ZERO')
    }
}

function Get-Q1LsaMutationAccounting {
    param([Parameter(Mandatory = $true)][string]$RunRoot)

    $rawRoot = Join-Path $RunRoot 'raw'
    $startedPath = Join-Path $rawRoot 'lsa-mutation-started.json'
    $ownershipPath = Join-Path $rawRoot 'lsa-ownership.json'
    $recoveryPath = Join-Path $rawRoot 'lsa-recovery.json'
    $attempted = Test-Path -LiteralPath $startedPath -PathType Leaf
    $ownership = $null
    $recovery = $null
    if (Test-Path -LiteralPath $ownershipPath -PathType Leaf) {
        try {
            $ownership = Get-Content -LiteralPath $ownershipPath -Raw | ConvertFrom-Json
        }
        catch {
            $ownership = $null
        }
    }
    if (Test-Path -LiteralPath $recoveryPath -PathType Leaf) {
        try {
            $recovery = Get-Content -LiteralPath $recoveryPath -Raw | ConvertFrom-Json
        }
        catch {
            $recovery = $null
        }
    }
    [pscustomobject]@{
        mutation_attempted = $attempted
        lsa_mutations = if ($attempted) { 1 } else { 0 }
        added_by_run = if ($null -ne $ownership) { [bool]$ownership.added_by_run } else { $false }
        ownership_known = ($null -ne $ownership)
        recovery_state = if ($null -ne $recovery) { [string]$recovery.recovery_state } else { $null }
        residual_state = if ($null -ne $recovery) { [string]$recovery.residual_state } else { if ($attempted) { 'UNKNOWN' } else { 'NONE' } }
        cleanup_verified = if ($null -ne $recovery) { [bool]$recovery.cleanup_verified } else { -not $attempted }
        started_path = $startedPath
        ownership_path = $ownershipPath
        recovery_path = $recoveryPath
    }
}

function New-OneShotGateRecord {
    param(
        [Parameter(Mandatory = $true)][string]$TaskId,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][int]$MaxRuns,
        [Parameter(Mandatory = $true)][int]$Retries,
        [Parameter(Mandatory = $true)][bool]$RealExecutionAllowed
    )

    [ordered]@{
        schema = 'amd-localservice-active-sampling-q1/gate/v1'
        task_id = $TaskId
        run_id = $RunId
        max_runs = $MaxRuns
        retries = $Retries
        state = 'CONSUMED'
        consumed_before_service_registration = $true
        consumed_at_utc = [DateTime]::UtcNow.ToString('o')
        real_execution_allowed = $RealExecutionAllowed
    }
}

function Acquire-OneShotGateFile {
    param(
        [Parameter(Mandatory = $true)][string]$GatePath,
        [Parameter(Mandatory = $true)]$GateRecord
    )

    $parent = Split-Path -Parent $GatePath
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    $json = $GateRecord | ConvertTo-Json -Depth 30
    $bytes = (New-Object System.Text.UTF8Encoding($false)).GetBytes($json)
    try {
        $stream = New-Object IO.FileStream(
            $GatePath,
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
        throw "one-shot gate unavailable or already consumed: $GatePath"
    }
    [pscustomobject]@{
        path = $GatePath
        state = [string]$GateRecord.state
        max_runs = [int]$GateRecord.max_runs
        retries = [int]$GateRecord.retries
    }
}

function Test-Q1LsaBeforeMaterialization {
    param(
        [Parameter(Mandatory = $true)]$Snapshot,
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)][string]$Right
    )

    $failures = New-Object System.Collections.Generic.List[string]
    $direct = Get-ContractPropertyValue -Object $Snapshot -Name 'direct'
    $assignment = Get-ContractPropertyValue -Object $Snapshot -Name 'assignment'
    if ([string](Get-ContractPropertyValue -Object $direct -Name 'status') -cne 'READ') {
        [void]$failures.Add('direct Service SID right snapshot is unavailable')
    }
    if ([string](Get-ContractPropertyValue -Object $assignment -Name 'status') -cne 'READ') {
        [void]$failures.Add('user-right assignment snapshot is unavailable')
    }
    $directRights = @((Get-ContractPropertyValue -Object $direct -Name 'direct_rights' -Default @()) |
        ForEach-Object { [string]$_ })
    $assigned = @((Get-ContractPropertyValue -Object $assignment -Name 'assigned_principals' -Default @()) |
        ForEach-Object { [string]$_ })
    if ($directRights -contains $Right) {
        [void]$failures.Add('SeSystemProfilePrivilege already exists on the exact Service SID')
    }
    if ($assigned | Where-Object { $_ -ieq $ServiceSid }) {
        [void]$failures.Add('exact Service SID already has the required user-right assignment')
    }
    $unexpected = @($directRights | Where-Object { $_ -cne $Right })
    if ($unexpected.Count -ne 0) {
        [void]$failures.Add(('unexpected pre-existing direct rights: {0}' -f ($unexpected -join ', ')))
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
        right = $Right
        service_sid = $ServiceSid
        right_absent_before = ($directRights -notcontains $Right -and
            -not ($assigned | Where-Object { $_ -ieq $ServiceSid }))
    }
}

function Test-Q1LsaAfterMaterialization {
    param(
        [Parameter(Mandatory = $true)]$Snapshot,
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)][string]$Right
    )

    $failures = New-Object System.Collections.Generic.List[string]
    $direct = Get-ContractPropertyValue -Object $Snapshot -Name 'direct'
    $assignment = Get-ContractPropertyValue -Object $Snapshot -Name 'assignment'
    if ([string](Get-ContractPropertyValue -Object $direct -Name 'status') -cne 'READ') {
        [void]$failures.Add('direct Service SID right readback is unavailable')
    }
    if ([string](Get-ContractPropertyValue -Object $assignment -Name 'status') -cne 'READ') {
        [void]$failures.Add('user-right assignment readback is unavailable')
    }
    $directRights = @((Get-ContractPropertyValue -Object $direct -Name 'direct_rights' -Default @()) |
        ForEach-Object { [string]$_ })
    $assigned = @((Get-ContractPropertyValue -Object $assignment -Name 'assigned_principals' -Default @()) |
        ForEach-Object { [string]$_ })
    if ($directRights.Count -ne 1 -or $directRights[0] -cne $Right) {
        [void]$failures.Add('exact Service SID direct rights readback is not only SeSystemProfilePrivilege')
    }
    if (-not ($assigned | Where-Object { $_ -ieq $ServiceSid })) {
        [void]$failures.Add('exact Service SID user-right assignment is absent after materialization')
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
        right = $Right
        service_sid = $ServiceSid
        exact_right_materialized = ($directRights.Count -eq 1 -and
            $directRights[0] -ceq $Right -and
            [bool]($assigned | Where-Object { $_ -ieq $ServiceSid }))
    }
}

function Get-Q1LsaMaterializationDecision {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After,
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)][string]$Right,
        [Parameter(Mandatory = $true)][bool]$MutationAttempted
    )

    $beforeGate = Test-Q1LsaBeforeMaterialization -Snapshot $Before -ServiceSid $ServiceSid -Right $Right
    $afterGate = Test-Q1LsaAfterMaterialization -Snapshot $After -ServiceSid $ServiceSid -Right $Right
    $owned = $MutationAttempted -and $beforeGate.valid -and $afterGate.valid
    [pscustomobject]@{
        valid = ($beforeGate.valid -and $afterGate.valid -and $MutationAttempted)
        before = $beforeGate
        after = $afterGate
        mutation_attempted = $MutationAttempted
        added_by_run = $owned
        cleanup_allowed = $owned
        failures = @($beforeGate.failures + $afterGate.failures)
    }
}

function Test-Q1LsaCleanupEvidence {
    param(
        [Parameter(Mandatory = $true)]$Snapshot,
        [Parameter(Mandatory = $true)][string]$ServiceSid,
        [Parameter(Mandatory = $true)][string]$Right
    )

    $failures = New-Object System.Collections.Generic.List[string]
    $direct = Get-ContractPropertyValue -Object $Snapshot -Name 'direct'
    $assignment = Get-ContractPropertyValue -Object $Snapshot -Name 'assignment'
    if ([string](Get-ContractPropertyValue -Object $direct -Name 'status') -cne 'READ') {
        [void]$failures.Add('direct Service SID cleanup readback is unavailable')
    }
    if ([string](Get-ContractPropertyValue -Object $assignment -Name 'status') -cne 'READ') {
        [void]$failures.Add('user-right cleanup readback is unavailable')
    }
    $directRights = @((Get-ContractPropertyValue -Object $direct -Name 'direct_rights' -Default @()) |
        ForEach-Object { [string]$_ })
    $assigned = @((Get-ContractPropertyValue -Object $assignment -Name 'assigned_principals' -Default @()) |
        ForEach-Object { [string]$_ })
    if ($directRights -contains $Right) {
        [void]$failures.Add('required right remains on the exact Service SID after cleanup')
    }
    if ($assigned | Where-Object { $_ -ieq $ServiceSid }) {
        [void]$failures.Add('exact Service SID remains assigned the required right after cleanup')
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
        service_sid = $ServiceSid
        right = $Right
    }
}

function Get-Q1LsaRecoveryPlan {
    param(
        [AllowNull()]$Intent,
        [AllowNull()]$MutationStarted,
        [AllowNull()]$Before,
        [AllowNull()]$Current,
        [Parameter(Mandatory = $true)][string]$ExpectedServiceName,
        [Parameter(Mandatory = $true)][string]$ExpectedServiceSid,
        [Parameter(Mandatory = $true)][string]$ExpectedRight
    )

    $failures = New-Object System.Collections.Generic.List[string]
    $mutationAttempted = [bool](Get-ContractPropertyValue -Object $MutationStarted -Name 'mutation_attempted' -Default $false) -or
        [string](Get-ContractPropertyValue -Object $MutationStarted -Name 'state' -Default $null) -eq 'MUTATION_ATTEMPTED'
    if (-not $mutationAttempted) {
        return [pscustomobject]@{
            valid = $true
            action = 'NONE'
            recovery_state = 'NO_MUTATION_ATTEMPTED'
            residual_state = 'NONE'
            mutation_attempted = $false
            service_sid = $ExpectedServiceSid
            right = $ExpectedRight
            failures = @()
        }
    }
    if ($null -eq $Intent) { [void]$failures.Add('durable LSA mutation intent is missing') }
    if ($null -eq $Before) { [void]$failures.Add('durable LSA pre-mutation snapshot is missing') }
    if ([string](Get-ContractPropertyValue -Object $Intent -Name 'task_id') -cne 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1') {
        [void]$failures.Add('LSA mutation intent task id is not the Q1 task')
    }
    if ([string](Get-ContractPropertyValue -Object $Intent -Name 'service_name') -cne $ExpectedServiceName) {
        [void]$failures.Add('LSA mutation intent service name mismatch')
    }
    if ([string](Get-ContractPropertyValue -Object $Intent -Name 'service_sid') -cne $ExpectedServiceSid) {
        [void]$failures.Add('LSA mutation intent Service SID mismatch')
    }
    if ([string](Get-ContractPropertyValue -Object $Intent -Name 'right') -cne $ExpectedRight) {
        [void]$failures.Add('LSA mutation intent right mismatch')
    }
    if ([string](Get-ContractPropertyValue -Object $Intent -Name 'target') -cne 'EXACT_Q1_SERVICE_SID_ONLY') {
        [void]$failures.Add('LSA mutation intent target is not the exact Q1 Service SID')
    }
    if ([string](Get-ContractPropertyValue -Object $Intent -Name 'mutation_intent') -cne 'ADD_EXACT_RIGHT') {
        [void]$failures.Add('LSA mutation intent is not ADD_EXACT_RIGHT')
    }
    if ([string](Get-ContractPropertyValue -Object $Intent -Name 'ownership_before') -cne 'ABSENT' -or
        -not [bool](Get-ContractPropertyValue -Object $Intent -Name 'right_absent_before' -Default $false)) {
        [void]$failures.Add('durable pre-mutation evidence does not prove exact right absence')
    }
    if ([string](Get-ContractPropertyValue -Object $Intent -Name 'cleanup_if_ambiguous') -cne 'REQUIRED') {
        [void]$failures.Add('ambiguous LSA cleanup was not marked required')
    }
    if ([string](Get-ContractPropertyValue -Object $MutationStarted -Name 'service_sid') -cne $ExpectedServiceSid -or
        [string](Get-ContractPropertyValue -Object $MutationStarted -Name 'right') -cne $ExpectedRight) {
        [void]$failures.Add('LSA mutation-started evidence target mismatch')
    }
    $beforeGate = $null
    if ($null -ne $Before) {
        $beforeGate = Test-Q1LsaBeforeMaterialization -Snapshot $Before -ServiceSid $ExpectedServiceSid -Right $ExpectedRight
        if (-not $beforeGate.valid) {
            foreach ($failure in @($beforeGate.failures)) { [void]$failures.Add([string]$failure) }
        }
    }
    $currentDirect = Get-ContractPropertyValue -Object $Current -Name 'direct'
    $currentAssignment = Get-ContractPropertyValue -Object $Current -Name 'assignment'
    $currentDirectStatus = [string](Get-ContractPropertyValue -Object $currentDirect -Name 'status')
    $currentAssignmentStatus = [string](Get-ContractPropertyValue -Object $currentAssignment -Name 'status')
    if ($currentDirectStatus -cne 'READ' -or $currentAssignmentStatus -cne 'READ') {
        [void]$failures.Add('current LSA readback is unavailable')
    }
    $currentDirectRights = @((Get-ContractPropertyValue -Object $currentDirect -Name 'direct_rights' -Default @()) |
        ForEach-Object { [string]$_ })
    $currentAssigned = @((Get-ContractPropertyValue -Object $currentAssignment -Name 'assigned_principals' -Default @()) |
        ForEach-Object { [string]$_ })
    $unexpected = @($currentDirectRights | Where-Object { $_ -cne $ExpectedRight })
    if ($unexpected.Count -ne 0) {
        [void]$failures.Add(('unexpected direct rights are present: {0}' -f ($unexpected -join ', ')))
    }
    $directPresent = $currentDirectRights -contains $ExpectedRight
    $assignedPresent = [bool]($currentAssigned | Where-Object { $_ -ieq $ExpectedServiceSid })
    if ($directPresent -xor $assignedPresent) {
        [void]$failures.Add('current exact-right direct and assignment readbacks disagree')
    }
    if ($failures.Count -ne 0) {
        [pscustomobject]@{
            valid = $false
            action = 'FAILED_CLOSED'
            recovery_state = if ($currentDirectStatus -eq 'READ' -and $currentAssignmentStatus -eq 'READ') { 'FAILED_CLOSED' } else { 'FAILED_CLOSED' }
            residual_state = if ($currentDirectStatus -eq 'READ' -and $currentAssignmentStatus -eq 'READ') { 'PRESENT_OR_UNKNOWN' } else { 'UNKNOWN' }
            mutation_attempted = $true
            service_sid = $ExpectedServiceSid
            right = $ExpectedRight
            before = $beforeGate
            current_direct_rights = $currentDirectRights
            current_assigned_principals = $currentAssigned
            failures = @($failures)
        }
        return
    }
    if (-not $directPresent) {
        return [pscustomobject]@{
            valid = $true
            action = 'NONE'
            recovery_state = 'RIGHT_ALREADY_ABSENT'
            residual_state = 'ABSENT'
            mutation_attempted = $true
            service_sid = $ExpectedServiceSid
            right = $ExpectedRight
            before = $beforeGate
            current_direct_rights = $currentDirectRights
            current_assigned_principals = $currentAssigned
            failures = @()
        }
    }
    [pscustomobject]@{
        valid = $true
        action = 'REMOVE_EXACT_RIGHT'
        recovery_state = 'OWNED_RIGHT_REMOVAL_REQUIRED'
        residual_state = 'PRESENT'
        mutation_attempted = $true
        service_sid = $ExpectedServiceSid
        right = $ExpectedRight
        before = $beforeGate
        current_direct_rights = $currentDirectRights
        current_assigned_principals = $currentAssigned
        failures = @()
    }
}

function Get-Q1OutputAclModel {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('STAGING', 'AUTHORIZED', 'SEALED')][string]$Phase,
        [AllowNull()][string]$ServiceSid
    )

    $rules = New-Object System.Collections.Generic.List[object]
    [void]$rules.Add([pscustomobject]@{ identity = 'S-1-5-18'; rights = 'FullControl' })
    [void]$rules.Add([pscustomobject]@{ identity = 'S-1-5-32-544'; rights = 'FullControl' })
    if ($Phase -eq 'AUTHORIZED') {
        if ([string]::IsNullOrWhiteSpace($ServiceSid) -or $ServiceSid -notmatch '^S-1-5-80-') {
            throw 'AUTHORIZED output ACL model requires an exact Service SID'
        }
        [void]$rules.Add([pscustomobject]@{ identity = $ServiceSid; rights = 'Modify' })
    }
    [pscustomobject]@{
        phase = $Phase
        service_sid = $ServiceSid
        rules = $rules.ToArray()
        localservice_account_wide_write = $false
    }
}

function Test-Q1OutputAclModel {
    param(
        [Parameter(Mandatory = $true)]$Model,
        [Parameter(Mandatory = $true)][ValidateSet('STAGING', 'AUTHORIZED', 'SEALED')][string]$ExpectedPhase,
        [AllowNull()][string]$ExpectedServiceSid
    )

    $failures = New-Object System.Collections.Generic.List[string]
    if ([string]$Model.phase -cne $ExpectedPhase) { [void]$failures.Add('ACL phase mismatch') }
    if ([bool]$Model.localservice_account_wide_write) { [void]$failures.Add('LocalService account-wide write access is present') }
    $rules = @($Model.rules)
    $serviceRule = @($rules | Where-Object { [string]$_.identity -ieq $ExpectedServiceSid })
    if ($ExpectedPhase -eq 'AUTHORIZED' -and $serviceRule.Count -ne 1) {
        [void]$failures.Add('exact Q1 Service SID write rule is absent')
    }
    if ($ExpectedPhase -ne 'AUTHORIZED' -and $serviceRule.Count -ne 0) {
        [void]$failures.Add('Q1 Service SID write rule remains outside the authorized phase')
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
        phase = $ExpectedPhase
        service_sid = $ExpectedServiceSid
        service_sid_has_write = ($serviceRule.Count -eq 1 -and [string]$serviceRule[0].rights -match 'Modify|Write|FullControl')
        localservice_account_wide_write = [bool]$Model.localservice_account_wide_write
    }
}

function Get-Q1EvidenceManifestEntries {
    param([Parameter(Mandatory = $true)][string]$Root)

    if (-not (Test-Path -LiteralPath $Root -PathType Container)) {
        throw "evidence root does not exist: $Root"
    }
    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\')
    $files = @(Get-ChildItem -LiteralPath $rootFull -File -Recurse -ErrorAction Stop |
        Where-Object {
            $relative = $_.FullName.Substring($rootFull.Length).TrimStart('\').Replace('\', '/')
            $relative -ne 'raw/evidence-manifest.json' -and $relative -notlike 'summary/*'
        } |
        Sort-Object FullName)
    @($files | ForEach-Object {
        $relative = $_.FullName.Substring($rootFull.Length).TrimStart('\').Replace('\', '/')
        [pscustomobject]@{
            relative_path = $relative
            size = [int64]$_.Length
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToUpperInvariant()
        }
    })
}

function Test-Q1EvidenceManifest {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)]$Manifest
    )

    $failures = New-Object System.Collections.Generic.List[string]
    foreach ($entry in @($Manifest.entries)) {
        $relative = ([string]$entry.relative_path).Replace('/', '\')
        $path = Join-Path $Root $relative
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            [void]$failures.Add(('missing evidence file: {0}' -f $entry.relative_path))
            continue
        }
        $file = [IO.FileInfo]$path
        if ([int64]$file.Length -ne [int64]$entry.size) {
            [void]$failures.Add(('evidence size mismatch: {0}' -f $entry.relative_path))
        }
        $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToUpperInvariant()
        if ($actual -cne [string]$entry.sha256) {
            [void]$failures.Add(('evidence SHA256 mismatch: {0}' -f $entry.relative_path))
        }
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = @($failures)
        checked_entries = @($Manifest.entries).Count
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

function Test-Q1ServiceConfigurationEvidence {
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
        if ([string]$Evidence.display_name -cne $Contract.service_display_name) {
            [void]$failures.Add('service display name mismatch')
        }
        if ([string]$Evidence.path_name -notmatch [regex]::Escape($ExpectedBinPath)) {
            [void]$failures.Add('service image path does not match the frozen host command')
        }
    }
    if (-not [bool]$ServiceSidEvidence.valid) {
        [void]$failures.Add('dedicated Service SID evidence is invalid')
    }
    if ([string]$ServiceSidEvidence.sid_type -ine $Contract.service_sid_type) {
        [void]$failures.Add('dedicated Service SID type is not unrestricted')
    }
    [pscustomobject]@{
        valid = ($failures.Count -eq 0)
        failures = $failures.ToArray()
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
    }
}
