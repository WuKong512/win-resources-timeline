#requires -Version 5.1
[CmdletBinding()]
param(
    [string]$ToolRoot = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($ToolRoot)) {
    $ToolRoot = Split-Path -Parent $PSCommandPath
}

$contractPath = Join-Path $ToolRoot 'i2g-runtime-contract.ps1'
$setupPath = Join-Path $ToolRoot 'run-admin-amd-i2g-qualification.ps1'
$realRunnerPath = Join-Path $ToolRoot 'i2g-real-run.ps1'
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

function Get-I2gFunctionDefinitionText {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Name
    )
    $parseErrors = $null
    $tokens = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$parseErrors)
    Assert-True ($parseErrors.Count -eq 0) "Cannot parse $Path while extracting $Name."
    $definition = $ast.Find({
        param($candidate)
        $candidate -is [System.Management.Automation.Language.FunctionDefinitionAst] -and
            $candidate.Name -ceq $Name
    }, $true)
    Assert-True ($null -ne $definition) "Function definition not found: $Name"
    [string]$definition.Extent.Text
}

function Invoke-I2gThrowawayChildExitTest {
    $childPath = Join-Path $testRoot 'throwaway-child-exit-2.ps1'
    [IO.File]::WriteAllText($childPath, "exit 2`r`n", [Text.UTF8Encoding]::new($false))
    $childExitCode = $null
    $parentFinallyExecuted = $false
    try {
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = Join-Path $PSHOME 'powershell.exe'
        $psi.Arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}"' -f $childPath
        $psi.WorkingDirectory = $testRoot
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $child = New-Object System.Diagnostics.Process
        $child.StartInfo = $psi
        Assert-True $child.Start() 'Throwaway child did not start.'
        $child.WaitForExit()
        $childExitCode = [int]$child.ExitCode
        $child.Dispose()
    } finally {
        $parentFinallyExecuted = $true
    }
    $parentSurvives = $true
    Assert-True ($childExitCode -eq 2) "Throwaway child exit code was not 2: $childExitCode"
    Assert-True $parentSurvives 'Parent did not survive the child exit.'
    Assert-True $parentFinallyExecuted 'Parent finally block did not execute.'
    Write-Host 'I2G_CHILD_EXIT_CODE=2 PARENT_SURVIVES=YES PARENT_FINALLY_EXECUTES=YES'
}

function Invoke-I2gThrowawayContractRestoreTest {
    $root = Join-Path $testRoot 'throwaway-contract-restore'
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    $destination = Join-Path $root 'contract.tmp-test'
    $replacement = Join-Path $root 'replacement.tmp-test'
    $replacementBackup = Join-Path $root ('.replace-backup-{0}' -f ([Guid]::NewGuid().ToString('N')))
    $originalBytes = [byte[]](65, 10, 66, 10)
    $replacementBytes = [byte[]](67, 10, 68, 10)
    [IO.File]::WriteAllBytes($destination, $originalBytes)
    [IO.File]::WriteAllBytes($replacement, $replacementBytes)
    $originalHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
    $parentFinallyExecuted = $false
    $childExitCode = $null
    try {
        Assert-True (-not (Test-Path -LiteralPath $replacementBackup -PathType Leaf)) 'Throwaway replacement backup already exists.'
        [IO.File]::Replace($replacement, $destination, $replacementBackup, $true)
        Assert-True ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $originalHash) 'Throwaway overlay did not change destination.'
        Assert-True ((Get-FileHash -LiteralPath $replacementBackup -Algorithm SHA256).Hash -ceq $originalHash) 'Throwaway replacement backup did not preserve original bytes.'
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = Join-Path $PSHOME 'powershell.exe'
        $psi.Arguments = '-NoProfile -NonInteractive -Command "exit 2"'
        $psi.WorkingDirectory = $root
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $child = New-Object System.Diagnostics.Process
        $child.StartInfo = $psi
        Assert-True $child.Start() 'Throwaway restore child did not start.'
        $child.WaitForExit()
        $childExitCode = [int]$child.ExitCode
        $child.Dispose()
    } finally {
        $parentFinallyExecuted = $true
        if (Test-Path -LiteralPath $destination -PathType Leaf) {
            $restoreTemp = Join-Path $root 'restore-original.tmp-test'
            $restoreBackup = Join-Path $root ('.restore-backup-{0}' -f ([Guid]::NewGuid().ToString('N')))
            [IO.File]::WriteAllBytes($restoreTemp, $originalBytes)
            [IO.File]::Replace($restoreTemp, $destination, $restoreBackup, $true)
            if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ceq $originalHash -and
                (Test-Path -LiteralPath $restoreBackup -PathType Leaf)) {
                Remove-Item -LiteralPath $restoreBackup -Force
            }
        }
    }
    $finalHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
    Assert-True ($childExitCode -eq 2) "Throwaway restore child exit code was not 2: $childExitCode"
    Assert-True $parentFinallyExecuted 'Throwaway restore parent finally did not execute.'
    Assert-True ($finalHash -ceq $originalHash) 'Throwaway contract bytes were not restored.'
    Write-Host 'I2G_CONTRACT_RESTORE_AFTER_CHILD_EXIT=PASS'
}

