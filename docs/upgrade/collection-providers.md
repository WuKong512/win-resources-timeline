# 采集 Provider 与指标选择

## 为什么采用 Provider 架构

Windows 原生 API 能稳定提供 CPU、内存、进程和磁盘基础指标，但 CPU 温度/功耗以及不同厂商 GPU 传感器没有一个统一、低成本且完整的系统接口。因此采集层必须把“产品指标”与“具体来源”解耦。

## Provider 分层

| 层级 | Provider | 典型能力 | 默认策略 |
| --- | --- | --- | --- |
| 基线 | Windows / PDH / sysinfo | CPU/内存/磁盘/进程 | 内置并默认启用 |
| 厂商 | NVIDIA NVML | GPU 使用率、温度、功耗、频率、显存 | 检测到兼容 NVIDIA 驱动时启用 |
| 厂商 | AMD ADLX | AMD GPU 常用指标 | 检测到兼容设备/运行库时启用 |
| 厂商 | Intel 官方接口 | Intel GPU 常用指标 | 能力探测通过时启用 |
| 兼容 | LibreHardwareMonitor bridge | CPU/主板等传感器补充 | 可选组件，明确版本与许可 |
| 外部连接 | Afterburner / HWiNFO shared data | 复用已运行软件提供的数据 | 仅在用户已运行并主动选择时 |
| 电源 | Windows EMI / Battery / 外部功率设备 | 整机或电池能量、功率 | 能力探测后按测量范围启用 |

不把 MSI Afterburner 或 HWiNFO 作为必需依赖，也不逆向复制其专有实现。优先使用公开、可再分发且许可清晰的官方接口；第三方连接器只读取其公开共享数据机制，并在 UI 标明来源与依赖状态。

本表描述候选路线，不等于接口已经通过产品验证。正式接入和默认启用必须提供 [Spike-01](./collection-feasibility-spike.md) 的能力、权限、语义和增量开销证据；单台设备读取成功不能外推为整个厂商系列支持。

## 统一接口

Provider 至少实现以下项目内部 contract：

- `probe(context, requested_categories)`：在受控调用边界内返回实际 capability/reason 结果。
- `start(plan, context)` / `reconfigure(plan, context)`：按 CollectionPlan 建立或调整采集资源，并可返回新的 runtime capability descriptor；这用于闭合 probe 成功后初始化再次失败的 capability degradation。
- `sample(context, timestamp, tracked_apps)`：context 携带绝对 deadline 和 cancellation signal。
- `stop(context) -> Result`：同样受 deadline/cancellation 约束，能报告 `StopFailed` 或 `Timeout`。
- `health()`：暴露最近成功、failure count 和简短错误摘要。

`ProviderHost` 通过每个 provider 的轻量 worker executor 调用这些同步 trait 方法。每个 pending call 记录 operation、generation 和 reply；collector 只在 deadline 内等待。超时会取消 context、保留隔离 worker 中的调用并把 provider 标为 failed，但 late reply 不会被无条件丢弃：Start/Reconfigure/Stop 会按 operation 和 generation reconcile，Sample 的过期 payload 会被丢弃，Probe 的旧 generation 结果不能覆盖新的 capability truth。普通 probe/start/reconfigure/stop/pause/resume control operation 为每个 provider 重新分配独立 budget；shutdown 才共享 collector 传入的单一绝对 deadline。Provider 若能合作，应在 OS/驱动调用前后检查 context；不合作的调用被隔离，不能在同一 provider 上并发发起下一次调用。这里不是动态插件系统，也不引入新的 async runtime。

## CollectionPlan

PR-03 的 CollectionPlan 由以下输入生成：

1. 用户的类别/指标开关与采样周期。
2. provider `probe` 返回的 capability support/reason 结果。
3. 当前产品采集 policy。

输出到 provider enable/category/interval。未支持的 capability 不进入 active sample plan；provider identity 与 metric category 保持分离。device routing、provider priority/fallback 和多设备 source 选择仍是后续硬件 Provider 工作，不由 PR-03 假定。

Plan 只在 startup、settings reload 或相关 capability 变化时重建；sample hot path 消费已编译的 plan，不每次 heartbeat 全量重建。

普通用户只选择轻量、均衡、详细预设；高级模式才覆盖类别或指标周期。系统进入 Windows 节能模式时，默认将均衡计划切换为轻量计划；退出节能模式后恢复。此自动行为可以关闭，事件类采集不因计划降频。

## 启停语义

- 关闭 GPU 类别：停止所有 GPU Provider 定时器，释放句柄，不写 GPU 行。
- 只关闭 GPU 温度：若 Provider 支持批量读取，可继续单次批量调用但丢弃温度字段；若温度需要独立调用，则不再调用。
- 设置重新开启：重新探测必要能力，建立新 session 后开始采样。
- Provider 缺失或设备不支持：显示明确原因和安装/开启建议，不使用 0 填充。
- 未探测到电池：不启动 Battery Provider、不创建定时器、不写电池行；设置显示“设备无电池”。

## 条件传感器

CPU 热节流、GPU Hotspot、GPU 热限制、内存温度/频率、风扇/水泵和屏幕亮度不作为核心功能依赖。只有来源可靠且实测开销达标时才加入计划；困难或高成本硬件上允许保持 unsupported。内存频率属于静态/设备变化信息，屏幕亮度优先使用变化事件。

