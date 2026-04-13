// Lynx Dashboard — Overview page

const OverviewPage = {
  async render(el) {
    el.innerHTML = `
      <h1 class="page-title">Overview</h1>
      <div class="card-grid" id="overview-cards">
        <a href="#/themes" class="card" id="card-theme">
          <div class="card-title">Active Theme</div>
          <div class="card-value" id="ov-theme">...</div>
        </a>
        <a href="#/plugins" class="card" id="card-plugins">
          <div class="card-title">Plugins</div>
          <div class="card-value" id="ov-plugins">...</div>
        </a>
        <a href="#/system" class="card" id="card-doctor">
          <div class="card-title">System Health</div>
          <div class="card-value" id="ov-doctor">...</div>
        </a>
        <a href="#/cron" class="card" id="card-cron">
          <div class="card-title">Scheduled Tasks</div>
          <div class="card-value" id="ov-cron">...</div>
        </a>
      </div>
      <div class="card" id="card-doctor-detail" style="margin-top: 16px;">
        <div class="section-title">Doctor Checks</div>
        <div id="ov-doctor-list"></div>
      </div>
    `;

    // Fetch all data concurrently
    await Promise.all([
      this.loadConfig(),
      this.loadPlugins(),
      this.loadDoctor(),
      this.loadCron(),
    ]);
  },

  async loadConfig() {
    try {
      const data = await Api.config();
      document.getElementById('ov-theme').textContent = data.active_theme || 'none';
    } catch (_) {
      document.getElementById('ov-theme').textContent = '\u2014';
    }
  },

  async loadPlugins() {
    try {
      const data = await Api.plugins();
      const count = data.plugins ? data.plugins.length : 0;
      document.getElementById('ov-plugins').textContent = `${count} enabled`;
    } catch (_) {
      document.getElementById('ov-plugins').textContent = '\u2014';
    }
  },

  async loadDoctor() {
    try {
      const data = await Api.doctor();
      const checks = data.checks || [];
      const pass = checks.filter(c => c.status === 'pass').length;
      const total = checks.length;
      const el = document.getElementById('ov-doctor');

      if (data.healthy) {
        el.innerHTML = `<span style="color: var(--success)">\u2713 ${pass}/${total}</span>`;
      } else {
        const fails = checks.filter(c => c.status === 'fail').length;
        el.innerHTML = `<span style="color: var(--error)">\u2717 ${fails} issue${fails !== 1 ? 's' : ''}</span>`;
      }

      // Detail list
      const listEl = document.getElementById('ov-doctor-list');
      listEl.innerHTML = checks.map(c => {
        const color = c.status === 'pass' ? 'var(--success)'
          : c.status === 'warn' ? 'var(--warning)' : 'var(--error)';
        const icon = c.status === 'pass' ? '\u2713' : c.status === 'warn' ? '\u26A0' : '\u2717';
        return `<div style="padding: 6px 0; border-bottom: 1px solid var(--border); display: flex; gap: 8px; align-items: baseline;">
          <span style="color: ${color}; width: 16px;">${icon}</span>
          <span style="color: var(--text-primary); min-width: 160px;">${c.name}</span>
          <span style="color: var(--text-secondary);">${c.detail}</span>
          ${c.fix ? `<code style="margin-left: auto; color: var(--text-muted); font-size: 12px;">${c.fix}</code>` : ''}
        </div>`;
      }).join('');
    } catch (_) {
      document.getElementById('ov-doctor').textContent = '\u2014';
    }
  },

  async loadCron() {
    try {
      const data = await Api.cron();
      const count = data.tasks ? data.tasks.length : 0;
      document.getElementById('ov-cron').textContent = `${count} task${count !== 1 ? 's' : ''}`;
    } catch (_) {
      document.getElementById('ov-cron').textContent = '\u2014';
    }
  },
};
