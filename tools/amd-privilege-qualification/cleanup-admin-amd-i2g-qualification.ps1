#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$ExecuteAuthorizedCleanup,
    [switch]$LibraryOnly,
    [switch]$OfflineSynthetic
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Pure constants/functions only.  The real cleanup rejection precedes every host access.
. (Join-Path $PSScriptRoot 'i2g-runtime-contract.ps1')

if ($ExecuteAuthorizedCleanup) {
    Write-Error 'I2G_REAL_CLEANUP_NOT_AUTHORIZED'
    Write-Error 'I2G_REAL_GATE_CONSUMED=true'
    Write-Error 'I2G_HUMAN_REAL_RUN_AUTHORIZATION=CONSUMED'
    exit 1
}

if ($LibraryOnly) {
    Write-Output 'I2G_CLEANUP_LIBRARY_ONLY=PASS'
    Write-Output 'I2G_REAL_CLEANUP_ALLOWED=false'
    Write-Output 'I2G_REAL_GATE_CONSUMED=true'
    return
}

if (-not $OfflineSynthetic) {
    Write-Output 'I2G_CLEANUP_PLAN_ONLY=true'
    Write-Output 'I2G_REAL_CLEANUP_ALLOWED=false'
    Write-Output 'I2G_REAL_GATE_CONSUMED=true'
    Write-Output 'I2G_CLEANUP_ORDER=STOP_CHILD,SERVICE,OWNED_RIGHTS,SERVICE_DELETE,READBACK,EVIDENCE'
    Write-Output 'No real service, policy, token, AMD, registry, ACL, or production cleanup was performed.'
    return
}

$artifactPath = Join-Path $PSScriptRoot $I2gHarnessArtifactRelativePath
$artifactIdentity = Test-I2gHarnessArtifactIdentity -Path $artifactPath
if (-not $artifactIdentity.pass) {
    throw "I2G_ARTIFACT_IDENTITY_REJECTED reason=$($artifactIdentity.reason) path=$artifactPath"
}
& $artifactPath --i2g-synthetic-cleanup
$childExitCode = [int]$LASTEXITCODE
if ($childExitCode -ne 0) { exit $childExitCode }