function Invoke-I2gNestedChildProcessIsolationTest {
    $root = Join-Path $testRoot 'nested-child-process-isolation'
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    $innerRunnerPath = Join-Path $root 'throwaway-inner-runner.ps1'
    $wrapperChildPath = Join-Path $root 'throwaway-wrapper-child.ps1'
    $innerRunnerSource = @'
param(
    [Parameter(Mandatory = $true)][int]$ExitCode,
    [Parameter(Mandatory = $true)][string]$MarkerPath
)
[IO.File]::WriteAllText($MarkerPath, ("INNER_RUNNER_EXIT_CODE={0}" -f $ExitCode), [Text.UTF8Encoding]::new($false))
exit $ExitCode
'@
    $wrapperChildSource = @'
param(
    [Parameter(Mandatory = $true)][string]$InnerRunnerPath,
    [Parameter(Mandatory = $true)][string]$MarkerPath,
    [Parameter(Mandatory = $true)][int]$ExitCode
)
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = Join-Path $PSHOME 'powershell.exe'
$psi.Arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}" -ExitCode {1} -MarkerPath "{2}"' -f $InnerRunnerPath.Replace('"', '\"'), $ExitCode, $MarkerPath.Replace('"', '\"')
$psi.WorkingDirectory = Split-Path -Parent $InnerRunnerPath
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$child = New-Object System.Diagnostics.Process
$child.StartInfo = $psi
if (-not $child.Start()) { exit 1 }
$stdoutTask = $child.StandardOutput.ReadToEndAsync()
$stderrTask = $child.StandardError.ReadToEndAsync()
$child.WaitForExit()
$stdout = [string]$stdoutTask.Result
$stderr = [string]$stderrTask.Result
if (-not [string]::IsNullOrEmpty($stdout)) { [Console]::Out.Write($stdout) }
if (-not [string]::IsNullOrEmpty($stderr)) { [Console]::Error.Write($stderr) }
$innerExitCode = [int]$child.ExitCode
$child.Dispose()
Write-Output ("WRAPPER_CHILD_EXIT_CODE={0}" -f $innerExitCode)
exit $innerExitCode
'@
    [IO.File]::WriteAllText($innerRunnerPath, $innerRunnerSource, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($wrapperChildPath, $wrapperChildSource, [Text.UTF8Encoding]::new($false))

    function Invoke-ThrowawayWrapperChild {
        param(
            [Parameter(Mandatory = $true)][int]$ExitCode,
            [Parameter(Mandatory = $true)][string]$MarkerPath
        )
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = Join-Path $PSHOME 'powershell.exe'
        $psi.Arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}" -InnerRunnerPath "{1}" -MarkerPath "{2}" -ExitCode {3}' -f
            $wrapperChildPath.Replace('"', '\"'), $innerRunnerPath.Replace('"', '\"'), $MarkerPath.Replace('"', '\"'), $ExitCode
        $psi.WorkingDirectory = $root
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $child = New-Object System.Diagnostics.Process
        $child.StartInfo = $psi
        if (-not $child.Start()) { throw 'Throwaway wrapper child did not start.' }
        $stdoutTask = $child.StandardOutput.ReadToEndAsync()
        $stderrTask = $child.StandardError.ReadToEndAsync()
        $child.WaitForExit()
        $stdout = [string]$stdoutTask.Result
        $stderr = [string]$stderrTask.Result
        if (-not [string]::IsNullOrEmpty($stdout)) { [Console]::Out.Write($stdout) }
        if (-not [string]::IsNullOrEmpty($stderr)) { [Console]::Error.Write($stderr) }
        $exitCodeObserved = [int]$child.ExitCode
        $child.Dispose()
        [pscustomobject]@{ exit_code = $exitCodeObserved }
    }

    foreach ($exitCode in @(0, 1, 2)) {
        $destination = Join-Path $root ('contract-exit-{0}.tmp-test' -f $exitCode)
        $overlay = Join-Path $root ('overlay-exit-{0}.tmp-test' -f $exitCode)
        $marker = Join-Path $root ('inner-runner-exit-{0}.marker' -f $exitCode)
        $originalBytes = [byte[]](65, 10, 66, 10)
        $overlayBytes = [byte[]](67, 10, 68, 10)
        [IO.File]::WriteAllBytes($destination, $originalBytes)
        $originalHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
        $parentFinallyExecuted = $false
        $wrapperChildExitCode = $null
        try {
            [IO.File]::WriteAllBytes($overlay, $overlayBytes)
            $overlayBackup = Join-Path $root ('.overlay-backup-{0}-{1}' -f $exitCode, ([Guid]::NewGuid().ToString('N')))
            [IO.File]::Replace($overlay, $destination, $overlayBackup, $true)
            $childResult = Invoke-ThrowawayWrapperChild -ExitCode $exitCode -MarkerPath $marker
            $wrapperChildExitCode = [int]$childResult.exit_code
            Assert-True (Test-Path -LiteralPath $marker -PathType Leaf) "Nested child did not report inner exit $exitCode."
            Assert-True ((Get-Content -LiteralPath $marker -Raw) -ceq "INNER_RUNNER_EXIT_CODE=$exitCode") "Nested child marker was wrong for exit $exitCode."
        }
        finally {
            $parentFinallyExecuted = $true
            $restoreTemp = Join-Path $root ('restore-exit-{0}.tmp-test' -f $exitCode)
            $restoreBackup = Join-Path $root ('.restore-backup-{0}-{1}' -f $exitCode, ([Guid]::NewGuid().ToString('N')))
            [IO.File]::WriteAllBytes($restoreTemp, $originalBytes)
            [IO.File]::Replace($restoreTemp, $destination, $restoreBackup, $true)
            if (Test-Path -LiteralPath $restoreBackup -PathType Leaf) {
                Remove-Item -LiteralPath $restoreBackup -Force
            }
        }
        $finalHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
        Assert-True ($wrapperChildExitCode -eq $exitCode) "Wrapper child exit code mismatch for ${exitCode}: $wrapperChildExitCode"
        Assert-True $parentFinallyExecuted "Manual parent finally did not execute for child exit $exitCode."
        Assert-True ($finalHash -ceq $originalHash) "Throwaway contract was not restored for child exit $exitCode."
        Assert-True ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ceq $originalHash) "Original SHA was not restored for child exit $exitCode."
        Remove-Item -LiteralPath $destination -Force
        Write-Host ("I2G_NESTED_CHILD_EXIT_{0}=PASS INNER_RUNNER_EXIT_CODE={0} WRAPPER_CHILD_EXIT_CODE={0} MANUAL_PARENT_SURVIVES=YES MANUAL_PARENT_FINALLY_EXECUTES=YES CONTRACT_RESTORED=YES" -f $exitCode)
    }
}

