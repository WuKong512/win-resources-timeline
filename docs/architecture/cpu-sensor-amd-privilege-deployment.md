# CPU-SENSOR-AMD CLI PRIVILEGE DEPLOYMENT ARCHITECTURE

This record decides how the optional AMD uProf CLI provider could cross the
observed privilege boundary without elevating the Resource Timeline main
application. It is an architecture, threat-model, and qualification plan. It
does not install a service, register a scheduled task, request elevation, or
run AMD profiling.

## AMD CURRENT STATE RECONCILIATION (AUTHORITATIVE)

This is the single current AMD handoff. All older qualification snapshots and
human command sequences below are historical records; they do not authorize a
new runtime. Sections that conflict with this block are explicitly marked
`HISTORICAL / SUPERSEDED`.

```text
AMD_SERVICE_CONTEXT_I1 = COMPLETED / PASS
AMD_PRIVILEGE_I2 = COMPLETED
I2E = CLOSED / RERUN_FORBIDDEN
I2E_REAL_PAIRED_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2E_REAL_TREATMENT_RESUME_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2E_REAL_CLEANUP_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2E_HISTORICAL_EVIDENCE = IMMUTABLE
I2E_RERUN = FORBIDDEN
I2E_CLEANUP_RERUN = FORBIDDEN
I2F = REAL_COMPLETED / PASS_WITH_NEGATIVE_COUNTER_ACCESS_RESULT / RERUN_FORBIDDEN
I2F_AUTHORITATIVE_SCOPE = f68bf4d3d36547a0ba753cff489bb6eb
I2F_REAL_EXECUTION_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2F_REAL_CLEANUP_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2F_FULL_ROLLBACK = REAL_PASS
I2_LEGACY_REAL_ENTRYPOINTS = RETIRED
I2B_REAL_ENTRYPOINTS = RETIRED
I2C_REAL_ENTRYPOINTS = RETIRED
AMD_QUALIFICATION_EXECUTABLE_ENTRYPOINT_AUDIT = PASS_NO_UNRETIRED_HISTORICAL_REAL_GATE
LOCAL_SERVICE_POWER_COUNTER_ACCESS = UNAVAILABLE
SYSTEM_POWER_COUNTER_ACCESS = AVAILABLE / HISTORICAL REAL DIFFERENTIAL
SECURITY_CONTEXT_DIFFERENTIAL = REAL_CONFIRMED
SE_SYSTEM_PROFILE_PRIVILEGE_ALONE_SUFFICIENT = false
SE_SYSTEM_PROFILE_PRIVILEGE_NECESSITY = UNRESOLVED
MINIMUM_REQUIRED_CAPABILITY = UNRESOLVED_AFTER_READ_ONLY_SELECTION
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_VARIABLE_SELECTION = PASS_READ_ONLY
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_SINGLE_VARIABLE_ISOLATABLE = true
I2G_HARNESS = IMPLEMENTED_OFFLINE
I2G_REAL_RUNTIME = ATTEMPT3_COMPLETE
I2G_REAL_SCIENTIFIC_RESULT = PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT
I2G_CAUSAL_INTERPRETATION_VALID = true
I2G_REAL_GATE_CONSUMED = true
I2G_REAL_EXECUTION_ALLOWED = false
I2G_REAL_CLEANUP_ALLOWED = false
I2G_HARNESS_IMPLEMENTATION_AUTHORIZED = true
I2G_REAL_RUNTIME_AUTHORIZED = false
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
PRODUCTION_ADMISSION = DEFER
I2H_JUSTIFIED = NO
AMD_CLI_LIST_PATH_VALIDITY_Q1 = COMPLETE / INSUFFICIENT
CLI_LIST_ROLE = ACCOUNT_SENSITIVE_COUNTER_ENUMERATION / DISCOVERY_ONLY
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
SELECTED_NEXT_TASK = NONE
NEXT_GATE = HUMAN_REVIEW_OPERATION_PATH_EVIDENCE_GAP
NEXT_TASK = NONE
POST_I2G_DECISION = ../upgrade/amd-post-i2g-production-admission.md
OPERATION_PATH_AUDIT = ../upgrade/amd-cli-list-path-validity.md
ARCHITECTURE_SINGLE_AUTHORITATIVE_CURRENT_STATE = PASS
```

`AMD-SERVICE-CONTEXT-I1` consumed the immutable LocalSystem/Session 0 evidence
at `C:\\ProgramData\\ResourceTimeline\\qualification\\amd-service-context\\20260904T080323173Z`.
This task must not modify that evidence and must not repeat its AMD runtime.
`AMD-PRIVILEGE-I2` has now consumed exactly one bounded real LocalService
qualification run. The broker, secure IPC, identity, CLI launch, and cleanup
boundaries passed, but AMD uProf reported that no counters were available from
the LocalService context. The I2B non-sampling `timechart --list` differential
and I2C SYSTEM-only comparison are historical real evidence, and all of their
legacy setup, client, and cleanup wrappers are now permanently retired. None
selects LocalSystem, alters AMD permissions, or admits a provider.

The operation-path audit is now complete with route verdict `INSUFFICIENT`.
The historical discovery results remain scoped to counter enumeration; they do
not by themselves establish or reject active production sampling capability.
See [`amd-cli-list-path-validity.md`](../upgrade/amd-cli-list-path-validity.md).

## HISTORICAL / SUPERSEDED I2E PRE-CONTROL INCIDENT

The first human-authorized I2E invocation stopped at `sc.exe create` with exit
code `1057`. The wrapper supplied the bare SCM account value `LocalService`,
which is not the Windows predefined account spelling accepted by the Service
Control Manager. The service was not created, no Service SID was resolved, and
neither paired phase began.

```text
I2E_FIRST_REAL_ATTEMPT = PRE_SERVICE_CREATE_FAILURE
SC_CREATE_EXIT = 1057
ROOT_CAUSE = SCM_ACCOUNT_NAME_WAS_BARE_LocalService
SCM_SERVICE_ACCOUNT = NT AUTHORITY\LocalService
SERVICE_CREATED = false
SERVICE_SID_RESOLVED = false
CONTROL_EXECUTED = false
TREATMENT_EXECUTED = false
LSA_MUTATION = false
AMD_RUNTIME = false
PAIRED_EXPERIMENT_GATE = UNCONSUMED
FAILED_ATTEMPT_POINTER_RECOVERY = PREPARED
CURRENT_POINTER_FINALIZATION = PREPARED
HISTORICAL_NEXT_GATE_AT_FIRST_INCIDENT = HUMAN_I2E_FAILED_ATTEMPT_CLEANUP
```

The repaired orchestration records explicit gate-consumption state and uses
`NT AUTHORITY\LocalService` for the SCM `obj=` value. The cleanup instruction
above was the historical next step for that pre-service incident; it is not the
current gate after the later real CONTROL completion. The current control
recovery and treatment-only resume state is documented below.

## HISTORICAL / SUPERSEDED INITIAL DECISION AND STATUS

