# AMD Runtime Prerequisite Audit Q1

Task ID: AMD-RUNTIME-PREREQUISITE-AUDIT-Q1

Audit date: 2026-09-09 (Asia/Shanghai)

Repository: WuKong512/win-resources-timeline

Audit mode: OFFLINE_READ_ONLY

This document is the detailed source of truth for the Q1 prerequisite audit.
It records what is statically established, what is only historically observed,
and what cannot be answered without a live AMD operation or a machine-state
mutation. It does not authorize either kind of operation.

The controlled finding statuses used below are:
SATISFIED, UNSATISFIED, UNKNOWN, NOT_APPLICABLE, NOT_EVALUATED, and
UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE. Descriptive qualifiers such as
“version difference observed” are evidence qualifiers, not replacements for a
controlled status.

## 1. Entry state

The repository entry gate was completed before any documentation change:

~~~text
TASK_ID = AMD-RUNTIME-PREREQUISITE-AUDIT-Q1
REPOSITORY = WuKong512/win-resources-timeline
ENTRY_HEAD = 3ffc2bdd2c901e147aa9618d2968e08c7e8107f8
ENTRY_MAIN = 3ffc2bdd2c901e147aa9618d2968e08c7e8107f8
ORIGIN_MAIN_AT_ENTRY = 3ffc2bdd2c901e147aa9618d2968e08c7e8107f8
BASE_IS_ANCESTOR = YES
PR27_MERGED = YES
PR27_MERGE_COMMIT = 3ffc2bdd2c901e147aa9618d2968e08c7e8107f8
ENTRY_WORKING_TREE = CLEAN
ENTRY_BRANCH = DETACHED_AT_ORIGIN_MAIN
FETCH_ORIGIN_PRUNE = PASS
BRANCH_CREATED = audit/amd-runtime-prerequisite-q1
BRANCH_BASE = origin/main
~~~

PR #27 is represented by the merge commit at the entry head:
“Merge pull request #27 from WuKong512/audit/amd-operation-path-evidence-gap-d1”.
The duplicate gate searched the fetched history, branches, refs, and relevant
AMD documentation for the exact task ID. The matches were inherited
selected-next-task markers, not an authoritative completed or active duplicate.
No authoritative duplicate was found.

The audit branch was created once from the fetched origin/main. No main branch
edit, rebase, merge, reset, force push, or history rewrite is part of this
audit.

## 2. Inherited authoritative state

The following documents were read as the minimum entry set:

- docs/upgrade/amd-operation-path-evidence-gap-d1.md
- docs/upgrade/amd-cli-list-path-validity.md
- docs/upgrade/amd-post-i2g-production-admission.md
- docs/upgrade/execution-plan.md
- tools/amd-privilege-qualification/README.md

Relevant measurement records and the historical I2G/I2F records were also
reviewed. Historical raw records were not rewritten.

The inherited state remains frozen unless this document explicitly adds a
current Q1 marker:

~~~text
AMD_CLI_LIST_PATH_VALIDITY_Q1 = COMPLETE / INSUFFICIENT
PROXY_VERDICT = INSUFFICIENT
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
AUTHORIZATION_EQUIVALENCE = UNKNOWN
PRODUCTION_ACCOUNT = UNRESOLVED
AMD_PRODUCTION_ADMISSION = DEFER
I2H_JUSTIFIED = NO
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
LIVE_PAIR_AUTHORIZED = NO
I2G_CURRENT_STATE = COMPLETE / ATTEMPT3_AUTHORITATIVE
I2G_REAL_GATE_CONSUMED = true
I2G_REAL_EXECUTION_ALLOWED = false
I2G_REAL_CLEANUP_ALLOWED = false
~~~

The authoritative historical I2G result is a paired LocalService/Session 0
context in which both control and treatment returned POWER_UNAVAILABLE and no
power-sampling run occurred. It does not generalize to every account or
environment. The historical qualification check remains blocked by the
pre-existing pinned release artifact SHA256 mismatch; that blocker is outside
this documentation-only audit and was not repaired.

## 3. Safety and non-execution boundary

This audit was limited to repository text, local vendor documentation and
headers/source, file identity and Authenticode metadata, static import evidence
already recorded by the repository, registry, service and driver metadata,
read-only OS/CPU/BIOS/security state, environment values, and bounded
installed-tree inspection.

The following were prohibited and were not performed:

- launching AMDuProfCLI.exe, AMDuProf.exe, AMDProfilerService.exe,
  AMDProfilerLoadService.exe, or any other AMD profiling executable;
- loading AMDPowerProfileAPI.dll or invoking an AMD API;
- running --list, active sampling, a qualification test, or a production
  profile;
- starting, stopping, creating, deleting, or reconfiguring a service;
- changing LSA policy, token privileges, ACLs, registry, devices, drivers,
  platform security, BIOS, or environment state;
- opening AMD device handles, issuing IOCTLs, using named AMD IPC objects, or
  attempting I2H/Attempt #4.

