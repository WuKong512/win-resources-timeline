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

不把 MSI Afterburner 或 HWiNFO 作为必需依赖，也不逆向复制其专有实现。优先使用公开、可再分发且许可清晰的官方接口；第三方连接器只读取其公开共享数据机制，并在 UI 标明来源与依赖状态。

## 统一接口

Provider 至少实现：

- `probe()`：返回设备、指标、权限、建议最小周期和预估成本。
- `start(plan)` / `reconfigure(plan)` / `stop()`：按 CollectionPlan 管理资源。
- `sample(deadline)`：返回值、时间、来源、quality 和错误类别。
- `health()`：暴露最近成功、连续失败、平均耗时和降级状态。

Provider 必须支持取消，不能在主采样线程无限阻塞。厂商 DLL 的加载、符号解析和调用封装在独立边界，故障不得带崩整个采集服务。

## CollectionPlan

CollectionPlan 由以下输入生成：

1. 用户的类别/指标开关与采样周期。
2. Capability Registry 的探测结果。
3. Provider 优先级、可靠性和最近健康状态。
4. 电池模式、系统压力等可选运行策略。

输出明确到 `(device, metric) -> provider, interval`。同一设备的同一指标同一时刻只允许一个 active source，避免重复轮询。若首选来源连续失败，可切换到已探测的备用来源并开启新的 collection session，以保持历史可解释。

## 启停语义

- 关闭 GPU 类别：停止所有 GPU Provider 定时器，释放句柄，不写 GPU 行。
- 只关闭 GPU 温度：若 Provider 支持批量读取，可继续单次批量调用但丢弃温度字段；若温度需要独立调用，则不再调用。
- 设置重新开启：重新探测必要能力，建立新 session 后开始采样。
- Provider 缺失或设备不支持：显示明确原因和安装/开启建议，不使用 0 填充。

## 进程选择器

每个进程采样帧保存以下集合的并集：CPU Top-N、内存 Top-N、I/O Top-N、当前前台、用户关注、异常规则命中。默认 N 通过基准测试决定，初始可取 5。每条记录带 `selection_reason`；不再因为某应用曾经处于前台，就永久采集它直到进程退出。

进程刷新应分层：轻量枚举每个系统帧执行，昂贵元数据只在首次看到 PID/映像变化时读取并缓存。PID 必须与启动时间或进程实例键组合，避免复用导致身份串联。

## 验收

- 运行时关闭某类别后，其 Provider 在两个采样周期内停止且数据库不再新增该类别行。
- Provider 失败不会阻断其他类别采集；UI 能区分禁用、不支持和失败。
- 切换来源后历史记录仍能追溯到 session/provider。
- 启用温度、功耗和频率后的后台开销满足性能预算。
