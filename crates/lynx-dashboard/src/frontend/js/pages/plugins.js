// Lynx Dashboard — Plugins page

const PluginsPage = {
  async render(el) {
    el.innerHTML = `
      <h1 class="page-title">Plugins</h1>
      <div id="pl-installed"></div>
      <div style="margin-top: 32px;">
        <div class="section-title">Quick Install from Registry</div>
        <div id="pl-available" class="card-grid"></div>
      </div>
    `;
    await Promise.all([this.loadInstalled(), this.loadAvailable()]);

    App.pageHandler = (data) => {
      if (data.type === 'plugins_updated') {
        this.loadInstalled();
        this.loadAvailable();
      }
    };
  },

  async loadInstalled() {
    const el = document.getElementById('pl-installed');
    if (!el) return;
    try {
      const data = await Api.plugins();
      const plugins = data.plugins || [];
      if (!plugins.length) {
        el.innerHTML = '<div class="empty-state"><p>No plugins installed.</p></div>';
        return;
      }
      el.innerHTML = `<table class="table">
        <thead><tr><th>Plugin</th><th>Version</th><th>Description</th><th>Enabled</th></tr></thead>
        <tbody>${plugins.map(p => `<tr>
          <td style="font-family: var(--font-mono);">${p.name}</td>
          <td><span class="badge">${p.version}</span></td>
          <td style="color: var(--text-secondary);">${p.description}</td>
          <td>
            <input type="checkbox" class="toggle" data-plugin="${p.name}"
              ${p.enabled ? 'checked' : ''}>
          </td>
        </tr>`).join('')}</tbody>
      </table>`;

      el.querySelectorAll('.toggle').forEach(toggle => {
        toggle.addEventListener('change', async (e) => {
          const name = e.target.dataset.plugin;
          try {
            if (e.target.checked) {
              await Api.pluginEnable(name);
              App.toast(`Enabled ${name}`, 'success');
            } else {
              await Api.pluginDisable(name);
              App.toast(`Disabled ${name}`);
            }
          } catch (err) {
            e.target.checked = !e.target.checked;
            App.toast('Error: ' + err.message, 'error');
          }
        });
      });
    } catch (e) {
      el.innerHTML = `<div class="empty-state"><p>Failed to load plugins.</p></div>`;
    }
  },

  async loadAvailable() {
    const el = document.getElementById('pl-available');
    if (!el) return;
    try {
      const [reg, installed] = await Promise.all([Api.registry(), Api.plugins()]);
      const installedNames = new Set((installed.plugins || []).map(p => p.name));
      const available = (reg.entries || [])
        .filter(e => e.type === 'plugin' && !installedNames.has(e.name))
        .slice(0, 12);

      if (!available.length) {
        el.innerHTML = '<div class="empty-state"><p>All registry plugins installed.</p></div>';
        return;
      }

      el.innerHTML = available.map(e => `
        <div class="card">
          <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
            <span style="font-family:var(--font-mono); font-weight:600;">${e.name}</span>
            <span class="badge badge-${e.trust === 'official' ? 'success' : e.trust === 'verified' ? 'info' : 'warning'}">${e.trust}</span>
          </div>
          <p style="color:var(--text-secondary); font-size:13px; margin-bottom:12px;">${e.description}</p>
          <button class="btn btn-primary" data-install="${e.name}">Install</button>
        </div>
      `).join('');

      el.querySelectorAll('[data-install]').forEach(btn => {
        btn.addEventListener('click', async () => {
          const name = btn.dataset.install;
          btn.disabled = true;
          btn.textContent = 'Installing\u2026';
          try {
            await Api.pluginInstall(name);
            App.toast(`Installed ${name}`, 'success');
            this.loadInstalled();
            this.loadAvailable();
          } catch (err) {
            App.toast('Install failed: ' + err.message, 'error');
            btn.disabled = false;
            btn.textContent = 'Install';
          }
        });
      });
    } catch (_) {
      el.innerHTML = '<div class="empty-state"><p>Registry unavailable.</p></div>';
    }
  },
};
