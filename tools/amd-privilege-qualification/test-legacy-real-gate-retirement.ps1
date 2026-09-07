#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ToolRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$legacyContractPath = Join-Path $ToolRoot 'legacy-real-gate-contract.ps1'
$legacyRoot = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-privilege'
$systemRoot = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-system-counter'

function Invoke-LegacyChild {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Arguments
    )

    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $Path @Arguments 2>&1 |
            ForEach-Object { [string]$_ })
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    [pscustomobject]@{
        exit_code = [int]$LASTEXITCODE
        text = $output -join [Environment]::NewLine
    }
}

function Assert-Marker {
    param(
        [Parameter(Mandatory = $true)]$Result,
        [Parameter(Mandatory = $true)][string]$Marker,
        [Parameter(Mandatory = $true)][string]$Description
    )
    if ($Result.text.IndexOf($Marker, [StringComparison]::Ordinal) -lt 0) {
        throw "$Description did not emit '$Marker'. exit=$($Result.exit_code)`n$($Result.text)"
    }
}

function Assert-StateUnchanged {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After,
        [Parameter(Mandatory = $true)][string]$Description
    )
    $beforeJson = $Before | ConvertTo-Json -Depth 20 -Compress
    $afterJson = $After | ConvertTo-Json -Depth 20 -Compress
    if ($beforeJson -cne $afterJson) {
        throw "$Description changed read-only machine/evidence state.`nBefore=$beforeJson`nAfter=$afterJson"
    }
}

function Get-RootObservation {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        return @('ABSENT')
    }
    try {
        $entries = @(
            Get-ChildItem -LiteralPath $Path -Force -Recurse -ErrorAction Stop |
                Sort-Object FullName |
                ForEach-Object {
                    $length = if ($_.PSIsContainer) { 0L } else { [int64]$_.Length }
                    '{0}|{1}|{2}|{3}' -f $_.FullName, $_.PSIsContainer, $length, $_.LastWriteTimeUtc.Ticks
                }
        )
        return @('PRESENT') + $entries
    }
    catch {
        return @('UNREADABLE_STABLE')
    }
}

function Get-HistoricalEvidenceObservation {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        return [pscustomobject]@{ status = 'ABSENT'; files = @() }
    }
    try {
        $files = @(
            Get-ChildItem -LiteralPath $Path -Force -Recurse -File -ErrorAction Stop |
                Sort-Object FullName |
                ForEach-Object {
                    try {
                        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256 -ErrorAction Stop).Hash
                        '{0}|SHA256:{1}' -f $_.FullName, $hash.ToUpperInvariant()
                    }
                    catch {
                        '{0}|UNREADABLE_STABLE' -f $_.FullName
                    }
                }
        )
        return [pscustomobject]@{ status = 'READABLE_OR_STABLE'; files = @($files) }
    }
    catch {
        return [pscustomobject]@{ status = 'UNREADABLE_STABLE'; files = @() }
    }
}

function Get-ServiceObservation {
    param([Parameter(Mandatory = $true)][string]$Name)
    try {
        $service = @(Get-CimInstance -ClassName Win32_Service -Filter "Name='$Name'" -ErrorAction Stop |
            Select-Object -First 1)
        if ($service.Count -eq 0) { return 'ABSENT' }
        return '{0}|{1}|{2}|{3}' -f $service[0].State, $service[0].ProcessId,
            $service[0].StartName, $service[0].PathName
    }
    catch {
        $fallback = @(Get-Service -Name $Name -ErrorAction SilentlyContinue)
        if ($fallback.Count -eq 0) { return 'ABSENT' }
        return 'PRESENT|{0}' -f $fallback[0].Status
    }
}

function Get-LegacyMachineObservation {
    [pscustomobject]@{
        privilege_service = Get-ServiceObservation -Name 'ResourceTimelineAmdPrivilegeQualification'
        system_service = Get-ServiceObservation -Name 'ResourceTimelineAmdSystemCounterQualification'
        broker_process_count = @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue).Count
        amd_cli_process_count = @(Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue).Count
        legacy_root_inventory = @(Get-RootObservation -Path $legacyRoot)
        system_root_inventory = @(Get-RootObservation -Path $systemRoot)
    }
}