Existing AMD processes and services were observed only as read-only machine
state. The audit did not start or attach to them.

## 4. Evidence inventory

| Evidence ID | Source | Authority and use | Observed evidence | Boundary |
| --- | --- | --- | --- | --- |
| E1 | Installed tree at D:\apps\AMDuProf | Local package identity and completeness | bin, Examples, Help, include, Legal, and lib are present; expected CLI, API, CXL, service, headers, sample, help, and manual are present | No package manifest proves that every user-mode component is compatible with every installed driver |
| E2 | AMDuProf timechart help | Vendor-documented CLI semantics | --list is supported-device/category discovery; --event is counter collection; output-dir defaults to the current working directory when omitted | Help does not disclose the exact initialization, account, device ACL, or backend gate behind --list |
| E3 | AMDProfilerService help | Vendor-documented remote-service semantics | The executable documents a remote target server, user registration/clear operations, and an authentication bypass option | It does not prove that the local CLI depends on this service; no service configuration was changed |
| E4 | AMDPowerProfilerAPI.pdf, installed headers, and sample | Vendor-documented public API sequence and errors | Initialize, enumerate, enable, set timer, start, read, stop, and close; documented errors include driver unavailable, version mismatch, access denied, counter inaccessible, BIOS unsupported, SMU failure, and hypervisor unsupported | Documentation exposes failure classes but does not answer the current machine's live counter accessibility |
| E5 | Static binary identity and import evidence | Component provenance and route shape | x64 user-mode files are AMD-signed; repository static analysis records the CLI/API/CXL graph and no CXL public header/library | Static imports do not prove which code path or authorization gate a command takes at runtime |
| E6 | Registry and uninstall metadata | Installation and environment identity | AMD uProf 5.3.521 is registered at D:\apps\AMDuProf; AMDPROFILERPATH and PATH point to its bin directory | Registry state is not a runtime authorization proof |
| E7 | Service registry and sc.exe query/qc | Read-only backend/driver presence | AMDPowerProfiler and AMDCpuProfiler are running kernel drivers; AMDProfilerLoadService is running as LocalSystem; AMDProfilerService is not registered | A running service/driver does not prove that the current account can use a counter |
| E8 | Driver files and signatures | Driver file identity | AMDPowerProfiler.sys and AMDCpuProfiler.sys are present in System32\drivers and AMD-signed; their product metadata is 5.3.481.0 | No PnP/INF association was recovered; compatibility with user-mode 5.3.521 remains open |
| E9 | OS/CPU/BIOS/DeviceGuard registry and processor feature query | Platform context | AMD Ryzen 7 9700X, AMD Family 26 Model 68; Windows version 10.0.26200.9168, DisplayVersion 25H2; BIOS 3222; hypervisor present; VBS/HVCI enabled | WMI and SecureBoot confirmation were access-denied; registry labels and inherited Windows classification disagree |
| E10 | PnP and driver-package inspection | Device/package corroboration | PnP WMI/Get-PnpDevice queries were access-denied; pnputil showed AMD platform packages but no uProf package by name; no duplicate uProf install was found in bounded roots | Negative PnP results are not authoritative under the access restriction |
| E11 | Historical repository measurements | Feasibility and confounder context | Admin CLI sampling and SYSTEM discovery were historically positive; non-admin/LocalService discovery and I2G paired discovery were negative in their named contexts | Historical success is not current production admission and does not close account/ACL/backend confounders |
| E12 | Existing qualification state | Safety and test boundary | I2G gate consumed, rollback clean, Attempt #4 not authorized, qualification test blocked by pre-existing artifact hash mismatch | No qualification or live operation was rerun |

Key identity rechecks from E1, E5, and E8:

| Path | Identity evidence |
| --- | --- |
| D:\apps\AMDuProf\bin\AMDuProfCLI.exe | SHA256 D0812D...1FBAC; File/Product 5.3.521.0; Authenticode Valid; AMD signer |
| D:\apps\AMDuProf\bin\AMDPowerProfileAPI.dll | SHA256 963402...A4277; File/Product 5.3.521.0; Authenticode Valid; AMD signer |
| D:\apps\AMDuProf\bin\CXLBaseTools.dll | SHA256 4815D...F8931; File/Product 5.3.521.0; Authenticode Valid; AMD signer |
| D:\apps\AMDuProf\bin\AMDProfilerService.exe | SHA256 DB2C537...7F255; File/Product 5.3.521.0; Authenticode Valid; AMD signer |
| D:\apps\AMDuProf\bin\AMDProfilerLoadService.exe | SHA256 43A747...14516; File/Product 5.3.521.0; Authenticode Valid; AMD signer |
| C:\Windows\System32\drivers\AMDPowerProfiler.sys | SHA256 C3C959...04123; File 10.6.3.0; Product 5.3.481.0; Authenticode Valid; AMD signer |
| C:\Windows\System32\drivers\AMDCpuProfiler.sys | SHA256 39EB5C...12BCD; File 4.4.1.0; Product 5.3.481.0; Authenticode Valid; AMD signer |

