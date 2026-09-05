# CPU-SENSOR-AMD CLI PRIVILEGE DEPLOYMENT ARCHITECTURE

This record decides how the optional AMD uProf CLI provider could cross the
observed privilege boundary without elevating the Resource Timeline main
application. It is an architecture, threat-model, and qualification plan. It
does not install a service, register a scheduled task, request elevation, or
run AMD profiling.

## AMD CURRENT STATE RECONCILIATION (AUTHORITATIVE)

The following block is the current state after the authoritative
`AMD-SERVICE-CONTEXT-I1` result in PR #21. Older investigation snapshots below
remain part of the record, but any conflicting value in a section explicitly
marked `HISTORICAL / SUPERSEDED` is not a current gate.

```text
BASELINE = dd399f681f74bb23530e0fbee3713d54d0ea866d
AMD_SERVICE_CONTEXT_I1 = completed / PASS
SERVICE_SESSION0_AMD_CLI_QUALIFIED = true
AMD_PRIVILEGE_ARCHITECTURE = WINDOWS_SERVICE_BROKER
SERVICE_BROKER_FEASIBILITY = PASS
SERVICE_BROKER_CANDIDATE = EVIDENCE_SUPPORTED_PENDING_PRIVILEGE_AND_IPC
MINIMUM_REQUIRED_WINDOWS_PRIVILEGES = UNPROVEN
SERVICE_ACCOUNT_FIRST_QUALIFICATION_CANDIDATE = NT AUTHORITY\\LOCAL SERVICE
SERVICE_ACCOUNT_FIRST_QUALIFICATION_SID = S-1-5-19
SERVICE_SID_REQUIRED = true
IPC_CANDIDATE = WINDOWS_NAMED_PIPE
AMD_PRIVILEGE_I2 = prepared / awaiting authorized LocalService + IPC runtime qualification
NEXT_GATE = HUMAN_SETUP_ONLY
NEXT_TASK = AMD-PRIVILEGE-I2
PRODUCTION_ADMISSION = NOT_COMPLETE
```

`AMD-SERVICE-CONTEXT-I1` consumed the immutable LocalSystem/Session 0 evidence
at `C:\\ProgramData\\ResourceTimeline\\qualification\\amd-service-context\\20260904T080323173Z`.
This task must not modify that evidence and must not repeat its AMD runtime.
`AMD-PRIVILEGE-I2` is a qualification-only least-privilege and secure-IPC
preparation task; it does not select the production account or admit a provider.

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

## AMD-PRIVILEGE-I2 CURRENT STATE

`AMD-PRIVILEGE-I2` prepares an independent qualification-only broker and
synthetic security harness. It is not a production broker and has no
collector, database, UI, installer, autostart, or production persistence
dependency.

```text
AMD_PRIVILEGE_I2 = prepared / awaiting authorized LocalService + IPC runtime qualification
SERVICE_ACCOUNT_FIRST_QUALIFICATION_CANDIDATE = NT AUTHORITY\\LOCAL SERVICE
SERVICE_ACCOUNT_FIRST_QUALIFICATION_SID = S-1-5-19
SERVICE_SID_REQUIRED = true
SERVICE_SID_TYPE_REQUIRED = UNRESTRICTED
IPC = WINDOWS_NAMED_PIPE
PIPE_ACL_POLICY = INSTALLING_USER_SID + SERVICE_SID + SYSTEM; NO_BROAD_USER_ACCESS
NAMED_PIPE_SECURITY_QUALIFICATION = synthetic PASS / real cross-integrity runtime pending
SEMANTIC_IPC_ONLY = true
SEMANTIC_IPC = synthetic PASS
ACTIVE_SESSIONS = 1
SESSION_OWNERSHIP = synthetic PASS
BUSY_ARBITRATION = synthetic PASS
OWNER_CANCELLATION = synthetic PASS
CANCELLATION = synthetic PASS
CLIENT_DISCONNECT_POLICY = CANCEL_OWNED_SESSION
NO_ORPHAN_SYNTHETIC_CHILD = PASS
REAL_AMD_RUNTIME_DURING_PREPARATION = 0
SERVICE_REGISTRATION_DURING_PREPARATION = 0
PENDING_ACCEPT_LIFETIME_GUARD = CLOSED_OFFLINE
PENDING_ACCEPT_CANCEL_AND_DRAIN = REQUIRED_BEFORE_RELEASE
HRESULT_NORMALIZATION = CLOSED
FIRST_ACCEPT_READINESS = CLOSED_OFFLINE_PENDING_REAL_REACCEPTANCE
NEXT_GATE = HUMAN_SETUP_ONLY
NEXT_TASK = AMD-PRIVILEGE-I2 / authorized runtime gate
```

The first candidate is a qualification hypothesis only. Even a future
LocalService runtime PASS may narrow the observed path to LocalService or
less; it cannot by itself prove the absolute minimum Windows privilege.
