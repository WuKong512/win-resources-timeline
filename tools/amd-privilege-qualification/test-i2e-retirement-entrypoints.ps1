#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ToolRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$setupPath = Join-Path $ToolRoot 'run-admin-amd-i2e-service-profile-experiment.ps1'
$resumePath = Join-Path $ToolRoot 'resume-admin-amd-i2e-treatment.ps1'
$cleanupPath = Join-Path $ToolRoot 'cleanup-admin-amd-i2e-service-profile-experiment.ps1'
$contractPath = Join-Path $ToolRoot 'i2e-service-profile-contract.ps1'
$serviceName = 'ResourceTimelineAmdSystemProfileQualification'
$qualificationRoot = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-system-profile'
$authoritativeExperimentId = '3935ac9082954bcfb2b1f94c54cf95d7'

function Invoke-I2eRetirementChild {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Arguments
    )

    $previousErrorActionPreference = $ErrorActionPreference
    try {
        # Forbidden real invocations intentionally return nonzero and write the
        # stable retirement marker through the error stream. Capture it as data.
        $ErrorActionPreference = 'Continue'
        $output = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $Path @Arguments 2>&1 |
            ForEach-Object { [string]$_ })
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    [pscustomobject]@{
        exit_code = [int]$LASTEXITCODE
        output = $output
        text = $output -join [Environment]::NewLine
    }
}

function Assert-I2eRetirementMarker {
    param(
        [Parameter(Mandatory = $true)]$Result,
        [Parameter(Mandatory = $true)][string]$Marker,
        [Parameter(Mandatory = $true)][string]$Description
    )

    if ($Result.text.IndexOf($Marker, [StringComparison]::Ordinal) -lt 0) {
        throw "$Description did not emit '$Marker'. exit=$($Result.exit_code)`n$($Result.text)"
    }
}

function Assert-I2eRetirementUnchanged {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After,
        [Parameter(Mandatory = $true)][string]$Description
    )

    $beforeJson = $Before | ConvertTo-Json -Depth 20 -Compress
    $afterJson = $After | ConvertTo-Json -Depth 20 -Compress
    if ($beforeJson -cne $afterJson) {
        throw "$Description changed read-only state.`nBefore=$beforeJson`nAfter=$afterJson"
    }
}

function Get-I2eRetirementRootInventory {
    if (-not (Test-Path -LiteralPath $qualificationRoot -PathType Container)) {
        return @('ABSENT')
    }
    try {
        $entries = @(
            Get-ChildItem -LiteralPath $qualificationRoot -Force -Recurse -ErrorAction Stop |
                Sort-Object FullName |
                ForEach-Object {
                    $length = if ($_.PSIsContainer) { 0L } else { [int64]$_.Length }
                    '{0}|{1}|{2}|{3}' -f $_.FullName, $_.PSIsContainer, $length, $_.LastWriteTimeUtc.Ticks
                }
        )
        return @('PRESENT') + $entries
    }
    catch {
        # Protected historical evidence is not opened or ACL-bypassed by this
        # test. Stable unreadability is an acceptable read-only observation.
        return @('UNREADABLE_STABLE')
    }
}

function Get-I2eRetirementMachineState {
    $serviceState = $null
    try {
        $service = @(Get-CimInstance -ClassName Win32_Service -Filter "Name='$serviceName'" -ErrorAction Stop |
            Select-Object -First 1)
        $serviceState = if ($service.Count -eq 0) {
            'ABSENT'
        } else {
            '{0}|{1}|{2}|{3}' -f $service[0].State, $service[0].ProcessId, $service[0].StartName, $service[0].PathName
        }
    }
    catch {
        $serviceFallback = @(Get-Service -Name $serviceName -ErrorAction SilentlyContinue)
        $serviceState = if ($serviceFallback.Count -eq 0) { 'ABSENT' } else { 'PRESENT|{0}' -f $serviceFallback[0].Status }
    }

    [pscustomobject]@{
        service = $serviceState
        qualification_root_inventory = @(Get-I2eRetirementRootInventory)
        broker_process_count = @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue).Count
        amd_cli_process_count = @(Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue).Count
    }
}