The abbreviated hashes above are a compact identity index. The full hashes
remain in the existing measurement and upgrade records and were not replaced
by this summary.

## 5. Installation identity and completeness matrix

| Prerequisite / source | Documented requirement | Current state | Satisfaction | Confidence | Production relevance | Remaining uncertainty | Live or mutate needed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| uProf installation identity / E1, E6 | A usable AMD uProf installation must be identifiable | D:\apps\AMDuProf; registry ProductVersion 5.3.521; bounded command resolution finds one uProf tree | SATISFIED | HIGH | Establishes which package is being discussed | Installer provenance and package manifest are not available | No |
| User-mode CLI/API/CXL files / E1, E5 | CLI route and public API support must be present | CLI, API DLL, CXL, common/data-accessor libraries, help, headers, and sample are present and AMD-signed | SATISFIED | HIGH | Required for a candidate provider package | Exact route dependency remains a runtime question | No for presence; yes for route behavior |
| Documentation/sample completeness / E1, E2, E4 | Vendor usage instructions and public API contract should be available | Help, API PDF, headers, and sample source are present; sample requires the uProf driver | SATISFIED | HIGH | Supports implementation and review | Manual v1.2 may be older than the installed header; exact release alignment is not proven | No |
| Kernel driver presence / E7, E8 | Vendor sample/API requires the uProf driver | AMDPowerProfiler.sys and AMDCpuProfiler.sys exist, are registered, running, and AMD-signed | SATISFIED | HIGH for presence | Necessary static/installed prerequisite | Functional loading, device accessibility, and user-mode compatibility remain open | Yes for functionality |
| Driver package/INF provenance / E10 | A complete deployment should expose a recoverable package association | No uProf INF/package entry was found by the permitted bounded checks; PnP queries were denied | UNKNOWN | LOW-MEDIUM | Affects upgrade, repair, and reproducibility | Whether the drivers were installed by an older package or another deployment path | Possibly read-only elevated inspection |
| Duplicate installation / E1, E10 | One coherent install should be selected | Bounded search found only D:\apps\AMDuProf copies for the named uProf executables/libraries | SATISFIED | MEDIUM-HIGH | Reduces path ambiguity | Unsearched arbitrary user directories could still contain copies | No within audit scope |
| Build-project path alignment / E1 | Samples should resolve include/lib paths to the selected install | Sample project defaults reference C:\Program Files\AMD\AMDuProf while selected install is D:\apps\AMDuProf | UNKNOWN | HIGH for the observed difference | Affects rebuilding the sample, not proof of installed runtime failure | Build-time override and exact sample build compatibility were not tested | Build/test would be outside this audit |

Overall installation identity is SATISFIED. Overall installation completeness is
UNKNOWN because the named runtime files are present but package provenance,
driver association, and full user-mode/driver compatibility are not closed.

## 6. Runtime and component coherence matrix

| Component relationship | Source / evidence | Current state | Satisfaction | Confidence | Production relevance | Remaining uncertainty | Live or mutate needed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| CLI to API/CXL user-mode version | E1, E5 | CLI/API/CXL and most user-mode components report 5.3.521.0; static graph is recorded | SATISFIED | HIGH | Supports a coherent user-mode candidate | Exact executable-directory and loader context for every entry point | No for identity; yes for runtime loader behavior |
| User-mode to kernel driver version | E8, historical records | User mode is 5.3.521.0; driver product metadata is 5.3.481.0 | UNKNOWN | HIGH for labels, LOW for compatibility conclusion | Could affect initialization, counters, and production repeatability | No package release manifest or vendor compatibility statement ties these exact labels together | Live initialize or vendor package evidence |
| API/header/error constant coherence | E4 | Installed header and API PDF document the same operation family; PDF is release v1.2 and may predate current binaries | UNKNOWN | MEDIUM-HIGH | Error interpretation and future diagnostics depend on exact release | Header/PDF release alignment is not fully proven | No for static comparison; yes for runtime behavior |
| CLI versus direct API loader path | E5, historical divergence record | CLI historically samples in Admin context; direct loader hit CXL/load divergence and a non-admin API ACCESSDENIED | UNKNOWN | HIGH | Determines whether CLI feasibility transfers to a service/provider | Executable directory, DLL search path, account, and backend differences are confounded | Live operation would be required |
| Same-context enumeration versus sampling | E2, E4, E11 | --list is documented discovery; active sampling is a separate configure/start/read/stop path; exact CLI mapping is absent | UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE | HIGH | This is the central production proxy question | No static artifact proves whether --list and sampling share the same gate | Live AMD operation prohibited here |

The overall runtime component coherence is UNKNOWN. The user-mode package is
internally identifiable, but the observed user-mode/driver product-version
difference must not be promoted to either a failure or compatibility proof.

