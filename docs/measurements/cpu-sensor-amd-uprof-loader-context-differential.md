# CPU-SENSOR-AMD LOADER CONTEXT DIFFERENTIAL

This record consumes the already-generated Administrator context matrix. It does not rerun any AMD diagnostic child, `AMDuProfCLI`, profiling, sampling, cadence, or workload. It does not modify the AMD installation or Windows configuration.

```text
RESULT: BLOCKED
CXL_CONTEXT_REQUIREMENT: CLI_INITIALIZATION_STATE
MINIMAL_CXL_REQUIRED_CONTEXT: CLI_SPECIFIC_INITIALIZATION_STATE
CXL_DIRECT_LOAD: PERSISTS
API_DIRECT_LOAD: NOT_RUN
INIT_ONLY: NOT_RUN
```

`CLI_INITIALIZATION_STATE` is the required residual classification after the simple context controls failed. It does not prove a particular private AMD initialization state or identify the exact CXL internal condition.

## BASELINE

- Repository: `WuKong512/win-resources-timeline`.
- Base commit: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`.
- Start and pre-documentation head: `315cbbf6514ac21f341868222d47d994b5f9c4af` (`docs: record AMD CLI and direct API divergence`).
- `origin/main`: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`.
- Branch: `spike/cpu-sensor-amd-uprof-live-qualification`.
- The qualification branch was already attached to the current worktree; no reset, rebase, merge, cherry-pick, amend, or force push was performed.
- The Q1 merge commit remains an ancestor of `origin/main`; no relevant remote drift was observed.
- Duplicate-task gate: `PASS` for this existing qualification branch. No competing context-differential branch was found. `gh` was unavailable, so open-PR visibility is not claimed.
- Historical blocked records remain preserved in [`cpu-sensor-amd-uprof-live-qualification.md`](cpu-sensor-amd-uprof-live-qualification.md), [`cpu-sensor-amd-uprof-load-abort-followup.md`](cpu-sensor-amd-uprof-load-abort-followup.md), and [`cpu-sensor-amd-uprof-cli-vs-direct-api-divergence.md`](cpu-sensor-amd-uprof-cli-vs-direct-api-divergence.md).

Evidence directory:

```text
C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-context-differential-20260829T051204605Z
```

## ADMIN MATRIX VALIDITY

`ADMIN-00` passed using a real Administrator PowerShell:

- `C:\WINDOWS\System32\whoami.exe /groups` exited `0` / `0x00000000`.
- `WindowsPrincipal` Administrator membership was `true`.
- The token output contained `S-1-16-12288`; SID parsing classified it as the accepted `High` level. The decision used the numeric SID, not the localized label.
- PowerShell was x64 (`5.1.26100.9168`).
- `no_elevation_performed_by_script = true`.
- The exact captured output is in `ADMIN-00-WHOAMI.stdout.txt` and is embedded in `ADMIN-00-WHOAMI.json` and `ADMIN-00-elevation-proof.json`.

Artifact identity:

| Artifact | Expected SHA-256 | Observed SHA-256 | Result |
|---|---|---|---|
| `metric-probe.exe` | `69D551BB8423823BC092605DA5C42CB21A45CACA618838B8437B080F2D659687` | same | PASS |
| `CXLBaseTools.dll` | `4815D4631BCA9C051DC4293538DF8D402BD848E705228F497DF718EDCA1F8931` | same | PASS |
| `AMDPowerProfileAPI.dll` | authoritative `9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277` | same | PASS after metadata correction |

The raw `ADMIN-ARTIFACT-INVENTORY.json` contains a 63-character API `expected_sha256` ending in `A427`, while the observed file hash is the correct 64-character value ending in `A4277`. This is `PREVIOUS_API_SHA_MISMATCH = HARNESS_METADATA_DEFECT`, not a vendor artifact mismatch. Raw evidence is preserved unchanged.

The three executed matrix children all had `process_started = true`, complete stdout/stderr capture, `timed_out = false`, and no process-tree kill or fallback kill. Their stderr files contain only the UTF-8 BOM (`EF BB BF`), while the captured stderr value is empty. The postcomputed `load_gate` is present in the main summary; the individual child JSON files were written before that derived field was added. The gate is independently established by the exact stdout and exit code, so these records are not incomplete.

The summary records `no_persistent_environment_change = true`, `no_system_mutation = true`, `no_amd_profile = true`, and `no_sampling = true`. CWD was supplied to each child through `ProcessStartInfo`; PATH changes were child-only.

## HISTORICAL A0

`CTX-A0-HISTORICAL-ADMIN-BASELINE` was intentionally not rerun. It preserves the previous Administrator direct CXL result:

```text
cwd: C:\Users\Hello\.codex\worktrees\08bd\resource-timeline
executable: C:\Users\Hello\.codex\worktrees\08bd\resource-timeline\tools\metric-probe\target\release\metric-probe.exe
args: amd-uprof-load-only-child --path D:\apps\AMDuProf\bin\CXLBaseTools.dll
exit: -1 / 0xFFFFFFFF
observation: KERNEL32!FatalExit(0xFFFFFFFF)
```

