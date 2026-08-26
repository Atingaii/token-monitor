# Token Monitor

一个 **无服务器、跨设备、CLI-only** 的 AI Coding Token / 费用分析工具。

Windows、Linux、macOS 上只安装一个 Rust 单文件可执行程序。它复用 Tokscale Core 扫描本地 AI coding 工具的会话统计，定期把**加密后的聚合账本**写入用户自己的 Token Monitor fork；所有图表与筛选都在中央静态 GitHub Pages 中完成。

> 不需要 Electron、Node.js、Python、Docker、VPS、数据库、常驻 Hub，也不需要额外创建“数据仓库”。

## 主要特性

- Windows / Linux / macOS。
- 单个 Rust 二进制；用户机器无需安装 Rust。
- 不常驻：默认由系统调度器每 15 分钟执行一次短暂 `sync`，其余时间进程占用为 0。
- 继承 Tokscale Core 的多客户端解析能力，不局限于 Codex；包括 Codex、Claude Code、OpenCode、Gemini、Kimi、Cursor 相关数据源以及 Tokscale 当前支持的其他客户端。
- 按 **时间 / 设备 / 工具 / 模型 / 模型厂商 / 路由提供商 / 路由类型 / Provider / Tier** 分析。
- 路由身份区分：官方、AWS Bedrock、Azure OpenAI、Google Vertex、OpenRouter、New API / One API / LiteLLM / CLIProxyAPI 类中转、自托管及未知路由。
- Provider 采用保守证据规则：只有日志或 provider 配置能证明时才标为官方；只根据模型名推断出的 OpenAI/Anthropic 等不会被误标成官方渠道。
- Fast / Standard / Priority 等 Tier 只在源日志明确暴露时记录，不通过价格或延迟猜测。
- API-equivalent cost：用于估算同等 Token 若按 API 价格计费的价值，不代表订阅用户的实际账单。
- 静态 Web 分析页：折线图、面积图、柱状图、堆叠柱、环形图、Treemap、明细表格与 CSV 导出。
- 页面风格采用克制的后台控制台布局，不使用 Electron GUI。

## 架构

```text
                  Atingaii/token-monitor
                         │
                         │ fork once
                         ▼
                YOUR_NAME/token-monitor
                ┌──────────────────────┐
                │ main = 项目源码       │
                │                      │
                │ tm-ledger-mac-*      │ ← macOS
                │ tm-ledger-win-*      │ ← Windows
                │ tm-ledger-linux-*    │ ← Linux
                └──────────┬───────────┘
                           │
                   encrypted ledger.json
                           │
                           ▼
              Atingaii GitHub Pages Dashboard
                           │
                   browser-local decrypt
```

每台设备只拥有一个独立 `tm-ledger-*` 分支。同步不会修改 `main`，因此 fork 仍然保持正常的源码仓库结构；不同设备也不会争抢同一个文件。

每次同步使用新的无父 snapshot commit 强制移动当前设备分支，因此不会不断积累可见的 telemetry commit 历史。

## 隐私边界

上传的统计行包含：日期、设备 ID/名称、客户端、模型、模型厂商、路由标签、Token buckets、消息数、Session 数、耗时和 API 等价费用。

不会上传：Prompt、模型回复、reasoning 文本、源代码、项目文件内容、项目路径、完整会话 JSONL、`auth.json`、API Key 或 GitHub Token。

账本在上传前用随机 256-bit dashboard key 进行 AES-256-GCM 加密。Dashboard 通过 URL fragment 中的 `#key=...` 在浏览器本地解密；fragment 不会随 HTTP 请求发送给 GitHub。

## 安装

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.sh | sh
```

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/Atingaii/token-monitor/main/install.ps1 | iex
```

安装脚本只下载对应平台的预编译二进制，不安装 Node、Python 或 Rust。

## 使用：只记住两条命令

### 第一台设备

```bash
token-monitor setup
```

程序会自动：

1. 识别当前 GitHub 账号；
2. 查找 `YOUR_NAME/token-monitor` fork；
3. 已存在就直接使用；如果没有，尝试自动 fork `Atingaii/token-monitor`；
4. 生成 Dashboard 加密 key；
5. 全量扫描当前设备；
6. 上传第一个加密 snapshot；
7. 安装当前系统的低负载定时同步；
8. 输出 Dashboard 地址和下一台设备可直接复制的 `token-monitor join ...` 命令。

因此**不需要再创建 `token-monitor-data`、不需要输入 OWNER/REPO、也不需要配置本机 Pages**。

如果用户把自己的 fork 改了名字，才需要高级覆盖参数：