## 功耗来源与范围

功耗样本必须带 `power_scope`，例如 `cpu_package`、`gpu_board`、`battery`、`wall`、`rail`。真实整机输入优先使用 Windows EMI、智能插座、UPS 或智能 PSU；笔记本电池充放电功率单独展示，不能冒充插座输入功率。

CPU Package 与独立 GPU Board Power 可分别积分并在范围不重叠时合计。若 CPU Package 已包含核显，不再叠加核显功耗。UI 使用“CPU + GPU 功耗/已记录耗电量”等明确名称；只有 CPU 可用时只显示 CPU，不使用模糊的“已知部件总功耗”。不会为补齐合计而额外启用低价值、高成本传感器。

## 进程选择器

每个进程采样帧保存以下集合的并集：CPU Top-N、内存 Top-N、I/O Top-N、当前前台、用户关注、异常规则命中。默认 N 通过基准测试决定，初始可取 5。每条记录带 `selection_reason`；不再因为某应用曾经处于前台，就永久采集它直到进程退出。

进程刷新应分层：轻量累计计数可覆盖所有可访问进程，用于长期应用聚合；详细样本只写 Top-N/前台/关注/异常集合。昂贵元数据只在首次看到 PID/映像变化时读取并缓存。PID 必须与启动时间或进程实例键组合，避免复用导致身份串联。

Windows 原始进程 CPU time 保留为内部单调累计计数，采样时计算 delta。默认 UI 展示平均/峰值 CPU、CPU 消耗贡献和高负载时长；高级详情才显示核时或除以逻辑处理器数后的等效整机满载时长，避免把多核 CPU time 误解为应用运行时间。

## 多设备语义

- 多 GPU 分设备存储和展示；利用率不相加，范围不重叠的板卡电量可以合计。
- 磁盘按物理设备存储，整机曲线为查询时派生汇总。
- 网络按物理接口存储；默认总量排除 loopback、重复虚拟接口和 VPN 重复流量，高级视图可逐接口查看。
- SSD 温度、风扇和水泵按稳定设备/传感器身份保存；无法可靠命名时显示来源标识，不猜测名称。

## 验收

- 正式 Provider 能追溯到探针报告；需要管理员、驱动、额外运行库或第三方进程时只能显式可选。
- 运行时关闭某类别后，其 Provider 在两个采样周期内停止且数据库不再新增该类别行。
- Provider 失败不会阻断其他类别采集；UI 能区分禁用、不支持和失败。
- 切换来源后历史记录仍能追溯到 session/provider。
- 启用温度、功耗和频率后的后台开销满足性能预算。

## PR-03 当前落地边界

PR-03 提供项目内部的 Provider contract，而不是动态插件系统：

- `ProviderHost` 在编译 CollectionPlan 前执行 provider `probe`，并用实际 capability 结果生成 plan；静态 descriptor 只是 probe 的初始 contract，不是最终可用性结论。Probe 可接收当前 settings 请求的类别范围，因此用户禁用的可选类别不会为了 capability 检查而建立采集 query；重新启用时再进行 probe。
- executor worker 为 `probe/start/reconfigure/sample/stop` 提供 bounded wait；sample、startup、reconfigure 使用 bounded exponential backoff，最大 60 秒；不合作的超时调用不会拖住 collector 或 foreground/computer-state timeline。
- `ProviderHost` 只对受影响的 provider 应用 start/stop/reconfigure delta。未变化的 settings 不重复启动或停止 provider；pause 会取消 retry、释放已启动资源，resume 按用户仍启用的 plan 重新 start。
- stop 返回结果并进入 health/status；shutdown 把同一个绝对 deadline 传给所有 provider，stop failure 不无限重试，也不能被伪装成正常 `Stopped`。
- timed-out lifecycle call 可以在隔离 worker 中继续完成；late Start/Reconfigure 成功会恢复实际 lifecycle 或触发一次针对当前 intent 的 reconcile，late failure 才进入当前 generation 的 bounded retry。Disable、pause、shutdown 和更新后的 plan 具有更高的 intent generation，旧完成不能恢复 Running；如果旧调用已经获得资源，Host 会在同一受控边界内清理。
- start/reconfigure 返回的 capability outcome 会更新 Host 的 canonical descriptor 和 active CollectionPlan。因 TOCTOU 导致的 Disk runtime unavailable 会从 plan 移除 Disk，但不会无条件判死 CPU、memory、process。
- Windows baseline 按当前 settings 请求的类别在 probe 阶段验证 PDH disk query/counter 初始化；不可用时仅 disk 进入 unsupported/reason，CPU、memory、process 仍可运行。合法磁盘吞吐 `0` 仍是值，不表示 unavailable。
- fake provider tests 覆盖 supported/disabled/unsupported/failed、真正停止采样、重新启用、startup/reconfigure retry、timeout 隔离、stop failure/timeout、Disk probe、pause 语义和 shutdown 幂等停止。
- 既有 Windows baseline sampler 通过 `windows-baseline` adapter 进入该框架，FrameWriter/SQLite 仍由 collector/writer 层负责。

本 PR 明确不新增 NVML production provider、`nvidia-smi` 调用、GPU 温度/功耗/频率、CPU 温度/功耗或任何新的硬件采集源。Spike-01 probe 仍是独立工具，PR-04 才负责真实硬件接入。