function Get-LegacyEvidenceObservation {
    [pscustomobject]@{
        legacy = Get-HistoricalEvidenceObservation -Path $legacyRoot
        system = Get-HistoricalEvidenceObservation -Path $systemRoot
    }
}

function Assert-GuardOrder {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Guard,
        [Parameter(Mandatory = $true)][string[]]$After,
        [Parameter(Mandatory = $true)][string]$Description
    )
    $source = Get-Content -LiteralPath $Path -Raw
    $guardIndex = $source.IndexOf($Guard, [StringComparison]::Ordinal)
    if ($guardIndex -lt 0) { throw "$Description is missing guard marker '$Guard'." }
    foreach ($needle in $After) {
        $index = $source.IndexOf($needle, [StringComparison]::Ordinal)
        if ($index -lt 0) { throw "$Description is missing executable marker '$needle'." }
        if ($guardIndex -ge $index) { throw "$Description places the guard after '$needle'." }
    }
}

if (-not (Test-Path -LiteralPath $legacyContractPath -PathType Leaf)) {
    throw "Legacy retirement contract is missing: $legacyContractPath"
}
. $legacyContractPath
foreach ($requiredContract in @(
        '$I2LegacyRealGateConsumed = $true',
        '$I2BrokerSetupAllowed = $false',
        '$I2PowerSamplingClientAllowed = $false',
        '$I2CounterDiscoveryClientAllowed = $false',
        '$I2CleanupAllowed = $false',
        '$I2cSystemComparisonAllowed = $false',
        '$I2cSystemCleanupAllowed = $false'
    )) {
    if ((Get-Content -LiteralPath $legacyContractPath -Raw).IndexOf($requiredContract, [StringComparison]::Ordinal) -lt 0) {
        throw "Legacy retirement contract is missing: $requiredContract"
    }
}

