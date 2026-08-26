'use strict';

const DEFAULT_API_BASE = 'https://api.github.com';

function sanitizeDeviceId(value) {
  const id = String(value || 'unknown-device').trim();
  return id.replace(/[^a-zA-Z0-9._-]+/g, '-').replace(/^-+|-+$/g, '') || 'unknown-device';
}

function normalizeRepo(value) {
  const repo = String(value || '').trim().replace(/^https?:\/\/github\.com\//, '').replace(/\.git$/, '').replace(/^\/+|\/+$/g, '');
  if (!/^[^/]+\/[^/]+$/.test(repo)) throw new Error('TOKEN_MONITOR_GITHUB_REPO must be owner/repo');
  return repo;
}

function encodePath(pathname) {
  return String(pathname || '').split('/').filter(Boolean).map(encodeURIComponent).join('/');
}

function githubHeaders(token) {
  const headers = {
    accept: 'application/vnd.github+json',
    'content-type': 'application/json',
    'x-github-api-version': '2022-11-28',
    'user-agent': 'token-monitor-github-ledger'
  };
  if (token) headers.authorization = `Bearer ${token}`;
  return headers;
}

async function githubRequest(fetchImpl, url, options, token) {
  const response = await fetchImpl(url, {
    ...options,
    headers: { ...githubHeaders(token), ...(options?.headers || {}) }
  });
  return response;
}

async function currentFileSha(fetchImpl, { apiBase = DEFAULT_API_BASE, repo, branch = 'main', path, token }) {
  const url = `${apiBase}/repos/${repo}/contents/${encodePath(path)}?ref=${encodeURIComponent(branch)}`;
  const response = await githubRequest(fetchImpl, url, { method: 'GET' }, token);
  if (response.status === 404) return '';
  if (!response.ok) throw new Error(`GitHub read failed ${response.status}: ${(await response.text()).slice(0, 300)}`);
  const json = await response.json();
  return String(json?.sha || '');
}

function ledgerDocument(summary, { deviceId } = {}) {
  const id = sanitizeDeviceId(deviceId || summary?.deviceId || summary?.id);
  return {
    schemaVersion: 1,
    kind: 'token-monitor-device-ledger',
    deviceId: id,
    updatedAt: new Date().toISOString(),
    summary
  };
}

async function putGithubLedgerSummary(fetchImpl, summary, {
  repo,
  token,
  branch = 'main',
  basePath = 'ledger/devices',
  apiBase = DEFAULT_API_BASE,
  deviceId
} = {}) {
  const normalizedRepo = normalizeRepo(repo);
  const id = sanitizeDeviceId(deviceId || summary?.deviceId || summary?.id);
  const path = `${String(basePath || 'ledger/devices').replace(/^\/+|\/+$/g, '')}/${id}.json`;
  const sha = await currentFileSha(fetchImpl, { apiBase, repo: normalizedRepo, branch, path, token });
  const document = ledgerDocument(summary, { deviceId: id });
  const body = {
    message: `chore(ledger): update ${id}`,
    content: Buffer.from(`${JSON.stringify(document, null, 2)}\n`, 'utf8').toString('base64'),
    branch
  };
  if (sha) body.sha = sha;

  const url = `${apiBase}/repos/${normalizedRepo}/contents/${encodePath(path)}`;
  const response = await githubRequest(fetchImpl, url, { method: 'PUT', body: JSON.stringify(body) }, token);
  if (!response.ok) throw new Error(`GitHub ledger write failed ${response.status}: ${(await response.text()).slice(0, 300)}`);
  const json = await response.json();
  return { ok: true, deviceId: id, path, commitSha: json?.commit?.sha || '', contentSha: json?.content?.sha || '' };
}

module.exports = {
  DEFAULT_API_BASE,
  sanitizeDeviceId,
  normalizeRepo,
  ledgerDocument,
  putGithubLedgerSummary
};
