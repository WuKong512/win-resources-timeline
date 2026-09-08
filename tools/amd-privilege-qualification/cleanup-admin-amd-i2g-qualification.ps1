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
    Write-Error 'I2G_REAL_GATE_CONSUMED=false'
    Write-Error 'I2G_HUMAN_AUTHORIZATION_RECORDED=false'
    exit 1
}

if ($LibraryOnly) {
    Write-Output 'I2G_CLEANUP_LIBRARY_ONLY=PASS'
    Write-Output 'I2G_REAL_CLEANUP_ALLOWED=false'
    Write-Output 'I2G_REAL_GATE_CONSUMED=false'
    return
}

if (-not $OfflineSynthetic) {
    Write-Output 'I2G_CLEANUP_PLAN_ONLY=true'
    Write-Output 'I2G_REAL_CLEANUP_ALLOWED=false'
    Write-Output 'I2G_REAL_GATE_CONSUMED=false'
    Write-Output 'I2G_CLEANUP_ORDER=STOP_CHILD,SERVICE,OWNED_RIGHTS,SERVICE_DELETE,READBACK,EVIDENCE'
    Write-Output 'No real service, policy, token, AMD, registry, ACL, or production cleanup was performed.'
    return
}

$artifactPath = Join-Path $PSScriptRoot 'target\release\amd-privilege-qualification.exe'
if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
    throw "Missing offline qualification artifact: $artifactPath"
}
& $artifactPath --i2g-synthetic-cleanup
$childExitCode = [int]$LASTEXITCODE
if ($childExitCode -ne 0) { exit $childExitCode }
