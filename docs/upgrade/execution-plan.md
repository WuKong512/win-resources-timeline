# 实施计划

## PR 原则

每个 PR 只承担一个可验证边界，包含相应迁移/单元/集成测试和性能证据。数据库变更、采集 Provider、崩溃证据回溯和大规模 UI 改造不得堆入同一个 PR。当前 UI 明显 bug 等用户给出复现后单独修复，可与本路线并行但不混入 GPU storage contract。

## 里程碑

| PR | 主题 | 主要交付 | 进入条件 |
| --- | --- | --- | --- |
| PR-00 | 架构基线 | 本目录文档 | 已完成仓库审计 |
| Spike-01 | 采集可行性 | 独立 metric probe、脱敏能力报告、接口语义/权限/开销证据 | PR-00 合并；可与 PR-01 并行 |
| PR-01 | v7 存储骨架 | 新 DDL、migration journal、备份/校验、frame writer、rollup 骨架 | PR-00 合并 |
| PR-02 | 使用时间 | 事件驱动前台跟踪、电脑状态、应用身份、日报 | PR-01 |
| PR-03 | Provider 框架 | capability、CollectionPlan、启停、health、设置 DTO | PR-01 |
| PR-04A | GPU storage contract | v7→v8 forward-only migration、GPU model/writer/query、per-device bounded query、quality round-trip/integrity fixtures | PR-03；不实现正式硬件 Provider |
| PR-04 | 常用硬件指标 | Windows baseline + 已准入厂商 GPU，CPU 传感器可选 Provider | PR-04A；对应指标 Spike-01 short-term implementation admission |
| PR-05 | 进程与崩溃 | Top-N selector、多级聚合、system events、case/hold/evidence summary | PR-01、PR-02/03 稳定接口 |
| PR-06 | 新信息架构 UI | 时间线、使用统计、崩溃回溯、采集/保留设置 | 后端 DTO 稳定 |
| PR-07 | 稳定与发布 | soak、迁移演练、性能/空间报告、发布说明 | 前述功能完成 |
| PR-08 | Dashboard chart interaction | stable ECharts lifecycle、tooltip/crosshair、hover/click/gap semantics | PR-07 merged |
| PR-09 | Dashboard information architecture + Metric Explorer | adaptive overview、unit-compatible trend workspace、capability-aware metric catalog | PR-08 merged；post-release UI enhancement |

## 每阶段验收门槛

### 数据安全

- 自动化迁移 fixture、外键/完整性检查和中断恢复通过。
- 禁止测试或生产路径删除用户数据库。
- 写入失败有重试与可见健康状态。

### 性能

- 硬件指标在正式接入前完成 [Spike-01](./collection-feasibility-spike.md)，报告来源、权限、更新粒度和增量开销。
- 报告测试硬件、采集配置、持续时间、平均/P95 CPU、内存和数据库增长。
- Provider 开关验证真正停止工作。
- PR-07 的 mandatory release/stability gate 是 multi-session extended native qualification：至少 3 个独立 native session、每个 session >=10 小时、至少 1 个 session >=12 小时、aggregate valid native runtime >=32 小时；整体覆盖 long idle/background、normal interactive、local-midnight rollover、Provider/category disable-enable、DB busy/recovery、clean process shutdown/reopen、schema/integrity continuity、bounded resource behavior 和 validated-profile performance/storage budget。各 session 必须分别检查无无界队列、句柄、线程或内存增长。
- continuous 24-hour soak 是 optional extended qualification，不是 PR-07 mandatory blocker。该模型与 Resource Timeline 的实际 duty cycle 对齐：Windows 启动、通常运行十多个小时、sleep/shutdown、下一 session 再启动并多日重复；它同时覆盖单 session 慢增长与 repeated startup/shutdown、DB reopen、WAL checkpoint/recovery、Provider lifecycle recreation 和 stale native object/mutex 风险。单次 >=10 小时且至少一次 >=12 小时仍用于检测 memory leak、handle/thread growth、queue accumulation、WAL runaway 和 retry loop。
- Real Windows sleep/wake 是当前 validated AMD/NVIDIA desktop profile 的 `DEFERRED POWER-STATE / COMPATIBILITY QUALIFICATION`：不是 mandatory PR-07B blocker，也不是 correctness PASS。任何观察到的异常必须保留为 evidence，不得归因于 Resource Timeline、Windows 或硬件；当前异常 attribution 为 `UNKNOWN`，后续 compatibility declaration gate 独立处理。

PR-07B 的 release/stability 结论只适用于实际验证的 hardware profile，不等同于 full cross-hardware support declaration。当前 profile 为 Windows desktop、AMD CPU、NVIDIA GPU、no battery。Intel CPU、AMD/Intel GPU 和 battery-capable device 的 hardware support/compatibility declaration gate 独立保留；未测设备不得宣称已覆盖，未完成时记录为 `Deferred hardware support declaration / compatibility qualification`。

### 产品语义

- disabled、unsupported、failed、zero 在 API/UI 中可区分。
- 使用时间跨 idle/lock/sleep/午夜计算正确。
- 崩溃功能只展示可追溯证据和客观统计，不生成原因、严重度或建议。
- 进程 rollup 的 additive totals、max、加权 avg 和 coverage 可从下层数据复算一致。

### UI

- 关键页面覆盖 loading/empty/error/unsupported/disabled。
- 键盘导航、缩放、浅色/深色对比度和常见窗口尺寸验证。
- 大时间范围图表使用下采样，不阻塞主线程。

## 首轮实现顺序

1. 先补迁移 fixture、性能采样脚本和健康指标，同时启动 Spike-01 独立探针。
2. 实现 v7 空表与旧数据迁移；随后由 PR-04A 以 v7→v8 方式补齐通用 GPU storage contract。此工作不等待硬件探针，也不扩大默认指标范围。
3. 将现有采样改为 frame writer，消除 15 秒批量丢失窗口和失败即清队列问题。
4. 分别落地事件驱动使用时间和 Provider/CollectionPlan；capability 语义复用探针结果。
5. 在代表硬件上逐个接入完成短期 Spike-01 implementation admission 的 GPU/CPU 传感器；default-enable、support matrix 和 release stability 另行验收。
6. 建立 crash case/hold 后再实现证据摘要，防止整理前数据已被清理；不实现诊断规则。
7. 后端 DTO 稳定后统一升级 UI。