```text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
START_HEAD = 326244ffa2917a3c9451c492b18fd504c93b5f84
DECISION_HEAD = 326244ffa2917a3c9451c492b18fd504c93b5f84
ORIGIN_MAIN = 0c470c48b4e60bd94dfe720ec8981f919db8b1c1
AMD_PROVIDER_ARCHITECTURE = CLI_SUBPROCESS
AMD_CLI_PROVIDER_SPIKE = TECHNICALLY_QUALIFIED_FOR_BOUNDED_SESSION
CPU_PACKAGE_POWER_W_RUNTIME_QUALIFIED = true
AMD_PROVIDER_PRODUCTION_ADMITTED = false
AMD_PRIVILEGE_ARCHITECTURE = DEFER_INSUFFICIENT_EVIDENCE
DECISION_CONFIDENCE = MEDIUM
ADMIN_CONSENT_MODEL = ONE_TIME_INSTALL_OR_ENABLE
PRIVILEGE_DEPLOYMENT_DECISION = DEFER_INSUFFICIENT_EVIDENCE
SERVICE_SESSION0_AMD_CLI_QUALIFIED = false
MINIMUM_REQUIRED_WINDOWS_PRIVILEGES = UNPROVEN
SERVICE_ACCOUNT_CANDIDATE = UNSELECTED_PENDING_RUNTIME_QUALIFICATION
SERVICE_ACCOUNT_RUNTIME_QUALIFICATION_REQUIRED = true
PUBLIC_REUSABLE_SERVICE_INTERFACE = NOT_FOUND
LONG_LIVED_SESSION_ORDERING = AFTER_PRIVILEGE_CONTEXT_QUALIFICATION
```

The deferral is intentional. The evidence is sufficient to reject direct
in-process loading from an arbitrary application directory and to show that a
manually elevated, bounded `AMDuProfCLI.exe` power session can work. It is not
sufficient to select a service account or to prove that the CLI works in
Session 0, nor to prove that a standard-user client may safely control an
elevated Scheduled Task. A production architecture must not turn either
unqualified assumption into a privilege boundary.

## CURRENT EVIDENCE

The accepted root-cause counterfactual is recorded in
[`cpu-sensor-amd-executable-directory-runtime-confirmation.md`](../measurements/cpu-sensor-amd-executable-directory-runtime-confirmation.md):
the byte-identical static fixture survives only when its executable directory
is one of the CXL-allowed AMD directories. Therefore:

```text
ROOT_CAUSE = CXL_PROCESS_EXECUTABLE_DIRECTORY_POLICY_MISMATCH
ROOT_CAUSE_CONFIDENCE = CONFIRMED_BY_STATIC_AND_RUNTIME_COUNTERFACTUAL
DIRECT_IN_PROCESS_AMD_UPROF_API_FROM_ARBITRARY_APP_DIRECTORY = NOT_VIABLE
```

The single accepted bounded CLI session is recorded in
[`cpu-sensor-amd-cli-spike-runtime.md`](../measurements/cpu-sensor-amd-cli-spike-runtime.md).
It used `AMDuProfCLI.exe` 5.3.521.0 in an Administrator x64 PowerShell and
produced nine parseable socket package-power samples. The process exited zero,
but this is only one bounded Administrator-user control; it is not evidence
for a service account, Session 0, unattended restart, or all-day collection.

Separate historical evidence established:

```text
NONADMIN_AMD_POWER_PATH = ACCESSDENIED
ADMIN_AMD_POWER_PATH = SUCCESS
```

This proves an observed privilege boundary, not the minimum required Windows
privilege. In particular, it does not prove that LocalSystem, LocalService,
NetworkService, a dedicated local account, or a task/service principal will
work.

The `AMDProfilerService.exe` child seen during the vendor GUI startup control
is not a public reusable telemetry interface. Its existence is evidence of
vendor application bootstrap only; no private IPC or authenticated/bypass
surface is selected here.

## EXISTING REPOSITORY SEAMS

The repository currently has no privileged component to reuse:

| Concern | Current evidence | Consequence |
|---|---|---|
| Main process | `src-tauri/src/lib.rs` builds a Tauri GUI/background process and starts the collector during app setup. The product requirement keeps it non-elevated by default. | No permanent main-process elevation. |
| Collector | `src-tauri/src/collector/manager.rs` starts one collector thread and currently registers Windows baseline plus NVIDIA only. | AMD failure can remain outside the default collector until admitted. |
| Provider boundary | `MetricProvider`, `ProviderHost`, `CollectionPlan`, health, capability, timeout, cancellation, and retry/backoff live in `src-tauri/src/collector/provider.rs`. | A future AMD adapter should reuse this seam rather than add a second supervisor. |
| Existing IPC | Tauri commands use `AppState` and `invoke_handler`. There is no named pipe, local socket, broker RPC, or cross-integrity IPC protocol. | A service/task design needs a new narrowly scoped IPC boundary; it is not present today. |
| Windows background behavior | `src-tauri/src/platform/windows/autostart.rs` refreshes a per-user Run entry; `session.rs` observes session/power events. | Autostart is user-logon behavior, not a privileged service or task. |
| Instance/process control | `platform/windows/instance.rs` owns a local mutex for single-instance behavior. | This is not a child-process or privileged-component ownership boundary. |
| Persistence | `src-tauri/src/lib.rs` opens SQLite below Tauri `app_local_data_dir()`; `src-tauri/src/db/mod.rs` owns the writer. | A Session 0 broker must not write the user-owned app database directly. |
| Installer | `src-tauri/tauri.conf.json` enables normal Tauri bundling only. No service/task registration or ACL lifecycle is configured. | Install-time elevation and rollback seams do not yet exist. |
| Privileged component | None found in the current application topology. | `EXISTING_PRIVILEGED_COMPONENT = NONE`. |

The existing `amd_uprof_cli.rs` module is a spike boundary, not a registered
production provider. Its direct argument-vector runner, bounded timeout,
captured output, and owned-process cleanup are reusable concepts, but its
current process model does not solve cross-integrity launch or IPC.

```text
MAIN_PROCESS_SECURITY_CONTEXT = NON_ELEVATED_BY_DEFAULT_USER_SESSION
EXISTING_IPC_SEAM = TAURI_MAIN_PROCESS_COMMANDS_ONLY
INSTALLER_PRIVILEGE_SEAM = NONE_FOR_SERVICE_OR_TASK
```

## NON-NEGOTIABLE PRODUCT REQUIREMENTS

The main application must remain a standard-user application by default:

- `MAIN_APP_PERMANENTLY_ELEVATED = false`.
- Interactive UAC per sample is forbidden.
- Interactive UAC per collection session is unacceptable for all-day mode.
- AMD is optional. Missing privilege, CLI, driver/service, counters, output,
  or a healthy session must not stop other providers, persistence, the UI, or
  the collector supervisor.
- One explicit install/enable authorization may create a future privileged
  component. Normal collection must not prompt repeatedly.
- Disable and uninstall must stop an owned session and remove only Resource
  Timeline-owned components and artifacts; they must not modify AMD binaries,
  drivers, or registry installation state.

The default fallback is:

```text
AMD_PROVIDER_STATUS = PERMISSION_REQUIRED
```

when no approved privileged execution path exists. The rest of the collector
continues and no AMD value is replaced with a synthetic zero.

```text
ADMIN_CONSENT_MODEL = ONE_TIME_INSTALL_OR_ENABLE
INTERACTIVE_UAC_PER_SAMPLE = FORBIDDEN
INTERACTIVE_UAC_PER_COLLECTION_SESSION = UNACCEPTABLE_FOR_ALL_DAY_MODE
```

## CANDIDATE ARCHITECTURES (HISTORICAL / SUPERSEDED ANALYSIS)

> HISTORICAL / SUPERSEDED: the candidate comparison below preserves why the
> Service Broker was initially deferred, why Session 0 was unknown, why the
> Scheduled Task was not selected, and the original threat model. The current
> state is the reconciliation block above and the I2 preparation record below.

### A. Windows Service Broker

Conceptually, a standard-user Resource Timeline client would use a protected
local IPC endpoint to request a fixed semantic power session from a dedicated
privileged broker. The broker would derive the trusted AMD installation path,
launch the installed CLI with its required AMD-bin working directory, own the
CLI process tree, parse/validate the result, and return typed samples.

