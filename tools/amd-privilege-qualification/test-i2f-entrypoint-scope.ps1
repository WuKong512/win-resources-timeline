#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ToolRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$setupPath = Join-Path $ToolRoot 'run-admin-amd-i2f-service-profile-experiment.ps1'
$cleanupPath = Join-Path $ToolRoot 'cleanup-admin-amd-i2f-service-profile-experiment.ps1'
$libraryPath = Join-Path $ToolRoot 'i2e-runtime-library.ps1'
$serviceName = 'ResourceTimelineAmdSystemProfileEnableQualification'
$qualificationRoot = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-system-profile-enable'

function Invoke-I2fChildFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [AllowEmptyCollection()][string[]]$Arguments
    )

    $previousErrorActionPreference = $ErrorActionPreference
    try {
        # A forbidden child entrypoint is expected to write its stable marker
        # through the error stream.  Capture that stream as data so the parent
        # test can assert the nonzero exit contract instead of terminating while
        # collecting the expected rejection.
        $ErrorActionPreference = 'Continue'
        $output = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $Path @Arguments 2>&1 |
            ForEach-Object { [string]$_ })
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    [pscustomobject]@{
        exit_code = [int]$LASTEXITCODE
        output = $output
        text = $output -join [Environment]::NewLine
    }
}

function Invoke-I2fChildCommand {
    param([Parameter(Mandatory = $true)][string]$Command)

    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -Command $Command 2>&1 |
            ForEach-Object { [string]$_ })
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    [pscustomobject]@{
        exit_code = [int]$LASTEXITCODE
        output = $output
        text = $output -join [Environment]::NewLine
    }
}

function Assert-I2fChildMarker {
    param(
        [Parameter(Mandatory = $true)]$Result,
        [Parameter(Mandatory = $true)][string]$Marker,
        [Parameter(Mandatory = $true)][string]$Description
    )

    if ($Result.exit_code -ne 0 -or $Result.text.IndexOf($Marker, [StringComparison]::Ordinal) -lt 0) {
        throw "$Description failed: exit=$($Result.exit_code)`n$($Result.text)"
    }
}

function Assert-I2fRerunGuard {
    param([Parameter(Mandatory = $true)]$Result)

    if ($Result.exit_code -eq 0 -or
        $Result.text.IndexOf('I2F_RERUN_FORBIDDEN', [StringComparison]::Ordinal) -lt 0 -or
        $Result.text.IndexOf('f68bf4d3d36547a0ba753cff489bb6eb', [StringComparison]::Ordinal) -lt 0) {
        throw "I2F consumed-gate guard failed: exit=$($Result.exit_code)`n$($Result.text)"
    }
}

function Assert-I2fCleanupRerunGuard {
    param([Parameter(Mandatory = $true)]$Result)

    if ($Result.exit_code -eq 0 -or
        $Result.text.IndexOf('I2F_CLEANUP_RERUN_FORBIDDEN', [StringComparison]::Ordinal) -lt 0 -or
        $Result.text.IndexOf('f68bf4d3d36547a0ba753cff489bb6eb', [StringComparison]::Ordinal) -lt 0) {
        throw "I2F cleanup retirement guard failed: exit=$($Result.exit_code)`n$($Result.text)"
    }
    foreach ($forbiddenEvidenceMarker in @(
            'I2F_POLICY_REMOVE_ATTEMPTED=',
            'I2F_LSA_REMOVE_CALLS=',
            'I2F_FULL_ROLLBACK_VERIFIED=')) {
        if ($Result.text.IndexOf($forbiddenEvidenceMarker, [StringComparison]::Ordinal) -ge 0) {
            throw "I2F cleanup retirement guard entered the historical cleanup state machine: $forbiddenEvidenceMarker"
        }
    }
}

function Get-I2fHistoricalEvidenceContentSnapshot {
    $root = 'C:\ProgramData\ResourceTimeline\qualification\amd-system-profile-enable\f68bf4d3d36547a0ba753cff489bb6eb'
    $names = @(
        'I2F-ROLLBACK.json',
        'I2F-LSA-BEFORE.json',
        'I2F-LSA-AFTER-ADD.json',
        'I2F-TOKEN-BEFORE-ENABLE.json',
        'I2F-ADJUST-TOKEN-PRIVILEGES.json',
        'I2F-TOKEN-AFTER-ENABLE.json',
        'I2F-TOKEN-ENABLE-DELTA.json',
        'I2F-COUNTER-DISCOVERY-RESULT.json',
        'I2F-COUNTER-DISCOVERY-SUMMARY.json'
    )
    $hashes = [ordered]@{}
    $readable = $true
    foreach ($name in $names) {
        $path = Join-Path $root $name
        try {
            $hashes[$name] = (Get-FileHash -LiteralPath $path -Algorithm SHA256 -ErrorAction Stop).Hash.ToUpperInvariant()
        } catch {
            # The authoritative root is ACL-protected on some qualification
            # hosts.  Preserve that observation instead of weakening the
            # retired-entrypoint test or attempting any ACL workaround.
            $readable = $false
            $hashes[$name] = 'UNREADABLE'
        }
    }
    [pscustomobject]@{
        root = $root
        readable = $readable
        hashes = $hashes
    }
}

