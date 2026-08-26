<p align="center">
  <img src=".github/assets/hero.svg" alt="Token Monitor" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/Atingaii/token-monitor/actions/workflows/rust-cli-ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Atingaii/token-monitor/rust-cli-ci.yml?branch=main&style=flat-square&label=CI"></a>
  <a href="https://github.com/Atingaii/token-monitor/releases"><img alt="Release" src="https://img.shields.io/github/v/release/Atingaii/token-monitor?style=flat-square&display_name=tag"></a>
  <img alt="Rust" src="https://img.shields.io/badge/runtime-Rust-000000?style=flat-square&logo=rust&logoColor=white">
  <img alt="Platforms" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-555?style=flat-square">
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-2f81f7?style=flat-square"></a>
</p>

<p align="center">
  <strong>一个轻量 CLI，把多台设备、多个 AI Coding 客户端的 Token、路由和费用聚合到同一个 Dashboard。</strong>
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="https://atingaii.github.io/token-monitor/">Dashboard</a> ·
  <a href="SOURCES.md">计费与解析来源</a> ·
  <a href="https://github.com/Atingaii/token-monitor/releases">Releases</a>
</p>

## Token Monitor 是什么

Token Monitor 是一个 **serverless、跨设备、CLI-only** 的 AI Coding 用量统计系统。

它在 Windows、Linux、macOS 上读取本机 AI Coding 客户端的用量记录，通过固定版本的 **Tokscale Core** 完成客户端解析、Token 语义归一、去重和模型标准化；再由 Token Monitor 补充路由证据、订阅等价计价和加密同步。每台设备只运行短暂的一次性同步任务，不存在常驻 Electron/Node/Python 服务。

最终所有设备的数据都汇总到同一个静态 Dashboard：

**https://atingaii.github.io/token-monitor/**

Dashboard 使用你自己的 fork 作为加密数据存储，不需要 VPS、数据库或额外的数据仓库。

## 核心特性

| 能力 | 实现 |
| --- | --- |
| 多客户端 | 直接复用 Tokscale v4.14.0 的成熟 parser/scanner，不重复造解析器 |
| 多设备 | Windows / Linux / macOS，x64 + ARM64 |
| 低负载 | OS 原生调度器每隔约 15 分钟启动一次 `sync --quiet`，执行完立即退出 |
| 私有聚合 | 设备账本 AES-256-GCM 加密后写入各自 `tm-ledger-*` 分支 |
| 好记的网址 | URL 不再携带长 `#key=...`；任意浏览器输入同一个 Dashboard 密码即可解锁 |
| 路由分析 | 模型厂商、路由供应商、路由类型、原始 Provider 分开记录 |
| 官方路由统一 | 已有证据确认是第一方官方时，路由供应商统一显示 **“官方”**；原始 provider 仍保留审计 |
| 成熟价格来源 | 通用价格跟随 CC Switch 使用的 models.dev；GPT-5.6 使用 CC Switch / Sub2API 交叉验证的订阅等价费率 |
| 精确图表 | 坐标轴自适应量级和边距；K/M/B/T 只用于可读刻度，hover/table 保留完整精确值 |
| 无变化不写 GitHub | 本地 accounting snapshot 未变化时不创建新的远端 snapshot |

## 架构

```text
Windows / Linux / macOS
        │
        │ local AI coding usage
        ▼
Tokscale Core v4.14.0
parser · dedup · token semantics · model normalization
        │
        ▼
Token Monitor
route evidence · subscription-equivalent pricing
        │
        ├── AES-256-GCM ledger
        │
        ▼
YOUR_NAME/token-monitor
├── main                       源码
├── tm-dashboard               access.json：密码包裹后的 workspace key
├── tm-ledger-<device-A>       加密 ledger.json
├── tm-ledger-<device-B>       加密 ledger.json
└── tm-ledger-<device-C>       加密 ledger.json
        │
        ▼
Atingaii GitHub Pages
输入密码 → 浏览器本地解 key → 浏览器本地解密账本 → 分析
```

设备同步不会把遥测写入 `main`。每个设备只 force-move 自己的无历史增长 snapshot branch，因此不会把周期同步变成主分支 commit 噪声。

## 安装

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
```

Linux v1.1+ Release 使用 **musl 静态二进制**，发布 CI 会明确拒绝带 glibc symbol-version 依赖的 Linux artifact。因此旧版 Debian / Ubuntu / CentOS 类系统不再要求与 GitHub runner 相同的新 glibc。

`curl | sh` 运行在子 shell 中，无法修改你当前父 shell 的 PATH。安装器会优先选择已经在 PATH 中的用户目录；若做不到，会直接打印一个当前终端可执行的绝对路径，例如：

```bash
'/home/you/.local/bin/token-monitor' setup
```

新终端会读取安装器写入的 shell profile。

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.ps1 | iex
```

