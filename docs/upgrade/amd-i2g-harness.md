# AMD-PRIVILEGE-I2G 离线资格 Harness

## 当前状态

本文档是 I2G 实现后的当前交接说明。默认 I2G execution surface 仍是离线、合成、
fail-closed；本任务另有一个仅限本次明确授权、固定 token、固定服务和固定 CLI 的
一次性 real qualification runner。它不是生产 runtime，也不能被当作未来授权。

```text
I2G_HARNESS = IMPLEMENTED_OFFLINE
I2G_HARNESS_IMPLEMENTED = true
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_EXPERIMENT_SHAPE = PAIRED_CONTROL_TREATMENT
HISTORICAL_I2F_ROLE = PREDECESSOR_EVIDENCE_ONLY
HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = false
I2G_GATE_CONSUMED = true
I2G_REAL_EXECUTION_ALLOWED = false
I2G_REAL_CLEANUP_ALLOWED = false
I2G_HUMAN_REAL_RUN_AUTHORIZATION = CONSUMED
I2G_REAL_RUNTIME = ATTEMPT1_ONLY_CONSUMED
I2G_OFFLINE_VALIDATION = PASS
I2G_REAL_GATE_CONSUMED = true
I2G_HARNESS_ARTIFACT_ARCHITECTURE = x64
I2G_HARNESS_ARTIFACT_PATH = tools/amd-privilege-qualification/target/release/amd-privilege-qualification.exe
I2G_HARNESS_ARTIFACT_SHA256 = 2613129D179EA2A0496AD680E68E77A79FFFBB569D0802A11AC03346E162DD80
I2G_EXECUTION_SURFACE = SYNTHETIC_OFFLINE_FAIL_CLOSED
I2G_SHARED_EXECUTABLE_OFFLINE_ONLY = false
I2G_TASK_LOCAL_REAL_RUNNER = ONE_SHOT_EXACT_AUTHORIZATION_ONLY
I2G_TASK_LOCAL_REAL_RUN_STATUS = ATTEMPT1_VALID_CONTROL_HARNESS_RUNTIME_FAILURE
NEXT_GATE = HUMAN_REVIEW_BEFORE_ANY_NEW_REAL_RUN_AUTHORIZATION
```

## AMD-I2G REAL ATTEMPT #1 — IMMUTABLE HISTORICAL EVIDENCE

Attempt `9ae1e7898f6b4a438f1acc41b76c2715` is retained exactly as generated and
is not a valid paired scientific result. CONTROL was valid and returned
`POWER_UNAVAILABLE` after one discovery run. TREATMENT was scientifically
allowed, but the harness failed before TREATMENT discovery because Windows
PowerShell attempted to clone an `OrderedDictionary`. Rollback passed and the
final machine state was clean. The correct classification is
`HARNESS_RUNTIME_ERROR` with `SCIENTIFIC_RESULT=NOT_OBTAINED`, not a scientific
TREATMENT rejection. The repair adds explicit CONTROL/TREATMENT progress state,
an explicit configuration copy, distinct placeholder reasons, deterministic
wrapper exit codes, and child-process isolation for the reviewed real runner.

```text
ATTEMPT1_RUN_ID = 9ae1e7898f6b4a438f1acc41b76c2715
ATTEMPT1_CONTROL_RUNS = 1
ATTEMPT1_TREATMENT_RUNS = 0
ATTEMPT1_TOTAL_DISCOVERY_RUNS = 1
ATTEMPT1_CONTROL_RESULT = POWER_UNAVAILABLE
ATTEMPT1_TREATMENT_DISCOVERY = NOT_RUN
ATTEMPT1_TREATMENT_ALLOWED_BY_SCIENTIFIC_GATE = true
ATTEMPT1_HARNESS_RUNTIME_FAILURE = true
ATTEMPT1_FAILURE_CLASS = HARNESS_RUNTIME_ERROR
ATTEMPT1_SCIENTIFIC_RESULT = NOT_OBTAINED
ATTEMPT1_CAUSAL_INTERPRETATION_VALID = false
ATTEMPT1_ROLLBACK = PASS
ATTEMPT1_FINAL_MACHINE_STATE = CLEAN
ATTEMPT1_AUTHORIZATION = CONSUMED
ATTEMPT1_RAW_EVIDENCE = IMMUTABLE
NEW_REAL_RUN_AUTHORIZATION_REQUIRED = true
```

The reviewed real wrapper uses a separate Windows PowerShell 5.1 child for the
real runner. Its deterministic exit contract is `0` for a complete paired
qualification with a scientific result, `1` for blocked/harness/runtime or
cleanup failure, and `2` for an invalid or non-causal scientific result. This
keeps the parent process alive to clear authorization state and restore the
original contract bytes.