function Get-I2fMachineState {
    $serviceState = $null
    try {
        $service = @(Get-CimInstance -ClassName Win32_Service -Filter "Name='$serviceName'" -ErrorAction Stop |
            Select-Object -First 1)
        $serviceState = if ($service.Count -eq 0) {
            'ABSENT'
        } else {
            '{0}|{1}|{2}|{3}' -f $service[0].State, $service[0].ProcessId, $service[0].StartName, $service[0].PathName
        }
    } catch {
        # Non-elevated plan-only validation may not be allowed to query the
        # full Win32_Service record.  Get-Service still proves registration
        # presence without requiring service mutation rights.
        $service = @(Get-Service -Name $serviceName -ErrorAction SilentlyContinue)
        $serviceState = if ($service.Count -eq 0) {
            'ABSENT'
        } else {
            'PRESENT|{0}' -f $service[0].Status
        }
    }

    $rootEntries = @()
    if (Test-Path -LiteralPath $qualificationRoot -PathType Container) {
        try {
            $rootEntries = @(
                Get-ChildItem -LiteralPath $qualificationRoot -Force -ErrorAction Stop |
                    Sort-Object FullName |
                    ForEach-Object {
                        $length = if ($_.PSIsContainer) { 0L } else { [int64]$_.Length }
                        '{0}|{1}|{2}|{3}' -f $_.FullName, $_.PSIsContainer, $length, $_.LastWriteTimeUtc.Ticks
                    }
            )
        } catch {
            # A protected historical evidence root is a stable read-only
            # observation for this plan-only test, not permission to mutate it.
            $rootEntries = @('UNREADABLE')
        }
    }

    [pscustomobject]@{
        service = $serviceState
        root_exists = Test-Path -LiteralPath $qualificationRoot -PathType Container
        root_entries = @($rootEntries)
        broker_process_count = @(
            Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue
        ).Count
        amd_cli_process_count = @(
            Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue
        ).Count
    }
}

function Assert-I2fMachineStateUnchanged {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After,
        [Parameter(Mandatory = $true)][string]$Description
    )

    if (($Before | ConvertTo-Json -Depth 10 -Compress) -cne ($After | ConvertTo-Json -Depth 10 -Compress)) {
        throw "$Description changed the machine-state snapshot.`nBefore=$($Before | ConvertTo-Json -Depth 10 -Compress)`nAfter=$($After | ConvertTo-Json -Depth 10 -Compress)"
    }
}

if (-not (Test-Path -LiteralPath $setupPath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $cleanupPath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $libraryPath -PathType Leaf)) {
    throw 'I2F entrypoint scope fixtures are missing.'
}

$setupSource = Get-Content -LiteralPath $setupPath -Raw
$cleanupSource = Get-Content -LiteralPath $cleanupPath -Raw
foreach ($source in @($setupSource, $cleanupSource)) {
    if ($source -match '(?i)run-admin-amd-i2e-service-profile-experiment\.ps1') {
        throw 'I2F executable wrappers must not dot-source the executable I2E wrapper.'
    }
}
Write-Host 'I2F_DOTSOURCE_EXECUTABLE_I2E_WRAPPER=PASS'

