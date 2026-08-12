# Resource Timeline Codex 交接文档

更新时间：2026-07-13

> 新 Codex 任务应先阅读本文件、`docs/mvp_implementation_audit.md`、`docs/roadmap_status.md` 和 `README.md`，然后以实际代码、数据库 migration 和测试结果为准继续维护。不要删除或重建用户数据库。

## 1. 项目目标

Resource Timeline 是 Windows 10/11 x64 本地桌面应用，用于低开销记录：

- 当前前台应用及 active / idle 时间线；
- 锁屏、休眠、恢复和采集 gap；
- 系统总 CPU、内存和磁盘吞吐采样；
- Today、Timeline、Resources、Settings 本地分析页面。

核心原则：本地优先、低开销、WebView 按需打开、数据缺口不伪造为零、不把系统资源错误归因给前台应用、不采集窗口标题、URL、键盘内容、剪贴板或文件访问路径。

## 2. 当前技术栈与目录

- 前端：React 18、TypeScript 5、Vite 5、Tailwind 3、ECharts 5、Zustand 5。
- 后端：Rust 2021、Tauri 2、rusqlite 0.32、SQLite WAL、Windows API、PDH。
- `src/`：React 管理窗口。
- `src-tauri/src/collector/`：采集调度、区间状态机、系统资源采样。
- `src-tauri/src/platform/windows/`：前台窗口、idle、session/power、单实例。
- `src-tauri/src/db/`：schema migration、writer、query 和数据库测试。
- `release/`：最终可交付便携版、NSIS、MSI 和中文使用说明。
- `docs/`：MVP 审计、路线图状态和本交接文档。

项目不包含 Git 元数据。`package-lock.json` 和 `src-tauri/Cargo.lock` 必须保留。

## 3. 已实现的生命周期

- 正常启动：采集器、托盘、数据库和管理 WebView 同时启动。
- `--background`：仅保留采集器和托盘，销毁初始 WebView。
- 关闭管理窗口：后台采集继续运行。
- 再次运行应用：单实例插件通知已有进程重新创建或聚焦管理窗口。
- 托盘：Open、Hide window (keep collecting)、Pause/Resume、Stop collection and exit；左键单击托盘图标打开管理窗口。托盘显式使用打包图标，不再显示透明占位。
- 开机自启：注册 `--background`，仅启动后台采集器和托盘。
- Release 入口使用 `windows_subsystem = "windows"`，不显示额外控制台黑窗；Debug 构建保留控制台。

## 4. 数据库与采集语义

数据库路径：

`C:\Users\Hello\AppData\Local\com.local.resource-timeline\resource-timeline.sqlite3`

当前 schema：`PRAGMA user_version = 6`。

- v1：应用身份、前台区间、系统样本、设置。
- v2：增加前台检查频率和系统采样频率设置，使用 `INSERT OR IGNORE` 无损迁移。
- v3：增加与系统样本关联的应用资源快照；按可执行文件聚合进程，保留 CPU、内存和进程 I/O 各前 5 名的并集，父系统样本清理时级联删除。
- v4：增加显式快照覆盖标记和单应用资源历史查询；曾作为前台出现的应用在运行时会持续保留，即使不在资源 Top 5。
- v5：修复旧版在前台检查频率为 5/10 秒时，被固定 2.5 秒 gap 阈值错误切碎的规律性区间；新区间引擎的 gap 判定随前台检查频率缩放。
- v6：持久保存 `start_with_windows` 偏好，默认开启；Release 启动时会把当前用户 Windows Run 项校正到稳定便携版路径并附加 `--background`。
- 时间使用 UTC Unix epoch 毫秒和半开区间。
- PID 不作为长期应用身份。
- open interval 定期 checkpoint；异常退出后恢复到 `last_seen_time_ms`。
- 锁屏、休眠、长调度 gap、暂停采集不会延续上一应用或资源速率。
- 原始系统样本按设置每日幂等清理；前台历史保留到用户主动清除。

当前本机已保存设置：

- 前台应用检查：5 秒；
- 系统资源采样：30 秒；
- idle 阈值：10 分钟；
- 原始系统样本保留：14 天。

代码默认值仍为前台 1 秒、系统 5 秒、idle 5 分钟、保留 7 天；管理窗口只允许选择不高于原始频率的配置。

## 5. 管理窗口

Settings 页面支持：

- 暂停 / 恢复后台采集；
- 控制仅后台开机自启；
- 修改前台检查、系统资源采样、idle 阈值和原始样本保留期；
- 查看数据库路径和大小；
- 隐藏 / 显示应用；
- 双重确认后清除采集数据，但保留设置；
- English / 简体中文切换，选择存入 WebView `localStorage`。
- 系统资源 5/10/30/60 秒采样选项展示准确性、短生命周期进程覆盖、后台开销和数据库增长的取舍及指标口径；
- Timeline 使用有数据日期月历，无数据日期置灰且不可选；idle 过滤始终可用并显示累计时长，当前日期会自动刷新。Today 当前日期同样会自动刷新。

本地化范围包括侧栏、Today、Timeline、Resources、Settings、图表图例、空数据和状态文本。后端错误字符串目前仍为英文。

## 6. 已验证证据

前一版已通过：

- 14 项 Rust 测试：区间切换、配置频率下的 gap 判定、long gap、missing foreground、pause/resume、数据库恢复、范围裁剪、v1 到 v6 数据保留、v5 历史碎片修复、自启动偏好、配置往返、边界校验、资源 Top 并集、快照级联清理和可用日期查询；
- 4 项 Vitest 时间/格式化测试；
- TypeScript lint、生产前端构建、Rust fmt、严格 Clippy；
- Windows 实机：真实前台/系统/磁盘样本、关闭 WebView 后继续采集、双启动单采集器、便携版打开管理窗口、设置即时保存、暂停/恢复；
- 早期 10 分钟后台基线：平均 CPU 0.0029%，平均工作集 46.46 MB，峰值 46.50 MB。

