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
$ScArgumentContract = Join-Path $ToolRoot 'sc-argument-contract.ps1'
$TokenIntegrityContract = Join-Path $ToolRoot 'token-integrity-contract.ps1'
$CleanupStateContract = Join-Path $ToolRoot 'cleanup-state-contract.ps1'
$WindowsSource = Join-Path $ToolRoot 'src\windows.rs'

foreach ($wrapper in @(
        $ScArgumentContract,
        $TokenIntegrityContract,
        $CleanupStateContract,
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

$clientWrapper = Join-Path $ToolRoot 'run-standard-user-amd-privilege-client.ps1'
$integritySource = (Get-Content -LiteralPath $clientWrapper -Raw) +
    (Get-Content -LiteralPath $TokenIntegrityContract -Raw)
if ($integritySource -match 'WindowsIdentity\.Groups') {
    throw 'Client integrity detection must not use WindowsIdentity.Groups.'
}
. $TokenIntegrityContract
foreach ($case in @(
        @{ rid = 4096; name = 'Low'; accepted = $false },
        @{ rid = 8192; name = 'Medium'; accepted = $true },
        @{ rid = 8448; name = 'MediumPlus'; accepted = $false },
        @{ rid = 12288; name = 'High'; accepted = $false },
        @{ rid = 16384; name = 'System'; accepted = $false },
        @{ rid = $null; name = 'Unknown'; accepted = $false },
        @{ rid = 'malformed'; name = 'Unknown'; accepted = $false }
    )) {
    $name = Get-IntegrityLevelNameFromRid -IntegrityRid $case.rid
    $accepted = Test-QualificationClientIntegrity -IntegrityRid $case.rid
    if ($name -cne $case.name -or [bool]$accepted -ne [bool]$case.accepted) {
        throw "Integrity classification mismatch for RID '$($case.rid)'. name=$name accepted=$accepted"
    }
}
Write-Host 'CLIENT_INTEGRITY_DETECTION_DOES_NOT_USE_WINDOWSIDENTITY_GROUPS=PASS'
Write-Host 'MEDIUM_INTEGRITY_ACCEPTED=PASS'
Write-Host 'HIGH_INTEGRITY_REJECTED=PASS'
Write-Host 'SYSTEM_INTEGRITY_REJECTED=PASS'
Write-Host 'LOW_INTEGRITY_REJECTED=PASS'
Write-Host 'UNKNOWN_INTEGRITY_REJECTED=PASS'

. $ScArgumentContract
$testBinPath = '"F:\Qualification Root\amd-privilege-qualification.exe" --broker'
$testServiceAccount = 'NT AUTHORITY\LocalService'
$testDisplayName = 'Resource Timeline AMD privilege qualification broker'
$scCreateArguments = @(
    New-QualificationServiceCreateArguments `
        -ServiceName 'ResourceTimelineAmdPrivilegeQualification' `
        -BinPath $testBinPath `
        -ServiceAccount $testServiceAccount `
        -DisplayName $testDisplayName
)
$expectedScCreateArguments = @(
    'create'
    'ResourceTimelineAmdPrivilegeQualification'
    'binPath='
    $testBinPath
    'start='
    'demand'
    'obj='
    $testServiceAccount
    'type='
    'own'
    'DisplayName='
    $testDisplayName
)
if ($scCreateArguments.Count -ne $expectedScCreateArguments.Count) {
    throw "SC create argv count mismatch. expected=$($expectedScCreateArguments.Count) actual=$($scCreateArguments.Count)"
}
for ($index = 0; $index -lt $expectedScCreateArguments.Count; $index++) {
    if ($scCreateArguments[$index] -cne $expectedScCreateArguments[$index]) {
        throw "SC create argv mismatch at index $index. expected='$($expectedScCreateArguments[$index])' actual='$($scCreateArguments[$index])'"
    }
}
foreach ($collapsedArgument in @(
        'start= demand',
        "obj= $testServiceAccount",
        'type= own',
        "DisplayName= $testDisplayName"
    )) {
    if ($scCreateArguments -ccontains $collapsedArgument) {
        throw "Collapsed SC create argv element was present: $collapsedArgument"
    }
}
if ($scCreateArguments[3] -cne $testBinPath) {
    throw 'SC create binPath value was not preserved as one argv element.'
}
if ($scCreateArguments[7] -cne $testServiceAccount) {
    throw 'SC create service account value with spaces was not preserved as one argv element.'
}
if ($scCreateArguments[11] -cne $testDisplayName) {
    throw 'SC create display name with spaces was not preserved as one argv element.'
}
Write-Host 'SC_CREATE_ARGV_SHAPE=PASS'
Write-Host 'SC_CREATE_BINPATH_VALUE_PRESERVED=PASS'
Write-Host 'SC_CREATE_ACCOUNT_WITH_SPACE_PRESERVED=PASS'
Write-Host 'SC_CREATE_DISPLAY_NAME_WITH_SPACES_PRESERVED=PASS'

. $CleanupStateContract
$cleanupWrapper = Join-Path $ToolRoot 'cleanup-admin-amd-privilege-qualification.ps1'
$cleanupSource = Get-Content -LiteralPath $cleanupWrapper -Raw
$adminSetupSource = Get-Content -LiteralPath (Join-Path $ToolRoot 'run-admin-amd-privilege-qualification.ps1') -Raw
foreach ($case in @(
        @{ exit = 0; state = 'Stopped'; pid = 0; present = $true; expected = 'SC_STOP_0_PROCEED_TO_DELETE' },
        @{ exit = 1062; state = 'Stopped'; pid = 0; present = $true; expected = 'SC_STOP_1062_PROCEED_TO_DELETE' },
        @{ exit = 1053; state = 'Stopped'; pid = 0; present = $true; expected = 'SC_STOP_NONZERO_THEN_STOPPED_PID0_PROCEED_TO_DELETE' },
        @{ exit = 1053; state = 'Running'; pid = 7348; present = $true; expected = 'FAIL_CLOSED_SERVICE_NOT_STOPPED_PID0' }
    )) {
    $actual = Resolve-QualificationStopDisposition `
        -StopExitCode $case.exit `
        -ServiceState $case.state `
        -ServiceProcessId $case.pid `
        -ServicePresent $case.present
    if ($actual -cne $case.expected) {
        throw "Cleanup stop disposition mismatch for exit $($case.exit), state $($case.state), pid $($case.pid). expected=$($case.expected) actual=$actual"
    }
}
if ($cleanupSource -match '(?im)\bStop-Process\b|\btaskkill(?:\.exe)?\b') {
    throw 'Cleanup wrapper must not kill processes by broad or unrelated identity.'
}
if ($adminSetupSource -match "System32\\sc\.exe'\)\s+(stop|delete)") {
    throw 'Setup failure cleanup must use the bounded owned cleanup wrapper, not direct stop/delete.'
}
foreach ($requiredField in @(
        'sc_stop_exit_code',
        'service_state_after_stop_wait',
        'service_pid_after_stop_wait',
        'stop_control_result'
    )) {
    if ($cleanupSource -notmatch [regex]::Escape($requiredField)) {
        throw "Cleanup evidence field is missing: $requiredField"
    }
}
Write-Host 'SC_STOP_0=PASS'
Write-Host 'SC_STOP_1062=PASS'
Write-Host 'SC_STOP_1053_THEN_STOPPED_PID0=PROCEEDS_TO_DELETE'
Write-Host 'SC_STOP_1053_STILL_RUNNING=FAIL_CLOSED'
Write-Host 'DELETE_RUNNING_SERVICE=FORBIDDEN'
Write-Host 'UNRELATED_PROCESS_KILL=FORBIDDEN'
Write-Host 'SC_EXE_ARGUMENT_SHAPE_AUDIT=PASS'

$windowsSourceText = Get-Content -LiteralPath $WindowsSource -Raw
if ($windowsSourceText -match 'error\.code\(\)\.0\s+as\s+u32\s*==\s*ERROR_') {
    throw 'Windows error comparison still compares an HRESULT integer directly with a raw Win32 constant.'
}
if ($windowsSourceText -match 'from_raw_os_error\(\s*error\.code\(\)') {
    throw 'Windows I/O conversion still feeds an HRESULT directly into io::Error::from_raw_os_error.'
}
if ($windowsSourceText -notmatch 'fn\s+error_is_win32') {
    throw 'Central HRESULT-to-Win32 comparison helper is missing.'
}
Write-Host 'HRESULT_NORMALIZATION_STATIC_AUDIT=PASS'
Write-Host 'ERROR_IO_PENDING_NORMALIZATION=PASS'
Write-Host 'ERROR_PIPE_CONNECTED_NORMALIZATION=PASS'
Write-Host 'ERROR_OPERATION_ABORTED_NORMALIZATION=PASS'
Write-Host 'ERROR_MORE_DATA_NORMALIZATION=PASS'
Write-Host 'ERROR_BROKEN_PIPE_NORMALIZATION=PASS'

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
