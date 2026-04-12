// Lynx Dashboard — reusable color picker component

const ColorPicker = {
  namedColors: [],

  async loadColors() {
    if (this.namedColors.length) return;
    try {
      this.namedColors = await Api.get('/api/colors');
    } catch (_) {}
  },

  resolveColor(val, palette) {
    if (!val) return '';
    palette = palette || {};
    if (val.startsWith('$')) {
      return palette[val.slice(1)] || val;
    }
    return val;
  },

  createRow(key, val, palette, onChanged) {
    const row = document.createElement('div');
    row.className = 'color-row';

    const swatch = document.createElement('div');
    swatch.className = 'color-preview';
    swatch.style.background = this.resolveColor(val, palette);

    const label = document.createElement('label');
    label.textContent = key;

    const input = document.createElement('input');
    input.className = 'color-input input';
    input.type = 'text';
    input.value = val;

    input.addEventListener('input', () => {
      swatch.style.background = this.resolveColor(input.value, palette);
    });
    input.addEventListener('change', () => onChanged(input.value));

    swatch.addEventListener('click', () => {
      const picker = document.createElement('input');
      picker.type = 'color';
      const resolved = this.resolveColor(input.value, palette);
      picker.value = resolved.startsWith('#') ? resolved : '#888888';
      picker.addEventListener('input', () => {
        input.value = picker.value;
        swatch.style.background = picker.value;
        onChanged(picker.value);
      });
      picker.click();
    });

    row.appendChild(swatch);
    row.appendChild(label);
    row.appendChild(input);
    return row;
  },
};
