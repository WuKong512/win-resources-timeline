# CPU-SENSOR-AMD PROVIDER ARCHITECTURE DECISION

This is an architecture decision record, not a production implementation or
metric-admission record. It follows the completed AMD uProf root-cause
investigation and does not authorize a new AMD runtime experiment.

## BASELINE AND DECISION

```text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
DECISION_HEAD = c9b1a1ed234c13ece097806d996ab465b4d2d943
ORIGIN_MAIN = 0c470c48b4e60bd94dfe720ec8981f919db8b1c1
ROOT_CAUSE = CXL_PROCESS_EXECUTABLE_DIRECTORY_POLICY_MISMATCH
ROOT_CAUSE_CONFIDENCE = CONFIRMED_BY_STATIC_AND_RUNTIME_COUNTERFACTUAL
AMD_PROVIDER_ARCHITECTURE = CLI_SUBPROCESS
DECISION_CONFIDENCE = MEDIUM
DECISION_STATUS = PROVISIONAL_DIRECTION / NOT_PRODUCTION_ADMITTED
AMD_PRIVILEGE_ARCHITECTURE = DEFER_INSUFFICIENT_EVIDENCE
PRIVILEGE_DEPLOYMENT_DECISION = DEFER_INSUFFICIENT_EVIDENCE
ADMIN_CONSENT_MODEL = ONE_TIME_INSTALL_OR_ENABLE
SERVICE_SESSION0_AMD_CLI_QUALIFIED = false
MINIMUM_REQUIRED_WINDOWS_PRIVILEGES = UNPROVEN
SPIKE_IMPLEMENTATION = PREPARED / BOUNDED_RUNTIME_TECHNICALLY_QUALIFIED
CPU_PACKAGE_POWER_W_RUNTIME_QUALIFIED = true
CPU_PACKAGE_POWER_W_PRODUCTION_QUALIFIED = false
PRODUCTION_IMPLEMENTATION = NOT_STARTED
```

The decisive counterfactual used the byte-identical hold fixture. From the
repository build directory it terminated before the durable main marker with
`-1 / 0xFFFFFFFF`; copied without modification into `D:\apps\AMDuProf\bin`,
it emitted both durable markers, held for approximately three seconds, and
returned zero. The recovered CXL policy compares the process executable
directory with `InstallationPath\bin` and `InstallationPath\bin\AMDPerf`.

The resulting hard constraint is:

```text
DIRECT_IN_PROCESS_AMD_UPROF_API_FROM_ARBITRARY_APP_DIRECTORY = NOT_VIABLE
PROCESS_GLOBAL_CWD_MUTATION = NOT_ACCEPTABLE_AS_DEFAULT_PROVIDER_DESIGN
```

The full runtime closure is in
[`cpu-sensor-amd-executable-directory-runtime-confirmation.md`](../measurements/cpu-sensor-amd-executable-directory-runtime-confirmation.md).

## WHAT THE EXISTING SEAM SUPPORTS

The repository already has the appropriate optional-provider boundary:

- `MetricProvider` exposes `probe`, `start`, `sample`, `stop`, health, and
  per-metric metadata.
- `ProviderHost` owns bounded provider calls, lifecycle reconciliation,
  timeout isolation, retry/backoff, and stale-generation handling.
- `CollectionPlan` carries requested categories, intervals, and disabled
  provider intent.
- `MetricCatalog` and `collection_session_metric` can preserve capability,
  provider identity, interval, and unavailable/failed status.
- The current production collector registers `windows-baseline` and the
  NVIDIA provider; no AMD provider was added here.

A future AMD source must be an optional provider alongside the baseline, not a
replacement for ordinary CPU usage collection. The existing `SystemSample`
DTO still needs additive work before AMD values can be persisted correctly;
that work is outside this decision.

## CANDIDATE EVALUATION

### A — Official `AMDuProfCLI.exe` subprocess

This is the selected provisional direction. The installed Administrator CLI
successfully listed capabilities and completed the already-qualified short
power timechart, producing real package-power samples. It is vendor-owned,
stays in its allowed installation directory, and gives Resource Timeline a
process boundary around vendor crashes. The evidence does not yet establish a
long-lived output contract, stable machine-readable schema, all-day behavior,
or whether the CLI can expose the required three metrics in one continuous
session.

