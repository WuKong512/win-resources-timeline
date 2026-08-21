# 崩溃检测与证据回溯

## 用户体验

每次应用启动后异步检查“上次检查点到现在”的 Windows 系统事件：

- 无新的崩溃证据：不弹窗、不创建空案例。
- 有新的 BSOD、意外关机或异常重启证据：建立案例，先保护证据时间窗，再在“崩溃回溯”页面显示。
- 证据不完整：标记事件类型的依据、数据覆盖率和缺口，不把普通应用异常退出宣称为系统崩溃。

## 事件证据

首版重点归一化常见 Windows Event Log 信号，例如 BugCheck、Kernel-Power、EventLog 非正常关机和正常启动/关机边界。事件 ID 只能作为信号之一，需结合 boot session、时间关系、BugCheck code/minidump 是否存在和 clean shutdown marker 分类。`event_time_ms` 表示日志记录时间；`incident_anchor_time_ms`（持久化为 `crash_case.anchor_time_ms`）表示物理事件时间估计，两者不能混用。Event 41 通常在重启后才写入，EventLog/6008 表示前一次关机，WER/1001 的日志时间也不自动当作物理锚点；优先使用事件里的可验证 previous-shutdown fact，否则使用 collection/session boundary，并保留原始 log time 供对齐。

读取游标使用 channel + record id/时间双重保护；原始事件摘要落入 `system_event` 并唯一去重，保证每次启动扫描幂等。每页最多读取 256 条，detector 会持续排空后续页，页间提交 cursor，并定期让出线程，避免 backlog 截断或把全部日志装入内存。系统日志访问失败只影响 crash 功能，不阻塞主采集；`ERROR_NO_MORE_ITEMS` 是正常结束，access denied 和其他 EvtNext 错误分别进入可见权限/失败状态。

## 案例与证据窗

默认候选窗口为崩溃前 30 分钟至崩溃后 5 分钟；实际后段通常跨 boot session，用于观察恢复后的启动压力。相同物理信号在 30 分钟关联范围内使用与分类无关的稳定 case identity；后续收到更强证据时从 abnormal/unexpected refinement 到 bsod，不重复建案或重复 hold。建立 `crash_case` 后立即创建 `retention_hold`，常规清理不得删除窗口内 raw system/process samples 和关键 system events。

`evidence_status` 至少包括 `pending`、`post_pending`、`partial`、`complete` 和 `failed`。在 anchor 后 5 分钟尚未过去时，先构建 pre 窗口，post 窗口保持 `post_pending`，不能把尚未到时误报成缺失；后台 detector 周期性 retry，到时后再生成 post summary。清理动作遇到 active hold 会拒绝并报告受保护范围；只有显式 release 后才允许普通清理，避免留下指向已删除证据的 hold。

保留策略应限制受保护案例数量或总空间；释放必须显式记录。默认保护最近 10 个案例，用户可改为 5/10/20 个或不限，并可单独设置空间上限。用户删除案例时，先解释这会解除保护，原始数据随后按普通保留策略异步清理。

## 证据整理管线

1. 识别物理崩溃锚点、日志记录时间、类型、boot 边界和证据完整度。
2. 加载窗口内系统曲线、Provider health、写入延迟和进程 Top-N。
3. 对崩溃前 1/5/30 分钟及恢复后窗口计算 avg/min/max/delta、峰值时间、Top-N 变化、样本数和覆盖率。
4. 生成结构化 `crash_evidence_summary`，只保存指标、窗口、统计值、证据引用和处理版本。
5. UI 将摘要、原始曲线和系统事件按时间对齐，不隐藏未知区间。

二次加工只做数学计算、排序和时间对齐。产品可以陈述“GPU 温度最高 86°C”或“进程私有内存增加 1.8 GB”，不能输出“过热”“内存泄漏”“可能原因”、严重度排行、排查结论或处理建议。若存在 BugCheck code 或 dump，只展示可验证元数据，不解释根因。

## 处理版本

`crash_case.processing_version` 与每个 summary 的版本固化统计口径。处理逻辑升级可从受保护证据重新生成摘要，原子替换并记录时间；任何版本都不得引入诊断结论。

当前 PR-05 实现使用 native Windows Event Log API（Rust `windows` bindings），扫描在 collector/database 启动后独立线程异步执行，并持续排空事件页；permission denied/API failure 只将 crash detector 标记为 `permission_denied`/`failed`，不阻塞 FrameWriter、Provider 或 Usage Tracker。事件默认只保留归一化字段和小型 payload facts，不保存 Event XML。

证据窗口固定为 `pre_1m`、`pre_5m`、`pre_30m`、`post_5m`。由于 v8 的唯一约束是 `(crash_case_id, metric_key)`，summary key 使用稳定的 `window:<window>:metric:<metric>` 前缀，GPU 追加 `:device:<stable_key>`，进程追加 `:process:<process_instance_key>`；DTO 仍单独暴露 window、metric、device/process identity。coverage 按实际可用 frame duration 计算，跨大间隔不填充；delta 定义为窗口内最后一个有效值减第一个有效值。

Crash evidence contains objective observations and statistics only; it does not infer root cause, severity, probability, blame, or remediation.

## 测试策略

- 单元测试：事件组合分类、窗口裁剪、去重、统计值、排序和 coverage。
- Fixture：导出的脱敏 Windows 事件与合成资源曲线。
- 集成测试：强杀采集进程、模拟未封口区间、数据库忙/写入失败。
- 真正 BSOD/强制重启：只在一次性虚拟机或专用测试机执行，并明确快照与恢复步骤。

## 验收

- 重复启动不会重复创建同一案例。
- 发现案例后，保留任务无法删除其证据窗；active hold 存在时 clear 也必须拒绝。
- 没有系统事件读取权限时，应用仍正常运行并给出可操作状态。
- 每个摘要可追溯到指标、时间范围或系统事件，且不包含原因判断、建议或严重度。

PR-06 负责 UI 呈现和 user-facing retention settings；dump analysis、root-cause analysis、severity scoring 和 recommendation engine 不是本产品的 crash evidence 目标。
