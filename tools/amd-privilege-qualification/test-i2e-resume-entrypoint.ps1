#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ToolRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$resumePath = Join-Path $ToolRoot 'resume-admin-amd-i2e-treatment.ps1'
$libraryPath = Join-Path $ToolRoot 'i2e-runtime-library.ps1'
$i2eSetupPath = Join-Path $ToolRoot 'run-admin-amd-i2e-service-profile-experiment.ps1'
$i2fSetupPath = Join-Path $ToolRoot 'run-admin-amd-i2f-service-profile-experiment.ps1'
$i2fCleanupPath = Join-Path $ToolRoot 'cleanup-admin-amd-i2f-service-profile-experiment.ps1'
$serviceName = 'ResourceTimelineAmdSystemProfileQualification'
$qualificationRoot = Join-Path $env:ProgramData 'ResourceTimeline\qualification\amd-system-profile'
$expectedArtifactSha256 = '871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9'

function Invoke-I2eResumeChild {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Arguments)

    $output = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $resumePath @Arguments 2>&1 |
        ForEach-Object { [string]$_ })
    [pscustomobject]@{
        exit_code = [int]$LASTEXITCODE
        output = $output
        text = $output -join [Environment]::NewLine
    }
}

function Get-I2eResumeMachineState {
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
        $service = @(Get-Service -Name $serviceName -ErrorAction SilentlyContinue)
        $serviceState = if ($service.Count -eq 0) { 'ABSENT' } else { 'PRESENT|{0}' -f $service[0].Status }
    }

    $rootEntries = @()
    $rootExists = Test-Path -LiteralPath $qualificationRoot -PathType Container
    if ($rootExists) {
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
            # Plan-only validation may run without access to protected
            # historical evidence. Treat that as a stable read-only snapshot
            # state rather than probing or changing the directory.
            $rootEntries = @('UNREADABLE')
        }
    }

    [pscustomobject]@{
        service = $serviceState
        root_exists = $rootExists
        root_entries = @($rootEntries)
        broker_process_count = @(Get-Process -Name 'amd-privilege-qualification' -ErrorAction SilentlyContinue).Count
        amd_cli_process_count = @(Get-Process -Name 'AMDuProfCLI' -ErrorAction SilentlyContinue).Count
    }
}

function Assert-I2eResumeStateUnchanged {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After,
        [Parameter(Mandatory = $true)][string]$Description
    )

    $beforeJson = $Before | ConvertTo-Json -Depth 10 -Compress
    $afterJson = $After | ConvertTo-Json -Depth 10 -Compress
    if ($beforeJson -cne $afterJson) {
        throw "$Description changed machine state.`nBefore=$beforeJson`nAfter=$afterJson"
    }
}

foreach ($path in @($resumePath, $libraryPath, $i2eSetupPath, $i2fSetupPath, $i2fCleanupPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required I2E/I2F path is missing: $path" }
}

$resumeSource = Get-Content -LiteralPath $resumePath -Raw
if ($resumeSource -match '(?i)run-admin-amd-i2e-service-profile-experiment\.ps1') {
    throw 'I2E treatment resume must not dot-source the executable I2E wrapper.'
}
$librarySource = Get-Content -LiteralPath $libraryPath -Raw
foreach ($name in @('ServiceName', 'ServiceAccount', 'ServiceSidAccount', 'ArtifactPath',
        'ExpectedArtifactSha256', 'QualificationRoot', 'ConfigPath', 'PointerPath')) {
    if ($librarySource -match ('(?m)^\s*\${0}\s*=' -f [regex]::Escape($name))) {
        throw "Shared runtime library owns executable-wrapper state: `$${name}"
    }
}

$resumeErrors = $null
$resumeTokens = $null
$resumeAst = [System.Management.Automation.Language.Parser]::ParseFile(
    $resumePath, [ref]$resumeTokens, [ref]$resumeErrors)
if ($resumeErrors.Count -ne 0) { throw 'I2E treatment resume has PowerShell parse errors.' }
$functionNodes = @($resumeAst.FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.FunctionDefinitionAst]
    }, $true))
$firstFunctionOffset = if ($functionNodes.Count -eq 0) {
    [int]::MaxValue
} else {
    ($functionNodes | ForEach-Object { $_.Extent.StartOffset } | Measure-Object -Minimum).Minimum
}
foreach ($name in @('ServiceName', 'ServiceAccount', 'ServiceSidAccount', 'ArtifactPath',
        'ExpectedArtifactSha256', 'QualificationRoot', 'ConfigPath', 'PointerPath')) {
    $assignment = @($resumeAst.FindAll({
            param($node)
            $node -is [System.Management.Automation.Language.AssignmentStatementAst] -and
                $node.Left -is [System.Management.Automation.Language.VariableExpressionAst] -and
                $node.Left.VariablePath.UserPath -ieq $name -and
                $node.Extent.StartOffset -lt $firstFunctionOffset
        }, $true))
    if ($assignment.Count -eq 0) { throw "Resume wrapper does not initialize `$${name} before its functions." }
}
Write-Host 'I2E_RESUME_WRAPPER_GLOBALS_EXPLICIT=PASS'
Write-Host 'I2E_RESUME_DOTSOURCE_EXECUTABLE_I2E_WRAPPER=ABSENT'
Write-Host 'SHARED_RUNTIME_LIBRARY_WRAPPER_GLOBALS=ABSENT'

