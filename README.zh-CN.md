<p align="center">
  <img src=".github/assets/hero.svg" alt="Token Monitor — 无服务器跨设备 AI Coding 用量统计" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/Atingaii/token-monitor/actions/workflows/rust-cli-ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Atingaii/token-monitor/rust-cli-ci.yml?branch=main&style=flat-square&label=CI"></a>
  <a href="https://github.com/Atingaii/token-monitor/releases"><img alt="Release" src="https://img.shields.io/github/v/release/Atingaii/token-monitor?style=flat-square&display_name=tag"></a>
  <img alt="Rust" src="https://img.shields.io/badge/runtime-Rust-000000?style=flat-square&logo=rust&logoColor=white">
  <img alt="Platforms" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-555?style=flat-square">
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-2f81f7?style=flat-square"></a>
</p>

<p align="center"><strong>一个轻量 CLI · 多设备 · 多客户端 · 一个无需登录的统计控制台</strong></p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="#快速开始">快速开始</a> ·
  <a href="https://atingaii.github.io/token-monitor/">在线 Dashboard</a> ·
  <a href="SOURCES.md">计费与实现来源</a>
</p>

Token Monitor 是一个面向 AI Coding 工具的**无服务器、跨设备用量统计系统**。它以单个预编译 Rust CLI 的形式运行在 Windows、Linux 和 macOS 上，使用固定版本的 Tokscale Core 读取本机用量，每台设备只维护一个无历史增长的 GitHub 快照分支，并通过 GitHub Pages 汇总展示严格脱敏后的统计数据。

不需要 Electron、Node.js、Python、Docker、VPS、数据库、常驻 Hub，也不需要额外的数据仓库。

> **隐私模式说明：** v1.0.1 起 Dashboard 按你的需求改为“任意浏览器直接打开”。`public.json` 因此是公开可读的**脱敏聚合统计**。Prompt、回复、Reasoning 文本、代码、项目路径、Session ID、凭据、主机名、本地设备名和原始设备 ID 均不会进入公开文件。若你连聚合统计本身也不希望公开，则不应使用这一公开 Dashboard 模式。

## 为什么做 Token Monitor

它主要解决单机统计工具不擅长回答的问题：

- 多台 Windows / Linux / Mac 一共消耗了多少 Tokens？
- 哪个设备、客户端、模型、路由提供商、路由类型、Tier 消耗最多？
- GPT / Claude 请求实际走的是官方、云厂商、OpenRouter，还是 New API / One API / LiteLLM 一类中转？
- 按**当前 API 价格**折算价值是多少？
- 对已准确识别 Codex Tier 的请求，按**套餐内含用量 / 旧计量口径**折算又是多少？
- 能否在不运行常驻服务的情况下自动同步？

## 核心原则

| 原则 | 实现 |
| --- | --- |
| **低开销** | 系统调度器定时启动一次 CLI，完成扫描后退出；空闲时常驻内存为 0。 |
| **跨平台** | Windows / Linux / macOS 均覆盖 x64 与 ARM64 的原生 CI / Release。 |
| **成熟核心优先** | 解析、去重、Token 语义、模型标准化和通用 API 计价直接使用 **Tokscale v4.14.0**。 |
| **路由证据优先** | 模型厂商和实际路由分开；看到 GPT 模型不等于“OpenAI 官方路由”。 |
| **Codex 严格对账** | Fast / Standard 拆分只有在每日 Token 总数和消息数都与 Tokscale 精确一致时才接受。 |
| **无需 Dashboard 密钥** | `public.json` 为脱敏聚合，任意浏览器打开网站即可读取。 |
| **明确公开边界** | 公开的是聚合统计，不是 Session 内容；兼容性的 `ledger.json` 继续 AES-GCM 加密。 |
| **不堆积遥测历史** | 每个 `tm-ledger-*` 分支都被替换为新的 root snapshot commit。 |

## 架构

```text
 Windows / Linux / macOS
          │
          │ 本地 AI Coding 用量文件 / 数据库
          ▼
     Tokscale Core v4.14.0
 解析 · 去重 · Token 语义
 模型标准化 · API 定价
          │
          ▼
      token-monitor
 路由证据 · Codex Tier
 脱敏 · 兼容加密
          │
          ▼
 YOUR_NAME/token-monitor fork
 ├─ main                    项目源码
 ├─ tm-ledger-<device-A>
 │   ├─ public.json         公开脱敏聚合
 │   └─ ledger.json         AES-GCM 加密兼容账本
 ├─ tm-ledger-<device-B>
 └─ ...
          │
          ▼
 https://atingaii.github.io/token-monitor/
 GitHub Pages · 无登录统计控制台
```

