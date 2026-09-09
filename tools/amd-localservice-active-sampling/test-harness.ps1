[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ToolRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$RunnerPath = Join-Path $ToolRoot 'run-amd-localservice-active-sampling.ps1'
$ServiceHostPath = Join-Path $ToolRoot 'service-host.ps1'
. (Join-Path $ToolRoot 'contract.ps1')
. (Join-Path $ToolRoot '..\amd-uprof-cli-spike\postprocess.ps1')

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (-not $Condition) {
        throw "ASSERTION_FAILED: $Message"
    }
}

function Assert-Equal {
    param(
        [Parameter(Mandatory = $true)]$Actual,
        [Parameter(Mandatory = $true)]$Expected,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if ([string]$Actual -cne [string]$Expected) {
        throw "ASSERTION_FAILED: $Message; expected=$Expected actual=$Actual"
    }
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$Needle,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if ($Text.IndexOf($Needle, [StringComparison]::Ordinal) -lt 0) {
        throw "ASSERTION_FAILED: $Message; missing=$Needle"
    }
}

function Test-PowerShellSyntax {
    param([Parameter(Mandatory = $true)][string]$Path)

    $tokens = $null
    $errors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors) | Out-Null
    Assert-Equal -Actual $errors.Count -Expected 0 -Message "PowerShell syntax: $Path"
}

