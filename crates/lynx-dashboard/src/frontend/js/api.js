// Lynx Dashboard — API client

const Api = {
  async get(path) {
    const res = await fetch(path);
    if (!res.ok) throw new Error(`GET ${path}: ${res.status}`);
    return res.json();
  },

  async post(path, body) {
    const res = await fetch(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: res.statusText }));
      throw new Error(err.error || res.statusText);
    }
    return res.json().catch(() => ({}));
  },

  // Read endpoints
  config:    () => Api.get('/api/config'),
  theme:     () => Api.get('/api/theme'),
  themes:    () => Api.get('/api/themes'),
  plugins:   () => Api.get('/api/plugins'),
  registry:  () => Api.get('/api/registry'),
  intros:    () => Api.get('/api/intros'),
  profiles:  () => Api.get('/api/profiles'),
  workflows: () => Api.get('/api/workflows'),
  jobs:      () => Api.get('/api/jobs'),
  cron:      () => Api.get('/api/cron'),
  doctor:    () => Api.get('/api/doctor'),
  diag:      () => Api.get('/api/diag'),

  // Mutations
  themePatch:        (path, value) => Api.post('/api/theme/patch', { path, value }),
  themeSegment:      (op, name, side, after) => Api.post('/api/theme/segment', { op, name, side, after }),
  themeSegmentOrder: (side, order) => Api.post('/api/theme/segment-order', { side, order }),
  themeApply:        () => Api.post('/api/theme/apply', {}),
  themeReset:        () => Api.post('/api/theme/reset', {}),
  pluginEnable:      (name) => Api.post('/api/plugin/enable', { name }),
  pluginDisable:     (name) => Api.post('/api/plugin/disable', { name }),
  pluginInstall:     (name) => Api.post('/api/plugin/install', { name }),
  configUpdate:      (fields) => Api.post('/api/config/update', fields),
  introSet:          (slug) => Api.post('/api/intro/set', { slug }),
  workflowRun:       (name, params) => Api.post('/api/workflow/run', { name, params }),
  cronAdd:           (task) => Api.post('/api/cron/add', task),
  cronRemove:        (id) => Api.post('/api/cron/remove', { id }),
};