## 7. Driver and backend prerequisite matrix

| Prerequisite / source | Documented requirement | Current state | Satisfaction | Confidence | Production relevance | Remaining uncertainty | Live or mutate needed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| AMDPowerProfiler driver installed | API PDF/sample describe the uProf driver as required | File exists, service registration exists, state is RUNNING, AMD signature is valid | SATISFIED | HIGH for presence | Required for power profiling | Whether this driver exposes usable counters to the candidate account | Yes |
| Driver version compatibility | Header/API exposes driver unavailable and version mismatch errors | Driver product metadata is 5.3.481.0 against user-mode 5.3.521.0; no mismatch error was observed because no API was loaded | UNKNOWN | HIGH for observation | Affects deployment acceptance | Vendor compatibility matrix and current initialization result absent | Yes or authoritative package evidence |
| AMDProfilerLoadService | Registry/service state records it as AMD uProf Driver Load Service | Registered, AUTO_START, RUNNING, LocalSystem, image under selected D install | SATISFIED | HIGH | Relevant to driver lifecycle | Exact dependency and CLI interaction are not documented in reviewed artifacts | Live service/backend inspection or vendor evidence |
| AMDProfilerService local registration | Help describes a remote target server, not necessarily a local provider dependency | Executable is present and signed; service name AMDProfilerService is not registered | NOT_APPLICABLE | HIGH | Matters only if remote service mode is selected | Whether production would use remote mode was not specified | No |
| Driver/device usability | API initialization and counter enumeration must succeed | Cannot be established without loading the API or executing a live CLI path | UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE | HIGH | Direct admission criterion | Device ACL, backend authorization, SMU access, and counter state | Yes |

The driver prerequisite is therefore UNKNOWN overall: installed presence is
SATISFIED, but functional usability and version compatibility are not.

## 8. CPU and platform support matrix

| Prerequisite / source | Documented requirement | Current state | Satisfaction | Confidence | Production relevance | Remaining uncertainty | Live or mutate needed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| CPU vendor/family | AMD uProf power path targets supported AMD platforms | AuthenticAMD; AMD Ryzen 7 9700X 8-Core Processor; AMD64 Family 26 Model 68 Stepping 0 | SATISFIED | HIGH | Strongly identifies the target platform | Exact model support table is not reproduced in the installed help | No for identity |
| OS support | Repository-captured AMD support material records Windows 11 support through 26H1 | Registry reports Windows version 10.0.26200.9168, DisplayVersion 25H2, ProductName Windows 10 Pro; inherited records classify this machine as Windows 11 Pro 25H2 | UNKNOWN | HIGH for the contradiction | OS support is a production prerequisite | ProductName label versus inherited classification needs authoritative OS/vendor resolution | Read-only authoritative evidence or later review |
| Architecture | x64 user-mode/driver route must match the process and binary architecture | Is64BitOperatingSystem=true; Is64BitProcess=true; AMD CLI is the x64 target used by historical records | SATISFIED | HIGH | Prevents x86/x64 route mismatch | No new process was launched | No |
| BIOS/platform firmware | API/header exposes BIOS unsupported as a possible failure | BIOS American Megatrends Inc., version 3222, release date 2025-03-05; ASUS TUF GAMING B650M-E WIFI | UNKNOWN | HIGH for observation | Firmware can determine SMU/platform support | Minimum/supported BIOS matrix and live initialization result absent | Yes for actual compatibility |

CPU identity and x64 architecture are statically SATISFIED. The overall
CPU/platform support field remains UNKNOWN because the OS edition label is
contradictory and the exact BIOS support boundary is not documented in the
available local artifacts.

## 9. Hypervisor, VBS, and HVCI matrix

| Prerequisite / source | Documented requirement or limitation | Current state | Satisfaction | Confidence | Production relevance | Remaining uncertainty | Live or mutate needed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Hypervisor presence | Header/API includes a hypervisor-unsupported error; repository AMD material records a temperature limitation under Microsoft Hypervisor | IsProcessorFeaturePresent(PF_HYPERVISOR_PRESENT) returned true; vmcompute and hvhost are running; vmms is absent | UNKNOWN | HIGH for observation | Can change telemetry support and interpretation | Whether the current power path is supported in this exact state | Yes for current runtime behavior |
| VBS | Security state can affect driver/platform operation | DeviceGuard EnableVirtualizationBasedSecurity=1 | UNKNOWN | HIGH | Relevant to production security and driver behavior | No AMD-specific compatibility result | Yes for current runtime behavior |
| HVCI/Memory Integrity | Security state can affect signed driver loading | HypervisorEnforcedCodeIntegrity Enabled=1; Microsoft-signed boot-chain requirement=1 | UNKNOWN | HIGH | Relevant to driver lifecycle and support | No live load/initialization result was obtained | Yes for current runtime behavior |
| Temperature-specific limitation | Inherited AMD material says temperature is not supported under the documented hypervisor limitation | Current environment has a hypervisor; no temperature operation was attempted | NOT_EVALUATED | HIGH | Temperature provider cannot be admitted from this audit | Exact applicability to this driver/build | Would require prohibited runtime operation |
| Power-specific historical signal | Historical Admin/SYSTEM CLI records collected package power while the machine had the recorded hypervisor/VBS context | Historical feasibility exists, but it is not a current same-account proof | UNKNOWN | MEDIUM | Supports only conditional feasibility, not admission | Account, version, and backend differences remain | Yes for current proof |

