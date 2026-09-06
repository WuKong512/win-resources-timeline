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

## HISTORICAL / SUPERSEDED I2C human handoff: SYSTEM counter-discovery comparison

The LocalService differential side is complete real evidence at scope
`4b30b3d64b7e469cbce7c8080c84b7d4`: the fixed non-sampling
`AMDuProfCLI.exe timechart --list` operation reported
`POWER_UNAVAILABLE`. Do not rerun the LocalService side. The operator's
duplicate cleanup invocation overwrote only the single cleanup summary; the
discovery evidence and run validity remain preserved.

I2C prepared one isolated SYSTEM comparison. It used the distinct fixed
service `ResourceTimelineAmdSystemCounterQualification`, `LocalSystem`
(`S-1-5-18`), Session 0, x64, a Service SID, broker-derived AMD CLI
discovery, and the fixed arguments `timechart --list`. It has no named-pipe
client, no sampling request, no arbitrary command surface, and no production
integration. Setup and discovery are intentionally coupled: starting the
dedicated service performs the one fixed non-sampling discovery operation.
The one authorized comparison completed at scope
`091a72e1d38341ca9eca0877b1625082` and reported `POWER_AVAILABLE`. Do not run
the commands below again; they are retained as the historical execution shape
only:

```powershell
# HISTORICAL command shape — already consumed; do not rerun
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-admin-amd-system-counter-qualification.ps1'

# No standard-user client and no named-pipe IPC are used for the SYSTEM comparison.

# Historical cleanup command shape — already consumed; do not rerun
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\cleanup-admin-amd-system-counter-qualification.ps1'
```

The completed SYSTEM run remained isolated in its own evidence root and
produced
`SYSTEM-SERVICE-CONTEXT.json`, `CLI-ARTIFACT-IDENTITY.json`,
`AMD-COUNTER-DISCOVERY-LAUNCH.json`,
`AMD-COUNTER-DISCOVERY-RESULT.json`, bounded stdout/stderr, and unique
`SYSTEM-CLEANUP-RESULT-<timestamp>-<id>.json` cleanup evidence. The cleanup
wrapper never overwrote a previous cleanup attempt and never killed an
unrelated process. The SYSTEM result informs the privilege differential; it
does not select LocalSystem as the production account.

```text
SYSTEM_COUNTER_DISCOVERY = REAL_POWER_AVAILABLE
SYSTEM_SCOPE = 091a72e1d38341ca9eca0877b1625082
SYSTEM_CLEANUP = PASS
SYSTEM_SERVICE_REGISTRATION_FINAL = absent
SYSTEM_BROKER_PROCESS_FINAL = 0
SYSTEM_REAL_RUN_CONSUMED = true
```

## Frozen future-run artifact

The completed LocalService real run used the historical artifact below. Its
hash remains immutable in the evidence and is not rewritten by the I2C build:

```text
LOCAL_SERVICE_REAL_ARTIFACT_SHA256 = C9973BAAA01AF3C2673D8C70D8C7E626C577642505E6DFF7BA3C6026DEA63FB1
LOCAL_SERVICE_ARTIFACT_STATUS = historical / used by completed LocalService run
```

The dedicated SYSTEM comparison wrapper is pinned to the offline release
artifact used by the completed SYSTEM comparison. Changing that binary
requires rebuilding and recording a new hash before any future comparison:

```text
path = tools/amd-privilege-qualification/target/release/amd-privilege-qualification.exe
architecture = x64
build_mode = release / cargo --offline
sha256 = 9E5A012B0A95C84DD28CD607D99EF43C9BC4D700683F33890CDE6C2108794AC3
system_wrapper = run-admin-amd-system-counter-qualification.ps1
system_artifact_status = historical artifact used by completed SYSTEM comparison
broker_authenticode = NotSigned; exact SHA-256 is required by the SYSTEM wrapper
```

The existing LocalService wrappers remain pinned to the historical C9973...
hash and therefore fail closed against the SYSTEM-comparison binary; do not
use them to retry the consumed LocalService run. The older sampling wrapper
must not be substituted for either counter-discovery path:

`run-standard-user-amd-privilege-client.ps1` remains preserved as the
historical **I2 POWER-SAMPLING CLIENT**, not an active I2B command.