支持 Windows PowerShell 5.1 和 PowerShell 7。安装器包含 TLS 1.2 兼容、x64/ARM64 原生架构检测、SHA-256 校验和 PATH fallback。

> **不要在 Windows PowerShell 里运行 `curl -fsSL ... | sh`。** Windows PowerShell 中的 `curl` 通常是 `Invoke-WebRequest` 的别名，并不支持 Unix curl 的 `-fsSL` 参数；Windows 必须使用上面的 `irm ... | iex` 命令。

如果当前 PowerShell 宿主没有立即继承新的用户 PATH，安装器会同时打印 `token-monitor.exe` 的绝对 `setup` 命令。

## 快速开始

### 第一台设备

```bash
token-monitor setup
```

`setup` 会：

1. 自动解析 GitHub 凭据；
2. 找到你的 `token-monitor` fork，必要时自动 fork；
3. 生成随机 256-bit workspace key；
4. 隐藏输入一个你容易记住的 Dashboard 密码；
5. 使用 PBKDF2-HMAC-SHA256 + AES-256-GCM 包裹 workspace key，并写入 `tm-dashboard/access.json`；
6. 执行完整本地扫描并上传首份加密账本；
7. 安装原生低负载调度器；
8. 输出 Dashboard 地址和新设备 join 命令。

Dashboard 地址稳定为：

```text
https://atingaii.github.io/token-monitor/?repo=YOUR_NAME/token-monitor
```

**不再包含随机 AES key。** 换浏览器、换电脑、无痕窗口都只需要输入同一个 Dashboard 密码。

### 添加其他设备

第一台设备会打印：

```bash
token-monitor join '<PAIR_CODE>'
```

把这条命令复制到另一台机器即可。Pair Code 包含仓库地址、workspace key 和同步间隔，不包含 GitHub Token。它仍属于敏感工作区凭据，不应公开。

以后重新查看 join 命令：

```bash
token-monitor invite
```

## Dashboard 密码

密码不是直接拿来加密全部账本，而是只负责包裹随机 workspace key：

```text
你的密码
  │ PBKDF2-HMAC-SHA256 / 310,000 iterations / random salt
  ▼
wrapping key
  │ AES-256-GCM
  ▼
随机 workspace key
  │ AES-256-GCM
  ▼
各设备 ledger.json
```

GitHub 只保存 salt、nonce、迭代次数和密文，不保存密码。浏览器每次输入密码后在本地完成解包，密码不会进入 URL，也不依赖 localStorage 才能跨浏览器使用。

纯静态架构无法阻止攻击者下载 `access.json` 后进行离线猜密码，因此建议使用较长、好记的 passphrase，而不是弱的 8 位数字。

修改密码：

```bash
token-monitor password
```

只会重新包裹同一个 workspace key，不需要重写各设备历史账本。

## 从 v1.0 升级到 v1.1

重新运行对应系统的安装命令覆盖旧二进制，然后在任意一台已经配置好的设备执行：

```bash
token-monitor password
token-monitor sync
```

v1.1 会发现本地 ledger schema/价格语义已经升级，**自动忽略旧缓存并执行一次 full rescan + 全历史重新计价**。因此不会出现“最近两天按新价格、更早历史仍按旧价格”的混合账本。第一次迁移完成后，后续同步自动恢复为两天 overlap 的增量扫描。

旧 `#key=...` 链接仍保留读取兼容，但新 CLI 不再输出这种链接。

## 价格口径

Dashboard 显示的是 **订阅等价费用（Subscription-equivalent cost）**，不是 OpenAI/Anthropic 的真实发票，也不是 ChatGPT/Codex 后台实际扣款。

### GPT-5.6

GPT-5.6 采用 CC Switch 当前内置价格，并由 Sub2API fallback 独立交叉核验。单位为 USD / 1M Tokens：

| 模型 | Input | Cache Read | Cache Write | Output |
| --- | ---: | ---: | ---: | ---: |
| GPT-5.6 Sol | **$5.00** | **$0.50** | **$6.25** | **$30.00** |
| GPT-5.6 Terra | $2.00 | $0.20 | $2.50 | $12.00 |
| GPT-5.6 Luna | $0.20 | $0.02 | $0.25 | $1.20 |

Fast / Priority 使用 GPT-5.6 明确的 **2×** tier 价格。长上下文 `>272K` 判断发生在**单次请求粒度**，随后才聚合，避免把一天几百万 Token 错当成一次长上下文请求。

例如：

```text
182,000 fresh input × $5/M
6,080,000 cache read × $0.50/M
12,000 output × $30/M
≈ $4.31
```

