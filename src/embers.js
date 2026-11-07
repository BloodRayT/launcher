/** Искры феникса: медленно поднимаются над фоном. Отключаются при «уменьшении движения». */

const EMBER_COUNT = 34;
const MAX_DPR = 1.5;

function spawn(width, height, fromBottom) {
  return {
    x: Math.random() * width,
    y: fromBottom ? height + Math.random() * 40 : Math.random() * height,
    radius: 0.6 + Math.random() * 1.8,
    speed: 0.18 + Math.random() * 0.45,
    sway: 0.4 + Math.random() * 1.2,
    phase: Math.random() * Math.PI * 2,
    warmth: Math.random(),
  };
}

/**
 * @param {HTMLCanvasElement} canvas
 * @returns {{ setEnabled(on: boolean): void }} выключенные искры не рисуются и не тратят процессор
 */
export function startEmbers(canvas) {
  const idle = { setEnabled: () => {} };
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return idle;
  const context = canvas.getContext('2d');
  if (!context) return idle;
  let enabled = true;

  let width = 0;
  let height = 0;
  let embers = [];
  let frame = 0;

  const resize = () => {
    const ratio = Math.min(window.devicePixelRatio || 1, MAX_DPR);
    width = window.innerWidth;
    height = window.innerHeight;
    canvas.width = Math.round(width * ratio);
    canvas.height = Math.round(height * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    embers = Array.from({ length: EMBER_COUNT }, () => spawn(width, height, false));
  };

  const draw = (time) => {
    context.clearRect(0, 0, width, height);
    context.globalCompositeOperation = 'lighter';
    embers = embers.map((ember) => {
      const next = { ...ember, y: ember.y - ember.speed };
      if (next.y < -20) return spawn(width, height, true);
      const x = next.x + Math.sin(time / 1400 + next.phase) * next.sway * 6;
      const fade = Math.min(1, next.y / height + 0.15);
      const glow = context.createRadialGradient(x, next.y, 0, x, next.y, next.radius * 6);
      const green = Math.round(150 + next.warmth * 70);
      glow.addColorStop(0, `rgba(255, ${green}, 80, ${0.55 * fade})`);
      glow.addColorStop(1, 'rgba(255, 120, 40, 0)');
      context.fillStyle = glow;
      context.beginPath();
      context.arc(x, next.y, next.radius * 6, 0, Math.PI * 2);
      context.fill();
      return next;
    });
    frame = requestAnimationFrame(draw);
  };

  resize();
  window.addEventListener('resize', resize);
  document.addEventListener('visibilitychange', () => {
    cancelAnimationFrame(frame);
    if (!document.hidden && enabled) frame = requestAnimationFrame(draw);
  });
  frame = requestAnimationFrame(draw);

  return {
    setEnabled(on) {
      enabled = on;
      cancelAnimationFrame(frame);
      if (on && !document.hidden) frame = requestAnimationFrame(draw);
      else context.clearRect(0, 0, width, height);
    },
  };
}
