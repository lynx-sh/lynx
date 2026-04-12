// Lynx Dashboard — Intros page

const IntrosPage = {
  async render(el) {
    el.innerHTML = `
      <h1 class="page-title">Intros</h1>
      <div id="intro-list" class="card-grid"></div>
      <div class="card" style="margin-top:20px;">
        <div class="section-title">Preview</div>
        <div id="intro-preview"
          style="background:#0d0e17; border:1px solid var(--border); border-radius:8px;
                 padding:20px; font-family:var(--font-mono); font-size:13px; line-height:1.4;
                 white-space:pre; overflow-x:auto; min-height:100px; color:var(--text-primary);">
          Select an intro to preview.
        </div>
      </div>
    `;
    await this.loadIntros();
  },

  async loadIntros() {
    const el = document.getElementById('intro-list');
    if (!el) return;
    try {
      const data = await Api.intros();
      const intros = data.intros || [];
      const enabled = data.enabled;

      el.innerHTML = intros.map(i => `
        <div class="card" style="cursor:pointer; ${i.active ? 'border-color:var(--accent);' : ''}"
             data-intro="${i.slug}">
          <div style="display:flex; justify-content:space-between; align-items:center;">
            <span style="font-weight:600;">${i.name}</span>
            <div style="display:flex; gap:4px;">
              ${i.builtin ? '<span class="badge">builtin</span>' : '<span class="badge badge-info">custom</span>'}
              ${i.active ? '<span class="badge badge-success">active</span>' : ''}
            </div>
          </div>
        </div>
      `).join('');

      el.querySelectorAll('[data-intro]').forEach(card => {
        card.addEventListener('click', async () => {
          const slug = card.dataset.intro;
          await this.previewIntro(slug);
          // Set as active
          try {
            await Api.introSet(slug);
            App.toast(`Switched to '${slug}'`, 'success');
            this.loadIntros();
          } catch (err) {
            App.toast('Error: ' + err.message, 'error');
          }
        });
      });
    } catch (_) {
      el.innerHTML = '<div class="empty-state"><p>Failed to load intros.</p></div>';
    }
  },

  async previewIntro(slug) {
    const el = document.getElementById('intro-preview');
    if (!el) return;
    try {
      const data = await Api.introPreview(slug);
      el.textContent = data.rendered || '(empty)';
    } catch (_) {
      el.textContent = 'Failed to load preview.';
    }
  },
};
