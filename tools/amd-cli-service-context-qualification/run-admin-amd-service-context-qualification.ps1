[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ProbePath,
    [Parameter(Mandatory = $true)][string]$ExpectedProbeSha256,
    [string]$ServiceName = 'ResourceTimelineAmdQualification',
    [string]$ExpectedCliSha256 = 'D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'qualification-common.ps1')
. (Join-Path $PSScriptRoot '..\amd-uprof-cli-spike\postprocess.ps1')

if ($ServiceName -ne 'ResourceTimelineAmdQualification') {
    throw 'the qualification service name is fixed'
}

$programData = [Environment]::GetFolderPath('CommonApplicationData')
$baseRoot = Join-Path $programData 'ResourceTimeline\qualification\amd-service-context'
$runId = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ')
$runRoot = Join-Path $baseRoot $runId
$serviceCreated = $false
$qualificationPath = Join-Path $runRoot 'AMD-SERVICE-CONTEXT.qualification-before-cleanup.json'
$servicePid = $null
$cliPid = $null
$qualification = $null
$qualificationSnapshot = $null
$wrapperError = $null
$cleanup = [ordered]@{
    service_name = $ServiceName
    service_created = $false
    stop_attempted = $false
    delete_attempted = $false
    delete_exit_code = $null
    service_delete_verified = $false
    service_process_gone = $null
    cli_process_gone = $null
    cleanup_status = 'NOT_STARTED'
    recorded_at_utc = $null
}

try {
    [void](New-Item -ItemType Directory -Path $runRoot -Force)
    Protect-ServiceRunRoot -Path $runRoot

    $adminProof = Get-AdminProof -EvidenceRoot $runRoot
    Write-JsonFile -Path (Join-Path $runRoot 'ADMIN-00-elevation-proof.json') -Value $adminProof
    if ($adminProof.whoami_groups_exit -ne 0 -or -not $adminProof.administrator_membership -or
        -not $adminProof.accepted_elevated_integrity_present -or -not $adminProof.powershell_x64 -or
        $adminProof.self_elevation_performed) {
        throw 'Administrator x64 proof failed; the service was not registered'
    }

    $probeRecord = Get-ArtifactRecord -Path $ProbePath -Role 'qualification_service_probe' `
        -ExpectedSha256 $ExpectedProbeSha256 -SignatureRequired $false
    $installRoot = Get-AmdInstallRoot
    $cliPath = Join-Path (Join-Path $installRoot 'bin') 'AMDuProfCLI.exe'
    $cliRecord = Get-ArtifactRecord -Path $cliPath -Role 'amd_vendor_cli' `
        -ExpectedSha256 $ExpectedCliSha256 -SignatureRequired $true -RequireAmdSigner $true
    $artifactPreflight = [pscustomobject]@{
        qualification_only = $true
        probe = $probeRecord
        cli = $cliRecord
        expected_cli_version = '5.3.521.0'
        cli_version_match = ($cliRecord.file_version -eq '5.3.521.0')
        all_sha_match = ($probeRecord.sha256_match -and $cliRecord.sha256_match)
        all_x64 = ($probeRecord.architecture_match -and $cliRecord.architecture_match)
        required_signatures_pass = $cliRecord.signature_requirement_passed
        preflight_pass = ($probeRecord.identity_passed -and $cliRecord.identity_passed -and
            ($cliRecord.file_version -eq '5.3.521.0'))
    }
    Write-JsonFile -Path (Join-Path $runRoot 'ARTIFACT-PREFLIGHT.json') -Value $artifactPreflight
    if (-not $artifactPreflight.preflight_pass) {
        throw 'probe or AMDuProfCLI artifact preflight failed; the service was not registered'
    }

    if (-not (Test-ServiceNameAbsent -ServiceName $ServiceName)) {
        throw 'BLOCKED_PREEXISTING_SERVICE: exact qualification service already exists'
    }
    $existingCli = @(
        Get-ExistingAmdCliProcesses -CliPath $cliPath
    )
    $existingCliEvidence = New-ProcessListEvidence -Processes $existingCli
    Write-JsonFile -Path (Join-Path $runRoot 'PREEXISTING-CLI-PROCESSES.json') `
        -Value $existingCliEvidence
    if ($existingCli.Count -gt 0) {
        throw 'BLOCKED_PREEXISTING_AMD_CLI_PROCESS: exact target is already running'
    }

    $context = [pscustomobject]@{
        captured_at_utc = Get-UtcTimestamp
        powershell_x64 = [Environment]::Is64BitProcess
        current_directory = (Get-Location).Path
        path = $env:Path
        temp = $env:TEMP
        tmp = $env:TMP
        output_root = $runRoot
        service_name = $ServiceName
        persistent_environment_mutation = $false
    }
    Write-JsonFile -Path (Join-Path $runRoot 'ADMIN-PARENT-CONTEXT.json') -Value $context

    $binaryPathName = Get-FixedServiceBinaryPathName -ProbePath $ProbePath -RunRoot $runRoot
    New-Service -Name $ServiceName -BinaryPathName $binaryPathName -DisplayName $ServiceName `
        -Description 'Resource Timeline one-shot AMD Session 0 qualification; qualification-only' `
        -StartupType Manual | Out-Null
    $serviceCreated = $true
    $cleanup.service_created = $true
    $serviceConfig = Get-CimInstance -ClassName Win32_Service -Filter "Name = '$ServiceName'" -ErrorAction Stop
    Write-JsonFile -Path (Join-Path $runRoot 'SERVICE-CONFIGURATION.json') -Value ([pscustomobject]@{
        name = $serviceConfig.Name
        start_name = $serviceConfig.StartName
        start_mode = $serviceConfig.StartMode
        path_name = $serviceConfig.PathName
        expected_account = 'LocalSystem'
        account_match = ($serviceConfig.StartName -match '(?i)^(LocalSystem|NT AUTHORITY\\SYSTEM)$')
    })
    if ($serviceConfig.StartName -notmatch '(?i)^(LocalSystem|NT AUTHORITY\\SYSTEM)$') {
        throw 'registered service did not use LocalSystem; AMD was not launched'
    }

    Start-Service -Name $ServiceName
    $deadline = [DateTime]::UtcNow.AddMilliseconds(50000)
    do {
        Start-Sleep -Milliseconds 250
        $serviceState = (Get-Service -Name $ServiceName -ErrorAction Stop).Status
        if ($serviceState -eq 'Stopped') { break }
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($serviceState -ne 'Stopped') {
        $cleanup.stop_attempted = $true
        Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
        throw 'service did not reach Stopped within the bounded service timeout'
    }

    $contextPath = Join-Path $runRoot 'SERVICE-CONTEXT.json'
    $cliResultPath = Join-Path $runRoot 'AMD-SERVICE-CLI-PROCESS-RESULT.json'
    if (-not (Test-Path -LiteralPath $contextPath -PathType Leaf) -or
        -not (Test-Path -LiteralPath $cliResultPath -PathType Leaf)) {
        throw 'service did not persist required context and CLI result evidence'
    }
    $serviceContext = Get-Content -LiteralPath $contextPath -Raw | ConvertFrom-Json
    $cliResult = Get-Content -LiteralPath $cliResultPath -Raw | ConvertFrom-Json
    $servicePid = [int]$serviceContext.process_id
    $cliPid = if ($null -ne $cliResult.target_pid) { [int]$cliResult.target_pid } else { $null }
    $sessionDirectory = Join-Path $runRoot 'timechart-output'
    $postRuntime = Invoke-AmdCliPostRuntimePipeline -SessionDirectory $sessionDirectory -Run $cliResult
    $cadence = Get-CadenceAssessment -Samples $postRuntime.parsed_package_power.samples
    $sessionIdPresent = $null -ne $serviceContext.session_id
    $integrityPresent = -not [string]::IsNullOrWhiteSpace([string]$serviceContext.integrity_level)
    $elevationPresent = $null -ne $serviceContext.token_elevated
    $contextValid = ($serviceContext.account_is_local_system -eq $true) -and
        ([string]$serviceContext.account_sid -ieq 'S-1-5-18') -and
        $sessionIdPresent -and ([int]$serviceContext.session_id -eq 0) -and
        $integrityPresent -and $elevationPresent -and ($serviceContext.token_elevated -eq $true) -and
        ([string]$serviceContext.process_architecture -eq 'x64') -and
        ([string]$serviceContext.service_name -eq $ServiceName) -and
        (-not [string]::IsNullOrWhiteSpace([string]$serviceContext.current_directory))
    $exitIsZero = ($null -ne $cliResult.target_exit_signed -and [int]$cliResult.target_exit_signed -eq 0)
    $cliPass = ($cliResult.process_started -and -not $cliResult.timeout -and
        -not $cliResult.cancelled -and $exitIsZero -and
        $cliResult.capture_complete -and -not $cliResult.harness_failed)
    if ($contextValid -and $cliPass -and $postRuntime.qualification -eq 'PASS' -and
        @($postRuntime.parsed_package_power.samples).Count -gt 0 -and $cadence.status -eq 'PASS') {
        $qualification = 'PASS'
    } elseif (-not $contextValid) {
        $qualification = 'BLOCKED_SERVICE_CONTEXT_NOT_ESTABLISHED'
    } elseif ($cliResult.timeout) {
        $qualification = 'CLI_TIMEOUT'
    } elseif ($postRuntime.qualification -ne 'PASS') {
        $qualification = $postRuntime.qualification
    } elseif ($cadence.status -ne 'PASS') {
        $qualification = 'CADENCE_INCONCLUSIVE'
    } else {
        $qualification = 'CLI_RUNTIME_FAILED'
    }

    $qualificationSnapshot = [pscustomobject]@{
        schema = 'cpu-sensor-amd-service-context/v1'
        qualification_only = $true
        service_name = $ServiceName
        service_account = $serviceContext.account
        account_sid = $serviceContext.account_sid
        session_id = $serviceContext.session_id
        service_context_valid = $contextValid
        service_context = $serviceContext
        cli_identity = $cliRecord
        cli_process_result = $cliResult
        process_started = $cliResult.process_started
        target_pid = $cliResult.target_pid
        started_at_utc = $cliResult.started_at_utc_unix_ms
        finished_at_utc = $cliResult.finished_at_utc_unix_ms
        duration_ms = $cliResult.duration_ms
        timeout = $cliResult.timeout
        target_exit_signed = $cliResult.target_exit_signed
        target_exit_hex = $cliResult.target_exit_hex
        stdout_path = $cliResult.stdout_path
        stderr_path = $cliResult.stderr_path
        stdout_bytes = $cliResult.stdout_bytes
        stderr_bytes = $cliResult.stderr_bytes
        capture_complete = $cliResult.capture_complete
        package_power = $postRuntime.parsed_package_power
        sample_cadence = $cadence
        output_artifacts = $postRuntime.output_artifacts
        qualification = $qualification
        service_completed_cleanly = ($serviceState -eq 'Stopped')
        persisted_before_service_cleanup = $true
        recorded_at_utc = Get-UtcTimestamp
    }
    Write-JsonFile -Path $qualificationPath -Value $qualificationSnapshot
} catch {
    $wrapperError = $_.Exception.Message
    if ($null -eq $qualification) {
        $qualification = 'SERVICE_HARNESS_FAILED'
    }
} finally {
    if ($serviceCreated) {
        try {
            $currentService = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
            if ($null -ne $currentService -and $currentService.Status -ne 'Stopped') {
                $cleanup.stop_attempted = $true
                Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
            }
        } catch {
            $cleanup.stop_error = $_.Exception.Message
        }
        $cleanup.delete_attempted = $true
        $scPath = Join-Path $env:SystemRoot 'System32\sc.exe'
        & $scPath delete $ServiceName | Out-Null
        $cleanup.delete_exit_code = [int]$LASTEXITCODE
        $cleanup.service_delete_verified = Test-ServiceDeleteVerified -ServiceName $ServiceName
        $cleanup.service_process_gone = if ($null -ne $servicePid) { -not (Get-ProcessAliveById -ProcessId $servicePid) } else { $null }
        $cleanup.cli_process_gone = if ($null -ne $cliPid) { -not (Get-ProcessAliveById -ProcessId $cliPid) } else { $null }
        $cleanup.cleanup_status = if ($cleanup.service_delete_verified) { 'REMOVED' } else { 'FAILED_SERVICE_REGISTRATION_REMAINS' }
        $cleanup.recorded_at_utc = Get-UtcTimestamp
        Write-JsonFile -Path (Join-Path $runRoot 'SERVICE-CLEANUP.json') -Value ([pscustomobject]$cleanup)
    }
}

$runtimeEvidence = Get-AmdCliExecutionEvidence -EvidenceRoot $runRoot
$finalSummary = [pscustomobject]@{
    schema = 'cpu-sensor-amd-service-context/v1'
    evidence_root = $runRoot
    qualification_before_cleanup = $qualificationSnapshot
    cleanup = [pscustomobject]$cleanup
    amd_runtime_executed = $runtimeEvidence.amd_runtime_executed
    process_spawned = $runtimeEvidence.process_spawned
    target_pid = $runtimeEvidence.target_pid
    amd_cli_execution_state = $runtimeEvidence.execution_state
    amd_cli_launch_evidence_path = $runtimeEvidence.launch_evidence_path
    launch_evidence_persisted = $runtimeEvidence.launch_evidence_persisted
    complete_result_persisted = $runtimeEvidence.complete_result_persisted
    runtime_evidence = $runtimeEvidence
    wrapper_error = $wrapperError
    note = 'Manual qualification wrapper; runtime state is derived from service lifecycle evidence and persisted result artifacts.'
}
if (Test-Path -LiteralPath $runRoot -PathType Container) {
    Write-JsonFile -Path (Join-Path $runRoot 'ADMIN-AMD-SERVICE-CONTEXT-SUMMARY.json') -Value $finalSummary
}
Write-Output "Evidence root: $runRoot"
Write-Output "Qualification: $qualification"
if ($cleanup.cleanup_status -eq 'FAILED_SERVICE_REGISTRATION_REMAINS') {
    throw 'qualification completed but exact service registration cleanup was not verified'
}
if ($null -ne $wrapperError) {
    throw $wrapperError
}
