<p align="center">
  <img src=".github/assets/hero.svg" alt="Token Monitor — 面向 AI Coding 工具的无服务器跨设备用量分析" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/Atingaii/token-monitor/actions/workflows/rust-cli-ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Atingaii/token-monitor/rust-cli-ci.yml?branch=main&style=flat-square&label=CI"></a>
  <a href="https://github.com/Atingaii/token-monitor/releases"><img alt="Release" src="https://img.shields.io/github/v/release/Atingaii/token-monitor?style=flat-square&display_name=tag"></a>
  <img alt="Rust" src="https://img.shields.io/badge/runtime-Rust-000000?style=flat-square&logo=rust&logoColor=white">
  <img alt="Platforms" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-555?style=flat-square">
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-2f81f7?style=flat-square"></a>
</p>

<p align="center">
  <strong>一个轻量 CLI，聚合所有设备；成熟解析与计价来源；一个加密 Dashboard。</strong>
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="#快速开始">快速开始</a> ·
  <a href="#dashboard-密码机制">Dashboard 密码</a> ·
  <a href="#价格口径">价格口径</a> ·
  <a href="SOURCES.md">实现来源</a>
</p>

Token Monitor 是一个面向 AI Coding 工具的**无服务器、跨设备用量分析系统**。它以预编译 Rust CLI 的形式运行在 Windows、Linux 和 macOS 上，使用固定版本的 Tokscale Core 负责本地客户端扫描与 Token 语义，在此基础上补充 Provider/路由证据、成熟来源的订阅等价计价、加密同步以及统一 Web Dashboard。

它不需要 Electron、Node.js、Python、Docker、VPS、数据库、常驻 Hub，也不需要额外创建一个数据仓库。

> **v1.1 的两个关键变化：**Dashboard 不再依赖很长的 `#key=...` 链接；GPT-5.6 改为 CC Switch 兼容的**订阅等价价格口径**，不再跟随更低的通用 API 价格自动变化。

## 为什么做 Token Monitor

它主要解决单机统计工具难以回答的问题：

- 多台电脑加起来到底用了多少 Token？
- 消耗来自哪台设备、哪个 CLI、哪个模型、哪个 Provider、哪种 Route？
- GPT/Claude 模型究竟走官方、云平台、OpenRouter 还是中转？
- 如果按照明确的成熟项目价格口径折算，这些用量价值多少？
- 能否长期自动统计，但完全不留一个常驻监控进程？

## 核心原则

| 原则 | 实现 |
| --- | --- |
| **低负载** | 不常驻。系统调度器定期启动一次 `sync --quiet`，同步完成即退出。 |
| **跨平台** | Windows / Linux / macOS，x64 与 ARM64 都有原生构建和测试。 |
| **成熟 Token 核心** | 客户端发现、解析、去重、Token 桶语义来自固定的 **Tokscale v4.14.0**。 |
| **成熟价格来源** | 通用模型参考 CC Switch 的 models.dev 来源；GPT-5.6 使用 CC Switch 内置价格并由 Sub2API 独立交叉核验。 |
| **路由证据优先** | 模型厂商和实际请求路径分开；看到 GPT 模型不等于证明走 OpenAI 官方。 |
| **账本仍然加密** | 每台设备的聚合账本继续使用 AES-256-GCM 加密后上传。 |
| **密码可记忆、浏览器无关** | 用户密码通过 PBKDF2 + AES-GCM 包裹随机 workspace key；密码不上传、不进入 URL。 |
| **Tier 不放松 Token gate** | Codex Tier 细分只有在每日 Token 总量与 Tokscale 精确一致时才会被采用。 |

## 架构

```text
 Windows / Linux / macOS
          │
          │ 本地 session / 数据库
          ▼
      Tokscale Core v4.14.0
  解析 · 去重 · Token 语义 · 模型归一
          │
          ├──────────────► Codex Tier 证据适配器
          │                 （请求级，只负责 Tier）
          ▼
      token-monitor
  路由证据 · 计价 · 加密
          │
          ├─ AES-256-GCM 设备账本
          │
          └─ 密码包裹的 workspace key
                     │
                     ▼
 YOUR_NAME/token-monitor
 ├─ main                       项目源码
 ├─ tm-dashboard               仅 access.json
 ├─ tm-ledger-<device-A>       加密 ledger.json
 ├─ tm-ledger-<device-B>       加密 ledger.json
 └─ tm-ledger-<device-C>       加密 ledger.json
                     │
                     ▼
 https://atingaii.github.io/token-monitor/
 输入密码 → 浏览器本地解 key → 本地解密账本 → 分析
```

