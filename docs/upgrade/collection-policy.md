# 采集范围、频率与能耗口径

本文冻结产品层采集策略。Provider 的技术实现可演进，但不得改变这里定义的用户语义、缺失值含义和默认计划。

## 最终指标范围

### 默认核心类别

- CPU：总使用率、平均频率、Package 温度、Package 功耗。
- GPU：按设备记录总/3D/Compute 使用率、专用/共享显存、核心频率、核心温度、Board Power。
- 内存：物理内存已用量、使用率和可用量。
- 磁盘：按物理设备记录读写速率和可用时的温度。
- 网络：按物理接口记录上传/下载速率；Wi-Fi 可记录信号、PHY 速率和断开/重连事件。
- 进程：CPU、工作集、私有内存、GPU/显存、磁盘/网络 I/O，以及生命周期和前台状态。
- 系统事件：Windows 启动/关机、睡眠/休眠、唤醒/唤醒源、BugCheck、意外关机、WHEA、TDR 和相关驱动事件。

### 条件提供

CPU 热节流、GPU Hotspot、GPU 热限制、内存温度/频率、风扇/水泵、屏幕亮度、真实整机功耗只在能力探测通过且开销达标时提供。采集困难或价值不足时允许舍弃，不得阻塞核心采集。

首版不采集刷新率和 HDR 状态。不会为完善功耗合计而新增本身价值有限的功耗传感器。

## 采样预设

| 模式 | 核心资源帧 | 进程资源帧 | 用途 |
| --- | ---: | ---: | --- |
| 轻量 | 5 秒 | 10 秒 | 电池节能和最低后台开销 |
| 均衡（默认） | 2 秒 | 5 秒 | 日常时间线与崩溃证据回溯 |
| 详细 | 1 秒 | 2 秒 | 更高分辨率，增加存储和开销 |

Windows 节能模式开启时默认从均衡/详细切换到轻量，退出后恢复；用户可以关闭自动切换。事件类、静态类和慢传感器不随预设盲目加速。任何自动或手动计划变化均开启新的 collection session。

## 独立周期

| 指标 | 默认方式/周期 |
| --- | --- |
| CPU/GPU/内存、磁盘与网络吞吐 | 跟随核心资源帧 |
| 进程 CPU/内存/GPU/I/O | 跟随进程资源帧 |
| SSD/HDD 温度 | 30 秒 |
| CPU/机箱风扇、水泵 | 5 秒；仅支持设备 |
| Wi-Fi 信号与 PHY 速率 | 5 秒；仅 Wi-Fi |
| 电池充放电功率 | 5 秒 |
| 电池剩余电量 | 启动读取 + 变化事件 + 60 秒兜底；值或状态未变不写库 |
| AC/充电状态、系统电源模式 | 变化事件，低频心跳兜底 |
| 电池健康度 | 启动及每天一次 |
| 电池循环次数 | 每天一次 |
| 内存工作频率 | 启动/设备变化，睡眠恢复可复核 |
| 内存温度 | 10 秒；仅支持设备 |
| 屏幕亮度 | 变化事件，低频兜底；仅支持的内置屏幕 |
| 显存总量、型号、容量 | Provider/设备启动和设备变化 |
| 前台应用、进程生命周期、系统/崩溃事件 | 事件驱动 |

系统运行时长、应用前台/运行时长、能耗和长期资源统计均为派生值，不新增固定轮询。

## 电池能力探测

未探测到系统电池时，Battery Provider 保持停止，不创建定时器或数据库行。电池百分比使用 60 秒兜底而非高频采样；充放电功率仍为 5 秒，因为它在百分比变化之前就具有能耗分析价值。

## 功率与耗电量

当来源提供累计能量计数时优先使用差值；只有瞬时功率时，以相邻有效样本的梯形积分计算 Wh。超过允许间隔、睡眠、暂停或 Provider 故障形成缺口，不延伸上一数值。

每个功率/电量结果保存 Provider、`power_scope`、参与组件、`covered_duration_ms` 和 expected duration。CPU Package 和独立 GPU Board Power 在范围不重叠时可形成“CPU + GPU 已记录耗电量”；它不等于整机插座用电。真实整机输入仅使用 EMI 或明确的外部功率设备。

长期保存 CPU、各 GPU、整机/电池和允许合计的每日 Wh；周、15 天、月统计由每日值相加，并始终展示覆盖率。

## 进程 CPU 表达

原始进程 CPU time 是所有线程在处理器上执行时间之和，多核并行时可以超过现实运行时间。它只作为 delta 与长期聚合的底层事实。默认 UI 展示运行期间平均/峰值 CPU、CPU 消耗贡献和高负载时长；高级详情可显示核时或等效整机满载时长。

## 用户控制

用户可以选择采样预设、类别/指标开关、进程各层保留期、崩溃保护数量和数据库空间上限。关闭类别必须停止对应 Provider。设置页展示实际/预计空间、数据覆盖率和不可恢复提示；已删除或压缩掉的详细历史不能因后来延长保留期而恢复。

PR-03 的启停语义如下：