## 发布与回退

v8 首次发布应分阶段启用，可通过设置/feature flag 关闭新 Provider 和 crash evidence builder，但不能回写破坏 v7/v8 数据。发布包保留诊断导出和备份恢复说明。若性能或稳定性未达标，回退采集计划和 UI 功能，不通过删除用户历史数据回退。

## PR-03 / PR-04A 当前落地状态

PR-03 已在 schema v7 runtime settings storage 上落地 Provider framework；PR-04A 已把 GPU storage contract 前置到 schema v8：

- collector 通过 `windows-baseline` adapter 运行既有 CPU、内存、磁盘、进程 production sampler。
- settings DTO 支持 `enabledCategories` 与 `disabledProviders`；collector status 暴露 provider capability/lifecycle/health。
- CollectionPlan 在 startup、settings reload 和 capability 变化时编译；ProviderHost 分开保存用户 desired plan 与 capability-filtered effective plan，apply delta 只影响变化的 provider；pause、resume、shutdown 复用既有 collector 生命周期。
- provider probe 在 startup 生成实际 capability；executor 为 probe/start/reconfigure/sample/stop 提供 bounded deadline/cancellation 边界，pending completion 按 operation/generation reconcile，startup/reconfigure failure 自动指数退避，stop failure/timeout 进入 health 且不阻塞 shutdown；普通 control operation 使用 per-provider budget，shutdown 保留单一绝对 deadline。
- fake provider tests 覆盖 capability state、plan determinism、disable/re-enable、unsupported、startup/reconfigure retry、late lifecycle reconciliation、stale generation、sample timeout isolation、per-provider control deadline、Disk probe/start-time degradation、pause、shutdown 和 DTO 区分。
- PR-04A 没有 NVML production integration、`nvidia-smi`、AMD/Intel Provider、CPU sensor、UI redesign 或 PR-05 内容；`tools/metric-probe` 保持独立且未修改。

Spike-01B 已在当前 RTX 5070 Ti 开发机完成 short-term implementation admission（PASS）：有效 administrator comparison、30-minute idle/representative load、enable/disable/re-enable、cleanup、failure isolation 和真实 sleep/wake evidence 均已完成，因此 PR-04 NVIDIA Provider entry gate 已满足。该结论仅限当前开发机，不改变 GPU 默认关闭策略，也不声明 NVIDIA 产品线支持、production-ready 或完整 support matrix。PR-04A 的 provider/session/device metadata 只证明 storage contract 具备 historical traceability capability；PR-04 负责让 production Provider 维护 runtime truth。PR-07 的后续 release/stability gate 采用本文件定义的 multi-session extended native qualification；continuous 24-hour soak 为 optional extended qualification，数据库增长、跨硬件验证和完整 release matrix 仍分别按相应 gate 执行。

## PR-05 当前落地状态

PR-05 已在 schema v8 上实现进程与崩溃证据后端边界：

- `ProcessSelector` 在 raw process instance 层按 CPU、working-set memory、I/O 各取固定 Top-N=5，加入可解析的 foreground process，并以稳定 PID+creation-time+executable identity 去重和合并 selection-reason bitmask；watched/anomaly 位保留但本 PR 不引入对应设置或检测器。
- FrameWriter 保存可空的 process metrics、PID/创建时间、quality/selection facts；1 分钟 rollup 聚合到 logical app，1 小时由分钟层生成，日报由已选观测分钟层生成。加权 avg、实际 frame duration 的 additive totals、max、OR reason 和 coverage 均落在已有 v8 表中；日报不是全进程全天账户，coverage 小于 1 必须保留为不完整，处理版本为 `process-rollup-v1`。
- Windows crash detector 在启动后异步读取 native Windows Event Log API，使用 channel + record id/time cursor 持续排空 256 条分页；Event 41/6008/1001 分开保留 log time 与物理 anchor，跨批次按稳定 incident identity 合并并按证据强度 refinement。`crash-evidence-v1` objective summaries 具备 `pending`/`post_pending`/`partial`/`complete`/`failed` 生命周期，atomic retention hold 在 active 时阻止 clear。四个 summary window 使用 `window:<window>:metric:<metric>` 命名空间以复用 v8 的 summary uniqueness。
- 已覆盖 selector、rollup、事件分类/游标、native 语义和 EvtNext 错误映射、跨批次 correlation、>256 backlog、hold 保护/释放、证据统计、延迟 post retry、分设备 GPU、幂等 rebuild、隐私摘要和 schema v8 回归测试。schema version 未变更，也未新增 migration。
- PR-06 UI、用户-facing retention settings、release soak、跨硬件真实崩溃矩阵、dump 解析、root-cause/diagnostic reasoning 均为 deferred/non-goal。

## 架构基线完成定义

- 设计主题文档与索引齐全，术语和表名一致。
- 明确产品非目标、隐私默认值、性能预算和数据安全约束。
- 后续 PR 顺序、依赖和验收条件可直接转为 issue/checklist。
- PR-03 的 Provider runtime/DTO 接口已落地；PR-04A 的 schema v8 GPU contract 和现有 UI 信息架构保持独立，硬件 Provider 与新信息架构 UI 仍由后续 PR 负责。

## PR-09 交付边界

PR-09 是 PR-08 合并后的 Dashboard UI 增强：复用 schema v8、既有 `ui.dashboard.v1` 配置和 Provider/session/device capability truth，不新增 Provider、不迁移数据库，也不改写 PR-00..PR-08 历史状态。

## CPU sensor spike 当前状态

