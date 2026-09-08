#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ExecuteAuthorizedExperiment,
    [switch]$LibraryOnly,
    [switch]$OfflineSynthetic,
    [ValidateSet(
        'happy', 'negative', 'control-drift', 'pre-control-failure',
        'control-token-gate-failure', 'config-delta-failure', 'token-delta-failure',
        'materialization-failure', 'system-profile-regression', 'process-ownership-failure',
        'control-timeout', 'treatment-timeout', 'cleanup-failure', 'identity-mismatch',
        'spawn-failure', 'exit-nonzero', 'unexpected-preexisting-profile-right',
        'recovery-matrix', 'crash-window-matrix'
    )]
    [string]$OfflineSyntheticScenario = 'happy',
    [string]$EvidenceRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Loading the contract is deliberately the only operation before the real gate.
# It contains constants and pure functions only; it does not query the machine.
. (Join-Path $PSScriptRoot 'i2g-runtime-contract.ps1')

# This guard must remain the first executable branch after the pure contract.  A future
# authorized implementation must be introduced in a separately reviewed change; this
# entrypoint never checks elevation, SCM, LSA, tokens, ACLs, the registry, or AMD files.
if ($ExecuteAuthorizedExperiment) {
    Write-Error 'I2G_REAL_EXECUTION_NOT_AUTHORIZED'
    Write-Error 'I2G_REAL_GATE_CONSUMED=false'
    Write-Error 'I2G_HUMAN_AUTHORIZATION_RECORDED=false'
    exit 1
}

if ($LibraryOnly) {
    Write-Output 'I2G_LIBRARY_ONLY=PASS'
    Write-Output 'I2G_REAL_EXECUTION_ALLOWED=false'
    Write-Output 'I2G_REAL_GATE_CONSUMED=false'
    return
}

if (-not $OfflineSynthetic) {
    (Get-I2gExperimentPlan | ConvertTo-Json -Depth 20)
    Write-Output 'I2G_HARNESS_IMPLEMENTED=true'
    Write-Output 'I2G_HARNESS=IMPLEMENTED_OFFLINE'
    Write-Output "I2G_VARIABLE=$I2gVariable"
    Write-Output "I2G_EXPERIMENT_SHAPE=$I2gExperimentShape"
    Write-Output 'I2G_REAL_EXECUTION_ALLOWED=false'
    Write-Output 'I2G_REAL_GATE_CONSUMED=false'
    Write-Output 'I2G_REAL_CLEANUP_ALLOWED=false'
    Write-Output 'I2G_HUMAN_REAL_RUN_AUTHORIZATION=NOT_GRANTED'
    Write-Output 'I2G_REAL_RUNTIME=0'
    Write-Output 'I2G_POWER_SAMPLING_RUNS=0'
    Write-Output 'No real service, policy, token, AMD, registry, ACL, or production operation was performed.'
    return
}

# The synthetic branch is a fixed semantic intent.  It has no arbitrary executable, argument,
# working-directory, environment, sampling, or live evidence-root surface.
$artifactPath = Join-Path $PSScriptRoot $I2gHarnessArtifactRelativePath
$artifactIdentity = Test-I2gHarnessArtifactIdentity -Path $artifactPath
if (-not $artifactIdentity.pass) {
    throw "I2G_ARTIFACT_IDENTITY_REJECTED reason=$($artifactIdentity.reason) path=$artifactPath"
}

$arguments = @('--i2g-synthetic', '--scenario', $OfflineSyntheticScenario)
if (-not [string]::IsNullOrWhiteSpace($EvidenceRoot)) {
    $arguments += @('--evidence-root', $EvidenceRoot)
}
& $artifactPath @arguments
$childExitCode = [int]$LASTEXITCODE
if ($childExitCode -ne 0) { exit $childExitCode }
