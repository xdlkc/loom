'use strict';

const $ = (s) => document.querySelector(s);
const SVG_NS = 'http://www.w3.org/2000/svg';

const PAGE_TITLES = {
  onboarding: '新手引导向导',
  providers: '模型厂商管理',
  integrations: '客户端同步与网络',
  overview: '调用概览',
  requests: '请求追踪记录',
  sessions: '会话轨迹工作台',
  settings: '采集与保留设置',
};

const HARNESS_LABELS = {
  pi: 'pi',
  opencode2: 'OpenCode V2',
  'claude-code': 'Claude Code',
  unknown: '未识别',
};

const STATUS_LABELS = {
  success: '成功',
  failed: '失败',
  partial: '不完整',
  cancelled: '已取消',
  interrupted: '服务中断',
  inflight: '进行中',
  untracked: '未经网关',
};

const state = {
  authenticated: false,
  page: 'onboarding',
  setup: null,
  providerFilter: 'all',
  testResults: {},
  offset: 0,
  groups: [],
  sessionOffset: 0,
  sessionSearch: '',
  sessionHarness: '',
  selectedSessionId: null,
  sessionListData: null,
  sessionDetailData: null,
  sessionView: 'conversation',
  sessionSteps: [],
  sessionStepSelection: {},
  inspectorTab: 'request',
  generation: 0,
  toastTimer: null,
  editingProviderId: null,
  providerUi: {},
  sessionDom: null,
  sessionLoadedId: null,
  sessionJump: null,
  sessionError: null,
  sessionLoadingId: null,
  sessionLoadToken: 0,
};

const num = (n) =>
  n == null
    ? '—'
    : new Intl.NumberFormat('zh-CN', {
        notation: n > 999999 ? 'compact' : 'standard',
        maximumFractionDigits: 1,
      }).format(n);
const pct = (n) => (n == null ? '—' : (n * 100).toFixed(1) + '%');
const ms = (n) =>
  n == null ? '—' : n >= 1000 ? (n / 1000).toFixed(2) + ' s' : Math.round(n) + ' ms';
const dateStr = (n) => (n ? new Date(n).toLocaleString('zh-CN', { hour12: false }) : '—');
const timeOnly = (n) =>
  n ? new Date(n).toLocaleTimeString('zh-CN', { hour12: false }) : '—';
const shortDate = (n) =>
  n
    ? new Date(n).toLocaleString('zh-CN', {
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false,
      })
    : '—';
// Elapsed time for timeline offsets: seconds under a minute, then two larger units.
const dur = (n) => {
  if (n == null) return '—';
  if (n < 60000) return `${(n / 1000).toFixed(1)} s`;
  const pad = (v) => String(v).padStart(2, '0');
  if (n < 3600000) return `${Math.floor(n / 60000)}m ${pad(Math.floor((n % 60000) / 1000))}s`;
  return `${Math.floor(n / 3600000)}h ${pad(Math.floor((n % 3600000) / 60000))}m`;
};
const harnessLabel = (h) => HARNESS_LABELS[h] || h || '未识别';

function el(tag, cls, text) {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text != null) node.textContent = String(text);
  return node;
}

function add(parent, ...children) {
  for (const child of children) {
    if (child != null && child !== false) parent.append(child);
  }
  return parent;
}

function svgEl(tag, attrs, text) {
  const node = document.createElementNS(SVG_NS, tag);
  if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      node.setAttribute(k, String(v));
    }
  }
  if (text != null) node.textContent = String(text);
  return node;
}

function btn(label, cls, fn) {
  const node = el('button', cls || 'btn', label);
  node.type = 'button';
  if (fn) node.addEventListener('click', fn);
  return node;
}

function pill(text, kind = '') {
  return el('span', kind ? `pill ${kind}` : 'pill', text);
}

const STATUS_KINDS = { inflight: 'pending', untracked: 'neutral' };
function statusPill(s) {
  const kind = ['failed', 'partial', 'cancelled', 'interrupted'].includes(s)
    ? 'bad'
    : STATUS_KINDS[s] || '';
  return pill(STATUS_LABELS[s] || s, kind);
}

function toast(message, kind = '') {
  const node = $('#toast');
  node.textContent = message;
  node.className = kind ? `toast ${kind}` : 'toast';
  node.hidden = false;
  clearTimeout(state.toastTimer);
  state.toastTimer = setTimeout(() => {
    node.hidden = true;
  }, 2400);
}

async function copyText(text, okMsg = '已复制到剪贴板') {
  try {
    await navigator.clipboard.writeText(text);
    toast(okMsg);
  } catch {
    toast('浏览器禁止自动复制，请手动选中文本复制');
  }
}

function showError(err) {
  const box = $('#error');
  box.textContent = err?.message || String(err) || '请求未能完成';
  box.hidden = false;
}

function clearError() {
  $('#error').hidden = true;
}

function showLoginGate() {
  state.authenticated = false;
  $('#login').hidden = false;
  $('#filters').hidden = true;
  $('#quick-sync').hidden = true;
  $('#content').className = 'stage-content';
  $('#content').replaceChildren();
  updateGatewayPill(null);
}

async function api(path, options = {}) {
  const response = await fetch(path, {
    credentials: 'same-origin',
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(options.headers || {}),
    },
  });
  if (response.status === 401) {
    showLoginGate();
    throw new Error('请通过桌面端 Loom.app 或运行 loom console 安全连接本机工作台。');
  }
  const data = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(data.message || data.error || '请求失败');
  }
  state.authenticated = true;
  $('#login').hidden = true;
  return data;
}

function updateGatewayPill(setup) {
  const box = $('#gateway-pill');
  const label = $('#gateway-pill-text');
  if (!setup) {
    box.className = 'health-pill';
    label.textContent = '等待登录连接';
    box.title = '';
    return;
  }
  const gw = setup.gateway || {};
  const readyCount = setup.ready_model_count || 0;
  box.className = `health-pill ${gw.healthy ? 'ok' : 'bad'}`;
  label.textContent = gw.healthy
    ? `网关运行中 · 127.0.0.1:${gw.port} · ${readyCount} 个可用模型`
    : `网关未响应 (127.0.0.1:${gw.port})`;
  // Narrow layouts collapse the pill to its status dot.
  box.title = label.textContent;
  $('#quick-sync').hidden = readyCount === 0;
}

async function loadSetup() {
  const setup = await api('/api/setup');
  state.setup = setup;
  updateGatewayPill(setup);
  return setup;
}

function qs(v) {
  return new URLSearchParams(
    Object.entries(v).filter(([, val]) => val !== '' && val != null),
  ).toString();
}

function filters() {
  const to = Date.now();
  return {
    from: to - Number($('#range').value) * 3600000,
    to,
    provider: $('#provider').value,
    model: $('#model').value,
    harness: $('#harness').value,
    status: state.page === 'requests' ? $('#status').value : '',
  };
}

function syncFilterOptions(groups) {
  const key = new Map(state.groups.map((x) => [`${x.provider}|${x.model}|${x.harness}`, x]));
  for (const g of groups) {
    key.set(`${g.provider}|${g.model}|${g.harness}`, g);
  }
  state.groups = [...key.values()];
  for (const [selector, field, title] of [
    ['#provider', 'provider', '全部供应商'],
    ['#model', 'model', '全部模型'],
  ]) {
    const select = $(selector);
    const current = select.value;
    select.replaceChildren(new Option(title, ''));
    [...new Set(state.groups.map((x) => x[field]).filter(Boolean))]
      .sort()
      .forEach((v) => select.add(new Option(v, v)));
    if ([...select.options].some((o) => o.value === current)) {
      select.value = current;
    }
  }
}

function tableEl(headers, rows) {
  const t = el('table');
  const head = el('thead');
  const htr = el('tr');
  for (const h of headers) add(htr, el('th', '', h));
  add(head, htr);
  const body = el('tbody');
  for (const row of rows) {
    const tr = el('tr');
    for (const cell of row) {
      add(
        tr,
        add(el('td'), cell instanceof Node ? cell : document.createTextNode(String(cell ?? '—'))),
      );
    }
    add(body, tr);
  }
  return add(t, head, body);
}

function paginationEl(offset, total, limit, onPage) {
  const div = el('div', 'pagination');
  const prev = btn('上一页', 'btn btn-sm', () => onPage(Math.max(0, offset - limit)));
  const next = btn('下一页', 'btn btn-sm', () => onPage(offset + limit));
  prev.disabled = offset === 0;
  next.disabled = offset + limit >= total;
  return add(
    div,
    el('span', '', `共 ${num(total)} 条 · 第 ${Math.floor(offset / limit) + 1} 页`),
    prev,
    next,
  );
}

function makeKeyInput(placeholder) {
  const wrap = el('div', 'key-input-wrap');
  const input = el('input');
  input.type = 'password';
  input.placeholder = placeholder;
  input.autocomplete = 'off';
  const eye = btn('显示', 'eye-toggle', () => {
    const hidden = input.type === 'password';
    input.type = hidden ? 'text' : 'password';
    eye.textContent = hidden ? '隐藏' : '显示';
  });
  add(wrap, input, eye);
  return { wrap, input };
}

function renderTestFeedback(pid) {
  const res = state.testResults[pid];
  if (!res) return null;
  const cls = res.ok ? 'ok' : 'bad';
  const box = el('div', `inline-feedback ${cls}`);
  const msg = el(
    'span',
    '',
    `${res.ok ? '✓' : '✗'} ${res.latency_ms != null ? `${res.latency_ms} ms · ` : ''}${res.message}`,
  );
  add(box, msg);
  if (!res.ok && ['connect_failed', 'timeout', 'dns_error', 'tls_error', 'proxy_invalid'].includes(res.category)) {
    add(
      box,
      btn('去配置本机代理 →', 'btn btn-sm', () => navigate('integrations')),
    );
  }
  return box;
}

async function runConnectivityTest(pid, modelId, triggerBtn) {
  const idleLabel = triggerBtn?.textContent;
  if (triggerBtn) {
    triggerBtn.disabled = true;
    triggerBtn.textContent = '检测中…';
  }
  try {
    const res = await api('/api/providers/test', {
      method: 'POST',
      body: JSON.stringify({ provider_id: pid, model_id: modelId || null }),
    });
    state.testResults[pid] = res;
    toast(res.ok ? '连通性检测通过' : '连通性检测发现异常', res.ok ? '' : 'bad');
    await refresh();
  } catch (e) {
    showError(e);
  } finally {
    // Without a re-render (for example on failure) the button would stay labelled as busy.
    if (triggerBtn) {
      triggerBtn.disabled = false;
      triggerBtn.textContent = idleLabel;
    }
  }
}

/* ==================== 1. 新手向导 Onboarding ==================== */

function renderClientCard(c) {
  const card = el('div', 'client-card');
  const badge = c.conflict
    ? pill('需人工处理冲突', 'bad')
    : c.synced
      ? pill(`已同步 · ${c.model_count} 个模型`, 'ok')
      : c.installed
        ? pill('待同步', 'warn')
        : pill('未检测到安装', 'neutral');
  add(
    card,
    add(el('div', 'client-card-head'), el('strong', '', c.name), badge),
    el('code', 'small-note', c.config_path),
    c.default_model
      ? el('div', 'small-note', `当前默认模型：${c.default_model}`)
      : null,
    c.conflict ? el('div', 'inline-feedback bad', c.conflict) : el('p', 'small-note', c.note),
  );
  return card;
}

