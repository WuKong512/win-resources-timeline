# Spike-01：采集接口与后台开销验证

## 目的

Provider 架构解决“如何替换来源和优雅降级”，但不能证明某项指标在真实 Windows 设备上一定可用、语义正确或适合全天轮询。Spike-01 在正式接入硬件指标前，用独立探针验证接口、权限、兼容性和增量开销。

Spike-01 不修改正式 schema、不接 UI、不写用户数据库，也不承诺某项指标进入默认配置。它输出可复核证据，决定指标是默认启用、可选启用、外部连接器提供，还是暂缓/舍弃。

## 交付物

优先建立独立 Rust CLI `tools/metric-probe`，复用未来 Provider 的最小调用边界，但不依赖 Tauri 生命周期。若某厂商 SDK 必须使用 C/C++ 或托管桥接，可建立隔离的实验适配器，不得因此提前引入正式应用依赖。

探针输出脱敏 JSON 和一份 Markdown 汇总，至少包含：

- OS/驱动/CPU/GPU/设备类别和探针版本；不保存序列号、用户名或文件路径。
- `(device, metric, provider)` 的 `supported`、`unsupported`、`permission_denied`、`provider_missing`、`probe_failed`。
- 原始值、单位、来源时间戳/更新粒度、功耗范围和建议最小周期。
- 调用 P50/P95/最大耗时、超时/失败率、进程 CPU、工作集、线程和句柄变化。
- 是否需要管理员权限、内核驱动、额外运行库、常驻服务或已运行的第三方软件。
- 与参考工具的对照结果、无法解释的差异和最终准入结论。

不支持或失败必须作为状态输出，不能用数值 0 代替。

## 首轮验证矩阵

| 组别 | 候选来源 | 重点指标与风险 |
| --- | --- | --- |
| Windows baseline | Win32/PDH/系统通知/Event Log | CPU/内存、磁盘/网络、进程、AC/电池、节能模式、启动/关机和睡眠/唤醒；验证语义、权限和事件补偿 |
| NVIDIA GPU | NVML | 使用率、温度、功率、频率、显存；逐函数处理 `NOT_SUPPORTED`，验证 GeForce/驱动差异和调用更新粒度 |
| AMD GPU | ADLX | 使用率、Edge/Hotspot、Board Power、频率、显存；先调用 support/range 接口再采样 |
| Intel GPU | Level Zero Sysman/其他许可明确的官方接口 | engine busy、温度、能量、频率、显存；验证消费级设备和 Windows 驱动覆盖 |
| CPU 传感器 | 可公开再分发的厂商接口、LibreHardwareMonitor bridge、外部连接器 | Package 温度/功率/频率；重点核对管理员/驱动要求、Intel/AMD 覆盖和空闲唤醒 |
| 电源 | Windows EMI、电池设备、外部功率设备 | 累计能量、充放电功率和真实整机范围；不得把电池或部件功耗冒充插座输入 |
| 进程归因 | Windows 进程计数器/GPU engine 来源 | CPU time、私有内存、I/O、进程 GPU/显存；验证 PID 复用、多 engine 去重和全进程枚举成本 |
| 存储与散热 | SMART/NVMe/可选硬件桥接 | SSD/HDD 温度、风扇/水泵；验证轮询是否唤醒休眠磁盘或增加系统中断 |
| 外部连接 | Afterburner/HWiNFO 公开共享数据 | 只在软件已运行且用户主动启用时验证；不作为核心功能依赖 |

首轮可以先在开发者当前设备完成可用来源验证；不能因单台机器通过就宣称跨厂商支持。正式默认开启前至少补齐 Intel/AMD CPU、NVIDIA/AMD/Intel GPU 和有/无电池设备的代表样本。

## 测试步骤

1. 非管理员和管理员环境各执行一次能力探测，记录权限差异。
2. 每个 Provider 单独运行：空闲 30 分钟、代表负载 30 分钟，使用产品均衡周期采样。
3. 关闭全部可选 Provider 取得基线，再逐个开启，计算增量 CPU、内存、线程、句柄和唤醒影响。
4. 与 Windows 任务管理器及可用的 MSI Afterburner/HWiNFO 对齐时间戳比较；差异必须解释采样窗口、平均/瞬时值和测量范围。
5. 测试运行时开关、Provider 超时、DLL 缺失、驱动更新/重启、睡眠/唤醒和 GPU 进入低功耗状态。
6. 存储温度探针单独观察磁盘电源状态；若轮询唤醒休眠 HDD，则不得默认固定周期采集，可改为磁盘活跃时读取或默认关闭。
7. 探针与 Afterburner/HWiNFO 同时运行，确认不会发生独占访问、明显额外唤醒或数值污染。

Spike-01 只做短期可行性和成本测量。正式发布仍需性能文档要求的 24 小时 soak、代表硬件矩阵和数据库增长测试。

## 准入门槛

指标进入默认采集需同时满足：

- 来源许可允许分发，初始化/卸载可控，失败不会拖垮其他 Provider。
- 能力探测与实际采样一致；不支持、权限不足和临时失败能可靠区分。
- 值的单位、范围、平均窗口和设备身份可解释，且无无法解释的明显偏差。
- 均衡计划下完整默认采集仍满足平均总 CPU < 0.5%、稳态内存 < 80 MB 的产品预算。
- 单个可选 Provider 若持续增加超过 0.1 个百分点的整机 CPU、阻止设备休眠或要求无提示常驻提权，不默认开启，除非后续有明确证据调整门槛。
- 停止 Provider 后定时器、线程、句柄和硬件访问恢复到基线；睡眠/唤醒后不形成忙循环。

需要管理员权限、内核驱动、额外运行库或第三方软件的来源只能作为显式可选能力，并在设置中提前说明。未通过门槛的指标保持条件项、外部连接项或舍弃，不阻塞 CPU/内存等基础采集。

## 与实施计划的关系

- PR-01 的 schema v7 和 frame writer 可以与 Spike-01 并行，不等待具体硬件来源。
- PR-03 可以实现 Provider 接口和 capability/health 语义，但不得凭假数据宣布某厂商指标受支持。
- PR-04 的每个正式 Provider 必须引用对应探针报告；默认指标必须满足本页准入门槛。
- 探针代码与报告单独成 PR，不混入 schema、UI 或正式 Provider 实现。

## 首选官方依据

- [Microsoft Energy Meter Interface](https://learn.microsoft.com/windows-hardware/drivers/powermeter/energy-meter-interface)
- [NVIDIA NVML API](https://docs.nvidia.com/deploy/nvml-api/nvml-api-reference.html)
- [AMD ADLX Performance Monitoring](https://gpuopen.com/manuals/adlx/adlx-sdk-references/adlx-interfaces/performance-monitoring/iadlxgpumetrics/)
- [Intel Level Zero Sysman](https://oneapi-src.github.io/level-zero-spec/level-zero/latest/sysman/PROG.html)
- [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor)