这就是为什么旧 API `$4/$20/$0.40` 口径约 `$3.40`，而 v1.1 的 CC Switch 兼容订阅等价口径约 `$4.31`。

### 其他模型

通用价格目录使用 CC Switch 同源的：

```text
https://models.dev/api.json
```

同一 normalized model 若存在多个 provider 条目，Token Monitor 会优先模型家族的 canonical provider，并使用稳定排序，避免 HashMap 遍历顺序导致不同机器取到不同价格。找不到完整价格的 Token bucket 会明确标为 lower bound，不会用相邻模型自行猜价。

详细来源、版本和许可证见 [`SOURCES.md`](SOURCES.md)。

## 路由语义

路由不会只靠模型名猜测。

| 字段 | 含义 | 示例 |
| --- | --- | --- |
| `model` | 标准化模型 | `gpt-5.6-sol` |
| `upstreamVendor` | 模型厂商 | `openai` |
| `routeProvider` | 实际路由供应商 | `official`, `azure-openai`, `openrouter`, `newapi` |
| `routeType` | 路由类别 | `official`, `cloud`, `aggregator`, `relay`, `unknown` |
| `provider` | 原始来源 provider | 保留用于审计 |

一旦明确证明是 OpenAI / Anthropic / Google 等第一方官方路由，`routeProvider` 统一规范为 `official`，中文 Dashboard 显示 **“官方”**。厂商信息仍然由 `upstreamVendor` 保留，所以不会丢失“这是 OpenAI 还是 Anthropic”的维度。

如果只是看到 GPT / Claude 模型、但没有第一方路由证据，则不会标记为官方。

## Dashboard 设计

v1.1 Dashboard 使用现代、克制的管理后台视觉语言：中性白/灰表面、轻边框、蓝色强调、紧凑筛选和可折叠侧边栏。设计参考当前 New API 的信息架构和视觉方向，但实现代码为 Token Monitor 自己编写，不复制其 AGPL 前端实现。

Dashboard 支持：

- 今日 / 7 天 / 30 天 / 本月 / 全部 / 自定义范围
- 设备、客户端、模型、模型厂商、路由供应商、路由类型、原始 Provider、Tier 联动筛选
- 折线、面积、柱状、堆叠柱、环形、Treemap、表格
- CSV 导出
- Light / Dark
- 桌面侧边栏折叠；移动端 drawer
- Y 轴 nice-scale、自适应左边距和 K/M/B/T 可读刻度
- X 轴按空间自动抽样，长标签自动旋转/省略
- 图表 tooltip 展示完整数值；表格 Token 使用完整千位分隔整数，费用显示到 4 位小数

## 后台负载

| 平台 | 调度器 |
| --- | --- |
| Windows | Task Scheduler |
| macOS | launchd |
| Linux | systemd --user timer；失败时回退 cron |

默认每 15 分钟执行一次。没有 Token Monitor 进程常驻内存。没有 accounting 变化时不会写 GitHub。

## 隐私边界

加密账本允许上传：

- 日期、设备标识
- client / model / provider / route / tier 标签
- input / cache read / cache write / output / reasoning Token
- 可加总记录数
- 订阅等价费用和 lower-bound 标志

不会上传：

- Prompt / 回复 / reasoning 文本
- 源代码和项目内容
- 项目路径
- 完整 JSONL / SQLite session
- `auth.json`
- API Key / GitHub Token

## CLI

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

## 发布门槛

正式 Release 不以“Linux 编译成功”作为完成标准。CI / Release 会覆盖：

- Linux x86_64 musl
- Linux ARM64 musl
- Windows x86_64
- Windows ARM64
- macOS Intel
- macOS Apple Silicon
- Rust clippy
- Dashboard JS / analytics / privacy regression
- Tokscale v4.14.0 完整 parser/scanner regression
- 发布后从 GitHub Release 真正执行安装器
- Windows PowerShell 7 + **Windows PowerShell 5.1** 安装 smoke test
- Linux ELF 静态链接 / GLIBC symbol-version 守卫

## 来源与许可证

Token Monitor 为 MIT License。项目会明确记录复用或适配的成熟实现来源：

- `Javis603/token-monitor`
- `junhoyeo/tokscale` v4.14.0
- `falyx6851-byte/codex-monitor`
- CC Switch 的 models.dev 定价同步思路和 GPT-5.6 参数参考
- Sub2API 的 LiteLLM 定价来源与 GPT-5.6 fallback 交叉核验

具体版本、职责边界与许可证见 [`NOTICE`](NOTICE) 和 [`SOURCES.md`](SOURCES.md)。

## Contributing

提交代码前请阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md) 和 [`SECURITY.md`](SECURITY.md)。对于 parser / token accounting，优先修复或升级成熟上游实现；不要在 Token Monitor 中复制一套未经验证的新 parser。
