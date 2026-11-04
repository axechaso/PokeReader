# PokeReader 时间控制开发版

版本：0.9.0-dev.1。更新日期：2026-10-26。

这是可测试的开发版：已集成 PokeReader 菜单、日期预设、时钟模拟、乱数诊断与独立 3DS 日期编辑器。默认状态不写入商业游戏时间；另有一个必须手动保存启用的 Pokémon Y 缓存写入实验，用于验证已定位的时间缓存。菜单中的 Preview 仍是时钟模型预览，不能单独据此判断游戏已经锁时。

## 交付内容

| 文件 | 用途 |
| --- | --- |
| `default.3gx` | 整合版 PokeReader：原有乱数功能、Time control (dev) 菜单、诊断导出 |
| `PokeReader-Time.3dsx` | Homebrew Launcher 中运行的独立日期预设编辑器 |
| `PokeReader-Clock-Lab.exe`（Windows 包） | 电脑端交互式时钟实验程序，无需游戏文件 |
| `build-info.txt` | 构建环境、依赖指纹、提交编号和产物 SHA-256 |
| `adapter-status.json` | 实际启用的游戏时间适配清单；本版为空 |

保留上游 GPL-3.0 许可。源码、测试和构建流程随本仓库提供。

## 安装与操作

1. 将整合版放到 SD 卡 `/luma/plugins/default.3gx`。已有同名插件时先保留副本，再使用本版替换。
2. 将编辑器放到 `/3ds/PokeReader-Time/PokeReader-Time.3dsx`，从 Homebrew Launcher 运行。
3. 编辑器左右键选择游戏，L/R 选择字段，上/下修改，A 保存；B 放弃未保存改动并重新读取，START 退出。非法日期会被拒绝，例如不能把 1 月 31 日直接改成 2 月 31 日。
4. 进入游戏后按 PokeReader 原有操作显示界面：START+上显示/隐藏，X+Y 解锁菜单，方向键选择 `Time control (dev)`，右键进入。

日期预设按游戏存放在 `/3ds/PokeReader/time-lock/<16位TitleID>.ini`。修改这些文件不会更改主机系统时间或用户时间偏移。编辑器也不改写游戏存档。

### 游戏内开发菜单

| 操作 | 行为 |
| --- | --- |
| L/R，上/下 | 选择并编辑下一次完整启动的预设字段 |
| A | 保存预设；保存成功才清除未保存标记 |
| X | 切换**预览**的锁定/运行状态，释放瞬间连续 |
| B | 在预览锁定时按选定步长调整：±1 毫秒、±1 秒或 ±1 分钟 |
| SELECT | 用编辑后的目标重新开始一次预览会话 |
| Y | 导出本次进程的诊断 CSV |
| 左键 | 返回 PokeReader 主菜单 |

编辑下一次启动的目标不会立即改变现有预览。SELECT 才会按新目标重置预览。START+SELECT 仍是原有游戏暂停组合；预览控制不修改游戏 RNG。

`Request next boot`、初始模式与重复使用开关会写入预设。默认情况下界面显示 `Game clock: ORIGINAL (no adapter)`；Y 更新 5 在保存 `Request: true` 且 `Initial mode: Frozen` 后会显示 `TEST FROZEN (cache + seed hook)`，每帧把目标毫秒值写入已验证的 `0x005D45E0`，并在初始 seed 的时间调用处使用同一目标值。插件在 Y 的游戏内存就绪后会自动读取已保存的预设，因此完整重启后也会重新启用；关闭 Request 或重启插件即可停用。这是临时的内存实验，不是已发布的适配器，不改写存档。原有暂停、逐帧和 RNG 页面可继续使用。

运行模式预览按平台 tick 前进；休眠覆盖、HOME 恢复和实际游戏时钟语义均未通过实机验收。出现计时倒退或越界会报错，SELECT 可重新开始预览。

## 预设格式与保存恢复

```ini
schema_version=1
title_id=00040000001B5000
target=2026-10-26T12:00:00.000
initial_mode=frozen
reuse=true
enabled=false
penalty_policy=observe
```

日期是本地日历值，不做电脑时区转换。有效范围为 2000–2099；省略毫秒时按 `.000` 读取。字段必须完整，重复、未知字段、错误 Title ID、不兼容 schema 和未知惩罚策略均会被拒绝。`penalty_policy` 本版只接受 `observe`，不修改惩罚状态。

