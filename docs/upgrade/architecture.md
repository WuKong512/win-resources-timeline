# 总体架构

## 技术栈结论

现有 Tauri 2 + Rust + React 18 + TypeScript + Vite + Tailwind + ECharts + Zustand 足以支持目标功能和接近 Codex 风格的精致桌面 UI。当前限制主要来自信息架构、组件规范、状态边界和数据接口，而不是框架能力，无需为了 UI 重写技术栈。

Rust 侧继续负责 Windows API、硬件 Provider、采集计划、SQLite、分层聚合和崩溃证据整理；React 侧只通过稳定的 Tauri command/event DTO 读取能力、设置、查询结果和进度。

## 模块边界

```mermaid
flowchart TD
    UI[React UI] --> API[Tauri Commands / Events]
    API --> QUERY[Query Service]
    API --> PLAN[CollectionPlan Manager]
    PLAN --> PROVIDERS[Metric Providers]
    PLAN --> USAGE[Usage Tracker]
    PROVIDERS --> WRITER[Frame Writer]
    USAGE --> WRITER
    WRITER --> DB[(SQLite v7)]
    DB --> QUERY
    CRASH[Crash Detector / Evidence Builder] --> DB
    MAINT[Retention / Rollup] --> DB
```

| 模块 | 责任 | 不应承担 |
| --- | --- | --- |
| Capability Registry | 探测 Provider、设备和指标能力 | 决定用户设置 |
| CollectionPlan Manager | 合并能力、设置、采样周期，管理启动/停止 | 直接写 SQL |
| Metric Provider | 读取一类来源并返回带质量信息的样本 | 做保留和聚合 |
| Usage Tracker | 记录前台切换和电脑状态区间 | 轮询所有进程资源 |
| Process Selector | 选择诊断价值高的进程并标注原因 | 保存所有历史前台应用 |
| Frame Writer | 将一次采样原子写入并重试 | 随意丢弃失败队列 |
| Query Service | 按时间范围查询、下采样、联表 | 暴露数据库内部结构给 UI |
| Crash Detector/Evidence Builder | 识别事件、锁定证据窗、生成可复核的统计摘要 | 生成原因判断、建议或严重度排行 |
| Maintenance | rollup、保留、checkpoint、空间统计 | 删除 retention hold 数据 |

## 运行时生命周期

1. 打开数据库，执行完整性检查和向前迁移。
2. 根据 Windows uptime/boot-time 信息建立或复用 `boot_session`，并为本次 Resource Timeline 运行建立新的 `collection_session`，读取用户配置。
3. 快速能力探测，生成 CollectionPlan；耗时探测可后台补充并增量更新计划。
4. 启动 Usage Tracker 和被启用的 Provider。
5. 每个系统采样帧以一个小事务写入；前台和电脑状态变化独立即时提交。
6. 后台分批执行 1 分钟/1 小时/每日 rollup、保留和 WAL checkpoint，避开启动和交互高峰。
7. 启动后异步扫描系统事件；发现崩溃时先建立 retention hold，再生成证据索引和客观摘要。
8. 正常退出时刷新队列、结束开放区间并记录 clean shutdown marker。

## 采样与背压

每个 Provider 有独立最小周期，不要求所有指标同频。均衡模式默认核心资源帧 2 秒、进程资源帧 5 秒；前台和电源状态变化即时写入，使用状态心跳 15–30 秒。Windows 节能模式下默认切换为核心 5 秒、进程 10 秒，并建立新的 collection session。慢传感器、静态信息和电池百分比遵循 [collection-policy.md](./collection-policy.md)，不随预设盲目加速。

若写入短暂失败，保留有界重试队列并指数退避；超过上限时记录 drop counter 和健康事件，不能静默清空。每个持久化样本保存实际时间和质量，不能用上一值填补睡眠、暂停、Provider 故障或调度长间隔。

## PR-01 存储骨架

PR-01 已把运行时写入边界接到 SQLite v7，但没有扩大 Provider、Tauri DTO 或 UI 范围：

