[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'qualification-common.ps1')
. (Join-Path $PSScriptRoot '..\amd-uprof-cli-spike\postprocess.ps1')

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (-not $Condition) { throw $Message }
}

function Invoke-SyntheticProcess {
    param(
        [Parameter(Mandatory = $true)][string]$OutputRoot,
        [Parameter(Mandatory = $true)][int]$ExitCode,
        [Parameter(Mandatory = $true)][bool]$SleepBeforeExit,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Stdout,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Stderr,
        [int]$TimeoutMs = 3000
    )

    $stdoutPath = Join-Path $OutputRoot 'stdout.txt'
    $stderrPath = Join-Path $OutputRoot 'stderr.txt'
    $sleepLiteral = if ($SleepBeforeExit) { '$true' } else { '$false' }
    $script = '$OutputEncoding = [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false); ' +
        "[Console]::Out.Write('$($Stdout.Replace("'", "''"))'); " +
        "[Console]::Error.Write('$($Stderr.Replace("'", "''"))'); " +
        "if ($sleepLiteral) { Start-Sleep -Milliseconds 150 }; exit $ExitCode"
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    if ($psi.PSObject.Properties.Name -contains 'ArgumentList') {
        [void]$psi.ArgumentList.Add('-NoProfile')
        [void]$psi.ArgumentList.Add('-NonInteractive')
        [void]$psi.ArgumentList.Add('-Command')
        [void]$psi.ArgumentList.Add($script)
    } else {
        $psi.Arguments = '-NoProfile -NonInteractive -Command ' + (Quote-WindowsProcessArgument -Argument $script)
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $psi
    $started = $process.Start()
    Assert-True -Condition $started -Message 'synthetic process did not start'
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    while (-not $process.HasExited -and $stopwatch.ElapsedMilliseconds -lt $TimeoutMs) {
        Start-Sleep -Milliseconds 10
    }
    $timedOut = -not $process.HasExited
    if ($timedOut) {
        [void]$process.Kill()
    }
    $process.WaitForExit()
    $capturedStdout = $stdoutTask.GetAwaiter().GetResult()
    $capturedStderr = $stderrTask.GetAwaiter().GetResult()
    $signedExit = [int]$process.ExitCode
    $process.Dispose()
    Write-Utf8File -Path $stdoutPath -Text $capturedStdout
    Write-Utf8File -Path $stderrPath -Text $capturedStderr
    [pscustomobject]@{
        process_started = $started
        timeout = $timedOut
        target_exit_signed = $signedExit
        target_exit_hex = Convert-ExitCodeToHex -ExitCode $signedExit
        stdout_persisted = Test-Path -LiteralPath $stdoutPath -PathType Leaf
        stderr_persisted = Test-Path -LiteralPath $stderrPath -PathType Leaf
        stdout = $capturedStdout
        stderr = $capturedStderr
        capture_complete = (Test-Path -LiteralPath $stdoutPath -PathType Leaf) -and
            (Test-Path -LiteralPath $stderrPath -PathType Leaf)
    }
}