$descriptors = @(
    [pscustomobject]@{
        name = 'I2 broker setup'
        path = Join-Path $ToolRoot 'run-admin-amd-privilege-qualification.ps1'
        plan_marker = 'I2_BROKER_SETUP_PLAN_ONLY=true'
        real_marker = 'I2_BROKER_SETUP_RERUN_FORBIDDEN'
        real_arguments = @('-ExecuteAuthorizedSetup')
        sentinel_arguments = @('-ExecuteAuthorizedSetup', '-InternalTestOnlyPreMutationSentinel')
        sentinel_marker = 'I2_BROKER_SETUP_AUTHORIZED_PRE_MUTATION_SENTINEL=true'
    }
    [pscustomobject]@{
        name = 'I2 power client'
        path = Join-Path $ToolRoot 'run-standard-user-amd-privilege-client.ps1'
        plan_marker = 'I2_POWER_CLIENT_PLAN_ONLY=true'
        real_marker = 'I2_POWER_CLIENT_RERUN_FORBIDDEN'
        real_arguments = @('-ExecuteAuthorizedClient')
        sentinel_arguments = @('-ExecuteAuthorizedClient', '-InternalTestOnlyPreRuntimeSentinel')
        sentinel_marker = 'I2_POWER_CLIENT_AUTHORIZED_PRE_RUNTIME_SENTINEL=true'
    }
    [pscustomobject]@{
        name = 'I2B counter client'
        path = Join-Path $ToolRoot 'run-standard-user-amd-counter-discovery.ps1'
        plan_marker = 'I2B_COUNTER_CLIENT_PLAN_ONLY=true'
        real_marker = 'I2B_COUNTER_DISCOVERY_RERUN_FORBIDDEN'
        real_arguments = @('-ExecuteAuthorizedClient')
        sentinel_arguments = @('-ExecuteAuthorizedClient', '-InternalTestOnlyPreRuntimeSentinel')
        sentinel_marker = 'I2B_COUNTER_CLIENT_AUTHORIZED_PRE_RUNTIME_SENTINEL=true'
    }
    [pscustomobject]@{
        name = 'I2 legacy cleanup'
        path = Join-Path $ToolRoot 'cleanup-admin-amd-privilege-qualification.ps1'
        plan_marker = 'I2_LEGACY_CLEANUP_PLAN_ONLY=true'
        real_marker = 'I2_LEGACY_CLEANUP_RERUN_FORBIDDEN'
        real_arguments = @('-ExecuteAuthorizedCleanup')
        sentinel_arguments = @('-ExecuteAuthorizedCleanup', '-InternalTestOnlyPreMutationSentinel')
        sentinel_marker = 'I2_LEGACY_CLEANUP_AUTHORIZED_PRE_MUTATION_SENTINEL=true'
    }
    [pscustomobject]@{
        name = 'I2C SYSTEM comparison'
        path = Join-Path $ToolRoot 'run-admin-amd-system-counter-qualification.ps1'
        plan_marker = 'I2C_SYSTEM_PLAN_ONLY=true'
        real_marker = 'I2C_SYSTEM_COMPARISON_RERUN_FORBIDDEN'
        real_arguments = @('-ExecuteAuthorizedComparison')
        sentinel_arguments = @('-ExecuteAuthorizedComparison', '-InternalTestOnlyPreMutationSentinel')
        sentinel_marker = 'I2C_SYSTEM_AUTHORIZED_PRE_MUTATION_SENTINEL=true'
    }
    [pscustomobject]@{
        name = 'I2C SYSTEM cleanup'
        path = Join-Path $ToolRoot 'cleanup-admin-amd-system-counter-qualification.ps1'
        plan_marker = 'I2C_SYSTEM_CLEANUP_PLAN_ONLY=true'
        real_marker = 'I2C_SYSTEM_CLEANUP_RERUN_FORBIDDEN'
        real_arguments = @('-ExecuteAuthorizedCleanup')
        sentinel_arguments = @('-ExecuteAuthorizedCleanup', '-InternalTestOnlyPreMutationSentinel')
        sentinel_marker = 'I2C_SYSTEM_CLEANUP_AUTHORIZED_PRE_MUTATION_SENTINEL=true'
    }
)

foreach ($descriptor in $descriptors) {
    if (-not (Test-Path -LiteralPath $descriptor.path -PathType Leaf)) {
        throw "Legacy retirement wrapper is missing: $($descriptor.path)"
    }
}

Assert-GuardOrder -Path $descriptors[0].path -Guard 'I2_BROKER_SETUP_RERUN_FORBIDDEN' -After @(
    '$adminIdentity = Assert-Administrator',
    '$hash = (Get-FileHash -LiteralPath $ArtifactPath',
    '$amdCliPreflight = Get-AmdCliPreflight',
    '$scope = [Guid]::NewGuid',
    'Invoke-Sc -Arguments $createArguments'
) -Description 'I2 broker setup retirement guard'
Assert-GuardOrder -Path $descriptors[1].path -Guard 'I2_POWER_CLIENT_RERUN_FORBIDDEN' -After @(
    'Get-CurrentProcessIntegrityLevel',
    '$config = Get-Content -LiteralPath $ConfigPath',
    '$hash = (Get-FileHash -LiteralPath $ArtifactPath',
    '& $ArtifactPath --client start'
) -Description 'I2 power-client retirement guard'
Assert-GuardOrder -Path $descriptors[2].path -Guard 'I2B_COUNTER_DISCOVERY_RERUN_FORBIDDEN' -After @(
    'Get-CurrentProcessIntegrityLevel',
    '$config = Get-Content -LiteralPath $ConfigPath',
    '$hash = (Get-FileHash -LiteralPath $ArtifactPath',
    '& $ArtifactPath --client counter-discovery'
) -Description 'I2B counter-client retirement guard'
Assert-GuardOrder -Path $descriptors[3].path -Guard 'I2_LEGACY_CLEANUP_RERUN_FORBIDDEN' -After @(
    '$identity = [Security.Principal.WindowsIdentity]::GetCurrent()',
    '$config = Get-Content -LiteralPath $ConfigPath',
    'Get-CimInstance -ClassName Win32_Service',
    'Write-CleanupEvidence'
) -Description 'I2 legacy cleanup retirement guard'
Assert-GuardOrder -Path $descriptors[4].path -Guard 'I2C_SYSTEM_COMPARISON_RERUN_FORBIDDEN' -After @(
    '$adminIdentity = Assert-Administrator',
    '$hash = (Get-FileHash -LiteralPath $ArtifactPath',
    '$scope = [Guid]::NewGuid',
    'Invoke-Sc -Arguments (New-QualificationServiceCreateArguments'
) -Description 'I2C SYSTEM comparison retirement guard'
Assert-GuardOrder -Path $descriptors[5].path -Guard 'I2C_SYSTEM_CLEANUP_RERUN_FORBIDDEN' -After @(
    '$identity = [Security.Principal.WindowsIdentity]::GetCurrent()',
    '$config = Get-Content -LiteralPath $ConfigPath',
    '$cleanupAttempt = ',
    'Get-SystemServiceSnapshot',
    'Write-SystemCleanupEvidence'
) -Description 'I2C SYSTEM cleanup retirement guard'

