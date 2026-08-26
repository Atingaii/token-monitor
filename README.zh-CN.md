<p align="center">
  <img src=".github/assets/hero.svg" alt="Token Monitor — 无服务器跨设备 AI Coding 用量分析" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/Atingaii/token-monitor/actions/workflows/rust-cli-ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Atingaii/token-monitor/rust-cli-ci.yml?branch=main&style=flat-square&label=CI"></a>
  <a href="https://github.com/Atingaii/token-monitor/releases"><img alt="Release" src="https://img.shields.io/github/v/release/Atingaii/token-monitor?style=flat-square&display_name=tag"></a>
  <img alt="Rust" src="https://img.shields.io/badge/runtime-Rust-000000?style=flat-square&logo=rust&logoColor=white">
  <img alt="Platforms" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-555?style=flat-square">
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-2f81f7?style=flat-square"></a>
</p>

<p align="center">
  <strong>一个轻量 CLI，连接所有设备、所有已支持 AI Coding 客户端，并在一个统一网页中查看用量。</strong>
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="#快速开始">快速开始</a> ·
  <a href="#web-dashboard">Dashboard</a> ·
  <a href="SOURCES.md">统计实现来源</a>
</p>

Token Monitor 是一个 **无服务器、跨设备的 AI Coding Token / 费用分析系统**。Windows、Linux、macOS 上只安装一个预编译 Rust 单文件程序；本地用量由固定版本的 Tokscale Core 负责解析和统计，设备只把规范化后的数字账本加密写入用户自己的 Token Monitor fork，再由中央 GitHub Pages Dashboard 在浏览器中本地解密并分析。

不需要 Electron、Node.js、Python、Docker、VPS、数据库、常驻 Hub，也不需要额外创建一个数据仓库。

## 为什么做这个项目

普通 Token Monitor 往往只能回答“这台电脑用了多少”。这个项目重点解决的是跨设备问题：

- Windows、Linux、Mac 一起用了多少 Token？
- 哪台设备、哪个 CLI、哪个模型、哪个 Provider 路由消耗最多？
- 同一个 GPT / Claude 模型到底走的是官方、Azure/AWS、OpenRouter 还是中转站？
- 这些 Token 按 API 价格折算大约值多少钱？
- 能否不运行常驻监控进程、不维护服务器，也完成长期统计？

## 设计原则

| 原则 | 实现 |
| --- | --- |
| **低负载** | 不运行常驻 daemon，由系统原生调度器短暂启动一次同步后退出。 |
| **跨平台** | Windows / Linux / macOS，x64 与 ARM64 均进入 CI / Release 矩阵。 |
| **成熟统计核心** | 客户端解析、Token 口径、去重、模型归一和通用计费使用固定的 **Tokscale v4.14.0**。 |
| **证据驱动 Provider** | 模型厂商与实际路由分开；看到 GPT 模型不等于“OpenAI 官方”。 |
| **隐私优先** | 只上传聚合数字和路由标签，并在上传前使用 AES-256-GCM 加密。 |
| **无需数据服务器** | 每台设备只使用用户 fork 中一个独立 `tm-ledger-*` 分支。 |
| **特殊统计失败即回退** | Codex Fast/Standard 只有与 Tokscale 当日总量严格对账成功才会展示。 |

## 架构

```text
 Windows / Linux / macOS
          │
          │ 本地会话文件 / 数据库
          ▼
     Tokscale Core v4.14.0
  解析 · 去重 · Token 口径
  模型归一 · 通用计费
          │
          ▼
      token-monitor
 Provider 路由证据 + 加密
          │
          │ AES-256-GCM snapshot
          ▼
 YOUR_NAME/token-monitor fork
 ├─ main                    正常项目源码
 ├─ tm-ledger-<device-A>    加密 ledger.json
 ├─ tm-ledger-<device-B>    加密 ledger.json
 └─ tm-ledger-<device-C>    加密 ledger.json
          │
          ▼
 Atingaii GitHub Pages Dashboard
 浏览器本地解密 + 筛选 + 图表
```

设备同步不会向 `main` 写统计数据。每台设备只控制自己的 `tm-ledger-<device-hash>` 分支，因此不同设备不会争抢同一个文件；设备分支通过新的无父 snapshot commit 强制移动，不会不断积累大量可见 telemetry commit。