Strengths:

- Fits unattended operation and keeps the main application standard-user.
- Gives one component explicit ownership of the CLI process, timeout, restart,
  cleanup, and single-session arbitration.
- Provides a natural place to keep raw vendor artifacts out of the client and
  to enforce a fixed command allowlist.
- Can use a service SID and a narrowly ACLed named pipe rather than granting a
  user arbitrary high-integrity process execution.

Blocking evidence:

- `SERVICE_SESSION0_AMD_CLI_QUALIFIED = false`; the accepted success was an
  elevated interactive Administrator user, not Session 0.
- The working service account and its minimum rights are unknown.
- A service installation/update/uninstall path, pipe ACL, service identity,
  and rollback contract do not exist in the repository.
- Sleep/resume, reboot, multi-user, and driver/session behavior in Session 0
  are unqualified.

Verdict: strongest long-lived shape in principle, but not selected before the
exact account and Session 0 behavior are qualified.

### B. Pre-registered Elevated Scheduled Task

An install/enable step could register a task at highest run level, while the
ordinary app requests a bounded session later. This keeps the main app
standard-user and avoids a persistent service binary, but it does not remove
the security-boundary problem.

The task's security descriptor must explicitly authorize the standard-user
client to start, query, receive status from, and cancel only the intended task.
Default Task Scheduler permissions must not be assumed to provide this. A
loosely writable task or task action would be a local privilege-escalation
primitive.

The model also needs a live result channel, exact task/run identity, bounded
cancellation, orphan cleanup, and restart behavior. Polling a task-owned CSV
from the user profile is a weak substitute for typed IPC and creates file ACL
and stale-result problems.

Verdict: technically possible, but control ACLs, live session supervision,
and result/cancellation semantics are currently unqualified. It is not
selected merely because it avoids implementing a service.

### C. Per-session UAC Elevated Helper

This is simple to prototype: the standard app starts a fixed helper through an
explicit user-approved elevation operation. It does not require a permanent
service or task, but a UAC interaction becomes part of every start/restart
boundary. That is incompatible with transparent all-day operation, crash
recovery, launch after reboot, and unattended operation while the desktop is
locked.

Verdict: acceptable only as an explicitly user-mediated diagnostic/manual
mode, not as the default production deployment architecture.

### D. Elevated Main Application

Rejected. It violates `MAIN_APP_PERMANENTLY_ELEVATED = false`, expands the
blast radius of every UI, parser, update, and provider defect, and would make
AMD availability a reason to run unrelated collection and UI code at high
integrity. The CXL directory policy does not justify this tradeoff.

### E. No uProf Provider Without Approved Privilege

Mandatory fallback, not a privilege deployment mechanism. Discovery or
collection reports a permission/unavailable state and the existing baseline,
GPU, persistence, and UI paths continue. The fallback remains valid if uProf
is missing, unsupported, disabled, or cannot be deployed safely.

## SERVICE ACCOUNT ANALYSIS

No account is selected by this record. The following is a risk and
qualification comparison, not a claim that any account works with AMD uProf:

| Account | Potential fit | Main risk/unknown | Current decision |
|---|---|---|---|
| `LocalSystem` | Broad local access and a stable service identity. | Excessive privilege and severe attack consequence if the broker or its IPC is compromised; profile and AMD driver behavior are still unknown. | Do not choose by default. |
| `LocalService` | More restricted local identity and limited network exposure; compatible with least-privilege investigation. | AMD CLI, driver access, profile/config discovery, Session 0, and output access are unqualified. | Candidate only after runtime qualification. |
| `NetworkService` | Stable service identity with machine credentials for network access. | Network identity is unnecessary for local telemetry and increases exposure; AMD behavior remains unknown. | Not preferred; qualification required if considered. |
| Dedicated local user | Can bound filesystem and process rights more narrowly than LocalSystem. | Credential lifecycle, logon/service identity, profile availability, and AMD/driver requirements need installer and security design. | Candidate only after product/security review and runtime qualification. |
| Elevated interactive user | Matches the only successful privilege evidence. | Not an unattended deployment model; depends on an interactive elevated token and user/session lifetime. | Evidence control only, not the production account decision. |

```text
SERVICE_ACCOUNT_CANDIDATE = UNSELECTED_PENDING_RUNTIME_QUALIFICATION
SERVICE_ACCOUNT_RUNTIME_QUALIFICATION_REQUIRED = true
MINIMUM_REQUIRED_WINDOWS_PRIVILEGES = UNPROVEN
```

No broad privilege such as `SeDebugPrivilege` is requested or implied. The
minimum requirement must be measured against the exact AMD CLI/driver behavior,
not inferred from the word “Administrator”.

## BROKER THREAT MODEL AND REQUEST CONTRACT

A future privileged broker must be a semantic capability endpoint, never a
generic elevated process launcher. The low-integrity client must not submit an
executable path, raw argv, shell command, working directory, environment
variable, registry path, or output path.

The narrow initial contract should be conceptually equivalent to:

```text
GetAmdProviderStatus
StartAmdPowerSession {
    duration_ms,
    interval_ms
}
GetAmdSessionStatus { session_id }
CancelAmdSession { session_id }
```

The broker owns and derives:

- the installation root from the legitimate AMD installed-state location;
- `bin\AMDuProfCLI.exe` and the required `bin` working directory;
- the fixed event `power`;
- the CLI version/signature/architecture policy;
- the output root and parser;
- one generated session identifier and one active-session policy.

The client controls no raw vendor command surface. Initial bounds should be
versioned and explicitly provisional rather than silently becoming permanent
product requirements. A reasonable pilot policy is:

```text
event = power                         # fixed, not client supplied
duration_ms = 5_000..60_000           # provisional pilot bound
interval_ms = 1_000..10_000           # provisional pilot bound
duration_ms >= interval_ms
active_sessions_per_install = 1      # reject concurrent session as BUSY
```

The final bounds must follow the supported CLI contract and product cadence
requirements. Invalid values fail before launch; a client cannot use bounds to
turn the broker into a general command runner.

## IPC SECURITY MODEL

There is no existing cross-integrity IPC seam. If a service broker is later
selected, a Windows named pipe is the preferred candidate because it supports
an explicit security descriptor and local identity checks. A local socket is
not automatically safer and has no current repository abstraction.

The intended named-pipe controls are:

- Create the pipe with a non-predictable, installation-scoped name and an ACL
  that grants access only to the installing user's SID (or an explicitly
  approved local-user set) plus the broker's service identity. Do not use
  `Everyone` or a broadly writable pipe directory.
- Configure the pipe DACL at creation, preventing pipe squatting/name
  collision. Treat a stale endpoint or mismatched protocol as failure.
- Verify the connected client's PID, process token, user SID, and expected
  integrity relationship. Impersonation is for identity/authorization checks,
  not for executing client-supplied commands.
- Serialize or explicitly arbitrate concurrent clients. A second client gets
  a status response, not a second vendor session.
- Keep request sizes, session counts, and cancellation operations bounded to
  limit local denial of service.

If a Scheduled Task is selected instead, its task security descriptor must
provide the equivalent restrictions for start/query/stop and result access.
The client must not gain write access to the task action, executable, working
directory, or output root. This ACL design is a first-class qualification gate.

```text
BROKER_CLIENT_AUTHORIZATION_MODEL = INSTALLING_USER_SID_ONLY_BY_DEFAULT
SERVICE_RESOURCE_IDENTITY = SERVICE_SID
IPC_REQUEST_MODEL = FIXED_SEMANTIC_ALLOWLIST
```

