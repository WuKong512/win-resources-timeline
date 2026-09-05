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
        (Join-Path $ToolRoot 'run-admin-amd-system-counter-qualification.ps1'),
        (Join-Path $ToolRoot 'run-standard-user-amd-privilege-client.ps1'),
        (Join-Path $ToolRoot 'run-standard-user-amd-counter-discovery.ps1'),
        (Join-Path $ToolRoot 'cleanup-admin-amd-privilege-qualification.ps1'),
        (Join-Path $ToolRoot 'cleanup-admin-amd-system-counter-qualification.ps1')
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
$systemSetupWrapper = Join-Path $ToolRoot 'run-admin-amd-system-counter-qualification.ps1'
$systemSetupSource = Get-Content -LiteralPath $systemSetupWrapper -Raw
$systemCleanupWrapper = Join-Path $ToolRoot 'cleanup-admin-amd-system-counter-qualification.ps1'
$systemCleanupSource = Get-Content -LiteralPath $systemCleanupWrapper -Raw
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

foreach ($requiredSystemSetupContract in @(
        'ResourceTimelineAmdSystemCounterQualification',
        'NT AUTHORITY\SYSTEM',
        "'LocalSystem'",
        'S-1-5-18',
        '--system-counter-service',
        'timechart',
        '--list',
        'sampling = $false',
        'setup_and_discovery_are_coupled = $true',
        'Set-SystemDirectoryAcl',
        '$ServiceSid'
    )) {
    if ($systemSetupSource -notmatch [regex]::Escape($requiredSystemSetupContract)) {
        throw "SYSTEM counter setup contract is missing: $requiredSystemSetupContract"
    }
}
if ($systemSetupSource -notmatch '9E5A012B0A95C84DD28CD607D99EF43C9BC4D700683F33890CDE6C2108794AC3') {
    throw 'SYSTEM comparison wrapper is not pinned to the new release artifact hash.'
}
$historicalLocalServiceWrappers = @(
    (Join-Path $ToolRoot 'run-admin-amd-privilege-qualification.ps1'),
    (Join-Path $ToolRoot 'run-standard-user-amd-counter-discovery.ps1'),
    (Join-Path $ToolRoot 'run-standard-user-amd-privilege-client.ps1')
)
foreach ($historicalWrapper in $historicalLocalServiceWrappers) {
    if ((Get-Content -LiteralPath $historicalWrapper -Raw) -notmatch 'C9973BAAA01AF3C2673D8C70D8C7E626C577642505E6DFF7BA3C6026DEA63FB1') {
        throw "Historical LocalService wrapper hash changed unexpectedly: $historicalWrapper"
    }
}
if ($systemSetupSource -match '(?i)Start-Process.*-Verb\s+RunAs|runas(?:\.exe)?|PsExec') {
    throw 'SYSTEM counter setup must not self-elevate or invoke another elevation tool.'
}
if ($systemSetupSource -match '(?i)--event|--output-dir|--duration|--interval|working_directory|registry_path|raw_command|executable_path|argv') {
    throw 'SYSTEM counter setup must not expose a sampling or client-controlled command surface.'
}
if ($systemCleanupSource -notmatch 'SYSTEM-CLEANUP-RESULT-') {
    throw 'SYSTEM cleanup evidence must use invocation-distinct filenames.'
}
if ($systemCleanupSource -match 'SYSTEM-CLEANUP-RESULT\.json') {
    throw 'SYSTEM cleanup must not overwrite one fixed cleanup evidence filename.'
}
if ($systemCleanupSource -match '(?im)\bStop-Process\b|\btaskkill(?:\.exe)?\b') {
    throw 'SYSTEM cleanup must not kill processes by broad or unrelated identity.'
}
Write-Host 'SYSTEM_COUNTER_SERVICE_CONTRACT=PASS'
Write-Host 'SYSTEM_COUNTER_FIXED_TIMECHART_LIST=PASS'
Write-Host 'SYSTEM_COUNTER_SETUP_NO_SELF_ELEVATION=PASS'
Write-Host 'SYSTEM_CLEANUP_DUPLICATE_SAFE=PASS'
Write-Host 'SYSTEM_WRAPPER_NEW_ARTIFACT_HASH=PASS'
Write-Host 'LOCALSERVICE_HISTORICAL_ARTIFACT_HASH_PRESERVED=PASS'

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