本次交付只影响 `tools/amd-privilege-qualification` 资格工具和升级文档；不改变
Resource Timeline 生产采集器、Provider、数据库、UI、安装器、开机启动或生产账户。
没有执行 AMD CLI、服务控制管理器、LSA、ACL、注册表、令牌调整、驱动、设备或采样。

## 固定的 paired contract

I2G 只验证一个预注册能力变量：`SeProfileSingleProcessPrivilege`。历史 I2F 的
`SeSystemProfilePrivilege` 结果是 predecessor evidence，不是 I2G 的活动因果 control。
两阶段使用同一个新建服务身份、同一个 Service SID、同一个 LocalService 账户、同一个
harness 和 AMD CLI 身份；阶段间不重建二进制，不改变非 treatment 配置。

CONTROL 只给该 Service SID 分配 `SeSystemProfilePrivilege`。服务 token 在实例化时
materialized disabled，之后只通过 `AdjustTokenPrivileges` 启用 SystemProfile；必须通过
pre/post token、`ERROR_SUCCESS`、身份、组和完整 privilege 分类门槛。CONTROL 只执行一次
固定的 `timechart --list`，且预期结果是 `POWER_UNAVAILABLE`。CONTROL 漂移、身份失败、
token 失败、超时、子进程归属失败或非零发现结果都会在 TREATMENT 前停止。

CONTROL 的 token、服务、AMD 子进程和所有 run-owned 状态完成 teardown 后，才允许唯一的
TREATMENT policy delta：向同一个 Service SID 增加 `SeProfileSingleProcessPrivilege`。
TREATMENT 重启同一个服务，确认两个权利在 materialized token 中均 disabled，再按固定顺序
独立启用 SystemProfile 和 ProfileSingle，每一次都要求 `AdjustTokenPrivileges` 返回成功、
`GetLastError = ERROR_SUCCESS`、post-state 正确。只有最终 token 和配置比较均通过，才执行
一次同样的 `timechart --list`。

```text
FRESH_SERVICE_ACCOUNT = NT AUTHORITY\LocalService
FRESH_SERVICE_ACCOUNT_SID = S-1-5-19
SERVICE_SID_TYPE = UNRESTRICTED
FIXED_OPERATION = COUNTER_DISCOVERY
FIXED_CLI_ARGUMENTS = timechart --list
AMD_CLI_PATH = D:\apps\AMDuProf\bin\AMDuProfCLI.exe
AMD_CLI_SHA256 = D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC
AMD_CLI_VERSION = 5.3.521.0
AMD_CLI_ARCHITECTURE = x64
COUNTER_DISCOVERY_TIMEOUT_MS = 30000
CHILD_SAFETY_CAP_MS = 90000
CONTROL_POLICY_RIGHTS = SeSystemProfilePrivilege only
TREATMENT_POLICY_DELTA = SeProfileSingleProcessPrivilege assignment to same Service SID only
POWER_SAMPLING_RUNS = 0
PLANNED_CONTROL_RUNS = 1
PLANNED_TREATMENT_RUNS = 1
MAX_TOTAL_RUNS = 2
CONTROL_RETRY_ALLOWED = false
TREATMENT_RETRY_ALLOWED = false
```

结果的科学维度和清理维度分开保存。有效 CONTROL + TREATMENT 的结果可以是
`PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT` 或
`PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT`；清理失败时保留原始
`experiment_result`，但 `normalized_result = CLEANUP_FAILED` 且
`causal_interpretation_valid = false`。结果不宣称 necessity、最低权限、生产适用性或
单独归因于 enablement。

## 状态、证据和所有权

状态持久化是单调的：`Prepared -> ControlPolicyReady -> ControlServiceRunning ->
ControlTokenReady -> ControlDiscoveryRunning -> ControlComplete ->
ControlTeardownComplete -> TreatmentPolicyReady -> TreatmentServiceRunning ->
TreatmentTokenReady -> TreatmentDiscoveryRunning -> TreatmentComplete ->
RollbackRunning -> RollbackComplete`；清理失败进入 `Failed`，该状态只能由 recovery
重新进入 rollback；只有 `Invalid` 按契约不可恢复。实际 discovery
计数只有在固定子进程成功 spawn 后增加，spawn failure 不计数；非零退出仍然计一次。

每个 evidence JSON 使用临时文件 `create_new`、写入、flush、同步、关闭、目标不存在检查、
rename 的原子写入顺序。已存在的目标不会覆盖。固定证据集合包括：

```text
EXPERIMENT-MANIFEST.json       PRE-MACHINE-STATE.json
CONTROL-POLICY.json            CONTROL-TOKEN-PRE.json
CONTROL-TOKEN-POST.json        CONTROL-AMD-IDENTITY.json
CONTROL-DISCOVERY.json         CONTROL-TEARDOWN.json
TREATMENT-POLICY.json          TREATMENT-TOKEN-PRE.json
TREATMENT-TOKEN-SYSTEMPROFILE.json
TREATMENT-TOKEN-FINAL.json     TREATMENT-AMD-IDENTITY.json
TREATMENT-DISCOVERY.json       PAIRED-CONFIG-COMPARISON.json
PAIRED-TOKEN-COMPARISON.json   PAIRED-RESULT.json
ROLLBACK.json                  FINAL-SUMMARY.json
```