## CLI IDENTITY AND OUTPUT SECURITY

Before a launch, or at a controlled refresh, the privileged component should
read the observed AMD installation value:

```text
HKLM\SOFTWARE\WOW6432Node\AMD\AMDProfiler\InstallationPath
<InstallationPath>\bin\AMDuProfCLI.exe
```

It should validate that the derived file exists, is x64, has a valid AMD
Authenticode signature, and falls within an explicitly supported version
policy. A user-supplied executable path is not allowed. The historical
SHA-256 is useful for evidence and release qualification, but exact hash
pinning is not suitable as the only cross-version production policy because
vendor updates legitimately change the binary. A signer plus version policy,
with optional known-build hashes, is more maintainable and still requires a
future trust/update review.

The broker must derive a broker-owned session directory. It must not accept an
arbitrary output path or write into the AMD installation tree. For a service,
that directory would normally be below a service-owned ProgramData location
with ACLs for the service SID and controlled diagnostic access. A user TEMP
directory is not assumed to be valid for Session 0.

The safer data path is:

```text
AMDuProfCLI.exe
    -> broker-owned raw session artifacts
    -> one validated/shared parser
    -> typed package-power samples over IPC
    -> existing main collector/DB writer
```

The main app should persist typed samples through its existing user-owned DB
writer rather than letting a high-integrity process write the app database.
The parser must have one source of truth (a shared library or broker-owned
parser with shared fixtures), not two silently divergent CSV contracts.

## SESSION OWNERSHIP AND FAILURE MODEL

A future broker must record an exact launch tuple:

```text
session_id
cli_pid + process_start_time
validated_executable_path + version/signature identity
validated_working_directory
broker-owned output directory
requesting client identity
```

It should use a job/process-tree ownership mechanism where available and kill
only the exact owned CLI tree on cancellation or timeout. It must never kill
by image name globally. `CANCELLED`, `TIMEOUT`, `PERMISSION_REQUIRED`,
`RUNTIME_FAILED`, `PARSE_FAILED`, and `BUSY` remain distinct outcomes.

Required future lifecycle behavior:

- If the main app crashes, the broker must not leave an uncontrolled profiling
  session; it needs a lease, client disconnect policy, or bounded session
  lifetime.
- If the broker crashes, the SCM/task supervisor may restart it, but the
  restart must reconcile and clean only its owned session state.
- If the CLI crashes, the broker reports failure and applies bounded backoff;
  it must not enter an immediate restart loop.
- Sleep/resume and driver/session loss must produce a controlled unavailable
  state and an explicit restart policy.
- Reboot must restore the privileged component only if the user enabled it,
  with protocol/version validation before accepting requests.

The existing `ProviderHost` already offers bounded in-process provider calls,
health status, generation handling, cancellation, and retry/backoff. A future
AMD adapter should translate broker outcomes into that vocabulary and leave
healthy providers running.

## SERVICE SESSION 0 GATE (HISTORICAL / SUPERSEDED BY AMD-SERVICE-CONTEXT-I1)

The vendor control that succeeded ran in an elevated interactive user session.
It did not qualify:

```text
SERVICE_SESSION0_AMD_CLI_QUALIFIED = false
```

The CLI's apparent non-interactive command syntax is not enough to prove
correct behavior under Session 0, a service account, a missing user profile,
or a locked desktop. A future service design must qualify the exact principal,
working directory, installation discovery, output access, driver interaction,
sleep/resume, and clean cancellation in that context before service
implementation is admitted.

## SCHEDULED TASK CONTROL GATE

Task registration is an install-time Administrator operation. Runtime start,
status, and cancellation are separate operations that a standard-user client
must be authorized to perform through the task's security descriptor. The
future design must prove all of the following without granting arbitrary task
or executable control:

- the client can start exactly the pre-registered task;
- the client can identify the exact run it started;
- the client can receive typed status/results without trusting stale files;
- the client can cancel only its owned run;
- a second client cannot alter the task action or hijack its output;
- upgrade/uninstall can stop the run and remove the task deterministically.

Until that evidence exists, `ELEVATED_SCHEDULED_TASK` is not a selected
architecture.

## INSTALL, UPDATE, DISABLE, AND UNINSTALL

A future privileged deployment has four versioned identities:

```text
main_app_version
broker_or_task_action_version
ipc_protocol_version
amd_cli_version
```

The installer/enable operation must be the only place that requests the
one-time Administrator consent. It must create the service/task, ACLs, and
broker-owned directories atomically enough to roll back. Updates must stop an
owned session, validate the replacement identity, preserve or migrate the
protocol contract, and restore the previous component if validation fails.

Disable/uninstall must stop an active owned session, remove the service/task,
remove Resource Timeline-created ACLs/directories/artifacts, and leave AMD
installation files, registry installation values, drivers, and services
untouched. A stale privileged component must reject an incompatible protocol
rather than accepting unknown request fields.

## MULTI-USER MODEL

AMD package telemetry is currently treated as hardware-global for deployment
purposes; per-user concurrent sessions have not been qualified. The safe
default for a future component is one active session per installation with
explicit `BUSY` arbitration. A per-installing-user pipe ACL prevents an
unrelated local user from controlling the broker until a multi-user policy is
approved. Fast user switching, two logged-in clients, and session lock/unlock
must be covered by a later qualification rather than inferred from the
Administrator CLI run.

## OPTIONS DECISION MATRIX

Ratings below describe fit to the product requirements and current evidence,
not implementation effort alone.

| Criterion | Windows Service Broker | Elevated Scheduled Task | Per-session UAC Helper | Elevated Main App |
|---|---|---|---|---|
| Main app stays standard user | GOOD | GOOD | GOOD | POOR |
| Unattended | UNKNOWN (Session 0) | UNKNOWN (task control/result path) | POOR | ACCEPTABLE technically, but violates requirement |
| IPC quality | GOOD if named-pipe ACL is correct | POOR without a separate secure result channel | UNKNOWN | GOOD in-process |
| Live session supervision | GOOD | POOR | POOR | ACCEPTABLE |
| Cancellation | GOOD | POOR until run ACL is proven | ACCEPTABLE | ACCEPTABLE |
| Crash isolation | GOOD | ACCEPTABLE | ACCEPTABLE | POOR |
| Security boundary | GOOD if fixed-command broker and ACLs are correct | UNKNOWN until task DACL is qualified | ACCEPTABLE | POOR |
| Least privilege potential | ACCEPTABLE | ACCEPTABLE | ACCEPTABLE | POOR |
| Installation complexity | POOR | ACCEPTABLE | ACCEPTABLE | GOOD |
| Update complexity | POOR | POOR | ACCEPTABLE | ACCEPTABLE |
| Multi-user behavior | UNKNOWN | UNKNOWN | POOR | ACCEPTABLE |
| Service/Session-0 uncertainty | UNKNOWN | ACCEPTABLE | GOOD | GOOD |
| User experience | ACCEPTABLE after enablement | POOR/UNKNOWN | POOR for unattended use | POOR |

The matrix does not make a service selection. Its key unknown is the exact
AMD CLI behavior under Session 0 and the selected least-privilege account. The
task's key unknown is not installation; it is secure runtime control and live
result/cancellation behavior.

## ARCHITECTURE DECISION (HISTORICAL / SUPERSEDED)

```text
AMD_PRIVILEGE_ARCHITECTURE = DEFER_INSUFFICIENT_EVIDENCE
DECISION_CONFIDENCE = MEDIUM
```