2026-07-13 最后一轮重复性能测试按用户要求停止，不再作为阻塞项。当前优先保证采集数据语义正确。

本交接版本新增黑窗修复和简体中文后，应重新执行前端测试、Rust 测试、Release 构建与 Windows 界面实测，结果写回 `docs/roadmap_status.md`。

## 7. 构建环境

- Visual Studio Build Tools：`C:\BuildTools`
- Cargo：`C:\Users\Hello\.cargo\bin\cargo.exe`
- MSVC：`14.44.35207`
- Windows SDK：`10.0.26100.0`

建议使用临时 `CARGO_TARGET_DIR`，避免再次把项目目录膨胀到 9 GB。前端依赖和临时构建目录验证后删除，最终产物复制到 `release/`。

PowerShell 中需通过 `VsDevCmd.bat -arch=x64` 初始化环境，并设置：

```text
PATH=C:\Users\Hello\.cargo\bin;%PATH%
LIB=C:\BuildTools\VC\Tools\MSVC\14.44.35207\lib\x64;C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\ucrt\x64;C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64
```

常规检查：

```powershell
npm ci
npm run lint
npm run test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri:build
```

WiX MSI 的 ICE 校验在当前机器无法访问 Windows Installer 服务。上一版 MSI 使用已经生成的 WiX object 加 `light.exe -sval` 链接；NSIS 正常构建，是普通使用的推荐安装包。

## 8. 发布产物

目标文件：

- `release/Resource Timeline Portable.exe`
- `release/Resource Timeline Setup.exe`
- `release/Resource Timeline.msi`
- `release/README.zh-CN.md`

交付文件名保持稳定且不含版本号；当前应用版本从 Tauri 运行时读取并显示在左侧栏。2026-07-14 实机启动 0.2.0 后，现有数据库已升级到 schema v5，修复 1086 个旧版规律性 `clock_gap` 碎片，今日活跃累计从约 4 分钟恢复到约 23 分钟，并确认 5 秒前台检查不会再次切碎新区间。

当前 `release/Resource Timeline Portable.exe` 为 0.3.1，Setup/MSI 仍保留 0.2.0。0.3.1 已通过 no-bundle Release 编译，用稳定文件名替换旧便携版并以 `--background` 重启；固定 12 秒 WAL 观察确认数据库写入已恢复。0.2.2 修复了 Switch 圆点缺少绝对定位左基准导致的错位；0.3.0 在不修改采集、数据库 schema 与数据语义的前提下，重构了四个管理页面的视觉系统与信息层级；0.3.1 新增独立“应用资源”页，按不区分大小写的进程名合并历史安装路径，支持本页独立的单日、7/30 天和起止日期范围。Today、Timeline、Resources 日期状态已解耦，并分别按实际数据日期禁用无数据和未来日期。Windows Run 自启命令会在启动与设置校正时规范化为带引号的稳定便携版路径。

9 GB 增长来自 `src-tauri/target`，不是软件、数据库或安装包。`src-tauri/target`、`node_modules`、`.vite` 和 `dist` 都是可重建缓存或产物，不应提交到源码仓库；本地持续开发时可临时保留，并继续将 Rust 构建限制为 4 jobs。

## 9. 已知限制

- 系统资源曲线仍是整机总量；点击曲线会读取同一采样点单独采集的主要应用资源快照，不从前台应用反推，应用行之和也不保证等于系统总量。
- UWP、受保护、提权或系统进程可能归入 unresolved。
- 采样之间的极短前台切换可能丢失。
- 尚无长期 1 分钟聚合、周/月视图、完整全进程历史、高分辨率归因或异常事件。
- 未完成安装包升级覆盖、多 DPI/多显示器和多日连续 dogfood。
- 托盘左键打开已编译，但上一轮最终 UI 自动化没有单独覆盖。
- MSI 未在当前机器运行 ICE；优先验证和发布 NSIS。

## 10. 后续优先规划

P0/P1：

1. 完成本交接版本的 lint、测试、Release 打包和黑窗/中英文实机验证。
2. 真实使用数日，核对 gap、unknown app 比例、system sample 数量、数据库日增长、WAL 回收和 autostart。
3. 补安装版升级测试，确保数据库和设置保留。
4. 增加轻量诊断页：schema 版本、最后前台/系统样本、WAL 大小、最近 lock/resume/gap、writer 错误计数。

P2：

5. 设计 1 分钟系统资源聚合表，保留缺口语义，再增加周/月视图。
6. 根据 unresolved 数据决定 App identity、图标、别名和合并规则的优先级。
7. 对 v3 应用资源快照重新测量 CPU、内存和数据库日增长；稳定前不要提高采样频率或扩大每个样本保留的应用数量。

不要直接进入 ETW、温度、网络或全进程高频扫描；这些属于可关闭的远期增强模式。

## 11. 新任务建议首条指令

```text
请先阅读 docs/codex_handoff_2026-07-13.md、docs/mvp_implementation_audit.md、docs/roadmap_status.md 和 README.md。
以当前源码、SQLite migration 和实际测试为事实基线，保留现有本地数据库，不重做已完成模块。
先执行交接文档第 10 节的 P0/P1 第一项，完成后更新 roadmap_status.md，并只报告实际运行过的验证。
```
