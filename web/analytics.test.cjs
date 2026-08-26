const assert = require('node:assert/strict');
const A = require('./analytics.js');

const rows = [
  {date:'2026-08-25',device:'mac',deviceName:'macos-a1b2c3',client:'codex',model:'gpt-5.6-sol',upstreamVendor:'openai',routeProvider:'openai',routeType:'official',provider:'openai',tier:'fast',input:10,cacheRead:90,cacheWrite:0,output:5,reasoning:1,messages:2,costUsd:1.5,planCostUsd:2.8,planCostAvailable:true,costLowerBound:true},
  {date:'2026-08-26',device:'win',deviceName:'windows-d4e5f6',client:'codex',model:'gpt-5.6-sol',upstreamVendor:'openai',routeProvider:'newapi',routeType:'relay',provider:'my-newapi',tier:'standard',input:20,cacheRead:80,cacheWrite:0,output:4,reasoning:0,messages:3,costUsd:2.5,planCostUsd:3.1,planCostAvailable:true,costLowerBound:false},
  {date:'2026-08-26',device:'linux',deviceName:'linux-112233',client:'claude',model:'claude-sonnet-4',upstreamVendor:'anthropic',routeProvider:'aws-bedrock',routeType:'cloud',provider:'bedrock',tier:null,input:30,cacheRead:0,cacheWrite:4,output:8,reasoning:0,messages:1,costUsd:3.5,planCostUsd:0,planCostAvailable:false,costLowerBound:false}
];

assert.equal(A.totalTokens(rows[0]), 106);
const sum=A.sumRows(rows);
assert.equal(sum.costUsd, 7.5);
assert.equal(sum.planCostUsd, 5.9);
assert.equal(sum.planCostAvailable, true);
assert.equal(sum.planCostIncomplete, false);
assert.equal(sum.costLowerBound, true);
assert.equal(A.filterRows(rows,{routeType:'relay'}).length,1);
assert.equal(A.filterRows(rows,{start:'2026-08-26',end:'2026-08-26'}).length,2);
assert.deepEqual(A.groupRows(rows,'device','totalTokens').map(x=>x.key).sort(),['linux-112233','macos-a1b2c3','windows-d4e5f6']);
assert.equal(A.groupRows(rows,'upstreamVendor','costUsd').find(x=>x.key==='openai').value,4);
assert.equal(A.groupRows(rows,'upstreamVendor','planCostUsd').find(x=>x.key==='openai').value,5.9);
const matrix=A.groupMatrix(rows,'date','routeProvider','costUsd');
assert.equal(matrix.value('2026-08-26','newapi'),2.5);
assert.ok(A.squarify([{key:'a',value:2},{key:'b',value:1}],0,0,300,100).length===2);
const csv=A.toCsv(rows);
assert.match(csv,/routeProvider/);
assert.match(csv,/planCostUsd/);
assert.match(csv,/planCostAvailable/);
assert.match(csv,/costLowerBound/);
assert.match(csv,/newapi/);
assert.doesNotMatch(csv,/sessions|durationMs|prompt/i);
console.log('dashboard analytics tests passed');