No candidate currently satisfies both the non-elevated-main requirement and
the evidence gates for unattended operation:

- Service Broker is the leading conceptual shape, but Session 0, account,
  ACL, installer, and lifecycle behavior are unqualified.
- Scheduled Task preserves the main process boundary, but standard-user
  control ACLs, live IPC, cancellation, and supervision are unqualified.
- Per-session UAC is not suitable for all-day unattended collection.
- Elevated Main App is explicitly rejected.
- Disabling the provider is the required safe fallback, not a production
  telemetry solution.

This deferral is a product/security decision, not permission to prototype a
service or task silently. The CLI provider remains provisional and not
production-admitted.

## NEXT RUNTIME QUALIFICATION (HISTORICAL / SUPERSEDED)

Choose exactly one future qualification family after the product selects a
candidate deployment model:

```text
NEXT_RUNTIME_QUALIFICATION = AMD-SERVICE-CONTEXT-I1
```

It must be one bounded, manually authorized test of one exact proposed
principal and context, not a broad matrix and not a production installation.
The selected branch determines the assertions:

- service candidate: exact account + Session 0, trusted installation
  discovery, one bounded package-power session, typed result, cancellation,
  cleanup, and no uncontrolled child;
- task candidate: exact task principal plus standard-user start/query/cancel
  ACL, one bounded session, typed result retrieval, exact run ownership, and
  cleanup.

The qualification must use the fixed semantic request contract, not
client-supplied executable/argv/cwd/environment/output. It must not run now,
register a service/task in this task, or start long-lived collection.

## LONG-LIVED SESSION ORDERING

```text
LONG_LIVED_SESSION_ORDERING = AFTER_PRIVILEGE_CONTEXT_QUALIFICATION
```

The production privilege context can change CLI paths, profile access,
working-directory behavior, output ownership, child supervision, and
restart/shutdown behavior. A long-lived or all-day session qualification is
therefore not the next experiment. It follows successful qualification of the
exact privilege deployment context and its IPC/lifecycle contract.

## FALLBACK AND PRODUCTION-ADMISSION BLOCKERS

Until the above gates pass:

| Condition | Required behavior |
|---|---|
| AMD uProf absent or unsupported | Report provider missing/unsupported; continue all other collectors. |
| Privilege unavailable | Report `PERMISSION_REQUIRED`; do not prompt per sample or self-elevate. |
| Driver/service unavailable | Report unavailable/runtime failure; do not install or repair vendor components. |
| CLI crash, timeout, or cancellation | Isolate the owned process/session, report the distinct state, and apply bounded backoff. |
| Output or counter invalid | Report parse/counter failure; never store a fabricated zero. |
| Provider disabled | Stop AMD work and release only owned resources. |

Production admission remains blocked by unattended privilege deployment,
stable supported session/output/timestamp semantics, all-day overhead and
responsiveness, restart/sleep/reboot behavior, version/update policy, legal
and licensing review, and an additive package-power storage/DTO contract.
Temperature and frequency remain deferred and are not part of this decision.

## SERVICE-CONTEXT QUALIFICATION PREPARATION (HISTORICAL / SUPERSEDED BY COMPLETED I1)

The leading service-broker candidate now has a separate qualification-only
SCM harness in
[`tools/amd-cli-service-context-qualification`](../../tools/amd-cli-service-context-qualification/README.md).
It is a genuine Windows Service executable, but it is not a production broker:
it has no IPC, installer, autostart, service registration, or application
dependency. It accepts only a controlled ProgramData run-root, derives the
AMD CLI from the observed registry installation path, and uses one fixed
ten-second package-power command. The existing package-power post-processor
remains the single parser.

The prepared future run uses a manually registered `LocalSystem` service with
manual/demand start, Session 0 proof, a bounded 30-second CLI timeout, and
qualification-before-cleanup evidence. The run must prove the service account
SID, Session 0, x64 process, token integrity/elevation, CLI identity, raw
capture, typed parser result, cadence, final SCM status, and exact service
deletion. It must not change AMD files, registry, PATH, drivers, services, or
security settings beyond the exact temporary qualification registration.

```text
SERVICE_BROKER_CANDIDATE = LEADING_PENDING_RUNTIME_QUALIFICATION
SERVICE_SESSION0_AMD_CLI_QUALIFIED = false
NEXT_RUNTIME_QUALIFICATION = AMD_CLI_SERVICE_CONTEXT_QUALIFICATION
LONG_LIVED_SESSION_ORDERING = AFTER_PRIVILEGE_CONTEXT_QUALIFICATION
```

The qualification package's non-AMD Rust and PowerShell tests passed without
registering a service or launching an AMD executable. The exact future
Administrator command is documented in
[`cpu-sensor-amd-service-context-qualification.md`](../measurements/cpu-sensor-amd-service-context-qualification.md).

## HISTORICAL / SUPERSEDED PREPARATION STATUS

- No production Windows Service was implemented or registered.
- No Scheduled Task was registered.
- No production installer, elevation flow, or service account was created.
- No AMD executable, profiling command, sampling session, driver, registry,
  PATH, or system state was changed.
- No production AMD provider was registered.

## HISTORICAL / SUPERSEDED — AMD-PRIVILEGE-I2 COMPLETION SNAPSHOT

`AMD-PRIVILEGE-I2` prepares an independent qualification-only broker and
synthetic security harness. It is not a production broker and has no
collector, database, UI, installer, autostart, or production persistence
dependency.

```text
AMD_PRIVILEGE_I2 = real bounded LocalService IPC/AMD launch PASS; counter backend unavailable
SERVICE_ACCOUNT_FIRST_QUALIFICATION_CANDIDATE = NT AUTHORITY\\LOCAL SERVICE
SERVICE_ACCOUNT_FIRST_QUALIFICATION_SID = S-1-5-19
SERVICE_SID_REQUIRED = true
SERVICE_SID_TYPE_REQUIRED = UNRESTRICTED
IPC = WINDOWS_NAMED_PIPE
PIPE_ACL_POLICY = INSTALLING_USER_SID + SERVICE_SID + SYSTEM; NO_BROAD_USER_ACCESS
NAMED_PIPE_SECURITY_QUALIFICATION = REAL_PASS
SEMANTIC_IPC_ONLY = true
SEMANTIC_IPC = REAL_PASS_FOR_PROVIDER_AND_SESSION_BOUNDARY
ACTIVE_SESSIONS = 1
SESSION_OWNERSHIP = REAL_PASS
BUSY_ARBITRATION = synthetic PASS
OWNER_CANCELLATION = synthetic PASS
CANCELLATION = synthetic PASS
CLIENT_DISCONNECT_POLICY = CANCEL_OWNED_SESSION
NO_ORPHAN_CHILD = REAL_PASS
REAL_AMD_RUNTIME_DURING_PREPARATION = 0
REAL_AMD_RUNTIME_DURING_I2 = 1
LOCAL_SERVICE_AMD_CLI_EXIT_CODE = 0
PACKAGE_POWER_SAMPLING = NOT_RUN
I2_REAL_RUNTIME_GATE_CONSUMED = true
SERVICE_REGISTRATION_DURING_PREPARATION = 0
PENDING_ACCEPT_LIFETIME_GUARD = CLOSED_OFFLINE
PENDING_ACCEPT_CANCEL_AND_DRAIN = REQUIRED_BEFORE_RELEASE
HRESULT_NORMALIZATION = CLOSED
FIRST_ACCEPT_READINESS = REAL_PASS
LIVE_SERVICE_STOP = REAL_PASS
NEXT_GATE = HUMAN_COUNTER_DISCOVERY_DIFFERENTIAL
NEXT_TASK = AMD-PRIVILEGE-I2B
```