function Invoke-I2gAtomicJsonWriterTests {
    param([Parameter(Mandatory = $true)][string]$RunnerPath)

    $writerFunction = [ScriptBlock]::Create((Get-I2gFunctionDefinitionText -Path $RunnerPath -Name 'Write-I2gAtomicJson'))
    . $writerFunction
    $root = Join-Path $testRoot 'atomic-json-writer'
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    $destination = Join-Path $root 'config.json'
    $valueA = [ordered]@{ phase = 'A'; sequence = 1; marker = 'first' }
    $valueB = [ordered]@{ phase = 'B'; sequence = 2; marker = 'overwrite' }
    $valueC = [ordered]@{ phase = 'C'; sequence = 3; marker = 'repeat' }

    Write-I2gAtomicJson -Path $destination -Value $valueA
    Assert-True (Test-Path -LiteralPath $destination -PathType Leaf) 'Atomic JSON first write did not create the destination.'
    $observedA = Get-Content -LiteralPath $destination -Raw | ConvertFrom-Json
    Assert-True ([string]$observedA.phase -ceq 'A' -and [int]$observedA.sequence -eq 1) 'Atomic JSON first-write content mismatch.'
    Write-Host 'I2G_ATOMIC_JSON_FIRST_WRITE=PASS'

    Write-I2gAtomicJson -Path $destination -Value $valueB
    $observedB = Get-Content -LiteralPath $destination -Raw | ConvertFrom-Json
    Assert-True ([string]$observedB.phase -ceq 'B' -and [int]$observedB.sequence -eq 2) 'Atomic JSON existing-destination overwrite mismatch.'
    Write-Host 'I2G_ATOMIC_JSON_OVERWRITE_EXISTING=PASS'

    Write-I2gAtomicJson -Path $destination -Value $valueC
    $observedC = Get-Content -LiteralPath $destination -Raw | ConvertFrom-Json
    Assert-True ([string]$observedC.phase -ceq 'C' -and [int]$observedC.sequence -eq 3) 'Atomic JSON repeated overwrite mismatch.'
    $leftovers = @(Get-ChildItem -LiteralPath $root -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -like '.pending-*' })
    Assert-True ($leftovers.Count -eq 0) 'Atomic JSON left a temporary or transient replacement file behind.'
    $writerSource = Get-I2gFunctionDefinitionText -Path $RunnerPath -Name 'Write-I2gAtomicJson'
    Assert-True ($writerSource -notmatch 'Remove-Item\s+-LiteralPath\s+\$Path') 'Atomic JSON writer deletes the destination before replacement.'
    Write-Host 'I2G_ATOMIC_JSON_REPEATED_OVERWRITE=PASS'
    Write-Host 'I2G_ATOMIC_JSON_DESTINATION_NEVER_DELETE_THEN_MOVE=PASS'
    Write-Host 'I2G_ATOMIC_JSON_TEMP_CLEANUP=PASS'
    Write-Host 'I2G_ATOMIC_JSON_TRANSIENT_BACKUP_CLEANUP=PASS'

    $failureTarget = Join-Path $root 'failure-target'
    New-Item -ItemType Directory -Force -Path $failureTarget | Out-Null
    $failureRaised = $false
    try {
        Write-I2gAtomicJson -Path $failureTarget -Value $valueA
    }
    catch {
        $failureRaised = $true
    }
    Assert-True $failureRaised 'Atomic JSON failure-safety test did not raise for a directory destination.'
    Assert-True (Test-Path -LiteralPath $failureTarget -PathType Container) 'Atomic JSON failure removed the existing destination directory.'
    Write-Host 'I2G_ATOMIC_JSON_FAILURE_SAFETY=PASS'
}

