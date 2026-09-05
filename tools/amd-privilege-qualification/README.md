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
    -> broker-derived AMD installation path and fixed `power` request
    -> broker-owned ProgramData output
    -> validated package-power parser
    -> typed response over the pipe
```

The request schema contains only:

```text
GetAmdProviderStatus
StartAmdPowerSession { duration_ms, interval_ms }
GetAmdSessionStatus { session_id }
CancelAmdSession { session_id }
```

The broker rejects unknown fields, unknown request types, incompatible
protocol versions, oversized messages, invalid bounds, stale session IDs, and
non-owner cancellation. It never accepts an executable path, raw argv, shell
command, working directory, environment, registry path, or output path.

## Synthetic qualification

From the repository root:

```powershell
pwsh -NoProfile -File .\tools\amd-privilege-qualification\test-qualification.ps1
```

This command does not register a service, request elevation, access the AMD
installation, or run AMD uProf.

## Future, manually authorized two-context run

The following commands are prepared but must not be run as part of automated
preparation. They are intentionally separate:

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

## Frozen future-run artifact

The manually authorized wrappers are pinned to this release artifact; changing
the binary requires rebuilding and recording a new hash before any future run:

```text
path = tools/amd-privilege-qualification/target/release/amd-privilege-qualification.exe
architecture = x64
build_mode = release / cargo --offline
sha256 = A656B0E95AA2BAEB0E09FE729AA502C23BF09C6F894766680D49026720B790CD
broker_authenticode = NotSigned; exact SHA-256 is required by both wrappers
```

Prepared commands, to be run only in the stated contexts:

```powershell
# Administrator x64 PowerShell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\tools\amd-privilege-qualification\run-admin-amd-privilege-qualification.ps1"

# normal non-elevated x64 PowerShell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\tools\amd-privilege-qualification\run-standard-user-amd-privilege-client.ps1"

# Administrator x64 PowerShell, after the bounded real run
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\tools\amd-privilege-qualification\cleanup-admin-amd-privilege-qualification.ps1"
```