# Every historical executable wrapper remains reviewable, but must carry an
# explicit retirement marker.  This prevents a future wrapper from becoming
# an accidentally active historical real gate.
$retiredWrapperMarkers = [ordered]@{
    'run-admin-amd-i2e-service-profile-experiment.ps1' = 'I2E_RERUN_FORBIDDEN'
    'resume-admin-amd-i2e-treatment.ps1' = 'I2E_TREATMENT_RESUME_RERUN_FORBIDDEN'
    'cleanup-admin-amd-i2e-service-profile-experiment.ps1' = 'I2E_CLEANUP_RERUN_FORBIDDEN'
    'run-admin-amd-i2f-service-profile-experiment.ps1' = 'I2F_RERUN_FORBIDDEN'
    'cleanup-admin-amd-i2f-service-profile-experiment.ps1' = 'I2F_CLEANUP_RERUN_FORBIDDEN'
    'run-admin-amd-privilege-qualification.ps1' = 'I2_BROKER_SETUP_RERUN_FORBIDDEN'
    'run-standard-user-amd-privilege-client.ps1' = 'I2_POWER_CLIENT_RERUN_FORBIDDEN'
    'run-standard-user-amd-counter-discovery.ps1' = 'I2B_COUNTER_DISCOVERY_RERUN_FORBIDDEN'
    'cleanup-admin-amd-privilege-qualification.ps1' = 'I2_LEGACY_CLEANUP_RERUN_FORBIDDEN'
    'run-admin-amd-system-counter-qualification.ps1' = 'I2C_SYSTEM_COMPARISON_RERUN_FORBIDDEN'
    'cleanup-admin-amd-system-counter-qualification.ps1' = 'I2C_SYSTEM_CLEANUP_RERUN_FORBIDDEN'
}
foreach ($entry in $retiredWrapperMarkers.GetEnumerator()) {
    $path = Join-Path $ToolRoot $entry.Key
    $source = Get-Content -LiteralPath $path -Raw
    if ($source.IndexOf([string]$entry.Value, [StringComparison]::Ordinal) -lt 0) {
        throw "Historical executable wrapper has no retirement marker: $($entry.Key)"
    }
}
Write-Host 'AMD_QUALIFICATION_EXECUTABLE_ENTRYPOINT_AUDIT=PASS_NO_UNRETIRED_HISTORICAL_REAL_GATE'

