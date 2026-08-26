# Token Monitor

一个 **无服务器、跨设备、CLI-only** 的 AI Coding Token / 费用分析工具。

Windows、Linux、macOS 上只安装一个 Rust 单文件可执行程序。它复用 Tokscale Core 扫描本地 AI coding 工具的会话统计，定期把**加密后的聚合账本**写入 GitHub；所有图表与筛选都在静态 GitHub Pages 中完成。

> 不需要 Electron、Node.js、Python、Docker、VPS、数据库或常驻 Hub。

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
Windows / Linux / macOS
        │
        │ local scan (Tokscale Core)
        ▼
 token-monitor sync
        │
        │ daily aggregate only
        │ AES-256-GCM
        ▼
GitHub public repository
  tm-ledger-<device-hash> branches
        │
        │ encrypted ledger.json
        ▼
GitHub Pages static dashboard
        │
        └─ browser-local decrypt + filter + chart
```

每台设备拥有独立的 `tm-ledger-*` 分支。每次同步用新的无父提交强制替换该设备分支，因此不会让主分支产生大量 telemetry commit，也不会让不同设备发生写冲突。

## 隐私边界

上传的统计行包含：日期、设备 ID/名称、客户端、模型、模型厂商、路由标签、Token buckets、消息数、Session 数、耗时和 API 等价费用。

不会上传：Prompt、模型回复、reasoning 文本、源代码、项目文件内容、项目路径、完整会话 JSONL、`auth.json`、API Key 或 GitHub Token。

账本在上传前用随机 256-bit dashboard key 进行 AES-256-GCM 加密。GitHub Pages 通过 URL fragment 中的 `#key=...` 在浏览器本地解密；fragment 不会随 HTTP 请求发送给 GitHub。

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

## 第一个设备

准备一个你有写权限的 **Public GitHub Repository**。账本本身是密文，因此仓库可以公开用于零服务器 Dashboard；GitHub Token 只需该仓库的 Contents read/write 权限。

```bash
token-monitor setup --repo YOUR_NAME/YOUR_REPO
```

Token 获取顺序：

1. `--token` 参数；
2. `TOKEN_MONITOR_GITHUB_TOKEN` / `GITHUB_TOKEN` / `GH_TOKEN`；
3. 已安装 `gh` 时自动读取 `gh auth token`；
4. 终端隐藏输入。

完成后会立即进行一次全量扫描并输出：

- Dashboard URL
- Join Code
- 当前设备快照分支
- 自动同步调度器状态

## 添加其他设备

在第一台设备执行：

```bash
token-monitor join
```

然后在 Windows / Linux / macOS 的另一台设备：

```bash
token-monitor setup --join '<JOIN_CODE>'
```

每台设备仍需要自己的 GitHub 写入凭据，但 Join Code 会携带仓库地址、dashboard 解密 key 和同步周期，不需要重复配置统计规则。

## 常用命令

```bash
token-monitor status
token-monitor sync
token-monitor sync --full
token-monitor clients
token-monitor dashboard
token-monitor join
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

整个页面是静态 HTML/CSS/JavaScript，无后端。

## 测试

CI 包含：

- Windows / macOS / Linux：Rust 单元测试、Release 编译、CLI smoke test。
- Web：JavaScript 语法检查和筛选/聚合/Provider/CSV 单测。
- Tokscale Core：固定 revision 的完整 parser/scanner 测试套件，用于覆盖其大量 AI coding 客户端。
- Release：Linux x86_64/aarch64、macOS x86_64/aarch64、Windows x86_64/aarch64 二进制构建。

## 上游与许可证

本项目由 `Javis603/token-monitor` fork 后针对 serverless cross-device 场景重构，并使用 `junhoyeo/tokscale` 的 Rust Core 作为多客户端扫描/定价底座。保留原项目 MIT License 与版权声明；Tokscale 依赖同样按其许可证使用。
