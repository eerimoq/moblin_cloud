import { asset } from "./asset";
const MOUTH =
  "m 135.29273,148.41464 c -4.41659,1.4602 -9.07107,1.57648 -16.05635,2.47412 -10.80109,1.38798 -13.75474,1.62222 -15.70254,3.85011 -2.5424,2.90799 -4.305824,6.768 -2.52151,9.20747 1.55524,2.12628 3.65572,3.33748 7.22763,3.33748 3.57191,0 9.72606,-1.99618 14.67851,-1.90143 5.58164,0.10679 11.22154,-2.61686 15.61774,-3.71591 4.63695,-3.18801 3.41254,-4.47987 3.63809,-7.42886 -1.13681,-4.02316 -2.2876,-6.70648 -6.88157,-5.82298 z";
const LEAF =
  "M 100.37154,92.233873 C 97.647491,84.288875 95.823232,81.090308 91.734907,77.055401 86.882916,72.2668 83.678897,71.484802 74.577282,69.069449 c 1.391845,7.292892 4.158537,11.10189 8.582666,15.107639 3.136677,2.840053 6.088521,4.986431 17.211592,8.056785 z";
const FONT = "'Nunito Sans', 'Helvetica Neue', Arial, sans-serif";
const DISPLAY = "Fredoka, 'Nunito Sans', sans-serif";
const MONO = "ui-monospace, Menlo, monospace";

type FaceOptions = {
  color?: string;
  stroke?: string;
  sw?: number;
  bodySw?: number;
  fill?: string;
  dash?: string;
  eyes?:
    | "open"
    | "happy"
    | "closed"
    | "wink"
    | "shades"
    | "x"
    | "sparkle"
    | "robot"
    | "glow"
    | "none";
  mouth?: "logo" | "open" | "big" | "flat" | "o" | "none";
  look?: [number, number];
  leaf?: boolean;
  leafColor?: string;
  rot?: number;
  opacity?: number;
  behind?: string;
  extra?: string;
};

function face(cx: number, cy: number, r: number, o: FaceOptions = {}): string {
  const s = r / 50;
  const col = o.color || "#008000";
  const st = o.stroke || "#1e1a1a";
  const sw = o.sw || 0.6;
  const lx = -2.43;
  const ly = -14.89;
  const rx = 25.48;
  const ry = -17.43;
  const dx = o.look ? o.look[0] : 0;
  const dy = o.look ? o.look[1] : 0;
  const white = (x: number, y: number, k = 1) =>
    `<ellipse cx="${x}" cy="${y}" rx="${11.3 * k}" ry="${12.69 * k}" fill="#fff" stroke="${st}" stroke-width="${sw}"/>`;
  const pupil = (x: number, y: number, k = 1, c = "#000") =>
    `<circle cx="${x + dx}" cy="${y + dy}" r="${6.7 * k}" fill="${c}"/>`;
  const arc = (x: number, y: number) =>
    `<path d="M ${x - 9} ${y + 2} q 9 -12 18 0" fill="none" stroke="${st}" stroke-width="3" stroke-linecap="round"/>`;
  const line = (x: number, y: number) =>
    `<path d="M ${x - 9} ${y} l 18 0" fill="none" stroke="${st}" stroke-width="3" stroke-linecap="round"/>`;
  let eyes = "";
  switch (o.eyes) {
    case "happy":
      eyes = arc(lx, ly) + arc(rx, ry);
      break;
    case "closed":
      eyes = line(lx, ly) + line(rx, ry);
      break;
    case "wink":
      eyes = white(lx, ly) + pupil(lx + 4.4, ly + 1.5) + arc(rx, ry);
      break;
    case "shades":
      eyes =
        white(lx, ly) +
        white(rx, ry) +
        `<path d="M ${lx - 14} ${ly - 8} h 56 v 4 a 12 12 0 0 1 -12 12 h -8 a 10 10 0 0 1 -10 -10 a 10 10 0 0 1 -10 10 h -6 a 10 10 0 0 1 -10 -10 z" fill="#111" stroke="${st}" stroke-width="${sw}"/>`;
      break;
    case "x":
      eyes = `<path d="M ${lx - 6} ${ly - 6} l 12 12 M ${lx + 6} ${ly - 6} l -12 12 M ${rx - 6} ${ry - 6} l 12 12 M ${rx + 6} ${ry - 6} l -12 12" stroke="${st}" stroke-width="3" stroke-linecap="round" fill="none"/>`;
      break;
    case "sparkle":
      eyes =
        white(lx, ly, 1.35) +
        white(rx, ry, 1.35) +
        pupil(lx + 3, ly + 2, 1.35, "#2a6bd6") +
        pupil(rx + 3, ry + 2, 1.35, "#2a6bd6") +
        `<circle cx="${lx - 1}" cy="${ly - 4}" r="3.2" fill="#fff"/><circle cx="${rx - 1}" cy="${ry - 4}" r="3.2" fill="#fff"/><circle cx="${lx + 8}" cy="${ly + 6}" r="1.6" fill="#fff"/><circle cx="${rx + 8}" cy="${ry + 6}" r="1.6" fill="#fff"/>`;
      break;
    case "robot":
      eyes = `<rect x="${lx - 9}" y="${ly - 8}" width="18" height="16" rx="3" fill="#dff" stroke="${st}" stroke-width="${sw}"/><rect x="${rx - 9}" y="${ry - 8}" width="18" height="16" rx="3" fill="#dff" stroke="${st}" stroke-width="${sw}"/><rect x="${lx - 5}" y="${ly - 3}" width="10" height="6" fill="#0aa"/><rect x="${rx - 5}" y="${ry - 3}" width="10" height="6" fill="#0aa"/>`;
      break;
    case "glow":
      eyes =
        `<g filter="url(#glow)">${white(lx, ly)}${white(rx, ry)}</g>` +
        pupil(lx + 4.4, ly + 1.5) +
        pupil(rx + 4.1, ry + 2.7);
      break;
    case "none":
      break;
    default:
      eyes = white(lx, ly) + white(rx, ry) + pupil(lx + 4.4, ly + 1.5) + pupil(rx + 4.1, ry + 2.7);
  }
  let mouth: string;
  switch (o.mouth) {
    case "open":
      mouth = `<ellipse cx="16" cy="21" rx="16" ry="12" fill="#901600" stroke="#000" stroke-width="${sw}"/><ellipse cx="16" cy="28" rx="8" ry="4.5" fill="#d9534f"/>`;
      break;
    case "big":
      mouth = `<ellipse cx="13" cy="22" rx="27" ry="19" fill="#901600" stroke="#000" stroke-width="${sw}"/><ellipse cx="13" cy="33" rx="13" ry="7" fill="#d9534f"/>`;
      break;
    case "flat":
      mouth = `<path d="M 0 21 L 32 19" stroke="#901600" stroke-width="4" stroke-linecap="round" fill="none"/>`;
      break;
    case "o":
      mouth = `<circle cx="16" cy="21" r="7" fill="#901600" stroke="#000" stroke-width="${sw}"/>`;
      break;
    case "none":
      mouth = "";
      break;
    default:
      mouth = `<g transform="translate(-104.847 -140.369)"><path d="${MOUTH}" fill="#901600" stroke="#000" stroke-width="${sw}"/></g>`;
  }
  const leaf =
    o.leaf === false
      ? ""
      : `<g transform="translate(-104.847 -140.369)"><path d="${LEAF}" fill="${o.leafColor || col}" stroke="#000" stroke-width="${sw}"/></g>`;
  const fill = o.fill === undefined ? col : o.fill;
  const body = `<circle r="50" fill="${fill}" stroke="${st}" stroke-width="${o.bodySw || sw}" ${o.dash ? `stroke-dasharray="${o.dash}"` : ""}/>`;
  return `<g transform="translate(${cx} ${cy}) scale(${s})${o.rot ? ` rotate(${o.rot})` : ""}"${o.opacity != null ? ` opacity="${o.opacity}"` : ""}>${o.behind || ""}${body}${leaf}${eyes}${mouth}${o.extra || ""}</g>`;
}