## 主要能力

### 多客户端统计

Token Monitor 不重新实现各个 AI Coding 客户端的 Token parser，而是直接使用固定版本 Tokscale Core 暴露的客户端集合，包括 Codex、Claude Code、OpenCode、Gemini 相关数据源、Kimi、Cursor 相关数据源、DeepSeek Harness、Copilot 等，以及 Tokscale v4.14.0 当前支持的其他客户端。

安装后可以直接查看本版本实际支持的客户端：

```bash
token-monitor clients
```

统计来源、固定版本和边界见 [`SOURCES.md`](SOURCES.md)。

### Provider / 路由身份

账本把以下身份分开保存：

| 字段 | 含义 | 示例 |
| --- | --- | --- |
| `model` | 规范化模型 | `gpt-5.6-sol` |
| `upstreamVendor` | 模型所属厂商 | `openai` |
| `routeProvider` | 有证据时识别出的实际路由/计费方 | `azure-openai`、`aws-bedrock`、`openrouter`、`newapi` |
| `routeType` | 路由类型 | `official`、`cloud`、`aggregator`、`relay`、`self-hosted`、`unknown` |

核心规则是：**模型身份不等于路由身份**。如果日志只能证明模型属于 OpenAI，那么 `upstreamVendor` 可以是 `openai`，但 `routeProvider` 仍应是 `unknown`，不会为了展示好看而标成官方。

可区分的路由包括官方 API、Azure OpenAI、AWS Bedrock、Google Vertex、OpenRouter、New API / One API / LiteLLM / CLIProxyAPI 类中转、推理服务、自托管以及证据不足的未知路由。

### Codex Fast / Standard

Codex request-level Fast/Standard 增强只负责补充 Tokscale 目前没有直接暴露的 service-tier 维度，其解析和 tier-aware 计费逻辑来自 MIT 开源项目 `falyx6851-byte/codex-monitor` 的成熟实现思路。

它**不能修改 Codex 主统计口径**。只有当增强解析得到的当日 Token 总量和 message count 与 Tokscale canonical 结果完全一致时，才允许把当日 Codex 数据拆成 Fast / Standard；对账失败则整日回退到 Tokscale，不展示不可信 Tier 明细。

### API 等价费用

`costUsd` 表示 **API-equivalent cost（API 等价估值）**，不是 ChatGPT / Codex 订阅实际账单。

通用模型费用继续由 Tokscale 计价；Codex service-tier 价格只存在于隔离的 Tier 增强层。如果源日志缺少 cache-write 等实际计费字段，相关费用会标为**下限估算**，网页不会把它伪装成精确值。

## Web Dashboard

Dashboard 采用克制的后台分析界面，而不是大量渐变和卡片堆叠。

支持：

- 今日 / 7 天 / 30 天 / 本月 / 全部 / 自定义时间
- 按设备、客户端、模型、模型厂商、路由 Provider、路由类型、原始 Provider、Tier 联动筛选
- Total Tokens、Input、Cache Read、Cache Write、Output、Reasoning、Messages、API 等价费用
- 折线图、面积图、柱状图、堆叠柱、环形图、Treemap、表格
- CSV 导出
- Light / Dark
- 浏览器本地 AES-GCM 解密

用户自己的 fork **不需要单独配置 GitHub Pages**。中央 Dashboard 会读取 URL 指定 fork 中的加密 `tm-ledger-*` 分支。

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

安装脚本只下载对应平台预编译程序并校验 SHA-256，不安装 Rust、Node.js 或 Python。

### 2. 第一台设备

```bash
token-monitor setup
```

这一条命令会自动完成：

1. 从显式参数、环境变量或已登录的 `gh` 中取得 GitHub 写入凭据；
2. 查找当前账号的 `token-monitor` fork；没有时尝试自动 fork；
3. 生成 Dashboard 加密 key；
4. 执行首次全量扫描；
5. 写入第一份加密设备 snapshot；
6. 安装当前操作系统的低负载定时同步；
7. 输出 Dashboard 地址和下一台设备可以直接复制的 join 命令。

不需要再创建 `token-monitor-data` 之类的第二个仓库。