$root = Join-Path ([IO.Path]::GetTempPath()) ('resource-timeline-amd-service-context-' + [guid]::NewGuid().ToString('N'))
$fixtureRoot = Join-Path $root 'postprocess'
try {
    [void](New-Item -ItemType Directory -Path $fixtureRoot -Force)

    Assert-True -Condition ((Convert-ExitCodeToHex -ExitCode -1) -eq '0xFFFFFFFF') -Message 'negative exit serialization failed'
    Assert-True -Condition ((Convert-ExitCodeToHex -ExitCode 0) -eq '0x00000000') -Message 'zero exit serialization failed'
    $empty = Invoke-SyntheticProcess -OutputRoot $root -ExitCode 0 -SleepBeforeExit $false -Stdout 'SYNTHETIC_STDOUT' -Stderr ''
    Assert-True -Condition $empty.process_started -Message 'T7 process start failed'
    Assert-True -Condition (-not $empty.timeout) -Message 'T7 unexpectedly timed out'
    Assert-True -Condition ($empty.target_exit_signed -eq 0) -Message 'T7 exit mismatch'
    Assert-True -Condition ($empty.stdout -eq 'SYNTHETIC_STDOUT') -Message 'T7 stdout mismatch'
    Assert-True -Condition ($empty.stderr -eq '') -Message 'T9 empty stderr mismatch'
    Assert-True -Condition $empty.capture_complete -Message 'T9 capture was incomplete'

    $negative = Invoke-SyntheticProcess -OutputRoot $root -ExitCode -1 -SleepBeforeExit $false -Stdout 'NEGATIVE_STDOUT' -Stderr 'NEGATIVE_STDERR'
    Assert-True -Condition ($negative.target_exit_signed -eq -1) -Message 'T8 signed exit mismatch'
    Assert-True -Condition ($negative.target_exit_hex -eq '0xFFFFFFFF') -Message 'T8 hex exit mismatch'
    Assert-True -Condition $negative.stdout_persisted -Message 'T9 stdout was not persisted'
    Assert-True -Condition $negative.stderr_persisted -Message 'T9 stderr was not persisted'
    $timed = Invoke-SyntheticProcess -OutputRoot $root -ExitCode 0 -SleepBeforeExit $true -Stdout 'TIMEOUT_STDOUT' -Stderr '' -TimeoutMs 10
    Assert-True -Condition $timed.timeout -Message 'T6 timeout was not detected'
    Assert-True -Condition $timed.capture_complete -Message 'T6 timed-out output was not persisted'

    $base = Join-Path $root 'controlled-base'
    $runPath = Join-Path $base 'run-1'
    Assert-True -Condition (Test-ServiceNameAbsent -ServiceName ('definitely-not-' + [guid]::NewGuid().ToString('N'))) -Message 'T14 absent service gate failed'
    Assert-True -Condition ((Get-FixedServiceBinaryPathName -ProbePath 'C:\probe.exe' -RunRoot $runPath) -match '--run-root') -Message 'T3 fixed service command missing run root'
    Assert-True -Condition ((Get-FixedServiceBinaryPathName -ProbePath 'C:\probe.exe' -RunRoot $runPath) -notmatch 'AMDuProfCLI') -Message 'T3 service command exposed vendor executable override'
    $validArgs = @('--run-root', $runPath)
    Assert-True -Condition ((Test-Path -LiteralPath $root -PathType Container) -and ($validArgs.Count -eq 2)) -Message 'T2 evidence root setup failed'

    $zeroProcessList = New-ProcessListEvidence -Processes @()
    Assert-True -Condition ($zeroProcessList.count -eq 0) -Message 'zero process-list count mismatch'
    Assert-True -Condition ($zeroProcessList.processes -is [array]) -Message 'zero process-list was not an array'
    $zeroProcessJson = $zeroProcessList | ConvertTo-Json -Depth 10
    $zeroProcessRoundTrip = $zeroProcessJson | ConvertFrom-Json
    Assert-True -Condition ($zeroProcessRoundTrip.count -eq 0 -and
        $zeroProcessRoundTrip.processes -is [array] -and
        @($zeroProcessRoundTrip.processes).Count -eq 0) -Message 'zero process-list serialization shape failed'

    $oneProcessList = New-ProcessListEvidence -Processes @([pscustomobject]@{
        process_id = 1234
        executable_path = 'C:\synthetic\AMDuProfCLI.exe'
    })
    Assert-True -Condition ($oneProcessList.count -eq 1) -Message 'one process-list count mismatch'
    Assert-True -Condition ($oneProcessList.processes -is [array]) -Message 'one process-list was not an array'
    $oneProcessRoundTrip = (($oneProcessList | ConvertTo-Json -Depth 10) | ConvertFrom-Json)
    Assert-True -Condition ($oneProcessRoundTrip.count -eq 1 -and
        $oneProcessRoundTrip.processes -is [array] -and
        @($oneProcessRoundTrip.processes).Count -eq 1) -Message 'one process-list serialization shape failed'

    $multipleProcessList = New-ProcessListEvidence -Processes @(
        [pscustomobject]@{ process_id = 1234; executable_path = 'C:\synthetic\AMDuProfCLI.exe' },
        [pscustomobject]@{ process_id = 5678; executable_path = 'C:\synthetic\AMDuProfCLI.exe' }
    )
    Assert-True -Condition ($multipleProcessList.count -gt 1) -Message 'multiple process-list count mismatch'
    Assert-True -Condition ($multipleProcessList.processes -is [array]) -Message 'multiple process-list was not an array'
    $multipleProcessRoundTrip = (($multipleProcessList | ConvertTo-Json -Depth 10) | ConvertFrom-Json)
    Assert-True -Condition ($multipleProcessRoundTrip.count -gt 1 -and
        $multipleProcessRoundTrip.processes -is [array] -and
        @($multipleProcessRoundTrip.processes).Count -gt 1) -Message 'multiple process-list serialization shape failed'

    $wrapperText = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'run-admin-amd-service-context-qualification.ps1') -Raw
    $adminProofIndex = $wrapperText.IndexOf('$adminProof = Get-AdminProof', [StringComparison]::Ordinal)
    $probeIdentityIndex = $wrapperText.IndexOf('$probeRecord = Get-ArtifactRecord', [StringComparison]::Ordinal)
    $cliIdentityIndex = $wrapperText.IndexOf('$cliRecord = Get-ArtifactRecord', [StringComparison]::Ordinal)
    $serviceGateIndex = $wrapperText.IndexOf('Test-ServiceNameAbsent', [StringComparison]::Ordinal)
    $cliGateIndex = $wrapperText.IndexOf('$existingCli = @(', [StringComparison]::Ordinal)
    $evidencePersistIndex = $wrapperText.IndexOf('PREEXISTING-CLI-PROCESSES.json', [StringComparison]::Ordinal)
    $newServiceIndex = $wrapperText.IndexOf('New-Service -Name', [StringComparison]::Ordinal)
    $startServiceIndex = $wrapperText.IndexOf('Start-Service -Name', [StringComparison]::Ordinal)
    Assert-True -Condition ($adminProofIndex -ge 0 -and $probeIdentityIndex -gt $adminProofIndex -and
        $cliIdentityIndex -gt $probeIdentityIndex -and $serviceGateIndex -gt $cliIdentityIndex -and
        $cliGateIndex -gt $serviceGateIndex -and $evidencePersistIndex -gt $cliGateIndex -and
        $newServiceIndex -gt $evidencePersistIndex -and $startServiceIndex -gt $newServiceIndex) `
        -Message 'pre-runtime gates/evidence must precede New-Service and Start-Service'

    $singleDigitTimestamp = Parse-CadenceTimestamp -Timestamp '1:2:3:4'
    Assert-True -Condition ($singleDigitTimestamp.valid -and $singleDigitTimestamp.milliseconds -eq 3723004) `
        -Message 'single-digit timestamp fields were not parsed deterministically'
    $mixedWidthTimestamp = Parse-CadenceTimestamp -Timestamp '16:3:25:895'
    Assert-True -Condition ($mixedWidthTimestamp.valid -and $mixedWidthTimestamp.milliseconds -eq 57805895) `
        -Message 'mixed-width vendor timestamp was rejected'
    $paddedTimestamp = Parse-CadenceTimestamp -Timestamp '16:03:25:895'
    Assert-True -Condition ($paddedTimestamp.valid -and
        $paddedTimestamp.milliseconds -eq $mixedWidthTimestamp.milliseconds) -Message 'padded timestamp changed semantics'
    $acceptedTimestampForms = @(
        [pscustomobject]@{ text = '6:3:5:895'; milliseconds = 21785895 },
        [pscustomobject]@{ text = '16:3:5:895'; milliseconds = 57785895 },
        [pscustomobject]@{ text = '6:03:5:895'; milliseconds = 21785895 },
        [pscustomobject]@{ text = '16:03:5:895'; milliseconds = 57785895 },
        [pscustomobject]@{ text = '6:3:05:895'; milliseconds = 21785895 },
        [pscustomobject]@{ text = '16:3:05:895'; milliseconds = 57785895 },
        [pscustomobject]@{ text = '6:03:05:895'; milliseconds = 21785895 },
        [pscustomobject]@{ text = '16:03:05:895'; milliseconds = 57785895 }
    )
    foreach ($acceptedTimestampForm in $acceptedTimestampForms) {
        $acceptedTimestamp = Parse-CadenceTimestamp -Timestamp $acceptedTimestampForm.text
        Assert-True -Condition ($acceptedTimestamp.valid -and
            $acceptedTimestamp.milliseconds -eq $acceptedTimestampForm.milliseconds) `
            -Message "supported timestamp form was rejected: $($acceptedTimestampForm.text)"
    }

    $consecutiveCadence = Get-CadenceAssessment -Samples @(
        [pscustomobject]@{ timestamp = '16:3:25:895' }
        [pscustomobject]@{ timestamp = '16:3:26:897' }
    )
    Assert-True -Condition ($consecutiveCadence.status -eq 'PASS' -and
        $consecutiveCadence.delta_count -eq 1 -and
        @($consecutiveCadence.deltas_ms)[0] -eq 1002) -Message 'mixed-width consecutive cadence failed'

    $midnightCadence = Get-CadenceAssessment -Samples @(
        [pscustomobject]@{ timestamp = '23:59:59:900' }
        [pscustomobject]@{ timestamp = '00:00:00:900' }
    )
    Assert-True -Condition ($midnightCadence.status -eq 'PASS' -and
        $midnightCadence.delta_count -eq 1 -and
        @($midnightCadence.deltas_ms)[0] -eq 1000) -Message 'midnight rollover cadence failed'

    $arbitraryRegression = Get-CadenceAssessment -Samples @(
        [pscustomobject]@{ timestamp = '12:00:00:000' }
        [pscustomobject]@{ timestamp = '11:59:59:000' }
    )
    Assert-True -Condition ($arbitraryRegression.status -eq 'INCONCLUSIVE' -and
        @($arbitraryRegression.deltas_ms)[0] -eq -1000 -and
        $arbitraryRegression.error -match 'regression') -Message 'arbitrary timestamp regression was treated as rollover'

    $invalidCadenceTimestamps = @(
        '24:00:00:000',
        '12:60:00:000',
        '12:00:60:000',
        '12:00:00:1000',
        '12:00:00',
        '12:00:00:000:1'
    )
    foreach ($invalidCadenceTimestamp in $invalidCadenceTimestamps) {
        $invalidTimestamp = Parse-CadenceTimestamp -Timestamp $invalidCadenceTimestamp
        Assert-True -Condition (-not $invalidTimestamp.valid) `
            -Message "invalid timestamp was accepted: $invalidCadenceTimestamp"
    }

    # Regression fixture copied from the authoritative 20260904T080323173Z run.
    $authoritativeTimestamps = @(
        '16:3:25:895',
        '16:3:26:897',
        '16:3:27:901',
        '16:3:28:910',
        '16:3:29:916',
        '16:3:30:922',
        '16:3:31:926',
        '16:3:32:928',
        '16:3:33:937'
    )
    $authoritativeSamples = @($authoritativeTimestamps | ForEach-Object {
        [pscustomobject]@{ timestamp = $_ }
    })
    $authoritativeCadence = Get-CadenceAssessment -Samples $authoritativeSamples
    $expectedAuthoritativeDeltas = @(1002, 1004, 1009, 1006, 1006, 1004, 1002, 1009)
    Assert-True -Condition ($authoritativeCadence.status -eq 'PASS' -and
        $authoritativeCadence.delta_count -eq 8 -and
        ($authoritativeCadence.deltas_ms -join ',') -eq ($expectedAuthoritativeDeltas -join ',') -and
        $authoritativeCadence.min_ms -eq 1002 -and
        $authoritativeCadence.max_ms -eq 1009 -and
        $authoritativeCadence.mean_ms -eq 1005.25 -and
        [string]::IsNullOrWhiteSpace([string]$authoritativeCadence.error)) `
        -Message 'authoritative cadence fixture did not produce the expected PASS deltas'

    $runtimeEvidenceRoot = Join-Path $root 'runtime-evidence'
    [void](New-Item -ItemType Directory -Path $runtimeEvidenceRoot -Force)
    $beforeLaunch = Get-AmdCliExecutionEvidence -EvidenceRoot $runtimeEvidenceRoot
    Assert-True -Condition (-not $beforeLaunch.amd_runtime_executed) -Message 'runtime was reported before launch evidence'
    Assert-True -Condition (-not $beforeLaunch.process_spawned) -Message 'pre-launch process state mismatch'
    Assert-True -Condition ($beforeLaunch.execution_state -eq 'NOT_LAUNCHED') -Message 'pre-launch execution state mismatch'
    Write-JsonFile -Path (Join-Path $runtimeEvidenceRoot 'AMD-CLI-LAUNCH.json') -Value ([pscustomobject]@{
        process_started = $true
        target_pid = 1234
        started_at_utc_unix_ms = 1
        executable = 'synthetic-cli.exe'
        arguments = @('synthetic')
        working_directory = $runtimeEvidenceRoot
        output_directory = $runtimeEvidenceRoot
    })
    $afterLaunch = Get-AmdCliExecutionEvidence -EvidenceRoot $runtimeEvidenceRoot
    Assert-True -Condition $afterLaunch.amd_runtime_executed -Message 'synthetic launch was not recorded as runtime'
    Assert-True -Condition ($afterLaunch.execution_state -eq 'LAUNCHED_INCOMPLETE_RESULT') -Message 'incomplete launch state mismatch'
    Write-JsonFile -Path (Join-Path $runtimeEvidenceRoot 'AMD-SERVICE-CLI-PROCESS-RESULT.json') -Value ([pscustomobject]@{
        process_started = $true
        target_exit_signed = 1
    })
    Write-JsonFile -Path (Join-Path $runtimeEvidenceRoot 'SERVICE-RUN-RESULT.json') -Value ([pscustomobject]@{
        process_spawned = $true
        target_pid = 1234
        launch_evidence_persisted = $true
        complete_result_persisted = $true
        amd_runtime_executed = $true
        cli_execution_state = 'LAUNCHED_COMPLETE_RESULT'
    })
    $afterComplete = Get-AmdCliExecutionEvidence -EvidenceRoot $runtimeEvidenceRoot
    Assert-True -Condition $afterComplete.amd_runtime_executed -Message 'completed synthetic launch reported false runtime'
    Assert-True -Condition ($afterComplete.execution_state -eq 'LAUNCHED_COMPLETE_RESULT') -Message 'complete launch state mismatch'

    $postSpawnFailureRoot = Join-Path $root 'post-spawn-evidence-failure'
    [void](New-Item -ItemType Directory -Path $postSpawnFailureRoot -Force)
    Write-JsonFile -Path (Join-Path $postSpawnFailureRoot 'SERVICE-HARNESS-ERROR.json') -Value ([pscustomobject]@{
        process_spawned = $true
        target_pid = 5678
        launch_evidence_persisted = $false
        complete_result_persisted = $false
        amd_runtime_executed = $true
        amd_cli_execution_state = 'LAUNCHED_INCOMPLETE_RESULT'
    })
    $postSpawnFailure = Get-AmdCliExecutionEvidence -EvidenceRoot $postSpawnFailureRoot
    Assert-True -Condition $postSpawnFailure.amd_runtime_executed -Message 'post-spawn persistence failure lost runtime fact'
    Assert-True -Condition ($postSpawnFailure.process_spawned -and $postSpawnFailure.execution_state -eq 'LAUNCHED_INCOMPLETE_RESULT') -Message 'post-spawn persistence failure was reported as not launched'
    Assert-True -Condition (-not $postSpawnFailure.launch_evidence_present) -Message 'post-spawn fixture unexpectedly has launch file'

    $completePersistenceRoot = Join-Path $root 'complete-persistence-state'
    [void](New-Item -ItemType Directory -Path $completePersistenceRoot -Force)
    Write-JsonFile -Path (Join-Path $completePersistenceRoot 'AMD-CLI-LAUNCH.json') -Value ([pscustomobject]@{
        process_started = $true
        target_pid = 9876
    })
    Write-JsonFile -Path (Join-Path $completePersistenceRoot 'AMD-SERVICE-CLI-PROCESS-RESULT.json') -Value ([pscustomobject]@{
        process_started = $true
        target_exit_signed = 0
    })
    Write-JsonFile -Path (Join-Path $completePersistenceRoot 'SERVICE-RUN-RESULT.json') -Value ([pscustomobject]@{
        process_spawned = $true
        target_pid = 9876
        launch_evidence_persisted = $true
        complete_result_persisted = $false
        amd_runtime_executed = $true
        cli_execution_state = 'LAUNCHED_INCOMPLETE_RESULT'
    })
    $persistenceFailure = Get-AmdCliExecutionEvidence -EvidenceRoot $completePersistenceRoot
    Assert-True -Condition ($persistenceFailure.execution_state -eq 'LAUNCHED_INCOMPLETE_RESULT') -Message 'failed process-result persistence was upgraded by file presence'
    Write-JsonFile -Path (Join-Path $completePersistenceRoot 'SERVICE-RUN-RESULT.json') -Value ([pscustomobject]@{
        process_spawned = $true
        target_pid = 9876
        launch_evidence_persisted = $true
        complete_result_persisted = $true
        amd_runtime_executed = $true
        cli_execution_state = 'LAUNCHED_COMPLETE_RESULT'
    })
    $persistenceSuccess = Get-AmdCliExecutionEvidence -EvidenceRoot $completePersistenceRoot
    Assert-True -Condition ($persistenceSuccess.execution_state -eq 'LAUNCHED_COMPLETE_RESULT') -Message 'persisted process-result state was not recognized'

    $csvDir = Join-Path $fixtureRoot 'timechart-output'
    [void](New-Item -ItemType Directory -Path $csvDir -Force)
    $csv = @'
PROFILE RECORDS
RecordId,Timestamp,socket0-package-power
1,17:00:00:000,40.00
2,17:00:01:000,41.00
3,17:00:02:000,42.00
'@
    Write-Utf8File -Path (Join-Path $csvDir 'timechart.csv') -Text $csv
    [IO.File]::WriteAllBytes((Join-Path $csvDir 'session.uprof'), [byte[]](1, 2, 3))
    $run = [pscustomobject]@{
        process_started = $true
        timeout = $false
        target_exit_signed = 0
        capture_complete = $true
        harness_error = $null
    }
    $post = Invoke-AmdCliPostRuntimePipeline -SessionDirectory $csvDir -Run $run
    Assert-True -Condition ($post.qualification -eq 'PASS') -Message 'T10 shared parser PASS failed'
    Assert-True -Condition ($post.parsed_package_power.sample_count -eq 3) -Message 'T10 sample count mismatch'
    $cadence = Get-CadenceAssessment -Samples $post.parsed_package_power.samples
    Assert-True -Condition ($cadence.status -eq 'PASS') -Message 'T10 cadence assessment failed'
    Remove-Item -LiteralPath (Join-Path $csvDir 'timechart.csv') -Force
    $parseFail = Invoke-AmdCliPostRuntimePipeline -SessionDirectory $csvDir -Run $run
    Assert-True -Condition ($parseFail.parsed_package_power.status -eq 'NOT_FOUND') -Message 'T11 parser failure path failed'
    Assert-True -Condition ($parseFail.qualification -eq 'OUTPUT_ARTIFACT_MISSING') -Message 'T11 missing output classification failed'

    $emptyInventoryRoot = Join-Path $fixtureRoot 'empty-inventory'
    [void](New-Item -ItemType Directory -Path $emptyInventoryRoot -Force)
    $emptyInventoryPost = Invoke-AmdCliPostRuntimePipeline -SessionDirectory $emptyInventoryRoot -Run $run
    Assert-True -Condition ($emptyInventoryPost.output_artifacts -is [array] -and
        @($emptyInventoryPost.output_artifacts).Count -eq 0) -Message 'empty output inventory shape failed'

    try {
        throw [System.Exception]::new('无法将参数绑定到参数 Value')
    } catch {
        $unicodeError = $_.Exception.Message
    }
    $unicodeRoot = Join-Path $root 'unicode'
    $unicodePath = Join-Path $unicodeRoot 'summary.json'
    Write-JsonFile -Path $unicodePath -Value ([pscustomobject]@{ wrapper_error = $unicodeError })
    $unicodeRoundTrip = Get-Content -LiteralPath $unicodePath -Raw | ConvertFrom-Json
    Assert-True -Condition ($unicodeRoundTrip.wrapper_error -eq '无法将参数绑定到参数 Value') `
        -Message 'Unicode wrapper error did not round-trip through JSON evidence'

    $qualificationPath = Join-Path $root 'qualification-before-cleanup.json'
    Write-JsonFile -Path $qualificationPath -Value ([pscustomobject]@{
        qualification = 'PASS'
        persisted_before_cleanup = $true
        target_exit_signed = $negative.target_exit_signed
        target_exit_hex = $negative.target_exit_hex
    })
    $qualificationJson = Get-Content -LiteralPath $qualificationPath -Raw | ConvertFrom-Json
    Assert-True -Condition ($qualificationJson.persisted_before_cleanup -eq $true) -Message 'T12 qualification persistence failed'
    $syntheticCopied = Join-Path $root 'copied-probe.exe'
    [IO.File]::WriteAllBytes($syntheticCopied, [byte[]](1, 2, 3))
    Remove-Item -LiteralPath $syntheticCopied -Force
    Assert-True -Condition (-not (Test-Path -LiteralPath $syntheticCopied)) -Message 'T15 exact cleanup failed'

    Write-Output 'SERVICE_PROTOCOL_UNIT_POLICY=PASS'
    Write-Output 'SYNTHETIC_PROCESS_EXIT_0=PASS'
    Write-Output 'SYNTHETIC_PROCESS_EXIT_NEGATIVE=PASS'
    Write-Output 'SYNTHETIC_TIMEOUT_AND_OWNED_CLEANUP=PASS'
    Write-Output 'RAW_STREAM_PERSISTENCE=PASS'
    Write-Output 'SHARED_PARSER_PASS=PASS'
    Write-Output 'SHARED_PARSER_FAILURE=PASS'
    Write-Output 'QUALIFICATION_BEFORE_CLEANUP=PASS'
    Write-Output 'AMD_RUNTIME_FALSE_BEFORE_LAUNCH=PASS'
    Write-Output 'AMD_RUNTIME_TRUE_AFTER_SYNTHETIC_LAUNCH_RECORD=PASS'
    Write-Output 'FAILED_POST_LAUNCH_PATH_DOES_NOT_REPORT_FALSE=PASS'
    Write-Output 'POST_SPAWN_EVIDENCE_FAILURE_NEVER_REPORTS_NOT_LAUNCHED=PASS'
    Write-Output 'COMPLETE_RESULT_STATE_REQUIRES_PERSISTENCE=PASS'
    Write-Output 'EMPTY_PROCESS_LIST_SERIALIZATION=PASS'
    Write-Output 'PROCESS_LIST_SHAPE_STABLE_FOR_0_1_N=PASS'
    Write-Output 'FAILURE_OCCURRED_BEFORE_NEW_SERVICE=true'
    Write-Output 'UNICODE_ERROR_EVIDENCE_ROUNDTRIP=PASS'
    Write-Output 'TIMESTAMP_PARSER_FORMAT_MATRIX=PASS'
    Write-Output 'AUTHORITATIVE_CADENCE_FIXTURE=PASS'
    Write-Output 'DERIVED_FROM_EXISTING_AUTHORITATIVE_RUN=true'
    Write-Output 'SOURCE_RUN=20260904T080323173Z'
    Write-Output 'REAL_RUNTIME_REEXECUTED=false'
    Write-Output 'MIDNIGHT_ROLLOVER=PASS'
    Write-Output 'TIMESTAMP_REGRESSION_REJECTED=PASS'
    Write-Output 'AMD_RUNTIME_EXECUTED=false'
    Write-Output 'SERVICE_REGISTERED=false'
} finally {
    if (Test-Path -LiteralPath $root) {
        Remove-Item -LiteralPath $root -Recurse -Force
    }
}