遥测数据从不写入 `main`。每台机器只更新自己的分支，因此多个设备不会争抢同一个数据文件。

## Dashboard

新版 Dashboard 按**新版 New API 管理后台**这类产品的视觉语言重做：白/灰主色、克制蓝色强调、轻边框、紧凑信息密度、清晰层级，不使用花哨的大渐变卡片。

包括：

- 桌面侧边栏一键折叠 / 展开，并记住折叠状态；
- 浅色 / 深色主题；
- 桌面、平板、移动端响应式布局；
- 今日 / 7 天 / 30 天 / 本月 / 全部 / 自定义时间范围；
- 设备、客户端、模型、模型厂商、路由提供商、路由类型、原始 Provider、Tier 联动筛选；
- 总 Tokens、输入、Cache Read、Cache Write、输出、Reasoning、消息数；
- **套餐额度等价费用**与**当前 API 等价费用**分开显示；
- 折线、面积、柱状、堆叠柱、环形、Treemap、表格；
- CSV 导出；
- 只显示匿名设备标签，不显示真实主机名。

中央站点：

**https://atingaii.github.io/token-monitor/**

查看其他用户的 fork 时使用 `?repo=OWNER/token-monitor`。

## 两种计费口径

之前只显示一个美元金额容易误导，因为 GPT-5.6 Sol 当前 API 促销价与套餐内含用量 / 旧计量口径不是同一个概念。

### 套餐额度等价费用

`planCostUsd` 是一个**套餐额度规划等价值**，不是账单。只有当 Codex 的 Standard / Fast Tier 被独立恢复，并且每日 Token + 消息数与 Tokscale 精确对账时才提供。

对于 GPT-5.6 Sol，OpenAI 官方说明后续促销降价**不会改变套餐内含用量、5 小时 / 每周限制以及旧版 credit meter**，因此这一口径保留 Sol 发布时 `$5 / $0.50 / $30` 的基础，并按适用的 Codex / Work Fast 计量倍率计算 Fast。

### 当前 API 等价费用

`costUsd` 表示按当前 API 风格价格折算的价值。通用模型价格直接来自 Tokscale `PricingService`；已对账的 GPT-5.6 Codex 请求使用当前 OpenAI Standard / Fast API 表。

这两个数字都不是你的 ChatGPT Pro / Plus 实际发票。缺失 cache-write 或不支持的模型 / Tier 不会被猜测，而会标成费用下限。

准确的实现来源与官方链接见 [`SOURCES.md`](SOURCES.md)。

## 多客户端统计

Token Monitor 不自己维护几十套客户端解析器，而是直接使用 Tokscale Core 暴露的客户端集合，包括 Codex、Claude Code、OpenCode、Gemini 系列、Kimi、Cursor 系列、Copilot 系列及 Tokscale v4.14.0 的其他客户端。

查看当前内置列表：

```bash
token-monitor clients
```

## Provider / Route 区分

| 字段 | 含义 | 示例 |
| --- | --- | --- |
| `model` | 标准化模型 | `gpt-5.6-sol` |
| `upstreamVendor` | 模型所属厂商 | `openai` |
| `routeProvider` | 有证据支持的实际路由 / 计费方 | `azure-openai`、`aws-bedrock`、`openrouter`、`newapi` |
| `routeType` | 路由类别 | `official`、`cloud`、`aggregator`、`relay`、`self-hosted`、`unknown` |
| `provider` | 来源中的原始 Provider 标识 | 依客户端而定 |
| `tier` | 可选服务等级 | `standard`、`fast` |

模型推断最多确定 `upstreamVendor`，不能单靠模型名认定 `routeType=official`。

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

安装脚本只下载对应平台的预编译二进制，并核验 SHA-256，不安装 Rust / Node / Python。

### 2. 第一台设备

```bash
token-monitor setup
```

它会自动：

1. 获取 GitHub 写入凭据；
2. 找到你的 `token-monitor` fork，没有时尝试自动 fork；
3. 全量扫描本地客户端；
4. 创建第一份无历史增长的设备快照；
5. 安装系统原生定时任务；
6. 输出无需密钥的 Dashboard 地址；
7. 输出另一台设备可直接粘贴的 `join` 命令。

