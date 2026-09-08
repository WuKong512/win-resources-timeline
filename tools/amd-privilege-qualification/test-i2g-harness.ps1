#requires -Version 5.1
[CmdletBinding()]
param(
    [string]$ToolRoot = $PSScriptRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$contractPath = Join-Path $ToolRoot 'i2g-runtime-contract.ps1'
$setupPath = Join-Path $ToolRoot 'run-admin-amd-i2g-qualification.ps1'
$cleanupPath = Join-Path $ToolRoot 'cleanup-admin-amd-i2g-qualification.ps1'
$manifestPath = Join-Path $ToolRoot 'Cargo.toml'
$releaseBinary = Join-Path $ToolRoot 'target\release\amd-privilege-qualification.exe'
$testRoot = Join-Path $ToolRoot ('target\qualification-synthetic\i2g-' + [Guid]::NewGuid().ToString('N'))

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (-not $Condition) { throw $Message }
}

function Assert-PowerShellSyntax {
    param([Parameter(Mandatory = $true)][string]$Path)
    $parseErrors = $null
    $tokens = $null
    [System.Management.Automation.Language.Parser]::ParseFile(
        $Path, [ref]$tokens, [ref]$parseErrors) | Out-Null
    Assert-True ($parseErrors.Count -eq 0) "PowerShell syntax errors in $Path"
}

function Invoke-I2gChild {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [AllowEmptyCollection()][string[]]$Arguments = @()
    )

    $previousErrorActionPreference = $ErrorActionPreference
    try {
        # Rejected real invocations are expected to exit nonzero.  Capture the error
        # stream as data so the parent can assert the stable marker.
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

function Invoke-I2gScenario {
    param(
        [Parameter(Mandatory = $true)][string]$Scenario,
        [Parameter(Mandatory = $true)][string]$EvidencePath
    )
    $result = Invoke-I2gChild -Path $setupPath -Arguments @(
        '-OfflineSynthetic', '-OfflineSyntheticScenario', $Scenario,
        '-EvidenceRoot', $EvidencePath
    )
    Assert-True ($result.exit_code -eq 0) "I2G scenario '$Scenario' failed: $($result.text)"
    try {
        $summary = $result.text | ConvertFrom-Json
    } catch {
        throw "I2G scenario '$Scenario' did not return JSON: $($result.text)"
    }
    [pscustomobject]@{ process = $result; summary = $summary }
}

foreach ($path in @($contractPath, $setupPath, $cleanupPath, $PSCommandPath)) {
    Assert-PowerShellSyntax -Path $path
}

. $contractPath
Assert-True $I2gHarnessImplemented 'I2G harness implementation marker is not true.'
Assert-True $I2gQualificationOnly 'I2G harness must remain qualification-only.'
Assert-True (-not $I2gRealGateConsumed) 'I2G gate must not be consumed.'
Assert-True (-not $I2gRealExecutionAllowed) 'I2G real execution must be forbidden.'
Assert-True (-not $I2gRealCleanupAllowed) 'I2G real cleanup must be forbidden.'
Assert-True (-not $I2gHumanAuthorizationRecorded) 'Human real-run authorization must be absent.'
Assert-True ($I2gVariable -ceq 'SeProfileSingleProcessPrivilege') 'I2G variable changed.'
Assert-True ($I2gExperimentShape -ceq 'PAIRED_CONTROL_TREATMENT') 'I2G experiment shape changed.'
Assert-True (Test-I2gFixedCliArguments -Arguments $I2gFixedArguments) 'Fixed CLI arguments are not stable.'
Assert-True (-not (Test-I2gFixedCliArguments -Arguments @('timechart', '--list', '--extra'))) 'Arbitrary CLI argument accepted.'
Assert-True ((Get-I2gRequiredEvidenceNames).Count -ge 19) 'Required evidence inventory is incomplete.'
Write-Host 'I2G_PURE_CONTRACT=PASS'

# The marker is intentionally before every live-operation token in both entrypoints.
# This is a source-order regression check in addition to the child-process rejection test.
$setupSource = Get-Content -LiteralPath $setupPath -Raw
$cleanupSource = Get-Content -LiteralPath $cleanupPath -Raw
function Assert-RealGuardOrdering {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Marker,
        [Parameter(Mandatory = $true)][string]$Description
    )
    $markerIndex = $Source.IndexOf($Marker, [StringComparison]::Ordinal)
    Assert-True ($markerIndex -ge 0) "$Description is missing its fail-closed marker."
    foreach ($token in @(
            'Test-Path', 'Get-FileHash', 'Get-Service', 'Get-Process', 'Start-Process',
            'New-Item', 'Get-CimInstance', 'ProgramData', 'sc.exe', 'LsaAddAccountRights',
            'LsaRemoveAccountRights', 'AdjustTokenPrivileges', '[Guid]::NewGuid'
        )) {
        $index = $Source.IndexOf($token, [StringComparison]::OrdinalIgnoreCase)
        if ($index -ge 0 -and $index -lt $markerIndex) {
            throw "$Description performs '$token' before the real gate."
        }
    }
}
Assert-RealGuardOrdering -Source $setupSource -Marker 'I2G_REAL_EXECUTION_NOT_AUTHORIZED' -Description 'I2G setup'
Assert-RealGuardOrdering -Source $cleanupSource -Marker 'I2G_REAL_CLEANUP_NOT_AUTHORIZED' -Description 'I2G cleanup'
Write-Host 'I2G_REAL_GATE_SOURCE_ORDER=PASS'