No current documented blocker was assigned solely from VBS/HVCI. The evidence
supports a documented temperature limitation and leaves current power-path
compatibility UNKNOWN.

## 10. Device, interface, and accessibility matrix

| Prerequisite / source | Documented requirement | Current state | Satisfaction | Confidence | Production relevance | Remaining uncertainty | Live or mutate needed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Kernel driver service object | Driver registration must exist for the API to initialize | AMDPowerProfiler and AMDCpuProfiler service registrations exist and are RUNNING | SATISFIED | HIGH | Establishes installed driver state | Device object/interface exposure | No for presence |
| PnP/INF exposure | A deployable driver normally has a recoverable package/device association | Get-PnpDevice and WMI signed-driver queries were access-denied; pnputil did not show a uProf package by name | UNKNOWN | LOW-MEDIUM | Affects repair and device-level diagnostics | Elevation-restricted PnP state and possible non-PnP driver loading | Possibly read-only elevated inspection |
| Device symbolic link/interface GUID | A user-mode client needs an accessible device/backend path | Prior bounded static searches found no AMD power symbolic link, interface GUID, named kernel object, named pipe, or device ACL | UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE | MEDIUM | Directly affects account authorization and service design | No current handle/open or IOCTL is permitted | Yes |
| AMDTPwrDevice.m_isAccessible | Header says this indicates whether counters are accessible | Field and category mask are documented; no current object was materialized | UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE | HIGH | Direct counter admission criterion | Account, device ACL, SMU, BIOS, and backend state | Yes |
| Counter accessibility | Header/API expose counter-not-accessible and SMU-access-failed errors | No current counter list or API result was obtained | UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE | HIGH | Determines whether power data can be emitted | Exact counters and failure status in current context | Yes |

The device/interface and counter-accessibility prerequisites remain open. No
negative PnP result was treated as proof of a missing runtime device because
the permitted PnP queries were access-denied and no device was opened.

## 11. AMDProfilerService and backend semantics

The installed AMDProfilerService.exe is a signed 5.3.521.0 file. Its vendor
help describes a remote target server for AMD uProf clients, including user
registration/clearing and an authentication-bypass option. The service name
AMDProfilerService is not registered on this machine.

AMDProfilerLoadService.exe is a different signed 5.3.521.0 component. Its
registry/service identity is AMD uProf Driver Load Service, and it is running
as LocalSystem. The reviewed local help does not establish that it is the
same backend as AMDProfilerService or that the CLI must use either service for
local discovery/sampling. AmdPpkgSvc is a separate AMD provisioning service
and was not promoted to a uProf prerequisite.

| Question | Source/authority | Current state | Satisfaction | Confidence | Production relevance | Remaining uncertainty | Live or mutate needed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Is the remote AMDProfilerService installed as a service? | E3, service registry | File present; service not registered | NOT_APPLICABLE | HIGH | Only relevant to remote mode | Production topology was not specified | No |
| Is AMDProfilerLoadService present and running? | E7, E8 | Yes; LocalSystem; running | SATISFIED | HIGH | Relevant to driver lifecycle | Exact behavior/dependency unknown | No for presence |
| Is a service/backend authorization gate required by local CLI? | E2, E3, E5, E11 | Not proven statically; historical contexts differ | UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE | HIGH | Central to production account choice | CLI-to-service-to-driver path and ACLs | Yes |
| Is remote authentication equivalent to local account authorization? | E3, inherited records | No evidence of equivalence | UNKNOWN | HIGH | Would invalidate a proxy based on one route | Remote/local topology and auth policy | Yes or authoritative vendor evidence |

Overall AMD backend service prerequisite: UNKNOWN.

## 12. Environment and process-context matrix

