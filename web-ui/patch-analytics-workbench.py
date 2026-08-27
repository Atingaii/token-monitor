from pathlib import Path
import re

path = Path("src/app.tsx")
text = path.read_text(encoding="utf-8")

usage_component = r'''const UsageAnalysisView: React.FC<Props> = ({ isDarkMode, records }) => {
  type Dimension = 'model' | 'device' | 'tool' | 'tier' | 'routeProvider';
  type Metric = 'totalTokens' | 'cost' | 'requestsCount';
  const [dimension, setDimension] = useState<Dimension>('model');
  const [metric, setMetric] = useState<Metric>('totalTokens');

  const summary = useMemo(() => {
    const totalTokens = sum(records, 'totalTokens');
    const input = sum(records, 'inputTokens');
    const cacheRead = sum(records, 'cacheReadTokens');
    const cacheWrite = sum(records, 'cacheWriteTokens');
    const output = sum(records, 'outputTokens') + sum(records, 'reasoningTokens');
    const requests = sum(records, 'requestsCount');
    const cost = sum(records, 'cost');
    const inputSide = input + cacheRead + cacheWrite;
    return {
      totalTokens,
      requests,
      cost,
      cacheHitRate: inputSide > 0 ? cacheRead / inputSide * 100 : 0,
      freshInputShare: inputSide > 0 ? input / inputSide * 100 : 0,
      outputInputRatio: inputSide > 0 ? output / inputSide * 100 : 0,
      avgTokensPerRequest: requests > 0 ? totalTokens / requests : 0,
      avgCostPerRequest: requests > 0 ? cost / requests : 0,
      equivalentCostPerMillion: totalTokens > 0 ? cost / totalTokens * 1_000_000 : 0,
    };
  }, [records]);

  const dimensionLabels: Record<Dimension, string> = {
    model: '模型',
    device: '设备',
    tool: '客户端',
    tier: '模式',
    routeProvider: '路由',
  };
  const metricLabels: Record<Metric, string> = {
    totalTokens: 'Tokens',
    cost: '订阅等价费用',
    requestsCount: '请求记录',
  };

  const grouped = useMemo(() => {
    const map = new Map<string, { name: string; totalTokens: number; cost: number; requestsCount: number }>();
    for (const row of records) {
      const name = String(row[dimension] || '未知');
      const item = map.get(name) || { name, totalTokens: 0, cost: 0, requestsCount: 0 };
      item.totalTokens += row.totalTokens;
      item.cost += row.cost;
      item.requestsCount += row.requestsCount;
      map.set(name, item);
    }
    return [...map.values()].sort((a, b) => Number(b[metric]) - Number(a[metric])).slice(0, 14);
  }, [records, dimension, metric]);

  const topShare = useMemo(() => {
    const total = grouped.reduce((acc, item) => acc + Number(item[metric]), 0);
    const top3 = grouped.slice(0, 3).reduce((acc, item) => acc + Number(item[metric]), 0);
    return total > 0 ? top3 / total * 100 : 0;
  }, [grouped, metric]);

  const matrix = useMemo(() => {
    const deviceTotals = new Map<string, number>();
    const modelTotals = new Map<string, number>();
    for (const row of records) {
      deviceTotals.set(row.device, (deviceTotals.get(row.device) || 0) + row.totalTokens);
      modelTotals.set(row.model, (modelTotals.get(row.model) || 0) + row.totalTokens);
    }
    const devices = [...deviceTotals.entries()].sort((a,b) => b[1]-a[1]).slice(0, 6).map(([name]) => name);
    const models = [...modelTotals.entries()].sort((a,b) => b[1]-a[1]).slice(0, 6).map(([name]) => name);
    const values = new Map<string, number>();
    let max = 1;
    for (const row of records) {
      if (!devices.includes(row.device) || !models.includes(row.model)) continue;
      const key = `${row.device}\u0000${row.model}`;
      const value = (values.get(key) || 0) + row.totalTokens;
      values.set(key, value);
      max = Math.max(max, value);
    }
    return { devices, models, values, max };
  }, [records]);

  const combinations = useMemo(() => {
    const map = new Map<string, { device: string; model: string; tool: string; tier: string; totalTokens: number; cost: number; requests: number; cacheRead: number; inputSide: number }>();
    for (const row of records) {
      const key = `${row.device}\u0000${row.model}\u0000${row.tool}\u0000${row.tier}`;
      const item = map.get(key) || { device: row.device, model: row.model, tool: row.tool, tier: row.tier, totalTokens: 0, cost: 0, requests: 0, cacheRead: 0, inputSide: 0 };
      item.totalTokens += row.totalTokens;
      item.cost += row.cost;
      item.requests += row.requestsCount;
      item.cacheRead += row.cacheReadTokens;
      item.inputSide += row.inputTokens + row.cacheReadTokens + row.cacheWriteTokens;
      map.set(key, item);
    }
    return [...map.values()].sort((a,b) => b.totalTokens - a.totalTokens).slice(0, 12);
  }, [records]);

  const compact = (value: number) => analysisCompact(Number(value || 0));
  const metricFormat = (value: number) => metric === 'cost' ? `$${Number(value).toFixed(value < 1 ? 3 : 2)}` : metric === 'requestsCount' ? Math.round(value).toLocaleString() : compact(value);
  const barTooltip = ({active, payload}: any) => {
    if (!active || !payload?.length) return null;
    const item = payload[0].payload;
    return <div className="rounded-xl border border-slate-800 bg-slate-950/95 p-3 text-xs text-white shadow-xl font-mono min-w-[220px]">
      <div className="mb-2 border-b border-slate-800 pb-1.5 text-slate-400 break-all">{item.name}</div>
      <div className="flex justify-between gap-4"><span>{metricLabels[metric]}</span><span className="font-bold text-blue-400">{metricFormat(Number(item[metric]))}</span></div>
      <div className="mt-1 flex justify-between gap-4 text-slate-400"><span>Tokens</span><span>{Math.round(item.totalTokens).toLocaleString()}</span></div>
      <div className="mt-1 flex justify-between gap-4 text-slate-400"><span>费用</span><span>${item.cost.toFixed(4)}</span></div>
    </div>;
  };

  const statCards = [
    { label: '缓存命中率', value: `${summary.cacheHitRate.toFixed(1)}%`, note: '输入侧缓存读取占比' },
    { label: '平均 Tokens / 请求', value: Math.round(summary.avgTokensPerRequest).toLocaleString(), note: `平均费用 $${summary.avgCostPerRequest.toFixed(3)}` },
    { label: '输出 / 输入侧', value: `${summary.outputInputRatio.toFixed(1)}%`, note: `新增输入占比 ${summary.freshInputShare.toFixed(1)}%` },
    { label: '等价费用 / 1M Tokens', value: `$${summary.equivalentCostPerMillion.toFixed(2)}`, note: '用于结构效率比较，不代表账单' },
  ];

  return <div className="space-y-4 transition-colors">
    <section className="rounded-xl border border-[var(--border-color)] bg-[var(--bg-card)] p-4 shadow-2xs">
      <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
        <div><div className="flex items-center gap-2"><Layers3 className="h-4 w-4 text-[var(--accent-blue)]" /><h2 className="text-base font-semibold text-[var(--text-primary)]">分析工作台</h2></div><p className="mt-1 text-xs text-[var(--text-muted)]">概览回答“用了多少”，这里用于定位“为什么高、由谁贡献、结构是否健康”。</p></div>
        <div className="rounded-lg border border-[var(--border-color)] bg-[var(--bg-main)] px-3 py-2 text-right"><div className="text-[10px] text-[var(--text-muted)]">TOP 3 集中度</div><div className="font-mono text-sm font-bold text-[var(--text-primary)]">{topShare.toFixed(1)}%</div></div>
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">{statCards.map(card => <div key={card.label} className="rounded-xl border border-[var(--border-color)] bg-[var(--bg-main)]/50 p-3.5"><div className="text-[11px] font-medium text-[var(--text-muted)]">{card.label}</div><div className="mt-1 font-mono-numbers text-xl font-bold text-[var(--text-primary)]">{card.value}</div><div className="mt-1 text-[10px] text-[var(--text-muted)]">{card.note}</div></div>)}</div>
    </section>

    <section className="overflow-hidden rounded-xl border border-[var(--border-color)] bg-[var(--bg-card)] shadow-2xs">
      <div className="flex flex-wrap items-end justify-between gap-3 border-b border-[var(--border-color)] px-4 py-3.5">
        <div><h3 className="text-sm font-semibold text-[var(--text-primary)]">结构贡献</h3><p className="mt-0.5 text-xs text-[var(--text-muted)]">切换维度与指标，找出当前筛选范围内的主要贡献者</p></div>
        <div className="flex flex-wrap items-end gap-2">
          <label className="text-[10px] text-[var(--text-muted)]">维度<select value={dimension} onChange={e => setDimension(e.target.value as Dimension)} className="mt-1 block h-8 rounded-lg border border-[var(--border-color)] bg-[var(--bg-main)] px-2.5 text-xs text-[var(--text-primary)] outline-none">{Object.entries(dimensionLabels).map(([key,label]) => <option key={key} value={key}>{label}</option>)}</select></label>
          <label className="text-[10px] text-[var(--text-muted)]">指标<select value={metric} onChange={e => setMetric(e.target.value as Metric)} className="mt-1 block h-8 rounded-lg border border-[var(--border-color)] bg-[var(--bg-main)] px-2.5 text-xs text-[var(--text-primary)] outline-none">{Object.entries(metricLabels).map(([key,label]) => <option key={key} value={key}>{label}</option>)}</select></label>
        </div>
      </div>
      <div className="h-[390px] w-full p-3 lg:p-4"><ResponsiveContainer width="100%" height="100%"><BarChart data={grouped} layout="vertical" margin={{top:4,right:28,left:12,bottom:4}}><CartesianGrid strokeDasharray="3 3" horizontal={false} stroke={isDarkMode ? '#262a33' : '#eef2f7'} /><XAxis type="number" tickFormatter={metricFormat} tick={{fontSize:10,fill:isDarkMode?'#9ca3af':'#667085'}} axisLine={false} tickLine={false} /><YAxis type="category" dataKey="name" width={120} tick={{fontSize:10,fill:isDarkMode?'#d1d5db':'#475467'}} axisLine={false} tickLine={false} tickFormatter={v => String(v).length > 18 ? `${String(v).slice(0,18)}…` : String(v)} /><Tooltip content={barTooltip} /><Bar dataKey={metric} fill="#2563eb" radius={[0,4,4,0]} /></BarChart></ResponsiveContainer></div>
      {!grouped.length && <div className="pb-8 text-center text-xs text-[var(--text-muted)]">当前筛选范围暂无可分析数据</div>}
    </section>

    <section className="overflow-hidden rounded-xl border border-[var(--border-color)] bg-[var(--bg-card)] shadow-2xs">
      <div className="border-b border-[var(--border-color)] px-4 py-3.5"><div className="flex items-center gap-2"><Monitor className="h-4 w-4 text-[var(--accent-blue)]" /><h3 className="text-sm font-semibold text-[var(--text-primary)]">设备 × 模型用量矩阵</h3></div><p className="mt-0.5 text-xs text-[var(--text-muted)]">快速识别某台机器是否集中消耗在某个模型上</p></div>
      <div className="overflow-x-auto p-4">
        {matrix.devices.length && matrix.models.length ? <div className="min-w-[760px]">
          <div className="grid gap-1.5" style={{gridTemplateColumns:`180px repeat(${matrix.models.length}, minmax(84px, 1fr))`}}><div />{matrix.models.map(model => <div key={model} className="truncate px-2 pb-1 text-center text-[10px] font-mono text-[var(--text-muted)]" title={model}>{model}</div>)}{matrix.devices.flatMap(device => [<div key={`${device}-label`} className="flex items-center truncate pr-3 text-xs font-medium text-[var(--text-primary)]" title={device}>{device}</div>, ...matrix.models.map(model => { const value = matrix.values.get(`${device}\u0000${model}`) || 0; const opacity = value <= 0 ? 0.04 : 0.12 + 0.78 * (value / matrix.max); return <div key={`${device}-${model}`} title={`${device} / ${model}: ${Math.round(value).toLocaleString()} Tokens`} className="rounded-md border border-blue-500/10 px-2 py-3 text-center font-mono text-[10px] text-[var(--text-primary)]" style={{backgroundColor:`rgba(37, 99, 235, ${opacity})`}}>{value ? compact(value) : '—'}</div>; })])}</div>
        </div> : <div className="py-10 text-center text-xs text-[var(--text-muted)]">当前筛选范围暂无矩阵数据</div>}
      </div>
    </section>

    <section className="overflow-hidden rounded-xl border border-[var(--border-color)] bg-[var(--bg-card)] shadow-2xs">
      <div className="border-b border-[var(--border-color)] px-4 py-3.5"><div className="flex items-center gap-2"><Table2 className="h-4 w-4 text-[var(--accent-blue)]" /><h3 className="text-sm font-semibold text-[var(--text-primary)]">高消耗组合</h3></div><p className="mt-0.5 text-xs text-[var(--text-muted)]">按“设备 + 模型 + 客户端 + 模式”聚合，便于定位异常来源</p></div>
      <div className="overflow-x-auto"><table className="w-full min-w-[900px] text-left text-xs"><thead className="border-b border-[var(--border-color)] bg-[var(--bg-main)] text-[10px] font-mono text-[var(--text-muted)]"><tr><th className="p-3">设备</th><th className="p-3">模型</th><th className="p-3">客户端</th><th className="p-3">模式</th><th className="p-3 text-right">Tokens</th><th className="p-3 text-right">费用</th><th className="p-3 text-right">请求</th><th className="p-3 text-right">缓存命中率</th></tr></thead><tbody className="divide-y divide-[var(--border-subtle)]">{combinations.map((item,index) => { const hit = item.inputSide > 0 ? item.cacheRead / item.inputSide * 100 : 0; return <tr key={`${item.device}-${item.model}-${item.tool}-${item.tier}-${index}`} className="hover:bg-[var(--bg-card-hover)]"><td className="p-3 font-medium text-[var(--text-primary)]">{item.device}</td><td className="p-3 font-mono text-[var(--text-primary)]">{item.model}</td><td className="p-3 text-[var(--text-secondary)]">{item.tool}</td><td className="p-3"><span className="rounded bg-blue-500/10 px-1.5 py-0.5 font-mono text-[10px] text-blue-600 dark:text-blue-400">{item.tier}</span></td><td className="p-3 text-right font-mono font-semibold">{Math.round(item.totalTokens).toLocaleString()}</td><td className="p-3 text-right font-mono text-emerald-600">${item.cost.toFixed(4)}</td><td className="p-3 text-right font-mono text-[var(--text-secondary)]">{Math.round(item.requests).toLocaleString()}</td><td className="p-3 text-right font-mono text-[var(--text-secondary)]">{hit.toFixed(1)}%</td></tr>; })}</tbody></table>{!combinations.length && <div className="py-10 text-center text-xs text-[var(--text-muted)]">暂无组合数据</div>}</div>
    </section>
  </div>;
};'''

pattern = re.compile(
    r"const UsageAnalysisView: React\.FC<Props> = \(\{ isDarkMode, records \}\) => \{.*?\n\};\n\nfunction ymd",
    re.DOTALL,
)
if not pattern.search(text):
    raise SystemExit("UsageAnalysisView block not found after base UI patch")
text = pattern.sub(usage_component + "\n\nfunction ymd", text, count=1)

for required in [
    "分析工作台",
    "结构贡献",
    "设备 × 模型用量矩阵",
    "高消耗组合",
    "TOP 3 集中度",
]:
    if required not in text:
        raise SystemExit(f"analytics workbench missing: {required}")

# The analytics page must not repeat the overview hero/summary composition.
for forbidden in [
    "真实消耗 Tokens",
    "查看当前筛选范围内的 Token 消耗、请求与成本统计",
]:
    # These strings may exist elsewhere in the app; only inspect the replaced block.
    block = pattern.search(text)
    if block and forbidden in block.group(0):
        raise SystemExit(f"overview-style analytics summary survived: {forbidden}")

path.write_text(text, encoding="utf-8")
