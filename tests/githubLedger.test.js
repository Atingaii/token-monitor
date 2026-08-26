'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const {
  sanitizeDeviceId,
  normalizeRepo,
  ledgerDocument,
  putGithubLedgerSummary
} = require('../src/shared/githubLedger');

test('sanitizeDeviceId keeps ledger paths portable', () => {
  assert.equal(sanitizeDeviceId('MacBook Pro / M2'), 'MacBook-Pro-M2');
  assert.equal(sanitizeDeviceId('win_01'), 'win_01');
});

test('normalizeRepo accepts owner/repo and GitHub URLs', () => {
  assert.equal(normalizeRepo('Atingaii/token-monitor'), 'Atingaii/token-monitor');
  assert.equal(normalizeRepo('https://github.com/Atingaii/token-monitor.git'), 'Atingaii/token-monitor');
  assert.throws(() => normalizeRepo('token-monitor'), /owner\/repo/);
});

test('ledgerDocument preserves the Token Monitor summary without raw-session expansion', () => {
  const summary = { deviceId: 'mac', today: { totalTokens: 42, costUsd: 0.5 } };
  const doc = ledgerDocument(summary);
  assert.equal(doc.schemaVersion, 1);
  assert.equal(doc.kind, 'token-monitor-device-ledger');
  assert.equal(doc.deviceId, 'mac');
  assert.deepEqual(doc.summary, summary);
});

test('putGithubLedgerSummary creates a per-device file', async () => {
  const calls = [];
  const fakeFetch = async (url, options = {}) => {
    calls.push({ url, options });
    if ((options.method || 'GET') === 'GET') {
      return new Response('not found', { status: 404 });
    }
    return new Response(JSON.stringify({ commit: { sha: 'commit-1' }, content: { sha: 'blob-1' } }), {
      status: 201,
      headers: { 'content-type': 'application/json' }
    });
  };

  const result = await putGithubLedgerSummary(fakeFetch, {
    deviceId: 'Linux Server',
    today: { totalTokens: 100 },
    month: { totalTokens: 100 },
    allTime: { totalTokens: 100 }
  }, {
    repo: 'owner/ledger',
    token: 'secret',
    branch: 'main'
  });

  assert.equal(result.path, 'ledger/devices/Linux-Server.json');
  assert.equal(calls.length, 2);
  assert.match(calls[1].url, /repos\/owner\/ledger\/contents\/ledger\/devices\/Linux-Server\.json$/);
  const body = JSON.parse(calls[1].options.body);
  const written = JSON.parse(Buffer.from(body.content, 'base64').toString('utf8'));
  assert.equal(written.deviceId, 'Linux-Server');
  assert.equal(written.summary.today.totalTokens, 100);
});