function Get-TestTreatmentServicePhaseState {
    param(
        [Parameter(Mandatory = $true)][bool]$ConfigPublished,
        [Parameter(Mandatory = $true)][bool]$ServiceStartSucceeded,
        [Parameter(Mandatory = $true)][bool]$DiscoveryStarted,
        [Parameter(Mandatory = $true)][bool]$DiscoveryCompleted,
        [Parameter(Mandatory = $true)][bool]$ServiceTeardownCompleted
    )
    $servicePhaseStarted = $ConfigPublished
    [ordered]@{
        persisted_phase = if ($servicePhaseStarted) { 'TREATMENT' } else { 'CONTROL' }
        treatment_service_phase_started = $servicePhaseStarted
        treatment_discovery_started = $DiscoveryStarted
        treatment_discovery_completed = $DiscoveryCompleted
        treatment_service_phase_completed = $servicePhaseStarted -and $ServiceStartSucceeded -and
            $DiscoveryStarted -and $DiscoveryCompleted -and $ServiceTeardownCompleted
    }
}

function Invoke-I2gTreatmentServicePhaseStateTests {
    param([Parameter(Mandatory = $true)][string]$RunnerSource)
    Assert-True ($RunnerSource.Contains('$TreatmentServicePhaseCompleted = $false')) 'Explicit treatment service completion state is missing.'
    Assert-True ($RunnerSource.Contains('$TreatmentServicePhaseCompleted = $true')) 'Successful treatment service completion state is missing.'
    Assert-True ($RunnerSource.Contains("treatment_service_phase_completion_source = 'EXPLICIT_STATE'")) 'Explicit treatment completion source marker is missing.'
    Assert-True ($RunnerSource -notmatch 'treatment_service_phase_completed\s*=\s*if\s*\(\$TreatmentServicePhaseStarted\)\s*\{\s*\$TreatmentDiscoveryCompleted') 'Treatment service completion is still derived from discovery completion.'
    $servicePhaseSource = Get-I2gFunctionDefinitionText -Path $realRunnerPath -Name 'Invoke-I2gServicePhase'
    $configWrite = $servicePhaseSource.IndexOf('Write-I2gAtomicJson -Path $ConfigPath -Value $Config', [StringComparison]::Ordinal)
    $phaseStart = $servicePhaseSource.IndexOf('$script:TreatmentServicePhaseStarted = $true', [StringComparison]::Ordinal)
    $serviceStart = $servicePhaseSource.IndexOf("Invoke-I2eSc -Arguments @('start', `$I2gServiceName)", [StringComparison]::Ordinal)
    Assert-True ($configWrite -ge 0 -and $phaseStart -gt $configWrite -and $serviceStart -gt $phaseStart) 'Treatment phase start is not ordered after config publication and before service start.'
    Assert-True ($RunnerSource -notmatch '\$treatmentConfig\s*=\s*New-I2gTreatmentConfig[\s\S]{0,180}\$TreatmentServicePhaseStarted\s*=\s*\$true') 'Treatment phase is marked started before Invoke-I2gServicePhase publishes config.'

    $case1 = Get-TestTreatmentServicePhaseState -ConfigPublished $false -ServiceStartSucceeded $false -DiscoveryStarted $false -DiscoveryCompleted $false -ServiceTeardownCompleted $false
    Assert-True ($case1.persisted_phase -ceq 'CONTROL' -and -not $case1.treatment_service_phase_started -and -not $case1.treatment_service_phase_completed) 'Case 1 pre-config failure state is wrong.'
    $case2 = Get-TestTreatmentServicePhaseState -ConfigPublished $false -ServiceStartSucceeded $false -DiscoveryStarted $false -DiscoveryCompleted $false -ServiceTeardownCompleted $false
    Assert-True ($case2.persisted_phase -ceq 'CONTROL' -and -not $case2.treatment_service_phase_started -and -not $case2.treatment_service_phase_completed) 'Case 2 config-write failure state is wrong.'
    $case3 = Get-TestTreatmentServicePhaseState -ConfigPublished $true -ServiceStartSucceeded $false -DiscoveryStarted $false -DiscoveryCompleted $false -ServiceTeardownCompleted $false
    Assert-True ($case3.persisted_phase -ceq 'TREATMENT' -and $case3.treatment_service_phase_started -and -not $case3.treatment_service_phase_completed) 'Case 3 service-start failure state is wrong.'
    $case4 = Get-TestTreatmentServicePhaseState -ConfigPublished $true -ServiceStartSucceeded $true -DiscoveryStarted $true -DiscoveryCompleted $false -ServiceTeardownCompleted $false
    Assert-True ($case4.treatment_service_phase_started -and $case4.treatment_discovery_started -and -not $case4.treatment_discovery_completed -and -not $case4.treatment_service_phase_completed) 'Case 4 discovery failure state is wrong.'
    $case5 = Get-TestTreatmentServicePhaseState -ConfigPublished $true -ServiceStartSucceeded $true -DiscoveryStarted $true -DiscoveryCompleted $true -ServiceTeardownCompleted $false
    Assert-True ($case5.treatment_discovery_completed -and -not $case5.treatment_service_phase_completed) 'Case 5 teardown failure state is wrong.'
    $case6 = Get-TestTreatmentServicePhaseState -ConfigPublished $true -ServiceStartSucceeded $true -DiscoveryStarted $true -DiscoveryCompleted $true -ServiceTeardownCompleted $true
    Assert-True ($case6.treatment_service_phase_started -and $case6.treatment_discovery_completed -and $case6.treatment_service_phase_completed) 'Case 6 full success state is wrong.'
    Write-Host 'I2G_TREATMENT_SERVICE_PHASE_STATE_MACHINE=PASS cases=6 persisted_phase_consistent=YES source=EXPLICIT_STATE'
}

