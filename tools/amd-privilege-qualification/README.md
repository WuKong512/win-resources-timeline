# AMD privilege and secure IPC qualification

This is an independent, qualification-only Windows Service Broker artifact for
`AMD-PRIVILEGE-I2`. It is not the Resource Timeline collector, production
provider, installer, autostart path, or database writer.

The automated path is completely synthetic. `--synthetic` exercises the
versioned semantic protocol, bounded framing, explicit pipe-DACL policy,
client identity authorization, one-session arbitration, ownership,
cancellation, disconnect cleanup, timeout, malformed input, and the existing
header-driven package-power parser fixture. It launches only a harmless child
of the qualification binary and reports:

```text
AMD_RUNTIME_EXECUTED = false
SERVICE_REGISTRATION_COUNT = 0
SCHEDULED_TASK_REGISTRATION_COUNT = 0
SELF_ELEVATION_PERFORMED = false
AMD_INSTALLATION_MUTATED = false
AMD_REGISTRY_MUTATED = false
```

The future broker path is explicit and narrow:

```text
standard-user client
    -> scoped Windows named pipe with explicit DACL
    -> LocalService broker + Service SID
    -> broker-derived AMD installation path and fixed semantic capability
    -> broker-owned ProgramData output
    -> validated package-power parser
    -> typed response over the pipe
```

The request schema contains only:

```text
GetAmdProviderStatus
GetAmdCounterAvailability
StartAmdPowerSession { duration_ms, interval_ms }
GetAmdSessionStatus { session_id }
CancelAmdSession { session_id }
```

The broker rejects unknown fields, unknown request types, incompatible
protocol versions, oversized messages, invalid bounds, stale session IDs, and
non-owner cancellation. It never accepts an executable path, raw argv, shell
command, working directory, environment, registry path, or output path.

`GetAmdCounterAvailability` is a qualification-only, non-sampling capability.
The broker derives the validated AMD CLI and executes exactly `timechart --list`
with broker-owned stdout/stderr, a bounded timeout, and a kill-on-job-close job.
It never accepts a command or argv from the client and never starts a power
timechart. The prepared standard-user handoff is
`run-standard-user-amd-counter-discovery.ps1`; it is not run by automated tests.

## Synthetic qualification

From the repository root:

```powershell
pwsh -NoProfile -File .\tools\amd-privilege-qualification\test-qualification.ps1
```

This command does not register a service, request elevation, access the AMD
installation, or run AMD uProf.

## Historical I2 power-sampling qualification path

The following path is retained as historical I2 qualification infrastructure.
It consumed the one bounded real power-sampling gate and is not the I2B
counter-discovery handoff:

`run-standard-user-amd-privilege-client.ps1` = **I2 POWER-SAMPLING CLIENT**
**NOT THE I2B COUNTER-DISCOVERY HANDOFF**; **DO NOT RUN DURING I2B
DIFFERENTIAL**.

1. An Administrator x64 PowerShell runs `run-admin-amd-privilege-qualification.ps1`.
   It verifies the exact x64 release artifact and SHA-256, registers the fixed
   service as `NT AUTHORITY\LocalService`, enables `UNRESTRICTED` Service SID,
   preflights the registry-derived x64 AMD CLI with valid AMD Authenticode,
   creates the scoped config/output ACLs, and starts the broker. This preflight
   reads metadata only and does not start AMD uProf.
2. A normal, non-elevated x64 PowerShell runs
    `run-standard-user-amd-privilege-client.ps1`. It sends only the semantic
    status/start/status requests and holds one pipe connection for the bounded
    normal-completion run.
3. The Administrator runs `cleanup-admin-amd-privilege-qualification.ps1`.
   It stops and deletes only the fixed qualification service and records
   cleanup evidence. Qualification evidence is retained for audit; AMD
   binaries, drivers, registry installation state, and production data are not
   touched.

The real run is at most one bounded `LocalService + Service SID + Session 0`
AMD package-power session. Cancellation is qualified synthetically and is not
performed against a real AMD runtime.

## I2B human handoff: non-sampling counter discovery

This is the only authorized LocalService client sequence for the I2B
differential. It is **NON_SAMPLING**, sends only
`GetAmdCounterAvailability`, and lets the broker derive the fixed
`AMDuProfCLI.exe timechart --list` command. It does not request a power event,
duration, interval, or CSV session.

Run these commands manually, exactly once per stated shell, only after human
authorization. They are not executed by this repair or by synthetic tests:

```powershell
# Administrator x64 PowerShell — setup only; starts no AMD runtime itself
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-admin-amd-privilege-qualification.ps1'

# Normal, non-elevated, medium-integrity x64 PowerShell — I2B only
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-standard-user-amd-counter-discovery.ps1'

# Administrator x64 PowerShell — cleanup after the bounded handoff
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\cleanup-admin-amd-privilege-qualification.ps1'
```

The SYSTEM comparison is prepared as a separate qualification-only service
path below, but remains `NOT_EXECUTED / HUMAN_AUTHORIZATION_REQUIRED`. It does
not mutate the LocalService broker contract, switch the production account, or
authorize a SYSTEM fallback for production.

The fixed two-context plan is retained at
`counter-discovery-differential-plan.json`; its SYSTEM entry is prepared until
separately authorized.

## I2C human handoff: SYSTEM counter-discovery comparison

The LocalService differential side is complete real evidence at scope
`4b30b3d64b7e469cbce7c8080c84b7d4`: the fixed non-sampling
`AMDuProfCLI.exe timechart --list` operation reported
`POWER_UNAVAILABLE`. Do not rerun the LocalService side. The operator's
duplicate cleanup invocation overwrote only the single cleanup summary; the
discovery evidence and run validity remain preserved.

I2C prepares one isolated SYSTEM comparison. It uses the distinct fixed
service `ResourceTimelineAmdSystemCounterQualification`, `LocalSystem`
(`S-1-5-18`), Session 0, x64, a Service SID, broker-derived AMD CLI
discovery, and the fixed arguments `timechart --list`. It has no named-pipe
client, no sampling request, no arbitrary command surface, and no production
integration. Setup and discovery are intentionally coupled: starting the
dedicated service performs the one fixed non-sampling discovery operation.
The commands below are a future human-authorized handoff only and were not run
by this preparation:

```powershell
# Administrator x64 PowerShell — SYSTEM setup and the coupled non-sampling discovery
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-admin-amd-system-counter-qualification.ps1'

# No standard-user client and no named-pipe IPC are used for the SYSTEM comparison.

# Administrator x64 PowerShell — exact SYSTEM cleanup, after the bounded run
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\cleanup-admin-amd-system-counter-qualification.ps1'
```

The SYSTEM run must remain isolated in its own evidence root and produce
`SYSTEM-SERVICE-CONTEXT.json`, `CLI-ARTIFACT-IDENTITY.json`,
`AMD-COUNTER-DISCOVERY-LAUNCH.json`,
`AMD-COUNTER-DISCOVERY-RESULT.json`, bounded stdout/stderr, and unique
`SYSTEM-CLEANUP-RESULT-<timestamp>-<id>.json` cleanup evidence. The cleanup
wrapper never overwrites a previous cleanup attempt and never kills an
unrelated process. A SYSTEM result can inform the privilege differential; it
cannot select LocalSystem as the production account.

## Frozen future-run artifact

The completed LocalService real run used the historical artifact below. Its
hash remains immutable in the evidence and is not rewritten by the I2C build:

```text
LOCAL_SERVICE_REAL_ARTIFACT_SHA256 = C9973BAAA01AF3C2673D8C70D8C7E626C577642505E6DFF7BA3C6026DEA63FB1
LOCAL_SERVICE_ARTIFACT_STATUS = historical / used by completed LocalService run
```

The dedicated SYSTEM comparison wrapper is pinned to a new offline release
artifact; changing that binary requires rebuilding and recording a new hash
before the human-authorized SYSTEM setup:

```text
path = tools/amd-privilege-qualification/target/release/amd-privilege-qualification.exe
architecture = x64
build_mode = release / cargo --offline
sha256 = 9E5A012B0A95C84DD28CD607D99EF43C9BC4D700683F33890CDE6C2108794AC3
system_wrapper = run-admin-amd-system-counter-qualification.ps1
system_artifact_status = new offline SYSTEM-comparison artifact / not executed
broker_authenticode = NotSigned; exact SHA-256 is required by the SYSTEM wrapper
```

The existing LocalService wrappers remain pinned to the historical C9973...
hash and therefore fail closed against the newly built SYSTEM-comparison
binary; do not use them to retry the consumed LocalService run. The active I2B
handoff remains the non-sampling LocalService sequence above, while the I2C
SYSTEM handoff uses only the dedicated Administrator setup and cleanup
wrappers. The older sampling wrapper must not be substituted for either
counter-discovery handoff:

`run-standard-user-amd-privilege-client.ps1` remains preserved as the
historical **I2 POWER-SAMPLING CLIENT**, not an active I2B command.