const gem = (x: number, y: number) =>
  `<ellipse cx="${x}" cy="${y}" rx="1.58" ry="4.74" fill="#ff0a00" transform="rotate(-3.64)"/>`;
const KING = `<g transform="translate(-104.847 -140.369)"><path d="m 63.636089,110.01898 -2.728176,-35.146164 10.829966,9.925875 8.500714,-10.814065 10.042946,9.628121 9.582953,-11.391137 9.630658,9.641489 9.66569,-11.021475 8.58851,9.745193 9.00335,-11.348496 2.9833,33.170669 z" fill="#fff80e" stroke="#000" stroke-width="0.6"/>${gem(74.7, 100.74)}${gem(94.33, 100.19)}${gem(114.39, 99.05)}</g>`;

const pill = (x: number, y: number, t: string, bg: string, fg = "#fff", fs = 9, w?: number) => {
  const ww = w || t.length * fs * 0.62 + 14;
  return `<rect x="${x - ww / 2}" y="${y - 8}" width="${ww}" height="16" rx="8" fill="${bg}"/><text x="${x}" y="${y + 3.4}" text-anchor="middle" font-family="${FONT}" font-weight="800" font-size="${fs}" fill="${fg}">${t}</text>`;
};

type TextOptions = {
  anchor?: "start" | "middle" | "end";
  font?: string;
  w?: number;
  fs?: number;
  fill?: string;
  op?: number;
  rot?: number;
  ls?: number;
};

const txt = (x: number, y: number, t: string | number, o: TextOptions = {}) =>
  `<text x="${x}" y="${y}" text-anchor="${o.anchor || "middle"}" font-family="${o.font || FONT}" font-weight="${o.w || 800}" font-size="${o.fs || 10}" fill="${o.fill || "#fff"}"${o.op ? ` opacity="${o.op}"` : ""}${o.ls ? ` letter-spacing="${o.ls}"` : ""}${o.rot ? ` transform="rotate(${o.rot} ${x} ${y})"` : ""}>${t}</text>`;

type PhoneOptions = { rx?: number; body?: string; stroke?: string; screen?: string };

const phone = (x: number, y: number, w: number, h: number, inner: string, o: PhoneOptions = {}) =>
  `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${o.rx || 8}" fill="${o.body || "#1c1c1e"}" stroke="${o.stroke || "#000"}" stroke-width="1"/><rect x="${x + 4}" y="${y + 6}" width="${w - 8}" height="${h - 12}" rx="${(o.rx || 8) - 3}" fill="${o.screen || "#0a0f0a"}"/><rect x="${x + w / 2 - 9}" y="${y + 2}" width="18" height="2.5" rx="1.2" fill="#444"/>${inner}`;

const svgH = (inner: string, defs = "") =>
  `<svg viewBox="0 0 260 150" preserveAspectRatio="xMidYMid slice" role="img">${defs}${inner}</svg>`;
const svgF = (inner: string, defs = "") =>
  `<svg viewBox="0 0 280 392" preserveAspectRatio="xMidYMid slice" role="img">${defs}${inner}</svg>`;
const GLOW =
  '<filter id="glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="4" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>';