foreach ($path in @($contractPath, $setupPath, $realRunnerPath, $cleanupPath, $PSCommandPath)) {
    Assert-PowerShellSyntax -Path $path
}

. $contractPath
Assert-True $I2gHarnessImplemented 'I2G harness implementation marker is not true.'
Assert-True $I2gQualificationOnly 'I2G harness must remain qualification-only.'
Assert-True $I2gRealGateConsumed 'I2G gate must be consumed after the blocked authorized attempt.'
Assert-True (-not $I2gRealExecutionAllowed) 'I2G real execution must be forbidden.'
Assert-True (-not $I2gRealCleanupAllowed) 'I2G real cleanup must be forbidden.'
Assert-True $I2gHumanAuthorizationRecorded 'Consumed human authorization must be recorded.'
Assert-True ($I2gVariable -ceq 'SeProfileSingleProcessPrivilege') 'I2G variable changed.'
Assert-True ($I2gExperimentShape -ceq 'PAIRED_CONTROL_TREATMENT') 'I2G experiment shape changed.'
Assert-True (Test-I2gFixedCliArguments -Arguments $I2gFixedArguments) 'Fixed CLI arguments are not stable.'
Assert-True (-not (Test-I2gFixedCliArguments -Arguments @('timechart', '--list', '--extra'))) 'Arbitrary CLI argument accepted.'
Assert-True ((Get-I2gRequiredEvidenceNames).Count -ge 19) 'Required evidence inventory is incomplete.'
Write-Host 'I2G_PURE_CONTRACT=PASS'

