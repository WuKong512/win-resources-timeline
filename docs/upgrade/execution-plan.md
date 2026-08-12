# 实施计划

## PR 原则

每个 PR 只承担一个可验证边界，包含相应迁移/单元/集成测试和性能证据。数据库变更、采集 Provider、崩溃证据回溯和大规模 UI 改造不得堆入同一个 PR。当前 UI 明显 bug 等用户给出复现后单独修复，可与本路线并行但不混入 schema v7。

## 里程碑

| PR | 主题 | 主要交付 | 进入条件 |
| --- | --- | --- | --- |
| PR-00 | 架构基线 | 本目录文档 | 已完成仓库审计 |
| PR-01 | v7 存储骨架 | 新 DDL、migration journal、备份/校验、frame writer、rollup 骨架 | PR-00 合并 |
| PR-02 | 使用时间 | 事件驱动前台跟踪、电脑状态、应用身份、日报 | PR-01 |
| PR-03 | Provider 框架 | capability、CollectionPlan、启停、health、设置 DTO | PR-01 |
| PR-04 | 常用硬件指标 | Windows baseline + 厂商 GPU，CPU 传感器可选 Provider | PR-03 与基准工具 |
| PR-05 | 进程与崩溃 | Top-N selector、多级聚合、system events、case/hold/evidence summary | PR-01、PR-02/03 稳定接口 |
| PR-06 | 新信息架构 UI | 时间线、使用统计、崩溃回溯、采集/保留设置 | 后端 DTO 稳定 |
| PR-07 | 稳定与发布 | soak、迁移演练、性能/空间报告、发布说明 | 前述功能完成 |

## 每阶段验收门槛

### 数据安全

- 自动化迁移 fixture、外键/完整性检查和中断恢复通过。
- 禁止测试或生产路径删除用户数据库。
- 写入失败有重试与可见健康状态。

### 性能

- 报告测试硬件、采集配置、持续时间、平均/P95 CPU、内存和数据库增长。
- Provider 开关验证真正停止工作。
- 24 小时 soak 无无界队列、句柄、线程或内存增长。

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

1. 先补迁移 fixture、性能采样脚本和健康指标，建立可测基线。
2. 实现 v7 空表与旧数据迁移，但暂不扩大指标范围。
3. 将现有采样改为 frame writer，消除 15 秒批量丢失窗口和失败即清队列问题。
4. 分别落地事件驱动使用时间和 Provider/CollectionPlan。
5. 在代表硬件上逐个接入 GPU/CPU 传感器，达标才默认开启。
6. 建立 crash case/hold 后再实现证据摘要，防止整理前数据已被清理；不实现诊断规则。
7. 后端 DTO 稳定后统一升级 UI。

## 发布与回退

v7 首次发布应分阶段启用，可通过设置/feature flag 关闭新 Provider 和 crash evidence builder，但不能回写破坏 v7 数据。发布包保留诊断导出和备份恢复说明。若性能或稳定性未达标，回退采集计划和 UI 功能，不通过删除用户历史数据回退。

## PR-00 完成定义

- 十份设计主题文档与索引齐全，术语和表名一致。
- 明确产品非目标、隐私默认值、性能预算和数据安全约束。
- 后续 PR 顺序、依赖和验收条件可直接转为 issue/checklist。
- 本 PR 不包含运行时代码、schema 或 UI 变更。