| Prerequisite / source | Documented requirement | Current state | Satisfaction | Confidence | Production relevance | Remaining uncertainty | Live or mutate needed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| AMDPROFILERPATH | Selected installation must be resolvable by the client | Machine environment contains AMDPROFILERPATH=D:\apps\AMDuProf\bin | SATISFIED | HIGH | Reduces DLL/path ambiguity | Per-service environment inheritance was not exercised | No for current value |
| PATH/command resolution | CLI and its libraries should resolve from one selected tree | PATH contains D:\apps\AMDuProf\bin; Get-Command resolves the named AMD files only there | SATISFIED | HIGH | Supports reproducible launch identity | Loader behavior in a future service context | No for static resolution |
| Working directory | Vendor help says omitted output-dir uses current working directory; direct loader evidence shows executable directory can affect CXL loading | Current audit CWD is the repository; historical working CLI CWD was the uProf bin directory | UNKNOWN | HIGH | A production wrapper must define CWD and loader roots | Whether CLI --list/sampling requires bin CWD in the exact release | Yes for route behavior |
| Output directory | Vendor help defines output-dir creation/session-subdirectory behavior | Historical service run used a ProgramData qualification root; current audit did not write output | UNKNOWN | HIGH | Ownership, cleanup, retention, and isolation are production requirements | Production root and ACL contract not established | A real run would write state and is prohibited |
| TEMP/TMP | Runtime may use temporary files and must have a safe writable location | Current process TEMP/TMP are the user's local temp directory | UNKNOWN | MEDIUM | Service and production account may inherit different temp paths | Service-context temp and cleanup behavior | Live process context |
| File ACLs | Client must read/execute user-mode files; production should control output and helper paths | uProf bin grants SYSTEM/Admin FullControl and Users ReadAndExecute; root ownership is mixed across root/bin | SATISFIED | HIGH | Helps static path feasibility | Service account, output ACL, and DLL side-loading boundary | No for observed ACL; yes for production validation |
| Current process context | Account/session/bitness can affect access and backend behavior | Audit process is x64 in the repository worktree; production account is unresolved | UNKNOWN | HIGH | Account/session is a known historical confounder | Exact production token, session, groups, privileges, and service environment | A live authorized context |

The environment prerequisite is UNKNOWN overall. Static path resolution is
SATISFIED, but working-directory, output ownership, temp, and service-context
loader behavior are not closed.

## 13. Account and authorization documentation boundary

The reviewed local vendor help, API PDF, installed headers, and sample document
the driver requirement and expose ACCESSDENIED/counter-inaccessible errors, but
do not state a definitive required Windows account, Administrator membership,
or named runtime privilege for local API use. Installing a kernel driver is a
deployment action that normally requires administrative authority, but that is
not evidence of a required production sampling account.

Historical evidence is separated from documentation:

| Evidence | What it shows | What it does not show |
| --- | --- | --- |
| Historical Admin CLI sampling | One interactive Admin context could collect package power | It does not prove least privilege, service behavior, or current package compatibility |
| Historical SYSTEM Session 0 discovery | SYSTEM could enumerate in its named context | It does not prove SYSTEM is the production account or that all services share its token |
| Historical non-admin API/CLI ACCESSDENIED | A named non-admin context hit access denied | It does not isolate account, device ACL, service SID, driver version, CXL, or backend cause |
| Historical LocalService/I2G discovery | The named LocalService paired contexts returned POWER_UNAVAILABLE | It does not prove a single missing privilege or a universal LocalService failure |

Therefore:

~~~text
ACCOUNT_REQUIREMENT_DOCUMENTED = UNKNOWN / NOT_ESTABLISHED
PRIVILEGE_REQUIREMENT_DOCUMENTED = UNKNOWN / NOT_ESTABLISHED
PRODUCTION_ACCOUNT = UNRESOLVED
AUTHORIZATION_EQUIVALENCE = UNKNOWN
~~~

No new privilege experiment, account switch, token adjustment, LSA mutation,
or production-account selection was performed or authorized.

## 14. Contradictions and unknowns

| Observation | Classification | Effect on conclusion |
| --- | --- | --- |
| An older CPU-sensor record says no uProf installation, while D1 and the current read-only inventory identify D:\apps\AMDuProf | Temporal contradiction; older snapshot is historical | Do not rewrite the historical record; use the current inventory for current state |
| User-mode files report 5.3.521.0 while uProf driver product metadata reports 5.3.481.0 | Version difference observed | Compatibility is UNKNOWN, not UNSATISFIED, because no vendor mismatch result or package manifest was obtained |
| Current registry ProductName is Windows 10 Pro while inherited records classify Windows 11 Pro 25H2 and build 26200 | OS identity-label contradiction | OS support mapping is UNKNOWN and needs authoritative resolution |
| AMDProfilerService.exe exists but its service is not registered; AMDProfilerLoadService is registered/running | Component semantic distinction | Do not treat file presence as a running remote service or infer CLI dependency |
| PnP queries were access-denied and no uProf package appeared in pnputil output | Inspection limitation | Do not infer that the running kernel drivers lack device exposure |
| Historical Admin/SYSTEM paths succeeded while other contexts failed | Contextual differential | Feasibility is conditional; account/backend/ACL/version confounders remain |
| Direct API loading diverged from the CLI path around CXL/executable-directory behavior | Loader-path divergence | CLI feasibility cannot be transferred to a new API wrapper or service without separate evidence |
| Vendor API exposes BIOS, hypervisor, SMU, access-denied, and counter-accessibility failure classes | Documented possible blockers, not observed current failures | No one is promoted to a current documented blocker without a current result |
| Qualification test is blocked by a pinned artifact SHA mismatch | Pre-existing external blocker | It is recorded, not repaired or used as evidence of AMD runtime failure |