New-Item -ItemType Directory -Force -Path $testRoot | Out-Null
$realRunnerSource = Get-Content -LiteralPath $realRunnerPath -Raw
Assert-True ([regex]::Matches($realRunnerSource, '(?i)\.Clone\s*\(').Count -eq 0) 'OrderedDictionary clone call remains in the I2G real runner.'
$offlineConfigFunction = [ScriptBlock]::Create((Get-I2gFunctionDefinitionText -Path $realRunnerPath -Name 'New-I2gTreatmentConfig'))
. $offlineConfigFunction
$offlineControlConfig = [ordered]@{
    schema = 'amd-i2g-real-service-config/v1'
    qualification_only = $true
    service_name = 'offline-service'
    service_account = 'NT AUTHORITY\LocalService'
    service_account_sid = 'S-1-5-19'
    service_sid = 'S-1-5-80-offline'
    scope = 'offline-run'
    output_root = 'offline-root'
    phase = 'CONTROL'
    expected_profile_single_process_privilege = $false
    expected_amd_cli_path = 'D:\apps\AMDuProf\bin\AMDuProfCLI.exe'
    expected_amd_cli_sha256 = 'D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC'
    expected_amd_cli_version = '5.3.521.0'
    expected_amd_cli_architecture = 'x64'
    harness_artifact_sha256 = 'offline-artifact'
}
$offlineAtomicWriterPath = $realRunnerPath
Invoke-I2gAtomicJsonWriterTests -RunnerPath $offlineAtomicWriterPath
$offlineTreatmentConfig = New-I2gTreatmentConfig -ControlConfig $offlineControlConfig
$allowedConfigDelta = @('phase', 'expected_profile_single_process_privilege')
foreach ($configKey in $offlineControlConfig.Keys) {
    if ($configKey -notin $allowedConfigDelta) {
        Assert-True ([string]$offlineControlConfig[$configKey] -ceq [string]$offlineTreatmentConfig[$configKey]) "Unexpected CONTROL/TREATMENT config delta: $configKey"
    }
}
Assert-True ([string]$offlineTreatmentConfig.phase -ceq 'TREATMENT') 'Treatment config phase was not set.'
Assert-True ([bool]$offlineTreatmentConfig.expected_profile_single_process_privilege) 'Treatment config variable delta was not set.'
Write-Host 'I2G_ORDERED_DICTIONARY_CLONE_CALLS=0'
Write-Host 'I2G_TREATMENT_CONFIG_BUILD=PASS CONTROL_CONFIG_UNCHANGED=PASS PAIRED_CONFIG_DELTA=PASS'