The CLI spike treats the CLI as an external installed dependency. It does not
copy or redistribute AMD binaries and does not treat a short control as
production approval. The implementation details and test boundary are recorded
in [`cpu-sensor-amd-cli-provider-spike.md`](cpu-sensor-amd-cli-provider-spike.md).

### B — Resource Timeline helper in the AMD installation directory

The directory counterfactual proves that a fixture placed in `bin` can pass
the visible CXL executable-directory gate, but it does not make a
Resource-Timeline-owned executable in the vendor tree supportable. This model
would require writing a third-party binary into a vendor-owned installation,
with permission, antivirus, update/uninstall, version-skew, trust, licensing,
and cleanup consequences. It is not selected and is not implemented.

### C — Direct API in the Resource Timeline main process

For an arbitrary application install directory this is technically
incompatible with the proven CXL policy. A process-global current-directory
change, registry spoof, DLL replacement, loader hook, or binary patch is not an
acceptable deployment strategy. This candidate is closed for the current
product deployment model.

### D — No uProf production backend

This remains a valid fallback and a possible future research direction, but no
alternative source in the current repository evidence has simultaneously
cleared AMD-specific metric semantics, privilege, distribution, security,
lifecycle, and long-run overhead gates. It is therefore not selected as the
specific next task.

## VENDOR SERVICE SURFACE

```text
PUBLIC_REUSABLE_SERVICE_INTERFACE = NOT_FOUND
```

The no-op startup observation saw `AMDProfilerService.exe` as a child with a
vendor command line, but that is not a public IPC contract. Existing static
and documentation evidence did not identify a supported reusable telemetry
interface. The private child/authentication behavior will not be reverse
engineered or used as a provider dependency.

## SUPPORTABILITY MATRIX

Ratings are qualitative and reflect the current evidence, not a completed
implementation review.

| Criterion | CLI subprocess | Helper in AMD `bin` | Direct main-process API | No uProf / alternative |
|---|---|---|---|---|
| Directory policy compatible | GOOD | GOOD | POOR | GOOD |
| Vendor-tree mutation | GOOD | POOR | GOOD | GOOD |
| Admin required | POOR | UNKNOWN | POOR | UNKNOWN |
| Crash isolation | GOOD | GOOD | POOR | UNKNOWN |
| All-day suitability | UNKNOWN | UNKNOWN | POOR | UNKNOWN |
| Startup overhead | UNKNOWN | UNKNOWN | UNKNOWN | UNKNOWN |
| Sampling control | UNKNOWN | UNKNOWN | UNKNOWN | UNKNOWN |
| Metric fidelity | ACCEPTABLE for short power evidence | UNKNOWN | NOT_ESTABLISHED | UNKNOWN |
| Upgrade resilience | UNKNOWN | POOR | POOR | UNKNOWN |
| Legal/support risk | UNKNOWN | POOR | UNKNOWN | UNKNOWN |
| Implementation complexity | ACCEPTABLE | POOR | POOR | UNKNOWN |

`GOOD` for CLI vendor-tree mutation means no vendor file is created or
modified. It does not mean that CLI redistribution, licensing, or operational
support has been cleared. The `Admin required` rating is `POOR` for the current
machine because non-administrator power initialization returned
`0x80070005 / AMDT_ERROR_ACCESSDENIED`, while the Administrator control
succeeded; it is not a claim that every AMD/uProf installation has identical
permission behavior.

## PRIVILEGE MODEL

Known facts are narrow:

- The non-administrator CLI/API boundary returned
  `0x80070005 / AMDT_ERROR_ACCESSDENIED`.
- The Administrator CLI `timechart --list` and short power control succeeded.
- Direct API load failures and public initialization failures must remain
  separate states; the directory root cause does not remove the permission
  boundary.

The Resource Timeline main process must not silently become permanently
elevated as a consequence of this decision. No automatic UAC, `RunAs`, service
creation, or privilege bypass is authorized. A future implementation may only
use an already-supported deployment mechanism for a privileged helper or an
explicitly user-approved elevated operation. If that mechanism is unavailable,
the AMD provider reports `permission_denied`/unavailable and the rest of the
application continues.