async function renderOnboarding() {
  const setup = await loadSetup();
  const root = el('div', 'page-stack');
  const hasModels = setup.ready_model_count > 0;
  const anySynced = ['pi', 'opencode2', 'claude_code'].some((k) => setup.clients[k]?.synced);

  const stepper = add(
    el('div', 'stepper'),
    add(
      el('div', `step-banner ${hasModels ? 'done' : ''}`),
      el('span', 'step-idx', hasModels ? '✓' : '1'),
      add(
        el('div'),
        el('h3', '', '选择厂商并粘贴 API Key'),
        el('p', 'small-note', hasModels ? `已配置 ${setup.ready_model_count} 个可用模型` : '挑你已有的厂商填 Key，其余直接跳过'),
      ),
    ),
    add(
      el('div', `step-banner ${anySynced ? 'done' : ''}`),
      el('span', 'step-idx', anySynced ? '✓' : '2'),
      add(
        el('div'),
        el('h3', '', '自动同步本地编程工具'),
        el('p', 'small-note', anySynced ? '已写入本机客户端配置（仅引用 hub.key）' : '保存 Key 后自动同步 pi / OpenCode / Claude Code'),
      ),
    ),
    add(
      el('div', `step-banner ${hasModels && anySynced ? 'done' : ''}`),
      el('span', 'step-idx', hasModels && anySynced ? '✓' : '3'),
      add(
        el('div'),
        el('h3', '', '验证连通并开始调用'),
        el(
          'p',
          'small-note',
          hasModels && anySynced
            ? '调用会自动出现在「概览」；连通性可在「厂商」页随时复测'
            : '保存 Key 后自动检测连通性，随时可回工作台更换 Key 或切换模型',
        ),
      ),
    ),
  );
  add(root, stepper);

  const intro = add(
    el('div', 'intro-box'),
    add(
      el('div'),
      el('strong', '', '第 1 步：粘贴你拥有的模型厂商 API Key'),
      el(
        'p',
        '',
        'Loom 已内置主流厂商地址、协议路由与默认模型。没有某家账号无需填写；所有密钥仅保存在本机 0600 私有目录，绝不回传网络。',
      ),
    ),
    btn('+ 添加自定义 HTTPS 厂商', 'btn', () => openCustomDialog()),
  );
  add(root, intro);

  const grid = el('div', 'preset-grid');
  for (const p of setup.presets) {
    const card = el('div', `vendor-card ${p.has_key && p.enabled ? 'ready' : ''}`);
    const statusBadge = p.has_key
      ? p.enabled
        ? pill(`已就绪 · ${p.active_model_count} 模型`, '')
        : pill('已保存（当前停用）', 'warn')
      : pill('未配置', 'neutral');

    const head = add(
      el('div', 'vendor-head'),
      add(
        el('div'),
        el('div', 'vendor-title', p.name),
        add(
          el('div', 'vendor-meta'),
          el('span', 'small-note', p.region_hint),
          el('span', 'small-note', '·'),
          el('span', 'small-note', p.billing),
        ),
      ),
      statusBadge,
    );

    const pitfall = el('div', 'pitfall-box', `注意：${p.pitfall_hint}`);
    const linkRow = el('div', 'vendor-links');
    const getKeyLink = el('a', '', '前往官网获取 Key ↗');
    getKeyLink.href = p.key_url;
    getKeyLink.target = '_blank';
    getKeyLink.rel = 'noopener noreferrer';
    const docLink = el('a', '', '接入文档 ↗');
    docLink.href = p.docs_url;
    docLink.target = '_blank';
    docLink.rel = 'noopener noreferrer';
    add(linkRow, getKeyLink, docLink);

    const { wrap, input } = makeKeyInput(
      p.configured ? '已在本机保存（留空只测连通性，输入新 Key 可更换）' : '在此粘贴 API Key（隐藏输入）',
    );
    const form = el('form', 'key-form');
    const saveBtn = el(
      'button',
      'btn btn-primary',
      p.configured ? '保存 / 测试连通性' : '保存并测试连通性',
    );
    saveBtn.type = 'submit';
    add(form, wrap, saveBtn);

    const localHint = el('div', 'inline-feedback warn');
    localHint.hidden = true;

    form.addEventListener('submit', async (ev) => {
      ev.preventDefault();
      localHint.hidden = true;
      const keyVal = input.value.trim();
      if (!keyVal && !p.configured) {
        localHint.textContent = '请先粘贴 API Key；如果不用这家厂商，直接跳过即可。';
        localHint.hidden = false;
        input.focus();
        return;
      }
      const idleLabel = saveBtn.textContent;
      saveBtn.disabled = true;
      saveBtn.textContent = keyVal ? '保存并检测…' : '检测中…';
      try {
        let targetPid = p.provider_id;
        if (keyVal) {
          const saved = await api('/api/providers/preset', {
            method: 'POST',
            body: JSON.stringify({ preset_id: p.id, api_key: keyVal, enabled: true }),
          });
          targetPid = saved.provider_id;
          input.value = '';
          if (saved.sync_warning) toast(saved.sync_warning, 'warn');
        }
        if (targetPid) {
          const testRes = await api('/api/providers/test', {
            method: 'POST',
            body: JSON.stringify({ provider_id: targetPid }),
          });
          state.testResults[targetPid] = testRes;
          toast(
            testRes.ok ? `${p.name} 已配置并连通` : `${p.name} 已保存，但连通性待检查`,
            testRes.ok ? '' : 'warn',
          );
        }
        await refresh();
      } catch (err) {
        localHint.textContent = err.message || '保存失败';
        localHint.hidden = false;
      } finally {
        saveBtn.disabled = false;
        saveBtn.textContent = idleLabel;
      }
    });

    add(
      card,
      head,
      pitfall,
      linkRow,
      form,
      localHint,
      p.provider_id ? renderTestFeedback(p.provider_id) : null,
    );
    add(grid, card);
  }
  add(root, grid);

  const clientPanel = el('section', 'panel');
  const clientHead = add(
    el('div', 'section-bar'),
    add(
      el('div'),
      el('h2', '', '第 2 步：AI 编程客户端自动同步'),
      el('p', 'small-note', '保存厂商后自动生成或更新客户端配置，各客户端仅引用本机 hub.key。'),
    ),
    btn('立即同步全部客户端', 'btn btn-primary', async () => {
      try {
        await api('/api/sync', { method: 'POST', body: '{}' });
        toast('已完成本机客户端同步');
        await refresh();
      } catch (err) {
        showError(err);
      }
    }),
  );
  const cgrid = add(
    el('div', 'client-grid'),
    renderClientCard(setup.clients.pi),
    renderClientCard(setup.clients.opencode2),
    renderClientCard(setup.clients.claude_code),
  );
  add(clientPanel, clientHead, el('div', 'section-bar'), cgrid);
  add(root, clientPanel);

  const nextBar = add(
    el('div', 'intro-box'),
    add(
      el('div'),
      el('strong', '', hasModels ? `第 3 步：已就绪（${setup.ready_model_count} 个模型可用）` : '第 3 步：完成任意厂商配置后即可开始使用'),
      el(
        'p',
        '',
        `通用接入地址：OpenAI 兼容 ${setup.gateway.openai_base_url} · Anthropic 原生 ${setup.gateway.anthropic_base_url}`,
      ),
    ),
    add(
      el('div', 'provider-actions'),
      btn('管理厂商与模型明细', 'btn', () => navigate('providers')),
      btn('代理与接入参数', 'btn', () => navigate('integrations')),
      btn('查看调用概览 →', 'btn btn-primary', () => navigate('overview')),
    ),
  );
  add(root, nextBar);
  return root;
}

/* ==================== 2. 模型厂商管理中心 Providers ==================== */

function openCustomDialog(existingProvider = null) {
  const dlg = $('#provider-dialog');
  const form = $('#custom-provider-form');
  form.reset();
  $('#cf-error').hidden = true;
  if (existingProvider) {
    state.editingProviderId = existingProvider.id;
    $('#provider-dialog-title').textContent = `编辑厂商 · ${existingProvider.name}`;
    $('#cf-provider-id').value = existingProvider.id;
    $('#cf-prefix').value = existingProvider.prefix;
    $('#cf-prefix').readOnly = true;
    $('#cf-name').value = existingProvider.name;
    $('#cf-base-url').value = existingProvider.base_url;
    $('#cf-anthropic-url').value = existingProvider.anthropic_base_url || '';
    $('#cf-billing').value = existingProvider.billing || '独立 API 钱包';
    const localModels = (existingProvider.models || []).filter((m) => m.source === 'local');
    $('#cf-models').value = localModels.map((m) => m.id).join(', ');
    $('#cf-reasoning').checked = localModels.some((m) => m.reasoning);
    $('#cf-key-label').textContent = '轮换 API Key（留空表示保留当前已保存密钥）';
    $('#cf-key').placeholder = '留空不修改；粘贴新 Key 则覆盖轮换';
    $('#cf-submit').textContent = '保存修改并同步';
  } else {
    state.editingProviderId = null;
    $('#provider-dialog-title').textContent = '添加自定义 HTTPS 模型厂商';
    $('#cf-provider-id').value = '';
    $('#cf-prefix').readOnly = false;
    $('#cf-billing').value = '独立 API 钱包';
    $('#cf-key-label').textContent = 'API Key（仅保存在本机 0600 私有文件中）';
    $('#cf-key').placeholder = '粘贴推理 API Key';
    $('#cf-submit').textContent = '保存并同步客户端';
  }
  dlg.showModal();
}