`SeSystemProfilePrivilege` 和 `SeProfileSingleProcessPrivilege` 的 run-owned 状态分别
记录：是否由本次 run 添加、是否尝试移除、移除是否验证成功以及 already-absent 状态。
回滚先停止接受工作、结束或杀死精确归属的子进程、证明子进程和 token 消失、停止服务并
确认 PID=0，再分别做双读回的精确权利移除，最后删除同一个服务并确认服务和 owned process
均不存在。禁止 `all=true`、全局 LocalService 修改和 Administrators 修改。

已持久化的中间状态必须通过 machine observation 恢复，而不是仅凭 evidence 文件判断。
`Prepared`、全部 CONTROL/TREATMENT 中间态、`ControlTeardownComplete`、
`RollbackRunning`、`Failed` 和 `RollbackComplete` 均有明确 reconciliation；只有契约上
不可恢复的 `Invalid` 才返回 `RecoveryDecision::Invalid`。不一致的主机状态一律
fail closed 到 rollback/cleanup。`ControlTeardownComplete` 可恢复到 policy path，
`TreatmentPolicyReady` 可恢复到 service/token-only path，但任何 recovery 都不会重新
launch discovery，也不会 retry。

每个 service/right mutation 都先 durable 写入 write-ahead intent，再用 fresh machine
readback 推导 ownership。`preexisting=true` 永远不可被推导为 run-owned；当 pre-state
absent、intent 已 durable、machine readback 显示 mutation 成功时，即使 added-by-run
字段尚未来得及落盘，recovery 仍会安全识别并回滚。每个 discovery phase 也先 durable
写入 spawn intent；child identity、completion journal 或仍存活的 exact child 足以消耗
该 phase 的唯一 run budget，crash 后不会把 count 恢复为零并重新 launch。

`execute_synthetic_recovery()` 会 load 最新 `STATE-*.json`、重新取得 synthetic
machine observation、调用 reconciliation，然后只执行 teardown/rollback 或
treatment-service-only resume；该 executable seam 不调用 discovery。20 个固定
crash-window（CONTROL 8、TREATMENT 7、ROLLBACK 5）均验证 cleanup、ownership、child
absence、no-retry、run caps 和 `causal_interpretation_valid=false`。

## 离线验证入口

纯契约库是 `tools/amd-privilege-qualification/i2g-runtime-contract.ps1`。普通入口
`run-admin-amd-i2g-qualification.ps1` 只输出 plan；`-LibraryOnly` 只加载契约；
`-OfflineSynthetic -OfflineSyntheticScenario <固定场景>` 执行确定性 synthetic backend。
cleanup 入口具有相同的 plan/library/offline 分层。

synthetic adapter 不启动 AMD、服务或任意外部命令，覆盖 happy、negative、CONTROL drift、
pre-control/token/materialization failure、配置 delta、token delta、SystemProfile regression、
ownership、timeout、identity、spawn、nonzero exit、pre-existing right、cleanup failure 和
recovery/no-retry 场景。Rust backend trait 保留未来 Windows seam，但当前
`RealWindowsI2gBackend` 和 PowerShell real 分支均 fail closed。

正式 offline wrapper 只接受精确的 release artifact path，并在 synthetic execution
前校验 PE architecture=`x64` 和 authoritative SHA-256；正确 artifact、tampered
artifact、missing artifact 均有回归测试。注意共享的
`amd-privilege-qualification.exe` 仍包含历史 `--broker`、`--system-counter-service`、
`--service-profile-counter-service`、`--service-profile-enable-counter-service` 和
`--client` 入口；因此整个 EXE 不是 offline-only。本 milestone 只开放
`I2G_EXECUTION_SURFACE = SYNTHETIC_OFFLINE_FAIL_CLOSED`；另外的 real runner 只接受
本任务 exact authorization token，并固定到 qualification-only Service SID paired
experiment，不开放通用 executable、argv、sampling 或生产调用面。

真实 I2G 开关在参数进入任何主机查询或写入前返回以下稳定标记并退出非零：

```text
I2G_REAL_EXECUTION_NOT_AUTHORIZED
I2G_REAL_CLEANUP_NOT_AUTHORIZED
I2G_REAL_GATE_CONSUMED=true
I2G_HUMAN_REAL_RUN_AUTHORIZATION=CONSUMED
```

未提供 exact task token 的 real path 仍在任何主机访问前 fail closed。历史 entrypoint
仍不因本 runner 获得授权；本 runner 的 authorization 只由本任务 prompt 覆盖，并在
完成、失败或中止后 consumed。