export const cardArt = {
  community() {
    let stars = "";
    for (let i = 0; i < 40; i++) {
      const x = (i * 73) % 280,
        y = (i * 131) % 392;
      stars += `<circle cx="${x}" cy="${y}" r="${(i % 3) * 0.5 + 0.6}" fill="#fff" opacity="${0.3 + (i % 5) * 0.12}"/>`;
    }
    let conf = "";
    const cols = ["#ff7ad9", "#f5a623", "#2ecc71", "#2f8cff", "#fff"];
    for (let i = 0; i < 26; i++) {
      const x = (i * 97 + 20) % 270,
        y = (i * 53 + 30) % 150;
      conf += `<rect x="${x}" y="${y}" width="6" height="3" rx="1" fill="${cols[i % 5]}" transform="rotate(${(i * 37) % 90} ${x} ${y})"/>`;
    }
    const petal = (a: number) =>
      `<ellipse cx="0" cy="-6" rx="3.2" ry="5.5" fill="#fff" stroke="#ddd" stroke-width="0.3" transform="rotate(${a})"/>`;
    const pink = `<g transform="translate(-104.847 -140.369)"><path d="m 129.37005,150.76102 c -3.30897,1.0419 -4.8247,1.19253 -10.7755,2.03529 -6.59559,0.93408 -11.48635,-0.52207 -13.67385,1.58645 -2.25718,2.17569 -2.33269,4.69597 -0.55188,7.8135 1.55216,2.71729 5.59826,4.20805 9.16311,4.20805 3.56485,0 5.24968,0.29168 10.15888,-0.45333 5.6482,-0.85715 6.549,-1.64363 10.85891,-3.84306 2.97944,-2.15106 4.25283,-3.64267 4.4601,-7.47271 -0.0481,-2.59291 -2.95976,-6.29496 -9.63977,-3.87419 z" fill="#cc1466" stroke="#000" stroke-width="0.68"/><path d="m 96.5,117.5 -2.5,-4.5 m 6.5,2 -1,-5 m 5,4 0.5,-5 m 19.5,4.5 -1,-5 m 5,4 0,-5 m 4,5.5 1.5,-4.5" fill="none" stroke="#000" stroke-width="0.8" stroke-linecap="round"/><g transform="translate(104.8 90)">${[0, 72, 144, 216, 288].map(petal).join("")}<circle r="3" fill="#ffd700" stroke="#e6be00" stroke-width="0.3"/></g></g>`;
    const happy = `<path transform="translate(-104.847 -140.369)" d="m 131.91219,147.62567 c -5.93121,1.32573 -7.76848,3.50932 -14.98179,3.56185 -6.00073,0.0437 -7.90128,-0.98135 -13.69461,-3.01114 -7.603974,-2.66418 -10.990677,-3.9e-4 -10.91261,3.98135 0.06726,3.43055 2.381812,6.65022 7.358431,10.24085 4.793869,3.45877 11.264229,5.66453 16.676339,5.67454 6.07564,0.0112 11.48158,-2.27885 16.86065,-5.5402 4.63695,-3.18801 8.53421,-7.34817 8.1306,-11.10608 -0.14813,-3.12436 -3.86018,-6.45383 -9.43701,-3.80117 z" fill="#901600" stroke="#000" stroke-width="0.6"/>`;
    return svgF(
      `<rect width="280" height="392" fill="url(#cg)"/>${stars}${conf}
      <ellipse cx="140" cy="250" rx="130" ry="40" fill="#000" opacity=".25"/>
      ${face(70, 182, 36, { color: "#ff69b4", mouth: "none", extra: pink, rot: -8 })}
      ${face(210, 182, 36, { extra: KING, rot: 8 })}
      ${face(140, 146, 62, { color: "#3579f6", mouth: "none", extra: happy })}`,
      `<radialGradient id="cg" cx="50%" cy="40%" r="75%"><stop offset="0" stop-color="#6a2fb5"/><stop offset="1" stop-color="#160a2b"/></radialGradient>`,
    );
  },
  pip() {
    return svgH(
      `<rect width="260" height="150" fill="url(#sky2)"/>
      <circle cx="214" cy="34" r="16" fill="#ffd166"/>
      <path d="M -10 96 Q 50 56 110 92 T 270 86 L 270 150 L -10 150 Z" fill="#4f8f4a"/>
      <path d="M -10 112 Q 70 84 140 112 T 270 108 L 270 150 L -10 150 Z" fill="#3b7a3a"/>
      <path d="M 118 150 L 126 110 L 134 110 L 150 150 Z" fill="#6d6d6d"/><path d="M 132 150 L 130 118" stroke="#fff" stroke-width="1.5" stroke-dasharray="4 4"/>
      <path d="M 40 60 q 6 -8 12 0 M 46 60 q 6 -8 12 0 M 150 40 q 6 -8 12 0 M 156 40 q 6 -8 12 0" fill="none" stroke="#fff" stroke-width="1.5" stroke-linecap="round" opacity=".8"/>
      <g clip-path="url(#pipclip)" transform="translate(260 150) scale(.85) translate(-260 -150)">
        <rect x="156" y="90" width="104" height="60" fill="url(#pipsky)"/>
        <path d="M 168 102 a 5 5 0 0 1 9 -3 a 6 6 0 0 1 11 2 a 4 4 0 0 1 1 8 h -20 a 4 4 0 0 1 -1 -7 z" fill="#fff" opacity=".9"/>
        <path d="M 150 150 L 150 128 Q 168 118 186 126 L 186 150 Z" fill="#6aa863"/>
        <rect x="241" y="122" width="3" height="16" fill="#7a5230"/><circle cx="242.5" cy="118" r="10" fill="#2f6b34"/>
        <rect x="229" y="128" width="2.5" height="12" fill="#7a5230"/><circle cx="230" cy="125" r="7" fill="#3f8a3e"/>
        <rect x="150" y="138" width="120" height="12" fill="#5c9a4f"/>
        <path d="M 156 136 h 104 M 156 141 h 104 M 162 134 v 10 M 178 134 v 10 M 194 134 v 10 M 210 134 v 10 M 226 134 v 10 M 242 134 v 10 M 258 134 v 10" stroke="#a0754a" stroke-width="1.5"/>
        <g transform="translate(206 0) scale(-1 1) translate(-206 0)">${face(206, 126, 24, { eyes: "open", look: [0, -3], mouth: "o" })}</g>
      </g>`,
      `<linearGradient id="sky2" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#6fb3ff"/><stop offset="1" stop-color="#d7ecff"/></linearGradient>
      <linearGradient id="pipsky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#9fd0ff"/><stop offset="1" stop-color="#eaf6ff"/></linearGradient>
      <clipPath id="pipclip"><rect x="156" y="90" width="104" height="60"/></clipPath>`,
    );
  },
  chat() {
    const msgs = [
      "first",
      "is this bonded?",
      "LETS GOOO",
      "W",
      "WWWWWW",
      "PogChamp",
      "W W W",
      "GOBLIN MODE",
    ];
    const cols = [
      "#e0669c",
      "#2f8cff",
      "#f5a623",
      "#2ecc71",
      "#9b59b6",
      "#1abc9c",
      "#ff7ad9",
      "#e74c3c",
    ];
    let b = "";
    msgs.forEach((m, i) => {
      const y = 18 + i * 34,
        w = [...m].reduce((n, ch) => n + (ch === "W" ? 11 : 7.2), 22);
      b += `${i % 2 ? face(24, y + 10, 10, { extra: KING }) : face(24, y + 10, 10, { eyes: "open", mouth: i % 3 ? "logo" : "open" })}<rect x="42" y="${y}" width="${w}" height="21" rx="10" fill="#fff"/><text x="52" y="${y + 14.5}" font-family="${FONT}" font-weight="800" font-size="11" fill="${cols[i]}">${m}</text>`;
    });
    return svgF(`<rect width="280" height="392" fill="#cdd9f2"/>${b}
      ${txt(176, 352, "W", { fs: 54, fill: "#f5c518", rot: -18 })}
      ${face(140, 372, 84, { mouth: "big", eyes: "open", look: [2, -3] })}`);
  },
  ingests() {
    const cable = (y: number, c: string, t: string, ex: number) =>
      `<path d="M 34 ${y} C 80 ${y} 95 ${118 + ex} 150 ${118 + ex}" fill="none" stroke="${c}" stroke-width="7" stroke-linecap="round"/><path d="M 34 ${y} C 80 ${y} 95 ${118 + ex} 150 ${118 + ex}" fill="none" stroke="#fff" stroke-width="1.5" stroke-dasharray="6 10" class="tcard-dash" opacity=".7"/>${pill(34, y, t, c, "#fff", 8, 42)}`;
    return svgH(`<rect width="260" height="150" fill="#f1efe4"/>
      ${face(168, 78, 64, { mouth: "big", eyes: "open", look: [-3, 2] })}
      ${cable(28, "#d2333c", "RTMP", -6)}${cable(70, "#e8891d", "SRT", 2)}${cable(118, "#2f7cf6", "RIST", 10)}
      <g transform="translate(150 134)"><rect x="-16" y="-10" width="32" height="20" rx="3" fill="#333" stroke="#000"/><circle cx="0" cy="0" r="6" fill="#111" stroke="#888"/><circle cx="0" cy="0" r="2.5" fill="#2f7cf6"/><rect x="-10" y="-14" width="10" height="5" fill="#333"/></g>
      ${txt(248, 17, "ALWAYS", { fs: 9, fill: "#6a6a60", anchor: "end" })}${txt(248, 28, "HUNGRY", { fs: 9, fill: "#6a6a60", anchor: "end" })}`);
  },
  pixellate() {
    const cell = 20,
      cx = 140,
      cy = 186,
      R = 118,
      s = R / 50;
    let out = "";
    const inside = (x: number, y: number, ex: number, ey: number, rx: number, ry: number) =>
      (x - ex) ** 2 / (rx * rx) + (y - ey) ** 2 / (ry * ry) <= 1;
    for (let y = 0; y < 392; y += cell)
      for (let x = 0; x < 280; x += cell) {
        const px = x + cell / 2,
          py = y + cell / 2;
        let c = "#2a2a2a";
        const d = (px - cx) ** 2 + (py - cy) ** 2;
        if (d <= R * R) {
          c = "#008000";
          const lx = cx - 2.43 * s,
            ly = cy - 14.89 * s,
            rx = cx + 25.48 * s,
            ry = cy - 17.43 * s;
          if (
            inside(px, py, lx, ly, 11.3 * s, 12.69 * s) ||
            inside(px, py, rx, ry, 11.3 * s, 12.69 * s)
          )
            c = "#fff";
          if (
            inside(px, py, lx + 4.4 * s, ly + 1.5 * s, 6.7 * s, 6.7 * s) ||
            inside(px, py, rx + 4.1 * s, ry + 2.7 * s, 6.7 * s, 6.7 * s)
          )
            c = "#000";
          if (inside(px, py, cx + 15 * s, cy + 17 * s, 19 * s, 8 * s)) c = "#901600";
        } else if ((x / cell + y / cell) % 2 === 0) c = "#303030";
        if (
          px > cx - 34 * s &&
          px < cx - 8 * s &&
          py > cy - 72 * s &&
          py < cy - 46 * s &&
          px - cx + 34 * s > (py - cy + 72 * s) * 0.7
        )
          c = "#008000";
        out += `<rect x="${x}" y="${y}" width="${cell}" height="${cell}" fill="${c}"/>`;
      }
    return svgF(`<rect width="280" height="392" fill="#2a2a2a"/>${out}
      <rect x="86" y="344" width="108" height="30" rx="3" fill="#f3f3f0" stroke="#999"/><rect x="92" y="350" width="96" height="18" fill="#ddd"/>
      ${["#777", "#444", "#999", "#555", "#333", "#888", "#666"].map((c, i) => `<rect x="${94 + i * 13}" y="352" width="12" height="14" fill="${c}"/>`).join("")}`);
  },
  remote() {
    return svgH(`<rect width="260" height="150" fill="#0f1b2d"/>
      <circle cx="130" cy="82" r="56" fill="#1c3a63" stroke="#8fb6ea" stroke-width="1"/>
      <ellipse cx="130" cy="82" rx="22" ry="56" fill="none" stroke="#8fb6ea" opacity=".6"/><ellipse cx="130" cy="82" rx="42" ry="56" fill="none" stroke="#8fb6ea" opacity=".4"/>
      <path d="M 74 82 h 112 M 80 58 h 100 M 80 106 h 100" stroke="#8fb6ea" opacity=".5"/>
      <path d="M 100 70 q 10 -14 22 -6 q 12 -6 18 8 q -8 16 -24 10 z M 120 110 q 14 -8 26 4 q -6 12 -24 6 z" fill="#2e8b57" opacity=".9"/>
      <path d="M 56 110 Q 130 10 206 110" fill="none" stroke="#ffd166" stroke-width="2.5" stroke-dasharray="6 8" class="tcard-dash"/>
      ${face(42, 112, 26, { eyes: "open", look: [2, -2], mouth: "o" })}
      ${face(218, 112, 26, { eyes: "happy", extra: '<path d="M -46 -14 a 46 46 0 0 1 92 0" fill="none" stroke="#222" stroke-width="6"/><rect x="-52" y="-16" width="12" height="22" rx="4" fill="#222"/><rect x="40" y="-16" width="12" height="22" rx="4" fill="#222"/><path d="M 46 6 q 0 22 -22 24" fill="none" stroke="#222" stroke-width="3"/><circle cx="24" cy="30" r="4" fill="#222"/>' })}
      <rect x="166" y="124" width="40" height="18" rx="9" fill="#ff3b30" stroke="#7a0f0f"/>${txt(186, 136, "ZOOM", { fs: 8 })}
      ${txt(42, 146, "you", { fs: 8, fill: "#8fb6ea" })}${txt(218, 146, "a friend", { fs: 8, fill: "#8fb6ea" })}`);
  },
  website() {
    const bolt = (x: number, y: number, r: number) =>
      `<path d="M 0 0 l -8 18 h 7 l -5 16 l 14 -22 h -7 l 5 -12 z" transform="translate(${x} ${y}) rotate(${r})" fill="#ffd60a" stroke="#b58900"/>`;
    return svgH(
      `<rect width="260" height="150" fill="#1a1030"/>
      <rect x="62" y="12" width="136" height="44" rx="10" fill="#2a0b14" stroke="#ff2d55" stroke-width="2" filter="url(#glow)"/>
      ${txt(131, 43, "LIVE", { fs: 28, ls: 2, fill: "#ff3b5c", font: DISPLAY, w: 400 })}
      <circle cx="78" cy="34" r="4" fill="#ff2d55" class="tcard-pulse"/><circle cx="182" cy="34" r="4" fill="#ff2d55" class="tcard-pulse"/>
      ${bolt(34, 30, -20)}${bolt(226, 30, 20)}
      ${face(130, 108, 42, { eyes: "happy", mouth: "open" })}`,
      GLOW,
    );
  },
  gamepad() {
    const k = 1.2;
    const at = (x: number, y: number): [number, number] => [
      130 + (x - 130) * k,
      104 + (y - 125) * k,
    ];
    const call = (lx: number, ly: number, t: string, bg: string, fg: string, b: [number, number]) =>
      `<path d="M ${lx + (lx < 130 ? 25 : -25)} ${ly} L ${b[0]} ${b[1]}" stroke="#888" stroke-width="1"/><circle cx="${b[0]}" cy="${b[1]}" r="1.8" fill="#888"/>${pill(lx, ly, t, bg, fg, 8, 50)}`;
    return svgH(`<rect width="260" height="150" fill="#eef0e6"/>
      ${face(130, 46, 30, { eyes: "open", look: [0, 4] })}
      <g transform="translate(130 104) scale(${k}) translate(-130 -125)">
      <rect x="94" y="94" width="20" height="8" rx="3" fill="#444" stroke="#000"/><rect x="146" y="94" width="20" height="8" rx="3" fill="#444" stroke="#000"/>
      <path d="M 98 100 h 64 q 20 0 24 20 l 7 28 q 2 12 -10 12 q -8 0 -14 -12 l -5 -8 h -68 l -5 8 q -6 12 -14 12 q -12 0 -10 -12 l 7 -28 q 4 -20 24 -20 z" fill="#2c2c2e" stroke="#000"/>
      <circle cx="102" cy="116" r="8" fill="#111" stroke="#555"/><circle cx="102" cy="116" r="4.5" fill="#333"/>
      <circle cx="146" cy="132" r="8" fill="#111" stroke="#555"/><circle cx="146" cy="132" r="4.5" fill="#333"/>
      <circle cx="158" cy="109" r="3.5" fill="#4cd964"/><circle cx="166" cy="116" r="3.5" fill="#ff3b30"/><circle cx="150" cy="116" r="3.5" fill="#2f7cf6"/><circle cx="158" cy="123" r="3.5" fill="#ffcc00"/>
      <rect x="108" y="130" width="14" height="4" rx="1" fill="#555"/><rect x="113" y="125" width="4" height="14" rx="1" fill="#555"/>
      </g>
      ${call(38, 30, "Stream", "#2c2c2e", "#fff", at(104, 96))}
      ${call(38, 56, "Record", "#2c2c2e", "#fff", at(98, 104))}
      ${call(38, 104, "Zoom", "#2c2c2e", "#fff", at(115, 130))}
      ${call(222, 30, "Replay", "#2c2c2e", "#fff", at(156, 96))}
      ${call(222, 56, "Mute", "#ff3b30", "#fff", at(166, 116))}
      ${call(222, 82, "Blackout", "#2f7cf6", "#fff", at(150, 116))}
      ${call(222, 108, "Torch", "#ffcc00", "#1c1c1e", at(158, 123))}`);
  },
  background() {
    return svgF(
      `<rect width="280" height="392" fill="url(#bgsky)"/>
      <path d="M -10 250 Q 80 200 160 236 T 290 226 L 290 392 L -10 392 Z" fill="#4f8f4a"/>
      <path d="M -10 290 Q 90 254 180 284 T 290 280 L 290 392 L -10 392 Z" fill="#3b7a3a"/>
      <g transform="translate(-6 0)">
      <path d="M 2 170 h 20 M 8 186 h 22 M 0 202 h 16" stroke="#fff" stroke-width="3" stroke-linecap="round" opacity=".8"/>
      <circle cx="54" cy="214" r="26" fill="none" stroke="#1c1c1e" stroke-width="5"/><path d="M 30 214 h 48 M 54 190 v 48 M 37 197 l 34 34 M 71 197 l -34 34" stroke="#999" stroke-width="1"/><circle cx="54" cy="214" r="3" fill="#555"/><circle cx="150" cy="214" r="26" fill="none" stroke="#1c1c1e" stroke-width="5"/><path d="M 126 214 h 48 M 150 190 v 48 M 133 197 l 34 34 M 167 197 l -34 34" stroke="#999" stroke-width="1"/><circle cx="150" cy="214" r="3" fill="#555"/>
      <path d="M 54 214 L 98 214 L 84 170 Z M 98 214 L 138 176 L 84 172 M 138 172 L 150 214" fill="none" stroke="#e8432f" stroke-width="4.5" stroke-linejoin="round" stroke-linecap="round"/>
      <path d="M 138 176 L 134 158 l 10 -4" fill="none" stroke="#1c1c1e" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
      <circle cx="98" cy="214" r="6" fill="none" stroke="#555" stroke-width="2"/>
      <path d="M 74 168 h 22" stroke="#1c1c1e" stroke-width="6" stroke-linecap="round"/>
      <g transform="translate(90 122) scale(0.77) translate(-84 -160)">
      ${face(84, 160, 52, { eyes: "open", look: [4, 0], mouth: "flat", extra: `<path d="M -14 -30 L 8 -24 M 15 -27 L 37 -35" stroke="#1e1a1a" stroke-width="4.5" stroke-linecap="round"/>` })}
      <path d="M 44 124 Q 84 104 124 124" fill="none" stroke="#1c1c1e" stroke-width="8" stroke-linecap="round"/>
      <rect x="66" y="88" width="44" height="34" rx="6" fill="#2c2c2e" stroke="#000"/><rect x="82" y="120" width="12" height="8" fill="#2c2c2e" stroke="#000"/>
      <circle cx="96" cy="105" r="11" fill="#111" stroke="#666" stroke-width="2"/><circle cx="96" cy="105" r="5" fill="#1a3a6b"/><circle cx="98" cy="103" r="1.8" fill="#fff"/>
      <circle cx="74" cy="96" r="2.5" fill="#ff2d3a" class="tcard-pulse"/>
      </g>
      <path d="M 116 134 Q 132 140 138 156" fill="none" stroke="#008000" stroke-width="4" stroke-linecap="round"/>
      </g>
      <path d="M 106 74 C 150 52 196 70 210 110" fill="none" stroke="#2f7cf6" stroke-width="3" stroke-dasharray="6 6" class="tcard-dash"/>
      ${pill(160, 58, "RTMP", "#2f7cf6", "#fff", 9, 46)}
      <rect x="176" y="112" width="86" height="150" rx="14" fill="#1c1c1e" stroke="#000"/>
      <rect x="181" y="118" width="76" height="138" rx="10" fill="url(#lock)"/>
      <path d="M 214 132 v -3 a 5 5 0 0 1 10 0 v 3" fill="none" stroke="#fff" stroke-width="1.6"/><rect x="211" y="132" width="16" height="11" rx="2" fill="#fff"/>
      ${txt(219, 170, "9:41", { fs: 24, fill: "#fff", w: 600 })}
      <rect x="187" y="186" width="64" height="26" rx="8" fill="#fff" opacity=".9"/>
      ${face(199, 199, 7)}${txt(210, 197, "Moblin", { fs: 6.5, fill: "#1c1c1e", anchor: "start" })}
      <circle cx="213" cy="205" r="2.2" fill="#ff2d3a" class="tcard-pulse"/>${txt(218, 207.5, "LIVE", { fs: 6.5, fill: "#ff2d3a", anchor: "start" })}`,
      `<linearGradient id="bgsky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#6fb3ff"/><stop offset=".6" stop-color="#d7ecff"/></linearGradient>
      <linearGradient id="lock" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3b2a8f"/><stop offset="1" stop-color="#c86dd7"/></linearGradient>`,
    );
  },
  mobcam() {
    return svgH(`<rect width="260" height="150" fill="#f1efe4"/>
      <path d="M 54 137 C 54 150 96 150 96 126 S 98 113 104 113" fill="none" stroke="#000" stroke-width="5.5" stroke-linecap="round"/><path d="M 54 137 C 54 150 96 150 96 126 S 98 113 104 113" fill="none" stroke="#1a8a1a" stroke-width="3.5" stroke-linecap="round"/><rect x="100" y="109.5" width="10" height="7" rx="1.5" fill="#1a8a1a" stroke="#000"/>
      ${phone(18, 14, 72, 124, `<rect x="22" y="20" width="64" height="112" rx="5" fill="#dfe8d8"/>${face(54, 80, 24, { extra: KING })}`)}
      <rect x="116" y="22" width="132" height="86" rx="5" fill="#2c2c2e" stroke="#000"/>
      <path d="M 108 108 h 144 l -6 10 h -132 z" fill="#bdbdc2" stroke="#666"/>
      <rect x="121" y="27" width="122" height="76" fill="#1f1f23"/>
      <rect x="125" y="31" width="114" height="50" fill="#dfe8d8"/>
      ${face(182, 60, 18, { extra: KING })}
      <rect x="125" y="85" width="55" height="15" rx="2" fill="#2b2b30"/><rect x="184" y="85" width="55" height="15" rx="2" fill="#2b2b30"/>
      ${txt(128, 91, "Scenes", { fs: 5, fill: "#bbb", anchor: "start" })}${txt(187, 91, "Sources", { fs: 5, fill: "#bbb", anchor: "start" })}
      <rect x="186" y="93" width="51" height="6" rx="1" fill="#3a5a9c"/>${txt(189, 98, "Mobcam", { fs: 4.5, fill: "#fff", anchor: "start", w: 600 })}
      <rect x="127" y="93" width="51" height="6" rx="1" fill="#3a3a40"/>${txt(130, 98, "IRL", { fs: 4.5, fill: "#ddd", anchor: "start", w: 600 })}
      ${pill(182, 12, "OBS", "#2c2c2e", "#fff", 8, 32)}`);
  },
  snapshot() {
    const ray = (a: number) =>
      `<path d="M 0 0 l 0 -22" stroke="#ffd60a" stroke-width="3" stroke-linecap="round" transform="rotate(${a})"/>`;
    return svgH(
      `<rect width="260" height="150" fill="#e9e5f6"/>
      <g transform="rotate(-7 100 76)"><rect x="40" y="18" width="118" height="128" rx="3" fill="#fff" stroke="#bbb"/><rect x="48" y="26" width="102" height="92" fill="#dfe8d8"/>${face(99, 74, 38, { eyes: "open", look: [0, -3], mouth: "o" })}${txt(99, 136, "perfect?", { fs: 9, fill: "#555", w: 400 })}</g>
      <g transform="translate(148 22)">${[0, 45, 90, 135, 180, 225, 270, 315].map(ray).join("")}<circle r="6" fill="#fff"/></g>
      <path d="M 164 83 L 186 83" fill="none" stroke="#5865f2" stroke-width="3" stroke-dasharray="6 6" class="tcard-dash" marker-end="url(#arr)"/>
      <rect x="200" y="60" width="46" height="46" rx="12" fill="#5865f2"/>
      <g transform="translate(223 83) scale(1.45) translate(-12 -12.4)"><path d="M4 6.5c2-1 4-1.5 4-1.5l.6 1.3a12 12 0 0 1 6.8 0L16 5s2 .5 4 1.5c1.5 4 2 8 1.5 11-1.6 1.3-4 2.2-4 2.2l-.9-1.6c-2.8 1.2-6.4 1.2-9.2 0L6.5 19.7s-2.4-.9-4-2.2C2 14.5 2.5 10.5 4 6.5Z" fill="#fff"/><ellipse cx="9" cy="12.5" rx="1.7" ry="1.9" fill="#5865f2"/><ellipse cx="15" cy="12.5" rx="1.7" ry="1.9" fill="#5865f2"/></g>
      ${txt(223, 120, "Discord", { fs: 9, fill: "#4b2f8a" })}`,
      `<marker id="arr" markerWidth="10" markerHeight="10" refX="2" refY="5" orient="auto" markerUnits="userSpaceOnUse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#5865f2"/></marker>`,
    );
  },
  vtuber() {
    const star = (x: number, y: number, s: number) =>
      `<path d="M 0 -${s} L ${s * 0.3} -${s * 0.3} L ${s} 0 L ${s * 0.3} ${s * 0.3} L 0 ${s} L -${s * 0.3} ${s * 0.3} L -${s} 0 L -${s * 0.3} -${s * 0.3} Z" transform="translate(${x} ${y})" fill="#fff"/>`;
    return svgH(
      `<rect width="260" height="150" fill="#1c0f33"/>
      ${face(50, 84, 30, { eyes: "open", look: [0, 4], mouth: "open" })}
      <path d="M 22 46 q 28 -14 56 0 M 26 42 q 24 -12 48 0" fill="none" stroke="#c9b8ff" stroke-width="2" stroke-linecap="round" opacity=".8"/>
      ${txt(50, 134, "you nod", { fs: 8, fill: "#c9b8ff" })}
      <path d="M 90 84 h 22" stroke="#c9b8ff" stroke-width="2" marker-end="url(#arr2)"/>
      <image href="${asset("cards/alicia.webp")}" x="120" y="9" width="132" height="150"/>
      <g transform="translate(120 9) scale(${132 / 422})"><path d="M 200 315 Q 211.5 311 223 315 Q 220 323 211.5 324 Q 203 323 200 315 Z" fill="#7a2a2e" stroke="#5a2024" stroke-width="1.2"/><ellipse cx="211.5" cy="321" rx="5" ry="2" fill="#e57a80"/></g>
      ${star(131, 31, 5.5)}${star(241, 15, 3)}${star(248, 71, 4.5)}${star(116, 98, 3.5)}${star(124, 128, 2.5)}${star(237, 112, 3)}`,
      `<marker id="arr2" markerWidth="8" markerHeight="8" refX="4" refY="4" orient="auto"><path d="M 0 0 L 8 4 L 0 8 z" fill="#c9b8ff"/></marker>`,
    );
  },
  chatbot() {
    const ant = `<path d="M 10 -50 l 4 -22" stroke="#333" stroke-width="3"/><circle cx="14" cy="-74" r="6" fill="#ff3b30" class="tcard-pulse"/><rect x="-40" y="-8" width="4" height="22" rx="1" fill="#333"/><rect x="36" y="-10" width="4" height="22" rx="1" fill="#333"/>`;
    const burst = (x: number, y: number, r: number, color: string) => {
      let rays = "";
      for (let k = 0; k < 12; k++) {
        const angle = (k * Math.PI) / 6,
          dx = Math.cos(angle),
          dy = Math.sin(angle);
        rays += `<path d="M ${x + dx * r * 0.35} ${y + dy * r * 0.35} L ${x + dx * r} ${y + dy * r}" stroke="${color}" stroke-width="1.6" stroke-linecap="round"/><circle cx="${x + dx * r * 1.15}" cy="${y + dy * r * 1.15}" r="1.2" fill="${color}"/>`;
      }
      return rays;
    };
    return svgH(
      `<rect width="260" height="150" fill="#eaf4ea"/>
      ${face(56, 90, 34, { eyes: "robot", mouth: "flat", extra: ant })}
      <rect x="106" y="16" width="142" height="24" rx="12" fill="#fff" stroke="#999"/>${txt(177, 31.5, "!moblin reaction fireworks", { fs: 8, fill: "#1d1d1a", font: MONO })}
      <rect x="110" y="50" width="136" height="88" rx="8" fill="#1c1c1e"/>
      <rect x="114" y="54" width="128" height="80" rx="5" fill="url(#night)"/>
      <g clip-path="url(#fwclip)">
      ${burst(150, 78, 15, "#ffcc00")}${burst(208, 70, 12, "#ff5c8a")}${burst(228, 98, 8, "#5ac8fa")}${burst(132, 106, 7, "#4cd964")}
      ${face(180, 122, 20, { eyes: "open", look: [0, -4], mouth: "o" })}
      </g>`,
      `<linearGradient id="night" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#0b1030"/><stop offset="1" stop-color="#2a2160"/></linearGradient>
      <clipPath id="fwclip"><rect x="114" y="54" width="128" height="80" rx="5"/></clipPath>`,
    );
  },
  watch() {
    return svgH(`<rect width="260" height="150" fill="#f1efe4"/>
      <rect x="104" y="-10" width="52" height="40" rx="8" fill="#3a3a3c"/><rect x="104" y="120" width="52" height="40" rx="8" fill="#3a3a3c"/>
      <rect x="86" y="24" width="88" height="102" rx="22" fill="#1c1c1e" stroke="#000"/><rect x="93" y="31" width="74" height="88" rx="18" fill="#000"/>
      <rect x="174" y="52" width="6" height="18" rx="2" fill="#3a3a3c"/><rect x="174" y="78" width="6" height="12" rx="2" fill="#3a3a3c"/>
      ${txt(130, 46, "12:34", { fs: 9, fill: "#fff", font: MONO })}
      ${face(130, 78, 22, { eyes: "open", look: [-2, -2], mouth: "logo" })}
      <rect x="99" y="100" width="46" height="12" rx="6" fill="#2f7cf6"/>${txt(122, 108.5, "W W W", { fs: 7 })}
      ${pill(140, 113, "4.2 Mb", "#ff9f0a", "#000", 7, 36)}
      <path d="M 60 50 q -10 25 0 50 M 48 44 q -14 31 0 62 M 200 50 q 10 25 0 50 M 212 44 q 14 31 0 62" fill="none" stroke="#ff3b30" stroke-width="2.5" stroke-linecap="round" class="tcard-pulse"/>
      ${txt(40, 140, "bzzzt", { fs: 9, fill: "#ff3b30" })}${txt(220, 140, "bzzzt", { fs: 9, fill: "#ff3b30" })}`);
  },
  textwidget() {
    const tag = (x: number, y: number, t: string, bg: string, fg = "#fff", w?: number) =>
      pill(x, y, t, bg, fg, 10, w);
    return svgH(`<rect width="260" height="150" fill="#e7ecef"/>
      ${face(76, 84, 46, { eyes: "open", look: [3, -2], mouth: "open" })}
      ${tag(178, 30, "12:34:56", "#1c1c1e", "#fff", 72)}
      ${tag(200, 58, "42 km/h", "#2f7cf6", "#fff", 66)}
      ${tag(168, 86, "♥ 128", "#ff3b30", "#fff", 52)}
      ${tag(214, 114, "3.9 km", "#0d7a30", "#fff", 56)}
      <g transform="translate(150 128)"><rect x="-26" y="-9" width="52" height="18" rx="9" fill="#999"/>${txt(0, 4, "oops", { fs: 10, fill: "#fff" })}<path d="M -22 0 h 44" stroke="#ff3b30" stroke-width="3"/><circle r="18" fill="none" stroke="#ff3b30" stroke-width="3"/><path d="M -13 -13 L 13 13" stroke="#ff3b30" stroke-width="3"/></g>`);
  },
  macros() {
    let small = "";
    for (let i = 0; i < 10; i++) {
      const a = -70 + i * 15.5,
        r = 84,
        x = 92 + Math.cos((a * Math.PI) / 180) * r,
        y = 84 + Math.sin((a * Math.PI) / 180) * r,
        lit = i < 3;
      small += `<path d="M 108 84 L ${x} ${y}" stroke="${lit ? "#ffcc00" : "#bbb"}" stroke-width="${lit ? 2 : 1}" stroke-dasharray="${lit ? "0" : "3 3"}"/><circle cx="${x}" cy="${y}" r="9" fill="${lit ? "#ffcc00" : "#ddd"}" stroke="#333"/>${txt(x, y + 3.5, i + 1, { fs: 8, fill: lit ? "#000" : "#777" })}`;
    }
    return svgH(`<rect width="260" height="150" fill="#f6f1e4"/>${small}
      <ellipse cx="72" cy="94" rx="40" ry="14" fill="#8a2020"/><rect x="32" y="70" width="80" height="24" fill="#8a2020"/>
      <ellipse cx="72" cy="70" rx="40" ry="14" fill="#ff3b30" stroke="#7a0f0f"/>
      ${face(72, 42, 26, { eyes: "closed", mouth: "open" })}
      <path d="M 60 72 h 24" stroke="#fff" stroke-width="3" opacity=".6"/>
      ${txt(72, 124, "ONE TAP", { fs: 11, fill: "#333" })}${txt(72, 136, "ten things. allegedly.", { fs: 8, w: 400, fill: "#777" })}`);
  },
  moblink() {
    const heat = (x: number, y: number) =>
      `<path d="M ${x} ${y} q 3 -5 0 -10 q -3 -5 0 -10 M ${x + 8} ${y} q 3 -5 0 -10 q -3 -5 0 -10" fill="none" stroke="#ff6b35" stroke-width="1.6" stroke-linecap="round" opacity=".9"/>`;
    const link = (x1: number, y1: number, x2: number, y2: number) =>
      `<path d="M ${x1} ${y1} Q ${(x1 + x2) / 2} ${Math.min(y1, y2) - 26} ${x2} ${y2}" fill="none" stroke="#2f7cf6" stroke-width="2.5" stroke-dasharray="5 6" class="tcard-dash"/>`;
    return svgH(`<rect width="260" height="150" fill="#eef3fb"/>
      ${link(60, 70, 118, 46)}${link(142, 46, 200, 70)}${link(42, 118, 112, 110)}
      ${phone(106, 22, 48, 110, face(130, 78, 16, { eyes: "happy", mouth: "open" }), {})}${pill(130, 124, "you", "#2f7cf6", "#fff", 7, 30)}
      ${phone(26, 54, 36, 70, face(44, 88, 11, { eyes: "open", look: [2, -2] }), {})}${heat(36, 52)}
      ${phone(198, 54, 36, 70, face(216, 88, 11, { eyes: "open", look: [-2, -2] }), {})}${heat(208, 52)}
      ${phone(14, 104, 30, 44, face(29, 126, 9, { eyes: "x", mouth: "flat" }), {})}${heat(22, 102)}
      ${txt(228, 140, "+ 3 links", { fs: 9, fill: "#2f7cf6", anchor: "end" })}
      ${txt(228, 32, "slightly warm", { fs: 7, w: 400, fill: "#ff6b35", anchor: "end" })}`);
  },
  photoshoot() {
    return svgH(
      `<rect width="260" height="150" fill="#12101a"/>
      <path d="M 20 -10 L 90 150 L 170 150 Z" fill="url(#spot)" opacity=".8"/><path d="M 240 -10 L 170 150 L 90 150 Z" fill="url(#spot)" opacity=".8"/>
      <rect x="92" y="34" width="86" height="86" fill="none" stroke="#fff" opacity=".2"/><rect x="100" y="28" width="86" height="86" fill="none" stroke="#fff" opacity=".35"/><rect x="108" y="22" width="86" height="86" rx="2" fill="none" stroke="#fff" opacity=".6"/>
      ${face(140, 88, 48, { eyes: "wink", mouth: "open", rot: -10, extra: '<path d="M -16 44 l 16 -8 l 16 8 l -16 8 z" fill="#1c1c1e" stroke="#fff" stroke-width=".8"/>' })}
      <g transform="translate(228 22)">${[0, 45, 90, 135, 180, 225, 270, 315].map((a) => `<path d="M 0 0 l 0 -14" stroke="#fff" stroke-width="2.5" stroke-linecap="round" transform="rotate(${a})"/>`).join("")}<circle r="4" fill="#fff"/></g>
      ${txt(140, 144, "burst · 3 of 30", { fs: 8, fill: "#bbb", font: MONO })}`,
      `<linearGradient id="spot" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff5c2" stop-opacity=".55"/><stop offset="1" stop-color="#fff5c2" stop-opacity="0"/></linearGradient>`,
    );
  },
  abr() {
    const pts = [
      [20, 60],
      [50, 44],
      [80, 48],
      [104, 70],
      [124, 112],
      [144, 118],
      [166, 96],
      [190, 66],
      [214, 50],
      [244, 46],
    ];
    const d = pts.map((p, i) => (i ? "L" : "M") + p[0] + " " + p[1]).join(" ");
    const bars = (x: number, y: number, n: number) =>
      [0, 1, 2, 3]
        .map(
          (i) =>
            `<rect x="${x + i * 6}" y="${y - 4 - i * 4}" width="4" height="${6 + i * 4}" rx="1" fill="${i < n ? "#0d7a30" : "#ccc"}"/>`,
        )
        .join("");
    return svgH(`<rect width="260" height="150" fill="#f6f6f1"/>
      <path d="M 20 140 H 250 M 20 140 V 20" stroke="#bbb" stroke-width="1"/>
      <path d="${d} L 244 140 L 20 140 Z" fill="#0d7a30" opacity=".15"/>
      <path d="${d}" fill="none" stroke="#0d7a30" stroke-width="3" stroke-linejoin="round" stroke-linecap="round"/>
      ${pts.map((p) => `<circle cx="${p[0]}" cy="${p[1]}" r="2.5" fill="#0d7a30"/>`).join("")}
      <ellipse cx="146" cy="116" rx="22" ry="5" fill="#ff9f0a" stroke="#7a4a00"/>
      ${face(146, 96, 18, { eyes: "open", look: [3, 2], mouth: "o", rot: 12 })}
      ${bars(112, 120, 1)}${bars(214, 36, 4)}
      ${txt(66, 40, "6000", { fs: 9, fill: "#0d7a30", font: MONO })}${txt(146, 136, "800", { fs: 9, fill: "#b00", font: MONO })}${txt(240, 36, "6000", { fs: 9, fill: "#0d7a30", font: MONO, anchor: "end" })}`);
  },
  bonding() {
    const rope = (x0: number, c: string, phase: number) => {
      let d = `M ${x0} -10`;
      for (let y = 0; y <= 200; y += 20) {
        d += ` Q ${140 + (x0 - 140) * (1 - y / 200) + Math.sin(y / 20 + phase) * 10} ${y + 10} ${140 + (x0 - 140) * (1 - (y + 20) / 200)} ${y + 20}`;
      }
      return `<path d="${d}" fill="none" stroke="${c}" stroke-width="9" stroke-linecap="round"/><path d="${d}" fill="none" stroke="#fff" stroke-width="1.5" opacity=".5" stroke-dasharray="4 8"/>`;
    };
    let braid = "";
    for (let y = 200; y < 400; y += 22) {
      braid += `<path d="M 128 ${y} q 12 -11 24 0 q -12 11 -24 0 z" fill="#ff8c2a" stroke="#8a2a02"/>`;
    }
    return svgF(
      `<rect width="280" height="392" fill="url(#sky)"/>
      ${[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].map((i) => `<circle cx="${(i * 67 + 13) % 280}" cy="${(i * 41 + 9) % 180}" r="${1 + (i % 3) * 0.5}" fill="#fff" opacity="${0.35 + (i % 4) * 0.15}"/>`).join("")}
      <path d="M 0 392 L 0 330 Q 60 300 100 340 L 100 392 Z M 280 392 L 280 320 Q 230 300 180 350 L 180 392 Z" fill="#0a1220"/>
      ${rope(60, "#ff9f0a", 0)}${rope(140, "#2f7cf6", 2)}${rope(220, "#0d7a30", 4)}
      <rect x="128" y="196" width="24" height="200" fill="#c44d00"/>${braid}
      ${pill(60, 26, "Cellular", "#ff9f0a", "#000", 9, 60)}${pill(140, 26, "Wi-Fi", "#2f7cf6", "#fff", 9, 50)}${pill(220, 26, "Ethernet", "#0d7a30", "#fff", 9, 64)}
      ${face(140, 318, 50, { eyes: "happy", mouth: "open", extra: '<path d="M -40 -4 q 40 -30 80 0" fill="none" stroke="#1e1a1a" stroke-width="6" stroke-linecap="round"/>' })}
      ${txt(140, 380, "one rope. zero fear.", { fs: 9, fill: "#ffe7a3" })}`,
      `<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#0b1a33"/><stop offset="1" stop-color="#1f4a7a"/></linearGradient>`,
    );
  },
  selfie() {
    return svgH(`<rect width="260" height="150" fill="#dfeee6"/>
      <path d="M 36 150 L 172 48" stroke="#333" stroke-width="7" stroke-linecap="round"/><path d="M 36 150 L 80 118" stroke="#1c1c1e" stroke-width="12" stroke-linecap="round"/>
      <g transform="rotate(53 180 42)"><rect x="138" y="22" width="84" height="40" rx="8" fill="#1c1c1e" stroke="#000"/><rect x="144" y="26" width="72" height="32" rx="5" fill="#0a0f0a"/><rect x="140.5" y="35" width="2.5" height="14" rx="1.2" fill="#444"/>${face(180, 44, 12, { eyes: "happy", mouth: "open" })}</g>
      <circle cx="62" cy="128" r="12" fill="#ff3b30" stroke="#7a0f0f" stroke-width="2"/><circle cx="62" cy="128" r="6" fill="#fff" opacity=".4"/>
      <path d="M 76 136 l 30 0" stroke="#333" stroke-width="1.5"/>${pill(152, 136, "press: switch scene", "#0d7a30", "#fff", 8, 92)}`);
  },
  wifiaware() {
    const rows = ["Streams", "Scenes", "Chat", "Bonding", "Wi-Fi Aware"];
    let list = "";
    rows.forEach((r, i) => {
      const y = 28 + i * 32,
        last = i === 4;
      list += `<g ${last ? 'class="tcard-glitch"' : ""}><rect x="24" y="${y}" width="232" height="26" rx="6" fill="${last ? "#3a0b12" : "#1d0c10"}" stroke="${last ? "#ff2d3a" : "#3a1a20"}"/>${txt(36, y + 17, r, { fs: 11, fill: last ? "#ff8a94" : "#c9a9ad", anchor: "start" })}${txt(244, y + 17, last ? "??" : "›", { fs: 12, fill: last ? "#ff2d3a" : "#6a4a50", anchor: "end" })}${last ? `<rect x="24" y="${y}" width="232" height="26" rx="6" fill="#0ff" opacity=".12" transform="translate(-3 1)"/>` : ""}</g>`;
    });
    const arcs = [16, 30, 44]
      .map(
        (r) =>
          `<path d="M ${140 - r} ${236} a ${r} ${r} 0 0 1 ${2 * r} 0" fill="none" stroke="#ff2d3a" stroke-width="3" stroke-linecap="round" stroke-dasharray="6 5" class="tcard-dash"/>`,
      )
      .join("");
    return svgF(
      `<rect width="280" height="392" fill="#07030a"/><rect width="280" height="392" fill="url(#vig)"/>${list}
      <ellipse cx="140" cy="300" rx="92" ry="60" fill="none" stroke="#ff2d3a" stroke-width="2" opacity=".5"/><ellipse cx="140" cy="300" rx="70" ry="44" fill="none" stroke="#ff2d3a" stroke-width="1" opacity=".35" stroke-dasharray="4 6"/>
      ${face(140, 300, 46, { fill: "none", stroke: "#ff2d3a", sw: 1.6, dash: "5 4", leafColor: "none", eyes: "none", mouth: "none", extra: '<ellipse cx="-2.4" cy="-14.9" rx="11.3" ry="12.7" fill="none" stroke="#ff2d3a" stroke-width="1.6" stroke-dasharray="4 3"/><ellipse cx="25.5" cy="-17.4" rx="11.3" ry="12.7" fill="none" stroke="#ff2d3a" stroke-width="1.6" stroke-dasharray="4 3"/><path d="M -8 20 q 20 12 40 -4" fill="none" stroke="#ff2d3a" stroke-width="2" stroke-dasharray="4 3"/>' })}
      ${arcs}
      ${txt(60, 290, "?", { fs: 26, fill: "#ff2d3a", op: 0.6, rot: -15 })}${txt(224, 330, "?", { fs: 20, fill: "#ff2d3a", op: 0.5, rot: 15 })}
      ${txt(140, 378, "universe B only", { fs: 8, w: 400, fill: "#8a4a52", font: MONO })}`,
      `<radialGradient id="vig" cx="50%" cy="60%" r="70%"><stop offset="0" stop-color="#3a0b12" stop-opacity=".9"/><stop offset="1" stop-color="#07030a" stop-opacity="0"/></radialGradient>`,
    );
  },
} satisfies Record<string, () => string>;