$plan = Invoke-I2gChild -Path $setupPath
Assert-True ($plan.exit_code -eq 0) "I2G plan-only entrypoint failed: $($plan.text)"
Assert-True ($plan.text.Contains('I2G_HARNESS=IMPLEMENTED_OFFLINE')) 'I2G plan marker is missing.'
Assert-True ($plan.text.Contains('I2G_REAL_EXECUTION_ALLOWED=false')) 'I2G plan real gate is not false.'
Assert-True ($plan.text.Contains('I2G_REAL_CLEANUP_ALLOWED=false')) 'I2G plan cleanup gate is not false.'
Assert-True ($plan.text.Contains('I2G_HUMAN_REAL_RUN_AUTHORIZATION=NOT_GRANTED')) 'I2G authorization marker is missing.'
Write-Host 'I2G_PLAN_ONLY=PASS'

$library = Invoke-I2gChild -Path $setupPath -Arguments @('-LibraryOnly')
Assert-True ($library.exit_code -eq 0 -and $library.text.Contains('I2G_LIBRARY_ONLY=PASS')) 'I2G LibraryOnly failed.'
Write-Host 'I2G_LIBRARY_ONLY=PASS'

$cleanupPlan = Invoke-I2gChild -Path $cleanupPath
Assert-True ($cleanupPlan.exit_code -eq 0 -and $cleanupPlan.text.Contains('I2G_CLEANUP_PLAN_ONLY=true')) 'I2G cleanup plan failed.'
$cleanupLibrary = Invoke-I2gChild -Path $cleanupPath -Arguments @('-LibraryOnly')
Assert-True ($cleanupLibrary.exit_code -eq 0 -and $cleanupLibrary.text.Contains('I2G_CLEANUP_LIBRARY_ONLY=PASS')) 'I2G cleanup LibraryOnly failed.'
Write-Host 'I2G_CLEANUP_PLAN_AND_LIBRARY_ONLY=PASS'

$realEvidenceSentinel = Join-Path $testRoot 'real-entrypoint-must-not-create-this'
$real = Invoke-I2gChild -Path $setupPath -Arguments @(
    '-ExecuteAuthorizedExperiment', '-EvidenceRoot', $realEvidenceSentinel
)
Assert-True ($real.exit_code -ne 0) 'I2G real entrypoint unexpectedly succeeded.'
Assert-True ($real.text.Contains('I2G_REAL_EXECUTION_NOT_AUTHORIZED')) 'I2G real rejection marker is missing.'
Assert-True (-not (Test-Path -LiteralPath $realEvidenceSentinel)) 'I2G real entrypoint created evidence before rejection.'
$realCleanup = Invoke-I2gChild -Path $cleanupPath -Arguments @('-ExecuteAuthorizedCleanup')
Assert-True ($realCleanup.exit_code -ne 0) 'I2G real cleanup unexpectedly succeeded.'
Assert-True ($realCleanup.text.Contains('I2G_REAL_CLEANUP_NOT_AUTHORIZED')) 'I2G cleanup rejection marker is missing.'
Write-Host 'I2G_REAL_EXECUTION_AND_CLEANUP_REJECTED=PASS'

