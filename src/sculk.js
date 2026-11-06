/**
 * Прожилки скалка на фоне. Холст рисуется в низком разрешении (одна клетка — несколько
 * пикселей экрана) и растягивается с чёткими пикселями: так он похож на блоки Minecraft
 * и почти не нагружает видеокарту. Импульс-волна запускается событием `fenix:pulse`.
 */

const CELL = 4;
const FPS = 24;
const VEIN_SEEDS = 28;
const MOTES = 18;
const PULSE_SPEED = 0.09; // клеток за миллисекунду
const PULSE_BAND = 7;
const PULSE_LIFE_MS = 2600;
/** Постоянное зерно: рисунок прожилок не меняется при изменении размера окна. */
const VEIN_SEED = 918616;
const MAX_PULSES = 6;
const DIRECTIONS = [[1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1]];

const DIM = [15, 191, 116];
const LIT = [24, 255, 140];

/** Детерминированный генератор (mulberry32). */
function seededRandom(seed) {
  let state = seed;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Ветвящиеся прожилки: случайное блуждание с инерцией направления. */
function growVeins(cols, rows) {
  const random = seededRandom(VEIN_SEED);
  const randomInt = (max) => Math.floor(random() * max);
  const cells = [];
  const walk = (x, y, dir, length, depth) => {
    let cx = x;
    let cy = y;
    let d = dir;
    for (let step = 0; step < length; step += 1) {
      if (random() < 0.28) d = (d + (random() < 0.5 ? 1 : 7)) % 8;
      cx += DIRECTIONS[d][0];
      cy += DIRECTIONS[d][1];
      if (cx < 0 || cy < 0 || cx >= cols || cy >= rows) return;
      cells.push({ x: cx, y: cy, phase: (step / 9) + depth, weight: 1 - step / (length * 1.6) });
      if (depth < 2 && random() < 0.035) walk(cx, cy, (d + 2) % 8, Math.floor(length / 2), depth + 1);
    }
  };
  for (let seed = 0; seed < VEIN_SEEDS; seed += 1) {
    // Прожилки растут от всех четырёх краёв к центру.
    const side = seed % 4;
    const x = side === 0 ? 0 : side === 1 ? cols - 1 : randomInt(cols);
    const y = side === 2 ? 0 : side === 3 ? rows - 1 : randomInt(rows);
    walk(x, y, [0, 4, 2, 6][side], 50 + randomInt(110), 0);
  }
  return cells;
}

function spawnMote(cols, rows, fromBottom) {
  return {
    x: Math.random() * cols,
    y: fromBottom ? rows + Math.random() * 10 : Math.random() * rows,
    speed: 0.02 + Math.random() * 0.04,
    glow: 0.25 + Math.random() * 0.45,
  };
}

/** @param {HTMLCanvasElement} canvas */
export function startSculk(canvas) {
  const context = canvas.getContext('2d');
  if (!context) return;
  const still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  let cols = 0;
  let rows = 0;
  let veins = [];
  let motes = [];
  let pulses = [];
  let image = null;
  let pixels = null;
  let frame = 0;
  let lastDraw = 0;

  const plot = (x, y, rgb, alpha) => {
    const ix = Math.round(x);
    const iy = Math.round(y);
    if (ix < 0 || iy < 0 || ix >= cols || iy >= rows) return;
    const at = (iy * cols + ix) * 4;
    const a = Math.min(255, Math.round(alpha * 255));
    if (a <= pixels[at + 3]) return;
    pixels[at] = rgb[0];
    pixels[at + 1] = rgb[1];
    pixels[at + 2] = rgb[2];
    pixels[at + 3] = a;
  };

  const draw = (time) => {
    pixels.fill(0);
    pulses = pulses.filter((pulse) => time - pulse.start < PULSE_LIFE_MS);
    for (const cell of veins) {
      const breath = still ? 0.5 : 0.5 + 0.5 * Math.sin(time / 2200 + cell.phase);
      let light = 0;
      for (const pulse of pulses) {
        const radius = (time - pulse.start) * PULSE_SPEED;
        const gap = Math.abs(Math.hypot(cell.x - pulse.x, cell.y - pulse.y) - radius);
        if (gap < PULSE_BAND) {
          const fade = 1 - (time - pulse.start) / PULSE_LIFE_MS;
          light = Math.max(light, (1 - gap / PULSE_BAND) * fade * pulse.strength);
        }
      }
      if (light > 0.05) plot(cell.x, cell.y, LIT, 0.45 + light * 0.55);
      else plot(cell.x, cell.y, DIM, (0.32 + breath * 0.3) * Math.max(0.4, cell.weight));
    }
    motes = motes.map((mote) => {
      const next = { ...mote, y: mote.y - mote.speed * (1000 / FPS) / 16 };
      if (next.y < -2) return spawnMote(cols, rows, true);
      plot(next.x, next.y, LIT, next.glow * Math.min(1, next.y / rows + 0.2));
      return next;
    });
    context.putImageData(image, 0, 0);
  };

  const loop = (time) => {
    frame = requestAnimationFrame(loop);
    if (time - lastDraw < 1000 / FPS) return;
    lastDraw = time;
    draw(time);
  };

  const resize = () => {
    cols = Math.ceil(window.innerWidth / CELL);
    rows = Math.ceil(window.innerHeight / CELL);
    canvas.width = cols;
    canvas.height = rows;
    image = context.createImageData(cols, rows);
    pixels = image.data;
    veins = growVeins(cols, rows);
    motes = still ? [] : Array.from({ length: MOTES }, () => spawnMote(cols, rows, false));
    draw(performance.now());
  };

  resize();
  let resizeTimer = 0;
  window.addEventListener('resize', () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(resize, 150);
  });
  if (still) return;

  window.addEventListener('fenix:pulse', (event) => {
    const { x = window.innerWidth / 2, y = window.innerHeight / 2, strength = 1 } = event.detail ?? {};
    // Не больше нескольких волн сразу: зажатая клавиша не нагружает слабый ПК.
    pulses = [...pulses, { x: x / CELL, y: y / CELL, start: performance.now(), strength }].slice(-MAX_PULSES);
  });
  document.addEventListener('visibilitychange', () => {
    cancelAnimationFrame(frame);
    if (!document.hidden) frame = requestAnimationFrame(loop);
  });
  frame = requestAnimationFrame(loop);
}

/** Запускает импульс из центра элемента. */
export function pulseFrom(element, strength = 1) {
  const box = element?.getBoundingClientRect?.();
  if (!box) return;
  window.dispatchEvent(new CustomEvent('fenix:pulse', {
    detail: { x: box.left + box.width / 2, y: box.top + box.height / 2, strength },
  }));
}