- `Database` 持有串行写连接；查询使用独立只读连接，并设置 busy timeout。打开数据库时先完成 schema 迁移、开放 foreground 区间恢复，再创建新的 boot/collection session。
- `FrameWriter` 以 `ResourceSnapshot` 为队列单位。每个 frame 通过一个小事务写入 `sample_frame` 及已有的 system/process 子表；只有事务成功提交后才从队列移除。失败样本留在队首，重试次数和重试总数进入 `WriterHealth`，队列达到上限时只增加 drop count 并拒绝新样本。
- 正常 shutdown 的 foreground final actions、frame drain 和 collection session close 共用单一 deadline；等待串行 writer mutex 和每次 SQLite 尝试都计入剩余时间，后者按剩余时间设置 busy timeout。drain 跳过等待中的退避但仍遵守 retry 上限；transient error 后成功清空队列不构成最终失败，只有 terminal drop、deadline 到期或其他未完成 drain 才返回错误；collection session 在最终 flush boundary 之后关闭。
- `WriterHealth` 暴露 queue depth、writer delay、drop count、retry count、队首重试次数、最近提交耗时、最近提交时间和最近错误。collector 在健康状态中继续同步已有的丢弃计数；完整诊断展示不属于本 PR。
- `rollup.rs` 只提供 rollup 表清单、待处理 frame 时间窗和维护状态接口。v7 DDL 已预留分钟、小时、日报和能耗表，但本 PR 不启动完整 rollup、保留清理或后台维护任务。
- v7 查询保留按进程名合并不同可执行版本的既有语义；历史字段仍按来源和 NULL 语义表达，不能通过查询层补成完整 coverage。

## API 与版本边界

Tauri DTO 应面向产品语义而非表结构，例如 `MetricCapability`、`CollectionSettings`、`TimelineSeries`、`UsageSummary`、`CrashEvidenceDetail`。每个证据摘要带 `processing_version` 和 coverage，每个采集会话保存有效指标、周期和 Provider，使历史数据始终可解释。

## PR-02 使用时间追踪

PR-02 已落地 Windows 使用时间的双时间轴：

- foreground interval 只在应用切换、无可归属前台、暂停、锁定、休眠、断开、退出或可信 clock/gap boundary 封口；active/idle 转换不会拆分 foreground interval。
- computer state interval 单独记录 `active`、`idle`、`locked`、`sleep`、`disconnected` 和 `unknown`。锁定、休眠和断开状态优先于 idle/active，并持续到明确的 unlock/resume/connect 或 ObserverGap；睡眠时缺少 heartbeat 不会提前截断 `sleep`。
- Windows 前台切换使用 `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`。回调只投递 HWND 和时间到 bounded channel；PID、可执行文件和应用身份在 collector worker 中解析，回调不写 SQLite。
- 20 秒 heartbeat/resync 重新确认前台窗口和电脑状态；普通前台事件与 session/power critical 事件使用独立的 bounded lane。critical lane 溢出时先建立 pending `unknown` gap，再应用后续 Resume/Connect，避免丢失的 Sleep/Disconnected/Locked 时段被桥接为 active；事务失败时 pending gap 保留到下一次重试。
- usage Close/Start/Checkpoint action batch 在一个短 SQLite 事务中提交，成功后才更新内存 persistence state；`SQLITE_BUSY`、writer deadline 和重试次数进入 collector health。原始区间即时落盘，日报只标记受影响日期并 debounce/限频重算，不在每次 heartbeat 或 app switch 中同步重建多日日报。
- timestamp handling 区分可信 platform boundary、按 sequence 排序的平台观察事件和连续 heartbeat/resync 观察。后到但 source timestamp 略早的事件使用 monotonic effective timestamp，不制造 clock gap；长 observation gap 只保守封口 active/idle，explicit exclusion state 由对应恢复事件或 ObserverGap 结束。
- 查询层以 `foreground_interval ∩ computer_state_interval` 派生 `active_usage` 与 `idle_foreground`，并单独返回 foreground total 与 computer active time。日报按 local day 重算，保留 UTC epoch milliseconds，支持跨午夜和幂等重算。
- platform event 在 normal foreground lane、critical lane 和 fallback recovery lane 进入 collector 时共享 process-wide sequence；collector 按 sequence 合并消费，失败的队首事件或 recovery gap 在成功提交前保持 pending，后续事件不能越过。
- collector 的 platform pending 状态显式区分 raw/captured/prepared event、失败队首、coalesced recovery barrier 和当前时间 resync。overflow barrier 按第一个可能丢失的 sequence 排序，失败队首不会被 gap 阻塞；gap 自身写失败时保留 barrier 并以 bounded backoff 重试。shutdown 通过独立有界 lane 进入，并对所有平台 retry/SQLite write/最终 flush 使用一个绝对 deadline。
- recovery event 的前台身份与 active/idle snapshot 只在第一次 prepare 时捕获；后续 SQLite retry 重放 prepared payload，不重新读取未来的 OS truth。窗口解析失败只保留 explicit unknown，不把无法解析的窗口归入前一个应用。
- `boot_session` 使用 Windows 的 best-effort boot-time system source（不是永久稳定的公开 Win32 contract），不可用时回退到 uptime/wall-clock observation，并以小容差 reconciliation 复用；同一次 Windows boot 的应用重启只创建新的 `collection_session`，旧 session 以最后可信 checkpoint/last_seen 封口。
- `EVENT_SYSTEM_FOREGROUND` 回调使用系统提供的 event tick timestamp（无法提供时才回退到 delivery time），包含 Resource Timeline 自身窗口；默认不保存窗口标题、文档标题、浏览器 URL 或网站名称。
- 默认不保存窗口标题、文档标题、浏览器 URL 或网站名称；现有 context 预留保持未启用。