- `enabled` 是用户计划状态；collector pause 是运行时暂停，两者不互相持久化或混淆。
- 关闭类别时，CollectionPlan 不再为该类别调度采样；如果 provider 已无启用类别，则调用 `stop` 并释放采集资源，而不是只跳过数据库写入。
- `unsupported` 表示当前机器或来源不提供该能力；`failed` 表示能力存在但最近启动或采样失败；二者都不能伪装成用户 `disabled`。
- 采样失败产生缺失/不可用样本和有界重试状态，不把失败写成 `0`。`0` 只表示真实且合法的数值。
- Provider 的 probe、sample 和 stop 都在 bounded deadline/cancellation 边界内执行；startup/reconfigure failure 使用有界指数退避，shutdown 使用同一个绝对 deadline，超时不拖住其他 Provider 或使用时间线。
- 普通 probe、start、reconfigure、disable stop、pause stop 和 resume start 都在每个 Provider 调用前重新计算独立 operation budget；只有 shutdown 复用 collector 的原始绝对 deadline，因此慢 Provider 不会污染下一个 Provider 的普通 control budget，也不会把 shutdown 扩展为 Provider 数量乘以单次 timeout。
- ProviderHost 分开保存 desired plan、effective plan 和 observed/runtime lifecycle：desired plan 保留用户仍想采集的类别，effective plan 过滤当前 unsupported capability，runtime state 表达实际 stopped/running/failed/paused。能力恢复时从 desired intent 重新编译 effective plan，不能从已过滤的 effective plan 反推用户设置。

## GPU 存储口径

PR-04A 只建立通用 GPU storage contract，不宣布任何厂商 Provider 已准入：

- 每个 `gpu_sample` 属于一个 `sample_frame` 和一个 `hardware_device`。多 GPU 永远按设备分别保存；利用率、温度、频率和 VRAM 不跨设备相加。
- `gpu.utilization_percent`、`gpu.memory_controller_utilization_percent`、`gpu.temperature_celsius`、`gpu.power_watts`、`gpu.graphics_clock_mhz`、`gpu.memory_clock_mhz`、`gpu.vram_used_bytes`、`gpu.vram_total_bytes` 分别映射到可空 GPU sample 列。`NULL` 表示当前样本没有该值，数值 `0` 表示合法零值。
- GPU board power 的单位是 W，且 `power_scope` 必须为 `gpu_board`。不得在 UI 或 energy rollup 中称为 whole-system power、wall power、PSU input 或 total machine power。
- device/vendor/model/capacity 存在 `hardware_device`；provider、metric enabled/support status 和 interval 存在 `provider` / `collection_session_metric`。这些会话元数据是历史来源追溯的规范入口，不复制到每一行 GPU sample。

## 准入层级

硬件路线按以下层级判断，不能混为一个 gate：

1. **Spike-01 short-term implementation admission**：来源/许可、probe、权限、单位/范围、unsupported/failed/zero 语义、基本调用开销和受控 lifecycle 足够支持开始正式 Provider 实现。
2. **PR-04A storage contract**：在没有任何 production NVML/AMD/Intel/CPU sensor 的前提下，提供可迁移、可回滚、可查询的 GPU 数据存储路径。
3. **PR-04 production Provider admission**：每个正式 Provider 仍必须引用对应 Spike 报告，并补齐该来源尚缺的短期证据。
4. **Default-enable / support-matrix / release-stability gate**：24 小时 soak、数据库增长 soak、代表硬件矩阵、AMD/Intel 覆盖和完整 release hardware matrix 在这里评估；它们不是 PR-04A 的实现入口条件。

当前 Spike-01B 仍只支持在 RTX 5070 Ti 开发机上继续 NVML feasibility work。Administrator comparison、30-minute idle、30-minute representative-load、cleanup/re-enable、failure/partial-support 和可行时的 sleep/wake evidence 完成前，NVIDIA production Provider 保持 pending。
- 超时调用会保留隔离 worker 中的 pending completion。Host 按 operation 和 generation reconcile late lifecycle result；新 settings、disable、pause 或 shutdown 产生的新 intent 会拒绝旧 result 恢复 Running。过期 sample payload 丢弃，不写当前 frame；旧 probe result 不覆盖更新后的 capability generation。
- current-generation 的 late Probe success/failure 若改变 capability truth，会同步更新 canonical descriptor 和 effective CollectionPlan；Unsupported -> Supported 可以恢复用户仍启用的 active category，Supported -> Unsupported 会移除它。若用户已经 disable、pause 或 shutdown，只更新 capability/status，不自动启动 provider。
- late Stop failure 若当前 intent 仍 inactive，则保持 Failed/StopFailed 并清除 retry；若用户已重新 enable，则为当前 generation 安排 bounded cleanup-before-start recovery，不会留下 enabled + failed + no retry 的永久状态。pause/shutdown 不会因旧 stop completion 安排新的 start。
- Windows baseline 的 Disk 只有在当前 settings 请求该类别且 PDH query/counter 初始化 probe 成功时才进入 active plan；用户禁用 Disk 时不建立 PDH Disk query。probe 失败显示明确不可用原因，不用永久 `None` 或 `0` 冒充磁盘数据，CPU、memory、process 不因 Disk 缺失而停止。
- probe 成功并不冻结能力 truth：如果 start/reconfigure 的 Disk sampler 初始化发生 TOCTOU failure，Provider 返回 capability outcome，Host 更新 canonical capability 和 CollectionPlan，Disk 变为明确 unavailable/failed，其他 baseline categories 继续运行。