设备同步从不把遥测数据写进 `main`。每台设备只更新自己的 `tm-ledger-<device-hash>` 分支，并使用无历史增长的 root snapshot。`tm-dashboard` 仅保存被密码包裹后的随机 workspace key，不保存密码，也不保存任何用量内容。

## 快速开始

### 1. 安装

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
```

**Windows PowerShell**

```powershell
irm https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.ps1 | iex
```

安装器会自动下载对应 OS/架构的预编译 Release，下载 `.sha256` 并校验，然后实际执行一次 `--version`。用户不需要安装 Rust、Node.js 或 Python。

> **Linux 特别说明：**`curl | sh` 运行在子 shell 中，POSIX 机制决定它不可能修改当前父 shell 的 PATH。因此 v1.1 会优先选择已经在当前 PATH 中的用户目录；如果找不到，安装器会直接打印一个**当前终端立即可用的绝对路径 setup 命令**。新终端则会读取已经写好的 shell profile。

### 2. 第一台设备

```bash
token-monitor setup
```

`setup` 会自动：

1. 从参数、环境变量、已登录的 `gh` 或隐藏输入解析 GitHub 写入凭据；
2. 自动找到自己的 Token Monitor fork，必要时尝试创建；
3. 生成随机 256-bit workspace 加密 key；
4. 让你在终端中**隐藏输入一个 Dashboard 密码**；
5. 用 PBKDF2 + AES-GCM 把随机 key 包裹后写入 `tm-dashboard/access.json`；
6. 扫描第一份真实本地用量并上传加密设备账本；
7. 安装低负载系统调度器；
8. 输出稳定的 Dashboard 地址和第二台设备的 join 命令。

Dashboard 地址从 v1.1 开始只有：

```text
https://atingaii.github.io/token-monitor/?repo=YOUR_NAME/token-monitor
```

**不再带 `#key=很长的一串字符`。**

自动化环境可以使用：

```text
TOKEN_MONITOR_DASHBOARD_PASSWORD
```

不建议把真实密码直接写成 `--dashboard-password ...` 留在 shell history 中。

### 3. 打开 Dashboard

任意浏览器打开相同网址，输入第一台设备设置的 Dashboard 密码即可。这个浏览器此前不需要保存过任何 key，也不依赖 localStorage 才能访问数据。

### 4. 加入其他设备

第一台设备会打印类似：

```bash
token-monitor join 'eyJ2ZXJzaW9uIjoyLC4uLn0'
```

复制到另一台 Windows、Linux 或 macOS 机器运行即可。以后可重新生成显示：

```bash
token-monitor invite
```

Pair Code 包含仓库地址、随机 workspace key 和同步间隔，**不包含 GitHub Token**。由于新设备必须拥有 workspace key 才能参与加密/解密，因此 Pair Code 本身仍应当视为工作区密钥资料，不应公开。

## 从 v1.0 升级

重新运行安装命令覆盖旧二进制，然后只需要在**任意一台已经配置好的设备**执行一次：

```bash
token-monitor password
token-monitor sync --full
```

第一条命令会用你新设置的短密码包裹**原来的随机 workspace key**，因此无需重建所有历史设备分支；第二条会把当前设备账本切换到 v1.1 的价格元数据和计价规则。

旧的 `#key=...` 链接仍保留兼容读取能力，便于迁移；但 v1.1 的 `token-monitor dashboard` 不会再把 key 输出到 URL。

## Dashboard 密码机制

Dashboard 密码和账本 AES key 是两件不同的东西：

1. Workspace 内部始终使用随机 256-bit key 加密设备账本；
2. 用户只需要记自己的短密码；
3. 密码使用 **PBKDF2-HMAC-SHA256，310,000 次迭代 + 随机 salt** 派生包装 key；
4. 包装 key 再通过 AES-256-GCM 加密随机 workspace key；
5. GitHub 的 `tm-dashboard/access.json` 只保存 salt、nonce、迭代次数与密文；
6. 任意浏览器输入密码后都可以在本地重新完成同一套解包过程；
7. 解出的随机 workspace key 只存在于当前页面内存中，用于继续解密设备账本。

因此：

- 密码不上传；
- 密码不写进 URL；
- 密码不依赖某个浏览器保存；
- GitHub 上仍然没有明文账本。

但这仍是纯静态网页架构，并不是有服务器参与的账户登录系统。攻击者可以下载公开的密码密文后离线尝试猜密码，所以不要使用非常弱的 8 位纯数字密码，建议使用容易记忆但更长的 passphrase。

修改密码：

```bash
token-monitor password
```

修改密码只重新包裹同一个 workspace key，不需要重写全部设备历史账本。

## 价格口径

v1.1 页面展示的是 **订阅等价费用（Subscription-equivalent cost）**。