保存顺序为：写 `.tmp` 并刷新 → 回读校验 → 将有效旧预设保留为 `.bak` → 将临时文件改名为 `.ini`。主文件缺失或解析失败时尝试有效备份；不把损坏的主文件覆盖到有效备份。遇到 SD 错误会报告保存失败。文件系统模拟测试覆盖保存各边界中断、部分写入和改名失败；真实 SD 断电行为尚未实测。

## 无需 3DS 的电脑测试

Windows 包中运行 `PokeReader-Clock-Lab.exe --deterministic`。也可安装 Rust 后从源码运行：

```text
cargo +nightly-2024-03-21 run --manifest-path time_lock_core/Cargo.toml --example clock_lab -- --deterministic
```

输入以下命令可以观察锁定、释放和再次锁定的连续性：

```text
read
step 1
release
tick 1500
freeze
step -1
retry
quit
```

默认目标是 `2026-10-26T12:00:00.000`。上述输出依次包含 `.000`、`.001`、`12:00:01.501`、`12:00:01.500`，重试后回到初始目标。`tick` 指绝对模拟 tick，单位为毫秒；减小它会触发倒退错误。省略 `--deterministic` 时使用电脑单调时钟。`target <日期>` 修改下次重试目标，`save <文件路径>` 导出预设。已有预设可用 `--preset <路径> --title <TitleID>` 读取；导出时保留其请求开关。

## 诊断与后续游戏适配

日志位于 `/3ds/PokeReader/time-lock/logs/<TitleID>-<启动tick>.csv`。第一批事件保留在固定 32 条缓冲中，满后增加丢弃计数；热路径不分配内存、不写 SD。导出时先在短互斥区复制快照，再格式化并写盘，避免 ARM 上 64 位字段撕裂。

### Pokémon Y 更新 5 的时间缓存诊断

开发版的诊断会在 Title ID `0004000000055E00`、更新值 5 下读取 `0x005D45E0`，缓存发生变化时写入一条 `GameTimeCache` 事件。Y 的初始 seed 分支 `0x0010DA38` 也会记录一次 `SeedTimeHook`，其中 `value=1` 表示用目标时间填入 seed 的时间输出，`value=0` 表示放行原始时间调用。诊断在 Y 的插件帧循环中运行，不要求一直停留在 Time control 页面；页面只用于按 Y 导出日志。默认状态不写入游戏内存；只有保存缓存写入实验后，才会执行单独的测试写入。事件的 `game_ms`/`value` 是游戏使用的 2000-01-01 纪元毫秒值。

测试步骤：

1. 构建并安装新的 `default.3gx`，启动 Pokémon Y 更新 5。
2. 冷启动后进入游戏，打开 PokeReader 的 Time control 页面，按 Y 导出诊断 CSV。
3. 完整退出并重新启动，再导出一次；随后分别做一次强制刷新、HOME 返回、休眠恢复和触发一个每日事件，再各导出一次。
4. 对比 CSV 中的 `GameTimeCache` 行：冷启动应先出现一次初始化值，刷新路径只在缓存更新时出现新值；值应按 2000 纪元毫秒递增。当前预览菜单不会改变这些值。

如果没有 `GameTimeCache` 行，先确认运行的是更新 5、Title ID 是 Y，并检查 `0x005D45E0` 是否映射。该日志只能证明缓存读写时序，仍不能证明所有每日事件或惩罚逻辑都只依赖这一处缓存。

### Y 更新 5 缓存写入实验

在 Time control 页面选择目标日期，切换 `Request next boot` 为 `true`，保持 `Initial mode` 为 `Frozen`，按 A 保存。保存成功后界面会显示 `TEST FROZEN (cache + seed hook)`。此时插件把目标的 2000-01-01 纪元毫秒值写入 seed 时间调用的输出，并每帧写入 `0x005D45E0`，同时继续记录 `SeedTimeHook` 与 `GameTimeCache`。

实验顺序：先在当前日期记录一次原始 CSV，再启用测试锁，使用软重启，或从存档返回标题后重新进入存档页面，让游戏重新生成 seed，然后记录 CSV。锁定记录应包含 `# game_clock=test-seed-time-and-cache` 和一条 `SeedTimeHook,...,game_ms=<目标毫秒>,value=1`，`GameTimeCache` 也应保持在目标值。完整退出游戏再启动用于验证插件重载后的预设读取，HOME/休眠恢复和每日事件可另外检查。出现崩溃、日期异常或事件不同步时，立即把 `Request next boot` 设为 `false` 并保存，恢复原始时钟。

该实验只验证缓存是否控制游戏日历；它没有免惩罚、软重置或每日事件兼容承诺，`adapter-status.json` 仍保持为空。

