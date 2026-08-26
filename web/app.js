(() => {
  'use strict';

  const A = window.TokenAnalytics;
  if (!A) throw new Error('analytics.js failed to load');

  const API = 'https://api.github.com';
  const RAW = 'https://raw.githubusercontent.com';
  const BRANCH_PREFIX = 'tm-ledger-';
  const PALETTE = [
    '#2563eb','#0f766e','#7c3aed','#b45309','#be123c','#0369a1',
    '#4d7c0f','#9333ea','#475569','#c2410c','#0891b2','#6d28d9'
  ];
  const METRICS = {
    totalTokens: ['总 Tokens', compact],
    costUsd: ['API 等价费用', money],
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
    key: '',
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
    'unlockPanel','keyInput','unlockButton','unlockError','appContent','syncState',
    'updatedAt','refreshButton','viewTitle','themeToggle','rangeButtons','customRange',
    'startDate','endDate','deviceFilter','clientFilter','modelFilter','upstreamFilter',
    'routeProviderFilter','routeTypeFilter','providerFilter','tierFilter','tierFilterLabel',
    'mTotal','mCost','mCostNote','mInput','mCache','mOutput','mMessages',
    'overviewView','analysisView','devicesView','dataView','overviewMetric','overviewTrend',
    'deviceBars','routeBars','modelBars','clientBars','metricSelect','groupSelect',
    'chartSelect','stackSelect','stackLabel','analysisChart','analysisTable','deviceTable',
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
      maximumFractionDigits: 2
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
      throw new Error('不支持的加密账本格式');
    }
    const rawKey = base64UrlBytes(encodedKey);
    if (rawKey.length !== 32) throw new Error('Dashboard key 无效');
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

  async function branchLedger(branch) {
    const rawUrl = `${RAW}/${state.repo}/${branch}/ledger.json`;
    const response = await fetch(rawUrl, { cache: 'no-store' });
    if (!response.ok) throw new Error(`读取 ${branch} 失败 (${response.status})`);
    return decryptEnvelope(await response.json(), state.key);
  }

  function setSync(kind, text) {
    els.syncState.classList.toggle('ok', kind === 'ok');
    els.syncState.querySelector('span:last-child').textContent = text;
  }

  function showUnlock(error = '') {
    els.unlockPanel.classList.remove('hidden');
    els.appContent.classList.add('hidden');
    els.unlockError.textContent = error;
  }

  function showApp() {
    els.unlockPanel.classList.add('hidden');
    els.appContent.classList.remove('hidden');
  }

  async function loadAll() {
    if (!state.repo) throw new Error('无法判断数据仓库，请在网址添加 ?repo=OWNER/REPO');
    if (!state.key) return showUnlock();

    setSync('loading', '正在读取 GitHub');
    const refs = await apiJson(`/repos/${state.repo}/git/matching-refs/heads/${BRANCH_PREFIX}`);
    const branches = refs
      .map(ref => String(ref.ref || '').replace(/^refs\/heads\//, ''))
      .filter(branch => branch.startsWith(BRANCH_PREFIX));
    const settled = await Promise.allSettled(branches.map(branchLedger));
    const ledgers = settled.filter(item => item.status === 'fulfilled').map(item => item.value);
    const failures = settled.filter(item => item.status === 'rejected');
    if (!ledgers.length && failures.length) throw failures[0].reason;

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
      ? `数据 ${new Date(latest).toLocaleString()}`
      : '暂无设备快照';
    setSync(
      'ok',
      failures.length
        ? `${ledgers.length} 台已载入，${failures.length} 台失败`
        : `${ledgers.length} 台设备`
    );
    showApp();
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
    els.mCost.textContent = money(sum.costUsd);
    els.mCostNote.textContent = sum.costLowerBound ? '含下限估算' : '';
    els.mCost.title = sum.costLowerBound
      ? '部分 Codex 记录缺少 cache-write token，因此 API 等价费用是已知下限。'
      : 'API 等价费用，不代表订阅实际账单。';
    els.mInput.textContent = compact(sum.input);
    els.mCache.textContent = compact(sum.cacheRead);
    els.mOutput.textContent = compact(sum.output);
    els.mMessages.textContent = integer(sum.messages);
  }

  function renderRank(container, rows, dimension, metric = 'totalTokens', limit = 8) {
    const items = A.groupRows(rows, dimension, metric).slice(0, limit);
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
    const width = Math.max(520, container.clientWidth - 36);
    const height = Math.max(240, container.clientHeight - 36);
    const root = node('svg', {
      viewBox: `0 0 ${width} ${height}`,
      preserveAspectRatio: 'none'
    });
    container.appendChild(root);
    return { root, width, height, left: 54, right: 16, top: 14, bottom: 36 };
  }

  function axes(layout, labels, max) {
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
      text.textContent = compact(max * (1 - index / 4));
      layout.root.appendChild(text);
    }
    const step = Math.max(1, Math.ceil(labels.length / 8));
    labels.forEach((label, index) => {
      if (index % step && index !== labels.length - 1) return;
      const x = labels.length < 2
        ? layout.left + innerWidth / 2
        : layout.left + innerWidth * index / (labels.length - 1);
      const text = node('text', {
        x, y: layout.height - 10, 'text-anchor': 'middle', class: 'axis-label'
      });
      const value = String(label);
      text.textContent = value.length > 14 ? `${value.slice(0, 12)}…` : value;
      layout.root.appendChild(text);
    });
    return { innerWidth, innerHeight };
  }

  function lineChart(container, items, metric, area = false) {
    if (!items.length) {
      container.innerHTML = '<div class="empty">当前筛选无数据</div>';
      return;
    }
    const data = [...items].sort((a, b) => a.key.localeCompare(b.key));
    const layout = frame(container);
    const max = Math.max(1, ...data.map(item => item.value));
    const { innerWidth, innerHeight } = axes(layout, data.map(item => item.key), max);
    const points = data.map((item, index) => ({
      x: data.length < 2
        ? layout.left + innerWidth / 2
        : layout.left + innerWidth * index / (data.length - 1),
      y: layout.top + innerHeight * (1 - item.value / max),
      item
    }));
    const path = points.map((point, index) => `${index ? 'L' : 'M'}${point.x},${point.y}`).join(' ');
    if (area) {
      const baseline = layout.top + innerHeight;
      layout.root.appendChild(node('path', {
        d: `${path} L${points.at(-1).x},${baseline} L${points[0].x},${baseline} Z`,
        class: 'chart-area'
      }));
    }
    layout.root.appendChild(node('path', { d: path, class: 'chart-line' }));
    points.forEach(point => {
      const circle = node('circle', {
        cx: point.x, cy: point.y, r: 3.5, fill: 'var(--accent)'
      });
      attachTip(circle, `${point.item.key} · ${fmt(metric, point.item.value)}`);
      layout.root.appendChild(circle);
    });
  }

  function barChart(container, items, metric) {
    if (!items.length) {
      container.innerHTML = '<div class="empty">当前筛选无数据</div>';
      return;
    }
    const data = items.slice(0, 35).reverse();
    const layout = frame(container);
    const innerWidth = layout.width - layout.left - layout.right;
    const innerHeight = layout.height - layout.top - layout.bottom;
    const max = Math.max(1, ...data.map(item => item.value));
    const gap = 4;
    const barHeight = Math.max(3, (innerHeight - gap * (data.length - 1)) / data.length);
    data.forEach((item, index) => {
      const y = layout.top + index * (barHeight + gap);
      const rect = node('rect', {
        x: layout.left,
        y,
        width: Math.max(1, innerWidth * item.value / max),
        height: barHeight,
        rx: 2,
        class: 'chart-bar'
      });
      attachTip(rect, `${item.key} · ${fmt(metric, item.value)}`);
      layout.root.appendChild(rect);
      if (data.length <= 16) {
        const text = node('text', {
          x: layout.left - 8,
          y: y + barHeight / 2 + 4,
          'text-anchor': 'end',
          class: 'axis-label'
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
      <span class="legend-item">
        <i class="legend-dot" style="background:${PALETTE[index % PALETTE.length]}"></i>${esc(label)}
      </span>`).join('');
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
    const layout = frame(container);
    const max = Math.max(1, ...totals);
    const { innerWidth, innerHeight } = axes(layout, xValues, max);
    const slot = innerWidth / Math.max(1, xValues.length);
    const barWidth = Math.max(2, slot * 0.72);

    xValues.forEach((x, xIndex) => {
      let cumulative = 0;
      stacks.forEach((stack, stackIndex) => {
        const value = matrix.value(x, stack);
        if (!value) return;
        const height = innerHeight * value / max;
        const y = layout.top + innerHeight - innerHeight * (cumulative + value) / max;
        const rect = node('rect', {
          x: layout.left + xIndex * slot + (slot - barWidth) / 2,
          y,
          width: barWidth,
          height: Math.max(0.8, height),
          fill: PALETTE[stackIndex % PALETTE.length],
          class: 'chart-segment'
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
    const cx = layout.width / 2;
    const cy = layout.height / 2;
    const radius = Math.min(layout.width, layout.height) * 0.31;
    const strokeWidth = radius * 0.36;
    const circumference = 2 * Math.PI * radius;
    let offset = 0;
    data.forEach((item, index) => {
      const length = circumference * item.value / total;
      const circle = node('circle', {
        cx, cy, r: radius,
        fill: 'none',
        stroke: PALETTE[index % PALETTE.length],
        'stroke-width': strokeWidth,
        'stroke-dasharray': `${length} ${circumference - length}`,
        'stroke-dashoffset': -offset,
        transform: `rotate(-90 ${cx} ${cy})`
      });
      attachTip(circle, `${item.key} · ${fmt(metric, item.value)} · ${(item.value / total * 100).toFixed(1)}%`);
      layout.root.appendChild(circle);
      offset += length;
    });
    const center = node('text', {
      x: cx, y: cy + 6, 'text-anchor': 'middle', class: 'donut-center'
    });
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
        x: item.x,
        y: item.y,
        width: Math.max(0, item.width - 2),
        height: Math.max(0, item.height - 2),
        rx: 3,
        fill: PALETTE[index % PALETTE.length]
      });
      attachTip(rect, `${item.key} · ${fmt(metric, item.value)}`);
      layout.root.appendChild(rect);
      if (item.width > 75 && item.height > 30) {
        const text = node('text', {
          x: item.x + 8, y: item.y + 18, class: 'treemap-label'
        });
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
    els.analysisChart.title = state.metric === 'costUsd' && rows.some(row => row.costLowerBound)
      ? '当前筛选包含费用下限估算。'
      : '';

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
        cost: sum.costUsd,
        lowerBound: sum.costLowerBound,
        messages: sum.messages,
        updatedAt: ledger.generatedAt || '',
        scanMs: ledger.scanMs || 0
      };
    }).sort((a, b) => b.tokens - a.tokens);

    els.deviceTable.innerHTML = tableHtml([
      { key: 'name', label: '设备' },
      { key: 'platform', label: '平台' },
      { key: 'arch', label: '架构' },
      { key: 'tokens', label: 'Tokens', number: true, render: value => esc(compact(value)) },
      { key: 'cost', label: 'API 等价费用', number: true, render: (value, row) => esc(`${row.lowerBound ? '≥' : ''}${money(value)}`) },
      { key: 'messages', label: '消息', number: true, render: value => esc(integer(value)) },
      { key: 'updatedAt', label: '数据时间', render: value => esc(value ? new Date(value).toLocaleString() : '—') },
      { key: 'scanMs', label: '最近扫描', number: true, render: value => esc(`${integer(value)} ms`) }
    ], entries);
  }

  function renderRaw(rows) {
    const data = [...rows]
      .sort((a, b) => b.date.localeCompare(a.date) || a.deviceName.localeCompare(b.deviceName))
      .slice(0, 2000);
    els.rawTable.innerHTML = tableHtml([
      { key: 'date', label: '日期' },
      { key: 'deviceName', label: '设备' },
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
      { key: 'costUsd', label: '费用', number: true, render: (value, row) => esc(`${row.costLowerBound ? '≥' : ''}${money(value)}`) }
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
    els.viewTitle.textContent = ({
      overview: '概览', analysis: '用量分析', devices: '设备', data: '数据'
    })[view] || 'Token Monitor';
    renderAll();
  }

  function metricOptions(select, selected) {
    select.innerHTML = Object.entries(METRICS)
      .map(([key, [label]]) => `<option value="${key}">${esc(label)}</option>`)
      .join('');
    select.value = selected;
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

    els.startDate.addEventListener('change', () => {
      state.customStart = els.startDate.value;
      renderAll();
    });
    els.endDate.addEventListener('change', () => {
      state.customEnd = els.endDate.value;
      renderAll();
    });
    els.metricSelect.addEventListener('change', () => {
      state.metric = els.metricSelect.value;
      renderAll();
    });
    els.overviewMetric.addEventListener('change', () => {
      state.overviewMetric = els.overviewMetric.value;
      renderAll();
    });
    els.groupSelect.addEventListener('change', () => {
      state.group = els.groupSelect.value;
      renderAll();
    });
    els.chartSelect.addEventListener('change', () => {
      state.chart = els.chartSelect.value;
      renderAll();
    });
    els.stackSelect.addEventListener('change', () => {
      state.stack = els.stackSelect.value;
      renderAll();
    });

    els.refreshButton.addEventListener('click', () => {
      loadAll().catch(error => showUnlock(error.message || String(error)));
    });
    els.unlockButton.addEventListener('click', () => {
      state.key = els.keyInput.value.trim();
      if (!state.key) return;
      history.replaceState(null, '', `${location.pathname}${location.search}#key=${encodeURIComponent(state.key)}`);
      loadAll().catch(error => showUnlock(error.message || String(error)));
    });
    els.keyInput.addEventListener('keydown', event => {
      if (event.key === 'Enter') els.unlockButton.click();
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
      renderAll();
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
    wire();
    if (!state.key) {
      showUnlock();
      return;
    }
    els.keyInput.value = state.key;
    try {
      await loadAll();
    } catch (error) {
      showUnlock(error.message || String(error));
      setSync('error', '读取失败');
    }
  }

  boot();
})();