The first candidate is a qualification hypothesis only. Even a future
LocalService runtime PASS may narrow the observed path to LocalService or
less; it cannot by itself prove the absolute minimum Windows privilege.

## HISTORICAL / SUPERSEDED — AMD-PRIVILEGE-I2B COMPLETION SNAPSHOT

```text
RESULT = COUNTER_PRIVILEGE_DIFFERENTIAL_REQUIRED
LOCAL_SERVICE_AMD_RUNTIME_LAUNCH = REAL_PASS
LOCAL_SERVICE_POWER_COUNTER_ACCESS = FAILED_OR_UNAVAILABLE
OUTPUT_DISCOVERY_DEFECT = NOT_ROOT_CAUSE
COUNTER_DISCOVERY_DIAGNOSTIC_PREPARED = true
LOCAL_SERVICE_LIST_CONTEXT_PREPARED = true
SYSTEM_LIST_CONTEXT_PREPARED = true / HUMAN_AUTHORIZATION_REQUIRED / PLAN_ONLY
TOKEN_DIFFERENTIAL_EVIDENCE_PREPARED = true
AMD_BACKEND_READ_ONLY_FORENSICS = PASS_WITH_SERVICE_ENUMERATION_DENIED
LOCAL_SERVICE_TO_SYSTEM_SWITCH = NOT_AUTHORIZED
I2_REAL_RUNTIME_GATE_CONSUMED = true
NEXT_GATE = HUMAN_COUNTER_DISCOVERY_DIFFERENTIAL
PRODUCTION_ADMISSION = NOT_COMPLETE
```

## HISTORICAL / SUPERSEDED — AMD-PRIVILEGE-I2C CURRENT STATE AT PRE-RUN

The LocalService counter-discovery side is complete real evidence at scope
`4b30b3d64b7e469cbce7c8080c84b7d4`: the fixed non-sampling AMD uProf
`timechart --list` operation reported `POWER_UNAVAILABLE` with the known
no-counters diagnostic and exit code `0`. A duplicate cleanup invocation
overwrote only the single cleanup summary; the discovery evidence remains
authoritative and the LocalService run remains valid.

```text
LOCAL_SERVICE_COUNTER_DISCOVERY = REAL_POWER_UNAVAILABLE
LOCAL_SERVICE_POWER_CATEGORY_PRESENT = false
LOCAL_SERVICE_DIFFERENTIAL_SIDE = COMPLETE
SYSTEM_DIFFERENTIAL_SIDE = PREPARED / NOT_EXECUTED
SYSTEM_HARNESS_SERVICE = ResourceTimelineAmdSystemCounterQualification
SYSTEM_SERVICE_ACCOUNT = NT AUTHORITY\SYSTEM
SYSTEM_SERVICE_ACCOUNT_SID = S-1-5-18
SYSTEM_SESSION = 0
SYSTEM_FIXED_COMMAND = timechart --list
SYSTEM_SAMPLING = false
SYSTEM_HARNESS_MODE = DEDICATED_NON_IPC_SERVICE
SYSTEM_SETUP_AND_DISCOVERY_ARE_COUPLED = true
SYSTEM_TOKEN_DIFFERENTIAL_EVIDENCE_PREPARED = true
SYSTEM_CLEANUP_DUPLICATE_SAFE = true
LOCAL_SERVICE_REAL_ARTIFACT_SHA256 = C9973BAAA01AF3C2673D8C70D8C7E626C577642505E6DFF7BA3C6026DEA63FB1
SYSTEM_PREPARATION_ARTIFACT_SHA256 = 9E5A012B0A95C84DD28CD607D99EF43C9BC4D700683F33890CDE6C2108794AC3
PRODUCTION_ACCOUNT_SELECTION = UNRESOLVED
LOCAL_SERVICE_TO_SYSTEM_SWITCH = NOT_AUTHORIZED
NEXT_GATE = HUMAN_SYSTEM_COUNTER_DISCOVERY_EXECUTION
```

The dedicated SYSTEM service and its Administrator-only setup/cleanup wrappers
are qualification-only and were not executed during preparation. They preserve
the fixed AMD CLI identity and `timechart --list` command while adding no
arbitrary command surface, IPC path, production installer integration, or
account-selection decision.

The old architecture decision and early Service/Session 0 unknowns above are
historical/superseded records. They remain to explain why the broker candidate
was originally deferred; the current evidence now supports the broker boundary
while leaving the AMD power-counter capability differential unresolved.

## HISTORICAL / SUPERSEDED — AMD-PRIVILEGE-I2E SERVICE-SID SYSTEM-PROFILE EXPERIMENT PREPARATION

I2D established a real LocalService-versus-SYSTEM counter-availability
differential, but did not isolate the minimum Windows capability. The
read-only evidence shows that SYSTEM has SeSystemProfilePrivilege enabled
while LocalService does not, and that a distinct service SID already has a
direct assignment of that right. I2E therefore prepares a paired,
minimum-variable experiment without granting the right to the global
LocalService account and without adding Administrators membership.

~~~text
EXPERIMENT_ACCOUNT = NT AUTHORITY\\LOCAL SERVICE
EXPERIMENT_ACCOUNT_SID = S-1-5-19
DEDICATED_SERVICE_NAME = ResourceTimelineAmdSystemProfileQualification
DEDICATED_SERVICE_SID = DERIVED_AT_FUTURE_SETUP
SERVICE_SID_TYPE = UNRESTRICTED
CONTROL = LocalService + same dedicated Service SID + SeSystemProfilePrivilege absent
TREATMENT = LocalService + same dedicated Service SID + SeSystemProfilePrivilege only
SERVICE_SID_SESYSTEMPROFILE_MUTATION = EXACT_ONE_RIGHT_ONLY
LOCAL_SERVICE_ACCOUNT_WIDE_RIGHT_MUTATION = FORBIDDEN
ADMINISTRATORS_MEMBERSHIP_MUTATION = FORBIDDEN
SEPROFILE_SINGLE_PROCESS_MUTATION = FORBIDDEN
SEDEBUG_MUTATION = FORBIDDEN
TOKEN_MATERIALIZATION_GATE = PREPARED
PAIRED_CONTROL_TREATMENT = PREPARED
PREEXISTING_RIGHT_PRESERVATION = PREPARED
EXACT_ROLLBACK = PREPARED
STATUS_NO_MORE_ENTRIES = READ_EMPTY
LSA_READ_POLICY_ACCESS = 0x00000801
LSA_ADD_POLICY_ACCESS = 0x00000810
LSA_REMOVE_POLICY_ACCESS = 0x00000800
LSA_FIRST_ASSIGNMENT_POLICY_CREATE_ACCOUNT = SUPPORTED
ACCOUNT_OBJECT_STATE_DIAGNOSTIC = PRESENT_OR_ABSENT_OR_UNKNOWN
FIXED_COMMAND = timechart --list
SAMPLING = false
FROZEN_ARTIFACT = ONE_SHA_FOR_CONTROL_AND_TREATMENT
FROZEN_EXPERIMENT_ARTIFACT_SHA256 = 871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9
REAL_AMD_RUNTIME_DURING_PREPARATION = 0
REAL_SERVICE_RUNTIME_DURING_PREPARATION = 0
REAL_LSA_MUTATION_DURING_PREPARATION = 0
I2E = CONTROL_REAL_COMPLETE_TREATMENT_PENDING
MINIMUM_REQUIRED_CAPABILITY = UNRESOLVED
PRODUCTION_ACCOUNT = UNRESOLVED
NEXT_TASK = AMD-PRIVILEGE-I2E
NEXT_GATE = HUMAN_I2E_TREATMENT_ONLY_RESUME
~~~