如果 fork 被改名或属于组织，可以使用高级参数：

```bash
token-monitor setup --repo OWNER/RENAMED_FORK
```

### 3. 添加其他设备

第一台机器会直接打印类似：

```bash
token-monitor join 'eyJ2ZXJzaW9uIjoyLC4uLn0'
```

在另一台 Windows / Linux / macOS 上原样粘贴即可。如果之后忘了配对命令：

```bash
token-monitor invite
```

Pair Code 只包含 fork 地址、Dashboard 解密 key 和同步周期，**不包含 GitHub Token**。

## GitHub 身份验证

凭据自动查找顺序：

1. `--token`
2. `TOKEN_MONITOR_GITHUB_TOKEN`
3. `GITHUB_TOKEN`
4. `GH_TOKEN`
5. 已登录的 `gh auth token`
6. 最后才隐藏输入一次 Token

GitHub 凭据只用于用户 fork 和加密设备分支写入，不会出现在 Pair Code 或 Dashboard URL 中。

## 低负载策略

Token Monitor 默认没有常驻进程。

| 平台 | 调度方式 |
| --- | --- |
| Windows | Task Scheduler |
| macOS | `launchd` |
| Linux | `systemd --user` timer，失败时回退 cron |

默认每 15 分钟执行一次短暂增量同步。增量扫描只重扫最近两天以覆盖延迟写入和跨日情况；如果统计数字没有变化，本次同步连 GitHub snapshot 写入都会跳过。

## 隐私模型

加密账本只包含规范化的聚合统计，例如：

- 日期与设备标识
- 客户端、模型、Provider/路由标签、可选 Tier
- Input / Cache / Output / Reasoning Token buckets
- message count
- API 等价费用，以及必要时的 lower-bound 标记

不会上传：

- Prompt / 模型回复
- reasoning 文本
- 源代码 / 项目内容
- 项目路径
- 完整 JSONL / SQLite Session 数据
- `auth.json`
- API Key / GitHub Token

账本上传前用随机 256-bit key 执行 AES-256-GCM 加密。Dashboard key 放在 URL fragment（`#key=...`）中，浏览器不会把 fragment 随 HTTP 请求发送给 GitHub。

## 常用命令

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

## 测试与发布门槛

CI 不是只在一台 Linux 上做编译，而是覆盖：

- Linux x86_64
- Linux ARM64
- Windows x86_64
- Windows ARM64
- macOS Intel
- macOS Apple Silicon
- Rust `clippy`
- Dashboard 语法、筛选/聚合、CSV、隐私回归测试
- 固定 Tokscale v4.14.0 的完整 parser/scanner 回归测试套件

Release workflow 对应发布同样六种 OS / 架构的单文件程序。

## 从源码构建

普通用户不需要 Rust。开发者可以：

```bash
cargo test --workspace --all-targets
cargo build --release --workspace
```

## 仓库结构

```text
rust-cli/     轻量采集、加密、GitHub 同步与系统调度
web/          静态分析 Dashboard
.github/      CI、Release、Pages workflow 与项目头图
SOURCES.md    核心统计实现来源、固定版本与边界
```

## 参与贡献

欢迎 Issue 和 PR。修改统计、Provider 归因、加密或发布链路之前，请先阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md)。统计核心优先复用成熟上游实现并保留来源与测试，不新增第二套未经验证的 parser / pricing 逻辑。

## 安全问题

凭据、加密账本或其他安全问题请参考 [`SECURITY.md`](SECURITY.md)。不要在公开 Issue 中提交真实 Token、密钥或包含隐私内容的会话文件。

## 上游与来源

本项目由 [Javis603/token-monitor](https://github.com/Javis603/token-monitor) 针对 serverless multi-device 场景重构；多客户端统计核心使用 [junhoyeo/tokscale](https://github.com/junhoyeo/tokscale) v4.14.0；Codex service-tier 增强参考 MIT 开源项目 [falyx6851-byte/codex-monitor](https://github.com/falyx6851-byte/codex-monitor)。

详细来源见 [`SOURCES.md`](SOURCES.md) 和 [`NOTICE`](NOTICE)。

## License

[MIT](LICENSE)，并按要求保留上游版权与许可证声明。