function Get-I2eHistoricalEvidenceSnapshot {
    if (-not (Test-Path -LiteralPath $qualificationRoot -PathType Container)) {
        return [pscustomobject]@{ status = 'ABSENT'; files = @() }
    }
    try {
        $files = @(
            Get-ChildItem -LiteralPath $qualificationRoot -Force -Recurse -File -ErrorAction Stop |
                Where-Object {
                    $_.Name -in @(
                        'AMD-CLI-PREFLIGHT.json',
                        'CONTROL-RECOVERY.json',
                        'SECURITY-MUTATION-APPLIED.json',
                        'SECURITY-MUTATION-ROLLBACK.json'
                    ) -or $_.Name -like 'I2E-EXPERIMENT-FINAL-*.json' -or
                    $_.Name -eq 'I2E-EXPERIMENT-CURRENT.json'
                } |
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
        [pscustomobject]@{ status = 'READABLE_OR_STABLE'; files = @($files) }
    }
    catch {
        [pscustomobject]@{ status = 'UNREADABLE_STABLE'; files = @() }
    }
}

function Assert-I2eRetirementOrder {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$GuardMarker,
        [Parameter(Mandatory = $true)][string[]]$RequiredAfter,
        [Parameter(Mandatory = $true)][string]$Description
    )

    $guardIndex = $Source.IndexOf($GuardMarker, [StringComparison]::Ordinal)
    if ($guardIndex -lt 0) { throw "$Description is missing the retirement guard marker." }
    foreach ($needle in $RequiredAfter) {
        $index = $Source.IndexOf($needle, [StringComparison]::Ordinal)
        if ($index -lt 0) { throw "$Description is missing executable-order marker: $needle" }
        if ($guardIndex -ge $index) {
            throw "$Description does not place the retirement guard before: $needle"
        }
    }
}

