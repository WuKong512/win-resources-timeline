# 下一阶段架构设计（PR-00）

状态：Draft；合并后作为下一阶段实现基线  
基线版本：v0.3.2 / SQLite schema v6  
基线提交：`0b50c5cc9acf33564c5274b6e58ce95dd16710c3`

本目录把下一阶段的产品边界、采集方式、数据模型、崩溃证据回溯和实施顺序固定下来。PR-00 只包含设计文档，不修改运行时代码、数据库或 UI。

## 设计目标

1. 持续、低干扰地记录日常最有价值的系统资源指标。
2. 允许用户按类别和指标选择采集项；关闭类别时停止相应 Provider，而不只是停止写库。
3. 同时支持屏幕使用时间、进程资源快照和系统崩溃后的证据回溯。
4. 在不破坏现有用户数据的前提下，将 schema v6 演进到 v7。
5. 保持 Tauri + Rust + React 技术栈，并为更精致、一致的桌面 UI 建立信息架构。

## 文档索引

| 文档 | 内容 |
| --- | --- |
| [product-scope.md](./product-scope.md) | 产品目标、非目标、隐私和默认行为 |
| [architecture.md](./architecture.md) | 模块边界、数据流和运行时生命周期 |
| [data-model-v7.md](./data-model-v7.md) | v7 表结构、语义和查询模型 |
| [collection-providers.md](./collection-providers.md) | 硬件 Provider、能力发现和降级策略 |
| [collection-policy.md](./collection-policy.md) | 最终指标范围、采样频率、能耗与用户设置 |
| [screen-time-tracking.md](./screen-time-tracking.md) | 前台应用与 Windows 使用时间统计 |
| [crash-detection.md](./crash-detection.md) | 崩溃识别、证据保护和客观摘要边界 |
| [performance-storage-budget.md](./performance-storage-budget.md) | 后台资源预算、写入策略和保留策略 |
| [migration-strategy.md](./migration-strategy.md) | v6 → v7 迁移、校验和恢复 |
| [execution-plan.md](./execution-plan.md) | 后续 PR 拆分、依赖和验收门槛 |

## 已冻结的关键决策

- 继续使用 SQLite；原始时序数据采用按类别宽表，不采用通用 `timestamp/metric/value` EAV。
- 屏幕使用时间与进程资源快照是两条独立数据链，查询时再关联。
- 前台切换优先使用 Windows 事件钩子，心跳仅用于恢复与校验。
- 硬件传感器使用可插拔 Provider；同一指标同一时刻只选择一个有效来源。
- 均衡模式默认核心资源 2 秒、进程资源 5 秒；Windows 节能模式下默认自动切换轻量采集。
- 功耗统计只组合本身有采集价值、来源可靠且范围不重叠的 CPU/GPU 数据，并明确标注构成与覆盖率。
- 进程数据按详细样本 → 1 分钟 → 1 小时 → 每日统计分层压缩，保留期限由用户控制。
- 崩溃功能只识别关键节点、保护证据并生成客观统计摘要，不提供原因判断、结论、严重度排行或处理建议。
- 任意迁移都不得通过删除或重建用户数据库来“修复”。

任何与这些决策冲突的实现变更，应先更新本目录并在 PR 中说明原因、影响和迁移方式。