Material unknowns remain for driver/user-mode compatibility, current API
initialization, actual counter/device accessibility, exact CLI initialization
and backend mapping, BIOS support, OS support-label resolution, production
account/privilege, service-context environment, and production output/ACL
semantics.

## 15. Production relevance assessment

The audit establishes a credible installed user-mode package, signed driver
files, running driver/service registrations, AMD CPU identity, x64 context,
and a statically coherent path. It does not establish that a production
account can enumerate or sample counters, that the user-mode/driver versions
are supported together, that the current BIOS/OS/hypervisor/VBS/HVCI state is
compatible for the intended power provider, or that service/output ownership
is production-safe.

The historical Admin and SYSTEM results are feasibility evidence only. They
are not least-privilege qualification, current same-context evidence, or
production admission. The separate legal/distribution review recorded in
inherited documents also remains a production gate; it is not relabeled as a
runtime prerequisite failure here.

~~~text
INSTALLATION_IDENTITY = SATISFIED
INSTALLATION_COMPLETENESS = UNKNOWN
RUNTIME_COMPONENT_COHERENCE = UNKNOWN
AMD_DRIVER_PREREQUISITE = UNKNOWN
AMD_BACKEND_SERVICE_PREREQUISITE = UNKNOWN
CPU_PLATFORM_SUPPORT = UNKNOWN
BIOS_PREREQUISITE = UNKNOWN
HYPERVISOR_PREREQUISITE = UNKNOWN
VBS_HVCI_PREREQUISITE = UNKNOWN
DEVICE_INTERFACE_PREREQUISITE = UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE
COUNTER_ACCESSIBILITY_PREREQUISITE = UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE
ENVIRONMENT_PREREQUISITE = UNKNOWN
WORKING_DIRECTORY_REQUIREMENT = UNKNOWN
OUTPUT_PATH_REQUIREMENT = UNKNOWN
ACCOUNT_REQUIREMENT_DOCUMENTED = UNKNOWN / NOT_ESTABLISHED
PRIVILEGE_REQUIREMENT_DOCUMENTED = UNKNOWN / NOT_ESTABLISHED
PRODUCTION_ACCOUNT = UNRESOLVED
PROXY_VERDICT = INSUFFICIENT
AMD_PRODUCTION_ADMISSION = DEFER
~~~

## 16. Next-route decision

The Q1 audit is complete as an offline characterization, but it is not
sufficient evidence for production admission:

~~~text
RUNTIME_PREREQUISITE_AUDIT = COMPLETE / INSUFFICIENT_EVIDENCE
RUNTIME_PREREQUISITE_CONFIDENCE = MEDIUM
DOCUMENTED_BLOCKER_FOUND = NO
DOCUMENTED_BLOCKER = NONE_ESTABLISHED; LEGAL/DISTRIBUTION REVIEW REMAINS A SEPARATE PRODUCTION GATE
MATERIAL_UNKNOWNS_REMAIN = YES
~~~

The remaining questions are not one narrowly resolvable read-only family. The
most consequential ones require an AMD API/CLI operation, a live account
context, or a mutating service/device/authorization test. Consequently the
single selected route is:

~~~text
SELECTED_NEXT_TASK = NONE / HUMAN_REVIEW_REQUIRED
SELECTED_NEXT_TASK_GOAL = NO_UNIQUE_LOWER_RISK_READ_ONLY_TASK; REVIEW_Q1_EVIDENCE_BEFORE_ANY_SEPARATELY_AUTHORIZED_LIVE_OPERATION
NEXT_GATE = HUMAN_REVIEW_RUNTIME_PREREQUISITE_AUDIT
~~~

This route does not nominate a live pair, an I2H run, Attempt #4, a privilege
test, a production-account switch, or a qualification test. If a future task
is separately authorized, it must define its account, session, binary
identity, version contract, output root, safety gate, and rollback before any
AMD invocation.

## 17. Explicit non-execution record

~~~text
AUDIT_MODE = OFFLINE_READ_ONLY
AMD_CLI_REAL_INVOCATIONS = 0
AMD_API_REAL_INVOCATIONS = 0
POWER_SAMPLING_RUNS = 0
SERVICE_MUTATIONS = 0
LSA_MUTATIONS = 0
TOKEN_MUTATIONS = 0
ACL_MUTATIONS = 0
DEVICE_MUTATIONS = 0
DRIVER_MUTATIONS = 0
PLATFORM_SECURITY_MUTATIONS = 0
AMD_EXECUTABLES_STARTED_BY_AUDIT = 0
AMD_APIS_LOADED_BY_AUDIT = 0
I2H_EXECUTED = NO
ATTEMPT4_CREATED = NO
LIVE_PAIR_AUTHORIZED = NO
QUALIFICATION_TEST_EXECUTED = NO
~~~