## CONTINUOUS COLLECTION MODEL

The existing evidence covers a short, five-second CLI session only. It does
not prove streaming output, a stable long-lived session, restart semantics, or
safe concurrent use. Re-launching a CLI for every Resource Timeline sample is
not an acceptable default all-day design until startup cost, process churn, and
driver/session behavior are measured.

The next CLI spike must first determine whether one long-lived vendor process
can provide a supported, bounded, parseable stream or periodic artifact. If
the only supported interface is a short file-producing command, the spike
must explicitly decide whether the resulting launch/parse/recovery cost is
acceptable; this record does not assume that it is.

## METRIC CONTRACT STATUS

These statuses describe evidence, not production admission. `AVAILABLE` means
the source produced the named kind of value in the existing control; it does
not mean the Resource Timeline metric is qualified.

| Candidate | `CPU_PACKAGE_POWER_W` | `CPU_PACKAGE_TEMPERATURE_C` | `CPU_EFFECTIVE_FREQUENCY_MHZ` | Current interpretation |
|---|---|---|---|---|
| CLI subprocess | AVAILABLE as short vendor-control evidence | NOT_ESTABLISHED | NOT_ESTABLISHED | Requires output/scope/semantics and long-run qualification |
| Helper in AMD `bin` | NOT_ESTABLISHED | NOT_ESTABLISHED | NOT_ESTABLISHED | Directory gate alone is not API/session qualification |
| Direct main-process API | NOT_ESTABLISHED | NOT_ESTABLISHED | NOT_ESTABLISHED | Load path is not viable from arbitrary app directory |
| No uProf / alternative | NOT_ESTABLISHED | NOT_ESTABLISHED | NOT_ESTABLISHED | No selected alternative source has cleared the gates |

The CLI `--list` capability result and historical uProf documentation are
source clues, not permission to rename or aggregate counters. In particular,
AMD per-core effective frequency must not be silently turned into a package
headline; package temperature scope and power averaging semantics require a
separate contract. Until that work passes, the existing metric decisions stay:

```text
CPU_PACKAGE_POWER = DEFER
AMD_CORE_EFFECTIVE_FREQUENCY = DEFER
CPU_EFFECTIVE_FREQUENCY = DEFER_AGGREGATION_CONTRACT
CPU_PACKAGE_TEMPERATURE = DEFER_UNREACHED_DUE_TO_LIBRARY_LOAD
```

The temperature reason can be refined in a future successful API/CLI source
qualification, but no value is admitted by this architecture decision.

## OVERHEAD AND STABILITY ACCEPTANCE FRAMEWORK

The repository does not define AMD-uProf-specific numeric thresholds. Before
implementation, the CLI spike must agree provisional gates and record the
baseline and source attribution for:

1. startup and capability-probe latency;
2. steady-state provider CPU usage and peak/P95 working set;
3. wakeups and actual source cadence versus requested cadence;
4. process count, handle/thread growth, and child lifetime;
5. disk I/O, temporary/session-file growth, and cleanup;
6. UI/input responsiveness while the collector runs;
7. provider crash, hang, timeout, restart, and disable/re-enable behavior;
8. sleep/resume and driver/session recovery;
9. privilege loss and missing/unsupported metric transitions.

No measurement from the small Windows/Afterburner probe is reused as an AMD
uProf overhead claim. A source that fails any gate must remain optional and
unavailable without stopping the baseline collector.

## FALLBACK AND FAILURE MODEL

The provider must fail closed and preserve the existing app:

| Condition | Provider behavior |
|---|---|
| uProf absent or unsupported version | `ProviderMissing` / `provider_missing` capability; no installation attempt |
| required driver/service unavailable | unavailable status with vendor evidence; no service/driver mutation |
| privilege unavailable | `permission_denied`; no automatic elevation |
| CLI/helper crash or abnormal exit | isolated provider failure; bounded recovery or unavailable state |
| timeout or hung child | terminate only the owned process tree in a future implementation; record timeout and back off |
| output malformed or metric absent | per-metric failed/unsupported status; never synthetic zero |
| provider disabled | stop provider work and release owned resources; baseline collection continues |