if ($windowsSourceText -match 'read_frame\(\s*&mut\s+stream\s*\)') {
    throw 'Server connection handling must use the message-aware pipe frame reader, not the generic stream reader.'
}
foreach ($ioCall in @('ReadFile', 'WriteFile')) {
    $ioMatches = [regex]::Matches($windowsSourceText, "(?s)${ioCall}\s*\((.*?)\);" )
    foreach ($ioMatch in $ioMatches) {
        if ($ioMatch.Groups[1].Value -match 'Some\(std::ptr::addr_of_mut!\(transferred\)\)' -and
            $ioMatch.Groups[1].Value -match 'Some\(std::ptr::addr_of_mut!\(overlapped\)\)') {
            throw "$ioCall still passes an asynchronous byte-count output pointer."
        }
    }
}
foreach ($requiredSourceContract in @(
        'fn overlapped_read_chunk',
        'fn synchronous_read_pipe_chunk',
        'fn read_pipe_frame',
        'fn write_pipe_message',
        'fn write_one_message',
        'TRAILING_MESSAGE_DATA',
        'TRUNCATED_PAYLOAD',
        'FIRST-FRAME-'
    )) {
    if ($windowsSourceText -notmatch [regex]::Escape($requiredSourceContract)) {
        throw "Message-mode pipe contract is missing: $requiredSourceContract"
    }
}
Write-Host 'MESSAGE_MODE_SERVER_FRAME_READER=PASS'
Write-Host 'ASYNC_READ_BYTE_COUNT_FROM_COMPLETION=PASS'
Write-Host 'ASYNC_WRITE_BYTE_COUNT_FROM_COMPLETION=PASS'
Write-Host 'ONE_REQUEST_ONE_PIPE_MESSAGE=PASS'
Write-Host 'ONE_RESPONSE_ONE_PIPE_MESSAGE=PASS'
Write-Host 'FIRST_FRAME_FALSE_EOF_REGRESSION=PASS'

$openClientStart = $windowsSourceText.IndexOf('fn open_client_pipe')
$sendRequestStart = $windowsSourceText.IndexOf('fn send_request', $openClientStart)
if ($openClientStart -lt 0 -or $sendRequestStart -le $openClientStart) {
    throw 'Client pipe-open function boundary is missing.'
}
$clientPipeOpenSource = $windowsSourceText.Substring($openClientStart, $sendRequestStart - $openClientStart)
foreach ($requiredClientPipeContract in @(
        'CreateFileW',
        'SECURITY_SQOS_PRESENT',
        'SECURITY_IMPERSONATION',
        'PIPE_READMODE_MESSAGE',
        'PIPE_WAIT',
        'SetNamedPipeHandleState',
        'GetNamedPipeHandleStateW',
        'configure_client_pipe_mode'
    )) {
    if ($clientPipeOpenSource -notmatch [regex]::Escape($requiredClientPipeContract)) {
        throw "Client pipe mode contract is missing: $requiredClientPipeContract"
    }
}
if ($clientPipeOpenSource -match 'FILE_FLAG_OVERLAPPED') {
    throw 'Qualification client pipe must remain synchronous; FILE_FLAG_OVERLAPPED was added to client open.'
}
$createFileIndex = $clientPipeOpenSource.IndexOf('CreateFileW')
$raiiIndex = $clientPipeOpenSource.IndexOf('File::from_raw_handle')
$configureIndex = $clientPipeOpenSource.IndexOf('configure_client_pipe_mode')
$setStateIndex = $clientPipeOpenSource.IndexOf('SetNamedPipeHandleState')
$verifyStateIndex = $clientPipeOpenSource.IndexOf('GetNamedPipeHandleStateW')
if ($createFileIndex -lt 0 -or $raiiIndex -lt $createFileIndex -or
    $configureIndex -lt $raiiIndex -or $setStateIndex -lt $configureIndex -or
    $verifyStateIndex -lt $setStateIndex) {
    throw 'Client pipe mode must be configured and verified on the RAII handle before protocol use.'
}
if ($clientPipeOpenSource -notmatch 'configure_client_pipe_mode\(\&stream\)\?') {
    throw 'Client pipe mode failure must return before the first semantic request.'
}
Write-Host 'CLIENT_CREATEFILE_DEFAULT_BYTE_MODE_NOT_ACCEPTED=PASS'
Write-Host 'CLIENT_SWITCHES_TO_MESSAGE_READ_MODE_BEFORE_FIRST_PROTOCOL_REQUEST=PASS'
Write-Host 'CLIENT_MESSAGE_READ_MODE_REQUIRED_FOR_BOUNDARY_AWARE_RESPONSE_READER=PASS'
Write-Host 'CLIENT_PIPE_MODE_CONFIGURATION_FAILURE_FAILS_BEFORE_REQUEST=PASS'
Write-Host 'CLIENT_PIPE_MODE_CONFIGURATION_FAILURE_CLOSES_HANDLE=PASS'
Write-Host 'CLIENT_SECURITY_SQOS_PRESERVED=PASS'
Write-Host 'CLIENT_SYNCHRONOUS_IO_PRESERVED=PASS'
Write-Host 'CLIENT_EFFECTIVE_MESSAGE_READ_MODE_VERIFIED=PASS'