Assert-True (Test-Path -LiteralPath $releaseBinary -PathType Leaf) "Build the release qualification artifact before running I2G tests: $releaseBinary"
New-Item -ItemType Directory -Force -Path $testRoot | Out-Null

$scenarioCases = @(
    [pscustomobject]@{ name = 'happy'; expected = 'PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT'; control = 1; treatment = 1; total = 2; cleanup = 'PASS'; treatment_allowed = $true },
    [pscustomobject]@{ name = 'negative'; expected = 'PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT'; control = 1; treatment = 1; total = 2; cleanup = 'PASS'; treatment_allowed = $true },
    [pscustomobject]@{ name = 'control-drift'; expected = 'CONTROL_DRIFT'; control = 1; treatment = 0; total = 1; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'pre-control-failure'; expected = 'TOKEN_GATE_FAILED'; control = 0; treatment = 0; total = 0; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'control-token-gate-failure'; expected = 'TOKEN_GATE_FAILED'; control = 0; treatment = 0; total = 0; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'config-delta-failure'; expected = 'INVALID_CONFIGURATION_DELTA'; control = 1; treatment = 0; total = 1; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'token-delta-failure'; expected = 'INVALID_TOKEN_DELTA'; control = 1; treatment = 0; total = 1; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'materialization-failure'; expected = 'TOKEN_GATE_FAILED'; control = 1; treatment = 0; total = 1; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'system-profile-regression'; expected = 'TOKEN_GATE_FAILED'; control = 1; treatment = 0; total = 1; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'process-ownership-failure'; expected = 'PROCESS_OWNERSHIP_FAILED'; control = 1; treatment = 0; total = 1; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'control-timeout'; expected = 'TIMEOUT'; control = 1; treatment = 0; total = 1; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'treatment-timeout'; expected = 'TIMEOUT'; control = 1; treatment = 1; total = 2; cleanup = 'PASS'; treatment_allowed = $true },
    [pscustomobject]@{ name = 'cleanup-failure'; expected = 'CLEANUP_FAILED'; control = 1; treatment = 1; total = 2; cleanup = 'FAILED'; treatment_allowed = $true },
    [pscustomobject]@{ name = 'identity-mismatch'; expected = 'IDENTITY_MISMATCH'; control = 0; treatment = 0; total = 0; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'spawn-failure'; expected = 'DISCOVERY_FAILED'; control = 0; treatment = 0; total = 0; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'exit-nonzero'; expected = 'DISCOVERY_FAILED'; control = 1; treatment = 0; total = 1; cleanup = 'PASS'; treatment_allowed = $false },
    [pscustomobject]@{ name = 'unexpected-preexisting-profile-right'; expected = 'INVALID_NO_CAUSAL_INTERPRETATION'; control = 0; treatment = 0; total = 0; cleanup = 'PASS'; treatment_allowed = $false }
)

