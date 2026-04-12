// Lynx Dashboard — Workflows page

const WorkflowsPage = {
  async render(el) {
    el.innerHTML = `<h1 class="page-title">Workflows</h1><div id="wf-content"></div>`;
    try {
      const data = await Api.workflows();
      // If we get data, render workflows
      const workflows = data.workflows || [];
      if (!workflows.length) {
        this.showPending(el);
        return;
      }
      // Future: render workflow cards
    } catch (e) {
      this.showPending(el);
    }
  },

  showPending(el) {
    document.getElementById('wf-content').innerHTML = `
      <div class="card" style="text-align:center; padding:48px;">
        <div style="font-size:32px; margin-bottom:12px;">\u21BB</div>
        <p style="color:var(--text-secondary);">
          The workflow engine is not yet available.<br>
          This feature is coming with Block 19.
        </p>
      </div>
    `;
  },
};
