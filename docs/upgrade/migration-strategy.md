# schema v6 → v7 迁移策略

## 不可违反的约束

- 不删除或重建用户数据库来规避迁移问题。
- 迁移前创建可恢复备份，并检查可用空间。
- 迁移要么完整提交，要么回滚到 v6 可读状态。
- 迁移过程可重复检查；应用崩溃后能安全继续或恢复。
- 现有实际 DDL、迁移代码和测试是事实来源，文档名称不能代替代码审计。

## 上线方式

建议分两个版本交付：先引入 v7 schema、双读/回填和验证，再切换默认写入并在稳定版本后删除兼容代码。若实现复杂度要求一次迁移，也必须把 DDL、回填、校验和切换拆成有日志的阶段。

## 迁移阶段

1. **Preflight**：读取 `user_version`、执行 quick/integrity check、确认磁盘空间、记录数据库与 WAL 大小。
2. **Backup**：使用 SQLite Online Backup API 或一致性等价方案生成带版本/时间的备份，不直接复制正在写入的 WAL 数据库。
3. **Create**：在事务内创建 v7 表、索引、约束和 migration journal。
4. **Identity backfill**：将 `app_identity` 归一化到 `app` / `app_executable`，保留旧 id 映射表。
5. **Usage backfill**：将 `foreground_interval` 迁移到新身份；历史数据缺少 active/idle 状态时不伪造 active usage。
6. **Resource backfill**：将 `system_sample`、`app_resource_sample`、`app_resource_snapshot` 转换为 frame/category/process 数据，记录来源为 legacy-v6。
7. **Verify**：比较行数、时间范围、关键总量、外键、空值语义和随机抽样查询。
8. **Commit and switch**：仅在验证通过后更新 `user_version = 7`；保留旧表一段兼容期或按经过验证的后续迁移删除。
9. **Postflight**：重新打开数据库、运行核心查询、记录迁移耗时和结果，随后再安排增量维护。

## 历史数据语义

v6 中不存在的数据不能推断：例如历史样本没有温度，就保持 NULL/unsupported-by-schema；没有 computer state 时，只能保留 foreground total，不能把全部前台时长标为 active usage。

旧系统样本转换为 `sample_frame` 时按原时间戳和采样来源生成稳定映射。若多个旧表时间戳无法严格对齐，允许形成相邻 frame 或保存 legacy timestamp，不能为减少行数而错误合并。

## 校验清单

- 迁移前后最早/最晚时间一致，允许明确记录的精度转换。
- 每个旧 foreground interval 有映射结果或带原因的 quarantine 记录。
- 旧系统/进程样本转换计数符合转换规则。
- 所有新外键通过 `foreign_key_check`。
- 典型日使用时长和时间线查询在容差内一致。
- 重复运行迁移入口不会重复回填。
- 使用真实大库测量耗时、额外空间和中断恢复。

## 失败与恢复

DDL/回填事务失败时回滚并继续使用原 v6 数据库；如果数据库在提交后无法通过 postflight，则停止采集，向用户提供恢复备份和导出诊断的明确操作，不自动删除数据库。备份清理只能在新版本经过多个正常启动和用户可配置保留期后发生。

## 测试矩阵

覆盖空库、小库、大库、WAL 未 checkpoint、旧版本逐级升级、重复身份、无效区间、磁盘空间不足、迁移中强杀、迁移后降级启动。所有 fixture 必须脱敏并固定预期计数/摘要。