$rerunGuardIndex = $setupSource.IndexOf('I2F_RERUN_FORBIDDEN', [StringComparison]::Ordinal)
if ($rerunGuardIndex -lt 0) {
    throw 'I2F consumed-gate runtime guard is missing.'
}
foreach ($requiredBeforeGuard in @(
        '$null = Assert-I2eAdministrator',
        '$artifactHash = Assert-I2fArtifact',
        'Get-I2eServiceSnapshot -ServiceName $ServiceName',
        '$identityCheck = Compare-I2fCurrentAmdIdentity',
        '[Guid]::NewGuid()',
        'New-Item -ItemType Directory -Force -Path $QualificationRoot, $outputRoot',
        'Invoke-I2eSc -Arguments $createArgs',
        'Add-I2eExactServiceProfileRight -ServiceSid $serviceSid'
    )) {
    $requiredIndex = $setupSource.IndexOf($requiredBeforeGuard, [StringComparison]::Ordinal)
    if ($requiredIndex -lt 0 -or $rerunGuardIndex -ge $requiredIndex) {
        throw "I2F consumed-gate runtime guard is not before: $requiredBeforeGuard"
    }
}
Write-Host 'I2F_RERUN_GUARD_PRECEDES_ADMIN_GATE=PASS'

$cleanupGuardIndex = $cleanupSource.IndexOf('I2F_CLEANUP_RERUN_FORBIDDEN', [StringComparison]::Ordinal)
if ($cleanupGuardIndex -lt 0) {
    throw 'I2F cleanup retirement guard is missing.'
}
foreach ($requiredBeforeCleanupGuard in @(
        '$null = Assert-I2eAdministrator',
        '$roots = Get-I2fCleanupRoots',
        '$cleanupResult = Invoke-I2fCleanup',
        'Get-I2eServiceSnapshot -ServiceName $ServiceName',
        'Read-I2fJson -Path $preflightPath'
    )) {
    $requiredIndex = $cleanupSource.IndexOf($requiredBeforeCleanupGuard, [StringComparison]::Ordinal)
    if ($requiredIndex -lt 0 -or $cleanupGuardIndex -ge $requiredIndex) {
        throw "I2F cleanup retirement guard is not before: $requiredBeforeCleanupGuard"
    }
}
Write-Host 'I2F_CLEANUP_RERUN_GUARD_PRECEDES_ADMIN_GATE=PASS'
Write-Host 'I2F_CLEANUP_GUARD_PRECEDES_ROOT_ENUMERATION=PASS'
Write-Host 'I2F_CLEANUP_GUARD_PRECEDES_STATE_MACHINE=PASS'

$libraryErrors = $null
$libraryTokens = $null
$libraryAst = [System.Management.Automation.Language.Parser]::ParseFile(
    $libraryPath, [ref]$libraryTokens, [ref]$libraryErrors)
if ($libraryErrors.Count -ne 0 -or $null -ne $libraryAst.ParamBlock) {
    throw 'I2E runtime library must have no executable parameter block or parse errors.'
}
$allowedContractLoads = @(
    'sc-argument-contract.ps1',
    'cleanup-state-contract.ps1',
    'i2e-service-profile-contract.ps1'
)
foreach ($statement in @($libraryAst.EndBlock.Statements)) {
    if ($statement -is [System.Management.Automation.Language.FunctionDefinitionAst]) { continue }
    $statementText = $statement.Extent.Text.Trim()
    if ($statementText -notmatch '^\.\s*\(Join-Path\s+\$PSScriptRoot\s+''([^'']+)''\)$') {
        throw "Shared runtime library has executable top-level code: $statementText"
    }
    $loadedContract = $Matches[1]
    if ($allowedContractLoads -notcontains $loadedContract) {
        throw "Shared runtime library loads an unapproved top-level script: $loadedContract"
    }
}
Write-Host 'SHARED_LIBRARY_LOAD_SIDE_EFFECTS=NONE'

$beforePlan = Get-I2fMachineState
$setupPlan = Invoke-I2fChildFile -Path $setupPath -Arguments @()
Assert-I2fChildMarker -Result $setupPlan -Marker 'I2F_PLAN_ONLY=true' -Description 'I2F setup plan-only entrypoint'
Assert-I2fChildMarker -Result $setupPlan -Marker 'I2F_GATE_CONSUMED=true' -Description 'I2F plan consumed-gate state'
Assert-I2fChildMarker -Result $setupPlan -Marker 'I2F_RERUN=FORBIDDEN' -Description 'I2F plan rerun state'
Assert-I2fChildMarker -Result $setupPlan -Marker 'AUTHORITATIVE_SCOPE=f68bf4d3d36547a0ba753cff489bb6eb' -Description 'I2F plan authoritative scope'
Assert-I2fChildMarker -Result $setupPlan -Marker 'No service, LSA mutation, token adjustment, or AMD runtime was performed.' -Description 'I2F setup plan-only safety output'
$afterSetupPlan = Get-I2fMachineState
Assert-I2fMachineStateUnchanged -Before $beforePlan -After $afterSetupPlan -Description 'I2F setup plan-only entrypoint'
Write-Host 'I2F_PLAN_ONLY_REAL_ENTRYPOINT=PASS'
Write-Host 'I2F_PLAN_ONLY_OUTPUT=I2F_PLAN_ONLY=true'

