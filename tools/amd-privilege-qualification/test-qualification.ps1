#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ToolRoot = $PSScriptRoot
$Manifest = Join-Path $ToolRoot 'Cargo.toml'
$TargetRoot = Join-Path $ToolRoot 'target\qualification-synthetic'
$EvidenceRoot = Join-Path $TargetRoot 'evidence'
$Binary = Join-Path $ToolRoot 'target\debug\amd-privilege-qualification.exe'

foreach ($wrapper in @(
        (Join-Path $ToolRoot 'run-admin-amd-privilege-qualification.ps1'),
        (Join-Path $ToolRoot 'run-standard-user-amd-privilege-client.ps1'),
        (Join-Path $ToolRoot 'cleanup-admin-amd-privilege-qualification.ps1')
    )) {
    $parseErrors = $null
    $tokens = $null
    [System.Management.Automation.Language.Parser]::ParseFile($wrapper, [ref]$tokens, [ref]$parseErrors) | Out-Null
    if ($parseErrors.Count -ne 0) {
        throw "PowerShell syntax errors in wrapper: $wrapper"
    }
}

Remove-Item -LiteralPath $EvidenceRoot -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $EvidenceRoot | Out-Null

& cargo fmt --manifest-path $Manifest -- --check
if ($LASTEXITCODE -ne 0) { throw 'cargo fmt --check failed.' }
& cargo test --offline --manifest-path $Manifest -- --nocapture
if ($LASTEXITCODE -ne 0) { throw 'focused qualification tests failed.' }
& cargo build --offline --manifest-path $Manifest
if ($LASTEXITCODE -ne 0) { throw 'synthetic qualification debug build failed.' }
if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) { throw "Synthetic binary is missing: $Binary" }
& $Binary --synthetic --evidence-root $EvidenceRoot
if ($LASTEXITCODE -ne 0) { throw 'synthetic qualification executable returned failure.' }
$summary = Get-Content -LiteralPath (Join-Path $EvidenceRoot 'SYNTHETIC-QUALIFICATION.json') -Raw | ConvertFrom-Json
if ($summary.result -ne 'PASS' -or $summary.amd_runtime_executed -ne $false) {
    throw 'Synthetic qualification summary did not pass without AMD execution.'
}
if ($summary.mutation_assertions.real_amd_runtime_count_during_task -ne 0 -or
    $summary.mutation_assertions.service_registration_count_during_task -ne 0 -or
    $summary.mutation_assertions.scheduled_task_registration_count -ne 0 -or
    $summary.mutation_assertions.self_elevation_performed -ne $false -or
    $summary.mutation_assertions.amd_installation_mutated -ne $false -or
    $summary.mutation_assertions.amd_registry_mutated -ne $false) {
    throw 'Synthetic mutation assertions are not clean.'
}

$serviceName = 'ResourceTimelineAmdPrivilegeQualification'
if (Get-Service -Name $serviceName -ErrorAction SilentlyContinue) {
    throw "Synthetic test refuses to run while the qualification service is registered: $serviceName"
}
Write-Host 'Synthetic qualification PASS. No service registration, AMD runtime, elevation, or AMD installation mutation was performed.'