## I2E service-SID SeSystemProfile minimum-variable experiment

I2E prepares, but does not execute, a paired control/treatment experiment
against the real LocalService counter-availability differential. It keeps the
service account as NT AUTHORITY\LOCAL SERVICE (S-1-5-19), uses the fixed
qualification-only service ResourceTimelineAmdSystemProfileQualification, and
uses one derived Service SID for both phases. The Service SID type must be
UNRESTRICTED.

The control is LocalService plus that Service SID with
SeSystemProfilePrivilege absent. The treatment is the same service, account,
machine, Session 0, x64 artifact, AMD CLI, working directory, timeout, job
policy, classifier, and fixed command, with exactly one change:
SeSystemProfilePrivilege is assigned to the dedicated Service SID. The
experiment never assigns the right to the global LocalService account, never
adds Administrators membership, and never adds SeProfileSingleProcessPrivilege,
SeDebugPrivilege, SeTcbPrivilege, or another SYSTEM-only privilege.

The default administrator command is deliberately plan-only:

~~~powershell
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-admin-amd-i2e-service-profile-experiment.ps1'
~~~

It performs no service registration, LSA mutation, or AMD invocation. A
future human-authorized execution would require the explicit
ExecuteAuthorizedExperiment switch and would consume the paired control /
treatment gate, so it must not be run during ordinary documentation or
synthetic validation. The future fixed operation is non-sampling
AMDuProfCLI.exe timechart --list; no power event, duration, interval, or CSV
sampling is allowed.

The token-materialization gate requires LocalService, Session 0, x64, the
expected Service SID, no Administrators SID, SeSystemProfilePrivilege present
and enabled only in treatment, and no newly introduced
SeProfileSingleProcessPrivilege or SeDebugPrivilege. Before any future right
mutation, the direct Service SID rights are read and the experiment fails
closed if the exact right already exists. Rollback removes only the exact
right when the mutation journal proves this experiment added it; it never
uses an all-rights removal. Cleanup evidence is invocation-distinct and can
recover from service creation or any later failure point.

The fixed artifact SHA is recorded in the I2E wrapper and is used unchanged
for both phases. I2E is prepared but not executed:

~~~text
FROZEN_EXPERIMENT_ARTIFACT_SHA256 = 871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9
I2E = PREPARED_NOT_EXECUTED
LOCAL_SERVICE_ACCOUNT_WIDE_RIGHT_MUTATION = FORBIDDEN
ADMINISTRATORS_MEMBERSHIP_MUTATION = FORBIDDEN
REAL_LSA_MUTATION_DURING_PREPARATION = 0
REAL_AMD_RUNTIME_DURING_PREPARATION = 0
NEXT_GATE = HUMAN_SERVICE_SID_SESYSTEMPROFILE_EXPERIMENT_REVIEW
PRODUCTION_ACCOUNT = UNRESOLVED
~~~

## I2D read-only minimum-capability forensics

> HISTORICAL / SUPERSEDED NEXT-GATE SNAPSHOT: I2D read-only evidence
> collection is complete. The active preparation is I2E above.

I2D compares the completed LocalService result with the completed SYSTEM
result. It does not execute AMD, open a device, register a service, or mutate
ACLs, privileges, policy, or production configuration. The repository-native
helper is read-only and emits JSON to the console; it reports protected
evidence as unavailable rather than attempting recovery:

```powershell
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\i2d-readonly-forensics.ps1'
```

The helper inventories fixed AMD service/file metadata and service DACLs,
opens the LSA policy with exactly
`POLICY_VIEW_LOCAL_INFORMATION | POLICY_LOOKUP_NAMES = 0x00000801`, records
raw NTSTATUS plus `LsaNtStatusToWinError`, cross-checks direct rights for
LocalService, SYSTEM, and Administrators with `LsaEnumerateAccountRights`,
and compares normalized token evidence when the two context JSON files are
readable. It never invokes
`AMDuProfCLI.exe`, `sc.exe create/start/stop/delete`, `sc.exe sdset`,
`Set-Acl`, or an LSA privilege-assignment API. Current classification is
`MINIMUM_REQUIRED_CAPABILITY = UNRESOLVED`; the next gate is
`HUMAN_ELEVATED_READ_ONLY_I2D_EVIDENCE_COLLECTION`.