`CPU-SENSOR-SPIKE` 已在 `spike/cpu-sensor-feasibility` 完成独立 Windows probe、AMD 当前机器 idle/load、cadence、开销和启停证据，结论为 `PASS_WITH_DEFERRED_METRICS`：CPU package temperature、package power、effective frequency 均暂不进入 production Provider。Windows baseline 只保留现有 CPU usage/OS 状态语义；后续若有来源通过许可、驱动安全、精确 scope、长时开销和 lifecycle gate，应作为 optional CPU sensor Provider 复用既有 ProviderHost/CollectionPlan/MetricCatalog。详见 [`docs/measurements/cpu-sensor-feasibility.md`](../measurements/cpu-sensor-feasibility.md)。本 spike 未修改 Dashboard、PR-09、production collector 或 schema。

## CPU-SENSOR-SOURCE-Q1 当前状态

`CPU-SENSOR-SOURCE-Q1` 已完成 AMD uProf 5.3 / `AMDPowerProfileAPI` 的静态 source qualification。AMD 当前公开 Windows build 为 `5.3.521`；Ryzen 9000 Live Power Profiling 和本机 `Family 1Ah / Model 44h` 的官方 family/model 范围证据成立，package temperature、estimated average package power、以及 per-core effective frequency 的 source semantics 已记录。但本机未安装 uProf；Microsoft Hypervisor 已启用，VBS/HVCI 作为平台 context 记录；因此 live evidence 为 `BLOCKED_LIVE_PROVIDER_NOT_INSTALLED`，temperature 另记为 `DEFER_CURRENT_PLATFORM_CONFIGURATION`，effective-frequency aggregate 保持 `DEFER_AGGREGATION_CONTRACT`。部署结论为 `DEFER`，distribution 结论为 `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`；Q1 未新增 probe command、production Provider、schema、MetricCatalog 或 UI。后续若取得合法的 external-installed uProf、官方 header/API PDF/sample，应执行 `CPU-SENSOR-AMD-LIVE-QUALIFICATION`，并继续复用既有 ProviderHost/CollectionPlan/MetricCatalog seam。详见 [`docs/measurements/cpu-sensor-amd-uprof-qualification.md`](../measurements/cpu-sensor-amd-uprof-qualification.md)。

## CPU-SENSOR-AMD-LIVE-QUALIFICATION 当前状态（HISTORICAL / SUPERSEDED）

> HISTORICAL / SUPERSEDED: this early load-abort record is retained as evidence.
> Its `BLOCKED` state and loader-trace next step are not the current AMD gate.
> The current consolidated state is defined by `CPU-SENSOR-AMD-ROOT-CAUSE-FINAL-CLOSURE`
> and `CPU-SENSOR-AMD-UPROF-LIVE-QUALIFICATION-SPIKE CLOSURE` below.

`CPU-SENSOR-AMD-LIVE-QUALIFICATION` 的历史 run 仍为 `BLOCKED`，并保留原始零样本记录：已安装 AMD uProf `5.3.521` 的 API DLL 路径、签名、x64 架构和官方 header/PDF/sample 均通过只读审计；非管理员隔离 load-only 子进程在显式 `LoadLibraryExW` 阶段以 signed `-1` / `0xFFFFFFFF` 终止。follow-up 已用已安装 CDB 观察到 `KERNEL32!FatalExit(0xFFFFFFFF)`，并在直接加载 `CXLBaseTools.dll` 时复现同一边界，因此当前主分类为 `DEPENDENCY_LOAD_FAILURE`，subcause 为 `CXLBASETOOLS_LOAD_PATH_FATAL_EXIT`；这不是普通 loader 返回错误，也尚未证明其 vendor 内部触发条件。后续受控 Administrator comparison 已证明 x64 High/Administrator token，但直接 CXL、直接 API DLL、以及 Resource Timeline init-only child 仍以 `-1` / `0xFFFFFFFF` 在 load boundary 终止；官方 `CollectAllCounters` evidence 缺失，保持 `INCONCLUSIVE`。与此同时，官方 CLI `timechart --list` 在管理员上下文报告 Socket/Core/Thread 与 Power/Frequency/P-State 能力，短时 `timechart --event power --interval 1000 --duration 5` 正常生成 3 条 package/core power records。后续只读 divergence investigation 进一步确认：CLI 对 `AMDPowerProfileAPI.dll` 与 `CXLBaseTools.dll` 使用非 delay 的 public-module import path，且既有 CLI 调试证据曾到达 `AMDTPwrProfileInitialize(0)`；因此 `CLI_RUNTIME_USES_PUBLIC_POWER_API_PATH = YES`，但 direct API 与 CLI 的行为仍为 `DIVERGENT`，具体 CXL context/initialization 触发条件仍 `UNPROVEN`。`DEPENDENCY_LOAD_ABORT = PERSISTS`，`PERMISSION_BOUNDARY = PARTIALLY_RESOLVED`（仅 vendor CLI path）。没有 Resource Timeline API 的 package power、per-identity frequency、temperature、cadence、lifecycle 或 busy qualification；package power 与 AMD per-identity frequency 保持 `DEFER`，temperature 保持 `DEFER_UNREACHED_DUE_TO_LIBRARY_LOAD`，CPU effective-frequency aggregate 保持 `DEFER_AGGREGATION_CONTRACT`，distribution 保持 `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`。不得据此开始 `CPU-SENSOR-AMD-PROVIDER-DESIGN`；下一步是窄范围的 `AUTHORIZED_CLI_VS_DIRECT_API_LOADER_CONTEXT_TRACE`（如需精确 loader order），且不得修改 Hypervisor/VBS/HVCI、driver/service、安装、Afterburner/RTSS 或任何 production seam。详见 [`docs/measurements/cpu-sensor-amd-uprof-load-abort-followup.md`](../measurements/cpu-sensor-amd-uprof-load-abort-followup.md) 与 [`docs/measurements/cpu-sensor-amd-uprof-cli-vs-direct-api-divergence.md`](../measurements/cpu-sensor-amd-uprof-cli-vs-direct-api-divergence.md)。

## CPU-SENSOR-AMD-STATIC-IMPORT-SURFACE-AUDIT 当前状态（HISTORICAL / SUPERSEDED）

> HISTORICAL / SUPERSEDED: the import-surface audit remains valid evidence, but
> its earlier “next control” framing is no longer the current execution gate.