The preparation is repository-native and qualification-only. The default
administrator wrapper invocation is plan-only and performs no service,
security-policy, or AMD operation. The future real gate requires explicit
human authorization for the paired control/treatment execution, followed by
the exact-right rollback and invocation-distinct cleanup. A treatment result
can show sufficiency for the observed bounded path, but cannot by itself
prove global minimum privilege or select the production account.

The LSA mutation contract uses operation-specific minimum access: read-only
enumeration uses `0x00000801`, first-assignment `LsaAddAccountRights` uses
`0x00000810` so the dedicated Service SID account object may be created, and
exact-right `LsaRemoveAccountRights` uses only `0x00000800`. No handle requests
broader policy access, and account-object state is recorded as
`PRESENT`, `ABSENT`, or `UNKNOWN` where the read-only status permits.

## HISTORICAL / SUPERSEDED — AMD-PRIVILEGE-I2E CONTROL COMPLETE, TREATMENT PENDING

The first corrected human I2E invocation completed CONTROL under the existing
LocalService qualification service. Its authoritative result is
`POWER_UNAVAILABLE` with a passing token gate, no orphan child, and no
SeSystemProfilePrivilege assignment. The orchestration then failed while
stopping the already-completed CONTROL service: PowerShell variable names are
case-insensitive, so assigning local `$pid` collided with the read-only `$PID`
automatic variable. The service is currently stopped with PID 0; this is a
post-control harness incident, not CONTROL failure.

```text
I2E_SECOND_HUMAN_INVOCATION = CONTROL_REAL_EXECUTED_THEN_ORCHESTRATION_STOP_FAILURE
CONTROL_REAL_EXECUTION = REAL_COMPLETE
CONTROL_RESULT = POWER_UNAVAILABLE
CONTROL_TOKEN_GATE = PASS
CONTROL_NO_ORPHAN_CHILD = true
TREATMENT_REAL_EXECUTION = 0
LSA_MUTATION = 0
ROOT_CAUSE = POWERSHELL_AUTOMATIC_VARIABLE_PID_COLLISION
CURRENT_SERVICE = STOPPED / PID0 / LocalService
PAIRED_GATE_CONSUMED = true
CURRENT_POINTER_STATE = STALE_AFTER_POST_CONTROL_STOP_FAILURE
CONTROL_RECOVERY = PREPARED
CONTROL_RERUN = FORBIDDEN
TREATMENT_ONLY_RESUME = PREPARED
NEXT_REAL_AMD_OPERATION = TREATMENT_ONLY
NEXT_GATE = HUMAN_I2E_TREATMENT_ONLY_RESUME
REAL_SERVICE_RUNTIME_DURING_REPAIR = 0
REAL_LSA_MUTATION_DURING_REPAIR = 0
REAL_AMD_RUNTIME_DURING_REPAIR = 0
```

The recovery contract validates the exact experiment, control/treatment
scopes, Service SID, frozen artifact, and immutable CONTROL evidence before it
reconciles the stale pointer. The treatment-only wrapper has no CONTROL
execution path: it rechecks the stopped existing service and absent right,
adds exactly `SeSystemProfilePrivilege` to the existing Service SID only,
enforces the treatment token gate before AMD, and performs exact rollback and
cleanup. No recovery, cleanup, service, LSA mutation, or AMD runtime was run
during this offline repair.

## HISTORICAL / SUPERSEDED — AMD-PRIVILEGE-I2E HISTORICAL POINTER SCHEMA COMPATIBILITY CLOSURE

The latest treatment-only attempt stopped before mutation because the
historical CURRENT pointer was a `PSCustomObject` without the newer rollback
fields and direct property assignment could not evolve its schema. Active I2E
pointer updates now use a canonical set-or-add helper and persist the
pre-mutation state before `LsaAddAccountRights` can run. The immutable CONTROL
evidence remains authoritative and the treatment gate remains unconsumed.

```text
I2E_TREATMENT_ATTEMPT = PRE_MUTATION_ORCHESTRATION_FAILURE
ROOT_CAUSE = HISTORICAL_PSCUSTOMOBJECT_SCHEMA_EVOLUTION_UNSAFE_DIRECT_PROPERTY_ASSIGNMENT
CONTROL_RECOVERY = REAL_PERSISTED
CONTROL_RESULT = POWER_UNAVAILABLE
AMD_CLI_REVALIDATION = REAL_READ_ONLY_PASS
LSA_MUTATION = 0
SERVICE_START = 0
TREATMENT_RUNTIME = 0
AMD_RUNTIME = 0
SET_OR_ADD_PROPERTY_HELPER = PASS
HISTORICAL_JSON_POINTER_FIXTURE = PASS
POINTER_PERSIST_BEFORE_LSA_ADD = PASS
PRE_REMOVE_DUAL_READBACK = PASS
POST_REMOVE_PRE_POINTER_CRASH_RECOVERY = PASS
POLICY_STATE_DRIFT = FAIL_CLOSED
NEXT_GATE = HUMAN_I2E_TREATMENT_ONLY_RESUME_REVIEW
```

Cleanup distinguishes policy rollback from effective token teardown and
performs fresh dual LSA readback before an exact removal. If both directions
already prove absence, it records `POLICY_ALREADY_ABSENT_ON_RECOVERY` and makes
zero duplicate removal calls. No service, LSA mutation, or AMD runtime was
executed during this offline closure.

> HISTORICAL / SUPERSEDED CURRENT-STATE SNAPSHOT: AMD-PRIVILEGE-I2D

## HISTORICAL / SUPERSEDED — AMD-PRIVILEGE-I2D MINIMUM CAPABILITY FORENSICS

The dedicated SYSTEM comparison has now consumed its one authorized,
non-sampling `timechart --list` run. The two results use the same signed x64
AMD CLI (`D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC`,
version `5.3.521.0`) and establish a real security-context differential:

```text
LOCAL_SERVICE_SCOPE = 4b30b3d64b7e469cbce7c8080c84b7d4
LOCAL_SERVICE_COUNTER_DISCOVERY = REAL_POWER_UNAVAILABLE
LOCAL_SERVICE_POWER_CATEGORY_PRESENT = false
LOCAL_SERVICE_NO_COUNTERS_DIAGNOSTIC = true
LOCAL_SERVICE_ACCOUNT_SID = S-1-5-19
LOCAL_SERVICE_SESSION_ID = 0
COMMON_ENABLED_PRIVILEGES = SeChangeNotifyPrivilege, SeCreateGlobalPrivilege, SeImpersonatePrivilege
LOCAL_SERVICE_ENABLED_ONLY = none
SYSTEM_ENABLED_ONLY = SeAuditPrivilege, SeCreatePagefilePrivilege, SeCreatePermanentPrivilege, SeCreateSymbolicLinkPrivilege, SeDebugPrivilege, SeDelegateSessionUserImpersonatePrivilege, SeIncreaseBasePriorityPrivilege, SeIncreaseWorkingSetPrivilege, SeLockMemoryPrivilege, SeProfileSingleProcessPrivilege, SeSystemProfilePrivilege, SeTcbPrivilege, SeTimeZonePrivilege
COMMON_DISABLED_PRIVILEGES = SeAssignPrimaryTokenPrivilege, SeIncreaseQuotaPrivilege, SeShutdownPrivilege, SeSystemtimePrivilege, SeUndockPrivilege
LOCAL_SERVICE_DISABLED_ONLY = SeAuditPrivilege, SeIncreaseWorkingSetPrivilege, SeTimeZonePrivilege
SYSTEM_DISABLED_ONLY = SeBackupPrivilege, SeLoadDriverPrivilege, SeManageVolumePrivilege, SeRestorePrivilege, SeSecurityPrivilege, SeSystemEnvironmentPrivilege, SeTakeOwnershipPrivilege
COMMON_RELEVANT_GROUPS = S-1-5-32-545, S-1-5-6
SYSTEM_RELEVANT_GROUPS_INCLUDE = S-1-5-32-544
SERVICE_SID_DIFFERENCE = controlled secondary difference between distinct qualification services
TOKEN_DIFFERENTIAL_SEMANTICS = ENABLED_DISABLED_ABSENT_AND_GROUP_FIELDS_NORMALIZED

SYSTEM_SCOPE = 091a72e1d38341ca9eca0877b1625082
SYSTEM_COUNTER_DISCOVERY = REAL_POWER_AVAILABLE
SYSTEM_POWER_CATEGORY_PRESENT = true
SYSTEM_ACCOUNT_SID = S-1-5-18
SYSTEM_SESSION_ID = 0

SECURITY_CONTEXT_DIFFERENTIAL = REAL_CONFIRMED
MINIMUM_REQUIRED_CAPABILITY = UNRESOLVED
PRODUCTION_ACCOUNT = UNRESOLVED
NEXT_TASK = AMD-PRIVILEGE-I2D
NEXT_GATE = HUMAN_ELEVATED_READ_ONLY_I2D_EVIDENCE_COLLECTION
```

## HISTORICAL / SUPERSEDED — I2E real result and I2F self-enable preparation

I2E is closed as a real token-materialization result, not as an AMD result.
The exact dedicated Service SID assignment was present in the LocalService
token, but `SeSystemProfilePrivilege` was disabled. The in-service token gate
failed before AMD and the exact right, service, and process state were fully
rolled back.

```text
I2E_RESULT = PASS_WITH_NEGATIVE_TOKEN_ENABLEMENT_RESULT
I2E_CONTROL_RESULT = POWER_UNAVAILABLE
I2E_TOKEN_PRIVILEGE = PRESENT_DISABLED
I2E_TOKEN_GATE = REAL_FAIL_EXPECTED_PRIVILEGE_DISABLED
I2E_AMD_RUNTIME = 0
I2E_COUNTER_DISCOVERY = NOT_EXECUTED
I2E_FULL_ROLLBACK = REAL_PASS
I2E_SERVICE_REMOVED = true
I2E_RESIDUAL_RIGHT = ABSENT
I2E_RERUN = FORBIDDEN
```

The next minimum-variable preparation is I2F. It retains LocalService and a
dedicated unrestricted Service SID, then explicitly enables only the already
materialized `SeSystemProfilePrivilege` in the qualification service's own
token with native `AdjustTokenPrivileges`. The service records before/after
token evidence and requires exactly one semantic transition,
`DISABLED -> ENABLED`, before launching only `timechart --list`. No arbitrary
privilege name, IPC command, sampling argument, production collector path, or
LocalSystem fallback exists.

```text
I2F_SERVICE = ResourceTimelineAmdSystemProfileEnableQualification
I2F_ACCOUNT = NT AUTHORITY\LocalService
I2F_ACCOUNT_SID = S-1-5-19
I2F_INTENTIONAL_VARIABLE = SeSystemProfilePrivilege DISABLED -> ENABLED via AdjustTokenPrivileges
I2F_COMMAND = timechart --list
I2F_SAMPLING = false
I2F_ARTIFACT_SHA256 = F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D
I2F_ARTIFACT_ARCHITECTURE = x64
I2F_STATUS = PREPARED / NOT_EXECUTED
MINIMUM_REQUIRED_CAPABILITY = UNRESOLVED
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
NEXT_GATE = HUMAN_I2F_SELF_ENABLE_QUALIFICATION_REVIEW

## HISTORICAL / SUPERSEDED — I2F PRE-RUN ROLLBACK SAFETY

I2F_ROLLBACK_STOP_FIRST = PASS_STATIC
I2F_PROCESS_EVIDENCE_UNKNOWN_NOT_ZERO = PASS
I2F_POLICY_REMOVE_AFTER_TOKEN_TEARDOWN_ONLY = PASS_STATIC
I2F_ROLLBACK_PARTIAL_FAILURE_EVIDENCE = PASS
I2F_STANDALONE_CLEANUP_IDEMPOTENT = PASS_STATIC
I2F_RUST_PRE_ENABLE_TO_AMD_ORDER = PASS
I2F_HUMAN_RUNTIME = NOT_EXECUTED
I2F_ARTIFACT_SHA256 = F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D
I2F_ARTIFACT_CHANGED = false
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
NEXT_GATE = HUMAN_I2F_SELF_ENABLE_QUALIFICATION_REVIEW
```

I2F uses a fresh service/evidence identity so immutable I2E evidence remains
untouched. Its rollback removes only the exact Service SID right, verifies both
LSA directions, proves service/process absence, and deletes the qualification
registration. No I2F service, LSA mutation, token adjustment, AMD runtime, or
sampling was performed during preparation.

The explicit offline closure contract is represented by
`tools/amd-privilege-qualification/i2e-token-materialization-final.example.json`.
It records the expected negative token-enablement shape without pretending to
be historical machine evidence.

The first forensic pass prioritizes `SeSystemProfilePrivilege` because it is
reported enabled in SYSTEM and absent from the LocalService enabled set. A
SYSTEM/Administrators group or AMD kernel device/object ACL remains a competing
hypothesis. Read-only service-object DACLs were identical across the inspected
AMD components, and the CLI/registry/file path was already usable from
LocalService; neither observation proves the missing capability. No AMD device
interface or device-object security descriptor was identified by the bounded
read-only pass, so that evidence is explicitly limited rather than treated as
negative proof.

User-right assignment enumeration for the prioritized rights was previously
attempted with an incomplete `LsaOpenPolicy` mask and returned
`0xC0000022 / STATUS_ACCESS_DENIED`; that result is not authoritative policy
evidence. I2D-A changes the read-only mask to
`POLICY_VIEW_LOCAL_INFORMATION | POLICY_LOOKUP_NAMES = 0x00000801`, records
raw NTSTATUS plus `LsaNtStatusToWinError`, and cross-checks direct assignment
for LocalService, SYSTEM, and Administrators with
`LsaEnumerateAccountRights`. Token privilege presence, token enablement,
direct user-right assignment, and group-derived rights remain separate fields.

The I2D helper and synthetic parser contract are
`tools/amd-privilege-qualification/i2d-readonly-forensics.ps1` and the existing
offline qualification test. They perform no service, AMD, ACL, privilege,
device, or production mutation. The prepared next step is a human review of a
minimum-variable experiment, not a production account switch.

```text
REAL_AMD_RUNTIME_DURING_I2D = 0
SERVICE_RUNTIME_DURING_I2D = 0
SECURITY_MUTATIONS_DURING_I2D = 0
SE_SYSTEM_PROFILE_REAL_EXPERIMENT = NOT_AUTHORIZED
ADMINISTRATORS_EXPERIMENT = NOT_AUTHORIZED
DEVICE_ACL_MUTATION_EXPERIMENT = NOT_AUTHORIZED
PRODUCTION_ACCOUNT_SWITCH = NOT_AUTHORIZED
NEXT_GATE = HUMAN_ELEVATED_READ_ONLY_I2D_EVIDENCE_COLLECTION
```
