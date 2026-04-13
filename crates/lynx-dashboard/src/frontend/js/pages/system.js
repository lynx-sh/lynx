// Lynx Dashboard — System page

const SystemPage = {
  async render(el) {
    el.innerHTML = `
      <h1 class="page-title">System</h1>
      <div style="display:flex; flex-direction:column; gap:20px;">
        <div class="card">
          <div class="section-title">Configuration</div>
          <textarea id="sys-config" class="input" rows="12"
            style="width:100%; font-family:var(--font-mono); font-size:13px;
                   background:var(--bg-primary); resize:vertical;"></textarea>
          <div style="margin-top:10px; display:flex; gap:8px;">
            <button class="btn btn-primary" id="sys-config-save">Save</button>
            <span id="sys-config-status" style="font-size:12px; color:var(--text-muted);
              align-self:center;"></span>
          </div>
        </div>
        <div class="card">
          <div class="section-title">Doctor</div>
          <div id="sys-doctor"></div>
        </div>
        <div class="card">
          <div class="section-title">Diagnostic Log</div>
          <div id="sys-diag"
            style="background:var(--bg-primary); border:1px solid var(--border);
                   border-radius:var(--radius-md); padding:12px; font-family:var(--font-mono);
                   font-size:12px; max-height:300px; overflow-y:auto; line-height:1.6;">
          </div>
        </div>
      </div>
    `;

    document.getElementById('sys-config-save')?.addEventListener('click', () => this.saveConfig());

    await Promise.all([this.loadConfig(), this.loadDoctor(), this.loadDiag()]);
  },

  async loadConfig() {
    try {
      const data = await Api.config();
      const textarea = document.getElementById('sys-config');
      if (textarea) textarea.value = JSON.stringify(data, null, 2);
    } catch (_) {}
  },

  async saveConfig() {
    const textarea = document.getElementById('sys-config');
    const status = document.getElementById('sys-config-status');
    if (!textarea) return;
    try {
      const fields = JSON.parse(textarea.value);
      await Api.configUpdate(fields);
      if (status) status.textContent = 'Saved';
      App.toast('Config updated', 'success');
      setTimeout(() => { if (status) status.textContent = ''; }, 2000);
    } catch (err) {
      if (status) status.textContent = 'Error';
      App.toast('Save failed: ' + err.message, 'error');
    }
  },

  async loadDoctor() {
    const el = document.getElementById('sys-doctor');
    if (!el) return;
    try {
      const data = await Api.doctor();
      const checks = data.checks || [];
      el.innerHTML = checks.map(c => {
        const color = c.status === 'pass' ? 'var(--success)'
          : c.status === 'warn' ? 'var(--warning)' : 'var(--error)';
        const icon = c.status === 'pass' ? '\u2713' : c.status === 'warn' ? '\u26A0' : '\u2717';
        return `<div style="padding:6px 0; border-bottom:1px solid var(--border); display:flex; gap:8px;">
          <span style="color:${color}; width:16px;">${icon}</span>
          <span style="min-width:160px;">${c.name}</span>
          <span style="color:var(--text-secondary);">${c.detail}</span>
          ${c.fix ? `<code style="margin-left:auto; color:var(--text-muted); font-size:12px;">${c.fix}</code>` : ''}
        </div>`;
      }).join('');
    } catch (_) {
      el.innerHTML = '<p style="color:var(--text-secondary)">Failed to run doctor.</p>';
    }
  },

  async loadDiag() {
    const el = document.getElementById('sys-diag');
    if (!el) return;
    try {
      const data = await Api.diag();
      const lines = data.lines || [];
      if (!lines.length) {
        el.innerHTML = '<span style="color:var(--text-muted);">No log entries.</span>';
        return;
      }
      el.innerHTML = lines.map(line => {
        let color = 'var(--text-secondary)';
        if (line.includes('[ERROR]')) color = 'var(--error)';
        else if (line.includes('[WARN]')) color = 'var(--warning)';
        else if (line.includes('[INFO]')) color = 'var(--info)';
        return `<div style="color:${color};">${this.escapeHtml(line)}</div>`;
      }).join('');
      el.scrollTop = el.scrollHeight;
    } catch (_) {
      el.innerHTML = '<span style="color:var(--text-muted);">Failed to load log.</span>';
    }
  },

  escapeHtml(s) {
    const d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
  },
};