function renderConfiguredProviderCard(p) {
  const card = el('article', 'provider-card');
  const badges = add(
    el('div', 'vendor-meta'),
    pill(`hub/${p.prefix}/*`, 'neutral'),
    pill(p.billing, 'neutral'),
    p.anthropic_supported ? pill('支持 Anthropic 原生', 'neutral') : null,
    p.has_key ? pill('Key 已安全保存', 'neutral') : pill('缺少 Key 文件', 'bad'),
    p.enabled ? pill(`已启用 · ${p.active_model_count} 模型`, '') : pill('已停用', 'warn'),
  );

  const modelSelect = el('select');
  modelSelect.setAttribute('aria-label', `${p.name} 测试模型`);
  for (const m of p.models.filter((x) => x.enabled)) {
    modelSelect.add(new Option(`${m.name} (${m.id})`, m.id));
  }
  if (!modelSelect.options.length && p.models.length) {
    modelSelect.add(new Option(p.models[0].id, p.models[0].id));
  }
  // Every action re-renders the page; keep per-provider control state outside the DOM.
  const ui = (state.providerUi[p.id] = state.providerUi[p.id] || {});
  if ([...modelSelect.options].some((o) => o.value === ui.testModel)) {
    modelSelect.value = ui.testModel;
  }
  modelSelect.addEventListener('change', () => {
    ui.testModel = modelSelect.value;
  });

  const testBtn = btn('测试连通性', 'btn btn-sm', () =>
    runConnectivityTest(p.id, modelSelect.value, testBtn),
  );
  const toggleBtn = btn(
    p.enabled ? '停用' : '启用',
    'btn btn-sm',
    async () => {
      try {
        await api('/api/providers/update', {
          method: 'POST',
          body: JSON.stringify({ provider_id: p.id, enabled: !p.enabled }),
        });
        toast(p.enabled ? `已停用 ${p.name}` : `已启用 ${p.name}`);
        await refresh();
      } catch (err) {
        showError(err);
      }
    },
  );
  const editBtn = !p.preset_id
    ? btn('编辑路由', 'btn btn-sm', () => openCustomDialog(p))
    : null;
  const delBtn = btn('删除', 'btn btn-sm btn-danger', async () => {
    if (!confirm(`确定移除「${p.name}」吗？旧配置与本机密钥将自动备份到 backups 目录。`)) {
      return;
    }
    try {
      await api('/api/providers/delete', {
        method: 'POST',
        body: JSON.stringify({ provider_id: p.id }),
      });
      delete state.testResults[p.id];
      toast(`已移除 ${p.name}`);
      await refresh();
    } catch (err) {
      showError(err);
    }
  });

  const head = add(
    el('div', 'provider-card-head'),
    add(
      el('div'),
      el('h3', 'vendor-title', p.name),
      el('div', 'small-note mono', p.base_url),
      badges,
    ),
    add(el('div', 'provider-actions'), modelSelect, testBtn, toggleBtn, editBtn, delBtn),
  );

  // Key rotation + bundled toggle
  const subrow = el('div', 'provider-subrow');
  const { wrap, input: rotateInput } = makeKeyInput('输入新 API Key 覆盖轮换（旧 Key 自动备份）');
  const rotateForm = el('form', 'key-form');
  const rotateBtn = el('button', 'btn btn-sm btn-primary', '更换 Key');
  rotateBtn.type = 'submit';
  add(rotateForm, wrap, rotateBtn);
  rotateForm.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const k = rotateInput.value.trim();
    if (!k) {
      toast('请先粘贴新的 API Key');
      rotateInput.focus();
      return;
    }
    rotateBtn.disabled = true;
    try {
      await api('/api/providers/update', {
        method: 'POST',
        body: JSON.stringify({ provider_id: p.id, api_key: k }),
      });
      rotateInput.value = '';
      toast(`已安全更换 ${p.name} 的 API Key`);
      await refresh();
    } catch (err) {
      showError(err);
    } finally {
      rotateBtn.disabled = false;
    }
  });
  add(subrow, rotateForm);

  if (p.supports_bundled_catalog) {
    const label = el('label', 'inline-check');
    const cb = el('input');
    cb.type = 'checkbox';
    cb.checked = p.include_bundled_models;
    cb.addEventListener('change', async () => {
      try {
        await api('/api/providers/update', {
          method: 'POST',
          body: JSON.stringify({
            provider_id: p.id,
            include_bundled_models: cb.checked,
          }),
        });
        toast(cb.checked ? '已开启公共目录补充' : '已仅保留本机自定义模型');
        await refresh();
      } catch (err) {
        showError(err);
      }
    });
    add(label, cb, el('span', '', '自动补充公共目录模型'));
    add(subrow, label);
  }

  // Models disclosure
  const modelsBox = el('details', 'models-box');
  modelsBox.open = !!ui.modelsOpen;
  modelsBox.addEventListener('toggle', () => {
    ui.modelsOpen = modelsBox.open;
  });
  add(
    modelsBox,
    el(
      'summary',
      'models-summary',
      `模型目录明细（共 ${p.models.length} 个 · 已启用 ${p.active_model_count} 个）`,
    ),
  );
  const tableWrap = el('div', 'models-table-wrap');
  const rows = p.models.map((m) => {
    const toggle = btn(
      m.enabled ? '停用' : '启用',
      'btn btn-sm',
      async () => {
        try {
          await api('/api/providers/update', {
            method: 'POST',
            body: JSON.stringify({
              provider_id: p.id,
              toggle_model: { id: m.id, enabled: !m.enabled },
            }),
          });
          await refresh();
        } catch (err) {
          showError(err);
        }
      },
    );
    const remove =
      m.source === 'local'
        ? btn('移除', 'btn btn-sm btn-danger', async () => {
            try {
              await api('/api/providers/update', {
                method: 'POST',
                body: JSON.stringify({ provider_id: p.id, remove_model_id: m.id }),
              });
              toast(`已移除模型 ${m.id}`);
              await refresh();
            } catch (err) {
              showError(err);
            }
          })
        : el('span', 'small-note', '公共目录');
    return [
      el('span', 'mono', `${p.prefix}/${m.id}`),
      m.name,
      `${num(m.contextWindow)} / ${num(m.maxTokens)}`,
      m.reasoning ? pill('支持思考', '') : pill('标准', 'neutral'),
      m.enabled ? pill('启用', '') : pill('停用', 'warn'),
      add(el('div', 'provider-actions'), toggle, remove),
    ];
  });
  add(
    tableWrap,
    tableEl(['客户端模型 ID', '名称', '上下文 / 输出上限', '思考能力', '状态', '操作'], rows),
  );

  // Add model mini-form
  const addForm = el('form', 'add-model-row');
  const idInput = el('input');
  idInput.type = 'text';
  idInput.placeholder = '新增模型 ID（如 glm-5.3）';
  idInput.required = true;
  const nameInput = el('input');
  nameInput.type = 'text';
  nameInput.placeholder = '显示名称（可选）';
  const maxInput = el('input');
  maxInput.type = 'number';
  maxInput.min = '1';
  maxInput.value = '8192';
  maxInput.setAttribute('aria-label', '最大输出 Token');
  const reasonWrap = el('label', 'inline-check');
  const reasonCb = el('input');
  reasonCb.type = 'checkbox';
  reasonCb.checked = true;
  add(reasonWrap, reasonCb, el('span', '', '支持思考'));
  const addBtn = el('button', 'btn btn-sm btn-primary', '+ 添加模型');
  addBtn.type = 'submit';
  add(addForm, idInput, nameInput, maxInput, reasonWrap, addBtn);
  addForm.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    try {
      await api('/api/providers/update', {
        method: 'POST',
        body: JSON.stringify({
          provider_id: p.id,
          add_model: {
            id: idInput.value.trim(),
            name: nameInput.value.trim() || idInput.value.trim(),
            maxTokens: Number(maxInput.value) || 8192,
            reasoning: reasonCb.checked,
          },
        }),
      });
      toast('已添加模型并同步客户端');
      await refresh();
    } catch (err) {
      showError(err);
    }
  });

  add(modelsBox, tableWrap, addForm);
  add(card, head, subrow, renderTestFeedback(p.id), modelsBox);
  return card;
}

async function renderProviders() {
  const setup = await loadSetup();
  const root = el('div', 'page-stack');
  const configured = setup.providers || [];
  const unconfigured = (setup.presets || []).filter((p) => !p.configured);

  const tabs = el('div', 'filter-tabs');
  for (const [id, label] of [
    ['all', `全部 (${configured.length + unconfigured.length})`],
    ['configured', `已配置 (${configured.length})`],
    ['unconfigured', `未配置预设 (${unconfigured.length})`],
  ]) {
    const b = btn(label, `filter-tab ${state.providerFilter === id ? 'active' : ''}`, () => {
      state.providerFilter = id;
      refresh();
    });
    add(tabs, b);
  }

  const bar = add(
    el('div', 'section-bar'),
    add(el('div', 'section-bar-left'), tabs),
    add(
      el('div', 'provider-actions'),
      btn('打开新手引导', 'btn', () => navigate('onboarding')),
      btn('+ 添加自定义 HTTPS 厂商', 'btn btn-primary', () => openCustomDialog()),
    ),
  );
  add(root, bar);

  if (state.providerFilter !== 'unconfigured') {
    const stack = el('div', 'provider-stack');
    if (configured.length) {
      for (const p of configured) {
        add(stack, renderConfiguredProviderCard(p));
      }
    } else {
      add(
        stack,
        add(
          el('div', 'panel empty'),
          el('p', '', '尚未配置任何模型厂商。'),
          btn('前往新手向导一键填写 API Key →', 'btn btn-primary', () => navigate('onboarding')),
        ),
      );
    }
    add(root, stack);
  }

  if (state.providerFilter !== 'configured' && unconfigured.length) {
    const sec = el('section', 'panel');
    add(sec, el('h2', '', '更多可一键接入的预设厂商'));
    const grid = el('div', 'preset-grid');
    for (const p of unconfigured) {
      const card = el('div', 'vendor-card');
      const { wrap, input } = makeKeyInput(`粘贴 ${p.name} API Key`);
      const form = el('form', 'key-form');
      const submit = el('button', 'btn btn-primary', '添加');
      submit.type = 'submit';
      add(form, wrap, submit);
      form.addEventListener('submit', async (ev) => {
        ev.preventDefault();
        if (!input.value.trim()) {
          input.focus();
          return;
        }
        submit.disabled = true;
        try {
          const saved = await api('/api/providers/preset', {
            method: 'POST',
            body: JSON.stringify({ preset_id: p.id, api_key: input.value.trim(), enabled: true }),
          });
          toast(`已添加 ${p.name}`);
          if (saved.provider_id) {
            state.testResults[saved.provider_id] = await api('/api/providers/test', {
              method: 'POST',
              body: JSON.stringify({ provider_id: saved.provider_id }),
            });
          }
          await refresh();
        } catch (err) {
          showError(err);
        } finally {
          submit.disabled = false;
        }
      });
      const link = el('a', 'small-note', '前往官网获取 Key ↗');
      link.href = p.key_url;
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
      add(
        card,
        add(el('div', 'vendor-head'), el('strong', '', p.name), pill(p.region_hint, 'neutral')),
        el('div', 'small-note', `注意：${p.pitfall_hint}`),
        link,
        form,
      );
      add(grid, card);
    }
    add(sec, grid);
    add(root, sec);
  }

  return root;
}

/* ==================== 3. 客户端同步与网络 Integrations ==================== */

