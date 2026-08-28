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

## CPU-SENSOR-AMD-LIVE-QUALIFICATION 当前状态

`CPU-SENSOR-AMD-LIVE-QUALIFICATION` 已完成安装审计与 qualification-only probe 实现，但当前 live qualification 为 `BLOCKED`：已安装 AMD uProf `5.3.521` 的 API DLL 路径、签名、x64 架构和官方 header/PDF/sample 均通过只读审计；非管理员隔离 load-check 子进程在显式 `LoadLibraryExW` 阶段异常退出（exit `-1`），尚未进入 `AMDTPwrProfileInitialize`。因此没有 live package power、per-identity frequency、temperature、cadence、lifecycle 或 busy evidence；没有管理员比较，也没有改变 Hypervisor/VBS/HVCI、driver/service、Afterburner/RTSS 或任何 production seam。package power 与 AMD per-identity frequency 保持 `DEFER`，temperature 保持 `DEFER_CURRENT_PLATFORM_CONFIGURATION`，CPU effective-frequency aggregate 保持 `DEFER_AGGREGATION_CONTRACT`，distribution 保持 `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`。不得据此开始 `CPU-SENSOR-AMD-PROVIDER-DESIGN`；应先在授权且配置不变的 Windows 环境解决 API load abort 后重跑本任务。详见 [`docs/measurements/cpu-sensor-amd-uprof-live-qualification.md`](../measurements/cpu-sensor-amd-uprof-live-qualification.md)。