`CPU-SENSOR-AMD-STATIC-IMPORT-SURFACE-AUDIT` 已完成只读安装树、import library、官方 sample 构建文件和 AMD `bin` PE import audit。已确认 `AMDPowerProfileAPI.lib` 是 x64 public API import library，但未发现 `CXLBaseTools.lib`、CXL header 或官方 CXL link surface；因此在当前安装 artifact 范围内 `CXL_LINK_SURFACE = PRIVATE_INTERNAL`。已发现 AMD 签名的 `D:\apps\AMDuProf\bin\AMDuProf.exe` 直接 import `CXLBaseTools.dll`，可作为未来 `EXISTING_VENDOR_BINARY_VS_DYNAMIC_PROBE` 候选；此前 `CollectAllCounters.exe` 的实际二进制未找到，sample build fidelity 仍为 `INCONCLUSIVE`。后续 A1 运行因 static fixture 以 `0xFFFFFFFF` 失败而未形成有效 A/B，B1 保持未执行；该静态 surface 结论仍为 `STATIC_IMPORT_SURFACE_DECISION = EXISTING_VENDOR_CONTROL`。详见 [`docs/measurements/cpu-sensor-amd-static-import-surface-audit.md`](../measurements/cpu-sensor-amd-static-import-surface-audit.md)。

## CPU-SENSOR-AMD-VENDOR-EXECUTABLE-CONTEXT-DIFFERENTIAL-AUDIT 当前状态（HISTORICAL / SUPERSEDED）

> HISTORICAL / SUPERSEDED: this static differential is retained for provenance;
> its candidate hypotheses were later narrowed by the directory counterfactual.

`CPU-SENSOR-AMD-VENDOR-EXECUTABLE-CONTEXT-DIFFERENTIAL-AUDIT` 已完成只读静态审计，审计本身未启动 `AMDuProfCLI.exe`、`AMDuProf.exe`、`AMDProfilerService.exe`、sample、`metric-probe` 或 CDB。已确认 V1/V2/V3 vendor executable 均直接 import `CXLBaseTools.dll` 并拥有多个 AMD CXL-importing parent，而失败的 M1 仅通过 `AMDPowerProfileAPI.dll -> AMDSysUtils.dll -> CXLBaseTools.dll` 到达 CXL；`AMDPowerProfileAPI.lib` 是唯一发现的 x64 public import library，未发现 CXL public header/import library。静态最强假设为 `VENDOR_IMPORT_TOPOLOGY_HYPOTHESIS = STRONG`，身份/路径发现为独立的 `VENDOR_PROCESS_IDENTITY_HYPOTHESIS = PLAUSIBLE`；二者都不是 runtime causality proof。随后已完成一次 native non-debugger `AMDuProf.exe` no-op startup control 并确认其存活 3,000 ms；该 survival divergence 仍不等于因果证明。详见 [`docs/measurements/cpu-sensor-amd-vendor-executable-context-differential.md`](../measurements/cpu-sensor-amd-vendor-executable-context-differential.md)。

## CPU-SENSOR-AMD-PUBLIC-API-STATIC-VS-DYNAMIC-MINIMAL-A/B 当前状态（HISTORICAL / SUPERSEDED）

> HISTORICAL / SUPERSEDED: the failed minimal A/B and its untestable B1 gate
> are preserved; they do not reopen the completed root-cause or CLI spike work.

`CPU-SENSOR-AMD-PUBLIC-API-STATIC-VS-DYNAMIC-MINIMAL-A/B` 已完成两个最小 x64 Rust/MSVC fixture 的构建和 PE import-table 静态门禁：static fixture 使用已安装官方 `AMDPowerProfileAPI.lib`，直接 import `AMDPowerProfileAPI.dll` 且不直接 import `CXLBaseTools.dll`；dynamic fixture 不直接 import AMD API/CXL，仅在 main 中对绝对 API DLL 路径调用 `LoadLibraryExW`。fixture 及 AMD API DLL 的 SHA 已锁定。首次 A1 确实启动并观察到 signed `-1`，但旧 wrapper 在负数退出码转十六进制时抛错，导致 stdout/stderr/result JSON 未持久化；该历史尝试保持 `EXECUTED_BUT_UNCLASSIFIABLE_DUE_TO_HARNESS_PERSISTENCE_FAILURE`，B1 未执行。wrapper 已修复 signed exit serialization、证据持久化顺序和空参数数组处理，且非 AMD synthetic regression 已通过；A1-R1 随后完成了完整 capture，但仍以 `-1 / 0xFFFFFFFF` 结束且没有 main marker，因此 `STATIC_CONTROL_INVALID` 已确认，`STATIC_VS_DYNAMIC_LOAD_BEHAVIOR_DIVERGENCE` 对该 A/B `NOT_TESTABLE_WITH_THIS_A/B`，B1 保持未授权。后续 vendor no-op、hold fixture 与 CXL 静态审计均保持独立，不将该 A/B 标为完成。这不是 AMD source qualification，也不开始 `CPU-SENSOR-AMD-PROVIDER-DESIGN`。详见 [`docs/measurements/cpu-sensor-amd-public-api-static-dynamic-ab.md`](../measurements/cpu-sensor-amd-public-api-static-dynamic-ab.md)。

## CPU-SENSOR-AMD-EXISTING-VENDOR-EXECUTABLE-NO-OP-STARTUP-CONTROL 当前状态（HISTORICAL / SUPERSEDED）

> HISTORICAL / SUPERSEDED: retained as the vendor-survival evidence preceding
> the final executable-directory counterfactual.

