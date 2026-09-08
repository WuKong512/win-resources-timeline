# AMD-PRIVILEGE-I2G 离线资格 Harness

## 当前状态

本文档是 I2G 实现后的唯一当前交接说明。此前文档中的设计快照和
`I2G_HARNESS = NOT_IMPLEMENTED` 均为历史记录，不能当作授权。

```text
I2G_HARNESS = IMPLEMENTED_OFFLINE
I2G_HARNESS_IMPLEMENTED = true
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_EXPERIMENT_SHAPE = PAIRED_CONTROL_TREATMENT
HISTORICAL_I2F_ROLE = PREDECESSOR_EVIDENCE_ONLY
HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = false
I2G_GATE_CONSUMED = false
I2G_REAL_EXECUTION_ALLOWED = false
I2G_REAL_CLEANUP_ALLOWED = false
I2G_HUMAN_REAL_RUN_AUTHORIZATION = NOT_GRANTED
I2G_REAL_RUNTIME = 0
I2G_OFFLINE_VALIDATION = PASS
I2G_HARNESS_ARTIFACT_ARCHITECTURE = x64
I2G_HARNESS_ARTIFACT_SHA256 = D9325E47F9F68C810A1CFC29F27F80E17D8A10828390D7B8D93F1E0FEF080A90
NEXT_GATE = I2G_HARNESS_REVIEW_BEFORE_HUMAN_REAL_RUN_AUTHORIZATION
```

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
RollbackRunning -> RollbackComplete`；不可恢复的失败进入 `Failed`。实际 discovery
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

已持久化的中间状态必须通过 machine observation 恢复，而不是仅凭 evidence 文件判断：
服务运行时继续 control teardown，treatment policy 已就绪时继续 treatment，treatment
完成或 rollback 中断时继续 rollback。恢复不重新 discovery，不重试，不增加 run cap。

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

真实开关在参数进入任何主机查询或写入前返回以下稳定标记并退出非零：

```text
I2G_REAL_EXECUTION_NOT_AUTHORIZED
I2G_REAL_CLEANUP_NOT_AUTHORIZED
I2G_REAL_GATE_CONSUMED=false
I2G_HUMAN_AUTHORIZATION_RECORDED=false
```

因此本次实现没有、也没有隐式取得 human real-run authorization。只有独立的人审阅、
明确授权和后续单独变更，才可以讨论 future Windows backend；本交付不会触发该步骤。
