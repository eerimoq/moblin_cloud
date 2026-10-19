type Piece = {
  x: number;
  y: number;
  vx: number;
  vy: number;
  angle: number;
  spin: number;
  width: number;
  height: number;
  color: string;
};

const colorTokens = [
  "--color-confetti-red",
  "--color-confetti-blue",
  "--color-confetti-purple",
  "--color-amber",
  "--color-leaf",
  "--color-wheel-5",
  "--color-wheel-6",
];

export function fireConfetti(parent: HTMLElement) {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
    return;
  }
  const canvas = document.createElement("canvas");
  canvas.setAttribute("aria-hidden", "true");
  canvas.style.cssText = "position:absolute;inset:0;width:100%;height:100%;pointer-events:none";
  parent.append(canvas);
  const context = canvas.getContext("2d");
  if (!context) {
    canvas.remove();
    return;
  }
  const scale = devicePixelRatio || 1;
  const width = innerWidth;
  const height = innerHeight;
  canvas.width = width * scale;
  canvas.height = height * scale;
  context.scale(scale, scale);
  const style = getComputedStyle(document.documentElement);
  const colors = colorTokens.map((token) => style.getPropertyValue(token).trim()).filter(Boolean);
  const speed = Math.sqrt(width * width + height * height) / 55;
  const pieces: Piece[] = [];
  for (const side of [-1, 1]) {
    const x = side < 0 ? 0 : width;
    const direction = Math.atan2(height * 0.45 - height, width / 2 - x);
    for (let i = 0; i < 90; i++) {
      const angle = direction + (Math.random() - 0.5) * 0.7;
      const velocity = speed * (0.6 + Math.random() * 0.6);
      pieces.push({
        x,
        y: height,
        vx: Math.cos(angle) * velocity,
        vy: Math.sin(angle) * velocity,
        angle: Math.random() * Math.PI * 2,
        spin: (Math.random() - 0.5) * 0.4,
        width: 6 + Math.random() * 6,
        height: 8 + Math.random() * 8,
        color: colors[Math.floor(Math.random() * colors.length)] ?? "#f6d21b",
      });
    }
  }
  const start = performance.now();
  let last = start;
  const frame = (now: number) => {
    const step = Math.min((now - last) / 16.7, 3);
    last = now;
    context.clearRect(0, 0, width, height);
    const fade = Math.max(0, Math.min(1, (4500 - (now - start)) / 1000));
    for (const piece of pieces) {
      piece.vx *= 0.985 ** step;
      piece.vy = piece.vy * 0.985 ** step + 0.25 * step;
      piece.x += piece.vx * step;
      piece.y += piece.vy * step;
      piece.angle += piece.spin * step;
      context.save();
      context.globalAlpha = fade;
      context.translate(piece.x, piece.y);
      context.rotate(piece.angle);
      context.scale(1, Math.cos(piece.angle * 2));
      context.fillStyle = piece.color;
      context.fillRect(-piece.width / 2, -piece.height / 2, piece.width, piece.height);
      context.restore();
    }
    if (fade > 0 && pieces.some((piece) => piece.y < height + 20)) {
      requestAnimationFrame(frame);
    } else {
      canvas.remove();
    }
  };
  requestAnimationFrame(frame);
}
