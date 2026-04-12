// Lynx Dashboard — main application: router, SSE, state

const App = {
  version: null,
  currentPage: null,
  eventSource: null,

  async init() {
    // Fetch version from root endpoint
    try {
      const info = await Api.get('/api/info');
      App.version = info.version;
    } catch (_) {
      App.version = '?';
    }

    // Start SSE connection
    App.connectSSE();

    // Set up hash router
    window.addEventListener('hashchange', () => App.route());

    // Initial route
    if (!location.hash || location.hash === '#/') {
      location.hash = '#/overview';
    } else {
      App.route();
    }
  },

  route() {
    const hash = location.hash.replace('#/', '') || 'overview';
    const page = hash.split('?')[0];

    if (page === App.currentPage) return;
    App.currentPage = page;

    Sidebar.render(page);
    App.renderPage(page);
  },

  renderPage(page) {
    const el = document.getElementById('page');
    const titles = {
      overview:  'Overview',
      themes:    'Themes',
      plugins:   'Plugins',
      registry:  'Registry',
      workflows: 'Workflows',
      cron:      'Cron Jobs',
      intros:    'Intros',
      system:    'System',
    };

    const title = titles[page] || page;
    el.innerHTML = `
      <h1 class="page-title">${title}</h1>
      <div class="empty-state">
        <div class="empty-state-icon">${Sidebar.pages.find(p => p.id === page)?.icon || '\u2699'}</div>
        <p>This page will be implemented in a future phase.</p>
      </div>
    `;
  },

  connectSSE() {
    if (App.eventSource) App.eventSource.close();

    App.eventSource = new EventSource('/api/events');
    App.eventSource.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        App.onEvent(data);
      } catch (_) {}
    };
    App.eventSource.onerror = () => {
      // Reconnect after 3s on error
      setTimeout(() => App.connectSSE(), 3000);
    };
  },

  onEvent(data) {
    // Dispatch to active page — pages will register handlers in later phases
    if (typeof App.pageHandler === 'function') {
      App.pageHandler(data);
    }
  },

  toast(message, type = 'info') {
    const container = document.getElementById('toast-container');
    const el = document.createElement('div');
    el.className = `toast toast-${type}`;
    el.textContent = message;
    container.appendChild(el);
    setTimeout(() => el.remove(), 4000);
  },
};

// Boot
document.addEventListener('DOMContentLoaded', () => App.init());
