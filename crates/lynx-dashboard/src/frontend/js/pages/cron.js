// Lynx Dashboard — Cron page

const CronPage = {
  async render(el) {
    el.innerHTML = `
      <h1 class="page-title">Scheduled Tasks</h1>
      <div id="cron-list"></div>
      <div class="card" style="margin-top:20px;">
        <div class="section-title">Add Task</div>
        <div style="display:flex; flex-direction:column; gap:10px;">
          <div style="display:flex; gap:10px;">
            <input type="text" class="input" id="cron-name" placeholder="Task name" style="flex:1;">
            <input type="text" class="input" id="cron-schedule" placeholder="*/5 * * * *" style="width:150px;">
          </div>
          <div style="display:flex; gap:10px;">
            <input type="text" class="input" id="cron-cmd" placeholder="Command to run" style="flex:1;">
            <button class="btn btn-primary" id="cron-add">Add</button>
          </div>
        </div>
      </div>
    `;

    document.getElementById('cron-add')?.addEventListener('click', () => this.addTask());
    await this.loadTasks();

    App.pageHandler = (data) => {
      if (data.type === 'cron_updated') this.loadTasks();
    };
  },

  async loadTasks() {
    const el = document.getElementById('cron-list');
    if (!el) return;
    try {
      const data = await Api.cron();
      const tasks = data.tasks || [];
      if (!tasks.length) {
        el.innerHTML = '<div class="empty-state"><p>No scheduled tasks.</p></div>';
        return;
      }
      el.innerHTML = `<table class="table">
        <thead><tr><th>Name</th><th>Schedule</th><th>Command</th><th></th></tr></thead>
        <tbody>${tasks.map(t => `<tr>
          <td style="font-family:var(--font-mono); font-weight:500;">${t.name}</td>
          <td><code>${t.schedule}</code></td>
          <td style="color:var(--text-secondary); font-size:13px; font-family:var(--font-mono);">${t.command}</td>
          <td><button class="btn btn-danger" data-remove="${t.name}" title="Remove">\u00D7</button></td>
        </tr>`).join('')}</tbody>
      </table>`;

      el.querySelectorAll('[data-remove]').forEach(btn => {
        btn.addEventListener('click', async () => {
          try {
            await Api.cronRemove(btn.dataset.remove);
            App.toast('Task removed');
            this.loadTasks();
          } catch (err) {
            App.toast('Error: ' + err.message, 'error');
          }
        });
      });
    } catch (_) {
      el.innerHTML = '<div class="empty-state"><p>Failed to load tasks.</p></div>';
    }
  },

  async addTask() {
    const name = document.getElementById('cron-name')?.value?.trim();
    const schedule = document.getElementById('cron-schedule')?.value?.trim();
    const command = document.getElementById('cron-cmd')?.value?.trim();
    if (!name || !schedule || !command) {
      App.toast('All fields are required', 'error');
      return;
    }
    try {
      await Api.cronAdd({ name, command, schedule });
      App.toast(`Added task '${name}'`, 'success');
      document.getElementById('cron-name').value = '';
      document.getElementById('cron-schedule').value = '';
      document.getElementById('cron-cmd').value = '';
      this.loadTasks();
    } catch (err) {
      App.toast('Error: ' + err.message, 'error');
    }
  },
};
