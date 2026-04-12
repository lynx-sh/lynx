// Lynx Dashboard — sidebar navigation component

const Sidebar = {
  pages: [
    { id: 'overview',  icon: '\u25A3', label: 'Overview' },
    { id: 'themes',    icon: '\u25D0', label: 'Themes' },
    { id: 'plugins',   icon: '\u29C9', label: 'Plugins' },
    { id: 'registry',  icon: '\u2B13', label: 'Registry' },
    { id: 'workflows', icon: '\u21BB', label: 'Workflows' },
    { id: 'cron',      icon: '\u23F0', label: 'Cron' },
    { id: 'intros',    icon: '\u2605', label: 'Intros' },
    { id: 'system',    icon: '\u2699', label: 'System' },
  ],

  render(activePage) {
    const el = document.getElementById('sidebar');
    el.innerHTML = `
      <div class="sidebar-brand">
        <span class="brand-icon">\u229B</span>
        <span class="nav-label brand-text">Lynx</span>
      </div>
      <nav class="sidebar-nav">
        ${this.pages.map(p => `
          <a href="#/${p.id}" class="nav-item ${p.id === activePage ? 'active' : ''}"
             data-page="${p.id}">
            <span class="nav-icon">${p.icon}</span>
            <span class="nav-label">${p.label}</span>
          </a>
        `).join('')}
      </nav>
      <div class="sidebar-footer">
        <span class="nav-label" style="color: var(--text-muted); font-size: 12px;">v${App.version || '?'}</span>
      </div>
    `;
  }
};