The historical full environment was `NOT_CAPTURED`. The current matrix therefore records intended context changes explicitly and does not reconstruct historical environment values.

## AMD COMMAND PROMPT CONTEXT

The installed shortcut audit found:

```text
target: C:\Windows\SysWOW64\cmd.exe
arguments: /k cd "D:\apps\AMDuProf\bin"
Start In: D:\apps\AMDuProf\bin\
```

No additional setup script or deterministic AMD-specific environment initialization was found in the bounded audit. The observed shortcut effect is working-directory selection only. The evidence does not establish that the 32-bit `cmd.exe` process is required; the successful prior CLI control was launched from elevated PowerShell with the AMD bin working directory.

## CWD-ONLY RESULT

| ID | CWD | Child PATH | Target | Exit | Gate | Interpretation |
|---|---|---|---|---|---|---|
| A0 | repository | historical | `CXLBaseTools.dll` | `-1 / 0xFFFFFFFF` | historical `FatalExit` | Existing Administrator baseline |
| A1 | `D:\apps\AMDuProf\bin` | inherited | `CXLBaseTools.dll` | `-1 / 0xFFFFFFFF` | `FATAL_EXIT_OR_0xFFFFFFFF` | AMD-bin CWD did not resolve CXL |
| A2 | repository | AMD bin prepended | `CXLBaseTools.dll` | `-1 / 0xFFFFFFFF` | `FATAL_EXIT_OR_0xFFFFFFFF` | PATH-only control did not resolve CXL |
| A3 | `D:\apps\AMDuProf\bin` | AMD bin prepended | `CXLBaseTools.dll` | `-1 / 0xFFFFFFFF` | `FATAL_EXIT_OR_0xFFFFFFFF` | CWD plus PATH did not resolve CXL |

For A1, A2, and A3 the child emitted only:

```text
BEFORE_LOAD path=\\?\D:\apps\AMDuProf\bin\CXLBaseTools.dll flags=0x00000900 process_architecture=x64
```

No `LOAD_RETURNED_SUCCESS` or ordinary loader-error record was emitted. Each child then exited signed `-1`, unsigned `0xFFFFFFFF`, without timeout. The executed timestamps were:

| ID | Start UTC | End UTC | Process ID |
|---|---|---|---:|
| A1 | `2026-08-29T05:12:06.1553222Z` | `2026-08-29T05:12:06.1946978Z` | 1600 |
| A2 | `2026-08-29T05:12:06.3079246Z` | `2026-08-29T05:12:06.3309957Z` | 14808 |
| A3 | `2026-08-29T05:12:06.3329961Z` | `2026-08-29T05:12:06.3540047Z` | 6000 |

The probe executable, arguments, target CXL path, and CXL SHA were identical across A1-A3. A1 used inherited PATH. A2 and A3 used the same inherited environment with `D:\apps\AMDuProf\bin;` prepended to the child PATH. `AMDPROFILERPATH`, `TEMP`, `TMP`, `PATHEXT`, `ComSpec`, and processor architecture were unchanged in the captured relevant environment.

`WORKING_DIRECTORY_ONLY = DISPROVEN` by A1. `DLL_SEARCH_PATH_ONLY = DISPROVEN` by A2. The combined CWD plus child-PATH hypothesis is also disproven by A3.

## PATH-ONLY RESULT

A2 failed with the repository CWD and a child-only PATH prefix. The inherited PATH already contained `D:\apps\AMDuProf\bin` later in the value; A2 made only the explicit prepend change. A3 added both the AMD-bin CWD and the same child-only PATH prefix and failed identically.

No persistent user or system PATH was changed. No B2/E2 controls were run because A2 did not return a successful CXL load; no B3/E3 controls were run because A3 also failed. Their `NOT_RUN` JSON records are preserved and are not failures.

## COMBINED CONTEXT RESULT

The result is not explained by either of the tested simple context dimensions, alone or together:

```text
A1: AMD-bin CWD + inherited PATH       -> CXL FatalExit / 0xFFFFFFFF
A2: repository CWD + AMD-bin child PATH -> CXL FatalExit / 0xFFFFFFFF
A3: AMD-bin CWD + AMD-bin child PATH    -> CXL FatalExit / 0xFFFFFFFF
```

Therefore:

```text
CXL_CONTEXT_REQUIREMENT = CLI_INITIALIZATION_STATE
MINIMAL_CXL_REQUIRED_CONTEXT = CLI_SPECIFIC_INITIALIZATION_STATE
```

This is a residual classification: the remaining difference may be CLI-specific module initialization order, process state, configuration discovery, or another unmeasured vendor context. It is not evidence of one exact private prerequisite.

## API LOAD RESULT