$libraryOnlyResult = Invoke-I2fChildFile -Path $setupPath -Arguments @('-LibraryOnly')
if ($libraryOnlyResult.exit_code -ne 0) {
    throw "I2F LibraryOnly entrypoint failed: $($libraryOnlyResult.text)"
}
Write-Host 'I2F_LIBRARY_ONLY_REAL_ENTRYPOINT=PASS'

$beforeRerunGuard = Get-I2fMachineState
$rerunGuard = Invoke-I2fChildFile -Path $setupPath -Arguments @('-ExecuteAuthorizedExperiment')
Assert-I2fRerunGuard -Result $rerunGuard
$afterRerunGuard = Get-I2fMachineState
Assert-I2fMachineStateUnchanged -Before $beforeRerunGuard -After $afterRerunGuard -Description 'I2F consumed-gate rerun guard'
Write-Host 'I2F_REAL_RERUN_GUARD=PASS'
Write-Host 'I2F_RERUN_GUARD_MACHINE_STATE_UNCHANGED=PASS'

$beforeCleanupPlan = Get-I2fMachineState
$cleanupPlan = Invoke-I2fChildFile -Path $cleanupPath -Arguments @()
Assert-I2fChildMarker -Result $cleanupPlan -Marker 'I2F_CLEANUP_PLAN_ONLY=true' -Description 'I2F cleanup plan-only entrypoint'
Assert-I2fChildMarker -Result $cleanupPlan -Marker 'I2F_GATE_CONSUMED=true' -Description 'I2F cleanup plan consumed-gate state'
Assert-I2fChildMarker -Result $cleanupPlan -Marker 'I2F_CLEANUP_RERUN=FORBIDDEN' -Description 'I2F cleanup plan rerun state'
Assert-I2fChildMarker -Result $cleanupPlan -Marker 'I2F_REAL_CLEANUP=FORBIDDEN' -Description 'I2F cleanup plan retirement state'
Assert-I2fChildMarker -Result $cleanupPlan -Marker 'AUTHORITATIVE_SCOPE=f68bf4d3d36547a0ba753cff489bb6eb' -Description 'I2F cleanup plan authoritative scope'
Assert-I2fChildMarker -Result $cleanupPlan -Marker 'AUTHORITATIVE_ROLLBACK=COMPLETE' -Description 'I2F cleanup plan rollback state'
Assert-I2fChildMarker -Result $cleanupPlan -Marker 'No service, LSA mutation, or AMD runtime was performed.' -Description 'I2F cleanup plan-only safety output'
$afterCleanupPlan = Get-I2fMachineState
Assert-I2fMachineStateUnchanged -Before $beforeCleanupPlan -After $afterCleanupPlan -Description 'I2F cleanup plan-only entrypoint'
Write-Host 'I2F_CLEANUP_PLAN_ONLY_REAL_ENTRYPOINT=PASS'
Write-Host 'I2F_CLEANUP_PLAN_ONLY_OUTPUT=I2F_CLEANUP_PLAN_ONLY=true'

$cleanupLibraryOnlyResult = Invoke-I2fChildFile -Path $cleanupPath -Arguments @('-LibraryOnly')
if ($cleanupLibraryOnlyResult.exit_code -ne 0) {
    throw "I2F cleanup LibraryOnly entrypoint failed: $($cleanupLibraryOnlyResult.text)"
}
Write-Host 'I2F_CLEANUP_LIBRARY_ONLY_REAL_ENTRYPOINT=PASS'