export type CardId = keyof typeof cardArt;

function backPhoneOnGreen(font: string, weight: number): string {
  const leaf = (x: number, y: number, rot: number) =>
    `<g transform="translate(${x} ${y}) rotate(${rot}) scale(0.55) translate(-87 -80)"><path d="${LEAF}" fill="#f3e9c6" stroke="#07230f" stroke-width="1"/></g>`;
  const lens = (x: number, y: number) =>
    `<circle cx="${x}" cy="${y}" r="8" fill="#1c1c1e" stroke="#4a4a4e" stroke-width="1"/><circle cx="${x}" cy="${y}" r="5" fill="#0d1a2e"/><circle cx="${x - 1.8}" cy="${y - 1.8}" r="1.4" fill="#fff" opacity="0.6"/>`;
  return `<svg viewBox="0 0 280 392" role="img">
    <defs>
      <pattern id="green-lattice" width="18" height="18" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><path d="M 0 9 H 18 M 9 0 V 18" stroke="#ffffff" stroke-opacity="0.07" stroke-width="1"/></pattern>
      <radialGradient id="green-glow" cx="50%" cy="48%" r="60%"><stop offset="0" stop-color="#1c6b31"/><stop offset="1" stop-color="#07230f"/></radialGradient>
      <linearGradient id="green-phone" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#5b5e63"/><stop offset="0.5" stop-color="#3e4146"/><stop offset="1" stop-color="#2c2e32"/></linearGradient>
    </defs>
    <rect width="280" height="392" fill="url(#green-glow)"/>
    <rect width="280" height="392" fill="url(#green-lattice)"/>
    <rect x="10" y="10" width="260" height="372" rx="10" fill="none" stroke="#f3e9c6" stroke-width="2"/>
    <rect x="16" y="16" width="248" height="360" rx="7" fill="none" stroke="#f3e9c6" stroke-opacity="0.45" stroke-width="1"/>
    ${leaf(34, 36, 0)}${leaf(246, 36, 90)}${leaf(246, 356, 180)}${leaf(34, 356, 270)}
    ${txt(140, 62, "MOBLIN", { font, fs: 30, w: weight, fill: "#f3e9c6" })}
    <path d="M 70 76 H 210" stroke="#f3e9c6" stroke-opacity="0.6" stroke-width="1"/>
    <ellipse cx="140" cy="318" rx="60" ry="8" fill="#000" opacity="0.35"/>
    <rect x="86" y="92" width="108" height="222" rx="18" fill="url(#green-phone)" stroke="#8a8d93" stroke-opacity="0.7" stroke-width="1.5"/>
    <rect x="90" y="96" width="100" height="214" rx="15" fill="none" stroke="#ffffff" stroke-opacity="0.12" stroke-width="1"/>
    <rect x="94" y="100" width="46" height="46" rx="11" fill="#34363b" stroke="#55585e" stroke-width="1"/>
    ${lens(107, 112)}${lens(107, 134)}${lens(128, 123)}
    <circle cx="130" cy="108" r="2.5" fill="#f3e6c2"/>
    ${face(140, 208, 24, { eyes: "open", look: [-1, 1] })}
    ${txt(140, 256, "Moblin", { font, fs: 13, w: weight, fill: "#f2f0e8" })}
    ${txt(140, 346, "IRL STREAMING", { font, fs: 13, w: weight, fill: "#f3e9c6" })}
    ${txt(140, 364, "moblin.app", { fs: 9, w: 600, fill: "#9fd9ad" })}
  </svg>`;
}

export function cardBack(): string {
  return backPhoneOnGreen("Marcellus, Georgia, serif", 400);
}