$beforePlan = Get-I2eResumeMachineState
$plan = Invoke-I2eResumeChild -Arguments @()
if ($plan.exit_code -ne 0 -or
    $plan.text.IndexOf('I2E_TREATMENT_RESUME_PLAN_ONLY=true', [StringComparison]::Ordinal) -lt 0 -or
    $plan.text.IndexOf('No service, LSA mutation, or AMD runtime was performed.', [StringComparison]::Ordinal) -lt 0) {
    throw "I2E treatment-resume plan-only entrypoint failed: exit=$($plan.exit_code)`n$($plan.text)"
}
$markerIndex = $plan.text.IndexOf('I2E_TREATMENT_RESUME_PLAN_ONLY=true', [StringComparison]::Ordinal)
$planJson = $plan.text.Substring(0, $markerIndex).Trim()
$planObject = $planJson | ConvertFrom-Json
if ([string]$planObject.service_name -cne $serviceName -or
    [string]$planObject.artifact_sha256 -cne $expectedArtifactSha256) {
    throw 'I2E treatment-resume plan does not expose the authoritative service/artifact identity.'
}
$afterPlan = Get-I2eResumeMachineState
Assert-I2eResumeStateUnchanged -Before $beforePlan -After $afterPlan -Description 'I2E treatment-resume plan-only entrypoint'
Write-Host 'I2E_RESUME_PLAN_ONLY_REAL_ENTRYPOINT=PASS'
Write-Host 'I2E_RESUME_PLAN_ONLY_OUTPUT=I2E_TREATMENT_RESUME_PLAN_ONLY=true'
Write-Host 'I2E_RESUME_PLAN_ONLY_MACHINE_STATE=UNCHANGED'

$previousSentinelValue = $env:I2E_RESUME_OFFLINE_TEST_SENTINEL
$beforeSentinel = Get-I2eResumeMachineState
$env:I2E_RESUME_OFFLINE_TEST_SENTINEL = 'true'
try {
    $sentinel = Invoke-I2eResumeChild -Arguments @(
        '-ExecuteAuthorizedTreatmentOnly',
        '-InternalTestOnlyPreMutationSentinel'
    )
    if ($sentinel.exit_code -ne 0 -or
        $sentinel.text.IndexOf('I2E_TREATMENT_RESUME_AUTHORIZED_PRE_MUTATION_SENTINEL=true', [StringComparison]::Ordinal) -lt 0) {
        throw "I2E treatment-resume authorized sentinel failed: exit=$($sentinel.exit_code)`n$($sentinel.text)"
    }
} finally {
    if ($null -eq $previousSentinelValue) {
        Remove-Item Env:I2E_RESUME_OFFLINE_TEST_SENTINEL -ErrorAction SilentlyContinue
    } else {
        $env:I2E_RESUME_OFFLINE_TEST_SENTINEL = $previousSentinelValue
    }
}
$afterSentinel = Get-I2eResumeMachineState
Assert-I2eResumeStateUnchanged -Before $beforeSentinel -After $afterSentinel -Description 'I2E treatment-resume authorized sentinel'
Write-Host 'I2E_RESUME_AUTHORIZED_FLAG_PRESERVATION=PASS'
Write-Host 'I2E_RESUME_AUTHORIZED_PRE_MUTATION_SENTINEL=PASS'

$callerAudit = @(
    @{ Path = $i2eSetupPath; Required = @('$ServiceName', '$ServiceAccount', '$ArtifactPath', '$QualificationRoot', '$ConfigPath') },
    @{ Path = $resumePath; Required = @('$ServiceName', '$ServiceAccount', '$ServiceSidAccount', '$ArtifactPath', '$ExpectedArtifactSha256', '$QualificationRoot', '$ConfigPath', '$PointerPath') },
    @{ Path = $i2fSetupPath; Required = @('$ServiceName', '$ArtifactPath', '$QualificationRoot', '$ConfigPath') },
    @{ Path = $i2fCleanupPath; Required = @('$ServiceName', '$ArtifactPath', '$QualificationRoot', '$ConfigPath') }
)
foreach ($caller in $callerAudit) {
    $source = Get-Content -LiteralPath $caller.Path -Raw
    if ($source -notmatch '(?i)i2e-runtime-library\.ps1') {
        throw "Shared-library caller does not load the runtime library: $($caller.Path)"
    }
    foreach ($required in $caller.Required) {
        $assignmentPattern = '(?m)^\s*' + [regex]::Escape($required) + '\s*='
        if ($source -notmatch $assignmentPattern) {
            throw ('Shared-library caller does not explicitly own {0}: {1}' -f $required, $caller.Path)
        }
    }
}
Write-Host 'SHARED_LIBRARY_CALLER_GLOBAL_AUDIT=PASS'