async function renderIntegrations() {
  const setup = await loadSetup();
  const root = el('div', 'page-stack');

  const clientPanel = el('section', 'panel');
  add(
    clientPanel,
    add(
      el('div', 'section-bar'),
      add(
        el('div'),
        el('h2', '', '本地 AI 编程客户端同步'),
        el('p', 'small-note', 'Loom 后台每 2 秒也会自动检查配置变更并保持同步；保留你原有的 OAuth、主题与权限模式。'),
      ),
      btn('立即同步全部客户端', 'btn btn-primary', async () => {
        try {
          await api('/api/sync', { method: 'POST', body: '{}' });
          toast('已完成全部客户端同步');
          await refresh();
        } catch (err) {
          showError(err);
        }
      }),
    ),
    el('div', 'section-bar'),
    add(
      el('div', 'client-grid'),
      renderClientCard(setup.clients.pi),
      renderClientCard(setup.clients.opencode2),
      renderClientCard(setup.clients.claude_code),
    ),
  );
  add(root, clientPanel);

  const twoCol = el('div', 'two-col');

  // Outbound Proxy Card
  const proxyPanel = el('section', 'panel');
  const proxyOn = setup.proxy?.configured;
  const proxyInput = el('input');
  proxyInput.type = 'url';
  proxyInput.id = 'proxy-url-input';
  proxyInput.placeholder = '例如：http://127.0.0.1:7890';
  proxyInput.autocomplete = 'off';

  const quickPorts = el('div', 'chip-row');
  for (const [label, url] of [
    ['Clash / Mihomo (:7890)', 'http://127.0.0.1:7890'],
    ['Surge (:6152)', 'http://127.0.0.1:6152'],
    ['V2Ray / Sing-box (:10808)', 'http://127.0.0.1:10808'],
  ]) {
    add(
      quickPorts,
      btn(label, 'btn btn-sm', () => {
        proxyInput.value = url;
        proxyInput.focus();
      }),
    );
  }

  const proxyForm = el('form', 'key-form');
  const saveProxyBtn = el('button', 'btn btn-primary', '保存持久代理');
  saveProxyBtn.type = 'submit';
  const offProxyBtn = btn('恢复直连', 'btn', async () => {
    try {
      await api('/api/proxy', { method: 'POST', body: JSON.stringify({ url: 'off' }) });
      proxyInput.value = '';
      toast('已切换为直连模式');
      await refresh();
    } catch (err) {
      showError(err);
    }
  });
  offProxyBtn.disabled = !proxyOn;
  add(proxyForm, proxyInput, saveProxyBtn, offProxyBtn);
  proxyForm.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const url = proxyInput.value.trim();
    if (!url) {
      proxyInput.focus();
      return;
    }
    try {
      await api('/api/proxy', { method: 'POST', body: JSON.stringify({ url }) });
      proxyInput.value = '';
      toast('已保存持久代理，下一条请求立即生效');
      await refresh();
    } catch (err) {
      showError(err);
    }
  });

  add(
    proxyPanel,
    add(
      el('div', 'client-card-head'),
      el('h2', '', '本机出站网络与代理'),
      proxyOn ? pill('已启用持久代理', '') : pill('当前直连', 'neutral'),
    ),
    el(
      'p',
      'small-note',
      '为保证自启动后台守护进程稳定工作，Loom 显式忽略终端 HTTP_PROXY 环境变量。需要经本地代理访问海外厂商时，请在此保存固定端口（不接受含用户名密码的代理地址，且不回显完整 URL）。',
    ),
    el('div', 'section-bar'),
    quickPorts,
    proxyForm,
  );

  // Universal Endpoint Guide Card
  const endpointPanel = el('section', 'panel');
  const firstProvider = (setup.providers || []).find((p) => p.enabled && p.active_model_count > 0);
  const firstModel = firstProvider?.models?.find((m) => m.enabled);
  const sampleModelId = firstProvider && firstModel
    ? `${firstProvider.prefix}/${firstModel.id}`
    : 'deepseek/deepseek-flash';

  const kv = el('dl', 'kv-list');
  const addKv = (k, val, copyable = true) => {
    add(kv, el('dt', '', k));
    const dd = el('dd');
    add(dd, el('code', '', val));
    if (copyable) add(dd, btn('复制', 'btn btn-sm', () => copyText(val)));
    add(kv, dd);
  };
  addKv('OpenAI Base URL', setup.gateway.openai_base_url);
  addKv('Anthropic Base URL', setup.gateway.anthropic_base_url);
  addKv('示例模型 ID', sampleModelId);
  addKv('本机入口密钥位置', setup.hub_key_hint, false);

  add(
    endpointPanel,
    el('h2', '', '其他客户端通用接入参数'),
    el('p', 'small-note', '任何支持 OpenAI 或 Anthropic 接口的工具（如 Cursor、OpenClaw 等），填入以下本机回环地址与 hub.key 内容即可复用全部已配置模型。'),
    el('div', 'section-bar'),
    kv,
  );

  add(twoCol, proxyPanel, endpointPanel);
  add(root, twoCol);

  const appCard = el('section', 'panel');
  add(
    appCard,
    add(
      el('div', 'client-card-head'),
      el('h2', '', '桌面端与终端命令行辅助'),
      setup.desktop_app?.supported
        ? setup.desktop_app.installed
          ? pill('macOS Loom.app 已安装', '')
          : pill('macOS 支持桌面应用', 'warn')
        : pill('当前为 Linux 环境', 'neutral'),
    ),
    setup.desktop_app?.supported
      ? add(
          el('p', 'small-note'),
          '在 macOS 上可通过 ~/Applications/Loom.app 在状态栏快速开关后台服务并免命令直达此工作台；也可在终端运行 ',
          el('code', '', 'loom'),
          ' 打开交互菜单，或运行 ',
          el('code', '', 'loom doctor --ping'),
          ' 一键体检。',
        )
      : add(
          el('p', 'small-note'),
          '随时可在终端运行 ',
          el('code', '', 'loom'),
          ' 进入交互式控制菜单、',
          el('code', '', 'loom doctor --ping'),
          ' 实测厂商连通性，或运行 ',
          el('code', '', 'loom console'),
          ' 打开此页面。',
        ),
  );
  add(root, appCard);

  return root;
}

/* ==================== 4. 流量概览 Overview ==================== */

// Ticks at 1 / 2 / 5 × 10^n so gridlines read as round numbers instead of quarters of the maximum.
function niceScale(max) {
  if (!(max > 0)) return { max: 4, step: 1 };
  const rough = max / 4;
  const magnitude = 10 ** Math.floor(Math.log10(rough));
  const fraction = rough / magnitude;
  const step = (fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10) * magnitude;
  return { max: step * 4, step };
}

// One unit for a whole axis: mixing "129万" with "964,743" reads as different quantities.
function axisNumber(n, max) {
  if (!n) return '0';
  if (max >= 1e8) return `${Number((n / 1e8).toFixed(2))}亿`;
  if (max >= 1e5) return `${Number((n / 1e4).toFixed(1))}万`;
  return num(Math.round(n));
}

function bucketLabel(timestamp, bucketMs) {
  const date = new Date(timestamp);
  const options =
    bucketMs === 86400000
      ? { month: 'short', day: 'numeric' }
      : { month: 'short', day: 'numeric', hour: '2-digit' };
  if (date.getMinutes()) options.minute = '2-digit';
  return date.toLocaleString('zh-CN', options);
}

// Input dwarfs output in agent traffic, so each series gets its own band and scale.
function tokenChart(series, bucketMs) {
  if (!series.length || series.every((s) => !s.requests)) {
    return el('div', 'empty', '当前时间窗口内暂无经过网关的调用，发起一次对话后这里会自动绘制 Token 曲线。');
  }
  if (series.every((s) => s.input_tokens == null && s.output_tokens == null)) {
    return el('div', 'empty', '暂无已返回 usage 的网关调用。客户端发过一次对话后这里会自动绘制 Token 曲线。');
  }
  const X0 = 64;
  const X1 = 786;
  const HEIGHT = 332;
  const bands = [
    {
      field: 'input_tokens',
      label: '输入 Tokens',
      top: 24,
      bottom: 134,
      line: 'chart-line',
      dot: 'chart-dot-in',
    },
    {
      field: 'output_tokens',
      label: '输出 Tokens',
      top: 190,
      bottom: 300,
      line: 'chart-line chart-output',
      dot: 'chart-dot-out',
    },
  ];
  const svg = svgEl('svg', {
    viewBox: `0 0 800 ${HEIGHT}`,
    class: 'chart-svg',
    role: 'img',
    'aria-label': '输入与输出 Token 随时间变化曲线',
  });
  const xAt = (i) => X0 + (i * (X1 - X0)) / Math.max(1, series.length - 1);
  for (const band of bands) {
    const { max, step } = niceScale(Math.max(...series.map((s) => s[band.field] || 0)));
    const yAt = (n) => band.bottom - ((n || 0) * (band.bottom - band.top)) / max;
    for (let tick = 0; tick <= 4; tick++) {
      const y = yAt(tick * step);
      svg.append(
        svgEl('line', { x1: X0, x2: X1, y1: y, y2: y, class: 'chart-grid' }),
        svgEl('text', { x: 4, y: y + 4, class: 'chart-label' }, axisNumber(tick * step, max)),
      );
    }
    svg.append(svgEl('text', { x: X0, y: band.top - 8, class: 'chart-title' }, band.label));
    svg.append(
      svgEl(
        'path',
        {
          d: series
            .map((s, i) => `${i ? 'L' : 'M'}${xAt(i).toFixed(1)} ${yAt(s[band.field]).toFixed(1)}`)
            .join(' '),
          class: band.line,
        },
      ),
    );
    if (series.length <= 60) {
      series.forEach((s, i) => {
        svg.append(
          svgEl('circle', {
            cx: xAt(i).toFixed(1),
            cy: yAt(s[band.field]).toFixed(1),
            r: 3,
            class: band.dot,
          }),
        );
      });
    }
  }

  const labels = Math.min(series.length > 40 ? 4 : 6, series.length);
  const shown = new Set(
    Array.from({ length: labels }, (_, k) =>
      Math.round((k * (series.length - 1)) / Math.max(1, labels - 1)),
    ),
  );
  series.forEach((s, i) => {
    if (!shown.has(i)) return;
    svg.append(
      svgEl(
        'text',
        {
          x: xAt(i).toFixed(1),
          y: HEIGHT - 6,
          'text-anchor': i === 0 ? 'start' : i === series.length - 1 ? 'end' : 'middle',
          class: 'chart-label',
        },
        bucketLabel(s.timestamp, bucketMs),
      ),
    );
  });

  // Transparent hit columns drive a readout; a native tooltip needs hovering a 3px dot first.
  const guide = svgEl('line', {
    x1: X0,
    x2: X0,
    y1: 14,
    y2: 304,
    class: 'chart-guide',
    visibility: 'hidden',
  });
  const readout = el('div', 'chart-readout');
  const show = (i) => {
    const s = series[i];
    if (!s) return;
    guide.setAttribute('x1', xAt(i));
    guide.setAttribute('x2', xAt(i));
    guide.setAttribute('visibility', 'visible');
    readout.replaceChildren(
      el('strong', '', bucketLabel(s.timestamp, bucketMs)),
      el('span', '', `输入 ${num(s.input_tokens)}`),
      el('span', '', `输出 ${num(s.output_tokens)}`),
      el('span', '', `请求 ${num(s.requests)}`),
    );
  };
  const width = (X1 - X0) / Math.max(1, series.length - 1);
  series.forEach((_, i) => {
    const hit = svgEl('rect', {
      x: xAt(i) - width / 2,
      y: 10,
      width,
      height: 300,
      class: 'chart-hit',
    });
    hit.addEventListener('mouseenter', () => show(i));
    svg.append(hit);
  });
  svg.append(guide);
  show(series.length - 1);
  return add(el('div', 'chart-box'), svg, readout);
}