fork 改名或属于组织时才需要：

```bash
token-monitor setup --repo OWNER/RENAMED_FORK
```

### 3. 添加其他设备

```bash
token-monitor join '<PAIR_CODE>'
```

需要重新查看加入命令时：

```bash
token-monitor invite
```

Pair Code **不包含 GitHub Token**。其中仍保留兼容账本所需的 AES 密钥，让多设备可以继续维护 `ledger.json`，但正常 Web Dashboard 已不再需要它。

## 从 v1.0.0 升级

v1.0.0 的设备分支只有加密的 `ledger.json`。升级到 v1.0.1 后，每台旧设备需要执行一次全量同步以生成 `public.json`：

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
token-monitor sync --full
token-monitor dashboard
```

内部 ledger schema 已提升到 v4，所以即使 Token 数没有变化，升级后的第一次同步也不会被“无变化跳过写入”逻辑吞掉。

完成后，任何浏览器都可以直接访问 Dashboard，不需要记住任何 key。

## 后台开销

Token Monitor 没有常驻进程。

| 平台 | 调度方式 |
| --- | --- |
| Windows | Task Scheduler |
| macOS | `launchd` |
| Linux | `systemd --user` timer，失败时回退 cron |

默认每 15 分钟执行一次增量同步，并重扫最近两天以覆盖延迟写入和日期边界。统计内容没有变化时不会产生新的 GitHub snapshot 写入。

## 隐私与安全

### 公开的 `public.json`

允许公开的只有：日期、匿名设备 hash/标签、平台/架构、客户端、模型、路由/Tier 标签、Token 各桶、消息数、费用估算及快照时间元数据。

明确不会公开：

- 本地设置的设备名称、hostname；
- 原始 device ID；
- Session ID / 完整 Session；
- Prompt / 回复；
- Reasoning 文本；
- 源代码 / 项目内容；
- 项目与 workspace 路径；
- `auth.json` 等认证文件；
- API Key、GitHub Token、Cookie、密码、Pair Code。

Rust 回归测试会序列化 `PublicLedger` 并检查真实设备名、hostname 和 raw device id 无法进入公开文件。

### 加密的 `ledger.json`

每个设备分支同时保留 AES-256-GCM 加密的兼容聚合账本，但正常 Dashboard 不再依赖它。

因为 GitHub Pages 是公开静态站，在前端加一个 JavaScript 密码并不能构成真实安全边界。如果你要求连聚合统计也必须私密，应改用带认证的私有后端，而不是给静态页面加“假密码”。详细边界见 [`SECURITY.md`](SECURITY.md)。

## 命令

```text
token-monitor setup [--repo OWNER/REPO]
token-monitor join <PAIR_CODE>
token-monitor invite
token-monitor sync [--full]
token-monitor status
token-monitor clients
token-monitor dashboard
token-monitor uninstall [--remove-remote] [--purge]
```

## 测试门槛

发布前 CI 覆盖：

- Linux x86_64 / ARM64；
- Windows x86_64 / ARM64；
- macOS Intel / Apple Silicon；
- Rust `clippy`；
- Dashboard JS 语法、聚合、CSV 与隐私回归；
- 固定 Tokscale v4.14.0 的完整 parser/scanner 回归套件。

## 从源码构建

```bash
cargo test --workspace --all-targets
cargo build --release --workspace
```

## 项目结构

```text
rust-cli/     采集、Tier、脱敏、GitHub 同步与系统调度
web/          GitHub Pages 无登录统计控制台
.github/      CI、Release、Pages 与项目头图
SOURCES.md    统计/计费实现来源
SECURITY.md   公开与私有数据边界
```

## 上游与署名

本项目是 [Javis603/token-monitor](https://github.com/Javis603/token-monitor) 的无服务器 CLI 方向重构；多客户端统计核心使用 [junhoyeo/tokscale](https://github.com/junhoyeo/tokscale) v4.14.0；Codex Tier 增强逻辑来自 MIT 许可的 [falyx6851-byte/codex-monitor](https://github.com/falyx6851-byte/codex-monitor)。

详细来源见 [`SOURCES.md`](SOURCES.md) 与 [`NOTICE`](NOTICE)。

## License

[MIT](LICENSE)。