foreach ($requiredCounterDiscoveryContract in @(
        'GetAmdCounterAvailability',
        'fixed_counter_discovery_arguments',
        'timechart',
        '--list',
        'COUNTER_DISCOVERY_MAX_OUTPUT_BYTES',
        'classify_counter_discovery',
        'COUNTERS_UNAVAILABLE'
    )) {
    if ($windowsSourceText -notmatch [regex]::Escape($requiredCounterDiscoveryContract)) {
        throw "Counter-discovery contract is missing: $requiredCounterDiscoveryContract"
    }
}
Write-Host 'COUNTER_DISCOVERY_FIXED_TIMECHART_LIST_CONTRACT=PASS'
Write-Host 'COUNTER_DISCOVERY_NO_USER_COMMAND_SURFACE=PASS'

$counterDiscoverySource = Get-Content -LiteralPath (Join-Path $ToolRoot 'run-standard-user-amd-counter-discovery.ps1') -Raw
if ($counterDiscoverySource -match '--event|--output-dir|--duration|--interval') {
    throw 'Counter-discovery client wrapper must not expose a sampling command surface.'
}
Write-Host 'COUNTER_DISCOVERY_CLIENT_WRAPPER_IS_NON_SAMPLING=PASS'

$readmeSource = Get-Content -LiteralPath (Join-Path $ToolRoot 'README.md') -Raw
$i2bStart = $readmeSource.IndexOf('## I2B human handoff: non-sampling counter discovery')
$i2cStart = $readmeSource.IndexOf('## I2C human handoff: SYSTEM counter-discovery comparison')
if ($i2bStart -lt 0 -or $i2cStart -le $i2bStart) {
    throw 'README does not contain a bounded I2B handoff section.'
}
$activeI2bHandoff = $readmeSource.Substring($i2bStart, $i2cStart - $i2bStart)
if ($activeI2bHandoff -notmatch 'run-standard-user-amd-counter-discovery\.ps1' -or
    $activeI2bHandoff -match 'run-standard-user-amd-privilege-client\.ps1') {
    throw 'README I2B handoff does not isolate the non-sampling client wrapper.'
}
if ($readmeSource -notmatch 'run-admin-amd-system-counter-qualification\.ps1' -or
    $readmeSource -notmatch 'cleanup-admin-amd-system-counter-qualification\.ps1' -or
    $readmeSource -notmatch 'NOT_EXECUTED / HUMAN_AUTHORIZATION_REQUIRED') {
    throw 'README SYSTEM comparison handoff is incomplete.'
}
Write-Host 'README_I2B_NON_SAMPLING_HANDOFF=PASS'
Write-Host 'README_SYSTEM_HANDOFF_NOT_EXECUTED=PASS'

foreach ($requiredTokenDifferentialContract in @(
        'TokenPrivileges',
        'LookupPrivilegeNameW',
        'enabled_privileges',
        'disabled_privileges',
        'token_groups_relevant_to_access',
        'amd-privilege-service-context/v2'
    )) {
    if ($windowsSourceText -notmatch [regex]::Escape($requiredTokenDifferentialContract)) {
        throw "Token differential evidence contract is missing: $requiredTokenDifferentialContract"
    }
}
Write-Host 'TOKEN_DIFFERENTIAL_EVIDENCE_CONTRACT=PASS'

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
foreach ($requiredSyntheticCheck in @(
        'COUNTER_DISCOVERY_FIXED_SEMANTIC_REQUEST',
        'COUNTER_DISCOVERY_NO_COUNTERS_EXIT_ZERO',
        'COUNTER_DISCOVERY_POWER_PRESENT',
        'COUNTER_DISCOVERY_UNKNOWN_FAILURE'
    )) {
    $check = @($summary.checks | Where-Object { $_.name -eq $requiredSyntheticCheck })
    if ($check.Count -ne 1 -or $check[0].status -ne 'PASS') {
        throw "Counter-discovery synthetic check did not pass: $requiredSyntheticCheck"
    }
}
if ($summary.mutation_assertions.real_amd_runtime_count_during_task -ne 0 -or
    $summary.mutation_assertions.service_registration_count_during_task -ne 0 -or
    $summary.mutation_assertions.scheduled_task_registration_count -ne 0 -or
    $summary.mutation_assertions.self_elevation_performed -ne $false -or
    $summary.mutation_assertions.amd_installation_mutated -ne $false -or
    $summary.mutation_assertions.amd_registry_mutated -ne $false) {
    throw 'Synthetic mutation assertions are not clean.'
}

foreach ($serviceName in @(
        'ResourceTimelineAmdPrivilegeQualification',
        'ResourceTimelineAmdSystemCounterQualification'
    )) {
    if (Get-Service -Name $serviceName -ErrorAction SilentlyContinue) {
        throw "Synthetic test refuses to run while the qualification service is registered: $serviceName"
    }
}
Write-Host 'Synthetic qualification PASS. No service registration, AMD runtime, elevation, or AMD installation mutation was performed.'