`CPU-SENSOR-AMD-EXISTING-VENDOR-EXECUTABLE-NO-OP-STARTUP-CONTROL` 已完成 native no-op startup qualification。管理员 evidence 在 3,000 ms observation window 后记录 `AMDuProf.exe` root PID 仍存活，`VENDOR_STARTUP_CONTROL = PASS`，并观察到 `AMDProfilerService.exe` 的一层子进程和 vendor 应用 bootstrap 日志；cleanup 记录单独保留，未成功 graceful-close 且未强制终止。该结果确认 vendor executable 与失败 M1 的 native survival divergence，但不证明 import topology、bootstrap、process identity 或其它差异的因果性；`EXACT_VENDOR_EXECUTABLE_REQUIREMENT = UNPROVEN`。没有 profiling、sampling、B1 或 production seam 变更。详见 [`docs/measurements/cpu-sensor-amd-vendor-noop-startup-control.md`](../measurements/cpu-sensor-amd-vendor-noop-startup-control.md)。

## CPU-SENSOR-AMD-STATIC-FIXTURE-LIFETIME-SHUTDOWN-DISCRIMINATOR 当前状态（HISTORICAL / SUPERSEDED）

> HISTORICAL / SUPERSEDED: retained to document the pre-counterfactual startup
> stage; the directory runtime confirmation is the later authoritative result.

`CPU-SENSOR-AMD-STATIC-FIXTURE-LIFETIME-SHUTDOWN-DISCRIMINATOR` 已完成一次受控 Administrator hold-fixture run。新二进制保留原 M1 的官方 `AMDPowerProfileAPI.lib` static import 策略和 `AMDPowerProfileAPI.dll -> AMDSysUtils.dll -> CXLBaseTools.dll` 依赖链，SHA 为 `B680E7761FC3E64193E7140B57326154A64AB702C62763C7693EA97234DC1676`；原 M1 未被覆盖。目标只运行约 `63.2 ms` 即以 `-1 / 0xFFFFFFFF` 结束，两个 checked synchronous `WriteFile` marker 均未出现，stdout/stderr 已持久化且 capture complete，因此 `M1_FAILURE_FAMILY = STARTUP`、`STATIC_FAILURE_STAGE = BEFORE_DURABLE_MAIN_MARKER`、`SHUTDOWN_OR_DETACH_HYPOTHESIS = DOWNGRADED`；不宣称 Rust `main` 绝对未进入，因为 retained import pointer read 位于第一 marker 之前。没有 B1、profiling/sampling 或 production seam 变更。详见 [`docs/measurements/cpu-sensor-amd-static-fixture-lifetime-discriminator.md`](../measurements/cpu-sensor-amd-static-fixture-lifetime-discriminator.md)。

## CPU-SENSOR-AMD-CXL-FATALEXIT-STATIC-AUDIT 当前状态（HISTORICAL / SUPERSEDED）

> HISTORICAL / SUPERSEDED: the CXL static control-flow findings remain useful
> evidence, but no longer form a pending runtime gate for the spike.

`CPU-SENSOR-AMD-CXL-FATALEXIT-STATIC-AUDIT` 已完成只读静态控制流审计，未运行 AMD 二进制。精确 `CXLBaseTools.dll`（SHA `4815D4631BCA9C051DC4293538DF8D402BD848E705228F497DF718EDCA1F8931`）不直接 import `KERNEL32!FatalExit`，因此直接 FatalExit import/xref 数为 `0`；可定位的是同一运行函数中的两个 `api-ms-win-crt-runtime-l1-1-0.dll!quick_exit` 调用点（`RVA 0x1A82` 与 `0x1B64`），两者均可静态装载 `0xFFFFFFFF`。`RVA 0x12378` 已由 IAT/原始导入名交叉解析为 `api-ms-win-crt-string-l1-1-0.dll!_wcsicmp`；`QE-1` 与历史 FatalExit 返回地址和最近导出定位 `asWideString+0x458` 强相关，但 CRT 后续如何到达 Kernel32 仍不可见。QE-1 的可达路径以 `GetModuleHandleW(NULL)` / `GetModuleFileNameW` 解析进程可执行文件目录，并读取 `HKLM\\SOFTWARE\\WOW6432Node\\AMD\\AMDProfiler` 的 `InstallationPath`，再将其与 `InstallationPath\\bin` 和 `InstallationPath\\bin\\AMDPerf` 进行 `_wcsicmp` 比较；当前只读注册表值为 `D:\\apps\\AMDuProf\\`，与实际安装根匹配，故 `FATAL_CONDITION_FAMILY = MODULE_IDENTITY_FAILURE`，而非当前机器已证明的 registry mismatch。该可见谓词与 hold fixture 目录不匹配、与 vendor `bin` 目录匹配，但 runtime 分支仍未直接观测。entrypoint 的 process-attach dispatcher 到候选函数的静态边存在，TLS callback 数为 `0`。下一步不再执行 `PROCESS_BASENAME_ONLY_CONTROL`；可选的 directory-only confirmation 仍未运行且需要单独授权，不得开始 B1 或 `CPU-SENSOR-AMD-PROVIDER-DESIGN`。详见 [`docs/measurements/cpu-sensor-amd-cxl-fatalexit-static-audit.md`](../measurements/cpu-sensor-amd-cxl-fatalexit-static-audit.md)。

## CPU-SENSOR-AMD-CXL-EXECUTABLE-DIRECTORY-ROOT-CAUSE-CLOSURE 当前状态（HISTORICAL / SUPERSEDED）

> HISTORICAL / SUPERSEDED: the earlier `HIGH` confidence wording was refined
> by the later byte-identical runtime counterfactual to confirmed causality.

`CPU-SENSOR-AMD-CXL-EXECUTABLE-DIRECTORY-ROOT-CAUSE-CLOSURE` 已完成只读根因闭合。根据 `CXLBaseTools.dll` 的可见 QE-1 谓词，M1 hold fixture 的进程 EXE 目录为 `F:\File\codex\codex-worktrees\08bd\resource-timeline\tools\amd-uprof-public-api-ab\target\release`，与 `D:\apps\AMDuProf\bin` 和 `D:\apps\AMDuProf\bin\AMDPerf` 均为非零不相等；存活的 `AMDuProf.exe` 目录为 `D:\apps\AMDuProf\bin`，与第一候选为零相等。只读注册表 `HKLM\SOFTWARE\WOW6432Node\AMD\AMDProfiler\InstallationPath` 与已知安装根匹配。因此当前 `ROOT_CAUSE = CXL_PROCESS_EXECUTABLE_DIRECTORY_POLICY_MISMATCH`、置信度 HIGH；`PROCESS_BASENAME_ONLY_CONTROL` 已取消，目录-only confirmation 仍为可选、未运行且需要单独授权的安装树临时写入。详见 [`docs/measurements/cpu-sensor-amd-executable-directory-root-cause.md`](../measurements/cpu-sensor-amd-executable-directory-root-cause.md)。