```bash
token-monitor setup --repo YOUR_NAME/RENAMED_FORK
```

### 第二、第三台及更多设备

第一台设备 setup 完成时已经会打印类似：

```bash
token-monitor join 'eyJ2ZXJzaW9uIjoyLC4uLn0'
```

在任何 Windows / Linux / macOS 新设备上**原样粘贴这一条命令即可**。

之后如果忘了配对命令，在任一已经连接的设备执行：

```bash
token-monitor invite
```

就会重新打印一条新的可复制命令。

Pair Code 只携带：

- 用户 fork 地址；
- Dashboard 解密 key；
- 同步周期。

它**不包含 GitHub Token**。每台设备会优先自动复用 `GITHUB_TOKEN` / `GH_TOKEN` / 已登录的 `gh auth token`；都没有时才在终端隐藏提示一次 GitHub Token。完成后不再要求输入。

## GitHub 身份验证

Token 获取顺序：

1. 命令行显式 `--token`（高级/自动化场景）；
2. `TOKEN_MONITOR_GITHUB_TOKEN` / `GITHUB_TOKEN` / `GH_TOKEN`；
3. 如果系统已经安装并登录 `gh`，自动读取 `gh auth token`；
4. 最后才在终端隐藏输入一次。

如果用户已经在开发机上使用 GitHub CLI，通常 `token-monitor setup` / `token-monitor join ...` 都不会再询问任何 GitHub 参数。

## 常用命令

```bash
token-monitor setup
token-monitor join '<PAIR_CODE>'
token-monitor invite
token-monitor status
token-monitor sync
token-monitor sync --full
token-monitor clients
token-monitor dashboard
token-monitor uninstall
token-monitor uninstall --remove-remote --purge
```

`token-monitor clients` 会列出当前嵌入 Tokscale Core 支持的全部客户端。

## 低负载策略

默认没有常驻 daemon：

- Windows：Task Scheduler
- macOS：launchd
- Linux：systemd user timer；不可用时回退到 cron

调度器只执行 `token-monitor sync --quiet`。增量同步仅重扫最近两天，用一个小重叠窗口修正延迟写入/跨日数据；首次安装或 `--full` 才扫描完整历史。

## Provider / Route Identity

数据模型同时保存：

- `upstreamVendor`：模型本身的厂商，例如 OpenAI / Anthropic / Google。
- `routeProvider`：实际路由或计费提供方，例如 `openai`、`aws-bedrock`、`azure-openai`、`openrouter`、`newapi`。
- `routeType`：`official`、`cloud`、`aggregator`、`relay`、`inference-provider`、`self-hosted`、`custom` 或 `unknown`。
- `provider`：源会话提供的原始 provider ID，便于审计。

例如同一个 Claude 模型可以分别显示为：

```text
claude-sonnet-4 / Anthropic / official
claude-sonnet-4 / AWS Bedrock / cloud
claude-sonnet-4 / OpenRouter / aggregator
```

同一个 GPT 模型也可以区分 OpenAI 官方、Azure、New API 中转和未知兼容端点。

## Web Dashboard

Dashboard 支持：

- 今日 / 7 天 / 30 天 / 本月 / 全部 / 自定义日期
- 设备、客户端、模型、模型厂商、路由提供商、路由类型、原始 Provider、Tier 联动筛选
- Total / Input / Cache Read / Cache Write / Output / Reasoning / Messages / Sessions / Duration / Cost 指标
- 折线、面积、柱状、堆叠柱、环形、Treemap、表格
- CSV 导出
- Light / Dark

整个页面是静态 HTML/CSS/JavaScript，无后端。用户自己的 fork 不需要单独开启 GitHub Pages；中央 Dashboard 根据 URL 中的 `?repo=YOUR_NAME/token-monitor` 读取该 fork 的加密设备分支。

## 测试

CI 包含：

- Windows / macOS / Linux：安装脚本语法、Rust 单元测试、Release 编译、CLI smoke test。
- Web：JavaScript 语法检查和筛选/聚合/Provider/CSV 单测。
- Tokscale Core：固定 revision 的完整 parser/scanner 测试套件，用于覆盖其大量 AI coding 客户端。
- Release：Linux x86_64/aarch64、macOS x86_64/aarch64、Windows x86_64/aarch64 二进制构建。

## 上游与许可证

本项目由 `Javis603/token-monitor` fork 后针对 serverless cross-device 场景重构，并使用 `junhoyeo/tokscale` 的 Rust Core 作为多客户端扫描/定价底座。保留原项目 MIT License 与版权声明；Tokscale 依赖同样按其许可证使用。