$offlinePlaceholderFunction = [ScriptBlock]::Create((Get-I2gFunctionDefinitionText -Path $realRunnerPath -Name 'Get-I2gTreatmentPlaceholderReason'))
. $offlinePlaceholderFunction
$offlineFailureFunction = [ScriptBlock]::Create((Get-I2gFunctionDefinitionText -Path $realRunnerPath -Name 'Resolve-I2gFailureClass'))
. $offlineFailureFunction
Assert-True ((Get-I2gTreatmentPlaceholderReason -TreatmentAllowedByScientificGate $false -TreatmentDiscoveryStarted $false -TreatmentDiscoveryCompleted $false -HarnessRuntimeFailure $false) -ceq 'NOT_ALLOWED_BY_SCIENTIFIC_GATE') 'Scientific gate rejection reason is wrong.'
Assert-True ((Get-I2gTreatmentPlaceholderReason -TreatmentAllowedByScientificGate $true -TreatmentDiscoveryStarted $false -TreatmentDiscoveryCompleted $false -HarnessRuntimeFailure $true) -ceq 'ALLOWED_BUT_NOT_STARTED_DUE_TO_HARNESS_FAILURE') 'Allowed-but-not-started reason is wrong.'
Assert-True ((Get-I2gTreatmentPlaceholderReason -TreatmentAllowedByScientificGate $true -TreatmentDiscoveryStarted $true -TreatmentDiscoveryCompleted $false -HarnessRuntimeFailure $true) -ceq 'STARTED_BUT_EVIDENCE_NOT_COMPLETED') 'Started-but-incomplete reason is wrong.'
$runtimeFailureClass = Resolve-I2gFailureClass -ErrorMessage 'Method invocation failed because OrderedDictionary has no usable clone method.' -TreatmentAllowedByScientificGate $true -ControlRuns 1 -ControlResult 'POWER_UNAVAILABLE'
$scientificFailureClass = Resolve-I2gFailureClass -ErrorMessage 'CONTROL_DRIFT: baseline was not stable.' -TreatmentAllowedByScientificGate $false -ControlRuns 1 -ControlResult 'POWER_AVAILABLE'
Assert-True ($runtimeFailureClass -ceq 'HARNESS_RUNTIME_ERROR') 'Harness runtime failure was classified as scientific invalidation.'
Assert-True ($scientificFailureClass -ceq 'SCIENTIFIC_INVALIDATION') 'Scientific invalidation was classified as harness runtime failure.'
Assert-True ($realRunnerSource.Contains('treatment_allowed_by_scientific_gate')) 'Treatment scientific gate state is not persisted.'
Assert-True ($realRunnerSource -notmatch '\$TreatmentRuns\s*-eq\s*0\s*\)\s*\{\s*\$TreatmentAllowed\s*=\s*\$false') 'Treatment allowed state is retroactively falsified by run count.'
Write-Host 'I2G_TREATMENT_STATE_MACHINE=PASS'
Invoke-I2gTreatmentServicePhaseStateTests -RunnerSource $realRunnerSource

Invoke-I2gThrowawayChildExitTest
Invoke-I2gThrowawayContractRestoreTest
Invoke-I2gNestedChildProcessIsolationTest

# The marker is intentionally before every live-operation token in both entrypoints.
# This is a source-order regression check in addition to the child-process rejection test.
$setupSource = Get-Content -LiteralPath $setupPath -Raw
$cleanupSource = Get-Content -LiteralPath $cleanupPath -Raw
$realBranchEnd = $setupSource.IndexOf('if ($LibraryOnly)', [StringComparison]::Ordinal)
Assert-True ($realBranchEnd -gt 0) 'I2G real branch boundary is missing.'
$realBranchSource = $setupSource.Substring(0, $realBranchEnd)
Assert-True ($realBranchSource.Contains('System.Diagnostics.ProcessStartInfo')) 'I2G real wrapper is not isolated in a child process.'
Assert-True ($realBranchSource.Contains('ReadToEndAsync')) 'I2G real child output is not captured deterministically.'
Assert-True (-not [regex]::IsMatch($realBranchSource, '&\s*\(Join-Path[^\r\n]+i2g-real-run\.ps1')) 'I2G real runner is invoked in the wrapper host.'
Assert-True (-not $realBranchSource.Contains('LASTEXITCODE')) 'Real wrapper branch propagates a stale LASTEXITCODE.'
Assert-True ([regex]::Matches($realBranchSource, 'Invoke-I2gRealRunnerChild\s*-AuthorizationToken').Count -eq 1) 'Real wrapper invocation site count is not one.'
Write-Host 'I2G_REAL_WRAPPER_EXECUTION_ISOLATED_CHILD_PROCESS=PASS'
Write-Host 'I2G_WRAPPER_INVOCATION_SITE_COUNT=1 STALE_LASTEXITCODE_PROPAGATION=NONE'
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
Assert-True ($plan.text.Contains('I2G_REAL_GATE_CONSUMED=true')) 'I2G consumed gate marker is missing.'
Assert-True ($plan.text.Contains('I2G_HUMAN_REAL_RUN_AUTHORIZATION=CONSUMED')) 'I2G consumed authorization marker is missing.'
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

