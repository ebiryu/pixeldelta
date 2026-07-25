// Expand / collapse entries.
document.querySelectorAll('.entry-head').forEach(h => {
  h.addEventListener('click', () => h.closest('.entry').classList.toggle('open'));
});

// Category filter.
const chips = document.querySelectorAll('#summary .chip');
function applyFilter() {
  const on = new Set([...chips].filter(c => c.getAttribute('aria-pressed') === 'true').map(c => c.dataset.cat));
  document.querySelectorAll('.entry').forEach(e => { e.hidden = !on.has(e.dataset.cat); });
}
chips.forEach(c => c.addEventListener('click', () => {
  c.setAttribute('aria-pressed', c.getAttribute('aria-pressed') === 'true' ? 'false' : 'true');
  applyFilter();
}));
applyFilter();

// Viewer: mode switch, slider drag, onion opacity.
document.querySelectorAll('.entry').forEach(entry => {
  const stage = entry.querySelector('.stage');
  if (!stage) return;
  const segBtns = entry.querySelectorAll('.seg button');
  segBtns.forEach(b => b.addEventListener('click', () => {
    segBtns.forEach(x => x.setAttribute('aria-pressed', 'false'));
    b.setAttribute('aria-pressed', 'true');
    const mode = b.dataset.mode;
    stage.dataset.mode = mode;
    const onionCtl = entry.querySelector('.onion-ctl');
    if (onionCtl) onionCtl.style.display = mode === 'onion' ? 'flex' : 'none';
  }));
  const handle = stage.querySelector('.handle');
  const stack = stage.querySelector('.stack');
  if (handle && stack) {
    let drag = false;
    const move = e => {
      if (!drag) return;
      const r = stack.getBoundingClientRect();
      const x = ((e.touches ? e.touches[0].clientX : e.clientX) - r.left) / r.width;
      stage.style.setProperty('--split', Math.max(0, Math.min(1, x)) * 100 + '%');
    };
    handle.addEventListener('mousedown', () => drag = true);
    handle.addEventListener('touchstart', () => drag = true, { passive: true });
    window.addEventListener('mousemove', move);
    window.addEventListener('touchmove', move, { passive: true });
    window.addEventListener('mouseup', () => drag = false);
    window.addEventListener('touchend', () => drag = false);
  }
  const op = entry.querySelector('.op-range');
  if (op) op.addEventListener('input', () => stage.style.setProperty('--op', op.value / 100));
});

// Theme toggle.
const root = document.documentElement;
document.getElementById('theme').addEventListener('click', () => {
  const cur = root.getAttribute('data-theme')
    || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  root.setAttribute('data-theme', cur === 'dark' ? 'light' : 'dark');
});