function New-TestRoot {
    $root = Join-Path ([IO.Path]::GetTempPath()) ('amd-localservice-active-sampling-test-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $root | Out-Null
    $root
}

$contract = Get-AmdLocalServiceSamplingContract
Assert-Equal -Actual $contract.task_id -Expected 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1' -Message 'task id'
Assert-Equal -Actual $contract.account_sid -Expected 'S-1-5-19' -Message 'LocalService SID'
Assert-Equal -Actual $contract.service_sid_type -Expected 'unrestricted' -Message 'Service SID type'
Assert-Equal -Actual $contract.session_id -Expected 0 -Message 'Session 0'
Assert-Equal -Actual $contract.interactive -Expected $false -Message 'non-interactive contract'
Assert-Equal -Actual $contract.amd_cli_sha256 -Expected 'D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC' -Message 'CLI SHA256'
Assert-Equal -Actual $contract.amd_cli_file_version -Expected '5.3.521.0' -Message 'CLI file version'
Assert-Equal -Actual $contract.amd_cli_interval_ms -Expected 1000 -Message 'CLI interval'
Assert-Equal -Actual $contract.amd_cli_duration_seconds -Expected 10 -Message 'CLI duration'
Assert-Equal -Actual $contract.max_runs -Expected 1 -Message 'one-shot max runs'
Assert-Equal -Actual $contract.retries -Expected 0 -Message 'retry prohibition'

$command = @(Get-FixedAmdCliArguments -OutputDirectory 'C:\ProgramData\run\raw\timechart-output')
Assert-Equal -Actual ($command -join '|') -Expected 'timechart|--event|power|--interval|1000|--duration|10|--format|csv|--output-dir|C:\ProgramData\run\raw\timechart-output' -Message 'exact CLI command'
Assert-True -Condition (-not ($command -contains '--list')) -Message 'active command must not include discovery'
Assert-True -Condition (-not ($command -contains '--temperature')) -Message 'active command must not add temperature'
Assert-True -Condition (-not ($command -contains '--frequency')) -Message 'active command must not add frequency'

$tokenFixture = Get-OfflineTokenFixture
$expectedServiceSid = 'S-1-5-80-1234567890-1234567890-1234567890-1234567890-1234'
$tokenGate = Test-EffectiveTokenEvidence -Evidence $tokenFixture -Contract $contract -ExpectedServiceSid $expectedServiceSid
Assert-True -Condition $tokenGate.valid -Message 'offline CONTROL token fixture'
$wrongServiceSidGate = Test-EffectiveTokenEvidence -Evidence $tokenFixture -Contract $contract -ExpectedServiceSid 'S-1-5-80-1-2-3-4-5'
Assert-True -Condition (-not $wrongServiceSidGate.valid) -Message 'unexpected Service SID must be rejected'
$profileSingle = $tokenFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$profileSingle.privileges += [pscustomobject]@{ name = 'SeProfileSingleProcessPrivilege'; state = 'Enabled' }
$profileGate = Test-EffectiveTokenEvidence -Evidence $profileSingle -Contract $contract -ExpectedServiceSid $expectedServiceSid
Assert-True -Condition (-not $profileGate.valid) -Message 'ProfileSingle must be rejected'
$adminGroup = $tokenFixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$adminGroup.group_sids += 'S-1-5-32-544'
$adminGate = Test-EffectiveTokenEvidence -Evidence $adminGroup -Contract $contract -ExpectedServiceSid $expectedServiceSid
Assert-True -Condition (-not $adminGate.valid) -Message 'Administrators membership must be rejected'

$binary = Get-OfflineBinaryFixture -Contract $contract
$binaryGate = Test-BinaryIdentityEvidence -Identity $binary -Contract $contract
Assert-True -Condition $binaryGate.valid -Message 'offline binary identity fixture'
$badBinary = $binary | ConvertTo-Json -Depth 20 | ConvertFrom-Json
$badBinary.sha256 = ('0' * 64)
$badBinaryGate = Test-BinaryIdentityEvidence -Identity $badBinary -Contract $contract
Assert-True -Condition (-not $badBinaryGate.valid) -Message 'binary SHA mismatch must block'

$drivers = @(Get-OfflineDriverFixture -Contract $contract)
$driverGate = Test-DriverVersionEvidence -Drivers $drivers -Contract $contract
Assert-True -Condition $driverGate.valid -Message 'offline driver identity fixtures'
$badDrivers = @($drivers | ForEach-Object {
    $copy = $_ | ConvertTo-Json -Depth 10 | ConvertFrom-Json
    if ($copy.name -eq 'AMDPowerProfiler') { $copy.file_version = '0.0.0.0' }
    $copy
})
$badDriverGate = Test-DriverVersionEvidence -Drivers $badDrivers -Contract $contract
Assert-True -Condition (-not $badDriverGate.valid) -Message 'driver version mismatch must block'

$fixtureRoot = New-TestRoot
try {
    $validRoot = Join-Path $fixtureRoot 'q1-run'
    $rootGate = Test-ControlledRunRoot -RunRoot $validRoot -OutputBase $fixtureRoot
    Assert-True -Condition $rootGate.valid -Message 'new isolated run root'
    New-Item -ItemType Directory -Path $validRoot | Out-Null
    $existingGate = Test-ControlledRunRoot -RunRoot $validRoot -OutputBase $fixtureRoot
    Assert-True -Condition (-not $existingGate.valid) -Message 'pre-existing run root must block'
    $traversal = Test-ControlledRunRoot -RunRoot (Join-Path $fixtureRoot '..\outside') -OutputBase $fixtureRoot
    Assert-True -Condition (-not $traversal.valid) -Message 'run root traversal must block'
}
finally {
    if (Test-Path -LiteralPath $fixtureRoot) {
        Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
    }
}

$validCsvRoot = New-TestRoot
try {
    $validCsv = Join-Path $validCsvRoot 'timechart.csv'
    [IO.File]::WriteAllText($validCsv, (Get-OfflineCsvFixtureText))
    $parsed = Parse-PackagePowerCsv -Path $validCsv
    $powerEvidence = Test-PackagePowerEvidence -Parsed $parsed
    Assert-Equal -Actual $powerEvidence.status -Expected 'PASS' -Message 'finite non-constant package power'
    Assert-Equal -Actual $powerEvidence.sample_count -Expected 3 -Message 'CSV sample count'
    $constantCsv = Join-Path $validCsvRoot 'constant.csv'
    [IO.File]::WriteAllText($constantCsv, @'
PROFILE RECORDS
Timestamp,Record ID,Socket0-Package-Power
2026-01-01T00:00:00Z,1,40
2026-01-01T00:00:01Z,2,40
'@)
    $constantEvidence = Test-PackagePowerEvidence -Parsed (Parse-PackagePowerCsv -Path $constantCsv)
    Assert-Equal -Actual $constantEvidence.status -Expected 'FAIL' -Message 'constant package power rejected'
    Assert-Equal -Actual $constantEvidence.non_constant -Expected $false -Message 'constant flag'
}
finally {
    if (Test-Path -LiteralPath $validCsvRoot) {
        Remove-Item -LiteralPath $validCsvRoot -Recurse -Force
    }
}

Test-PowerShellSyntax -Path $RunnerPath
Test-PowerShellSyntax -Path $ServiceHostPath
$runnerText = Get-Content -LiteralPath $RunnerPath -Raw
$serviceText = Get-Content -LiteralPath $ServiceHostPath -Raw
Assert-Contains -Text $runnerText -Needle 'sc.exe' -Message 'live service lifecycle exists'
Assert-Contains -Text $runnerText -Needle 'Get-GitBaselineEvidence' -Message 'baseline pin and clean-tree preflight exists'
Assert-Contains -Text $runnerText -Needle 'Get-RuntimeFailureCategory' -Message 'runtime failure classification exists'
Assert-Contains -Text $runnerText -Needle 'showsid' -Message 'exact Service SID capture exists'
Assert-Contains -Text $runnerText -Needle 'Get-ServiceConfigurationEvidence' -Message 'service configuration validation exists'
Assert-Contains -Text $runnerText -Needle 'Acquire-OneShotGate' -Message 'one-shot gate exists'
Assert-Contains -Text $runnerText -Needle 'max_runs = 1' -Message 'one-shot max is encoded'
Assert-Contains -Text $runnerText -Needle 'retries = 0' -Message 'zero retries is encoded'
Assert-Contains -Text $serviceText -Needle 'ServiceBase' -Message 'dedicated ServiceBase host exists'
Assert-Contains -Text $serviceText -Needle 'Get-EffectiveTokenEvidence' -Message 'effective token capture exists'
Assert-Contains -Text $serviceText -Needle 'ExpectedServiceSid' -Message 'exact Service SID token validation exists'
Assert-Contains -Text $serviceText -Needle 'Invoke-BoundedAmdCli' -Message 'bounded child path exists'
Assert-Contains -Text $serviceText -Needle 'taskkill.exe' -Message 'owned process-tree cleanup exists'
Assert-True -Condition (-not ($runnerText -match 'I2G_REAL_RUN_AUTHORIZATION')) -Message 'I2G authorization marker is not reused'

$powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
if (-not (Test-Path -LiteralPath $powershell -PathType Leaf)) {
    $powershell = (Get-Command pwsh -ErrorAction Stop).Source
}
$dryOutput = & $powershell -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $RunnerPath -Mode DryRun 2>&1 | Out-String
$dryExitCode = $LASTEXITCODE
Assert-Equal -Actual $dryExitCode -Expected 0 -Message 'runner dry-run exit code'
$dry = $dryOutput | ConvertFrom-Json
Assert-Equal -Actual $dry.result -Expected 'OFFLINE_VALIDATION_PASS' -Message 'runner dry-run result'
Assert-Equal -Actual $dry.plan.command.arguments[0] -Expected 'timechart' -Message 'dry-run command verb'
Assert-Equal -Actual $dry.plan.command.arguments[2] -Expected 'power' -Message 'dry-run package power event'
Assert-Equal -Actual $dry.plan.gate.max_runs -Expected 1 -Message 'dry-run gate max runs'
Assert-Equal -Actual $dry.plan.gate.retries -Expected 0 -Message 'dry-run gate retries'
Assert-Equal -Actual $dry.amd_cli_real_invocations -Expected 0 -Message 'dry-run AMD CLI count'
Assert-Equal -Actual $dry.power_sampling_runs -Expected 0 -Message 'dry-run sampling count'
Assert-True -Condition (-not (Test-Path -LiteralPath $contract.output_base)) -Message 'dry-run did not create ProgramData output base'

$liveOutput = & $powershell -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $RunnerPath -Mode Live 2>&1 | Out-String
$liveExitCode = $LASTEXITCODE
Assert-True -Condition ($liveExitCode -ne 0) -Message 'live mode without dedicated authorization must fail closed'
Assert-Contains -Text $liveOutput -Needle 'AuthorizeLiveRun' -Message 'live authorization failure is explicit'
$residualService = Get-CimInstance Win32_Service -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -eq $contract.service_name }
Assert-True -Condition ($null -eq $residualService) -Message 'unauthorized live path created no service'
$residualProcess = Get-Process -Name 'AMDuProfCLI', 'AMDuProf' -ErrorAction SilentlyContinue
Assert-True -Condition ($null -eq $residualProcess) -Message 'offline tests started no AMD process'

[pscustomobject]@{
    result = 'AMD_LOCAL_SERVICE_ACTIVE_SAMPLING_HARNESS_OFFLINE_TEST_PASS'
    task_id = 'AMD-LOCALSERVICE-ACTIVE-SAMPLING-HARNESS-I1'
    dry_run = 'PASS'
    syntax = 'PASS'
    command_contract = 'PASS'
    token_contract = 'PASS'
    binary_driver_contract = 'PASS'
    csv_validation = 'PASS'
    live_default_fail_closed = 'PASS'
    amd_cli_real_invocations = 0
    amd_api_real_invocations = 0
    power_sampling_runs = 0
    service_mutations = 0
    lsa_mutations = 0
    token_mutations = 0
    acl_mutations = 0
    driver_mutations = 0
    platform_security_mutations = 0
} | ConvertTo-Json -Depth 10