async function renderOverview() {
  const v = await api('/api/overview?' + qs(filters()));
  syncFilterOptions(v.groups);
  const s = v.summary;
  const root = el('div', 'page-stack');

  const cards = add(
    el('div', 'cards'),
    add(
      el('div', 'card'),
      el('small', '', 'Token 总用量'),
      el(
        'div',
        'number',
        s.input_tokens == null && s.output_tokens == null
          ? '—'
          : num((s.input_tokens || 0) + (s.output_tokens || 0)),
      ),
      el(
        'div',
        'small-note',
        `输入 ${num(s.input_tokens)} / 输出 ${num(s.output_tokens)} · 已观测 usage ${num(s.usage_observed)}/${num(s.requests)} 次`,
      ),
    ),
    add(
      el('div', 'card'),
      el('small', '', '请求数'),
      el('div', 'number', num(s.requests)),
      el('div', 'small-note', `成功 ${num(s.success)} · 异常 ${num(s.errors)}`),
    ),
    add(
      el('div', 'card'),
      el('small', '', 'Token 加权缓存命中率'),
      el('div', 'number', pct(s.cache_hit_rate)),
      el('div', 'small-note', `读 ${num(s.cache_read_tokens)} / 写 ${num(s.cache_write_tokens)} · 已观测 ${num(s.cache_observed)} 次`),
    ),
    add(
      el('div', 'card'),
      el('small', '', '平均首 Token 延迟'),
      el('div', 'number', ms(s.avg_ttft_ms)),
      el('div', 'small-note', `请求耗时 P50 ${ms(v.p50_duration_ms)} · P95 ${ms(v.p95_duration_ms)}`),
    ),
  );
  add(root, cards);

  const panels = el('div', 'panels');
  const trendPanel = add(
    el('section', 'panel'),
    el('h2', '', 'Token 消耗趋势'),
    tokenChart(v.series, v.bucket_ms),
  );

  const byProvider = new Map();
  for (const g of v.groups) {
    const k = g.provider || '未路由';
    byProvider.set(k, (byProvider.get(k) || 0) + (g.input_tokens || 0) + (g.output_tokens || 0));
  }
  // A distribution is read top-down: rank it and show each share.
  const ranked = [...byProvider].sort((a, b) => b[1] - a[1]);
  const providerTotal = ranked.reduce((sum, [, n]) => sum + n, 0);
  const distPanel = add(
    el('section', 'panel'),
    el('h2', '', '供应商分布'),
    ranked.length
      ? tableEl(
          ['供应商', '占比', 'Tokens'],
          ranked.map(([p, n]) => [
            p,
            shareBar(providerTotal ? n / providerTotal : null),
            num(n),
          ]),
        )
      : el('div', 'empty', '尚无经过 Loom 的网关调用'),
    el('p', 'small-note', '仅统计经过本机网关的真实调用。缺失 usage 保持未知（—）；厂商余额与历史导入不混入此处。'),
  );
  add(panels, trendPanel, distPanel);
  add(root, panels);

  const modelPanel = add(
    el('section', 'panel'),
    el('h2', '', '模型与 Harness 明细'),
    v.groups.length
      ? tableEl(
          ['模型', '供应商 / Harness', '输入 / 输出', '缓存读 / 写', '命中率', '成功 / 请求', '平均首 Token'],
          v.groups.map((g) => [
            btn(g.model || '未识别', 'link-btn', () => {
              $('#model').value = g.model || '';
              navigate('requests');
            }),
            `${g.provider || '未路由'} / ${harnessLabel(g.harness)}`,
            `${num(g.input_tokens)} / ${num(g.output_tokens)}`,
            `${num(g.cache_read_tokens)} / ${num(g.cache_write_tokens)}`,
            pct(g.cache_hit_rate),
            `${num(g.success)} / ${num(g.requests)}`,
            ms(g.avg_ttft_ms),
          ]),
        )
      : el('div', 'empty', '等待首次调用'),
  );
  add(root, modelPanel);
  return root;
}

/* ==================== 5. 请求记录 Requests ==================== */

function timingBarSvg(waitMs, totalMs, isErr = false) {
  const svg = svgEl('svg', {
    viewBox: '0 0 1000 14',
    preserveAspectRatio: 'none',
    class: 'timing-svg',
    role: 'img',
    'aria-label': `首 Token 等待 ${ms(waitMs)}，总计 ${ms(totalMs)}`,
  });
  svg.append(svgEl('rect', { x: 0, y: 0, width: 1000, height: 14, rx: 4, class: 'svg-track-bg' }));
  if (totalMs && totalMs > 0) {
    const ratio = Math.min(1000, Math.max(4, Math.round(((waitMs || totalMs) / totalMs) * 1000)));
    svg.append(
      svgEl('rect', {
        x: 0,
        y: 0,
        width: ratio,
        height: 14,
        rx: 4,
        class: isErr ? 'svg-err-bar' : 'svg-wait-bar',
      }),
    );
  }
  return svg;
}

// SVG rather than an inline width: the console CSP allows no inline styles.
function shareBar(ratio) {
  const svg = svgEl('svg', {
    viewBox: '0 0 100 10',
    preserveAspectRatio: 'none',
    class: 'share-svg',
    role: 'img',
    'aria-label': `占比 ${pct(ratio)}`,
  });
  svg.append(svgEl('rect', { x: 0, y: 0, width: 100, height: 10, rx: 5, class: 'svg-track-soft' }));
  if (ratio) {
    svg.append(
      svgEl('rect', {
        x: 0,
        y: 0,
        width: Math.max(1, ratio * 100),
        height: 10,
        rx: 5,
        class: 'svg-share-bar',
      }),
    );
  }
  return add(el('div', 'share'), svg, el('span', '', pct(ratio)));
}

// List rows repeat one date dozens of times; keep the full timestamp on hover.
function timeCell(ts) {
  const node = el('span', '', shortDate(ts));
  node.title = dateStr(ts);
  return node;
}

function showDetail(title, content) {
  $('#detail-title').textContent = title;
  $('#detail-content').replaceChildren(content);
  if (!$('#detail').open) $('#detail').showModal();
}

function rawFold(title, data, open = false) {
  const d = el('details', 'fold');
  d.open = open;
  const text = typeof data === 'string' ? data : JSON.stringify(data, null, 2);
  return add(
    d,
    el('summary', '', title),
    add(el('div', 'section-bar'), el('span'), btn('复制 JSON', 'btn btn-sm', () => copyText(text))),
    el('pre', 'code-block', text),
  );
}

async function openRequestDetail(id) {
  try {
    const r = await api('/api/requests/' + encodeURIComponent(id));
    const root = el('div', 'page-stack');
    const kv = el('dl', 'kv-list');
    for (const [k, v] of [
      ['供应商 / 模型', `${r.provider || '—'} / ${r.model || '—'}`],
      ['Harness / 协议', `${harnessLabel(r.harness)} / ${r.protocol}${r.streaming ? ' · SSE' : ''}`],
      ['状态 / HTTP', `${STATUS_LABELS[r.status] || r.status} / ${r.http}`],
      ['开始 / 总耗时', `${dateStr(r.started)} / ${ms(r.duration_ms)}`],
      ['输入 / 输出 Tokens', `${num(r.usage?.input)} / ${num(r.usage?.output)}`],
      ['缓存读 / 写', `${num(r.usage?.cache_read)} / ${num(r.usage?.cache_write)}`],
      ['首字节 / 首 Token', `${ms(r.first_byte_ms)} / ${ms(r.ttft_ms)}`],
    ]) {
      add(kv, el('dt', '', k), el('dd', 'mono', v));
    }
    kv.prepend(
      el('dt', '', '请求 ID'),
      add(el('dd'), el('span', 'mono', r.id), btn('复制', 'btn btn-sm', () => copyText(r.id))),
    );
    add(root, kv);

    if (r.duration_ms != null) {
      add(root, timingBarSvg(r.ttft_ms, r.duration_ms, r.status !== 'success'));
    }
    if (r.session) {
      add(
        root,
        btn('跳转到所属会话轨迹 →', 'btn btn-primary btn-sm', () => {
          $('#detail').close();
          jumpToSession(r.session);
        }),
      );
    }
    if (r.error) add(root, el('div', 'inline-feedback bad', r.error));
    if (r.truncated || r.parse_errors) {
      add(
        root,
        el(
          'p',
          'small-note',
          `观测正文${r.truncated ? '超过 16 MiB 保留上限，已标记截断' : ''} · 解析异常 ${r.parse_errors}，推理原始字节仍原样透传。`,
        ),
      );
    }

    const stages = el('div', 'stages-row');
    for (const s of r.spans || []) {
      add(stages, el('span', 'stage-chip', `${s.stage} · ${ms(s.ms)}`));
    }
    add(
      root,
      stages,
      rawFold('客户端 → Loom', r.request, true),
      rawFold('Loom → 上游（凭据已剥离）', r.forwarded),
      rawFold('上游 → 客户端 · 完整响应 / SSE 事件', r.response),
    );
    showDetail(`${r.model || '未识别模型'} · ${dateStr(r.started)}`, root);
  } catch (err) {
    showError(err);
  }
}

async function renderRequests(f) {
  const v = await api('/api/requests?' + qs({ ...f, offset: state.offset }));
  const panel = el('section', 'panel');
  add(
    panel,
    el('h2', '', `请求追踪 · 共 ${num(v.total)} 条`),
    v.items.length
      ? tableEl(
          ['时间', '模型', '供应商', 'Harness', '状态', '输入 / 输出', '首 Token / 耗时', '所属会话'],
          v.items.map((r) => [
            timeCell(r.started),
            btn(r.model || r.id, 'link-btn mono', () => openRequestDetail(r.id)),
            r.provider || '未路由',
            harnessLabel(r.harness),
            statusPill(r.status),
            `${num(r.input)} / ${num(r.output)}`,
            `${ms(r.ttft_ms)} / ${ms(r.duration_ms)}`,
            r.session
              ? btn('打开轨迹 →', 'link-btn', () => jumpToSession(r.session))
              : el('span', 'small-note', '未关联'),
          ]),
        )
      : el('div', 'empty', '当前筛选条件下暂无网关请求'),
    paginationEl(state.offset, v.total, 50, (next) => {
      state.offset = next;
      refresh();
    }),
  );
  return panel;
}

/* ==================== 6. 三栏会话轨迹工作台 Sessions ==================== */

function extractReadablePayload(v) {
  if (!v) return '';
  const m = v.message || v;
  if (typeof m.content === 'string') return m.content;
  if (typeof v.text === 'string') return v.text;
  const parts = m.content || m.parts || v.parts;
  if (Array.isArray(parts)) {
    return parts
      .map((p) => {
        if (p.type === 'text') return p.text || '';
        if (p.type === 'thinking' || p.type === 'reasoning') {
          return '【思考】\n' + (p.thinking || p.text || '');
        }
        if (p.type === 'tool' && p.state) {
          const s = p.state;
          const out = Array.isArray(s.content)
            ? s.content.map((x) => x.text || JSON.stringify(x)).join('\n')
            : typeof s.content === 'string'
              ? s.content
              : JSON.stringify(s.content ?? s.error ?? null, null, 2);
          return `【工具 ${p.name || ''} · ${s.status || ''}】\n输入:\n${JSON.stringify(s.input ?? {}, null, 2)}\n结果:\n${out}`;
        }
        if (['tool_use', 'toolCall', 'tool', 'tool-call'].includes(p.type)) {
          return `【调用工具 ${p.name || p.toolName || p.tool || ''}】\n${JSON.stringify(
            p.input || p.arguments || p.state || p,
            null,
            2,
          )}`;
        }
        if (['tool_result', 'toolResult', 'tool-result'].includes(p.type)) {
          return `【工具返回】\n${JSON.stringify(p.content ?? p.output ?? p, null, 2)}`;
        }
        return JSON.stringify(p, null, 2);
      })
      .filter(Boolean)
      .join('\n\n');
  }
  return JSON.stringify(v, null, 2);
}

// The detail endpoint pages events; load up to this many automatically and offer the rest on demand.
const SESSION_EVENT_CAP = 2000;

