# Windows 使用时间与前台应用统计

## 统计语义

“应用开着多久”不能等同于“用户使用多久”。产品同时提供：

| 指标 | 定义 |
| --- | --- |
| 前台总时长 | 应用拥有前台窗口的总区间，包括用户暂时离开 |
| 活跃使用时长 | 前台区间与电脑 `active` 状态区间的交集 |
| 空闲前台时长 | 前台区间与 `idle` 状态区间的交集 |
| 电脑使用时长 | 全部 `active` 区间之和，与具体应用无关 |

查询层按区间求交，不把状态复制到每秒采样行。这样既减少存储，也允许未来调整 idle 阈值后重新计算派生统计。

## 事件来源

- 前台切换：优先使用 `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`。
- 用户空闲：基于 `GetLastInputInfo` 定期判断，阈值可配置。
- 锁定/解锁、会话连接：Windows session notification。
- 休眠/恢复：power broadcast/系统事件。
- 心跳：每 15–30 秒校验当前前台窗口和开放区间，用于补偿事件丢失。

事件回调只收集最少标识并投递到内部队列，解析进程身份和数据库写入不在 Windows 回调线程完成。

critical session/power 事件使用独立的有界通道和非阻塞 fallback。若 critical lane 溢出，collector 会在处理后续 Resume/Connect 之前先写入明确的 `unknown` gap，再通过 heartbeat/resync 重新确认当前状态；因此无法确认的时间不会被猜成 `active`，也不会延长上一应用。回调不会执行 SQLite、进程路径重试、文件版本扫描或签名验证。

## 区间状态机

foreground 与 computer state 是两个独立时间轴。可信的前台切换、无前台、暂停、锁定、休眠、断开、退出和 clock/gap recovery 会封口 foreground；active↔idle 只切换 computer state，不拆分 foreground。重复事件去抖；短暂无法解析的窗口进入明确的 unknown/gap，不会归入上一应用。

应用启动时恢复未封口区间：最多延伸到上次可信 heartbeat/last_seen，不能直接延伸到本次启动时间。锁定和休眠期间不属于任何应用的 active usage；恢复后重新确认当前状态和前台应用。

同一批 foreground/computer-state Close、Start、Checkpoint action 在一个短事务中提交，事务成功后才确认 collector 内存状态。SQLite busy/deadline 使用有界重试，失败、重试和最后错误通过 collector health 计数/字段可见；失败的恢复 gap 会保留到后续重试。

原始区间变化只标记受影响的 local dates。日报重算采用 debounce 和最低执行间隔的维护调度，最终 shutdown 会强制执行一次；重算按 `foreground_interval ∩ computer_state_interval`、local-day boundary 和 `processing_version` 幂等更新，不把 heartbeat 次数当作使用时长。

## Windows 与应用会话

`boot_session` 表示 Windows 的一次启动/电源周期，使用 uptime/boot-time identity 并以小容差与已有记录 reconciliation；`collection_session` 只表示 Resource Timeline 的一次采集运行。同一 boot session 内可以有多个 collection session，应用重启不得被解释为 Windows 重启。锁定/解锁、睡眠/恢复和会话连接使用 Windows live notifications；应用未运行期间的完整历史 Event Log 补齐留给后续 system-event/crash 工作。

Windows boot identity 优先使用不随 wall-clock 校正整体漂移的系统 boot-time 信息；同一次 boot 的轻微 key 漂移通过 boot time reconciliation 复用，真实 reboot 才创建新的 `boot_session`。`EVENT_SYSTEM_FOREGROUND` 使用 callback 提供的 tick timestamp 转换为 UTC epoch milliseconds，并包含 Resource Timeline 自身窗口。

## 应用身份归一化

逻辑 `app` 与具体 `app_executable` 分离。当前运行时至少保存 `process_name`、display name、规范化 executable path；如果可靠获得 PID 与 process creation time，则建立 `process_instance`。可执行文件路径/版本变化不会在查询层自动拆分明显同名逻辑应用；无法可靠确认时保持 separate/unknown，不猜测。

系统组件、浏览器和多进程应用需要聚合规则，但不在第一版通过窗口标题推测网站或文档。进程资源样本关联 executable，日报聚合关联 logical app。

## 隐私

默认不保存窗口标题、文档标题、浏览器 URL 或网站名称。schema 中的 window context 预留保持未启用，应用使用统计不依赖窗口内容。

## 参考方向

可借鉴 ActivityWatch 的事件桶、AFK 语义和本地优先思想，以及 macOS 屏幕使用时间类产品的展示结构；Windows 端仍应使用原生事件和会话接口实现，不依赖持续高频轮询。macOS 在应用身份和系统级使用统计上有不同 API 优势，但不能直接移植其权限或数据模型假设。

## 验收

- 前台快速切换、锁屏、空闲、休眠和跨午夜场景无重叠或负区间。
- foreground total、active usage、idle foreground 来自两条时间轴的区间求交，日报重算幂等。
- 应用所有 active usage 之和不超过电脑 active time（允许 unknown 未归属）。
- 强杀应用后，恢复逻辑不会把停机时间计入前台使用。
- launch_count 只统计同时有 PID 和 process creation time 的 observed process instance；无法可靠确认时不以 foreground activation 次数代替。
- 数据库不存在新窗口标题、文档标题或 URL 内容。
