// Lynx Dashboard — Registry page

const RegistryPage = {
  entries: [],
  filter: 'all',
  search: '',

  async render(el) {
    el.innerHTML = `
      <h1 class="page-title">Registry</h1>
      <div style="display:flex; gap:10px; margin-bottom:20px; flex-wrap:wrap;">
        <input type="text" class="input" id="reg-search" placeholder="Search packages\u2026"
          style="flex:1; min-width:200px;">
        <div style="display:flex; gap:4px;">
          <button class="btn reg-filter active" data-type="all">All</button>
          <button class="btn reg-filter" data-type="plugin">Plugins</button>
          <button class="btn reg-filter" data-type="tool">Tools</button>
          <button class="btn reg-filter" data-type="theme">Themes</button>
          <button class="btn reg-filter" data-type="intro">Intros</button>
        </div>
      </div>
      <div id="reg-list" class="card-grid"></div>
      <div style="margin-top:32px;">
        <div class="section-title">Tap Management</div>
        <div id="reg-taps"></div>
        <div style="display:flex; gap:8px; margin-top:12px;">
          <input type="text" class="input" id="reg-tap-input" placeholder="owner/repo or URL">
          <button class="btn btn-primary" id="reg-tap-add">Add Tap</button>
        </div>
      </div>
    `;

    document.getElementById('reg-search').addEventListener('input', (e) => {
      this.search = e.target.value.toLowerCase();
      this.renderList();
    });

    el.querySelectorAll('.reg-filter').forEach(btn => {
      btn.addEventListener('click', () => {
        el.querySelectorAll('.reg-filter').forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        this.filter = btn.dataset.type;
        this.renderList();
      });
    });

    document.getElementById('reg-tap-add')?.addEventListener('click', () => this.addTap());

    await Promise.all([this.loadEntries(), this.loadTaps()]);
  },

  async loadEntries() {
    try {
      const data = await Api.registry();
      this.entries = data.entries || [];
      this.renderList();
    } catch (_) {
      document.getElementById('reg-list').innerHTML =
        '<div class="empty-state"><p>Failed to load registry.</p></div>';
    }
  },

  renderList() {
    const el = document.getElementById('reg-list');
    if (!el) return;
    let items = this.entries;
    if (this.filter !== 'all') items = items.filter(e => e.type === this.filter);
    if (this.search) items = items.filter(e =>
      e.name.toLowerCase().includes(this.search) ||
      e.description.toLowerCase().includes(this.search)
    );

    if (!items.length) {
      el.innerHTML = '<div class="empty-state"><p>No packages match.</p></div>';
      return;
    }

    el.innerHTML = items.map(e => `
      <div class="card">
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
          <span style="font-family:var(--font-mono); font-weight:600;">${e.name}</span>
          <div style="display:flex; gap:4px;">
            <span class="badge badge-info">${e.type}</span>
            <span class="badge badge-${e.trust === 'official' ? 'success' : e.trust === 'verified' ? 'info' : 'warning'}">${e.trust}</span>
          </div>
        </div>
        <p style="color:var(--text-secondary); font-size:13px; margin-bottom:8px;">${e.description}</p>
        <div style="display:flex; justify-content:space-between; align-items:center;">
          <span style="font-size:12px; color:var(--text-muted);">from ${e.tap}</span>
          <button class="btn btn-primary" data-install="${e.name}">Install</button>
        </div>
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
        } catch (err) {
          App.toast('Install failed: ' + err.message, 'error');
        }
        btn.disabled = false;
        btn.textContent = 'Install';
      });
    });
  },

  async loadTaps() {
    const el = document.getElementById('reg-taps');
    if (!el) return;
    try {
      const data = await Api.get('/api/taps');
      const taps = data.taps || [];
      el.innerHTML = `<table class="table">
        <thead><tr><th>Name</th><th>URL</th><th>Trust</th><th></th></tr></thead>
        <tbody>${taps.map(t => `<tr>
          <td style="font-family:var(--font-mono);">${t.name}</td>
          <td style="color:var(--text-secondary); font-size:12px;">${t.url}</td>
          <td><span class="badge badge-${t.trust === 'official' ? 'success' : 'info'}">${t.trust}</span></td>
          <td>${t.name !== 'official' ? `<button class="btn btn-danger" data-remove-tap="${t.name}">\u00D7</button>` : ''}</td>
        </tr>`).join('')}</tbody>
      </table>`;

      el.querySelectorAll('[data-remove-tap]').forEach(btn => {
        btn.addEventListener('click', async () => {
          try {
            await Api.post('/api/tap/remove', { name: btn.dataset.removeTap });
            App.toast('Tap removed');
            this.loadTaps();
            this.loadEntries();
          } catch (err) {
            App.toast('Error: ' + err.message, 'error');
          }
        });
      });
    } catch (_) {}
  },

  async addTap() {
    const input = document.getElementById('reg-tap-input');
    const url = input?.value?.trim();
    if (!url) return;
    try {
      const name = url.split('/').pop() || url;
      await Api.post('/api/tap/add', { name, url });
      App.toast('Tap added', 'success');
      input.value = '';
      this.loadTaps();
      this.loadEntries();
    } catch (err) {
      App.toast('Error: ' + err.message, 'error');
    }
  },
};