function buildSessionSteps(detail) {
  const events = detail.events || [];
  const requests = detail.requests || [];
  if (!events.length) {
    return requests.map((r, i) => ({
      index: i + 1,
      kind: 'model',
      title: `网关请求 · ${r.model || r.id}`,
      role: 'gateway',
      timestamp: r.started,
      status: r.status,
      model: r.model || '未识别模型',
      text: `输入 ${num(r.input)} / 输出 ${num(r.output)} / 缓存读 ${num(r.cache_read)} Tokens`,
      requestId: r.id,
      attempts: [],
      linkedRequest: r,
      raw: r,
    }));
  }
  const byResponse = new Map();
  for (const r of requests) if (r.response_id) byResponse.set(r.response_id, r);
  const claimed = new Set();
  return events.map((e, i) => {
    const p = e.payload || {};
    const role = p.message?.role || e.kind || 'event';
    const text = extractReadablePayload(p);
    const parts = p.message?.content || p.message?.parts || p.parts || [];
    const hasTool =
      e.kind.includes('tool') ||
      role === 'toolResult' ||
      (Array.isArray(parts) &&
        parts.some((x) =>
          ['tool_use', 'toolCall', 'tool', 'tool-call', 'tool_result', 'toolResult', 'tool-result'].includes(
            x.type,
          ),
        ));
    // Link by response id. Without one, the request behind a response is whichever started after
    // the previous event and before the next: harnesses differ on whether an assistant message is
    // stamped when the request starts or when the response lands, and both fall inside that window.
    let linkedRequest = null;
    let attempts = [];
    const isResponse = role === 'assistant' || e.kind === 'assistant';
    const matched = isResponse ? byResponse.get(e.response_id) || null : null;
    // One response can be written as several events; only its first event carries the request.
    const continuation = matched != null && claimed.has(matched);
    if (isResponse && !continuation) {
      const since = i ? events[i - 1].timestamp : null;
      const until = events[i + 1]?.timestamp ?? Infinity;
      attempts = requests.filter(
        (r) =>
          r !== matched &&
          !claimed.has(r) &&
          (since == null || r.started > since) &&
          r.started < until,
      );
      if (matched) {
        linkedRequest = matched;
      } else {
        // Prefer the latest request that actually completed; the rest read as earlier attempts.
        const done = (r) => r.status === 'success' || r.status === 'partial';
        linkedRequest = attempts.findLast(done) || attempts.at(-1) || null;
        attempts = attempts.filter((r) => r !== linkedRequest);
      }
      if (linkedRequest) claimed.add(linkedRequest);
    }
    const title =
      role === 'user'
        ? '用户输入'
        : hasTool
          ? `工具交互 (${role})`
          : role === 'assistant'
            ? `模型响应${linkedRequest?.model ? ' · ' + linkedRequest.model : ''}`
            : `事件 · ${e.kind}`;
    return {
      index: i + 1,
      kind: hasTool ? 'tool' : role === 'user' ? 'user' : 'model',
      title,
      role,
      timestamp: e.timestamp,
      // Only a model response can have gone through the gateway; one without a matching request must
      // not be reported as a success.
      status:
        e.kind === 'import_error'
          ? 'failed'
          : isResponse
            ? (linkedRequest || matched)?.status || 'untracked'
            : null,
      model: linkedRequest?.model || null,
      text,
      requestId: linkedRequest?.id || null,
      attempts,
      linkedRequest,
      raw: e,
    };
  });
}

function waterfallBarSvg(step, minTs, spanMs) {
  const svg = svgEl('svg', {
    viewBox: '0 0 1000 18',
    preserveAspectRatio: 'none',
    class: 'waterfall-svg',
  });
  svg.append(svgEl('rect', { x: 0, y: 5, width: 1000, height: 8, rx: 4, class: 'svg-track-bg' }));
  const pos = spanMs > 0 ? Math.min(960, Math.max(0, Math.round(((step.timestamp - minTs) / spanMs) * 960))) : 0;
  const barCls = step.status === 'failed' ? 'svg-err-bar' : step.kind === 'tool' ? 'svg-output-bar' : 'svg-wait-bar';
  svg.append(svgEl('rect', { x: pos, y: 2, width: 28, height: 14, rx: 4, class: barCls }));
  return svg;
}

function mountSessionWorkspace() {
  const shell = el('div', 'session-shell');

  // The list controls are mounted once: rebuilding them would drop focus while the user types.
  const searchInput = el('input');
  searchInput.type = 'text';
  searchInput.placeholder = '搜索标题或 ID（/ 聚焦）';
  searchInput.value = state.sessionSearch;
  searchInput.setAttribute('aria-label', '搜索会话');
  let searchTimer = null;
  searchInput.addEventListener('input', () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      state.sessionSearch = searchInput.value;
      state.sessionOffset = 0;
      state.sessionJump = null;
      refresh();
    }, 320);
  });

  const harnessSelect = el('select');
  harnessSelect.setAttribute('aria-label', '筛选客户端');
  harnessSelect.add(new Option('全部', ''));
  for (const [value, label] of Object.entries(HARNESS_LABELS)) {
    if (value !== 'unknown') harnessSelect.add(new Option(label, value));
  }
  harnessSelect.value = state.sessionHarness;
  harnessSelect.addEventListener('change', () => {
    state.sessionHarness = harnessSelect.value;
    state.sessionOffset = 0;
    state.sessionJump = null;
    refresh();
  });

  const title = el('h2', '', '会话');
  const head = add(
    el('div', 'session-sidebar-head'),
    add(el('div', 'session-sidebar-title'), title, btn('立即导入', 'btn btn-sm', importSessions)),
    add(el('div', 'session-search-row'), searchInput, harnessSelect),
  );
  const listWrap = el('div', 'session-list');
  const pager = el('div', 'session-pager');
  add(shell, add(el('aside', 'session-sidebar'), head, listWrap, pager));

  const canvas = el('section', 'session-canvas');
  const inspector = el('aside', 'session-inspector');
  add(shell, canvas, inspector);
  state.sessionDom = {
    shell,
    title,
    searchInput,
    harnessSelect,
    listWrap,
    pager,
    canvas,
    inspector,
    scroll: null,
  };
  return shell;
}

async function importSessions() {
  try {
    await api('/api/import', { method: 'POST', body: '{}' });
    toast('已完成本机原生会话增量导入');
    state.sessionLoadedId = null;
    await refresh();
  } catch (err) {
    showError(err);
  }
}

// "13:05:09" today, otherwise "11/09 13:05" so an old session does not look like today's.
function listDate(ts) {
  if (!ts) return '—';
  const d = new Date(ts);
  return d.toDateString() === new Date().toDateString()
    ? timeOnly(ts)
    : d.toLocaleString('zh-CN', {
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      });
}

function renderSessionList() {
  const dom = state.sessionDom;
  const list = state.sessionListData || { total: 0, items: [] };
  dom.title.textContent = `会话 · ${num(list.total)}`;
  // Jumping here from a request clears the filters; show that in the mounted controls.
  if (dom.searchInput.value !== state.sessionSearch) {
    dom.searchInput.value = state.sessionSearch;
  }
  dom.harnessSelect.value = state.sessionHarness;
  const top = dom.listWrap.scrollTop;
  dom.listWrap.replaceChildren();
  if (!list.items.length) {
    add(
      dom.listWrap,
      el(
        'div',
        'empty',
        state.sessionSearch || state.sessionHarness
          ? '没有匹配的会话。'
          : '暂无本机会话。后台每 15 秒只读检查 pi / OpenCode / Claude Code 轨迹。',
      ),
    );
  }
  for (const s of list.items) {
    const item = el('button', `session-card ${s.id === state.selectedSessionId ? 'selected' : ''}`);
    item.type = 'button';
    item.dataset.sessionId = s.id;
    const cov =
      s.coverage === 'native'
        ? pill('原生轨迹', '')
        : s.coverage === 'partial'
          ? pill('含解析异常', 'warn')
          : pill('仅网关请求', 'neutral');
    add(
      item,
      add(
        el('div', 'session-card-top'),
        el('span', 'session-card-title', s.title || s.native_id),
        el('span', 'session-card-time', listDate(s.updated)),
      ),
      add(
        el('div', 'session-card-sub'),
        el('span', '', harnessLabel(s.harness)),
        el('span', '', `${num(s.events)} 事件`),
        el('span', '', `${num(s.requests)} 请求`),
        cov,
      ),
    );
    item.addEventListener('click', () => selectSession(s.id));
    add(dom.listWrap, item);
  }
  dom.listWrap.scrollTop = top;
  dom.pager.replaceChildren();
  if (list.total > 50) {
    add(
      dom.pager,
      paginationEl(state.sessionOffset, list.total, 50, (next) => {
        state.sessionOffset = next;
        refresh();
      }),
    );
  }
}

function selectSession(id) {
  state.sessionJump = null;
  const current = [state.sessionLoadedId, state.sessionLoadingId];
  if (state.selectedSessionId === id && current.includes(id)) return;
  state.selectedSessionId = id;
  for (const card of state.sessionDom.listWrap.children) {
    card.classList.toggle('selected', card.dataset.sessionId === id);
  }
  loadSessionEvents();
}

async function loadSessionEvents({ resume = false } = {}) {
  const token = ++state.sessionLoadToken;
  const id = state.selectedSessionId;
  // Only a newer load, another session or another page may cut this one short. An unrelated
  // refresh (a search keystroke, the refresh button) must not truncate it.
  const stale = () =>
    token !== state.sessionLoadToken ||
    state.selectedSessionId !== id ||
    state.page !== 'sessions';
  let detail = resume ? state.sessionDetailData : null;
  let first = detail ? detail.events.length : 0;
  const last = first + (resume ? 500 : SESSION_EVENT_CAP);
  if (!resume) {
    state.sessionLoadedId = null;
    state.sessionDetailData = null;
    state.sessionSteps = [];
  }
  state.sessionError = null;
  state.sessionLoadingId = id;
  renderSessionCanvas();
  renderSessionInspector();
  if (!id) {
    state.sessionLoadingId = null;
    return;
  }
  // The re-render replaces the "加载更多" button; keep keyboard focus on it.
  const refocus = document.activeElement?.classList.contains('session-more-btn');
  try {
    let pages = 0;
    // Paint after the first page, then once more when the remaining pages are in.
    while (first < last) {
      const page = await api('/api/sessions/' + encodeURIComponent(id) + '?offset=' + first);
      if (stale()) return;
      detail = { ...page, events: (detail?.events || []).concat(page.events) };
      first += page.events.length;
      pages += 1;
      state.sessionLoadedId = id;
      state.sessionDetailData = detail;
      state.sessionSteps = buildSessionSteps(detail);
      if (pages === 1) renderSessionCanvas({ keepScroll: resume });
      if (!page.events.length || first >= page.total_events) break;
    }
    renderSessionCanvas({ keepScroll: resume || pages > 1 });
  } catch (err) {
    if (stale()) return;
    if (state.sessionDetailData) {
      // Part of the history is already on screen; say that the rest failed to load.
      showError(err);
    } else {
      state.sessionError = err?.message || String(err);
    }
    renderSessionCanvas({ keepScroll: resume });
  } finally {
    // A superseding load has taken ownership of this field.
    if (token === state.sessionLoadToken) state.sessionLoadingId = null;
  }
  if (refocus) state.sessionDom.canvas.querySelector('.session-more-btn')?.focus();
}