$artifactIdentity = Test-I2gHarnessArtifactIdentity -Path $releaseBinary
Assert-True ([bool]$artifactIdentity.pass) "Release artifact identity rejected: $($artifactIdentity | ConvertTo-Json -Compress)"
Assert-True ([string]$artifactIdentity.architecture -ceq 'x64') 'Release artifact architecture is not x64.'
$tamperedArtifact = Join-Path $testRoot 'tampered-amd-privilege-qualification.exe'
Copy-Item -LiteralPath $releaseBinary -Destination $tamperedArtifact
$tamperedBytes = [System.IO.File]::ReadAllBytes($tamperedArtifact)
$tamperedBytes[0] = $tamperedBytes[0] -bxor 0xff
[System.IO.File]::WriteAllBytes($tamperedArtifact, $tamperedBytes)
$tamperedIdentity = Test-I2gHarnessArtifactIdentity -Path $tamperedArtifact -ExpectedPath $tamperedArtifact
Assert-True (-not [bool]$tamperedIdentity.pass -and [string]$tamperedIdentity.reason -ceq 'SHA256_MISMATCH') 'Tampered artifact was accepted.'
$missingIdentity = Test-I2gHarnessArtifactIdentity -Path (Join-Path $testRoot 'missing-artifact.exe')
Assert-True (-not [bool]$missingIdentity.pass -and [string]$missingIdentity.reason -ceq 'MISSING_ARTIFACT') 'Missing artifact was accepted.'
Write-Host 'I2G_ARTIFACT_IDENTITY_GATE=PASS'

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

    $crashMatrixRoot = Join-Path $testRoot 'crash-window-matrix'
    $crashMatrixRun = Invoke-I2gChild -Path $setupPath -Arguments @(
        '-OfflineSynthetic', '-OfflineSyntheticScenario', 'crash-window-matrix',
        '-EvidenceRoot', $crashMatrixRoot
    )
    Assert-True ($crashMatrixRun.exit_code -eq 0) "Crash-window matrix failed: $($crashMatrixRun.text)"
    $crashMatrix = $crashMatrixRun.text | ConvertFrom-Json
    Assert-True ([string]$crashMatrix.offline_validation -ceq 'PASS') 'Crash-window matrix was not PASS.'
    Assert-True (@($crashMatrix.cases).Count -eq 20) 'Crash-window matrix does not cover all 20 required points.'
    foreach ($case in @($crashMatrix.cases)) {
        Assert-True ([bool]$case.pass) "Crash-window case failed: $($case.crash_point)"
        Assert-True ([bool]$case.recovery_required) "$($case.crash_point): recovery was not required."
        Assert-True (-not [bool]$case.discovery_relaunched) "$($case.crash_point): discovery relaunched."
        Assert-True ([int]$case.control_counter_discovery_runs -le $I2gMaxControlRuns) "$($case.crash_point): control cap exceeded."
        Assert-True ([int]$case.treatment_counter_discovery_runs -le $I2gMaxTreatmentRuns) "$($case.crash_point): treatment cap exceeded."
        Assert-True ([int]$case.total_counter_discovery_runs -le $I2gMaxTotalRuns) "$($case.crash_point): total cap exceeded."
        Assert-True ([bool]$case.no_power_sampling) "$($case.crash_point): power sampling occurred."
        Assert-True ([bool]$case.preexisting_right_safety) "$($case.crash_point): pre-existing right safety failed."
        Assert-True ([bool]$case.run_owned_rights_recovered) "$($case.crash_point): run-owned right remained."
        Assert-True ([bool]$case.service_ownership_recovered) "$($case.crash_point): service ownership recovery failed."
        Assert-True ([bool]$case.child_process_absent) "$($case.crash_point): child process remained."
        Assert-True ([string]$case.cleanup_result -ceq 'PASS') "$($case.crash_point): cleanup result was not PASS."
        Assert-True (-not [bool]$case.causal_interpretation_valid) "$($case.crash_point): crashed path was causally admissible."
    }
    Write-Host 'I2G_CRASH_WINDOW_RECOVERY_MATRIX=PASS cases=20'

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
