(() => {
  'use strict';

  const A = window.TokenAnalytics;
  if (!A) throw new Error('analytics.js failed to load');

  const API = 'https://api.github.com';
  const RAW = 'https://raw.githubusercontent.com';
  const BRANCH_PREFIX = 'tm-ledger-';
  const PALETTE = [
    '#2563eb','#0f766e','#7c3aed','#c2410c','#be123c','#0369a1',
    '#4d7c0f','#9333ea','#475569','#b45309','#0891b2','#6d28d9'
  ];
  const METRICS = {
    totalTokens: ['总 Tokens', compact],
    planCostUsd: ['套餐额度等价费用', money],
    costUsd: ['当前 API 等价费用', money],
    input: ['输入 Tokens', compact],
    cacheRead: ['缓存读取', compact],
    cacheWrite: ['缓存写入', compact],
    output: ['输出 Tokens', compact],
    reasoning: ['Reasoning', compact],
    messages: ['消息数', integer]
  };
  const DIMENSIONS = {
    date: '日期',
    device: '设备',
    client: '工具',
    model: '模型',
    upstreamVendor: '模型厂商',
    routeProvider: '路由提供商',
    routeType: '路由类型',
    provider: '原始 Provider',
    tier: 'Tier'
  };

  const state = {
    repo: '',
    key: '', // Legacy v1.0.0 compatibility only. Public snapshots need no key.
    ledgers: [],
    rows: [],
    view: 'overview',
    range: '30d',
    customStart: '',
    customEnd: '',
    device: '*',
    client: '*',
    model: '*',
    upstreamVendor: '*',
    routeProvider: '*',
    routeType: '*',
    provider: '*',
    tier: '*',
    metric: 'totalTokens',
    overviewMetric: 'totalTokens',
    group: 'date',
    chart: 'line',
    stack: 'device'
  };

  const $ = id => document.getElementById(id);
  const ids = [
    'layout','sidebarToggle','themeToggle','themeLabel','appContent','dataNotice','syncState',
    'updatedAt','refreshButton','viewTitle','subtitle','rangeButtons','customRange','startDate','endDate',
    'deviceFilter','clientFilter','modelFilter','upstreamFilter','routeProviderFilter','routeTypeFilter',
    'providerFilter','tierFilter','tierFilterLabel','mTotal','mPlanCost','mPlanCostNote','mCost','mCostNote',
    'mInput','mCache','mOutput','mMessages','overviewView','analysisView','devicesView','dataView',
    'overviewMetric','overviewTrend','deviceBars','routeBars','modelBars','clientBars','metricSelect',
    'groupSelect','chartSelect','stackSelect','stackLabel','analysisChart','analysisTable','deviceTable',
    'rawTable','exportCsv'
  ];
  const els = Object.fromEntries(ids.map(id => [id, $(id)]));

  function compact(value) {
    const n = Number(value || 0);
    const abs = Math.abs(n);
    if (abs >= 1e12) return `${(n / 1e12).toFixed(abs >= 1e13 ? 1 : 2)}T`;
    if (abs >= 1e9) return `${(n / 1e9).toFixed(abs >= 1e10 ? 1 : 2)}B`;
    if (abs >= 1e6) return `${(n / 1e6).toFixed(abs >= 1e7 ? 1 : 2)}M`;
    if (abs >= 1e3) return `${(n / 1e3).toFixed(abs >= 1e4 ? 1 : 2)}K`;
    return Math.round(n).toLocaleString();
  }

  function integer(value) {
    return Math.round(Number(value || 0)).toLocaleString();
  }

  function money(value) {
    return `$${Number(value || 0).toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 4
    })}`;
  }

  function fmt(metric, value) {
    return (METRICS[metric]?.[1] || compact)(value);
  }

  function esc(value) {
    return String(value ?? '').replace(/[&<>"']/g, ch => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
    }[ch]));
  }

  function deriveRepo() {
    const param = new URLSearchParams(location.search).get('repo');
    if (param && /^[^/]+\/[^/]+$/.test(param)) return param;
    const host = location.hostname.toLowerCase();
    if (!host.endsWith('.github.io')) return '';
    const owner = host.slice(0, -'.github.io'.length);
    const first = location.pathname.split('/').filter(Boolean)[0];
    return first ? `${owner}/${first}` : `${owner}/${owner}.github.io`;
  }

  function fragmentKey() {
    return new URLSearchParams(location.hash.replace(/^#/, '')).get('key') || '';
  }

  function base64UrlBytes(value) {
    let text = String(value || '').replace(/-/g, '+').replace(/_/g, '/');
    while (text.length % 4) text += '=';
    return Uint8Array.from(atob(text), char => char.charCodeAt(0));
  }

  async function decryptEnvelope(envelope, encodedKey) {
    if (envelope?.kind !== 'token-monitor-encrypted-ledger' || Number(envelope?.schemaVersion) !== 2) {
      throw new Error('不支持的旧版加密账本格式');
    }
    const rawKey = base64UrlBytes(encodedKey);
    if (rawKey.length !== 32) throw new Error('旧版 Dashboard key 无效');
    const key = await crypto.subtle.importKey('raw', rawKey, 'AES-GCM', false, ['decrypt']);
    const plaintext = await crypto.subtle.decrypt({
      name: 'AES-GCM',
      iv: base64UrlBytes(envelope.nonce),
      additionalData: new TextEncoder().encode(`token-monitor-ledger-v2:${envelope.deviceHash}`)
    }, key, base64UrlBytes(envelope.ciphertext));
    return JSON.parse(new TextDecoder().decode(plaintext));
  }

  async function apiJson(path) {
    const response = await fetch(`${API}${path}`, {
      headers: { Accept: 'application/vnd.github+json' },
      cache: 'no-store'
    });
    if (!response.ok) throw new Error(`GitHub API ${response.status}`);
    return response.json();
  }

  async function fetchJson(url) {
    const response = await fetch(url, { cache: 'no-store' });
    if (!response.ok) return { response, json: null };
    return { response, json: await response.json() };
  }

  async function branchLedger(branch) {
    const publicUrl = `${RAW}/${state.repo}/${branch}/public.json`;
    const publicResult = await fetchJson(publicUrl);
    if (publicResult.response.ok) {
      const ledger = publicResult.json;
      if (ledger?.kind !== 'token-monitor-public-ledger' || Number(ledger?.schemaVersion) !== 1) {
        throw new Error(`${branch} 的 public.json 格式不受支持`);
      }
      return { ledger, mode: 'public' };
    }

    // Seamless transition for links generated by v1.0.0. Once the device writes
    // public.json this path is no longer used and the fragment is removed.
    if (publicResult.response.status === 404 && state.key) {
      const legacyUrl = `${RAW}/${state.repo}/${branch}/ledger.json`;
      const legacyResult = await fetchJson(legacyUrl);
      if (!legacyResult.response.ok) {
        throw new Error(`读取 ${branch} 失败 (${legacyResult.response.status})`);
      }
      return { ledger: await decryptEnvelope(legacyResult.json, state.key), mode: 'legacy' };
    }

    const error = new Error(
      publicResult.response.status === 404
        ? `${branch} 尚未生成 public.json`
        : `读取 ${branch} 失败 (${publicResult.response.status})`
    );
    error.code = publicResult.response.status === 404 ? 'upgrade-required' : 'read-failed';
    throw error;
  }

  function setSync(kind, text) {
    els.syncState.classList.remove('ok', 'loading', 'error');
    if (kind) els.syncState.classList.add(kind);
    const target = els.syncState.querySelector('.status-copy strong');
    if (target) target.textContent = text;
  }

  function showNotice(kind, text) {
    if (!text) {
      els.dataNotice.className = 'notice hidden';
      els.dataNotice.textContent = '';
      return;
    }
    els.dataNotice.className = `notice ${kind || ''}`.trim();
    els.dataNotice.textContent = text;
  }

  function clearData() {
    state.ledgers = [];
    state.rows = [];
    populateFilters();
    renderAll();
    els.updatedAt.textContent = '暂无设备快照';
  }

  async function loadAll() {
    if (!state.repo) throw new Error('无法判断数据仓库，请在网址添加 ?repo=OWNER/REPO');

    setSync('loading', '正在同步');
    showNotice('', '');
    const refs = await apiJson(`/repos/${state.repo}/git/matching-refs/heads/${BRANCH_PREFIX}`);
    const branches = refs
      .map(ref => String(ref.ref || '').replace(/^refs\/heads\//, ''))
      .filter(branch => branch.startsWith(BRANCH_PREFIX));

    if (!branches.length) {
      clearData();
      setSync('', '等待设备');
      showNotice('info', '还没有设备快照。先在任意设备运行 token-monitor setup，随后页面会自动出现聚合数据。');
      return;
    }

    const settled = await Promise.allSettled(branches.map(branchLedger));
    const loaded = settled.filter(item => item.status === 'fulfilled').map(item => item.value);
    const failures = settled.filter(item => item.status === 'rejected').map(item => item.reason);
    const ledgers = loaded.map(item => item.ledger);
    const legacyCount = loaded.filter(item => item.mode === 'legacy').length;
    const upgradeCount = failures.filter(error => error?.code === 'upgrade-required').length;

    if (!ledgers.length) {
      clearData();
      setSync('error', '设备待升级');
      if (upgradeCount) {
        showNotice('warning', `检测到 ${upgradeCount} 个旧版设备快照。请在这些设备升级 Token Monitor 后执行 token-monitor sync --full；随后任何浏览器都可直接查看，无需密钥。`);
        return;
      }
      throw failures[0] || new Error('没有可读取的设备快照');
    }

    state.ledgers = ledgers;
    state.rows = ledgers.flatMap(ledger => (ledger.rows || []).map(row => ({
      ...row,
      device: ledger.device?.id || 'unknown',
      deviceName: ledger.device?.name || ledger.device?.id || 'Unknown',
      platform: ledger.device?.platform || 'unknown',
      arch: ledger.device?.arch || '',
      updatedAt: ledger.generatedAt || ''
    })));

    populateFilters();
    renderAll();
    const latest = ledgers.map(ledger => ledger.generatedAt).filter(Boolean).sort().at(-1);
    els.updatedAt.textContent = latest
      ? `更新于 ${new Date(latest).toLocaleString()}`
      : '暂无设备快照';
    setSync('ok', `${ledgers.length} 台设备`);

    if (legacyCount) {
      showNotice('warning', `${legacyCount} 台设备仍使用 v1.0.0 加密兼容读取；升级并同步后将切换为无需密钥的公开脱敏聚合。`);
    } else if (failures.length) {
      showNotice('warning', `${ledgers.length} 台设备已载入，${failures.length} 台暂时读取失败。`);
    } else {
      showNotice('', '');
      if (location.hash) history.replaceState(null, '', `${location.pathname}${location.search}`);
    }
  }

  function unique(key, labelKey = key) {
    const map = new Map();
    for (const row of state.rows) {
      const value = row[key];
      if (value === undefined || value === null || value === '') continue;
      map.set(String(value), String(row[labelKey] ?? value));
    }
    return [...map.entries()].sort((a, b) => a[1].localeCompare(b[1]));
  }

  function fillSelect(select, entries, allLabel) {
    const old = select.value || '*';
    select.innerHTML = `<option value="*">${esc(allLabel)}</option>`
      + entries.map(([value, label]) => `<option value="${esc(value)}">${esc(label)}</option>`).join('');
    select.value = [...select.options].some(option => option.value === old) ? old : '*';
  }

  function populateFilters() {
    fillSelect(els.deviceFilter, unique('device', 'deviceName'), '全部设备');
    fillSelect(els.clientFilter, unique('client'), '全部工具');
    fillSelect(els.modelFilter, unique('model'), '全部模型');
    fillSelect(els.upstreamFilter, unique('upstreamVendor'), '全部厂商');
    fillSelect(els.routeProviderFilter, unique('routeProvider'), '全部路由');
    fillSelect(els.routeTypeFilter, unique('routeType'), '全部类型');
    fillSelect(els.providerFilter, unique('provider'), '全部 Provider');
    const tiers = unique('tier');
    fillSelect(els.tierFilter, tiers, '全部 Tier');
    els.tierFilterLabel.classList.toggle('hidden', !tiers.length);
  }

  function iso(date) {
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
  }

  function bounds() {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    let start = null;
    let end = iso(today);
    if (state.range === 'today') start = end;
    else if (state.range === '7d') {
      const date = new Date(today); date.setDate(date.getDate() - 6); start = iso(date);
    } else if (state.range === '30d') {
      const date = new Date(today); date.setDate(date.getDate() - 29); start = iso(date);
    } else if (state.range === 'month') {
      start = iso(new Date(today.getFullYear(), today.getMonth(), 1));
    } else if (state.range === 'all') {
      end = null;
    } else if (state.range === 'custom') {
      start = state.customStart || null;
      end = state.customEnd || end;
    }
    return { start, end };
  }

  function filtered() {
    return A.filterRows(state.rows, {
      ...bounds(),
      device: state.device,
      client: state.client,
      model: state.model,
      upstreamVendor: state.upstreamVendor,
      routeProvider: state.routeProvider,
      routeType: state.routeType,
      provider: state.provider,
      tier: state.tier
    });
  }

  function renderSummary(rows) {
    const sum = A.sumRows(rows);
    els.mTotal.textContent = compact(sum.totalTokens);
    els.mInput.textContent = compact(sum.input);
    els.mCache.textContent = compact(sum.cacheRead);
    els.mOutput.textContent = compact(sum.output);
    els.mMessages.textContent = integer(sum.messages);

    els.mCost.textContent = `${sum.costLowerBound ? '≥' : ''}${money(sum.costUsd)}`;
    els.mCostNote.textContent = sum.costLowerBound ? '含无法完整定价的下限项' : '当前公开 API 价格基准';
    els.mCost.title = '当前 API 等价费用，仅作估算，不代表订阅实际账单。';

    if (sum.planCostAvailable) {
      els.mPlanCost.textContent = `${sum.costLowerBound ? '≥' : ''}${money(sum.planCostUsd)}`;
      els.mPlanCostNote.textContent = sum.planCostIncomplete
        ? '部分 Codex 未能完成 Tier 对账'
        : '套餐 / 旧计量基准';
    } else {
      els.mPlanCost.textContent = '—';
      els.mPlanCostNote.textContent = rows.some(row => row.client === 'codex')
        ? '当前筛选的 Codex Tier 尚未完成对账'
        : '仅对可识别 Codex Tier 提供';
    }
    els.mPlanCost.title = '用于比较套餐额度消耗的计划估算，不是发票金额。';
  }

  function renderRank(container, rows, dimension, metric = 'totalTokens', limit = 8) {
    const items = A.groupRows(rows, dimension, metric).filter(item => item.value > 0).slice(0, limit);
    const max = Math.max(1, ...items.map(item => item.value));
    container.innerHTML = items.length
      ? items.map(item => `
        <div class="rank-row">
          <span class="rank-name" title="${esc(item.key)}">${esc(item.key)}</span>
          <span class="rank-track"><span class="rank-fill" style="width:${Math.max(1, item.value / max * 100)}%"></span></span>
          <span class="rank-value">${esc(fmt(metric, item.value))}</span>
        </div>`).join('')
      : '<div class="empty">当前筛选无数据</div>';
  }

  const NS = 'http://www.w3.org/2000/svg';
  function node(tag, attrs = {}) {
    const element = document.createElementNS(NS, tag);
    Object.entries(attrs).forEach(([key, value]) => element.setAttribute(key, String(value)));
    return element;
  }

  function tooltip() {
    let element = document.querySelector('.tooltip');
    if (!element) {
      element = document.createElement('div');
      element.className = 'tooltip hidden';
      document.body.appendChild(element);
    }
    return element;
  }

  function attachTip(element, text) {
    const tip = tooltip();
    element.addEventListener('pointermove', event => {
      tip.textContent = text;
      tip.style.left = `${event.clientX + 12}px`;
      tip.style.top = `${event.clientY + 12}px`;
      tip.classList.remove('hidden');
    });
    element.addEventListener('pointerleave', () => tip.classList.add('hidden'));
  }

  function frame(container) {
    container.innerHTML = '';
    const width = Math.max(480, container.clientWidth - 32);
    const height = Math.max(240, container.clientHeight - 32);
    const root = node('svg', { viewBox: `0 0 ${width} ${height}`, preserveAspectRatio: 'none' });
    container.appendChild(root);
    return { root, width, height, left: 58, right: 16, top: 14, bottom: 36 };
  }

  function axes(layout, labels, max, metric) {
    const innerWidth = layout.width - layout.left - layout.right;
    const innerHeight = layout.height - layout.top - layout.bottom;
    for (let index = 0; index <= 4; index++) {
      const y = layout.top + innerHeight * index / 4;
      layout.root.appendChild(node('line', {
        x1: layout.left, y1: y, x2: layout.width - layout.right, y2: y, class: 'grid-line'
      }));
      const text = node('text', {
        x: layout.left - 8, y: y + 4, 'text-anchor': 'end', class: 'axis-label'
      });
      text.textContent = fmt(metric, max * (1 - index / 4));
      layout.root.appendChild(text);
    }
    const step = Math.max(1, Math.ceil(labels.length / 8));
    labels.forEach((label, index) => {
      if (index % step && index !== labels.length - 1) return;
      const x = labels.length < 2
        ? layout.left + innerWidth / 2
        : layout.left + innerWidth * index / (labels.length - 1);
      const text = node('text', { x, y: layout.height - 10, 'text-anchor': 'middle', class: 'axis-label' });
      const value = String(label);
      text.textContent = value.length > 14 ? `${value.slice(0, 12)}…` : value;
      layout.root.appendChild(text);
    });
    return { innerWidth, innerHeight };
  }

  function lineChart(container, items, metric, area = false) {
    if (!items.length || !items.some(item => item.value > 0)) {
      container.innerHTML = '<div class="empty">当前筛选无数据</div>';
      return;
    }
    const data = [...items].sort((a, b) => a.key.localeCompare(b.key));
    const layout = frame(container);
    const max = Math.max(1e-12, ...data.map(item => item.value));
    const { innerWidth, innerHeight } = axes(layout, data.map(item => item.key), max, metric);
    const points = data.map((item, index) => ({
      x: data.length < 2 ? layout.left + innerWidth / 2 : layout.left + innerWidth * index / (data.length - 1),
      y: layout.top + innerHeight * (1 - item.value / max),
      item
    }));
    const path = points.map((point, index) => `${index ? 'L' : 'M'}${point.x},${point.y}`).join(' ');
    if (area) {
      const baseline = layout.top + innerHeight;
      layout.root.appendChild(node('path', {
        d: `${path} L${points.at(-1).x},${baseline} L${points[0].x},${baseline} Z`, class: 'chart-area'
      }));
    }
    layout.root.appendChild(node('path', { d: path, class: 'chart-line' }));
    points.forEach(point => {
      const circle = node('circle', { cx: point.x, cy: point.y, r: 3.2, fill: 'var(--accent)' });
      attachTip(circle, `${point.item.key} · ${fmt(metric, point.item.value)}`);
      layout.root.appendChild(circle);
    });
  }

  function barChart(container, items, metric) {
    const data = items.filter(item => item.value > 0).slice(0, 35).reverse();
    if (!data.length) {
      container.innerHTML = '<div class="empty">当前筛选无数据</div>';
      return;
    }
    const layout = frame(container);
    const innerWidth = layout.width - layout.left - layout.right;
    const innerHeight = layout.height - layout.top - layout.bottom;
    const max = Math.max(1e-12, ...data.map(item => item.value));
    const gap = 4;
    const barHeight = Math.max(3, (innerHeight - gap * (data.length - 1)) / data.length);
    data.forEach((item, index) => {
      const y = layout.top + index * (barHeight + gap);
      const rect = node('rect', {
        x: layout.left, y, width: Math.max(1, innerWidth * item.value / max),
        height: barHeight, rx: 2, class: 'chart-bar'
      });
      attachTip(rect, `${item.key} · ${fmt(metric, item.value)}`);
      layout.root.appendChild(rect);
      if (data.length <= 16) {
        const text = node('text', {
          x: layout.left - 8, y: y + barHeight / 2 + 4, 'text-anchor': 'end', class: 'axis-label'
        });
        text.textContent = item.key.length > 12 ? `${item.key.slice(0, 10)}…` : item.key;
        layout.root.appendChild(text);
      }
    });
  }

  function renderLegend(container, labels) {
    const legend = document.createElement('div');
    legend.className = 'legend';
    legend.innerHTML = labels.slice(0, 12).map((label, index) => `
      <span class="legend-item"><i class="legend-dot" style="background:${PALETTE[index % PALETTE.length]}"></i>${esc(label)}</span>
    `).join('');
    container.appendChild(legend);
  }

  function stackedChart(container, rows, primary, secondary, metric) {
    if (primary === secondary) secondary = primary === 'device' ? 'client' : 'device';
    const matrix = A.groupMatrix(rows, primary, secondary, metric);
    if (!matrix.xValues.length) {
      container.innerHTML = '<div class="empty">当前筛选无数据</div>';
      return;
    }
    const xValues = matrix.xValues.slice(primary === 'date' ? -45 : 0);
    const stacks = matrix.stacks.slice(0, 12);
    const totals = xValues.map(x => stacks.reduce((sum, stack) => sum + matrix.value(x, stack), 0));
    if (!totals.some(Boolean)) {
      container.innerHTML = '<div class="empty">当前筛选无数据</div>';
      return;
    }
    const layout = frame(container);
    const max = Math.max(1e-12, ...totals);
    const { innerWidth, innerHeight } = axes(layout, xValues, max, metric);
    const slot = innerWidth / Math.max(1, xValues.length);
    const barWidth = Math.max(2, slot * .72);
    xValues.forEach((x, xIndex) => {
      let cumulative = 0;
      stacks.forEach((stack, stackIndex) => {
        const value = matrix.value(x, stack);
        if (!value) return;
        const height = innerHeight * value / max;
        const y = layout.top + innerHeight - innerHeight * (cumulative + value) / max;
        const rect = node('rect', {
          x: layout.left + xIndex * slot + (slot - barWidth) / 2,
          y, width: barWidth, height: Math.max(.8, height),
          fill: PALETTE[stackIndex % PALETTE.length], class: 'chart-segment'
        });
        attachTip(rect, `${x} · ${stack} · ${fmt(metric, value)}`);
        layout.root.appendChild(rect);
        cumulative += value;
      });
    });
    renderLegend(container, stacks);
  }

  function donutChart(container, items, metric) {
    const data = items.filter(item => item.value > 0).slice(0, 12);
    const total = data.reduce((sum, item) => sum + item.value, 0);
    if (!total) {
      container.innerHTML = '<div class="empty">当前筛选无数据</div>';
      return;
    }
    const layout = frame(container);
    const cx = layout.width / 2, cy = layout.height / 2;
    const radius = Math.min(layout.width, layout.height) * .31;
    const strokeWidth = radius * .36;
    const circumference = 2 * Math.PI * radius;
    let offset = 0;
    data.forEach((item, index) => {
      const length = circumference * item.value / total;
      const circle = node('circle', {
        cx, cy, r: radius, fill: 'none', stroke: PALETTE[index % PALETTE.length],
        'stroke-width': strokeWidth,
        'stroke-dasharray': `${length} ${circumference - length}`,
        'stroke-dashoffset': -offset,
        transform: `rotate(-90 ${cx} ${cy})`
      });
      attachTip(circle, `${item.key} · ${fmt(metric, item.value)} · ${(item.value / total * 100).toFixed(1)}%`);
      layout.root.appendChild(circle);
      offset += length;
    });
    const center = node('text', { x: cx, y: cy + 6, 'text-anchor': 'middle', class: 'donut-center' });
    center.textContent = fmt(metric, total);
    layout.root.appendChild(center);
    renderLegend(container, data.map(item => item.key));
  }

  function treemapChart(container, items, metric) {
    const data = items.filter(item => item.value > 0).slice(0, 25);
    if (!data.length) {
      container.innerHTML = '<div class="empty">当前筛选无数据</div>';
      return;
    }
    const layout = frame(container);
    const rects = A.squarify(data, 5, 5, layout.width - 10, layout.height - 10);
    rects.forEach((item, index) => {
      const rect = node('rect', {
        x: item.x, y: item.y, width: Math.max(0, item.width - 2),
        height: Math.max(0, item.height - 2), rx: 4, fill: PALETTE[index % PALETTE.length]
      });
      attachTip(rect, `${item.key} · ${fmt(metric, item.value)}`);
      layout.root.appendChild(rect);
      if (item.width > 75 && item.height > 30) {
        const text = node('text', { x: item.x + 8, y: item.y + 18, class: 'treemap-label' });
        text.textContent = item.key.length > 18 ? `${item.key.slice(0, 16)}…` : item.key;
        layout.root.appendChild(text);
      }
    });
  }

  function tableHtml(headers, rows) {
    return `<table class="data-table"><thead><tr>${headers.map(header =>
      `<th class="${header.number ? 'number' : ''}">${esc(header.label)}</th>`
    ).join('')}</tr></thead><tbody>${rows.map(row =>
      `<tr>${headers.map(header =>
        `<td class="${header.number ? 'number' : ''}">${header.render ? header.render(row[header.key], row) : esc(row[header.key])}</td>`
      ).join('')}</tr>`
    ).join('')}</tbody></table>`;
  }

  function aggregateTable(rows, dimension, metric) {
    const items = A.groupRows(rows, dimension, metric);
    els.analysisTable.innerHTML = tableHtml([
      { key: 'key', label: DIMENSIONS[dimension] || dimension },
      { key: 'value', label: METRICS[metric]?.[0] || metric, number: true, render: value => esc(fmt(metric, value)) }
    ], items);
  }

  function renderAnalysis(rows) {
    els.stackLabel.classList.toggle('hidden', state.chart !== 'stacked');
    els.analysisTable.classList.toggle('hidden', state.chart !== 'table');
    els.analysisChart.classList.toggle('hidden', state.chart === 'table');
    const items = A.groupRows(rows, state.group, state.metric);
    els.analysisChart.title = ['costUsd','planCostUsd'].includes(state.metric) && rows.some(row => row.costLowerBound)
      ? '当前筛选包含费用下限估算。' : '';

    if (state.chart === 'line') lineChart(els.analysisChart, items, state.metric, false);
    else if (state.chart === 'area') lineChart(els.analysisChart, items, state.metric, true);
    else if (state.chart === 'bar') barChart(els.analysisChart, items, state.metric);
    else if (state.chart === 'stacked') stackedChart(els.analysisChart, rows, state.group, state.stack, state.metric);
    else if (state.chart === 'donut') donutChart(els.analysisChart, items, state.metric);
    else if (state.chart === 'treemap') treemapChart(els.analysisChart, items, state.metric);
    else aggregateTable(rows, state.group, state.metric);
  }

  function renderDevices(rows) {
    const entries = state.ledgers.map(ledger => {
      const id = ledger.device?.id || 'unknown';
      const sum = A.sumRows(rows.filter(row => row.device === id));
      return {
        name: ledger.device?.name || id,
        platform: ledger.device?.platform || 'unknown',
        arch: ledger.device?.arch || '',
        tokens: sum.totalTokens,
        planCost: sum.planCostUsd,
        planAvailable: sum.planCostAvailable,
        apiCost: sum.costUsd,
        lowerBound: sum.costLowerBound,
        messages: sum.messages,
        updatedAt: ledger.generatedAt || '',
        scanMs: ledger.scanMs || 0
      };
    }).sort((a, b) => b.tokens - a.tokens);

    els.deviceTable.innerHTML = tableHtml([
      { key: 'name', label: '匿名设备' },
      { key: 'platform', label: '平台' },
      { key: 'arch', label: '架构' },
      { key: 'tokens', label: 'Tokens', number: true, render: value => esc(compact(value)) },
      { key: 'planCost', label: '套餐等价', number: true, render: (value, row) => esc(row.planAvailable ? `${row.lowerBound ? '≥' : ''}${money(value)}` : '—') },
      { key: 'apiCost', label: 'API 等价', number: true, render: (value, row) => esc(`${row.lowerBound ? '≥' : ''}${money(value)}`) },
      { key: 'messages', label: '消息', number: true, render: value => esc(integer(value)) },
      { key: 'updatedAt', label: '数据时间', render: value => esc(value ? new Date(value).toLocaleString() : '—') },
      { key: 'scanMs', label: '扫描耗时', number: true, render: value => esc(`${integer(value)} ms`) }
    ], entries);
  }

  function renderRaw(rows) {
    const data = [...rows]
      .sort((a, b) => b.date.localeCompare(a.date) || a.deviceName.localeCompare(b.deviceName))
      .slice(0, 3000);
    els.rawTable.innerHTML = tableHtml([
      { key: 'date', label: '日期' },
      { key: 'deviceName', label: '匿名设备' },
      { key: 'client', label: '工具' },
      { key: 'upstreamVendor', label: '模型厂商' },
      { key: 'routeProvider', label: '路由提供商' },
      { key: 'routeType', label: '路由类型' },
      { key: 'model', label: '模型' },
      { key: 'tier', label: 'Tier' },
      { key: 'input', label: 'Input', number: true, render: value => esc(compact(value)) },
      { key: 'cacheRead', label: 'Cache R', number: true, render: value => esc(compact(value)) },
      { key: 'cacheWrite', label: 'Cache W', number: true, render: value => esc(compact(value)) },
      { key: 'output', label: 'Output', number: true, render: value => esc(compact(value)) },
      { key: 'reasoning', label: 'Reasoning', number: true, render: value => esc(compact(value)) },
      { key: 'messages', label: '消息', number: true, render: value => esc(integer(value)) },
      { key: 'planCostUsd', label: '套餐等价', number: true, render: (value, row) => esc(row.planCostAvailable ? `${row.costLowerBound ? '≥' : ''}${money(value)}` : '—') },
      { key: 'costUsd', label: 'API 等价', number: true, render: (value, row) => esc(`${row.costLowerBound ? '≥' : ''}${money(value)}`) }
    ], data);
  }

  function renderAll() {
    const rows = filtered();
    renderSummary(rows);
    if (state.view === 'overview') {
      lineChart(els.overviewTrend, A.groupRows(rows, 'date', state.overviewMetric), state.overviewMetric, true);
      renderRank(els.deviceBars, rows, 'device');
      renderRank(els.routeBars, rows, 'routeProvider');
      renderRank(els.modelBars, rows, 'model');
      renderRank(els.clientBars, rows, 'client');
    } else if (state.view === 'analysis') {
      renderAnalysis(rows);
    } else if (state.view === 'devices') {
      renderDevices(rows);
    } else if (state.view === 'data') {
      renderRaw(rows);
    }
  }

  function selectView(view) {
    state.view = view;
    document.querySelectorAll('.nav-item').forEach(button => {
      button.classList.toggle('active', button.dataset.view === view);
    });
    ['overview','analysis','devices','data'].forEach(name => {
      els[`${name}View`].classList.toggle('hidden', name !== view);
    });
    const copy = {
      overview: ['概览', '跨设备 AI Coding Token、模型路由与双口径费用统计'],
      analysis: ['用量分析', '按时间、设备、工具、模型、路由和 Tier 交叉分析'],
      devices: ['设备', '匿名设备同步状态与累计用量'],
      data: ['公开聚合数据', '浏览和导出严格脱敏后的统计行']
    }[view] || ['Token Monitor', ''];
    els.viewTitle.textContent = copy[0];
    els.subtitle.textContent = copy[1];
    renderAll();
  }

  function metricOptions(select, selected) {
    select.innerHTML = Object.entries(METRICS)
      .map(([key, [label]]) => `<option value="${key}">${esc(label)}</option>`)
      .join('');
    select.value = selected;
  }

  function updateThemeLabel() {
    const dark = document.documentElement.dataset.theme === 'dark';
    els.themeLabel.textContent = dark ? '浅色模式' : '深色模式';
  }

  function applySidebarPreference() {
    const collapsed = localStorage.getItem('tm-sidebar') === 'collapsed';
    els.layout.classList.toggle('sidebar-collapsed', collapsed);
    els.sidebarToggle.title = collapsed ? '展开侧边栏' : '收起侧边栏';
    els.sidebarToggle.setAttribute('aria-label', els.sidebarToggle.title);
  }

  function wire() {
    metricOptions(els.metricSelect, state.metric);
    metricOptions(els.overviewMetric, state.overviewMetric);

    document.querySelectorAll('.nav-item').forEach(button => {
      button.addEventListener('click', () => selectView(button.dataset.view));
    });
    document.querySelectorAll('.jump-analysis').forEach(button => {
      button.addEventListener('click', () => {
        state.group = button.dataset.group;
        els.groupSelect.value = state.group;
        state.chart = state.group === 'date' ? 'line' : 'bar';
        els.chartSelect.value = state.chart;
        selectView('analysis');
      });
    });

    els.rangeButtons.addEventListener('click', event => {
      const button = event.target.closest('button[data-range]');
      if (!button) return;
      state.range = button.dataset.range;
      els.rangeButtons.querySelectorAll('button').forEach(item => item.classList.toggle('active', item === button));
      els.customRange.classList.toggle('hidden', state.range !== 'custom');
      renderAll();
    });

    [
      [els.deviceFilter, 'device'], [els.clientFilter, 'client'], [els.modelFilter, 'model'],
      [els.upstreamFilter, 'upstreamVendor'], [els.routeProviderFilter, 'routeProvider'],
      [els.routeTypeFilter, 'routeType'], [els.providerFilter, 'provider'], [els.tierFilter, 'tier']
    ].forEach(([element, key]) => {
      element.addEventListener('change', () => {
        state[key] = element.value;
        renderAll();
      });
    });

    els.startDate.addEventListener('change', () => { state.customStart = els.startDate.value; renderAll(); });
    els.endDate.addEventListener('change', () => { state.customEnd = els.endDate.value; renderAll(); });
    els.metricSelect.addEventListener('change', () => { state.metric = els.metricSelect.value; renderAll(); });
    els.overviewMetric.addEventListener('change', () => { state.overviewMetric = els.overviewMetric.value; renderAll(); });
    els.groupSelect.addEventListener('change', () => { state.group = els.groupSelect.value; renderAll(); });
    els.chartSelect.addEventListener('change', () => { state.chart = els.chartSelect.value; renderAll(); });
    els.stackSelect.addEventListener('change', () => { state.stack = els.stackSelect.value; renderAll(); });

    els.refreshButton.addEventListener('click', () => {
      loadAll().catch(error => {
        setSync('error', '读取失败');
        showNotice('error', error.message || String(error));
      });
    });

    els.exportCsv.addEventListener('click', () => {
      const blob = new Blob([A.toCsv(filtered())], { type: 'text/csv;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = `token-monitor-${new Date().toISOString().slice(0, 10)}.csv`;
      anchor.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    });

    els.themeToggle.addEventListener('click', () => {
      const dark = document.documentElement.dataset.theme === 'dark';
      const next = dark ? 'light' : 'dark';
      document.documentElement.dataset.theme = next;
      localStorage.setItem('tm-theme', next);
      updateThemeLabel();
      renderAll();
    });

    els.sidebarToggle.addEventListener('click', () => {
      const collapsed = !els.layout.classList.contains('sidebar-collapsed');
      els.layout.classList.toggle('sidebar-collapsed', collapsed);
      localStorage.setItem('tm-sidebar', collapsed ? 'collapsed' : 'expanded');
      applySidebarPreference();
      setTimeout(renderAll, 210);
    });

    window.addEventListener('resize', () => {
      clearTimeout(window.__tmResize);
      window.__tmResize = setTimeout(renderAll, 120);
    });
  }

  async function boot() {
    state.repo = deriveRepo();
    state.key = fragmentKey();
    const theme = localStorage.getItem('tm-theme');
    if (theme) document.documentElement.dataset.theme = theme;
    applySidebarPreference();
    updateThemeLabel();
    wire();
    try {
      await loadAll();
    } catch (error) {
      clearData();
      setSync('error', '读取失败');
      showNotice('error', error.message || String(error));
    }
  }

  boot();
})();