function renderSessionCanvas({ keepScroll = false } = {}) {
  const dom = state.sessionDom;
  const top = dom.scroll?.scrollTop || 0;
  const detail = state.sessionDetailData;
  const s = detail?.session;
  const steps = state.sessionSteps;
  dom.canvas.replaceChildren();
  dom.scroll = null;
  if (!state.selectedSessionId) {
    add(dom.canvas, el('div', 'empty session-fill', '请在左侧选择一个会话以回放对话与时间轴'));
    return;
  }
  if (!s) {
    const fill = add(
      el('div', 'empty session-fill'),
      el('div', '', state.sessionError || '正在载入会话…'),
    );
    if (state.sessionError) {
      add(
        fill,
        add(el('div', 'session-retry'), btn('重试', 'btn btn-sm', () => loadSessionEvents())),
      );
    }
    add(dom.canvas, fill);
    return;
  }
  const head = add(
    el('div', 'session-canvas-head'),
    add(
      el('div', 'session-canvas-meta'),
      el('strong', '', s.title || s.native_id),
      el(
        'div',
        'small-note',
        `${harnessLabel(s.harness)} · ${s.native_id}${s.cwd ? ' · ' + s.cwd : ''}`,
      ),
    ),
    add(
      el('div', 'vendor-meta'),
      pill(`${num(steps.length)} 步骤`, 'neutral'),
      pill(`${num(detail.requests.length)} 个网关请求`, 'neutral'),
      btn('步骤详情', 'btn btn-sm inspector-toggle', toggleInspectorDrawer),
    ),
  );
  const tabs = el('div', 'session-canvas-tabs');
  tabs.setAttribute('role', 'tablist');
  for (const [id, label] of [
    ['conversation', '对话回放'],
    ['timeline', '时间轴瀑布'],
  ]) {
    const tab = btn(label, `tab-btn ${state.sessionView === id ? 'active' : ''}`, () =>
      setSessionView(id),
    );
    tab.dataset.view = id;
    tab.setAttribute('role', 'tab');
    tab.setAttribute('aria-selected', String(state.sessionView === id));
    add(tabs, tab);
  }
  const scroll = el('div', 'session-scroll');
  renderSessionScroll(scroll);
  add(dom.canvas, head, tabs, scroll);
  if (detail.total_events > detail.events.length) {
    add(
      dom.canvas,
      add(
        el('div', 'session-more'),
        el(
          'span',
          'small-note',
          `已加载 ${num(detail.events.length)} / ${num(detail.total_events)} 个事件`,
        ),
        btn('加载更多', 'btn btn-sm session-more-btn', () => loadSessionEvents({ resume: true })),
      ),
    );
  }
  dom.scroll = scroll;
  updateStepSelection({ scroll: !keepScroll });
  if (keepScroll) {
    scroll.scrollTop = top;
  } else if (state.sessionStepSelection[state.selectedSessionId] == null) {
    // A remembered selection has just been scrolled into view; do not pull the pane back.
    scroll.scrollTop = 0;
  }
}

function renderSessionScroll(scroll) {
  const steps = state.sessionSteps;
  if (!steps.length) {
    add(scroll, el('div', 'empty', '该会话暂无可展示的事件或请求。'));
    return;
  }
  if (state.sessionView === 'timeline') {
    const stamps = steps.map((x) => x.timestamp || 0);
    const minTs = stamps.reduce((a, b) => Math.min(a, b), Infinity);
    const spanMs = Math.max(1, stamps.reduce((a, b) => Math.max(a, b), -Infinity) - minTs);
    add(
      scroll,
      el('p', 'small-note', '按本机记录时间戳绘制事件先后顺序；未采集起止时间的工具事件不伪造耗时。'),
    );
    const list = el('div', 'waterfall-list');
    steps.forEach((st, i) => {
      const item = el('div', 'waterfall-item');
      item.dataset.stepIndex = String(i);
      item.addEventListener('click', () => selectSessionStep(i, { openDrawer: true }));
      add(
        item,
        el('div', 'small-note', `#${st.index} ${st.title}`),
        waterfallBarSvg(st, minTs, spanMs),
        el('span', 'small-note mono', `+${dur(st.timestamp - minTs)}`),
      );
      add(list, item);
    });
    add(scroll, list);
    return;
  }
  for (let i = 0; i < steps.length; i++) {
    const st = steps[i];
    const card = el('article', `step-card ${st.status === 'failed' ? 'is-error' : ''}`);
    card.dataset.stepIndex = String(i);
    const head = add(
      el('div', 'step-head'),
      el('span', 'step-num', st.index),
      el('strong', '', st.title),
      st.model ? pill(st.model, 'neutral') : null,
      st.attempts.length ? pill(`此前尝试 ${st.attempts.length} 次`, 'warn') : null,
      st.status === 'untracked' ? pill(STATUS_LABELS.untracked, 'neutral') : null,
      el('span', 'small-note', listDate(st.timestamp)),
    );
    if (st.linkedRequest) {
      const lr = st.linkedRequest;
      add(
        head,
        pill(
          `输入 ${num(lr.input)} / 输出 ${num(lr.output)}`,
          lr.status === 'success' ? '' : 'warn',
        ),
      );
    }
    add(card, head, el('div', 'step-body-text', st.text || '（无正文内容，见右侧原始事件）'));
    card.addEventListener('click', () => selectSessionStep(i, { scroll: false, openDrawer: true }));
    add(scroll, card);
  }
}

function currentStepIndex() {
  const steps = state.sessionSteps;
  const chosen = state.sessionStepSelection[state.selectedSessionId] ?? 0;
  return Math.min(Math.max(0, chosen), Math.max(0, steps.length - 1));
}

// Selection only flips a class and re-renders the inspector; the cards keep their scroll position.
function updateStepSelection({ scroll = false } = {}) {
  const idx = currentStepIndex();
  let selected = null;
  for (const node of state.sessionDom.scroll?.querySelectorAll('[data-step-index]') || []) {
    const on = Number(node.dataset.stepIndex) === idx;
    node.classList.toggle('selected', on);
    if (on) selected = node;
  }
  renderSessionInspector();
  if (scroll && selected) selected.scrollIntoView({ block: 'nearest' });
}

function selectSessionStep(index, { scroll = true, openDrawer = false } = {}) {
  const id = state.selectedSessionId;
  if (!id || !state.sessionSteps.length) return;
  state.sessionStepSelection[id] = Math.min(Math.max(0, index), state.sessionSteps.length - 1);
  if (openDrawer) setInspectorDrawer(true);
  updateStepSelection({ scroll });
}

function setSessionView(view) {
  if (state.sessionView === view) return;
  state.sessionView = view;
  for (const tab of state.sessionDom.canvas.querySelectorAll('.session-canvas-tabs .tab-btn')) {
    const on = tab.dataset.view === view;
    tab.classList.toggle('active', on);
    tab.setAttribute('aria-selected', String(on));
  }
  if (state.sessionDom.scroll) {
    state.sessionDom.scroll.replaceChildren();
    renderSessionScroll(state.sessionDom.scroll);
  }
  updateStepSelection({ scroll: true });
}

// Below 1180px the inspector becomes a drawer; wider layouts always show it.
function setInspectorDrawer(open) {
  state.sessionDom?.inspector.classList.toggle('mobile-open', open);
}

function toggleInspectorDrawer() {
  setInspectorDrawer(!state.sessionDom.inspector.classList.contains('mobile-open'));
}

function renderSessionInspector() {
  const dom = state.sessionDom;
  const idx = currentStepIndex();
  const step = state.sessionSteps[idx] || null;
  dom.inspector.replaceChildren();
  if (!step) {
    add(dom.inspector, el('div', 'empty session-fill', '选择任意步骤查看详情'));
    return;
  }
  const nav = (label, enabled, delta) => {
    const node = btn(label, 'btn btn-sm', () => selectSessionStep(idx + delta));
    node.disabled = !enabled;
    node.setAttribute('aria-label', delta < 0 ? '上一步' : '下一步');
    return node;
  };
  const close = btn('关闭 ✕', 'btn btn-sm inspector-close', () => setInspectorDrawer(false));
  close.setAttribute('aria-label', '关闭步骤详情');
  const head = add(
    el('div', 'inspector-head'),
    el('h3', '', `步骤 ${step.index} · ${step.title}`),
    add(
      el('div', 'provider-actions'),
      nav('←', idx > 0, -1),
      nav('→', idx < state.sessionSteps.length - 1, 1),
      close,
    ),
  );
  const tabs = el('div', 'inspector-tabs');
  for (const [id, label] of [
    ['request', '详情与关联'],
    ['output', '文本内容'],
    ['raw', '原始事件'],
  ]) {
    const tab = btn(label, `tab-btn ${state.inspectorTab === id ? 'active' : ''}`, () => {
      state.inspectorTab = id;
      renderSessionInspector();
    });
    tab.setAttribute('role', 'tab');
    tab.setAttribute('aria-selected', String(state.inspectorTab === id));
    add(tabs, tab);
  }
  const body = el('div', 'inspector-body');
  if (state.inspectorTab === 'request') {
    const kv = el('dl', 'kv-list');
    add(kv, el('dt', '', '时间'), el('dd', 'mono', dateStr(step.timestamp)));
    add(kv, el('dt', '', '角色 / 类型'), el('dd', '', step.role));
    if (step.status) {
      add(kv, el('dt', '', '状态'), add(el('dd'), statusPill(step.status)));
    }
    if (step.linkedRequest) {
      const lr = step.linkedRequest;
      add(kv, el('dt', '', '模型'), el('dd', 'mono', lr.model || '—'));
      add(kv, el('dt', '', '输入 / 输出'), el('dd', 'mono', `${num(lr.input)} / ${num(lr.output)}`));
      add(
        kv,
        el('dt', '', '缓存读 / 写'),
        el('dd', 'mono', `${num(lr.cache_read)} / ${num(lr.cache_write)}`),
      );
    }
    add(body, kv);
    step.attempts.forEach((attempt, n) => {
      add(
        body,
        add(
          el('div', 'attempt-row'),
          pill(`此前第 ${n + 1} 次尝试`, 'neutral'),
          statusPill(attempt.status),
          btn('查看请求', 'link-btn', () => openRequestDetail(attempt.id)),
        ),
      );
    });
    if (step.requestId) {
      add(
        body,
        btn('查看完整网关请求详情 →', 'btn btn-primary btn-sm', () =>
          openRequestDetail(step.requestId),
        ),
      );
    }
    if (step.status === 'untracked') {
      add(body, el('p', 'small-note', '没有与之对应的网关请求：这一步不是经 Loom 发出的，不计入调用统计。'));
    }
  } else if (state.inspectorTab === 'output') {
    add(
      body,
      btn('复制内容', 'btn btn-sm', () => copyText(step.text || '')),
      el('pre', 'code-block', step.text || '（无文本输出）'),
    );
  } else {
    const rawJson = JSON.stringify(step.raw, null, 2);
    add(
      body,
      btn('复制原始 JSON', 'btn btn-sm', () => copyText(rawJson)),
      el('pre', 'code-block', rawJson),
    );
  }
  add(dom.inspector, head, tabs, body);
}

async function renderSessions() {
  const generation = state.generation;
  const list = await api(
    '/api/sessions?' +
      qs({
        harness: state.sessionHarness,
        q: state.sessionSearch,
        offset: state.sessionOffset,
      }),
  );
  if (generation !== state.generation) return null;
  state.sessionListData = list;
  // Fall back to the first row unless the user jumped here from one specific request.
  const visible = list.items.some((s) => s.id === state.selectedSessionId);
  const jumped = state.sessionJump != null && state.sessionJump === state.selectedSessionId;
  if (!list.items.length) {
    state.selectedSessionId = state.sessionJump = null;
  } else if (!visible && !jumped) {
    state.selectedSessionId = list.items[0].id;
  }
  const mounted = state.sessionDom?.shell.isConnected;
  const shell = mounted ? state.sessionDom.shell : mountSessionWorkspace();
  renderSessionList();
  const wanted = state.selectedSessionId;
  if (wanted !== state.sessionLoadedId && wanted !== state.sessionLoadingId) {
    // Not awaited: the shell mounts with a placeholder and fills in as pages arrive.
    loadSessionEvents();
  } else if (!mounted) {
    renderSessionCanvas();
    renderSessionInspector();
  }
  // Already on screen: the panes were updated in place, so there is nothing to swap in.
  return mounted ? null : shell;
}