$beforeMachine = Get-LegacyMachineObservation
$beforeEvidence = Get-LegacyEvidenceObservation
$previousSentinel = $env:AMD_LEGACY_OFFLINE_TEST_SENTINEL
try {
    $env:AMD_LEGACY_OFFLINE_TEST_SENTINEL = 'true'
    foreach ($descriptor in $descriptors) {
        $plan = Invoke-LegacyChild -Path $descriptor.path -Arguments @()
        if ($plan.exit_code -ne 0) { throw "$($descriptor.name) plan-only failed: $($plan.text)" }
        Assert-Marker -Result $plan -Marker $descriptor.plan_marker -Description "$($descriptor.name) plan-only"
        Write-Host (($descriptor.plan_marker -replace '=true$', '=PASS'))

        $library = Invoke-LegacyChild -Path $descriptor.path -Arguments @('-LibraryOnly')
        if ($library.exit_code -ne 0) { throw "$($descriptor.name) LibraryOnly failed: $($library.text)" }

        $sentinel = Invoke-LegacyChild -Path $descriptor.path -Arguments $descriptor.sentinel_arguments
        if ($sentinel.exit_code -ne 0) { throw "$($descriptor.name) offline sentinel failed: $($sentinel.text)" }
        Assert-Marker -Result $sentinel -Marker $descriptor.sentinel_marker -Description "$($descriptor.name) offline sentinel"

        $beforeRealMachine = Get-LegacyMachineObservation
        $beforeRealEvidence = Get-LegacyEvidenceObservation
        $forbidden = Invoke-LegacyChild -Path $descriptor.path -Arguments $descriptor.real_arguments
        if ($forbidden.exit_code -eq 0) { throw "$($descriptor.name) real path unexpectedly succeeded." }
        Assert-Marker -Result $forbidden -Marker $descriptor.real_marker -Description "$($descriptor.name) retirement guard"
        $afterRealMachine = Get-LegacyMachineObservation
        $afterRealEvidence = Get-LegacyEvidenceObservation
        Assert-StateUnchanged -Before $beforeRealMachine -After $afterRealMachine -Description "$($descriptor.name) real invocation"
        Assert-StateUnchanged -Before $beforeRealEvidence -After $afterRealEvidence -Description "$($descriptor.name) historical evidence"

        switch ($descriptor.name) {
            'I2 broker setup' { Write-Host 'I2_BROKER_SETUP_REAL_RERUN_GUARD=PASS' }
            'I2 power client' { Write-Host 'I2_POWER_CLIENT_REAL_RERUN_GUARD=PASS' }
            'I2B counter client' { Write-Host 'I2B_COUNTER_CLIENT_REAL_RERUN_GUARD=PASS' }
            'I2 legacy cleanup' { Write-Host 'I2_LEGACY_CLEANUP_REAL_RERUN_GUARD=PASS' }
            'I2C SYSTEM comparison' { Write-Host 'I2C_SYSTEM_REAL_RERUN_GUARD=PASS' }
            'I2C SYSTEM cleanup' { Write-Host 'I2C_SYSTEM_CLEANUP_REAL_RERUN_GUARD=PASS' }
        }
    }
}
finally {
    if ($null -eq $previousSentinel) {
        Remove-Item Env:AMD_LEGACY_OFFLINE_TEST_SENTINEL -ErrorAction SilentlyContinue
    } else {
        $env:AMD_LEGACY_OFFLINE_TEST_SENTINEL = $previousSentinel
    }
}

$afterMachine = Get-LegacyMachineObservation
$afterEvidence = Get-LegacyEvidenceObservation
Assert-StateUnchanged -Before $beforeMachine -After $afterMachine -Description 'legacy real-gate matrix'
Assert-StateUnchanged -Before $beforeEvidence -After $afterEvidence -Description 'legacy historical evidence matrix'
Write-Host 'LEGACY_REAL_GATES_PRECEDE_MACHINE_ACCESS=PASS'
Write-Host 'LEGACY_REAL_GATE_MACHINE_STATE_UNCHANGED=PASS'
Write-Host 'LEGACY_AMD_HISTORICAL_EVIDENCE_UNCHANGED=PASS'
Write-Host 'LEGACY_REAL_GATE_RETIREMENT=PASS'