该结论不宣称 CXL 直接调用 `KERNEL32!FatalExit`，也不改变历史记录、B1 门或生产 provider 计划。

## CPU-SENSOR-AMD-EXECUTABLE-DIRECTORY-FINAL-RUNTIME-CONFIRMATION 当前状态（HISTORICAL / SUPERSEDED）

> HISTORICAL / SUPERSEDED: this preparation record says “awaiting human
> authorization” because it predates the authoritative runtime evidence below.

已准备但尚未执行一次严格的 byte-identical directory counterfactual：复用
未修改的 `amd-uprof-static-api-hold-fixture.exe`，只将其临时复制到
`D:\apps\AMDuProf\bin\resource-timeline-amd-static-hold-confirm.exe`，从同一
手工管理员 x64 PowerShell、同一 `bin` 工作目录、无参数、无 debugger、无
profiling/sampling 启动，并在持久化 qualification 后删除且校验该精确文件。
wrapper 的非 AMD synthetic validation 和静态 PE/hash/signature preflight 已
通过；本目录确认仍为 `prepared / awaiting human authorization`，不能视为
runtime complete。不得运行 B1 或开始 `CPU-SENSOR-AMD-PROVIDER-DESIGN`。
详见 [`docs/measurements/cpu-sensor-amd-executable-directory-runtime-confirmation.md`](../measurements/cpu-sensor-amd-executable-directory-runtime-confirmation.md)。

## AMD CURRENT STATE RECONCILIATION

The earlier AMD investigation sections explicitly marked
`HISTORICAL / SUPERSEDED` above retain their raw findings, but their former
`BLOCKED`, `prepared / awaiting human authorization`, “do not start provider
design”, and loader-trace next-step wording is not current state. The
`CPU-SENSOR-AMD-CLI-SERVICE-CONTEXT-QUALIFICATION` section records the
completed Service/Session 0 result and its deferred privilege/IPC follow-ups.
The authoritative current AMD block is the spike closure below; this plan
intentionally has one current state for the completed spike and its deferred
follow-ups.

```text
AMD_ROOT_CAUSE_INVESTIGATION = completed
AMD_UPROF_FEASIBILITY = completed
AMD_UPROF_ROOT_CAUSE = completed
AMD_CLI_BOUNDED_SESSION = completed
SPIKE_RESULT = PASS_WITH_FOLLOW_UPS
PRODUCTION_ADMISSION = NOT_COMPLETE
AMD_SERVICE_CONTEXT = completed / PASS
AMD_PRIVILEGE_DEPLOYMENT = prepared / awaiting authorized LocalService + IPC runtime qualification
AMD_LONG_LIVED_SESSION = planned
AMD_TEMPERATURE_FREQUENCY = planned
AMD_PRODUCTION_PROVIDER = planned
NEXT_TASK = AMD-PRIVILEGE-I2
EXECUTION_PLAN_SINGLE_CURRENT_STATE = PASS
```

The authoritative next-task handoff is:

```text
AMD_SERVICE_CONTEXT_I1 = completed / PASS
SERVICE_SESSION0_AMD_CLI_QUALIFIED = true
AMD_PRIVILEGE_ARCHITECTURE = WINDOWS_SERVICE_BROKER
SERVICE_BROKER_FEASIBILITY = PASS
SERVICE_BROKER_CANDIDATE = EVIDENCE_SUPPORTED_PENDING_PRIVILEGE_AND_IPC
SERVICE_ACCOUNT_FIRST_QUALIFICATION_CANDIDATE = NT AUTHORITY\LOCAL SERVICE
SERVICE_ACCOUNT_FIRST_QUALIFICATION_SID = S-1-5-19
SERVICE_SID_REQUIRED = true
IPC_CANDIDATE = WINDOWS_NAMED_PIPE
MINIMUM_REQUIRED_WINDOWS_PRIVILEGES = UNPROVEN
NEXT_TASK = AMD-PRIVILEGE-I2
PRODUCTION_ADMISSION = NOT_COMPLETE
```

## CPU-SENSOR-AMD-ROOT-CAUSE-FINAL-CLOSURE 当前状态

`CPU-SENSOR-AMD-ROOT-CAUSE-FINAL-CLOSURE` 已消费用户生成的最终
directory counterfactual evidence。未修改的 hold fixture 从 repository
build directory 复制到 `D:\apps\AMDuProf\bin` 后保持相同 SHA，正常存活约
3 秒、写出两个 durable main markers 并以 `0x00000000` 退出；原目录运行仍
为约 63.2 ms、`0xFFFFFFFF` 且 marker 缺失。因此：

```text
PROCESS_DIRECTORY_RUNTIME_CONFIRMATION = PASS
BYTE_IDENTICAL_DIRECTORY_COUNTERFACTUAL = CONFIRMED
CXL_EXECUTABLE_DIRECTORY_POLICY_CAUSALITY = RUNTIME_CONFIRMED
ROOT_CAUSE = CXL_PROCESS_EXECUTABLE_DIRECTORY_POLICY_MISMATCH
ROOT_CAUSE_CONFIDENCE = CONFIRMED_BY_STATIC_AND_RUNTIME_COUNTERFACTUAL
```