/* ==================== 7. 采集与保留设置 Settings ==================== */

async function renderSettings() {
  const v = await api('/api/settings');
  $('#version').textContent = 'v' + v.version;
  const panel = el('section', 'panel settings-card');
  const bodyCb = el('input');
  bodyCb.type = 'checkbox';
  bodyCb.checked = v.capture_bodies;
  const nativeCb = el('input');
  nativeCb.type = 'checkbox';
  nativeCb.checked = v.import_sessions;
  const daysInput = el('input');
  daysInput.type = 'number';
  daysInput.min = '1';
  daysInput.max = '3650';
  daysInput.value = v.retention_days;
  daysInput.setAttribute('aria-label', '保留天数');

  const saveBtn = btn('保存设置', 'btn btn-primary', async () => {
    try {
      await api('/api/settings', {
        method: 'POST',
        body: JSON.stringify({
          capture_bodies: bodyCb.checked,
          import_sessions: nativeCb.checked,
          retention_days: Number(daysInput.value),
        }),
      });
      toast('采集与保留设置已保存');
    } catch (err) {
      showError(err);
    }
  });

  const pruneBtn = btn('按保留策略立即清理', 'btn btn-danger', async () => {
    if (!confirm('仅清理已到期的本机观测副本，不删除客户端原生会话文件，继续？')) return;
    try {
      const res = await api('/api/prune', { method: 'POST', body: '{}' });
      toast(`已清理 ${num(res.removed_requests)} 条过期请求记录`);
      refresh();
    } catch (err) {
      showError(err);
    }
  });

  add(
    panel,
    el('h2', '', '本机观测采集与数据保留'),
    add(
      el('div', 'setting-row'),
      add(
        el('div'),
        el('strong', '', '保存完整请求 / 响应正文'),
        el('p', 'small-note', '仅存于本机 0600 SQLite 数据库，落盘前自动脱敏所有已知密钥。关闭后只记录耗时与 Token 指标。'),
      ),
      bodyCb,
    ),
    add(
      el('div', 'setting-row'),
      add(
        el('div'),
        el('strong', '', '自动导入客户端原生会话'),
        el('p', 'small-note', '后台每 15 秒只读检查 pi、OpenCode V2、Claude Code 的本机轨迹。'),
      ),
      nativeCb,
    ),
    add(
      el('div', 'setting-row'),
      add(
        el('div'),
        el('strong', '', '本机保留天数（1–3650）'),
        el('p', 'small-note', '到期仅清理本机观测副本，绝不修改或删除客户端原会话文件。'),
      ),
      daysInput,
    ),
    add(
      el('div', 'setting-row'),
      el(
        'p',
        'small-note',
        `数据库体积 ${(v.database_bytes / 1048576).toFixed(2)} MB · 最近导入 ${dateStr(v.last_import?.checked_at)} · 失败来源 ${v.last_import?.failed_sources ?? 0}`,
      ),
      add(el('div', 'provider-actions'), pruneBtn, saveBtn),
    ),
    el(
      'p',
      'small-note',
      '历史原生会话中的非网关请求不会冒充实测网关调用；未知 Token、缓存与计费金额严格显示为 —。',
    ),
  );
  return panel;
}

const PAGE_RENDERERS = {
  onboarding: renderOnboarding,
  providers: renderProviders,
  integrations: renderIntegrations,
  overview: renderOverview,
  requests: () => renderRequests(filters()),
  sessions: renderSessions,
  settings: renderSettings,
};

async function refresh() {
  if (!state.authenticated && $('#login').hidden === false) return;
  const generation = ++state.generation;
  clearError();

  const showTopFilters = state.page === 'overview' || state.page === 'requests';
  $('#filters').hidden = !showTopFilters;
  $('#status-field').hidden = state.page !== 'requests';

  try {
    if (showTopFilters && !state.groups.length) {
      const ov = await api('/api/overview?' + qs(filters()));
      syncFilterOptions(ov.groups);
    }
    // Only the setup pages read /api/setup themselves; keep the gateway pill current everywhere.
    if (!['onboarding', 'providers', 'integrations'].includes(state.page)) {
      loadSetup().catch(() => {});
    }
    const node = await (PAGE_RENDERERS[state.page] || renderOnboarding)();
    if (generation !== state.generation) return;

    const content = $('#content');
    content.className = state.page === 'sessions' ? 'stage-content sessions-mode' : 'stage-content';
    if (node) content.replaceChildren(node);
    // A freshly mounted workspace needs layout before its selected step can scroll into view.
    if (node && state.page === 'sessions') updateStepSelection({ scroll: true });
    $('#updated').textContent = '更新于 ' + timeOnly(Date.now());
  } catch (err) {
    if (generation === state.generation) {
      showError(err);
      $('#content .page-loading')?.replaceWith(
        el('div', 'empty', '这一页没能载入，请点右上角「刷新」重试。'),
      );
    }
  }
}

function navigate(page, { push = true } = {}) {
  const target = PAGE_TITLES[page] ? page : 'onboarding';
  // A failed load must not leave another page's content under the new title.
  if (target !== state.page && state.authenticated) {
    const content = $('#content');
    // Drop the sessions layout before the placeholder, or it renders without padding.
    content.className = 'stage-content';
    content.replaceChildren(el('div', 'empty page-loading', '正在载入…'));
  }
  state.page = target;
  state.offset = 0;
  $('#title').textContent = PAGE_TITLES[target];
  document.querySelectorAll('.rail-btn[data-page]').forEach((b) => {
    const active = b.dataset.page === target;
    b.classList.toggle('active', active);
    b.setAttribute('aria-current', active ? 'page' : 'false');
  });
  // pushState keeps Back/Forward inside the workspace; replaceState dropped that history.
  if (location.hash !== '#' + target) {
    history[push ? 'pushState' : 'replaceState'](null, '', '#' + target);
  }
  refresh();
}

// A request can belong to a session outside the visible list page; load it by id instead of the first row.
function jumpToSession(id) {
  // From the inspector's own request dialog the target is the session already on screen.
  const shown = state.page === 'sessions' && state.selectedSessionId === id;
  if (shown && state.sessionLoadedId === id) return;
  state.sessionSearch = '';
  state.sessionHarness = '';
  state.sessionOffset = 0;
  state.selectedSessionId = state.sessionJump = id;
  state.sessionLoadedId = null;
  navigate('sessions');
}

function parseHashTarget() {
  const raw = location.hash.slice(1);
  if (!raw) return { ticket: null, page: null };
  if (raw.includes('=')) {
    const params = new URLSearchParams(raw);
    return { ticket: params.get('ticket'), page: params.get('page') };
  }
  return { ticket: null, page: PAGE_TITLES[raw] ? raw : null };
}

document.querySelectorAll('.rail-btn[data-page]').forEach((b) => {
  b.addEventListener('click', () => navigate(b.dataset.page));
});

$('#refresh').addEventListener('click', () => {
  state.sessionLoadedId = null; // An explicit refresh should pick up newly imported events.
  refresh();
});
$('#close-detail').addEventListener('click', () => $('#detail').close());
$('#close-provider-dialog').addEventListener('click', () => $('#provider-dialog').close());
$('#cancel-provider-dialog').addEventListener('click', () => $('#provider-dialog').close());
$('#copy-console-cmd').addEventListener('click', () => copyText('loom console', '已复制登录命令'));

$('#quick-sync').addEventListener('click', async () => {
  try {
    await api('/api/sync', { method: 'POST', body: '{}' });
    toast('已完成本机客户端同步');
    if (['onboarding', 'providers', 'integrations'].includes(state.page)) {
      refresh();
    }
  } catch (err) {
    showError(err);
  }
});

$('#custom-provider-form').addEventListener('submit', async (ev) => {
  ev.preventDefault();
  const errBox = $('#cf-error');
  errBox.hidden = true;
  const submitBtn = $('#cf-submit');
  submitBtn.disabled = true;
  try {
    const payload = {
      provider_id: $('#cf-provider-id').value.trim() || undefined,
      prefix: $('#cf-prefix').value.trim(),
      name: $('#cf-name').value.trim(),
      base_url: $('#cf-base-url').value.trim(),
      anthropic_base_url: $('#cf-anthropic-url').value.trim(),
      billing: $('#cf-billing').value.trim(),
      models_text: $('#cf-models').value.trim(),
      reasoning: $('#cf-reasoning').checked,
      api_key: $('#cf-key').value.trim(),
    };
    const saved = await api('/api/providers/custom', {
      method: 'POST',
      body: JSON.stringify(payload),
    });
    $('#provider-dialog').close();
    if (saved.sync_warning) toast(saved.sync_warning, 'warn');
    else toast('自定义厂商已保存并同步客户端');
    state.page = 'providers';
    navigate('providers');
  } catch (err) {
    errBox.textContent = err.message || '保存失败';
    errBox.hidden = false;
  } finally {
    submitBtn.disabled = false;
  }
});

for (const id of ['range', 'provider', 'model', 'harness', 'status']) {
  $('#' + id).addEventListener('change', () => {
    state.offset = 0;
    refresh();
  });
}

window.addEventListener('hashchange', () => {
  const { page } = parseHashTarget();
  if (page && page !== state.page) {
    navigate(page, { push: false });
  }
});

window.addEventListener('keydown', (e) => {
  if (e.target?.matches?.('input,textarea,select')) return;
  // An open dialog owns the keyboard.
  if (document.querySelector('dialog[open]')) return;
  if (state.page !== 'sessions' || !state.sessionDom?.shell.isConnected) return;
  if (e.key === '/') {
    e.preventDefault();
    state.sessionDom.searchInput.focus();
  }
  if (e.key === 'Escape') setInspectorDrawer(false);
  if (e.altKey && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
    e.preventDefault();
    selectSessionStep(currentStepIndex() + (e.key === 'ArrowDown' ? 1 : -1));
  }
});

(async () => {
  const { ticket, page } = parseHashTarget();
  if (ticket) {
    history.replaceState(null, '', location.pathname);
    try {
      await api('/api/login', {
        method: 'POST',
        body: JSON.stringify({ ticket }),
      });
    } catch (err) {
      showError(err);
      return;
    }
  }
  try {
    const setup = await loadSetup();
    const target = page && PAGE_TITLES[page] ? page : setup.needs_setup ? 'onboarding' : 'overview';
    navigate(target, { push: false });
  } catch (err) {
    if (state.authenticated) {
      showError(err);
    }
  }
})();

setInterval(() => {
  if (!state.authenticated || document.hidden || $('#detail').open || $('#provider-dialog').open) {
    return;
  }
  if (['overview', 'requests'].includes(state.page)) {
    // Leave the page alone while someone is reading a hovered chart or filling in a field.
    const busy =
      document.activeElement?.matches('input,select,textarea') ||
      document.querySelector('.chart-box:hover');
    if (!busy) refresh();
  } else if ($('#login').hidden) {
    // Other pages are not re-rendered behind the user; only the gateway pill must stay current.
    loadSetup().catch(() => {});
  }
}, 15000);
