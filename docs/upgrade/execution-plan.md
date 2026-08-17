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

## 每阶段验收门槛

### 数据安全

- 自动化迁移 fixture、外键/完整性检查和中断恢复通过。
- 禁止测试或生产路径删除用户数据库。
- 写入失败有重试与可见健康状态。

### 性能

- 硬件指标在正式接入前完成 [Spike-01](./collection-feasibility-spike.md)，报告来源、权限、更新粒度和增量开销。
- 报告测试硬件、采集配置、持续时间、平均/P95 CPU、内存和数据库增长。
- Provider 开关验证真正停止工作。
- 24 小时 soak 无无界队列、句柄、线程或内存增长；这是 PR-07 release/stability gate，不是 PR-04A storage contract 的 entry blocker。

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

PR-04A 的完成不代表 NVML production admission。当前 Spike-01B 的单台 RTX 5070 Ti 60 秒 non-admin 结果只支持继续 feasibility work；administrator comparison、30-minute idle/representative-load、enable/disable/re-enable、cleanup/failure isolation 和可行时的 sleep/wake evidence 仍属于 PR-04 Provider 的短期 entry work。PR-04A 的 provider/session/device metadata 只证明 storage contract 具备 historical traceability capability，不证明生产 Provider 已自动维护完整 runtime truth。24-hour soak、数据库增长、跨硬件验证和完整 release matrix 属于后续 release/stability 或 support declaration gate。

## 架构基线完成定义

- 设计主题文档与索引齐全，术语和表名一致。
- 明确产品非目标、隐私默认值、性能预算和数据安全约束。
- 后续 PR 顺序、依赖和验收条件可直接转为 issue/checklist。
- PR-03 的 Provider runtime/DTO 接口已落地；PR-04A 的 schema v8 GPU contract 和现有 UI 信息架构保持独立，硬件 Provider 与新信息架构 UI 仍由后续 PR 负责。