精确临时 copy 已在 qualification 持久化后删除并核验不存在；本次核验的
`AMDPowerProfileAPI.dll` 与 `CXLBaseTools.dll` hash 未变化。该结果关闭
static/dynamic load、basename、signature、import topology、shutdown/detach
作为本 incident primary cause 的假设，并将 vendor executable-specific
context 收敛为 executable-directory policy；CRT 到 Kernel32 的 termination
transition 与 QE2 private role 仍是无需在 architecture work 前解析的内部
细节。详见 [`docs/measurements/cpu-sensor-amd-executable-directory-runtime-confirmation.md`](../measurements/cpu-sensor-amd-executable-directory-runtime-confirmation.md)
与 [`docs/measurements/cpu-sensor-amd-executable-directory-root-cause.md`](../measurements/cpu-sensor-amd-executable-directory-root-cause.md)。

## CPU-SENSOR-AMD-PROVIDER-ARCHITECTURE 当前状态

根因 investigation 已完成；该架构记录在 spike 实现前建立。基于官方
CLI 在已允许的 AMD 安装目录中成功产生真实短时 package-power evidence，且
直接 main-process API 不能从任意应用目录安全加载，当前架构方向为：

```text
AMD_PROVIDER_ARCHITECTURE = CLI_SUBPROCESS
DECISION_CONFIDENCE = MEDIUM
DECISION_STATUS = PROVISIONAL / NOT_PRODUCTION_ADMITTED
```

这不是全应用永久提权的决定，也不是把 CLI 短时结果直接批准为 production
metric。all-day lifecycle、输出稳定性、cadence、privilege deployment、
distribution/license、overhead 和 metric scope 仍须在单独 spike 中通过。
vendor-tree helper 因安装树 mutation/support/legal 风险未选为默认方向；
direct API 对 arbitrary install location 不兼容；alternative backend 尚无
足够 evidence 被选定。未来实现必须复用现有 `MetricProvider`、`ProviderHost`、
`CollectionPlan`、health/capability 和 failure-isolation seam，不得修改
Windows baseline 的基本 CPU usage 语义来承载 AMD 失败。

该架构方向随后进入独立的 `CPU-SENSOR-AMD CLI PROVIDER SPIKE`；当前状态
以本文件下方的专项条目为准。详见
[`docs/architecture/cpu-sensor-amd-provider-architecture.md`](../architecture/cpu-sensor-amd-provider-architecture.md)。

## CPU-SENSOR-AMD-CLI-PROVIDER-SPIKE 当前状态

AMD 根因调查已完成：

```text
AMD_ROOT_CAUSE_INVESTIGATION = completed
AMD_PROVIDER_ARCHITECTURE = completed / provisional CLI_SUBPROCESS
AMD_CLI_PROVIDER_SPIKE = technically qualified for bounded session / production admission deferred
CPU_PACKAGE_POWER_W_RUNTIME_QUALIFIED = true
CPU_PACKAGE_POWER_W_PRODUCTION_QUALIFIED = false
AMD_PRODUCTION_PROVIDER = not completed / not registered
AMD_PRIVILEGE_ARCHITECTURE = WINDOWS_SERVICE_BROKER
PRIVILEGE_DEPLOYMENT_DECISION = DEFER_LEAST_PRIVILEGE_AND_IPC
SERVICE_SESSION0_AMD_CLI_QUALIFIED = true
MINIMUM_REQUIRED_WINDOWS_PRIVILEGES = UNPROVEN
```

本 spike 已在不修改 production registration 的前提下准备可测试的 CLI
boundary：
`src-tauri/src/collector/amd_uprof_cli.rs` 复用现有
`MetricProvider`/`ProviderHost`/`CollectionPlan` 语义，提供 registry-derived
CLI discovery、x64/signature/version identity、直接 argument-vector subprocess
runner、bounded timeout/cancellation、session state、failure mapping 和
header-driven package-power CSV parser。`collector::manager` 没有注册 AMD，当前
`SystemSample`/生产 value path 也未被修改，因此默认 collector 行为保持不变。

用户随后仅运行了一次手工 Administrator x64 PowerShell 的 bounded
`timechart --event power --interval 1000 --duration 10 --format csv` session。
目标进程以 exit 0 完成，生成可解析的 9 条 socket package-power 数据，
wrapper 的 post-runtime summary 因 PowerShell `if` argument-expression 缺陷
未完成；目标结果和 raw artifacts 已离线恢复，未进行重跑。该证据只将
package power spike 提升到 bounded-session technical qualification，不能
推断 production metric approval。详见
[`cpu-sensor-amd-cli-spike-runtime.md`](../measurements/cpu-sensor-amd-cli-spike-runtime.md)。

`AMD_CLI_PRIVILEGE_DEPLOYMENT_ARCHITECTURE` 已完成架构、威胁模型和
fallback 审计；LocalSystem/Session 0 bounded run 已验证 Service Broker
可行性，但仍不能证明 minimum privilege 或 production deployment：
`AMD_PRIVILEGE_ARCHITECTURE = WINDOWS_SERVICE_BROKER`。
主应用继续保持 non-elevated-by-default，AMD provider 继续 optional、
failure-isolated；不注册 service/task，不实现 elevation，不修改 AMD
installation。详见
[`cpu-sensor-amd-privilege-deployment.md`](../architecture/cpu-sensor-amd-privilege-deployment.md)。

下一项单一 privilege qualification family 是
`AMD-PRIVILEGE-I2`：验证 least-privilege account、Service SID、named-pipe
ACL、semantic IPC、session ownership/cancellation。它位于已通过的
LocalSystem/Session 0 bounded run之后并先于 long-lived/all-day session
qualification；temperature/frequency、timestamp/storage contract、provider
registration、schema/UI 和 production admission 仍保持 deferred。详见
[`cpu-sensor-amd-cli-provider-spike.md`](../architecture/cpu-sensor-amd-cli-provider-spike.md)。

## CPU-SENSOR-AMD-CLI-SERVICE-CONTEXT-QUALIFICATION 当前状态

`AMD-SERVICE-CONTEXT-I1` 已基于既有 immutable authoritative run 完成；本次
修复只离线重算 post-runtime cadence，没有重新运行 AMD 或 Service：

