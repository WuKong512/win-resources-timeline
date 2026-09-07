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

    $output = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $Path @Arguments 2>&1 |
        ForEach-Object { [string]$_ })
    [pscustomobject]@{
        exit_code = [int]$LASTEXITCODE
        output = $output
        text = $output -join [Environment]::NewLine
    }
}

function Invoke-I2fChildCommand {
    param([Parameter(Mandatory = $true)][string]$Command)

    $output = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -Command $Command 2>&1 |
        ForEach-Object { [string]$_ })
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
Assert-I2fChildMarker -Result $setupPlan -Marker 'No service, LSA mutation, token adjustment, or AMD runtime was performed.' -Description 'I2F setup plan-only safety output'
$afterSetupPlan = Get-I2fMachineState
Assert-I2fMachineStateUnchanged -Before $beforePlan -After $afterSetupPlan -Description 'I2F setup plan-only entrypoint'
Write-Host 'I2F_PLAN_ONLY_REAL_ENTRYPOINT=PASS'
Write-Host 'I2F_PLAN_ONLY_OUTPUT=I2F_PLAN_ONLY=true'

$cleanupPlan = Invoke-I2fChildFile -Path $cleanupPath -Arguments @()
Assert-I2fChildMarker -Result $cleanupPlan -Marker 'I2F_CLEANUP_PLAN_ONLY=true' -Description 'I2F cleanup plan-only entrypoint'
Assert-I2fChildMarker -Result $cleanupPlan -Marker 'No service, LSA mutation, or AMD runtime was performed.' -Description 'I2F cleanup plan-only safety output'
$afterCleanupPlan = Get-I2fMachineState
Assert-I2fMachineStateUnchanged -Before $afterSetupPlan -After $afterCleanupPlan -Description 'I2F cleanup plan-only entrypoint'
Write-Host 'I2F_CLEANUP_PLAN_ONLY_REAL_ENTRYPOINT=PASS'
Write-Host 'I2F_CLEANUP_PLAN_ONLY_OUTPUT=I2F_CLEANUP_PLAN_ONLY=true'

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