它不是 OpenAI / Anthropic 的真实发票，也不是 ChatGPT/Codex 订阅后台实际扣掉多少美元，而是按照一个明确、可审计、与成熟工具一致的模型价格表折算出的价值。

### 通用模型

通用模型价格采用 CC Switch 当前使用的公开 catalog：

```text
https://models.dev/api.json
```

Token Monitor 不重新解释原始客户端 Token。Tokscale 先把数据归一为：

- Fresh input
- Cache read
- Cache write
- Output
- Reasoning

然后再按照 CC Switch 的计价结构分别乘价。找不到价格时不会拿相邻模型猜一个价格，而是显示为未完全计价 / lower bound。

### GPT-5.6 固定订阅等价价格

GPT-5.6 不允许通用 catalog 的 API 降价静默改变这里的订阅等价统计。v1.1 固定采用 CC Switch 内置价格，同时由 Sub2API 的 fallback 表独立交叉核验：

| 模型 | 输入 / 100万 | 输出 / 100万 | Cache Read / 100万 | Cache Write / 100万 |
| --- | ---: | ---: | ---: | ---: |
| `gpt-5.6-sol` | **$5.00** | **$30.00** | **$0.50** | **$6.25** |
| `gpt-5.6-terra` | $2.00 | $12.00 | $0.20 | $2.50 |
| `gpt-5.6-luna` | $0.20 | $1.20 | $0.02 | $0.25 |

`gpt-5.6` 以及 effort 别名归入 Sol。

GPT-5.6 的 `fast` / `priority` 使用明确的 **2×** 价格卡，不再自己猜一个倍率。

当**某一条请求**的 input-side Token 超过 **272K** 时，按兼容的 Long Context 规则：

- Fresh input × 2
- Cache read × 2
- Cache write × 2
- Output（包含 Reasoning Token）× 1.5

这个判断发生在**单次请求粒度**，然后才汇总成日/模型/Tier；不会把“一天 600 万 Token”错误当成“一条 600 万 Token 的请求”。

如果源日志没有把 cache-write 等字段独立记录出来，相关费用会标记 `≥` / lower bound，不会伪装成精确金额。

完整来源见 [`SOURCES.md`](SOURCES.md)。

## Codex Fast / Standard

Tokscale 始终是 Codex Token 总量的 canonical source。Token Monitor 只额外使用 `falyx6851-byte/codex-monitor` 的成熟 Tier 状态/请求解析思路来恢复 `standard` / `fast`。

v1.0 的 gate 同时要求：

- Token 总量一致；
- message / record 数也一致；
- tier 价格完整。

而两个成熟 parser 的“记录数”粒度本来就不一定相同，这会造成真实 Fast 证据被错误丢弃。

v1.1 改为：**每日 additive Token 总量必须与 Tokscale 精确一致**；record count 不作为 Tier 接受 gate。只要 Token 有 1 个不一致，整个当天仍然回退 Tokscale canonical row，不会为了识别 Fast 而放松 Token 正确性。

## Provider / Route 区分

账本明确区分：

| 字段 | 含义 | 示例 |
| --- | --- | --- |
| `model` | Canonical 模型 | `gpt-5.6-sol` |
| `upstreamVendor` | 模型家族厂商 | `openai` |
| `routeProvider` | 有证据时的实际路由/计费方 | `azure-openai`、`aws-bedrock`、`openrouter`、`newapi` |
| `routeType` | 路由类别 | `official`、`cloud`、`aggregator`、`relay`、`self-hosted`、`unknown` |

模型名只能推断 `upstreamVendor`，不能仅凭 GPT/Claude 名字宣称 `routeType=official`。官方、云、Relay 的判断必须来自实际 source/config evidence。

## Web Dashboard

v1.1 UI 已按现代管理后台重新设计：

- 新版 New API 类的中性白/灰视觉语言与克制蓝色强调；
- 桌面端侧边栏支持折叠/展开；
- 折叠状态仅作为 UI 偏好记录，不参与数据解密；
- 移动端自动切换抽屉菜单；
- 今日 / 7 天 / 30 天 / 本月 / 全部 / 自定义；
- 设备、工具、模型、模型厂商、路由提供商、Route Type、原始 Provider、Tier 多维筛选；
- 总 Token、订阅等价费用、输入、缓存、输出/Reasoning、请求记录 KPI；
- 折线、面积、柱状、堆叠柱、环形、Treemap、表格；
- 设备视图、聚合原始数据视图；
- CSV 导出；
- 深浅色主题；
- 顶部直接显示当前计价来源；
- 浏览器本地 PBKDF2 + AES-GCM 解锁与账本解密。

用户自己的 fork **不需要另外配置 Pages**。统一页面通过 `?repo=OWNER/REPO` 读取对应 fork 的加密分支。

## 多客户端适配