```text
AMD-SERVICE-CONTEXT-I1 = completed / PASS
AMD_CLI_SERVICE_CONTEXT_QUALIFICATION = PASS / existing authoritative run reparsed
RESULT = PASS
RUNTIME = COMPLETED_FROM_EXISTING_AUTHORITATIVE_EVIDENCE
AUTOMATED_PREPARATION = PASS
AMD_RUNTIME_EXECUTED = true
REAL_AMD_RUNTIME_COUNT_BEFORE_TASK = 1
REAL_AMD_RUNTIME_COUNT_DURING_REPAIR = 0
SERVICE_CONTEXT_RUNTIME_COUNT_BEFORE_TASK = 1
SERVICE_CONTEXT_RUNTIME_COUNT_DURING_REPAIR = 0
SERVICE_REGISTERED_DURING_AUTHORITATIVE_RUN = true
SERVICE_REGISTRATION_REMOVED = true
SERVICE_REGISTERED_CURRENT = false
FIRST_HUMAN_WRAPPER_INVOCATION = BLOCKED_PRE_RUNTIME_HARNESS
FAILURE_OCCURRED_BEFORE_NEW_SERVICE = true
INCIDENT_CLASSIFICATION = POST_RUNTIME_EVIDENCE_PARSER_DEFECT
SERVICE_BROKER_CANDIDATE = EVIDENCE_SUPPORTED_PENDING_PRIVILEGE_AND_IPC
AMD_PRIVILEGE_ARCHITECTURE = WINDOWS_SERVICE_BROKER
SERVICE_SESSION0_AMD_CLI_QUALIFIED = true
MINIMUM_REQUIRED_WINDOWS_PRIVILEGES = UNPROVEN
LONG_LIVED_SESSION_ORDERING = AFTER_PRIVILEGE_CONTEXT_QUALIFICATION
NEXT_TASK = AMD-PRIVILEGE-I2
```

已新增独立 qualification-only SCM service harness：它不属于生产 Provider、没有
IPC/installer/autostart，不接受任意可执行文件、argv、cwd、环境或输出路径。
既有人工 run 在 LocalSystem/Session 0 下完成一次固定的 10 秒 package-power
session，目标 exit 0，产生 9 个样本并完成 Service/CLI cleanup。wrapper 最初
因 vendor 的 `H:m:s:fff` timestamp 被旧 parser 拒绝而报告
`CADENCE_INCONCLUSIVE`；本任务以 exact timestamp fixture 离线修复并验证
cadence 为 PASS。此前的 pre-runtime process-list bug 保持独立历史 incident，
没有被错误重分类为 Service/Session 0 failure；未进行第二次人工 runtime。生产
provider、least privilege、IPC、long-lived 和 production admission 仍未完成。
详见
[`cpu-sensor-amd-service-context-qualification.md`](../measurements/cpu-sensor-amd-service-context-qualification.md)。

## CPU-SENSOR-AMD-UPROF-LIVE-QUALIFICATION-SPIKE CLOSURE

当前 AMD uProf 技术可行性和 bounded live qualification spike 已完成；
生产准入保持独立 deferred：

```text
AMD_UPROF_FEASIBILITY = completed
AMD_UPROF_ROOT_CAUSE = completed
AMD_CLI_BOUNDED_SESSION = completed
AMD_UPROF_LIVE_QUALIFICATION_SPIKE = PASS_WITH_FOLLOW_UPS
SPIKE_RESULT = PASS_WITH_FOLLOW_UPS
PRODUCTION_ADMISSION = NOT_COMPLETE
AMD_SERVICE_CONTEXT = completed / PASS
AMD_PRIVILEGE_DEPLOYMENT = prepared / awaiting authorized LocalService + IPC runtime qualification
AMD_LONG_LIVED_SESSION = planned
AMD_TEMPERATURE_FREQUENCY = planned
AMD_PRODUCTION_PROVIDER = planned
NEXT_TASK = AMD-PRIVILEGE-I2
```

已证明的 package-power、CLI bounded session、LocalSystem/Session 0 Service
context 和 CXL executable-directory root cause 结果不会因 parser repair 而
回退为 blocked。以下 follow-ups 均未完成：

- `AMD-SERVICE-CONTEXT-I1` 已完成 LocalSystem/Session 0 bounded-session
  qualification；cadence parser repair 使用 immutable authoritative evidence
  完成，未执行第二次 runtime。
- `AMD-PRIVILEGE-I2`：least-privilege account、Service SID、named-pipe ACL、
  semantic IPC、session ownership/cancellation。
- `AMD-LIFECYCLE-I1`：long-lived session、restart/recovery、sleep/resume、
  orphan prevention、all-day overhead。
- `AMD-METRICS-I1`：package temperature、effective/average frequency 和
  aggregation contract。
- `AMD-PROVIDER-I1`：production registration、DTO/storage、settings、
  installer/update、fallback states 和最终 admission。

本 spike 的 PR scope 是：关闭 AMD uProf technical feasibility 和 bounded
live qualification；production privilege deployment、long-duration lifecycle、
additional metrics、storage/integration 和 final provider admission 有意保留
为独立 follow-up tasks。上面的 spike-era “当前不开始
`AMD-PRIVILEGE-I2` implementation” 是 `HISTORICAL / SUPERSEDED`，不再是
当前 execution gate。

## AMD-PRIVILEGE-I2 CURRENT STATE

`AMD-PRIVILEGE-I2` is the current task and is prepared but remains pre-runtime
until the single authorized LocalService + IPC qualification completes. It must
not be marked completed by synthetic tests alone.

```text
AMD_PRIVILEGE_I2 = prepared / awaiting authorized LocalService + IPC runtime qualification
SERVICE_ACCOUNT_CANDIDATE = LocalService
SERVICE_SID_QUALIFICATION = prepared
NAMED_PIPE_SECURITY_QUALIFICATION = synthetic PASS / real cross-integrity runtime pending
SEMANTIC_IPC = synthetic PASS
SESSION_OWNERSHIP = synthetic PASS
CANCELLATION = synthetic PASS
REAL_AMD_RUNTIME_DURING_PREPARATION = 0
NEXT_GATE = AUTHORIZED_LOCAL_SERVICE_IPC_RUNTIME
PRODUCTION_ADMISSION = NOT_COMPLETE
```