第七世代沿用上游的种子初始化观察点，记录该上下文里的日历毫秒与种子 tick。它是 `SeedInput`，**不是已经定位的日历读取函数**。第六世代在原有读取循环中记录观察到的种子变化，标为 `SeedObserved`；日志不能证明捕获了首次读时。日志中的真实日历值使用 1900 纪元，游戏输入使用 2000 纪元，未知游戏值为 -1。启动时 plugin 的耗时会影响实际种子，关闭观察的对照实验仍待实机进行。

获得自己的解密、解压 ExeFS `code.bin` 后，可运行只读工具：

```text
python tools/inspect_time_code.py code.bin --title 00040000001B5000 --output report.json
```

输出文件散列、排除前 16 字节的候选指纹、上游身份标记匹配情况、共享时间页字面量和 tick SVC 的候选位置。**这些结果不是函数地址、ABI 或可安装适配器。** 工具不修改输入文件、不导出游戏字节、不自动生成可启用补丁。

每个未来的适配构建必须核实：Title ID、内部更新版本、代码指纹、原指令、ARM/Thumb、返回 ABI、单位/纪元、原函数路径、首读覆盖、存档/每日事件/惩罚路径、休眠及恢复。当前适配契约只覆盖 ARM 对齐补丁；不支持的布局必须拒绝。先校验所有位置再写入；任一写入或缓存同步失败会回滚，包括失败调用可能已经写入的部分。回滚失败必须中止启动，不能当作普通未启用继续执行。该事务目前仅在合成内存样本上测试，未接入任何商业游戏的时间函数。

上游游戏识别标记和已知 seed hook 只用于保留现有 PokeReader 能力；不计为时间锁定适配证据。新增启动检查在 C 层安装任何 overlay 或游戏 hook 前执行，未知/不匹配版本不写入，已识别游戏失败时通过加载器消息说明原因。界面特征缺失、重复或越界时也不安装。

## 兼容性与验收状态

| 游戏 | 候选内部更新值 | 时间预设编辑/核心测试 | 游戏锁时、首读、免惩罚、软重置 |
| --- | ---: | --- | --- |
| X / Y（各自身份标记） | 5 | 已实现 | 未验证、未启用 |
| 欧米伽红宝石 / 阿尔法蓝宝石（各自身份标记） | 7 | 已实现 | 未验证、未启用 |
| 太阳 / 月亮（各自身份标记） | 2 | 已实现 | 未验证、未启用 |
| 究极之日 / 究极之月（各自身份标记） | 2 | 已实现 | 未验证、未启用 |

内部更新值不是游戏界面上的 `1.x` 版本号。水晶 VC 和传送器保留原有功能，没有时间菜单；此功能不扩大其支持范围。未对其他插件或补丁组合做兼容验证。

下一步需要游戏构建与设备：先在一款 ORAS 和一款 USUM 上定位时间路径，完成至少 20 次首次读取观察；单版本锁时闭环通过后，再做 30 次完整重启、HOME/休眠、时间惩罚与每日事件验证，最后逐个扩展到八款游戏。此版本不承诺同一日期产生相同种子，也不提供免惩罚声明。

## 构建与核查

固定上游底座：`155de39c4b69de89a0f81381c60861bc21d10364`，Rust `nightly-2024-03-21`。CI 使用固定 digest 的 devkitARM/Rust 镜像，先复现上游构建，再 lint、测试并生成 3GX 与 3DSX。实际编译器版本及 libctru/3gxtool 指纹见产物的 `build-info.txt`。`.3dsx` 复用同一 Rust 日期解析及编辑函数，与游戏菜单没有第二套宽松解析器。

```text
make lint test
make
python -m unittest discover -s tests -v
```

源码测试覆盖日期所有日界两侧、闰日、非法输入、1900/2000 纪元、毫秒边界、长 tick 换算、重复命令、失败不改变状态、完整启动模拟会话、适配校验与回滚、有限日志、预设解析和编辑器 ABI。它们不替代实机验证。

公开资料：[GodMode9 导出/解密工具](https://github.com/d0k3/GodMode9#usage)、[libctru 时间实现](https://github.com/devkitPro/libctru/blob/1f8d386e375ce734e11a3199eb51b8c98cc7e93a/libctru/source/os.c)、[Luma 加载器 IPC](https://github.com/LumaTeam/Luma3DS/blob/144f55305ec3e62b6e25207865df527b31abae1e/sysmodules/rosalina/source/plugin/plgldr.c)。公开的[《月亮》逆向项目](https://github.com/gradenGnostic/pokemoon/blob/main/docs/PROGRESS.md)面向基础 1.0，不能直接当成本版更新构建的补丁表。