Read-only observations of already-running AMD service/process names do not
count as invocations or mutations and were not initiated by the audit.

## 18. Validation record

Documentation validation for this branch consists of:

~~~text
VALIDATION_GIT_DIFF_CHECK = PASS
VALIDATION_CURRENT_STATE_MARKERS = PASS
VALIDATION_POWERSHELL_PARSE = NOT_APPLICABLE / NO_POWERSHELL_FILE_MODIFIED
RUNTIME_CODE_CHANGED = NO
RUST_RUNTIME_CHANGED = NO
HISTORICAL_RAW_EVIDENCE_CHANGED = NO
QUALIFICATION_TEST = BLOCKED
QUALIFICATION_TEST_BLOCKER = PRE_EXISTING_PINNED_RELEASE_ARTIFACT_SHA256_MISMATCH
QUALIFICATION_BLOCKER_TASK_CAUSED = NO
QUALIFICATION_BLOCKER_SCOPE = PRE_EXISTING / OUT_OF_SCOPE / NOT_REPAIRED
~~~

The final patch validation must confirm that only the new audit document and
the four minimal current-state handoff markers are changed, that git diff
--check passes, and that no current authoritative marker still selects Q1 as
an unfinished task.

## 19. Current-state handoff

The following is the current Q1 handoff after this audit; it is intentionally
small enough to be copied into the existing authoritative handoff documents:

~~~text
AMD_RUNTIME_PREREQUISITE_AUDIT_Q1 = COMPLETE / INSUFFICIENT_EVIDENCE
RUNTIME_PREREQUISITE_AUDIT = COMPLETE / INSUFFICIENT_EVIDENCE
RUNTIME_PREREQUISITE_CONFIDENCE = MEDIUM
AUDIT_DOCUMENT = docs/upgrade/amd-runtime-prerequisite-audit-q1.md
INSTALLATION_IDENTITY = SATISFIED
INSTALLATION_COMPLETENESS = UNKNOWN
RUNTIME_COMPONENT_COHERENCE = UNKNOWN
AMD_DRIVER_PREREQUISITE = UNKNOWN
AMD_BACKEND_SERVICE_PREREQUISITE = UNKNOWN
CPU_PLATFORM_SUPPORT = UNKNOWN
BIOS_PREREQUISITE = UNKNOWN
HYPERVISOR_PREREQUISITE = UNKNOWN
VBS_HVCI_PREREQUISITE = UNKNOWN
DEVICE_INTERFACE_PREREQUISITE = UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE
COUNTER_ACCESSIBILITY_PREREQUISITE = UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE
ENVIRONMENT_PREREQUISITE = UNKNOWN
WORKING_DIRECTORY_REQUIREMENT = UNKNOWN
OUTPUT_PATH_REQUIREMENT = UNKNOWN
ACCOUNT_REQUIREMENT_DOCUMENTED = UNKNOWN / NOT_ESTABLISHED
PRIVILEGE_REQUIREMENT_DOCUMENTED = UNKNOWN / NOT_ESTABLISHED
DOCUMENTED_BLOCKER_FOUND = NO
MATERIAL_UNKNOWNS_REMAIN = YES
PROXY_VERDICT = INSUFFICIENT
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
AUTHORIZATION_EQUIVALENCE = UNKNOWN
PRODUCTION_ACCOUNT = UNRESOLVED
AMD_PRODUCTION_ADMISSION = DEFER
I2H_JUSTIFIED = NO
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
LIVE_PAIR_AUTHORIZED = NO
SELECTED_NEXT_TASK = NONE / HUMAN_REVIEW_REQUIRED
SELECTED_NEXT_TASK_GOAL = NO_UNIQUE_LOWER_RISK_READ_ONLY_TASK; REVIEW_Q1_EVIDENCE_BEFORE_ANY_SEPARATELY_AUTHORIZED_LIVE_OPERATION
NEXT_GATE = HUMAN_REVIEW_RUNTIME_PREREQUISITE_AUDIT
AMD_CLI_REAL_INVOCATIONS = 0
AMD_API_REAL_INVOCATIONS = 0
POWER_SAMPLING_RUNS = 0
SERVICE_MUTATIONS = 0
LSA_MUTATIONS = 0
TOKEN_MUTATIONS = 0
ACL_MUTATIONS = 0
DEVICE_MUTATIONS = 0
DRIVER_MUTATIONS = 0
PLATFORM_SECURITY_MUTATIONS = 0
RUNTIME_CODE_CHANGED = NO
RUST_RUNTIME_CHANGED = NO
HISTORICAL_RAW_EVIDENCE_CHANGED = NO
QUALIFICATION_TEST = BLOCKED
QUALIFICATION_TEST_BLOCKER = PRE_EXISTING_PINNED_RELEASE_ARTIFACT_SHA256_MISMATCH
~~~

The four handoff documents receive only an additive current marker pointing to
this source of truth. Their historical snapshots remain intact.