`API_DIRECT_LOAD = NOT_RUN` for this matrix. The harness correctly gated API controls because no tested context returned a successful direct CXL load. The earlier independent Administrator direct API control remains historical evidence of `-1 / 0xFFFFFFFF` through the transitive CXL path; it is not silently replaced by a new result.

The condition `CXL_LOAD_SUCCEEDS_BUT_API_TRANSITIVE_INITIALIZATION_STILL_FAILS` was not observed because CXL never succeeded in A1-A3.

## INIT-ONLY RESULT

`CTX-B1`, `CTX-E1`, `CTX-B3`, and `CTX-E3` are explicit `NOT_RUN` records gated on CXL failure. No init-only child reached library load return, symbol resolution, `AMDTPwrProfileInitialize`, enumeration, or close in this matrix. The highest reached gate was the direct CXL child's `BEFORE_LOAD` emission followed by process termination.

```text
INIT_ONLY = NOT_RUN
enumerated descriptor count = N/A
```

No sampling or production metric conclusion follows from this matrix.

## CXL FATAL EXIT

The current matrix preserves the exact process result `-1 / 0xFFFFFFFF` and the load-only stdout boundary. The prior debugger evidence remains the only captured callsite evidence:

```text
KERNEL32!FatalExit(0xFFFFFFFF)
CXLBaseTools!gtString::asWideString+0x458
CXLBaseTools!gtStringTokenizer::getNextToken+0x2f96
ntdll!LdrLoadDll
KERNELBASE!LoadLibraryExW
```

No new debugger trace was run for this evidence-consumption task. `EXACT_CXL_INTERNAL_CONDITION = UNPROVEN` remains unchanged.

## ROOT CAUSE REFINEMENT

```text
ROOT_CAUSE = DEPENDENCY_LOAD_FAILURE
SUBCAUSE = CXLBASETOOLS_LOAD_PATH_FATAL_EXIT
SIMPLE_CWD_PATH_CONTEXT = DISPROVEN
EXACT_CXL_INTERNAL_CONDITION = UNPROVEN
```

Confidence is high that the three tested simple CWD/PATH contexts are insufficient, because all three independent child observations reached the same fatal boundary with complete capture. Confidence is intentionally low about the remaining CLI-specific state because no module-load trace or private vendor initialization evidence was collected here.

The matrix does not support attributing the result to Hyper-V, VBS, HVCI, missing CRT, or application control. Those prior hypotheses remain separately bounded by their existing evidence; no platform or security configuration was changed.

## PRODUCT IMPLICATION

- Do not call `SetCurrentDirectory("D:\\apps\\AMDuProf\\bin")` in production; Windows CWD is process-global shared state.
- Do not persistently modify user or system PATH.
- The AMD CLI's prior successful power output does not approve the direct public API for Resource Timeline.
- No provider, production collector, ProviderHost, CollectionPlan, MetricCatalog, schema, dashboard, or UI change was made.
- Any future CLI/direct-API trace must first capture the successful CLI's module initialization context. It must not brute-force DLL preload permutations or patch vendor binaries.

## METRIC DECISIONS

The API data path was not reached in this matrix:

- `CPU_PACKAGE_POWER = DEFER`.
- `AMD_CORE_EFFECTIVE_FREQUENCY = DEFER`.
- `CPU_EFFECTIVE_FREQUENCY = DEFER_AGGREGATION_CONTRACT`.
- `CPU_PACKAGE_TEMPERATURE = DEFER_UNREACHED_DUE_TO_LIBRARY_LOAD`.

The prior successful CLI samples remain vendor-CLI evidence only and are not used to approve Resource Timeline metrics.

## VALIDATION

- Source code unchanged; no diagnostic rebuild was performed.
- Evidence files and child stdout/stderr were read and cross-checked.
- Artifact SHA identities were checked against the authoritative values; the API expected-hash discrepancy was recorded as a harness metadata defect.
- `git diff --check`: PASS before this documentation addition.
- Full Rust suites were not rerun because this phase changed measurement documentation only.

## DELIVERY

This document is the only repository change for the evidence-consumption phase. No AMD DLL, driver, installer, header, PDF, sample, license, dump, or machine-specific binary was committed. The existing qualification branch is the delivery target; push and commit status are reported separately with the task result. Draft-PR visibility is unavailable without `gh`.

## USER_AUTHORIZATION_REQUIRED

No system mutation is requested. The next loader-context trace, if pursued, must remain a manually authorized Administrator operation. No elevation, reboot, installer action, service/driver action, PATH/registry/security change, or Hyper-V/VBS/HVCI change is authorized or required by this record.

## NEXT STEP

```text
AUTHORIZED_CLI_VS_DIRECT_API_LOADER_CONTEXT_TRACE
```

The next task should capture ordered module-load events and process-tree context for one successful Administrator `AMDuProfCLI timechart` control and one representative failing direct load. Do not begin `CPU-SENSOR-AMD-PROVIDER-DESIGN`.