foreach ($path in @($setupPath, $resumePath, $cleanupPath, $contractPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required I2E retirement path is missing: $path" }
}

$contractSource = Get-Content -LiteralPath $contractPath -Raw
foreach ($requiredContract in @(
        '$I2eRealGateConsumed = $true',
        '$I2eRealExperimentAllowed = $false',
        '$I2eTreatmentResumeAllowed = $false',
        '$I2eRealCleanupAllowed = $false',
        '$I2eAuthoritativeExperimentId = ''3935ac9082954bcfb2b1f94c54cf95d7''',
        '$I2eAuthoritativeRollbackComplete = $true'
    )) {
    if ($contractSource.IndexOf($requiredContract, [StringComparison]::Ordinal) -lt 0) {
        throw "I2E retirement contract is missing: $requiredContract"
    }
}

$setupSource = Get-Content -LiteralPath $setupPath -Raw
$resumeSource = Get-Content -LiteralPath $resumePath -Raw
$cleanupSource = Get-Content -LiteralPath $cleanupPath -Raw
Assert-I2eRetirementOrder -Source $setupSource -GuardMarker 'I2E_RERUN_FORBIDDEN' -RequiredAfter @(
    '$null = Assert-I2eAdministrator',
    '$artifactHash = (Get-FileHash',
    '$amdCliPreflight = Get-I2eAmdCliPreflight',
    '$experimentId = [Guid]::NewGuid',
    'Get-I2eServiceSnapshot -ServiceName $ServiceName',
    'Add-I2eExactServiceProfileRight -ServiceSid',
    '$control = Invoke-I2ePhase'
) -Description 'I2E paired retirement guard'
Assert-I2eRetirementOrder -Source $resumeSource -GuardMarker 'I2E_TREATMENT_RESUME_RERUN_FORBIDDEN' -RequiredAfter @(
    '$null = Assert-I2eAdministrator',
    '$pointer = Assert-I2eTreatmentResumePointer',
    '$currentAmdPreflight = Get-I2eAmdCliPreflight',
    'Add-I2eExactServiceProfileRight -ServiceSid',
    '$treatment = Invoke-I2ePhase',
    "Write-I2eJson -Path (Join-Path `$experimentRoot 'TREATMENT-AMD-CLI-PREFLIGHT.json')"
) -Description 'I2E treatment-resume retirement guard'
Assert-I2eRetirementOrder -Source $cleanupSource -GuardMarker 'I2E_CLEANUP_RERUN_FORBIDDEN' -RequiredAfter @(
    '$null = Assert-I2eCleanupAdministrator',
    '$pointer = Read-I2eCleanupJson -Path $PointerPath',
    '$experimentRoot = Join-Path $QualificationRoot $experimentId',
    'Remove-I2eExactServiceProfileRight -ServiceSid',
    'Write-I2eCleanupJson -Path $PointerPath'
) -Description 'I2E cleanup retirement guard'
Write-Host 'I2E_GUARDS_PRECEDE_MACHINE_ACCESS=PASS'

$beforePlan = Get-I2eRetirementMachineState
$pairedPlan = Invoke-I2eRetirementChild -Path $setupPath -Arguments @()
if ($pairedPlan.exit_code -ne 0) { throw "I2E paired plan-only entrypoint failed: $($pairedPlan.text)" }
Assert-I2eRetirementMarker -Result $pairedPlan -Marker 'I2E_PLAN_ONLY=true' -Description 'I2E paired plan-only entrypoint'
Assert-I2eRetirementMarker -Result $pairedPlan -Marker 'I2E_RERUN=FORBIDDEN' -Description 'I2E paired plan retirement state'
Assert-I2eRetirementMarker -Result $pairedPlan -Marker "AUTHORITATIVE_EXPERIMENT=$authoritativeExperimentId" -Description 'I2E paired plan identity'
$afterPlan = Get-I2eRetirementMachineState
Assert-I2eRetirementUnchanged -Before $beforePlan -After $afterPlan -Description 'I2E paired plan-only entrypoint'
Write-Host 'I2E_PAIRED_PLAN_ONLY=PASS'

$beforePairedForbidden = Get-I2eRetirementMachineState
$pairedForbidden = Invoke-I2eRetirementChild -Path $setupPath -Arguments @('-ExecuteAuthorizedExperiment')
if ($pairedForbidden.exit_code -eq 0) { throw 'I2E paired real entrypoint unexpectedly succeeded.' }
Assert-I2eRetirementMarker -Result $pairedForbidden -Marker 'I2E_RERUN_FORBIDDEN' -Description 'I2E paired real retirement guard'
Assert-I2eRetirementMarker -Result $pairedForbidden -Marker $authoritativeExperimentId -Description 'I2E paired retirement scope'
$afterPairedForbidden = Get-I2eRetirementMachineState
Assert-I2eRetirementUnchanged -Before $beforePairedForbidden -After $afterPairedForbidden -Description 'I2E paired real retirement guard'
Write-Host 'I2E_PAIRED_REAL_RERUN_GUARD=PASS'

$beforeResumePlan = Get-I2eRetirementMachineState
$resumePlan = Invoke-I2eRetirementChild -Path $resumePath -Arguments @()
if ($resumePlan.exit_code -ne 0) { throw "I2E treatment-resume plan-only entrypoint failed: $($resumePlan.text)" }
Assert-I2eRetirementMarker -Result $resumePlan -Marker 'I2E_TREATMENT_RESUME_PLAN_ONLY=true' -Description 'I2E treatment-resume plan-only entrypoint'
Assert-I2eRetirementMarker -Result $resumePlan -Marker 'I2E_RERUN=FORBIDDEN' -Description 'I2E treatment-resume plan retirement state'
Assert-I2eRetirementMarker -Result $resumePlan -Marker "AUTHORITATIVE_EXPERIMENT=$authoritativeExperimentId" -Description 'I2E treatment-resume plan identity'
$afterResumePlan = Get-I2eRetirementMachineState
Assert-I2eRetirementUnchanged -Before $beforeResumePlan -After $afterResumePlan -Description 'I2E treatment-resume plan-only entrypoint'
Write-Host 'I2E_TREATMENT_RESUME_PLAN_ONLY=PASS'

$previousResumeSentinel = $env:I2E_RESUME_OFFLINE_TEST_SENTINEL
$env:I2E_RESUME_OFFLINE_TEST_SENTINEL = 'true'
try {
    $beforeResumeSentinel = Get-I2eRetirementMachineState
    $resumeSentinel = Invoke-I2eRetirementChild -Path $resumePath -Arguments @(
        '-ExecuteAuthorizedTreatmentOnly',
        '-InternalTestOnlyPreMutationSentinel'
    )
    if ($resumeSentinel.exit_code -ne 0) { throw "I2E treatment-resume offline sentinel failed: $($resumeSentinel.text)" }
    Assert-I2eRetirementMarker -Result $resumeSentinel -Marker 'I2E_TREATMENT_RESUME_AUTHORIZED_PRE_MUTATION_SENTINEL=true' -Description 'I2E treatment-resume offline sentinel'
    $afterResumeSentinel = Get-I2eRetirementMachineState
    Assert-I2eRetirementUnchanged -Before $beforeResumeSentinel -After $afterResumeSentinel -Description 'I2E treatment-resume offline sentinel'
}
finally {
    if ($null -eq $previousResumeSentinel) { Remove-Item Env:I2E_RESUME_OFFLINE_TEST_SENTINEL -ErrorAction SilentlyContinue }
    else { $env:I2E_RESUME_OFFLINE_TEST_SENTINEL = $previousResumeSentinel }
}
Write-Host 'I2E_TREATMENT_RESUME_OFFLINE_SENTINEL=PASS'

$beforeResumeForbidden = Get-I2eRetirementMachineState
$beforeResumeEvidence = Get-I2eHistoricalEvidenceSnapshot
$resumeForbidden = Invoke-I2eRetirementChild -Path $resumePath -Arguments @('-ExecuteAuthorizedTreatmentOnly')
if ($resumeForbidden.exit_code -eq 0) { throw 'I2E treatment-resume real entrypoint unexpectedly succeeded.' }
Assert-I2eRetirementMarker -Result $resumeForbidden -Marker 'I2E_TREATMENT_RESUME_RERUN_FORBIDDEN' -Description 'I2E treatment-resume real retirement guard'
Assert-I2eRetirementMarker -Result $resumeForbidden -Marker $authoritativeExperimentId -Description 'I2E treatment-resume retirement scope'
$afterResumeForbidden = Get-I2eRetirementMachineState
$afterResumeEvidence = Get-I2eHistoricalEvidenceSnapshot
Assert-I2eRetirementUnchanged -Before $beforeResumeForbidden -After $afterResumeForbidden -Description 'I2E treatment-resume real retirement guard'
Assert-I2eRetirementUnchanged -Before $beforeResumeEvidence -After $afterResumeEvidence -Description 'I2E treatment-resume historical evidence'
Write-Host 'I2E_TREATMENT_RESUME_REAL_RERUN_GUARD=PASS'

$beforeCleanupPlan = Get-I2eRetirementMachineState
$cleanupPlan = Invoke-I2eRetirementChild -Path $cleanupPath -Arguments @()
if ($cleanupPlan.exit_code -ne 0) { throw "I2E cleanup plan-only entrypoint failed: $($cleanupPlan.text)" }
Assert-I2eRetirementMarker -Result $cleanupPlan -Marker 'I2E_CLEANUP_PLAN_ONLY=true' -Description 'I2E cleanup plan-only entrypoint'
Assert-I2eRetirementMarker -Result $cleanupPlan -Marker 'I2E_RERUN=FORBIDDEN' -Description 'I2E cleanup plan retirement state'
Assert-I2eRetirementMarker -Result $cleanupPlan -Marker "AUTHORITATIVE_EXPERIMENT=$authoritativeExperimentId" -Description 'I2E cleanup plan identity'
$afterCleanupPlan = Get-I2eRetirementMachineState
Assert-I2eRetirementUnchanged -Before $beforeCleanupPlan -After $afterCleanupPlan -Description 'I2E cleanup plan-only entrypoint'
Write-Host 'I2E_CLEANUP_PLAN_ONLY=PASS'

$cleanupLibrary = Invoke-I2eRetirementChild -Path $cleanupPath -Arguments @('-LibraryOnly')
if ($cleanupLibrary.exit_code -ne 0) { throw "I2E cleanup LibraryOnly entrypoint failed: $($cleanupLibrary.text)" }
Write-Host 'I2E_CLEANUP_LIBRARY_ONLY=PASS'

$previousCleanupSentinel = $env:I2E_CLEANUP_OFFLINE_TEST_SENTINEL
$env:I2E_CLEANUP_OFFLINE_TEST_SENTINEL = 'true'
try {
    $beforeCleanupSentinel = Get-I2eRetirementMachineState
    $cleanupSentinel = Invoke-I2eRetirementChild -Path $cleanupPath -Arguments @(
        '-ExecuteAuthorizedCleanup',
        '-InternalTestOnlyPreMutationSentinel'
    )
    if ($cleanupSentinel.exit_code -ne 0) { throw "I2E cleanup offline sentinel failed: $($cleanupSentinel.text)" }
    Assert-I2eRetirementMarker -Result $cleanupSentinel -Marker 'I2E_CLEANUP_AUTHORIZED_PRE_MUTATION_SENTINEL=true' -Description 'I2E cleanup offline sentinel'
    $afterCleanupSentinel = Get-I2eRetirementMachineState
    Assert-I2eRetirementUnchanged -Before $beforeCleanupSentinel -After $afterCleanupSentinel -Description 'I2E cleanup offline sentinel'
}
finally {
    if ($null -eq $previousCleanupSentinel) { Remove-Item Env:I2E_CLEANUP_OFFLINE_TEST_SENTINEL -ErrorAction SilentlyContinue }
    else { $env:I2E_CLEANUP_OFFLINE_TEST_SENTINEL = $previousCleanupSentinel }
}
Write-Host 'I2E_CLEANUP_OFFLINE_SENTINEL=PASS'

$beforeCleanupForbidden = Get-I2eRetirementMachineState
$beforeCleanupEvidence = Get-I2eHistoricalEvidenceSnapshot
$cleanupForbidden = Invoke-I2eRetirementChild -Path $cleanupPath -Arguments @('-ExecuteAuthorizedCleanup')
if ($cleanupForbidden.exit_code -eq 0) { throw 'I2E cleanup real entrypoint unexpectedly succeeded.' }
Assert-I2eRetirementMarker -Result $cleanupForbidden -Marker 'I2E_CLEANUP_RERUN_FORBIDDEN' -Description 'I2E cleanup real retirement guard'
Assert-I2eRetirementMarker -Result $cleanupForbidden -Marker $authoritativeExperimentId -Description 'I2E cleanup retirement scope'
$afterCleanupForbidden = Get-I2eRetirementMachineState
$afterCleanupEvidence = Get-I2eHistoricalEvidenceSnapshot
Assert-I2eRetirementUnchanged -Before $beforeCleanupForbidden -After $afterCleanupForbidden -Description 'I2E cleanup real retirement guard'
Assert-I2eRetirementUnchanged -Before $beforeCleanupEvidence -After $afterCleanupEvidence -Description 'I2E cleanup historical evidence'
Write-Host 'I2E_CLEANUP_REAL_RERUN_GUARD=PASS'

$evidenceBeforeJson = $beforeCleanupEvidence | ConvertTo-Json -Depth 20 -Compress
$evidenceAfterJson = $afterCleanupEvidence | ConvertTo-Json -Depth 20 -Compress
if ($evidenceBeforeJson -cne $evidenceAfterJson) { throw 'I2E historical evidence content changed during retirement tests.' }
if ($beforeCleanupEvidence.status -eq 'UNREADABLE_STABLE' -or
    @($beforeCleanupEvidence.files | Where-Object { $_ -like '*UNREADABLE_STABLE' }).Count -gt 0) {
    Write-Host 'I2E_HISTORICAL_EVIDENCE_CONTENT_UNCHANGED=PASS_WITH_UNREADABLE_STABLE'
} else {
    Write-Host 'I2E_HISTORICAL_EVIDENCE_CONTENT_UNCHANGED=PASS'
}
Write-Host 'I2E_RERUN_MACHINE_STATE_UNCHANGED=PASS'
Write-Host 'I2E_RETIREMENT_ENTRYPOINTS=PASS'
