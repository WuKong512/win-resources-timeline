# 性能与存储预算

## 背景预算

项目已有历史测量约为后台 CPU 0.0029%、工作集 46.46 MB；该数据早于新增 Provider 和进程快照，不能直接作为新版本结论。下一阶段目标是在典型 Windows 11 设备上维持：

| 场景 | CPU 目标 | 内存目标 | 说明 |
| --- | --- | --- | --- |
| 默认后台采集 | 平均 < 0.5% 单机总 CPU | 稳态 < 80 MB | 核心 2 秒、进程 5 秒，常用指标开启 |
| 最小模式 | 平均 < 0.2% | 稳态 < 65 MB | 仅 CPU/内存/使用时间 |
| 查询/聚合 | 短时峰值允许更高 | 有界 | 不应造成持续卡顿 |

这些是工程门槛，不是未经测量的宣传值。PR-07B 负责 validated hardware profile 的 release/stability qualification；本轮 profile 为 Windows desktop、AMD CPU、NVIDIA GPU、no battery。它不能证明 full cross-hardware support declaration。Intel CPU、AMD/Intel GPU、NVIDIA GPU 和 battery-capable device 的代表机仍属于独立的 hardware support/compatibility declaration gate；未完成时必须标记 `Deferred hardware support declaration / compatibility qualification`，不得把单一 profile 结果写成全硬件支持 PASS。

硬件指标正式接入前先按 [Spike-01](./collection-feasibility-spike.md) 隔离测量每个候选 Provider。探针只决定接口与开销是否值得继续；它不能替代 PR-07 的 multi-session extended native qualification、存储增长测试或独立 hardware support declaration gate。

## 采样成本控制

- 均衡模式默认核心 2 秒、进程 5 秒；轻量为 5/10 秒，详细为 1/2 秒。
- Windows 节能模式下默认自动切换轻量计划，事件采集不降频；用户可以关闭自动切换。
- 前台变化事件驱动，15–30 秒心跳，不持续 1 秒轮询窗口。
- Provider 合并批量读取；同一指标不从多个来源重复采集。
- 类别关闭后停止线程/定时器和句柄。
- 进程只存 Top-N 并集、前台、关注和异常项；昂贵身份元数据缓存。
- 电池百分比 60 秒兜底并监听变化；无电池时不启动 Battery Provider。
- SSD/HDD 温度 30 秒，风扇/水泵与电池功率 5 秒，内存温度 10 秒；静态信息不进入高频循环。
- 查询按像素宽度/时间范围在 Rust/SQL 侧下采样，避免前端加载全部原始点。

## 写入可靠性

当前 15 秒内存批量意味着应用或系统崩溃时可能丢失最关键的末尾 5–15 秒。v7 建议每个核心资源 frame 使用一个小事务写入；到期的进程 frame 可加入同一事务，前台和关键状态事件即时短事务提交。SQLite 继续使用 WAL + `synchronous=NORMAL` 作为默认起点。

若基准显示可接受，可提供“强化崩溃取证”模式，评估更短周期或更强同步级别；不能在没有断电测试的情况下声称 NORMAL 可保证保存最后一帧。

写入失败时：保留有界队列、重试、记录丢弃计数并暴露健康状态。只有事务提交成功后才能移除队列数据。

## 默认分层保留

| 数据 | 默认建议 | 备注 |
| --- | --- | --- |
| 使用区间/日报 | 长期或不限 | 体积小，支持屏幕时间趋势 |
| 原始系统帧 | 7 天（可选 3–14 天） | 崩溃窗口例外保护 |
| 5 秒进程详细样本 | 3 天 | 数据量最大；可选 1/3/7/14/30 天 |
| 1 分钟进程聚合 | 90 天 | 时间桶相对 5 秒减少 12 倍 |
| 1 小时进程聚合 | 1 年 | 时间桶相对 5 秒减少 720 倍 |
| 每日应用资源统计 | 长期或不限 | 时间桶相对 5 秒减少 17,280 倍 |
| 1 分钟系统/能耗聚合 | 180 天（可选 90–365 天） | 长期趋势与日能耗来源 |
| 崩溃证据 | 最近案例数/空间上限 | retention hold 保护原始窗口 |

默认预设：轻量为详细 1 天/分钟 30 天/小时 180 天/日报 1 年；均衡为 3 天/90 天/1 年/不限；长期详细为 14 天/180 天/2 年/不限。用户也可分别调整和设置数据库空间上限。UI 展示预计每日增长、稳定占用和实际 main/WAL/SHM 总量，并明确延长保留期无法恢复已删除细节。

## 聚合与清理顺序

1. 先生成并校验分钟聚合。
2. 从分钟数据生成小时和每日聚合。
3. 核对源范围、行数、coverage 和 additive totals 后，才删除到期下层数据。
4. 小事务分批运行，限制每批行数/耗时并避开交互高峰。
5. 与 active retention hold 相交的原始系统/进程数据永不参与普通压缩删除。

功率积分只连接间隔合理的有效样本。每日 energy rollup 保存 Wh、覆盖/预期时长、Provider、power_scope 和组件构成；周、15 天、月由每日值相加。

## SQLite 维护

- 启用 incremental auto-vacuum 的迁移方式需单独评估，因为初次转换可能需要 VACUUM。
- 采用分批 rollup/删除，每批限制行数/耗时，避免长事务。
- 设置 WAL 自动 checkpoint/大小上限，并在空闲时执行受控 checkpoint。
- 空间统计包含 `.db`、`-wal`、`-shm`；不能只报告主库文件。
- 聚合完成并校验覆盖范围、计数和可加总字段后，才清理对应原始数据。
- 任何清理查询必须排除与 active retention hold 相交的时间范围。

## 基准与可观测性

每个 Provider 记录调用耗时、失败率、连续失败和采样超时；Writer 记录事务耗时、队列深度、writer delay、drop count；Maintenance 记录删除/rollup 行数和耗时。开发构建提供诊断页，正式构建至少能导出脱敏健康报告。

PR-07 mandatory acceptance 使用 multi-session extended native qualification：至少 3 个独立 native session、每个 >=10 小时、至少一个 >=12 小时、aggregate valid native runtime >=32 小时，并 collectively 覆盖 long idle/background、normal interactive、local-midnight rollover、sleep/wake、动态 Provider/category 启停、数据库忙/恢复、clean process shutdown/reopen 和 schema/integrity continuity。continuous 24-hour soak 保留为 optional extended qualification，不是 mandatory blocker。

该 multi-session model aligned with the application's expected real-world duty cycle：Windows 启动后通常运行十多个小时，再经历 sleep/shutdown 并在下一 session 重新启动、reopen DB，多日重复。它同时观察 within-session slow resource growth、repeated startup/shutdown、WAL checkpoint/recovery、Provider lifecycle recreation、stale native object/mutex risk 和 sleep/wake recovery；单次 >=10 小时与 >=12 小时约束仍保留对 memory leak、handle/thread growth、queue accumulation、WAL runaway 和 retry loop 的敏感性。性能回归基于同一采样配置比较，不能混用配置得出结论。

若真实宿主机 sleep/wake 被外部电源/输入状态异常打断，无法形成完整且可归因的应用恢复证据，必须保留实际 observation 并标记 `DEFERRED — NON-BLOCKING`；这不是 sleep/wake correctness 或 full hardware support 的 PASS，后续 compatibility declaration gate 仍需独立完成。