Token Monitor 不自行维护一套 Codex/Claude/Gemini/Kimi/Cursor 等 parser，而是直接使用固定的 Tokscale Core。查看当前 Release 实际支持的客户端：

```bash
token-monitor clients
```

## GitHub 凭据解析顺序

1. `--token`
2. `TOKEN_MONITOR_GITHUB_TOKEN`
3. `GITHUB_TOKEN`
4. `GH_TOKEN`
5. 已登录的 `gh auth token`
6. 终端隐藏输入

GitHub credential 只保存在设备本地私有配置中，用于写自己的加密分支；不会放进 Pair Code、Dashboard URL 或 Web 页面。

## 后台负载

Token Monitor 没有常驻进程。

| 平台 | 调度方式 |
| --- | --- |
| Windows | Task Scheduler |
| macOS | `launchd` |
| Linux | `systemd --user`，失败时回退 cron |

默认每 15 分钟执行一次短进程。增量扫描会重扫最近两天，处理延迟写入和跨日边界；如果 Token、价格身份等真实 accounting 没变化，则直接跳过 GitHub 写入。

## Windows / Linux 安装修复

### Linux

v1.1 修复 `curl | sh` 之后当前 shell 找不到 `token-monitor` 的问题：

- 优先使用已经位于当前 PATH 的用户目录；
- 正确写入 `.zshrc` / `.bashrc` / `.profile`；
- 如果父 shell 当前仍无法看到新 PATH，安装器直接打印绝对路径 setup 命令；
- `systemd --user` 不可用时自动尝试 cron；
- systemd `%` specifier、空格路径与 cron shell quoting 均有回归测试。

### Windows

v1.1 安装器新增：

- Windows PowerShell 5.1 TLS 1.2 兼容；
- x64 / ARM64 更稳健的架构检测；
- Local App Data fallback；
- Current Process PATH + User PATH 同时处理；
- 绝对 executable setup 命令 fallback；
- Task Scheduler 对含空格路径的引用处理与原生 Windows 回归测试。

## 隐私边界

加密账本仅包含：

- 日期、设备标识；
- Client / Model / Provider / Route / Tier 标签；
- Input / Cache Read / Cache Write / Output / Reasoning 数值；
- 可加总的记录数；
- 订阅等价费用与 lower-bound 标识；
- 价格来源元数据。

**不会上传：**

- Prompt；
- Assistant 回复；
- Reasoning 文本；
- 源代码或项目内容；
- 项目路径；
- 完整 JSONL / SQLite Session；
- `auth.json`；
- API Key；
- GitHub Token。

## 命令

```text
token-monitor setup [--repo OWNER/REPO]
token-monitor join <PAIR_CODE>
token-monitor password
token-monitor invite
token-monitor sync [--full]
token-monitor status
token-monitor clients
token-monitor dashboard
token-monitor uninstall [--remove-remote] [--purge]
```

## 测试与 Release Gate

每一个最终提交都要求在真实 GitHub 托管 OS Runner 上覆盖：

- Linux x86_64
- Linux ARM64
- Windows x86_64
- Windows ARM64
- macOS Apple Silicon
- macOS Intel
- Rust `clippy`
- Dashboard JS / Analytics / Privacy regression
- 固定 `tokscale-core v4.14.0` 的完整 parser/scanner 回归套件

正式 Release 会再次在六个平台原生构建。**Release 发布完成之后**还有第二个六平台安装矩阵：通过公开的 Release 下载 URL 真正执行 `install.sh` / `install.ps1`、校验 SHA-256、执行安装后的二进制并跑 CLI smoke test。因此“编译成功”不等于最终“可安装发布成功”。

## 开发

```bash
git clone https://github.com/Atingaii/token-monitor.git
cd token-monitor
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets
node web/analytics.test.cjs
```

## 实现来源与许可证

Token Monitor 本身使用 MIT License。重要实现来源在 [`NOTICE`](NOTICE) 和 [`SOURCES.md`](SOURCES.md) 中逐项记录：

- [`junhoyeo/tokscale`](https://github.com/junhoyeo/tokscale) v4.14.0 —— 多客户端解析与 Token accounting（MIT）
- [`falyx6851-byte/codex-monitor`](https://github.com/falyx6851-byte/codex-monitor) —— Codex request/Tier evidence（MIT）
- [`farion1231/cc-switch`](https://github.com/farion1231/cc-switch) —— 计价行为与 models.dev 数据源（MIT）
- [`Wei-Shaw/sub2api`](https://github.com/Wei-Shaw/sub2api) —— 仅作为 GPT-5.6 价格/长上下文规则的独立交叉核验（LGPL-3.0；Token Monitor 不复制、不链接其源码）

安全问题见 [`SECURITY.md`](SECURITY.md)，贡献说明见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。
