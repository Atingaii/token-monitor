(() => {
  'use strict';

  const A = window.TokenAnalytics;
  if (!A) throw new Error('analytics.js failed to load');

  const API = 'https://api.github.com';
  const RAW = 'https://raw.githubusercontent.com';
  const BRANCH_PREFIX = 'tm-ledger-';
  const PALETTE = ['#2563eb','#0f766e','#7c3aed','#b45309','#be123c','#0369a1','#4d7c0f','#9333ea','#475569','#c2410c','#0891b2','#6d28d9'];
  const METRICS = {
    totalTokens: ['总 Tokens', compact],
    costUsd: ['API 等价费用', money],
    input: ['输入 Tokens', compact],
    cacheRead: ['缓存读取', compact],
    cacheWrite: ['缓存写入', compact],
    output: ['输出 Tokens', compact],
    reasoning: ['Reasoning', compact],
    messages: ['消息数', integer],
    sessions: ['Session 数', integer],
    durationMs: ['生成耗时', duration]
  };
  const DIMENSIONS = {
    date: '日期', device: '设备', client: '工具', model: '模型', upstreamVendor: '模型厂商',
    routeProvider: '路由提供商', routeType: '路由类型', provider: '原始 Provider', tier: 'Tier'
  };

  const state = {
    repo: '', key: '', ledgers: [], rows: [], view: 'overview', range: '30d', customStart: '', customEnd: '',
    device: '*', client: '*', model: '*', upstreamVendor: '*', routeProvider: '*', routeType: '*', provider: '*', tier: '*',
    metric: 'totalTokens', overviewMetric: 'totalTokens', group: 'date', chart: 'line', stack: 'device'
  };

  const $ = id => document.getElementById(id);
  const els = Object.fromEntries([
    'unlockPanel','keyInput','unlockButton','unlockError','appContent','syncState','updatedAt','refreshButton','viewTitle','subtitle','themeToggle',
    'rangeButtons','customRange','startDate','endDate','deviceFilter','clientFilter','modelFilter','upstreamFilter','routeProviderFilter','routeTypeFilter',
    'providerFilter','tierFilter','tierFilterLabel','mTotal','mCost','mInput','mCache','mOutput','mMessages','overviewView','analysisView','devicesView','dataView',
    'overviewMetric','overviewTrend','deviceBars','routeBars','modelBars','clientBars','metricSelect','groupSelect','chartSelect','stackSelect','stackLabel',
    'analysisChart','analysisTable','deviceTable','rawTable','exportCsv'
  ].map(id => [id, $(id)]));

  function compact(value) {
    const n = Number(value || 0), a = Math.abs(n);
    if (a >= 1e12) return `${(n/1e12).toFixed(a >= 1e13 ? 1 : 2)}T`;
    if (a >= 1e9) return `${(n/1e9).toFixed(a >= 1e10 ? 1 : 2)}B`;
    if (a >= 1e6) return `${(n/1e6).toFixed(a >= 1e7 ? 1 : 2)}M`;
    if (a >= 1e3) return `${(n/1e3).toFixed(a >= 1e4 ? 1 : 2)}K`;
    return Math.round(n).toLocaleString();
  }
  function integer(value) { return Math.round(Number(value || 0)).toLocaleString(); }
  function money(value) { return `$${Number(value || 0).toLocaleString(undefined,{minimumFractionDigits:2,maximumFractionDigits:2})}`; }
  function duration(value) {
    const ms = Number(value || 0);
    if (ms < 1000) return `${Math.round(ms)} ms`;
    if (ms < 60000) return `${(ms/1000).toFixed(1)} s`;
    if (ms < 3600000) return `${(ms/60000).toFixed(1)} min`;
    return `${(ms/3600000).toFixed(1)} h`;
  }
  function fmt(metric, value) { return (METRICS[metric]?.[1] || compact)(value); }
  function esc(value) { return String(value ?? '').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c])); }

  function deriveRepo() {
    const param = new URLSearchParams(location.search).get('repo');
    if (param && /^[^/]+\/[^/]+$/.test(param)) return param;
    const host = location.hostname.toLowerCase();
    if (!host.endsWith('.github.io')) return '';
    const owner = host.slice(0,-'.github.io'.length);
    const first = location.pathname.split('/').filter(Boolean)[0];
    return first ? `${owner}/${first}` : `${owner}/${owner}.github.io`;
  }
  function parseFragment() {
    const params = new URLSearchParams(location.hash.replace(/^#/,''));
    return params.get('key') || '';
  }
  function base64UrlBytes(value) {
    let s = String(value || '').replace(/-/g,'+').replace(/_/g,'/');
    while (s.length % 4) s += '=';
    return Uint8Array.from(atob(s), c => c.charCodeAt(0));
  }
  function githubBase64Text(value) {
    const bytes = Uint8Array.from(atob(String(value || '').replace(/\s+/g,'')), c => c.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  }
  async function decryptEnvelope(envelope, encodedKey) {
    if (envelope?.kind !== 'token-monitor-encrypted-ledger' || Number(envelope?.schemaVersion) !== 2) throw new Error('不支持的账本格式');
    const rawKey = base64UrlBytes(encodedKey);
    if (rawKey.length !== 32) throw new Error('Dashboard key 无效');
    const key = await crypto.subtle.importKey('raw',rawKey,'AES-GCM',false,['decrypt']);
    const plaintext = await crypto.subtle.decrypt({
      name:'AES-GCM', iv:base64UrlBytes(envelope.nonce),
      additionalData:new TextEncoder().encode(`token-monitor-ledger-v2:${envelope.deviceHash}`)
    },key,base64UrlBytes(envelope.ciphertext));
    return JSON.parse(new TextDecoder().decode(plaintext));
  }

  async function apiJson(path) {
    const response = await fetch(`${API}${path}`,{headers:{Accept:'application/vnd.github+json'},cache:'no-store'});
    if (!response.ok) throw new Error(`GitHub API ${response.status}`);
    return response.json();
  }
  async function branchLedger(branch) {
    // Raw GitHub avoids spending one Contents-API request per device.
    const rawUrl = `${RAW}/${state.repo}/${encodeURIComponent(branch)}/ledger.json`;
    const response = await fetch(rawUrl,{cache:'no-store'});
    if (!response.ok) throw new Error(`读取 ${branch} 失败 (${response.status})`);
    return decryptEnvelope(await response.json(),state.key);
  }
  async function loadAll() {
    if (!state.repo) throw new Error('无法判断数据仓库；请在网址添加 ?repo=OWNER/REPO');
    if (!state.key) return showUnlock('');
    setSync('loading','正在读取 GitHub');
    // Matching refs returns every device branch without enumerating unrelated branches.
    const refs = await apiJson(`/repos/${state.repo}/git/matching-refs/heads/${BRANCH_PREFIX}`);
    const branches = refs.map(ref => String(ref.ref || '').replace(/^refs\/heads\//,'')).filter(Boolean);
    const settled = await Promise.allSettled(branches.map(branchLedger));
    const ledgers = settled.filter(x=>x.status==='fulfilled').map(x=>x.value);
    const failures = settled.filter(x=>x.status==='rejected');
    if (!ledgers.length && failures.length) throw failures[0].reason;
    state.ledgers = ledgers;
    state.rows = ledgers.flatMap(ledger => (ledger.rows || []).map(row => ({
      ...row,
      device: ledger.device?.id || 'unknown', deviceName: ledger.device?.name || ledger.device?.id || 'Unknown',
      platform: ledger.device?.platform || 'unknown', arch: ledger.device?.arch || '', updatedAt: ledger.generatedAt || ''
    })));
    populateFilters();
    renderAll();
    const latest = ledgers.map(x=>x.generatedAt).filter(Boolean).sort().at(-1);
    els.updatedAt.textContent = latest ? `更新 ${new Date(latest).toLocaleString()}` : '暂无设备快照';
    setSync('ok',failures.length ? `${ledgers.length} 台已载入，${failures.length} 台失败` : `${ledgers.length} 台设备`);
    showApp();
  }

  function setSync(kind,text) {
    els.syncState.classList.toggle('ok',kind==='ok');
    els.syncState.querySelector('span:last-child').textContent = text;
  }
  function showUnlock(error) { els.unlockPanel.classList.remove('hidden'); els.appContent.classList.add('hidden'); els.unlockError.textContent = error || ''; }
  function showApp() { els.unlockPanel.classList.add('hidden'); els.appContent.classList.remove('hidden'); }

  function unique(key,labelKey=key) {
    const map = new Map();
    for (const row of state.rows) {
      const value = row[key]; if (value === undefined || value === null || value === '') continue;
      map.set(String(value),String(row[labelKey] ?? value));
    }
    return [...map.entries()].sort((a,b)=>a[1].localeCompare(b[1]));
  }
  function fillSelect(select,entries,allLabel) {
    const old = select.value || '*';
    select.innerHTML = `<option value="*">${esc(allLabel)}</option>` + entries.map(([v,l])=>`<option value="${esc(v)}">${esc(l)}</option>`).join('');
    select.value = [...select.options].some(o=>o.value===old) ? old : '*';
  }
  function populateFilters() {
    fillSelect(els.deviceFilter,unique('device','deviceName'),'全部设备');
    fillSelect(els.clientFilter,unique('client'),'全部工具');
    fillSelect(els.modelFilter,unique('model'),'全部模型');
    fillSelect(els.upstreamFilter,unique('upstreamVendor'),'全部厂商');
    fillSelect(els.routeProviderFilter,unique('routeProvider'),'全部路由');
    fillSelect(els.routeTypeFilter,unique('routeType'),'全部类型');
    fillSelect(els.providerFilter,unique('provider'),'全部 Provider');
    const tiers = unique('tier'); fillSelect(els.tierFilter,tiers,'全部 Tier'); els.tierFilterLabel.classList.toggle('hidden',!tiers.length);
  }

  function iso(date) { return `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`; }
  function bounds() {
    const today = new Date(); today.setHours(0,0,0,0); let start=null,end=iso(today);
    if (state.range==='today') start=end;
    else if (state.range==='7d') { const d=new Date(today); d.setDate(d.getDate()-6); start=iso(d); }
    else if (state.range==='30d') { const d=new Date(today); d.setDate(d.getDate()-29); start=iso(d); }
    else if (state.range==='month') { start=iso(new Date(today.getFullYear(),today.getMonth(),1)); }
    else if (state.range==='all') end=null;
    else if (state.range==='custom') { start=state.customStart||null; end=state.customEnd||end; }
    return {start,end};
  }
  function filtered() {
    return A.filterRows(state.rows,{...bounds(),device:state.device,client:state.client,model:state.model,upstreamVendor:state.upstreamVendor,
      routeProvider:state.routeProvider,routeType:state.routeType,provider:state.provider,tier:state.tier});
  }

  function renderSummary(rows) {
    const s=A.sumRows(rows); els.mTotal.textContent=compact(s.totalTokens); els.mCost.textContent=money(s.costUsd); els.mInput.textContent=compact(s.input);
    els.mCache.textContent=compact(s.cacheRead); els.mOutput.textContent=compact(s.output); els.mMessages.textContent=integer(s.messages);
  }
  function renderRank(container,rows,dimension,metric='totalTokens',limit=8) {
    const items=A.groupRows(rows,dimension,metric).slice(0,limit), max=Math.max(1,...items.map(x=>x.value));
    container.innerHTML=items.length?items.map(item=>`<div class="rank-row"><span class="rank-name" title="${esc(item.key)}">${esc(item.key)}</span><span class="rank-track"><span class="rank-fill" style="width:${Math.max(1,item.value/max*100)}%"></span></span><span class="rank-value">${esc(fmt(metric,item.value))}</span></div>`).join(''):'<div class="empty">当前筛选无数据</div>';
  }

  const NS='http://www.w3.org/2000/svg';
  function node(tag,attrs={}) { const n=document.createElementNS(NS,tag); Object.entries(attrs).forEach(([k,v])=>n.setAttribute(k,String(v))); return n; }
  function tip() { let e=document.querySelector('.tooltip'); if(!e){e=document.createElement('div');e.className='tooltip hidden';document.body.appendChild(e);}return e; }
  function attachTip(n,text) { const t=tip(); n.addEventListener('pointermove',e=>{t.textContent=text;t.style.left=`${e.clientX+12}px`;t.style.top=`${e.clientY+12}px`;t.classList.remove('hidden');});n.addEventListener('pointerleave',()=>t.classList.add('hidden')); }
  function frame(container) { container.innerHTML=''; const w=Math.max(520,container.clientWidth-36),h=Math.max(240,container.clientHeight-36),root=node('svg',{viewBox:`0 0 ${w} ${h}`,preserveAspectRatio:'none'});container.appendChild(root);return{root,w,h,left:54,right:16,top:14,bottom:36}; }
  function axes(f,labels,max) {
    const iw=f.w-f.left-f.right,ih=f.h-f.top-f.bottom;
    for(let i=0;i<=4;i++){const y=f.top+ih*i/4;f.root.appendChild(node('line',{x1:f.left,y1:y,x2:f.w-f.right,y2:y,class:'grid-line'}));const t=node('text',{x:f.left-8,y:y+4,'text-anchor':'end',class:'axis-label'});t.textContent=compact(max*(1-i/4));f.root.appendChild(t);}
    const step=Math.max(1,Math.ceil(labels.length/8));labels.forEach((label,i)=>{if(i%step&&i!==labels.length-1)return;const x=labels.length<2?f.left+iw/2:f.left+iw*i/(labels.length-1);const t=node('text',{x,y:f.h-10,'text-anchor':'middle',class:'axis-label'});t.textContent=String(label).length>14?`${String(label).slice(0,12)}…`:label;f.root.appendChild(t);});return{iw,ih};
  }
  function lineChart(container,items,metric,area) {
    if(!items.length){container.innerHTML='<div class="empty">当前筛选无数据</div>';return;} const data=[...items].sort((a,b)=>a.key.localeCompare(b.key)),f=frame(container),max=Math.max(1,...data.map(x=>x.value)),{iw,ih}=axes(f,data.map(x=>x.key),max);
    const pts=data.map((item,i)=>({x:data.length<2?f.left+iw/2:f.left+iw*i/(data.length-1),y:f.top+ih*(1-item.value/max),item})),d=pts.map((p,i)=>`${i?'L':'M'}${p.x},${p.y}`).join(' ');
    if(area){const base=f.top+ih;f.root.appendChild(node('path',{d:`${d} L${pts.at(-1).x},${base} L${pts[0].x},${base} Z`,class:'chart-area'}));} f.root.appendChild(node('path',{d,class:'chart-line'}));
    pts.forEach(p=>{const c=node('circle',{cx:p.x,cy:p.y,r:3.5,fill:'var(--accent)'});attachTip(c,`${p.item.key} · ${fmt(metric,p.item.value)}`);f.root.appendChild(c);});
  }
  function barChart(container,items,metric) {
    if(!items.length){container.innerHTML='<div class="empty">当前筛选无数据</div>';return;} const data=items.slice(0,35).reverse(),f=frame(container),iw=f.w-f.left-f.right,ih=f.h-f.top-f.bottom,max=Math.max(1,...data.map(x=>x.value)),gap=4,bh=Math.max(3,(ih-gap*(data.length-1))/data.length);
    data.forEach((item,i)=>{const y=f.top+i*(bh+gap),r=node('rect',{x:f.left,y,width:Math.max(1,iw*item.value/max),height:bh,rx:2,class:'chart-bar'});attachTip(r,`${item.key} · ${fmt(metric,item.value)}`);f.root.appendChild(r);if(data.length<=16){const t=node('text',{x:f.left-8,y:y+bh/2+4,'text-anchor':'end',class:'axis-label'});t.textContent=item.key.length>12?`${item.key.slice(0,10)}…`:item.key;f.root.appendChild(t);}});
  }
  function stackedChart(container,rows,primary,secondary,metric) {
    if(primary===secondary) secondary=primary==='device'?'client':'device'; const m=A.groupMatrix(rows,primary,secondary,metric); if(!m.xValues.length){container.innerHTML='<div class="empty">当前筛选无数据</div>';return;}
    const xs=m.xValues.slice(primary==='date'?-45:0),stacks=m.stacks.slice(0,12),totals=xs.map(x=>stacks.reduce((s,k)=>s+m.value(x,k),0)),f=frame(container),max=Math.max(1,...totals),{iw,ih}=axes(f,xs,max),slot=iw/Math.max(1,xs.length),bw=Math.max(2,slot*.72);
    xs.forEach((x,i)=>{let cumulative=0;stacks.forEach((s,j)=>{const v=m.value(x,s);if(!v)return;const h=ih*v/max,y=f.top+ih-(ih*(cumulative+v)/max),r=node('rect',{x:f.left+i*slot+(slot-bw)/2,y,width:bw,height:Math.max(.8,h),fill:PALETTE[j%PALETTE.length],class:'chart-segment'});attachTip(r,`${x} · ${s} · ${fmt(metric,v)}`);f.root.appendChild(r);cumulative+=v;});});renderLegend(container,stacks);
  }
  function renderLegend(container,labels) { const box=document.createElement('div');box.className='legend';box.innerHTML=labels.slice(0,12).map((l,i)=>`<span class="legend-item"><i class="legend-dot" style="background:${PALETTE[i%PALETTE.length]}"></i>${esc(l)}</span>`).join('');container.appendChild(box); }
  function donutChart(container,items,metric) {
    const data=items.filter(x=>x.value>0).slice(0,12),total=data.reduce((s,x)=>s+x.value,0);if(!total){container.innerHTML='<div class="empty">当前筛选无数据</div>';return;}const f=frame(container),cx=f.w/2,cy=f.h/2,r=Math.min(f.w,f.h)*.31,sw=r*.36,circ=2*Math.PI*r;let acc=0;
    data.forEach((item,i)=>{const len=circ*item.value/total,c=node('circle',{cx,cy,r,fill:'none',stroke:PALETTE[i%PALETTE.length],'stroke-width':sw,'stroke-dasharray':`${len} ${circ-len}`,'stroke-dashoffset':-acc,transform:`rotate(-90 ${cx} ${cy})`});attachTip(c,`${item.key} · ${fmt(metric,item.value)} · ${(item.value/total*100).toFixed(1)}%`);f.root.appendChild(c);acc+=len;});const t=node('text',{x:cx,y:cy+6,'text-anchor':'middle',class:'donut-center'});t.textContent=fmt(metric,total);f.root.appendChild(t);renderLegend(container,data.map(x=>x.key));
  }
  function treemapChart(container,items,metric) {
    const data=items.filter(x=>x.value>0).slice(0,25);if(!data.length){container.innerHTML='<div class="empty">当前筛选无数据</div>';return;}const f=frame(container),rects=A.squarify(data,5,5,f.w-10,f.h-10);rects.forEach((it,i)=>{const r=node('rect',{x:it.x,y:it.y,width:Math.max(0,it.width-2),height:Math.max(0,it.height-2),rx:3,fill:PALETTE[i%PALETTE.length]});attachTip(r,`${it.key} · ${fmt(metric,it.value)}`);f.root.appendChild(r);if(it.width>75&&it.height>30){const t=node('text',{x:it.x+8,y:it.y+18,class:'treemap-label'});t.textContent=it.key.length>18?`${it.key.slice(0,16)}…`:it.key;f.root.appendChild(t);}});
  }

  function tableHtml(headers,rows) {
    return `<table class="data-table"><thead><tr>${headers.map(h=>`<th class="${h.number?'number':''}">${esc(h.label)}</th>`).join('')}</tr></thead><tbody>${rows.map(row=>`<tr>${headers.map(h=>`<td class="${h.number?'number':''}">${h.render? h.render(row[h.key],row):esc(row[h.key])}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  }
  function aggregateTable(rows,dimension,metric) {
    const items=A.groupRows(rows,dimension,metric);els.analysisTable.innerHTML=tableHtml([{key:'key',label:DIMENSIONS[dimension]||dimension},{key:'value',label:METRICS[metric]?.[0]||metric,number:true,render:v=>esc(fmt(metric,v))}],items);
  }
  function renderAnalysis(rows) {
    els.stackLabel.classList.toggle('hidden',state.chart!=='stacked'); els.analysisTable.classList.toggle('hidden',state.chart!=='table'); els.analysisChart.classList.toggle('hidden',state.chart==='table');
    const items=A.groupRows(rows,state.group,state.metric);
    if(state.chart==='line') lineChart(els.analysisChart,items,state.metric,false);
    else if(state.chart==='area') lineChart(els.analysisChart,items,state.metric,true);
    else if(state.chart==='bar') barChart(els.analysisChart,items,state.metric);
    else if(state.chart==='stacked') stackedChart(els.analysisChart,rows,state.group,state.stack,state.metric);
    else if(state.chart==='donut') donutChart(els.analysisChart,items,state.metric);
    else if(state.chart==='treemap') treemapChart(els.analysisChart,items,state.metric);
    else aggregateTable(rows,state.group,state.metric);
  }
  function renderDevices(rows) {
    const by=new Map();for(const ledger of state.ledgers){const d=ledger.device||{},deviceRows=rows.filter(r=>r.device===(d.id||'unknown')),s=A.sumRows(deviceRows);by.set(d.id||'unknown',{name:d.name||d.id||'Unknown',platform:d.platform||'unknown',arch:d.arch||'',updatedAt:ledger.generatedAt||'',tokens:s.totalTokens,cost:s.costUsd,messages:s.messages,scanMs:ledger.scanMs||0});}
    const data=[...by.values()].sort((a,b)=>b.tokens-a.tokens);els.deviceTable.innerHTML=tableHtml([
      {key:'name',label:'设备'},{key:'platform',label:'平台'},{key:'arch',label:'架构'},{key:'tokens',label:'Tokens',number:true,render:v=>esc(compact(v))},{key:'cost',label:'API 等价费用',number:true,render:v=>esc(money(v))},{key:'messages',label:'消息',number:true,render:v=>esc(integer(v))},{key:'updatedAt',label:'最近同步',render:v=>esc(v?new Date(v).toLocaleString():'—')},{key:'scanMs',label:'最近扫描',number:true,render:v=>esc(`${v} ms`)}
    ],data);
  }
  function renderRaw(rows) {
    const data=[...rows].sort((a,b)=>b.date.localeCompare(a.date)||a.deviceName.localeCompare(b.deviceName)).slice(0,2000);
    els.rawTable.innerHTML=tableHtml([
      {key:'date',label:'日期'},{key:'deviceName',label:'设备'},{key:'client',label:'工具'},{key:'upstreamVendor',label:'模型厂商'},{key:'routeProvider',label:'路由提供商'},{key:'routeType',label:'路由类型'},{key:'model',label:'模型'},{key:'tier',label:'Tier'},
      {key:'input',label:'Input',number:true,render:v=>esc(compact(v))},{key:'cacheRead',label:'Cache R',number:true,render:v=>esc(compact(v))},{key:'output',label:'Output',number:true,render:v=>esc(compact(v))},{key:'reasoning',label:'Reasoning',number:true,render:v=>esc(compact(v))},{key:'costUsd',label:'费用',number:true,render:v=>esc(money(v))}
    ],data);
  }

  function renderAll() {
    const rows=filtered();renderSummary(rows);
    if(state.view==='overview'){
      lineChart(els.overviewTrend,A.groupRows(rows,'date',state.overviewMetric),state.overviewMetric,true);renderRank(els.deviceBars,rows,'device');renderRank(els.routeBars,rows,'routeProvider');renderRank(els.modelBars,rows,'model');renderRank(els.clientBars,rows,'client');
    } else if(state.view==='analysis') renderAnalysis(rows); else if(state.view==='devices') renderDevices(rows); else if(state.view==='data') renderRaw(rows);
  }

  function selectView(view) {
    state.view=view;document.querySelectorAll('.nav-item').forEach(b=>b.classList.toggle('active',b.dataset.view===view));
    ['overview','analysis','devices','data'].forEach(v=>els[`${v}View`].classList.toggle('hidden',v!==view));
    const names={overview:'概览',analysis:'用量分析',devices:'设备',data:'数据'};els.viewTitle.textContent=names[view]||'Token Monitor';renderAll();
  }
  function metricOptions(select,selected) { select.innerHTML=Object.entries(METRICS).map(([k,[label]])=>`<option value="${k}">${esc(label)}</option>`).join('');select.value=selected; }

  function wire() {
    metricOptions(els.metricSelect,state.metric);metricOptions(els.overviewMetric,state.overviewMetric);
    document.querySelectorAll('.nav-item').forEach(b=>b.addEventListener('click',()=>selectView(b.dataset.view)));
    document.querySelectorAll('.jump-analysis').forEach(b=>b.addEventListener('click',()=>{state.group=b.dataset.group;els.groupSelect.value=state.group;state.chart=state.group==='date'?'line':'bar';els.chartSelect.value=state.chart;selectView('analysis');}));
    els.rangeButtons.addEventListener('click',e=>{const b=e.target.closest('button[data-range]');if(!b)return;state.range=b.dataset.range;els.rangeButtons.querySelectorAll('button').forEach(x=>x.classList.toggle('active',x===b));els.customRange.classList.toggle('hidden',state.range!=='custom');renderAll();});
    [[els.deviceFilter,'device'],[els.clientFilter,'client'],[els.modelFilter,'model'],[els.upstreamFilter,'upstreamVendor'],[els.routeProviderFilter,'routeProvider'],[els.routeTypeFilter,'routeType'],[els.providerFilter,'provider'],[els.tierFilter,'tier']].forEach(([el,key])=>el.addEventListener('change',()=>{state[key]=el.value;renderAll();}));
    els.startDate.addEventListener('change',()=>{state.customStart=els.startDate.value;renderAll();});els.endDate.addEventListener('change',()=>{state.customEnd=els.endDate.value;renderAll();});
    els.metricSelect.addEventListener('change',()=>{state.metric=els.metricSelect.value;renderAll();});els.overviewMetric.addEventListener('change',()=>{state.overviewMetric=els.overviewMetric.value;renderAll();});
    els.groupSelect.addEventListener('change',()=>{state.group=els.groupSelect.value;renderAll();});els.chartSelect.addEventListener('change',()=>{state.chart=els.chartSelect.value;renderAll();});els.stackSelect.addEventListener('change',()=>{state.stack=els.stackSelect.value;renderAll();});
    els.refreshButton.addEventListener('click',()=>loadAll().catch(err=>showUnlock(err.message)));
    els.unlockButton.addEventListener('click',()=>{state.key=els.keyInput.value.trim();if(!state.key)return;history.replaceState(null,'',`${location.pathname}${location.search}#key=${encodeURIComponent(state.key)}`);loadAll().catch(err=>showUnlock(err.message));});
    els.keyInput.addEventListener('keydown',e=>{if(e.key==='Enter')els.unlockButton.click();});
    els.exportCsv.addEventListener('click',()=>{const blob=new Blob([A.toCsv(filtered())],{type:'text/csv;charset=utf-8'}),url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=`token-monitor-${new Date().toISOString().slice(0,10)}.csv`;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);});
    els.themeToggle.addEventListener('click',()=>{const dark=document.documentElement.dataset.theme==='dark';document.documentElement.dataset.theme=dark?'light':'dark';localStorage.setItem('tm-theme',dark?'light':'dark');renderAll();});
    window.addEventListener('resize',()=>{clearTimeout(window.__tmResize);window.__tmResize=setTimeout(renderAll,120);});
  }

  async function boot() {
    state.repo=deriveRepo();state.key=parseFragment();const theme=localStorage.getItem('tm-theme');if(theme)document.documentElement.dataset.theme=theme;wire();
    if(!state.key){showUnlock('');return;}els.keyInput.value=state.key;
    try{await loadAll();}catch(error){showUnlock(error.message||String(error));setSync('error','读取失败');}
  }
  boot();
})();
