// Lynx Dashboard — Themes page (WYSIWYG editor ported from studio)

const ThemesPage = {
  theme: {},
  patchTimer: null,

  async render(el) {
    el.innerHTML = `
      <h1 class="page-title">Theme Editor</h1>
      <div class="theme-editor">
        <div class="theme-sidebar">
          <div class="theme-section">
            <div class="section-title">Palette</div>
            <div id="te-palette"></div>
          </div>
          <div class="theme-section">
            <div class="section-title">Separators</div>
            <div id="te-separators"></div>
          </div>
          <div class="theme-section">
            <div class="section-title">Segments \u2014 Left</div>
            <ul class="segment-list" id="te-seg-left" data-side="left"></ul>
          </div>
          <div class="theme-section">
            <div class="section-title">Segments \u2014 Right</div>
            <ul class="segment-list" id="te-seg-right" data-side="right"></ul>
          </div>
        </div>
        <div class="theme-content">
          <div class="preview-terminal">
            <div class="terminal-titlebar">
              <div class="term-btn" style="background:#ff5f57"></div>
              <div class="term-btn" style="background:#ffbd2e"></div>
              <div class="term-btn" style="background:#28ca41"></div>
            </div>
            <div class="terminal-body">
              <div class="prompt-line" id="te-preview"></div>
              <div style="color:#4a5568; margin-top:8px; font-size:12px">
                ls -la  \u2192  exa --long
              </div>
            </div>
          </div>
          <div id="te-meta" style="font-size:13px; color:var(--text-secondary);"></div>
          <div class="theme-actions">
            <button class="btn btn-primary" id="te-apply">Apply Theme</button>
            <button class="btn" id="te-reset">Reset to Saved</button>
            <span class="save-status" id="te-status"></span>
          </div>
        </div>
      </div>
    `;

    await this.loadTheme();
    this.bindActions();

    App.pageHandler = (data) => {
      if (data.type === 'theme_updated') this.loadTheme();
    };
  },

  async loadTheme() {
    try {
      this.theme = await Api.theme();
      this.renderAll();
    } catch (e) {
      App.toast('Failed to load theme: ' + e.message, 'error');
    }
  },

  renderAll() {
    const t = this.theme;
    const meta = document.getElementById('te-meta');
    if (meta) {
      meta.innerHTML = `<strong>${t.meta?.name || '?'}</strong> \u2014 ${t.meta?.description || ''}
        <br><span style="font-size:11px; color:var(--text-muted)">by ${t.meta?.author || 'unknown'}</span>`;
    }
    this.renderPalette();
    this.renderSeparators();
    this.renderSegments();
    this.renderPreview();
  },

  renderPalette() {
    const el = document.getElementById('te-palette');
    if (!el) return;
    el.innerHTML = '';
    const colors = this.theme.colors || {};
    for (const [key, val] of Object.entries(colors)) {
      el.appendChild(ColorPicker.createRow(key, val, colors, (v) => {
        this.schedulePatch(`colors.${key}`, v);
      }));
    }
  },

  renderSeparators() {
    const el = document.getElementById('te-separators');
    if (!el) return;
    el.innerHTML = '';
    const fields = [
      ['left', 'separators.left.char'],
      ['right', 'separators.right.char'],
      ['l-edge', 'separators.left_edge.char'],
      ['r-edge', 'separators.right_edge.char'],
    ];
    for (const [lbl, path] of fields) {
      const parts = path.split('.');
      const val = parts.reduce((o, k) => o?.[k], this.theme) || '';
      const row = document.createElement('div');
      row.className = 'separator-row';
      const label = document.createElement('label');
      label.textContent = lbl;
      const input = document.createElement('input');
      input.className = 'sep-input input';
      input.type = 'text';
      input.value = val;
      input.addEventListener('change', () => this.schedulePatch(path, input.value));
      row.appendChild(label);
      row.appendChild(input);
      el.appendChild(row);
    }
  },

  renderSegments() {
    for (const side of ['left', 'right']) {
      const ul = document.getElementById(`te-seg-${side}`);
      if (!ul) continue;
      ul.innerHTML = '';
      const order = this.theme.segments?.[side]?.order || [];
      for (const seg of order) {
        ul.appendChild(this.makeSegItem(seg, side));
      }
      this.setupDragDrop(ul, side);
    }
  },

  makeSegItem(name, side) {
    const li = document.createElement('li');
    li.className = 'segment-item';
    li.draggable = true;
    li.dataset.name = name;
    li.innerHTML = `
      <span class="drag-handle">\u2807</span>
      <span class="segment-badge">${name}</span>
      <span class="side-label">${side}</span>
      <button class="btn btn-danger" style="padding:2px 8px; font-size:11px" title="Remove">\u00D7</button>
    `;
    li.querySelector('button').addEventListener('click', () => {
      this.patchSegment('remove', name, side);
    });
    return li;
  },

  setupDragDrop(ul, side) {
    let dragged = null;
    ul.addEventListener('dragstart', e => {
      dragged = e.target.closest('li');
      if (dragged) dragged.style.opacity = '0.4';
    });
    ul.addEventListener('dragend', () => {
      if (dragged) dragged.style.opacity = '';
      dragged = null;
      this.commitSegmentOrder(ul, side);
    });
    ul.addEventListener('dragover', e => {
      e.preventDefault();
      const over = e.target.closest('li');
      if (over && over !== dragged) {
        const rect = over.getBoundingClientRect();
        const after = e.clientY > rect.top + rect.height / 2;
        ul.insertBefore(dragged, after ? over.nextSibling : over);
      }
    });
  },

  async commitSegmentOrder(ul, side) {
    const names = [...ul.querySelectorAll('li')].map(li => li.dataset.name);
    try {
      await Api.themeSegmentOrder(side, names);
      App.toast('Segment order updated');
    } catch (e) {
      App.toast('Error: ' + e.message, 'error');
    }
  },

  async patchSegment(op, name, side) {
    try {
      const updated = await Api.themeSegment(op, name, side);
      this.theme = updated;
      this.renderAll();
      App.toast(`${op} ${name}`);
    } catch (e) {
      App.toast('Error: ' + e.message, 'error');
    }
  },

  schedulePatch(path, value) {
    clearTimeout(this.patchTimer);
    this.patchTimer = setTimeout(() => this.applyPatch(path, value), 400);
  },

  async applyPatch(path, value) {
    const status = document.getElementById('te-status');
    if (status) status.textContent = 'saving\u2026';
    try {
      const updated = await Api.themePatch(path, value);
      this.theme = updated;
      this.renderPreview();
      if (status) {
        status.textContent = 'saved';
        setTimeout(() => { if (status) status.textContent = ''; }, 2000);
      }
    } catch (e) {
      App.toast('Error: ' + e.message, 'error');
      if (status) status.textContent = 'error';
    }
  },

  renderPreview() {
    const line = document.getElementById('te-preview');
    if (!line) return;
    line.innerHTML = '';
    const palette = this.theme.colors || {};
    const leftOrder = this.theme.segments?.left?.order || [];
    const seps = this.theme.separators || {};
    const leftSep = seps.left?.char || ' ';

    const segData = {
      dir: '\uD83D\uDCC2 ~/projects/lynx',
      git_branch: '\uE0A0 main',
      git_status: '\u2713',
      git_stash: '\u2691 1',
      git_sha: 'a5bce2f',
      git_time_since_commit: '2h',
      cmd_duration: '\u23F1 1.2s',
      username: 'proxy',
      hostname: 'mac',
      exit_code: '0',
      venv: 'venv',
      node_version: '20.11',
      rust_version: '\uD83E\uDD80 stable',
      aws_profile: 'dev',
      hist_number: '1234',
      prompt_char: '\u276F',
    };

    leftOrder.forEach((name, i) => {
      const text = segData[name] || name;
      const segCfg = this.theme.segment?.[name] || {};
      const fg = ColorPicker.resolveColor(segCfg?.color?.fg || '', palette) || 'var(--text-primary)';
      const bg = ColorPicker.resolveColor(segCfg?.color?.bg || '', palette);
      const span = document.createElement('span');
      span.className = 'seg';
      span.textContent = text;
      span.style.color = fg;
      if (bg) span.style.background = bg;
      line.appendChild(span);
      if (i < leftOrder.length - 1 && leftSep) {
        const s = document.createElement('span');
        s.style.color = ColorPicker.resolveColor(seps.left?.color || '#4a5568', palette);
        s.style.fontSize = '16px';
        s.textContent = leftSep;
        line.appendChild(s);
      }
    });

    // Prompt char
    const pChar = this.theme.segment?.prompt_char || {};
    const caretColor = ColorPicker.resolveColor(pChar?.color?.fg || '', palette) || 'var(--accent)';
    const caretSym = pChar?.symbol || '\u276F';
    const caret = document.createElement('span');
    caret.style.color = caretColor;
    caret.style.marginLeft = '4px';
    caret.style.fontWeight = 'bold';
    caret.textContent = caretSym;
    line.appendChild(caret);

    const cursor = document.createElement('span');
    cursor.className = 'prompt-cursor';
    line.appendChild(cursor);
  },

  bindActions() {
    document.getElementById('te-apply')?.addEventListener('click', async () => {
      try {
        await Api.themeApply();
        App.toast('Theme applied!', 'success');
      } catch (e) {
        App.toast('Apply failed: ' + e.message, 'error');
      }
    });
    document.getElementById('te-reset')?.addEventListener('click', async () => {
      try {
        await Api.themeReset();
        await this.loadTheme();
        App.toast('Reset to saved theme');
      } catch (e) {
        App.toast('Reset failed: ' + e.message, 'error');
      }
    });
  },
};
