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

// Viewer: mode switch, zoom switch, slider drag, onion opacity, side-by-side
// scroll sync.
document.querySelectorAll('.entry').forEach(entry => {
  const stage = entry.querySelector('.stage');
  if (!stage) return;
  // Each segmented control only touches the buttons in its own group: the
  // zoom control sits next to the mode control in the same `.seg` markup,
  // and a shared selector would reset the wrong group's `aria-pressed`.
  const modeBtns = entry.querySelectorAll('.seg button[data-mode]');
  modeBtns.forEach(b => b.addEventListener('click', () => {
    modeBtns.forEach(x => x.setAttribute('aria-pressed', 'false'));
    b.setAttribute('aria-pressed', 'true');
    const mode = b.dataset.mode;
    stage.dataset.mode = mode;
    const onionCtl = entry.querySelector('.onion-ctl');
    if (onionCtl) onionCtl.style.display = mode === 'onion' ? 'flex' : 'none';
  }));
  const zoomBtns = entry.querySelectorAll('.seg button[data-zoom]');
  zoomBtns.forEach(b => b.addEventListener('click', () => {
    zoomBtns.forEach(x => x.setAttribute('aria-pressed', 'false'));
    b.setAttribute('aria-pressed', 'true');
    stage.dataset.zoom = b.dataset.zoom;
  }));
  const handle = stage.querySelector('.handle');
  // The overlay's frame is sized to the image at the current zoom level, so
  // it is what the drag position is measured against, not the `.stack` pane
  // around it (which can be wider once the pane scrolls).
  const frame = stage.querySelector('.m-overlay .frame');
  if (handle && frame) {
    let drag = false;
    const move = e => {
      if (!drag) return;
      const r = frame.getBoundingClientRect();
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
  // Mirror scroll position between the two side-by-side panes, so scrolling
  // one at a zoom level moves the other to the same spot. Setting the other
  // pane's position queues a scroll event of its own, which the browser fires
  // before the next frame's callbacks, so `syncing` is cleared a frame later
  // rather than on the next line. Clearing it synchronously would let the
  // mirrored scroll write back to the pane the reader is dragging, which
  // fights the drag whenever the two panes can scroll different distances.
  const sidePanes = entry.querySelectorAll('.m-side .pane');
  if (sidePanes.length === 2) {
    const [a, b] = sidePanes;
    let syncing = false;
    const mirror = (from, to) => () => {
      if (syncing) return;
      syncing = true;
      to.scrollLeft = from.scrollLeft;
      to.scrollTop = from.scrollTop;
      requestAnimationFrame(() => { syncing = false; });
    };
    a.addEventListener('scroll', mirror(a, b));
    b.addEventListener('scroll', mirror(b, a));
  }
});

// Theme toggle.
const root = document.documentElement;
document.getElementById('theme').addEventListener('click', () => {
  const cur = root.getAttribute('data-theme')
    || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  root.setAttribute('data-theme', cur === 'dark' ? 'light' : 'dark');
});