完整 Windows Event Log 历史补齐、Crash evidence、Retention Hold、Provider framework 和硬件指标属于后续 PR，不在 PR-02 中实现。

## PR-03 Provider Framework

PR-03 将 Provider framework 接入现有 collector，但不新增硬件采集源：

- `MetricProvider` 只负责 probe、descriptor/capability、受控 `start`/`reconfigure`/`sample`/`stop` 和 health observation；每次调用接收 deadline/cancellation context，lifecycle outcome 可以反馈 runtime capability 变化，Provider 不写 SQLite，也不拥有 UI。
- `MetricCategory` 与 provider identity 分离。当前保留 `cpu`、`gpu`、`memory`、`disk`、`network`、`power`、`battery`、`process` 这些产品类别；一个 provider 可以提供多个类别，未来一个类别也可以有多个来源。
- `CollectionPlan` 由持久化 settings、descriptor capabilities 和采样 policy 确定性编译。它只在启动、settings reload 或能力变化时重建，采样热路径只消费已编译计划。
- `ProviderHost` 是 collector 内的生命周期边界。用户禁用类别或 provider 时，相关 provider 被停止、采样调度清除、采集资源释放；pause 则暂时停止资源但保留用户启用计划，resume 可重新初始化。
- Provider status DTO 独立表达 `supportedEnabled`、`supportedDisabled`、`unsupported` 和 `failed`。合法的 `0` 仍作为 metric value 保存，不能用 0、缺行或 `None` 代替状态。
- `ProviderHost` 通过 bounded worker executor 等待 provider 调用；pending call 带 operation/generation，late lifecycle completion 按当前 user intent/plan reconcile，旧 probe/sample 结果不能覆盖新 generation，过期 sample payload 不进入 FrameWriter。单个 provider 的 startup/reconfigure/sample failure 只更新其 bounded health/backoff 状态，超时调用被隔离，其他 provider 和 foreground/computer-state timeline 继续运行。普通 control operation 为每个 provider 使用独立 budget；shutdown 使用现有绝对 deadline，provider stop 结果可报告 failure/timeout，不得无限阻塞或无限重试。
- Windows baseline 在启动或相关 settings reload 前按请求类别 probe PDH disk query/counter；用户禁用 Disk 时不建立 Disk query。若 start/reconfigure 时再次初始化发现 Disk 已不可用，lifecycle outcome 会把 capability degradation 反馈给 Host，Host 更新 descriptor 并修正 CollectionPlan；Disk 只从 active plan 移除，CPU、memory、process 仍可运行。
- 当前唯一接入的 production path 是把既有 Windows `sysinfo`/PDH CPU、内存、磁盘、进程 sampler 包装为 `windows-baseline` provider；没有引入 NVML、`nvidia-smi`、CPU 温度/功耗或其他新硬件源。

真实 NVIDIA GPU、CPU sensor 和其他硬件 Provider 留到 PR-04，并须先满足 Spike-01 的能力、权限、语义与开销证据。

## UI 架构方向

保留现有前端栈，逐步补充：

- 设计 token：颜色、间距、圆角、字体层级、阴影、动效和图表色板。
- 一套 App Shell 与导航，一级视图为“时间线 / 使用统计 / 崩溃回溯 / 设置”。
- 查询状态统一处理 loading、empty、unsupported、disabled、error 和 stale。
- 图表只消费下采样后的视图模型，避免把大量原始点送到 React。
- UI bug 修复单独成 PR，避免与 v7 数据模型改造混在一起。