try {
    foreach ($case in $scenarioCases) {
        $scenarioRoot = Join-Path $testRoot $case.name
        $run = Invoke-I2gScenario -Scenario $case.name -EvidencePath $scenarioRoot
        $summary = $run.summary
        Assert-True ([string]$summary.normalized_result -ceq $case.expected) "$($case.name): unexpected result $($summary.normalized_result)"
        Assert-True ([int]$summary.actual_control_counter_discovery_runs -eq $case.control) "$($case.name): control count mismatch"
        Assert-True ([int]$summary.actual_treatment_counter_discovery_runs -eq $case.treatment) "$($case.name): treatment count mismatch"
        Assert-True ([int]$summary.actual_total_counter_discovery_runs -eq $case.total) "$($case.name): total count mismatch"
        Assert-True ([string]$summary.cleanup_result -ceq $case.cleanup) "$($case.name): cleanup result mismatch"
        Assert-True ([bool]$summary.treatment_allowed -eq $case.treatment_allowed) "$($case.name): treatment gate mismatch"
        Assert-True ([bool]$summary.control_retry_allowed -eq $false -and [bool]$summary.treatment_retry_allowed -eq $false) "$($case.name): retry was allowed"
        Assert-True ([int]$summary.power_sampling_runs -eq 0) "$($case.name): sampling was not zero"
        Assert-True ([int]$summary.mutation_assertions.real_amd_counter_discovery -eq 0) "$($case.name): real AMD runtime was nonzero"
        Assert-True ([int]$summary.mutation_assertions.real_lsa_right_add -eq 0) "$($case.name): real LSA add was nonzero"
        Assert-True ([int]$summary.mutation_assertions.real_lsa_right_remove -eq 0) "$($case.name): real LSA remove was nonzero"
        Assert-True ([int]$summary.mutation_assertions.real_adjust_token_privileges -eq 0) "$($case.name): real token adjustment was nonzero"
        Assert-True ([string]$summary.service_account_sid -ceq 'S-1-5-19') "$($case.name): account SID changed"
        Assert-True ([string]$summary.service_sid -like 'S-1-5-80-*') "$($case.name): service SID is not a Service SID"
        Write-Host ("I2G_SCENARIO_{0}=PASS result={1} counts={2}/{3}/{4} cleanup={5}" -f
            $case.name.ToUpperInvariant().Replace('-', '_'), $summary.normalized_result,
            $summary.actual_control_counter_discovery_runs,
            $summary.actual_treatment_counter_discovery_runs,
            $summary.actual_total_counter_discovery_runs,
            $summary.cleanup_result)
    }

    $happyRoot = Join-Path $testRoot 'happy'
    foreach ($name in $I2gRequiredEvidenceFiles) {
        Assert-True (Test-Path -LiteralPath (Join-Path $happyRoot $name) -PathType Leaf) "Happy evidence is missing: $name"
    }
    $temporaryEvidence = @(Get-ChildItem -LiteralPath $happyRoot -File -Filter '*.tmp' -ErrorAction SilentlyContinue)
    Assert-True ($temporaryEvidence.Count -eq 0) 'Atomic evidence left a temporary file behind.'
    $happySummary = Get-Content -LiteralPath (Join-Path $happyRoot 'FINAL-SUMMARY.json') -Raw | ConvertFrom-Json
    Assert-True (@($happySummary.evidence_files | Where-Object { $_ -eq 'PAIRED-RESULT.json' }).Count -eq 1) 'Final summary omitted paired-result evidence.'
    Assert-True (@($happySummary.evidence_files | Where-Object { $_ -eq 'FINAL-SUMMARY.json' }).Count -eq 1) 'Final summary omitted itself from evidence inventory.'
    $paired = Get-Content -LiteralPath (Join-Path $happyRoot 'PAIRED-RESULT.json') -Raw | ConvertFrom-Json
    Assert-True ([string]$paired.experiment_result -ceq 'PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT') 'Paired result lost the scientific result.'
    Assert-True ([string]$paired.normalized_result -ceq 'PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT') 'Paired result was not normalized on clean rollback.'
    Write-Host 'I2G_ATOMIC_EVIDENCE_AND_FINAL_INVENTORY=PASS'

    $recovery = Invoke-I2gScenario -Scenario 'recovery-matrix' -EvidencePath (Join-Path $testRoot 'recovery-matrix')
    Assert-True ([string]$recovery.summary.offline_validation -ceq 'PASS') 'Recovery matrix failed.'
    Assert-True ([int]$recovery.summary.actual_total_counter_discovery_runs -eq 0) 'Recovery matrix launched discovery.'
    Write-Host 'I2G_RECOVERY_NO_RETRY_MATRIX=PASS'

    $cleanupOffline = Invoke-I2gChild -Path $cleanupPath -Arguments @('-OfflineSynthetic')
    Assert-True ($cleanupOffline.exit_code -eq 0) "I2G offline cleanup failed: $($cleanupOffline.text)"
    $cleanupSummary = $cleanupOffline.text | ConvertFrom-Json
    Assert-True ([string]$cleanupSummary.result -ceq 'PASS') 'I2G offline cleanup result was not PASS.'
    Assert-True (-not [bool]$cleanupSummary.real_cleanup_allowed) 'I2G offline cleanup exposed real cleanup.'
    Write-Host 'I2G_OFFLINE_CLEANUP_CONTRACT=PASS'
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}

Write-Host 'I2G_OFFLINE_HARNESS=PASS'
