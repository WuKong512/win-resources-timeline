# AMD CLI service-context qualification

This is a qualification-only Windows Service harness. It is not a production
broker, installer, IPC endpoint, or Resource Timeline provider.

The service accepts exactly one controlled `--run-root` child directory under
the machine-owned ProgramData qualification root. It derives the AMD uProf
installation root from the observed registry value, validates the current
qualified CLI identity, and launches one fixed ten-second package-power
`timechart` session directly. The service writes context, raw process, output,
and SCM status evidence; the existing
`tools/amd-uprof-cli-spike/postprocess.ps1` remains the single package-power
post-processor.

The PowerShell wrapper is preparation for one explicit Administrator run. It
does not self-elevate and must not be run as part of normal application
startup. It creates a manual, one-shot LocalSystem service and removes only
that exact registration after the qualification snapshot has been persisted.

No service registration, AMD executable, profiling command, or sampling
session is performed by the repository tests.
