(() => {
  'use strict';

  const A = window.TokenAnalytics;
  if (!A) throw new Error('analytics.js failed to load');

  const API = 'https://api.github.com';
  const RAW = 'https://raw.githubusercontent.com';
  const BRANCH_PREFIX = 'tm-ledger-';
  const PALETTE = [
    '#1677ff','#16a66a','#7457d9','#d98b16','#d85a73','#0f94ad',
    '#5d8a24','#8b5cf6','#64748b','#c46921','#0891b2','#6d5bd0'
  ];
  const METRICS = {
    totalTokens: ['总 Tokens', compact],
    costUsd: ['等价费用', money],
    input: ['输入 Tokens', compact],
    cacheRead: ['缓存读取', compact],
    cacheWrite: ['缓存写入', compact],
    output: ['输出 Tokens', compact],
    reasoning: ['Reasoning', compact],
    messages: ['消息数', integer]
  };
  const DIMENSIONS = {
    date: '日期', device: '设备', client: '工具', model: '模型',
    upstreamVendor: '模型厂商', routeProvider: '路由提供商',
    routeType: '路由类型', provider: '原始 Provider', tier: 'Tier'
  };

  const state = {
    repo: '', ledgers: [], rows: [], view: 'overview', range: '30d',
    customStart: '', customEnd: '', device: '*', client: '*', model: '*',
    upstreamVendor: '*', routeProvider: '*', routeType: '*', provider: '*',
    tier: '*', metric: 'totalTokens', overviewMetric: 'totalTokens',
    group: 'date', chart: 'line', stack: 'device'
  };

  const $ = id => document.getElementById(id);
  const ids = [
    'layout','sidebar','sidebarToggle','mobileNavToggle','sidebarBackdrop',
    'emptyState','emptyTitle','emptyText','emptyRefresh','appContent','syncState',
    'updatedAt','refreshButton','viewTitle','themeToggle','rangeButtons','customRange',
    'startDate','endDate','repoLabel','deviceFilter','clientFilter','modelFilter',
    'upstreamFilter','routeProviderFilter','routeTypeFilter','providerFilter',
    'tierFilter','tierFilterLabel','mTotal','mCost','mCostNote','mInput','mCache',
    'mOutput','mMessages','overviewView','analysisView','devicesView','dataView',
    'overviewMetric','overviewTrend','deviceBars','routeBars','modelBars','clientBars',
    'metricSelect','groupSelect','chartSelect','stackSelect','stackLabel','analysisChart',
    'analysisTable','deviceTable','rawTable','exportCsv'
  ];
  const els = Object.fromEntries(ids.map(id => [id, $(id)]));

  function compact(value) {
    const n = Number(value || 0), abs = Math.abs(n);
    if (abs >= 1e12) return `${(n / 1e12).toFixed(abs >= 1e13 ? 1 : 2)}T`;
    if (abs >= 1e9) return `${(n / 1e9).toFixed(abs >= 1e10 ? 1 : 2)}B`;
    if (abs >= 1e6) return `${(n / 1e6).toFixed(abs >= 1e7 ? 1 : 2)}M`;
    if (abs >= 1e3) return `${(n / 1e3).toFixed(abs >= 1e4 ? 1 : 2)}K`;
    return Math.round(n).toLocaleString();
  }
  function integer(value) { return Math.round(Number(value || 0)).toLocaleString(); }
  function money(value) {
    return `$${Number(value || 0).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
  }
  function fmt(metric, value) { return (METRICS[metric]?.[1] || compact)(value); }
  function esc(value) {
    return String(value ?? '').replace(/[&<>"']/g, ch => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[ch]));
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

  async function apiJson(path) {
    const response = await fetch(`${API}${path}`, {
      headers: { Accept: 'application/vnd.github+json' }, cache: 'no-store'
    });
    if (!response.ok) throw new Error(`GitHub API ${response.status}`);
    return response.json();
  }

  async function branchPublicLedger(branch) {
    const url = `${RAW}/${state.repo}/${branch}/public.json`;
    const response = await fetch(url, { cache: 'no-store' });
    if (!response.ok) throw new Error(`${branch}: public.json ${response.status}`);
    const ledger = await response.json();
    if (ledger?.kind !== 'token-monitor-public-ledger' || Number(ledger?.schemaVersion) !== 1) {
      throw new Error(`${branch}: 不支持的公开摘要格式`);
    }
    return ledger;
  }

  function setSync(kind, text) {
    els.syncState.classList.remove('ok','error');
    if (kind === 'ok' || kind === 'error') els.syncState.classList.add(kind);
    els.syncState.querySelector('span:last-child').textContent = text;
  }
  function showState(title, text, button = true) {
    els.emptyTitle.textContent = title;
    els.emptyText.textContent = text;
    els.emptyRefresh.classList.toggle('hidden', !button);
    els.emptyState.classList.remove('hidden');
    els.appContent.classList.add('hidden');
  }
  function showApp() {
    els.emptyState.classList.add('hidden');
    els.appContent.classList.remove('hidden');
  }

  async function loadAll() {
    if (!state.repo) {
      setSync('error', '缺少数据源');
      showState('无法确定数据仓库', '请在网址中添加 ?repo=OWNER/REPO。');
      return;
    }
    els.repoLabel.textContent = state.repo;
    setSync('loading', '正在同步');

    const refs = await apiJson(`/repos/${state.repo}/git/matching-refs/heads/${BRANCH_PREFIX}`);
    const branches = refs
      .map(ref => String(ref.ref || '').replace(/^refs\/heads\//, ''))
      .filter(branch => branch.startsWith(BRANCH_PREFIX));

    if (!branches.length) {
      state.ledgers = []; state.rows = [];
      setSync('ok', '暂无设备');
      showState('暂无设备数据', '先在一台设备上运行 token-monitor setup；首次同步后这里会自动显示聚合统计。');
      return;
    }

    const settled = await Promise.allSettled(branches.map(branchPublicLedger));
    const ledgers = settled.filter(item => item.status === 'fulfilled').map(item => item.value);
    const failures = settled.filter(item => item.status === 'rejected');
    if (!ledgers.length) {
      setSync('error', '等待 v1.0.1 数据');
      showState(
        '需要生成公开聚合摘要',
        '当前设备分支仍是 v1.0.0 加密账本。升级到 v1.0.1 后执行 token-monitor sync --full，即可在任何浏览器直接查看。'
      );
      return;
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
    els.updatedAt.textContent = latest ? `更新于 ${new Date(latest).toLocaleString()}` : '暂无快照时间';
    setSync('ok', failures.length ? `${ledgers.length} 台已载入 · ${failures.length} 台待升级` : `${ledgers.length} 台设备在线`);
    showApp();
  }

  function unique(key, labelKey = key) {
    const map = new Map();
    for (const row of state.rows) {
      const value = row[key];
      if (value === undefined || value === null || value === '') continue;
      map.set(String(value), String(row[labelKey] ?? value));
    }
    return [...map.entries()].sort((a,b) => a[1].localeCompare(b[1]));
  }
  function fillSelect(select, entries, allLabel) {
    const old = select.value || '*';
    select.innerHTML = `<option value="*">${esc(allLabel)}</option>` + entries.map(([value,label]) => `<option value="${esc(value)}">${esc(label)}</option>`).join('');
    select.value = [...select.options].some(option => option.value === old) ? old : '*';
  }
  function populateFilters() {
    fillSelect(els.deviceFilter, unique('device','deviceName'), '全部设备');
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
    return `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`;
  }
  function bounds() {
    const today = new Date(); today.setHours(0,0,0,0);
    let start = null, end = iso(today);
    if (state.range === 'today') start = end;
    else if (state.range === '7d') { const d = new Date(today); d.setDate(d.getDate()-6); start = iso(d); }
    else if (state.range === '30d') { const d = new Date(today); d.setDate(d.getDate()-29); start = iso(d); }
    else if (state.range === 'month') start = iso(new Date(today.getFullYear(), today.getMonth(), 1));
    else if (state.range === 'all') end = null;
    else if (state.range === 'custom') { start = state.customStart || null; end = state.customEnd || end; }
    return { start, end };
  }
  function filtered() {
    return A.filterRows(state.rows, {
      ...bounds(), device:state.device, client:state.client, model:state.model,
      upstreamVendor:state.upstreamVendor, routeProvider:state.routeProvider,
      routeType:state.routeType, provider:state.provider, tier:state.tier
    });
  }

  function renderSummary(rows) {
    const sum = A.sumRows(rows);
    els.mTotal.textContent = compact(sum.totalTokens);
    els.mCost.textContent = `${sum.costLowerBound ? '≥' : ''}${money(sum.costUsd)}`;
    els.mCostNote.textContent = sum.costLowerBound ? '含下限估算' : 'Codex 计划等价 / 其他 API 等价';
    els.mCost.title = 'Codex 使用按订阅/额度等价口径估算；其他客户端沿用其可验证的 API 等价价格。该数字不等于实际信用卡账单。';
    els.mInput.textContent = compact(sum.input);
    els.mCache.textContent = compact(sum.cacheRead);
    els.mOutput.textContent = compact(sum.output);
    els.mMessages.textContent = integer(sum.messages);
  }

  function renderRank(container, rows, dimension, metric='totalTokens', limit=8) {
    const items = A.groupRows(rows, dimension, metric).slice(0, limit);
    const max = Math.max(1, ...items.map(item => item.value));
    container.innerHTML = items.length ? items.map(item => `
      <div class="rank-row">
        <span class="rank-name" title="${esc(item.key)}">${esc(item.key)}</span>
        <span class="rank-track"><span class="rank-fill" style="width:${Math.max(1,item.value/max*100)}%"></span></span>
        <span class="rank-value">${esc(fmt(metric,item.value))}</span>
      </div>`).join('') : '<div class="empty">当前筛选无数据</div>';
  }

  const NS='http://www.w3.org/2000/svg';
  function node(tag, attrs={}) {
    const el=document.createElementNS(NS,tag);
    Object.entries(attrs).forEach(([key,value])=>el.setAttribute(key,String(value)));
    return el;
  }
  function tooltip() {
    let el=document.querySelector('.tooltip');
    if(!el){el=document.createElement('div');el.className='tooltip hidden';document.body.appendChild(el);} return el;
  }
  function attachTip(element,text){const tip=tooltip();element.addEventListener('pointermove',event=>{tip.textContent=text;tip.style.left=`${event.clientX+12}px`;tip.style.top=`${event.clientY+12}px`;tip.classList.remove('hidden')});element.addEventListener('pointerleave',()=>tip.classList.add('hidden'));}
  function frame(container){container.innerHTML='';const width=Math.max(520,container.clientWidth-30),height=Math.max(240,container.clientHeight-28);const root=node('svg',{viewBox:`0 0 ${width} ${height}`,preserveAspectRatio:'none'});container.appendChild(root);return{root,width,height,left:58,right:16,top:14,bottom:36};}
  function axes(layout,labels,max,metric){const innerWidth=layout.width-layout.left-layout.right,innerHeight=layout.height-layout.top-layout.bottom;for(let i=0;i<=4;i++){const y=layout.top+innerHeight*i/4;layout.root.appendChild(node('line',{x1:layout.left,y1:y,x2:layout.width-layout.right,y2:y,class:'grid-line'}));const text=node('text',{x:layout.left-8,y:y+4,'text-anchor':'end',class:'axis-label'});text.textContent=fmt(metric,max*(1-i/4));layout.root.appendChild(text)}const step=Math.max(1,Math.ceil(labels.length/8));labels.forEach((label,index)=>{if(index%step&&index!==labels.length-1)return;const x=labels.length<2?layout.left+innerWidth/2:layout.left+innerWidth*index/(labels.length-1);const text=node('text',{x,y:layout.height-10,'text-anchor':'middle',class:'axis-label'});const value=String(label);text.textContent=value.length>14?`${value.slice(0,12)}…`:value;layout.root.appendChild(text)});return{innerWidth,innerHeight};}
  function lineChart(container,items,metric,area=false){if(!items.length){container.innerHTML='<div class="empty">当前筛选无数据</div>';return}const data=[...items].sort((a,b)=>a.key.localeCompare(b.key)),layout=frame(container),max=Math.max(1,...data.map(item=>item.value)),{innerWidth,innerHeight}=axes(layout,data.map(item=>item.key),max,metric);const points=data.map((item,index)=>({x:data.length<2?layout.left+innerWidth/2:layout.left+innerWidth*index/(data.length-1),y:layout.top+innerHeight*(1-item.value/max),item}));const path=points.map((p,i)=>`${i?'L':'M'}${p.x},${p.y}`).join(' ');if(area){const baseline=layout.top+innerHeight;layout.root.appendChild(node('path',{d:`${path} L${points.at(-1).x},${baseline} L${points[0].x},${baseline} Z`,class:'chart-area'}))}layout.root.appendChild(node('path',{d:path,class:'chart-line'}));points.forEach(p=>{const c=node('circle',{cx:p.x,cy:p.y,r:3.6,fill:'var(--accent)'});attachTip(c,`${p.item.key} · ${fmt(metric,p.item.value)}`);layout.root.appendChild(c)});}
  function barChart(container,items,metric){if(!items.length){container.innerHTML='<div class="empty">当前筛选无数据</div>';return}const data=items.slice(0,35).reverse(),layout=frame(container),innerWidth=layout.width-layout.left-layout.right,innerHeight=layout.height-layout.top-layout.bottom,max=Math.max(1,...data.map(item=>item.value)),gap=4,barHeight=Math.max(3,(innerHeight-gap*(data.length-1))/data.length);data.forEach((item,index)=>{const y=layout.top+index*(barHeight+gap),rect=node('rect',{x:layout.left,y,width:Math.max(1,innerWidth*item.value/max),height:barHeight,rx:3,class:'chart-bar'});attachTip(rect,`${item.key} · ${fmt(metric,item.value)}`);layout.root.appendChild(rect);if(data.length<=16){const text=node('text',{x:layout.left-8,y:y+barHeight/2+4,'text-anchor':'end',class:'axis-label'});text.textContent=item.key.length>12?`${item.key.slice(0,10)}…`:item.key;layout.root.appendChild(text)}});}
  function renderLegend(container,labels){const legend=document.createElement('div');legend.className='legend';legend.innerHTML=labels.slice(0,12).map((label,index)=>`<span class="legend-item"><i class="legend-dot" style="background:${PALETTE[index%PALETTE.length]}"></i>${esc(label)}</span>`).join('');container.appendChild(legend);}
  function stackedChart(container,rows,primary,secondary,metric){if(primary===secondary)secondary=primary==='device'?'client':'device';const matrix=A.groupMatrix(rows,primary,secondary,metric);if(!matrix.xValues.length){container.innerHTML='<div class="empty">当前筛选无数据</div>';return}const xValues=matrix.xValues.slice(primary==='date'?-45:0),stacks=matrix.stacks.slice(0,12),totals=xValues.map(x=>stacks.reduce((sum,s)=>sum+matrix.value(x,s),0)),layout=frame(container),max=Math.max(1,...totals),{innerWidth,innerHeight}=axes(layout,xValues,max,metric),slot=innerWidth/Math.max(1,xValues.length),barWidth=Math.max(2,slot*.72);xValues.forEach((x,xi)=>{let cumulative=0;stacks.forEach((stack,si)=>{const value=matrix.value(x,stack);if(!value)return;const height=innerHeight*value/max,y=layout.top+innerHeight-innerHeight*(cumulative+value)/max,rect=node('rect',{x:layout.left+xi*slot+(slot-barWidth)/2,y,width:barWidth,height:Math.max(.8,height),fill:PALETTE[si%PALETTE.length],class:'chart-segment'});attachTip(rect,`${x} · ${stack} · ${fmt(metric,value)}`);layout.root.appendChild(rect);cumulative+=value})});renderLegend(container,stacks);}
  function donutChart(container,items,metric){const data=items.filter(item=>item.value>0).slice(0,12),total=data.reduce((s,i)=>s+i.value,0);if(!total){container.innerHTML='<div class="empty">当前筛选无数据</div>';return}const layout=frame(container),cx=layout.width/2,cy=layout.height/2,radius=Math.min(layout.width,layout.height)*.31,strokeWidth=radius*.34,circ=2*Math.PI*radius;let offset=0;data.forEach((item,index)=>{const length=circ*item.value/total,c=node('circle',{cx,cy,r:radius,fill:'none',stroke:PALETTE[index%PALETTE.length],'stroke-width':strokeWidth,'stroke-dasharray':`${length} ${circ-length}`,'stroke-dashoffset':-offset,transform:`rotate(-90 ${cx} ${cy})`});attachTip(c,`${item.key} · ${fmt(metric,item.value)} · ${(item.value/total*100).toFixed(1)}%`);layout.root.appendChild(c);offset+=length});const center=node('text',{x:cx,y:cy+6,'text-anchor':'middle',class:'donut-center'});center.textContent=fmt(metric,total);layout.root.appendChild(center);renderLegend(container,data.map(i=>i.key));}
  function treemapChart(container,items,metric){const data=items.filter(i=>i.value>0).slice(0,25);if(!data.length){container.innerHTML='<div class="empty">当前筛选无数据</div>';return}const layout=frame(container),rects=A.squarify(data,5,5,layout.width-10,layout.height-10);rects.forEach((item,index)=>{const rect=node('rect',{x:item.x,y:item.y,width:Math.max(0,item.width-2),height:Math.max(0,item.height-2),rx:5,fill:PALETTE[index%PALETTE.length]});attachTip(rect,`${item.key} · ${fmt(metric,item.value)}`);layout.root.appendChild(rect);if(item.width>75&&item.height>30){const text=node('text',{x:item.x+8,y:item.y+18,class:'treemap-label'});text.textContent=item.key.length>18?`${item.key.slice(0,16)}…`:item.key;layout.root.appendChild(text)}});}

  function tableHtml(headers,rows){return `<table class="data-table"><thead><tr>${headers.map(h=>`<th class="${h.number?'number':''}">${esc(h.label)}</th>`).join('')}</tr></thead><tbody>${rows.map(row=>`<tr>${headers.map(h=>`<td class="${h.number?'number':''}">${h.render?h.render(row[h.key],row):esc(row[h.key])}</td>`).join('')}</tr>`).join('')}</tbody></table>`;}
  function aggregateTable(rows,dimension,metric){const items=A.groupRows(rows,dimension,metric);els.analysisTable.innerHTML=tableHtml([{key:'key',label:DIMENSIONS[dimension]||dimension},{key:'value',label:METRICS[metric]?.[0]||metric,number:true,render:value=>esc(fmt(metric,value))}],items);}
  function renderAnalysis(rows){els.stackLabel.classList.toggle('hidden',state.chart!=='stacked');els.analysisTable.classList.toggle('hidden',state.chart!=='table');els.analysisChart.classList.toggle('hidden',state.chart==='table');const items=A.groupRows(rows,state.group,state.metric);els.analysisChart.title=state.metric==='costUsd'&&rows.some(row=>row.costLowerBound)?'当前筛选包含费用下限估算。':'';if(state.chart==='line')lineChart(els.analysisChart,items,state.metric,false);else if(state.chart==='area')lineChart(els.analysisChart,items,state.metric,true);else if(state.chart==='bar')barChart(els.analysisChart,items,state.metric);else if(state.chart==='stacked')stackedChart(els.analysisChart,rows,state.group,state.stack,state.metric);else if(state.chart==='donut')donutChart(els.analysisChart,items,state.metric);else if(state.chart==='treemap')treemapChart(els.analysisChart,items,state.metric);else aggregateTable(rows,state.group,state.metric);}
  function renderDevices(rows){const entries=state.ledgers.map(ledger=>{const id=ledger.device?.id||'unknown',sum=A.sumRows(rows.filter(row=>row.device===id));return{name:ledger.device?.name||id,platform:ledger.device?.platform||'unknown',arch:ledger.device?.arch||'',tokens:sum.totalTokens,cost:sum.costUsd,lowerBound:sum.costLowerBound,messages:sum.messages,updatedAt:ledger.generatedAt||'',scanMs:ledger.scanMs||0}}).sort((a,b)=>b.tokens-a.tokens);els.deviceTable.innerHTML=tableHtml([{key:'name',label:'设备'},{key:'platform',label:'平台'},{key:'arch',label:'架构'},{key:'tokens',label:'Tokens',number:true,render:v=>esc(compact(v))},{key:'cost',label:'等价费用',number:true,render:(v,row)=>esc(`${row.lowerBound?'≥':''}${money(v)}`)},{key:'messages',label:'消息',number:true,render:v=>esc(integer(v))},{key:'updatedAt',label:'数据时间',render:v=>esc(v?new Date(v).toLocaleString():'—')},{key:'scanMs',label:'最近扫描',number:true,render:v=>esc(`${integer(v)} ms`)}],entries);}
  function renderRaw(rows){const data=[...rows].sort((a,b)=>b.date.localeCompare(a.date)||a.deviceName.localeCompare(b.deviceName)).slice(0,2000);els.rawTable.innerHTML=tableHtml([{key:'date',label:'日期'},{key:'deviceName',label:'设备'},{key:'client',label:'工具'},{key:'upstreamVendor',label:'模型厂商'},{key:'routeProvider',label:'路由提供商'},{key:'routeType',label:'路由类型'},{key:'model',label:'模型'},{key:'tier',label:'Tier'},{key:'input',label:'Input',number:true,render:v=>esc(compact(v))},{key:'cacheRead',label:'Cache R',number:true,render:v=>esc(compact(v))},{key:'cacheWrite',label:'Cache W',number:true,render:v=>esc(compact(v))},{key:'output',label:'Output',number:true,render:v=>esc(compact(v))},{key:'reasoning',label:'Reasoning',number:true,render:v=>esc(compact(v))},{key:'messages',label:'消息',number:true,render:v=>esc(integer(v))},{key:'costUsd',label:'等价费用',number:true,render:(v,row)=>esc(`${row.costLowerBound?'≥':''}${money(v)}`)}],data);}
  function renderAll(){const rows=filtered();renderSummary(rows);if(state.view==='overview'){lineChart(els.overviewTrend,A.groupRows(rows,'date',state.overviewMetric),state.overviewMetric,true);renderRank(els.deviceBars,rows,'device');renderRank(els.routeBars,rows,'routeProvider');renderRank(els.modelBars,rows,'model');renderRank(els.clientBars,rows,'client')}else if(state.view==='analysis')renderAnalysis(rows);else if(state.view==='devices')renderDevices(rows);else if(state.view==='data')renderRaw(rows);}

  function closeMobileNav(){els.sidebar.classList.remove('mobile-open');els.sidebarBackdrop.classList.remove('visible');}
  function selectView(view){state.view=view;document.querySelectorAll('.nav-item').forEach(button=>button.classList.toggle('active',button.dataset.view===view));['overview','analysis','devices','data'].forEach(name=>els[`${name}View`].classList.toggle('hidden',name!==view));els.viewTitle.textContent=({overview:'概览',analysis:'用量分析',devices:'设备',data:'数据明细'})[view]||'Token Monitor';closeMobileNav();renderAll();}
  function metricOptions(select,selected){select.innerHTML=Object.entries(METRICS).map(([key,[label]])=>`<option value="${key}">${esc(label)}</option>`).join('');select.value=selected;}
  function applySidebarState(collapsed){els.layout.classList.toggle('sidebar-collapsed',collapsed);localStorage.setItem('tm-sidebar-collapsed',collapsed?'1':'0');els.sidebarToggle.setAttribute('aria-label',collapsed?'展开侧边栏':'收起侧边栏');els.sidebarToggle.title=collapsed?'展开侧边栏':'收起侧边栏';setTimeout(renderAll,220);}

  function wire(){
    metricOptions(els.metricSelect,state.metric);metricOptions(els.overviewMetric,state.overviewMetric);
    document.querySelectorAll('.nav-item').forEach(button=>button.addEventListener('click',()=>selectView(button.dataset.view)));
    document.querySelectorAll('.jump-analysis').forEach(button=>button.addEventListener('click',()=>{state.group=button.dataset.group;els.groupSelect.value=state.group;state.chart=state.group==='date'?'line':'bar';els.chartSelect.value=state.chart;selectView('analysis')}));
    els.rangeButtons.addEventListener('click',event=>{const button=event.target.closest('button[data-range]');if(!button)return;state.range=button.dataset.range;els.rangeButtons.querySelectorAll('button').forEach(item=>item.classList.toggle('active',item===button));els.customRange.classList.toggle('hidden',state.range!=='custom');renderAll()});
    [[els.deviceFilter,'device'],[els.clientFilter,'client'],[els.modelFilter,'model'],[els.upstreamFilter,'upstreamVendor'],[els.routeProviderFilter,'routeProvider'],[els.routeTypeFilter,'routeType'],[els.providerFilter,'provider'],[els.tierFilter,'tier']].forEach(([element,key])=>element.addEventListener('change',()=>{state[key]=element.value;renderAll()}));
    els.startDate.addEventListener('change',()=>{state.customStart=els.startDate.value;renderAll()});els.endDate.addEventListener('change',()=>{state.customEnd=els.endDate.value;renderAll()});els.metricSelect.addEventListener('change',()=>{state.metric=els.metricSelect.value;renderAll()});els.overviewMetric.addEventListener('change',()=>{state.overviewMetric=els.overviewMetric.value;renderAll()});els.groupSelect.addEventListener('change',()=>{state.group=els.groupSelect.value;renderAll()});els.chartSelect.addEventListener('change',()=>{state.chart=els.chartSelect.value;renderAll()});els.stackSelect.addEventListener('change',()=>{state.stack=els.stackSelect.value;renderAll()});
    els.refreshButton.addEventListener('click',()=>loadAll().catch(handleLoadError));els.emptyRefresh.addEventListener('click',()=>loadAll().catch(handleLoadError));
    els.exportCsv.addEventListener('click',()=>{const blob=new Blob([A.toCsv(filtered())],{type:'text/csv;charset=utf-8'}),url=URL.createObjectURL(blob),anchor=document.createElement('a');anchor.href=url;anchor.download=`token-monitor-${new Date().toISOString().slice(0,10)}.csv`;anchor.click();setTimeout(()=>URL.revokeObjectURL(url),1000)});
    els.themeToggle.addEventListener('click',()=>{const dark=document.documentElement.dataset.theme==='dark',next=dark?'light':'dark';document.documentElement.dataset.theme=next;localStorage.setItem('tm-theme',next);renderAll()});
    els.sidebarToggle.addEventListener('click',()=>applySidebarState(!els.layout.classList.contains('sidebar-collapsed')));
    els.mobileNavToggle.addEventListener('click',()=>{els.sidebar.classList.add('mobile-open');els.sidebarBackdrop.classList.add('visible')});els.sidebarBackdrop.addEventListener('click',closeMobileNav);
    window.addEventListener('resize',()=>{clearTimeout(window.__tmResize);window.__tmResize=setTimeout(renderAll,120)});
  }

  function handleLoadError(error){console.error(error);setSync('error','读取失败');showState('无法读取统计数据',error?.message||String(error));}
  async function boot(){state.repo=deriveRepo();els.repoLabel.textContent=state.repo||'—';const theme=localStorage.getItem('tm-theme');if(theme)document.documentElement.dataset.theme=theme;wire();applySidebarState(localStorage.getItem('tm-sidebar-collapsed')==='1');showState('正在载入统计数据','Token Monitor 正在从 GitHub 读取各设备的公开聚合摘要。',false);try{await loadAll()}catch(error){handleLoadError(error)}}

  boot();
})();