The failure should be visible through the existing provider status and
capability/metric metadata, while CPU usage, memory, disk, GPU, process, and UI
operation continue normally.

## ARCHITECTURE DECISION

```text
AMD_PROVIDER_ARCHITECTURE = CLI_SUBPROCESS
DECISION_CONFIDENCE = MEDIUM
STATUS = PROVISIONAL / NOT_PRODUCTION_ADMITTED
```

This is the least disruptive candidate with direct evidence of useful AMD
power output, vendor-owned installation context, and crash isolation. The
confidence is not high because all-day collection, privilege deployment,
output stability, license/support terms, and the requested metric contract
remain open. The helper is rejected as a default because it mutates a vendor
installation; direct in-process API is rejected for arbitrary install
locations; an alternative backend is not evidence-backed enough to choose.

This decision does not justify permanently requiring Administrator for the
whole application and does not make the CLI a production provider today.

## SPIKE IMPLEMENTATION STATUS

The spike-only implementation is prepared in
`src-tauri/src/collector/amd_uprof_cli.rs` and the manually authorized runtime
wrapper is `tools/amd-uprof-cli-spike/run-admin-amd-cli-spike.ps1`. The module
is exported for focused tests and future adapter work, but `collector::manager`
does not register it. That deliberate registration gate keeps the current
collector and schema unchanged after the bounded Administrator qualification
passed for one recovered ten-second package-power session. The target result
and raw artifacts were preserved before the wrapper's post-runtime parsing
exception; the result was recovered offline, with no AMD rerun. Details are in
[`cpu-sensor-amd-cli-spike-runtime.md`](../measurements/cpu-sensor-amd-cli-spike-runtime.md).

The implementation includes registry-derived discovery, x64/signature/version
metadata, a direct argument-vector runner, bounded cancellation/timeout and
failure mapping, a file-producing session state machine, and a header-driven
package-power CSV parser. It does not perform an AMD call in-process, request
elevation, expose settings, or write a metric value into production storage.

## PRIVILEGE DEPLOYMENT DECISION

```text
AMD_PRIVILEGE_ARCHITECTURE = DEFER_INSUFFICIENT_EVIDENCE
DECISION_CONFIDENCE = MEDIUM
SERVICE_ACCOUNT_RUNTIME_QUALIFICATION_REQUIRED = true
PUBLIC_REUSABLE_SERVICE_INTERFACE = NOT_FOUND
```

The architecture-level comparison is complete, but no unattended privilege
deployment model is admitted. The service-broker shape is promising for
supervision, yet the exact service account and Session 0 behavior are
unqualified. A Scheduled Task still needs a secure standard-user control ACL,
typed result/cancellation semantics, and run ownership. Per-session UAC is not
appropriate for transparent all-day collection, and elevating the main app is
rejected by the product requirement.

The complete threat model, request allowlist, output ownership, account
comparison, and decision matrix are in
[`cpu-sensor-amd-privilege-deployment.md`](cpu-sensor-amd-privilege-deployment.md).
This decision does not implement or register a service/task, request UAC, or
change the production collector.

The required fallback remains a permission/unavailable provider state while
the baseline and other providers continue. The next qualification family is:

```text
NEXT_RUNTIME_QUALIFICATION = AMD-SERVICE-CONTEXT-I1
LONG_LIVED_SESSION_ORDERING = AFTER_PRIVILEGE_CONTEXT_QUALIFICATION
```

It must qualify one exact proposed principal/context with a bounded session and
fixed semantic request, not a broad matrix. It must prove result delivery,
cancellation, cleanup, and failure isolation before long-lived collection or
production admission is considered.

## DEFERRED / EXPLICITLY NOT DONE

- No AMD production provider was implemented or registered.
- No production catalog, schema, DTO, UI, lifecycle, or privilege behavior was
  changed.
- No new AMD command, profiling, sampling, cadence, workload, or service
  operation was run for this decision.
- No conclusion is made that an undocumented vendor service or private API is
  reusable.