$libraryPathLiteral = $libraryPath.Replace("'", "''")
$parameterProbe = @"
`$ExecuteAuthorizedExperiment = `$true
`$ExecuteAuthorizedCleanup = `$true
`$LibraryOnly = `$false
. '$libraryPathLiteral'
if (-not `$ExecuteAuthorizedExperiment -or -not `$ExecuteAuthorizedCleanup -or `$LibraryOnly) { throw 'shared library changed caller parameter state' }
Write-Output 'I2F_SETUP_PARAMETER_PRESERVATION=PASS'
Write-Output 'I2F_CLEANUP_PARAMETER_PRESERVATION=PASS'
"@
$parameterResult = Invoke-I2fChildCommand -Command $parameterProbe
Assert-I2fChildMarker -Result $parameterResult -Marker 'I2F_SETUP_PARAMETER_PRESERVATION=PASS' -Description 'I2F setup parameter preservation'
Assert-I2fChildMarker -Result $parameterResult -Marker 'I2F_CLEANUP_PARAMETER_PRESERVATION=PASS' -Description 'I2F cleanup parameter preservation'
Write-Host 'I2F_SETUP_PARAMETER_PRESERVATION=PASS'
Write-Host 'I2F_CLEANUP_PARAMETER_PRESERVATION=PASS'

$beforeSentinel = Get-I2fMachineState
$previousSentinelValue = $env:I2F_OFFLINE_TEST_SENTINEL
$env:I2F_OFFLINE_TEST_SENTINEL = 'true'
try {
    $authorizedSetup = Invoke-I2fChildFile -Path $setupPath -Arguments @(
        '-ExecuteAuthorizedExperiment',
        '-InternalTestOnlyPreMutationSentinel'
    )
    Assert-I2fChildMarker -Result $authorizedSetup -Marker 'I2F_AUTHORIZED_PRE_MUTATION_SENTINEL=true' -Description 'I2F authorized setup sentinel'

    $authorizedCleanup = Invoke-I2fChildFile -Path $cleanupPath -Arguments @(
        '-ExecuteAuthorizedCleanup',
        '-InternalTestOnlyPreMutationSentinel'
    )
    Assert-I2fChildMarker -Result $authorizedCleanup -Marker 'I2F_CLEANUP_AUTHORIZED_PRE_MUTATION_SENTINEL=true' -Description 'I2F authorized cleanup sentinel'
}
finally {
    if ($null -eq $previousSentinelValue) {
        Remove-Item Env:I2F_OFFLINE_TEST_SENTINEL -ErrorAction SilentlyContinue
    } else {
        $env:I2F_OFFLINE_TEST_SENTINEL = $previousSentinelValue
    }
}
$afterSentinel = Get-I2fMachineState
Assert-I2fMachineStateUnchanged -Before $beforeSentinel -After $afterSentinel -Description 'I2F authorized pre-mutation sentinels'
Write-Host 'AUTHORIZED_EXPERIMENT_FLAG_SURVIVES_HELPER_LOAD=PASS'
Write-Host 'AUTHORIZED_CLEANUP_FLAG_SURVIVES_HELPER_LOAD=PASS'
Write-Host 'AUTHORIZED_SETUP_PRE_MUTATION_SENTINEL=PASS'
Write-Host 'AUTHORIZED_CLEANUP_PRE_MUTATION_SENTINEL=PASS'
Write-Host 'I2F_CLEANUP_OFFLINE_AUTHORIZED_SENTINEL=PASS'

$beforeCleanupRerunGuard = Get-I2fMachineState
$beforeHistoricalEvidence = Get-I2fHistoricalEvidenceContentSnapshot
$cleanupRerunGuard = Invoke-I2fChildFile -Path $cleanupPath -Arguments @('-ExecuteAuthorizedCleanup')
Assert-I2fCleanupRerunGuard -Result $cleanupRerunGuard
$afterCleanupRerunGuard = Get-I2fMachineState
$afterHistoricalEvidence = Get-I2fHistoricalEvidenceContentSnapshot
Assert-I2fMachineStateUnchanged -Before $beforeCleanupRerunGuard -After $afterCleanupRerunGuard -Description 'I2F cleanup consumed-gate retirement guard'
if (($beforeHistoricalEvidence | ConvertTo-Json -Depth 10 -Compress) -cne
    ($afterHistoricalEvidence | ConvertTo-Json -Depth 10 -Compress)) {
    throw "I2F authoritative historical evidence content changed.`nBefore=$($beforeHistoricalEvidence | ConvertTo-Json -Depth 10 -Compress)`nAfter=$($afterHistoricalEvidence | ConvertTo-Json -Depth 10 -Compress)"
}
Write-Host 'I2F_CLEANUP_REAL_RERUN_GUARD=PASS'
Write-Host 'I2F_CLEANUP_RERUN_MACHINE_STATE_UNCHANGED=PASS'
Write-Host 'I2F_HISTORICAL_EVIDENCE_CONTENT_UNCHANGED=PASS'
if ($beforeHistoricalEvidence.readable -and $afterHistoricalEvidence.readable) {
    Write-Host 'I2F_HISTORICAL_EVIDENCE_CONTENT_HASH_CHECK=PASS'
} else {
    Write-Host 'I2F_HISTORICAL_EVIDENCE_CONTENT_HASH_CHECK=UNREADABLE_STABLE'
}
