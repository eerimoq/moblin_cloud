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
    const cable = (y: number, c: string, t: string, ex: number, end = 150) =>
      `<path d="M 34 ${y} C 80 ${y} 95 ${112 + ex} ${end} ${112 + ex}" fill="none" stroke="${c}" stroke-width="7" stroke-linecap="round"/><path d="M 34 ${y} C 80 ${y} 95 ${112 + ex} ${end} ${112 + ex}" fill="none" stroke="#fff" stroke-width="1.5" stroke-dasharray="6 10" class="tcard-dash" style="--dash: 16" opacity=".7"/>${pill(34, y, t, c, "#fff", 8, 42)}`;
    return svgH(`<rect width="260" height="150" fill="#f1efe4"/>
      ${face(168, 78, 64, { mouth: "big", eyes: "open", look: [-3, 2] })}
      ${cable(24, "#d2333c", "RTMP", -8)}${cable(54, "#e8891d", "SRT", -2)}${cable(84, "#2f7cf6", "RIST", 4)}${cable(114, "#8e44ad", "WHIP", 10, 155)}
      <g transform="translate(150 136)"><rect x="-16" y="-10" width="32" height="20" rx="3" fill="#333" stroke="#000"/><circle cx="0" cy="0" r="6" fill="#111" stroke="#888"/><circle cx="0" cy="0" r="2.5" fill="#2f7cf6"/><rect x="-10" y="-14" width="10" height="5" fill="#333"/></g>
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
    return svgF(`<rect width="280" height="392" fill="#2a2a2a"/>${out}`);
  },
  remote() {
    const queen = `<g transform="translate(-104.847 -140.369)"><path d="m 63.636089,110.01898 -2.728176,-35.146164 10.829966,9.925875 8.500714,-10.814065 10.042946,9.628121 9.582953,-11.391137 9.630658,9.641489 9.66569,-11.021475 8.58851,9.745193 9.00335,-11.348496 2.9833,33.170669 z" fill="#fff80e" stroke="#000" stroke-width="0.6"/>${[
      [74.7, 100.74],
      [94.33, 100.19],
      [114.39, 99.05],
    ]
      .map(
        ([x, y]) =>
          `<ellipse cx="${x}" cy="${y}" rx="1.58" ry="4.74" fill="#ff00ba" transform="rotate(-3.64)"/>`,
      )
      .join("")}${[
      [62.07, 73.77],
      [80.08, 72.47],
      [99.88, 70.55],
      [119.01, 69.28],
      [135.72, 68.15],
    ]
      .map(
        ([x, y]) =>
          `<circle cx="${x}" cy="${y}" r="3" fill="#fff80e" stroke="#000" stroke-width="0.5"/>`,
      )
      .join("")}</g>`;
    const heart = `<g transform="translate(-104.847 -140.369) matrix(1.8079465,0.99176829,-0.93893779,1.9031765,-123.74366,-280.65907)" fill="#ff0a00" stroke="#000" stroke-width="0.3"><path d="m 177.72832,103.21526 -4.51816,-4.935088 c 0,0 -2.38729,-2.231374 -0.32188,-4.149985 2.14478,-1.992344 4.86842,0.888714 4.86842,0.888714"/><path d="m 177.5911,103.21811 4.51816,-4.935087 c 0,0 2.38729,-2.23138 0.32189,-4.14999 -2.14478,-1.99234 -4.86843,0.88872 -4.86843,0.88872"/></g>`;
    return svgH(
      `<rect width="260" height="150" fill="#0f1b2d"/>
      <circle cx="130" cy="82" r="56" fill="#1f4a80" stroke="#8fb6ea" stroke-width="1"/>
      <ellipse cx="130" cy="82" rx="22" ry="56" fill="none" stroke="#8fb6ea" opacity=".6"/><ellipse cx="130" cy="82" rx="42" ry="56" fill="none" stroke="#8fb6ea" opacity=".4"/>
      <path d="M 74 82 h 112 M 80 58 h 100 M 80 106 h 100" stroke="#8fb6ea" opacity=".5"/>
      <clipPath id="globe"><circle cx="130" cy="82" r="56"/></clipPath>
      <g clip-path="url(#globe)"><g transform="translate(130 82) scale(56)" >
      <path d="M -0.95 -0.45 C -0.9 -0.7 -0.7 -0.85 -0.5 -0.85 C -0.35 -0.88 -0.2 -0.8 -0.22 -0.65 C -0.25 -0.55 -0.35 -0.52 -0.33 -0.42 C -0.3 -0.33 -0.25 -0.25 -0.35 -0.18 C -0.45 -0.12 -0.48 -0.05 -0.45 0.05 C -0.55 0 -0.62 -0.08 -0.68 -0.15 C -0.78 -0.2 -0.9 -0.3 -0.95 -0.45 Z" fill="#4a8a3c"/>
      <path d="M -0.45 0.05 C -0.35 0 -0.2 0.05 -0.12 0.15 C -0.05 0.28 -0.12 0.42 -0.2 0.55 C -0.26 0.68 -0.3 0.82 -0.36 0.88 C -0.4 0.8 -0.42 0.65 -0.45 0.5 C -0.5 0.38 -0.55 0.25 -0.55 0.15 C -0.55 0.08 -0.5 0.05 -0.45 0.05 Z" fill="#3a7f33"/>
      <path d="M 0.22 -0.6 C 0.3 -0.75 0.5 -0.8 0.65 -0.78 C 0.75 -0.7 0.72 -0.6 0.78 -0.5 C 0.7 -0.45 0.6 -0.42 0.5 -0.4 C 0.42 -0.36 0.34 -0.4 0.3 -0.45 C 0.26 -0.5 0.2 -0.55 0.22 -0.6 Z" fill="#5c9440"/>
      <path d="M 0.3 -0.35 C 0.45 -0.42 0.62 -0.4 0.75 -0.3 C 0.85 -0.2 0.85 -0.05 0.78 0.02 C 0.74 0.15 0.68 0.3 0.6 0.45 C 0.55 0.58 0.47 0.6 0.43 0.5 C 0.4 0.38 0.4 0.25 0.42 0.12 C 0.36 0.1 0.26 0.08 0.2 0 C 0.15 -0.1 0.18 -0.25 0.3 -0.35 Z" fill="url(#africa)"/>
      <path d="M 0.78 -0.75 C 0.9 -0.7 1 -0.6 1 -0.4 C 1 -0.2 0.95 -0.1 0.85 -0.05 C 0.9 -0.2 0.85 -0.35 0.8 -0.45 C 0.78 -0.55 0.75 -0.65 0.78 -0.75 Z" fill="#8a9a4a"/>
      <path d="M -0.3 -0.84 C -0.2 -0.9 -0.06 -0.86 -0.06 -0.74 C -0.08 -0.63 -0.16 -0.55 -0.24 -0.58 C -0.32 -0.64 -0.34 -0.76 -0.3 -0.84 Z" fill="#e6eef2"/><ellipse cx="0.12" cy="-0.6" rx="0.055" ry="0.035" fill="#5c9440"/>
      <path d="M -0.78 0.96 C -0.68 0.86 -0.56 0.9 -0.46 0.84 C -0.36 0.79 -0.28 0.87 -0.18 0.83 C -0.08 0.79 0 0.86 0.1 0.82 C 0.2 0.79 0.3 0.87 0.4 0.84 C 0.5 0.81 0.6 0.89 0.74 0.93 L 0.74 1.1 L -0.78 1.1 Z M -0.78 -0.96 C -0.66 -0.9 -0.56 -0.83 -0.46 -0.86 C -0.36 -0.89 -0.3 -0.8 -0.2 -0.82 C -0.1 -0.85 -0.02 -0.78 0.08 -0.8 C 0.18 -0.84 0.28 -0.89 0.38 -0.84 C 0.48 -0.79 0.58 -0.87 0.74 -0.93 L 0.74 -1.1 L -0.78 -1.1 Z" fill="#eef3f5"/>
      </g></g>
      <g transform="translate(126 27)"><ellipse rx="5" ry="2.6" fill="#fff" stroke="#333" stroke-width=".6"/><g fill="#fff" stroke="#333" stroke-width=".6"><rect x="-4.2" y="1.5" width="1.6" height="3" rx=".6"/><rect x="-1.8" y="1.8" width="1.6" height="2.8" rx=".6"/><rect x="1" y="1.8" width="1.6" height="2.8" rx=".6"/><rect x="3" y="1.5" width="1.6" height="3" rx=".6"/></g><circle cx="4.6" cy="-2.9" r=".7" fill="#fff" stroke="#333" stroke-width=".5"/><circle cx="5.5" cy="-1.2" r="1.9" fill="#fff" stroke="#333" stroke-width=".6"/><circle cx="7.2" cy="-1" r=".5" fill="#111"/><circle cx="5.4" cy="-1.8" r=".35" fill="#111"/></g>
      <g transform="translate(134 135) rotate(180)"><ellipse rx="2.3" ry="3.3" fill="#111" stroke="#8fb6ea" stroke-width=".4"/><circle cy="-2.8" r="1.5" fill="#111" stroke="#8fb6ea" stroke-width=".4"/><ellipse cy=".7" rx="1.4" ry="2.3" fill="#fff"/><path d="M 1.3 -2.8 l 1.5 .4 l -1.5 .5 z" fill="#f5a623"/><circle cx=".6" cy="-3.2" r=".3" fill="#fff"/><path d="M -1.4 3.4 h 1.2 M .2 3.4 h 1.2" stroke="#f5a623" stroke-width=".8" stroke-linecap="round"/></g>
      <path d="M 56 110 Q 130 10 206 110" fill="none" stroke="#ffd166" stroke-width="2.5" stroke-dasharray="6 8" class="tcard-dash" style="--dash: 14"/>
      ${face(218, 112, 26, { extra: queen })}
      ${face(42, 112, 26, { extra: `<path d="M -53.6 -8 A 54 54 0 0 1 53.6 -8" fill="none" stroke="#9b6dff" stroke-width="6" stroke-linecap="round"/><rect x="-59" y="-16" width="13" height="26" rx="6" fill="#9b6dff" stroke="#4b2a99" stroke-width="1.2"/><rect x="46" y="-16" width="13" height="26" rx="6" fill="#9b6dff" stroke="#4b2a99" stroke-width="1.2"/><path d="M 50 8 q 0 16 -8 18" fill="none" stroke="#9b6dff" stroke-width="3.5" stroke-linecap="round"/><circle cx="40" cy="26" r="4.5" fill="#4b2a99"/>${heart}` })}
      <rect x="72" y="124" width="40" height="18" rx="9" fill="#ff3b30" stroke="#7a0f0f"/>${txt(92, 136, "ZOOM", { fs: 8 })}
      ${txt(42, 146, "assistant", { fs: 8, fill: "#8fb6ea" })}${txt(218, 146, "streamer", { fs: 8, fill: "#8fb6ea" })}`,
      `<linearGradient id="africa" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#c9a85c"/><stop offset=".32" stop-color="#c9a85c"/><stop offset=".48" stop-color="#4a8a3c"/><stop offset=".8" stop-color="#4a8a3c"/><stop offset="1" stop-color="#b89a60"/></linearGradient>`,
    );
  },
  website() {
    const marks: Record<string, [string, string, string]> = {
      twitch: [
        "#9146ff",
        "#fff",
        "M11.571 4.714h1.715v5.143H11.571zm4.715 0H18v5.143h-1.714zM6 0L1.714 4.286v15.428h5.143V24l4.286-4.286h3.428L22.286 12V0zm14.571 11.143l-3.428 3.428h-3.429l-3 3v-3H6.857V1.714h13.714Z",
      ],
      youtube: [
        "#ff0033",
        "#fff",
        "M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z",
      ],
      kick: [
        "#000",
        "#00e701",
        "M1.333 0h8v5.333H12V2.667h2.667V0h8v8H20v2.667h-2.667v2.666H20V16h2.667v8h-8v-2.667H12v-2.666H9.333V24h-8Z",
      ],
    };
    const code = (x: number, y: number) =>
      `<image href="${asset("cards/eerimoq.webp")}" x="${x}" y="${y}" width="60" height="34" preserveAspectRatio="xMidYMid slice"/>`;
    const night = (x: number, y: number) =>
      `<rect x="${x}" y="${y}" width="60" height="34" fill="#0d1230"/><circle cx="${x + 50}" cy="${y + 8}" r="4" fill="#fff6c2"/><rect x="${x + 4}" y="${y + 14}" width="12" height="20" fill="#1a2047"/><rect x="${x + 20}" y="${y + 6}" width="14" height="28" fill="#161b3e"/><rect x="${x + 38}" y="${y + 18}" width="10" height="16" fill="#1a2047"/><g fill="#ffd866"><rect x="${x + 7}" y="${y + 18}" width="2" height="2"/><rect x="${x + 11}" y="${y + 24}" width="2" height="2"/><rect x="${x + 23}" y="${y + 10}" width="2" height="2"/><rect x="${x + 29}" y="${y + 16}" width="2" height="2"/><rect x="${x + 24}" y="${y + 24}" width="2" height="2"/><rect x="${x + 42}" y="${y + 22}" width="2" height="2"/></g>${face(x + 30, y + 28, 6, { eyes: "open", mouth: "logo", look: [0, -2] })}`;
    const sunset = (x: number, y: number) =>
      `<rect x="${x}" y="${y}" width="60" height="34" fill="url(#wsdusk)"/><circle cx="${x + 30}" cy="${y + 20}" r="8" fill="#ffb347"/><rect x="${x}" y="${y + 24}" width="60" height="10" fill="#2b4f7a"/>${face(x + 30, y + 30, 6, { eyes: "happy", mouth: "o" })}`;
    const row = (
      x: number,
      y: number,
      name: string,
      cat: string,
      platform: string,
      logo: string,
    ) => {
      const [bg, fg, d] = marks[platform]!;
      return `<clipPath id="wa${x}${y}"><circle cx="${x + 13}" cy="${y}" r="7"/></clipPath><image href="${asset(`cards/avatar-${logo}.png`)}" x="${x + 6}" y="${y - 7}" width="14" height="14" clip-path="url(#wa${x}${y})"/><circle cx="${x + 18.5}" cy="${y + 5.5}" r="4.3" fill="#1b1e1c"/><circle cx="${x + 18.5}" cy="${y + 5.5}" r="3.3" fill="${bg}"/><g transform="translate(${x + 16.5} ${y + 3.5}) scale(0.167)"><path d="${d}" fill="${fg}"/></g>
      <clipPath id="wt${x}${y}"><rect x="${x + 24}" y="${y - 10}" width="42" height="20"/></clipPath><g clip-path="url(#wt${x}${y})">${txt(x + 24, y + (cat ? -1 : 2.5), name, { fs: 6.5, fill: "#f2f0e8", anchor: "start" })}${cat ? txt(x + 24, y + 7, cat, { fs: 5, fill: "#a3a89f", anchor: "start", w: 600 }) : ""}</g>`;
    };
    const card = (
      x: number,
      name: string,
      cat: string,
      platform: string,
      scene: (x: number, y: number) => string,
      logo: string,
    ) =>
      `<rect x="${x}" y="44" width="72" height="66" rx="6" fill="#1b1e1c" stroke="#2a2f2c" stroke-width="1.2"/>
      <clipPath id="ws${x}"><rect x="${x + 6}" y="50" width="60" height="34" rx="3.5"/></clipPath><g clip-path="url(#ws${x})">${scene(x + 6, 50)}</g>
      ${row(x, 97, name, cat, platform, logo)}`;
    const small = (x: number, name: string, platform: string, logo: string) =>
      `<rect x="${x}" y="118" width="72" height="30" rx="6" fill="#1b1e1c" stroke="#2a2f2c" stroke-width="1.2"/>${row(x, 133, name, "", platform, logo)}`;
    return svgH(
      `<rect width="260" height="150" fill="#161917"/>
      ${txt(14, 17, "STREAMERS", { fs: 6, fill: "#4cd964", anchor: "start", ls: 0.8, font: DISPLAY, w: 600 })}
      ${txt(14, 33, "Who's streaming with Moblin.", { fs: 12, fill: "#f2f0e8", anchor: "start", font: DISPLAY, w: 600 })}
      ${txt(246, 23, "IRL streamers who recently", { fs: 4.6, fill: "#a3a89f", anchor: "end", w: 600 })}${txt(246, 29, "went live with Moblin. Drop by", { fs: 4.6, fill: "#a3a89f", anchor: "end", w: 600 })}${txt(246, 35, "their channels and say hi.", { fs: 4.6, fill: "#a3a89f", anchor: "end", w: 600 })}
      ${card(94, "TokyoTom", "Just Chatting", "kick", night, "king")}
      ${card(174, "sofia.cycles", "IRL", "youtube", sunset, "pink")}
      ${card(14, "eerimoq", "Software and Ga…", "twitch", code, "default")}
      ${small(14, "ChatterBox", "kick", "goblina")}${small(94, "moblin_mike", "youtube", "happy")}${small(174, "irl_ida", "twitch", "tetris")}`,
      `<linearGradient id="wsdusk" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4a2a6b"/><stop offset="1" stop-color="#ff7a59"/></linearGradient>`,
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
    return svgH(
      `<rect width="260" height="150" fill="url(#gpsky)"/>
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
      ${call(222, 108, "Torch", "#ffcc00", "#1c1c1e", at(158, 123))}`,
      `<linearGradient id="gpsky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#8fbcf0"/><stop offset="1" stop-color="#f4f7fb"/></linearGradient>`,
    );
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
      <path d="M 106 74 C 150 70 196 84 210 112" fill="none" stroke="#2f7cf6" stroke-width="3" stroke-dasharray="6 6" class="tcard-dash" style="--dash: 12"/>
      ${pill(160, 72, "RTMP", "#2f7cf6", "#fff", 9, 46)}
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
    const cable = "M 58.5 129 C 58.5 148 72 150 84 142 C 92 137 90 121 107 121";
    return svgH(
      `<rect width="260" height="150" fill="#f1efe4"/>
      <path d="${cable}" fill="none" stroke="#000" stroke-width="4" stroke-linecap="round"/><path d="${cable}" fill="none" stroke="#1a8a1a" stroke-width="2.4" stroke-linecap="round"/><rect x="104" y="118.5" width="9" height="5" rx="1.2" fill="#1a8a1a" stroke="#000"/>
      ${phone(31, 16, 55, 114, `<clipPath id="mcphone"><rect x="33.5" y="18.5" width="50" height="109" rx="5.5"/></clipPath><image href="${asset("cards/mobcam-phone.webp")}" x="33.5" y="18.5" width="50" height="109" preserveAspectRatio="xMidYMid slice" clip-path="url(#mcphone)"/>`)}
      <g transform="translate(-2 7)">
      <rect x="116" y="22" width="132" height="86" rx="5" fill="#2c2c2e" stroke="#000"/>
      <path d="M 116 108 h 132 l 3 11 h -138 z" fill="#bdbdc2" stroke="#666"/><path d="M 113 119 h 138 v 3 h -138 z" fill="#9a9aa0" stroke="#666"/><path d="M 130 109.8 h 104 l 1.2 3.8 h -106.4 z" fill="#3a3a40"/><path d="M 156 114.8 h 52 l 1 3.2 h -54 z" fill="#aeaeb4" stroke="#7a7a80" stroke-width="0.6"/>
      <rect x="117.5" y="23.5" width="129" height="83" rx="3.5" fill="#1f1f23"/>
      <rect x="162" y="25" width="83" height="80" fill="#000"/><image href="${asset("cards/mobcam-obs.webp")}" x="162" y="25" width="83" height="80" preserveAspectRatio="xMidYMid meet"/>
      <rect x="119" y="25" width="41" height="25" rx="2" fill="#2b2b30"/><rect x="119" y="51.5" width="41" height="25" rx="2" fill="#2b2b30"/><rect x="119" y="78" width="41" height="27" rx="2" fill="#2b2b30"/>
      ${txt(122, 31.5, "Scenes", { fs: 5, fill: "#bbb", anchor: "start" })}${txt(122, 58, "Sources", { fs: 5, fill: "#bbb", anchor: "start" })}${txt(122, 84.5, "Audio Mixer", { fs: 5, fill: "#bbb", anchor: "start" })}
      <rect x="121" y="34.5" width="37" height="7" rx="1" fill="#3a3a40"/>${txt(124, 39.7, "IRL", { fs: 4.5, fill: "#ddd", anchor: "start", w: 600 })}
      <rect x="121" y="61" width="37" height="7" rx="1" fill="#3a5a9c"/>${txt(124, 66.2, "Mobcam", { fs: 4.5, fill: "#fff", anchor: "start", w: 600 })}
      ${[
        [124, 28],
        [127.5, 34],
        [146, 48],
        [149.5, 54],
      ]
        .map(
          ([x, top]) =>
            `<rect x="${x}" y="87" width="2.5" height="12" fill="#3a3a40"/><rect x="${x}" y="87" width="2.5" height="12" fill="url(#mcvu)" clip-path="inset(${top}% 0 0 0)"/>`,
        )
        .join("")}
      ${txt(127, 103.5, "Mobcam", { fs: 3.5, fill: "#aaa", w: 600 })}${txt(149, 103.5, "Mic/Aux", { fs: 3.5, fill: "#aaa", w: 600 })}
      ${pill(190, 7.5, "OBS Plugin", "#2c2c2e", "#fff", 8, 58)}</g>
      <image href="${asset("logos/logo-mobcam.png")}" x="140" y="4.5" width="${(20 * 363) / 440}" height="20"/>`,
      `<linearGradient id="mcvu" x1="0" y1="1" x2="0" y2="0"><stop offset="0" stop-color="#4caf50"/><stop offset=".6" stop-color="#4caf50"/><stop offset=".8" stop-color="#ffd54f"/><stop offset="1" stop-color="#e53935"/></linearGradient>`,
    );
  },
  snapshot() {
    const ray = (a: number) =>
      `<path d="M 0 0 l 0 -22" stroke="#ffd60a" stroke-width="3" stroke-linecap="round" transform="rotate(${a})"/>`;
    return svgH(
      `<rect width="260" height="150" fill="#e9e5f6"/>
      <g transform="rotate(-7 100 76)"><rect x="40" y="18" width="118" height="128" rx="3" fill="#fff" stroke="#bbb"/><rect x="48" y="26" width="102" height="92" fill="#dfe8d8"/>${face(99, 74, 38, { eyes: "open", look: [0, -3], mouth: "o" })}${txt(99, 136, "perfect?", { fs: 9, fill: "#555", w: 400 })}</g>
      <g transform="translate(148 22)">${[0, 45, 90, 135, 180, 225, 270, 315].map(ray).join("")}<circle r="6" fill="#fff"/></g>
      <path d="M 164 83 L 186 83" fill="none" stroke="#5865f2" stroke-width="3" stroke-dasharray="6 6" class="tcard-dash" style="--dash: 12" marker-end="url(#arr)"/>
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
      ${txt(50, 134, "You", { fs: 8, fill: "#c9b8ff" })}
      <path d="M 96 84 h 30" stroke="#c9b8ff" stroke-width="2" marker-end="url(#arr2)"/>
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
    const bubble = (x: number, y: number, w: number, t: string, color: string, rot: number) =>
      `<g transform="rotate(${rot} ${x + w / 2} ${y + 9})"><rect x="${x}" y="${y}" width="${w}" height="18" rx="9" fill="#fff" stroke="#d6d2c2"/>${txt(x + w / 2, y + 12.5, t, { fs: 8, fill: color })}</g>`;
    return svgH(`<rect width="260" height="150" fill="#f1efe4"/>
      ${bubble(14, 30, 50, "W W W", "#2f7cf6", -8)}${bubble(22, 60, 58, "LETS GOOO", "#e0669c", 5)}${bubble(12, 92, 40, "gg", "#2ecc71", -4)}
      <path d="M 70 44 q 8 4 12 12 M 74 98 q 6 -2 10 -8" fill="none" stroke="#bbb" stroke-width="1.5" stroke-dasharray="2 3" stroke-linecap="round"/>
      <image href="${asset("screenshots/watch.webp")}" x="82" y="-4" width="${(158 * 400) / 650}" height="158"/>
      <g transform="rotate(6 216 62)">
        <rect x="186" y="40" width="62" height="44" rx="4" fill="#0b10ac"/>
        ${txt(191, 55, "ERIK", { fs: 6, w: 500, anchor: "start" })}${txt(191, 62, "ANNA", { fs: 6, w: 500, anchor: "start" })}
        ${txt(191, 73, "MAX", { fs: 6, w: 500, anchor: "start" })}${txt(191, 80, "LISA", { fs: 6, w: 500, anchor: "start" })}
        ${txt(224, 61, "6", { fs: 13, w: 800 })}${txt(238, 61, "4", { fs: 13, w: 400 })}
        ${txt(224, 80, "3", { fs: 13, w: 400 })}${txt(238, 80, "6", { fs: 13, w: 800 })}
      </g>
      <g transform="rotate(-10 222 110)"><rect x="204" y="100" width="36" height="20" rx="10" fill="#34c759"/>${txt(222, 114, "+1", { fs: 11 })}</g>
      <path d="M 196 100 l -5 -4 M 198 112 l -7 1 M 246 98 l 5 -4" stroke="#34c759" stroke-width="2" stroke-linecap="round"/>`);
  },
  textwidget() {
    const row = (y: number, v: string, t: string, bg: string) =>
      `<rect x="14" y="${y - 9}" width="118" height="18" rx="9" fill="#cfd6dc"/>${txt(73, y + 3.5, v, { fs: 9, fill: "#1c1c1e", font: MONO, w: 600 })}${pill(196, y, t, bg, "#fff", 9.5, 92)}`;
    return svgH(`<rect width="260" height="150" fill="#e7ecef"/>
      ${txt(73, 16, "YOU TYPE", { fs: 7, fill: "#6a737a", ls: 1 })}${txt(196, 16, "ON STREAM", { fs: 7, fill: "#6a737a", ls: 1 })}
      ${row(34, "{time}", "12:34:56", "#1c1c1e")}
      ${row(66, "Ran {distance}", "Ran 3.9 km", "#0d7a30")}
      ${row(98, "{speed} ♥ {heartRate}", "42 km/h ♥ 128", "#7b3fe4")}
      ${face(60, 152, 30, { eyes: "open", look: [2, -4], mouth: "open" })}`);
  },
  macros() {
    const steps: [string, string, string][] = [
      ["Wait for raid", "pauses until someone raids", "#9146ff"],
      ["Shoutout the raider", "", "#e8891d"],
      ["Reaction fireworks", "", "#ff3b7a"],
    ];
    const rows = steps
      .map(
        ([t, sub, c], i) =>
          `<circle cx="108" cy="${60 + i * 30}" r="8" fill="${c}"/>${txt(108, 63 + i * 30, i + 1, { fs: 8 })}${txt(120, 63 + i * 30 - (sub ? 3 : 0), t, { fs: 8.5, fill: "#1c1c1e", anchor: "start" })}${sub ? txt(120, 70 + i * 30, sub, { fs: 5.5, fill: "#888", anchor: "start", w: 600 }) : ""}`,
      )
      .join("");
    return svgH(`<rect width="260" height="150" fill="#f6f1e4"/>
      <rect x="92" y="14" width="150" height="122" rx="8" fill="#fff" stroke="#d9d4c6"/>
      <rect x="92" y="14" width="150" height="30" rx="8" fill="#ece8dc"/><rect x="92" y="36" width="150" height="8" fill="#ece8dc"/>
      <path d="M 103 25 L 113 30 L 103 35 Z" fill="#555"/>${txt(120, 33, "Raid hype macro", { fs: 9, fill: "#1c1c1e", anchor: "start", font: DISPLAY, w: 600 })}
      <path d="M 108 68 V 112" stroke="#d9d4c6" stroke-width="2"/>
      ${rows}
      <circle cx="44" cy="112" r="17" fill="#8a2020"/><circle cx="44" cy="109" r="16" fill="#ff3b30" stroke="#7a0f0f"/><path d="M 39 102 L 52 109 L 39 116 Z" fill="#fff"/>${txt(44, 140, "TAP TO RUN", { fs: 6, fill: "#555", ls: 0.5 })}
      ${face(44, 48, 26, { eyes: "open", look: [0, 6], mouth: "o" })}
      <path d="M 50 72 L 46 92" stroke="#1e1a1a" stroke-width="7.2" stroke-linecap="round"/><path d="M 50 72 L 46 92" stroke="#008000" stroke-width="6" stroke-linecap="round"/>`);
  },
  moblink() {
    const antenna = `<path transform="translate(-104.847 -157.801)" d="m 65.533703,191.87947 a 1.6070492,1.6070492 0 0 1 -2.408228,2.12858 c -3.806446,-4.30323 -5.750793,-8.87293 -5.675125,-13.51832 0.07896,-4.52365 2.069364,-9.03085 6.122557,-13.33407 a 1.6120649,1.6120649 0 0 1 2.345718,2.20753 c -3.480744,3.69459 -5.178347,7.48459 -5.250725,11.17919 -0.0658,3.81303 1.605485,7.64908 4.865803,11.33709 z m 13.597275,-7.79384 v 21.50296 a 1.4936277,1.4936277 0 0 1 -2.987255,0 v -21.50296 a 3.7899975,3.7899975 0 1 1 2.987255,0 z m 7.155594,4.66182 a 1.6090156,1.6090156 0 0 1 -2.497055,-2.02988 c 1.608774,-1.97396 2.444416,-4.06964 2.450995,-6.1423 0.0066,-2.07265 -0.779712,-4.09596 -2.408227,-6.09294 a 1.6090169,1.6090169 0 1 1 2.497056,-2.02989 c 2.115424,2.59247 3.132005,5.35271 3.125427,8.12283 0,2.83264 -1.092251,5.61263 -3.168196,8.17218 z m -14.801388,-2.02988 a 1.6090161,1.6090161 0 0 1 -2.497056,2.02988 c -2.075944,-2.55955 -3.158331,-5.33954 -3.168201,-8.17218 0,-2.77012 1.010009,-5.53036 3.128722,-8.12283 a 1.608775,1.608775 0 1 1 2.493766,2.02989 c -1.644965,1.99698 -2.411518,4.06635 -2.408228,6.09294 0.0032,2.0266 0.845512,4.16834 2.450997,6.1423 z m 20.664039,7.29048 a 1.6070492,1.6070492 0 0 1 -2.408228,-2.12858 c 3.263613,-3.68801 4.934893,-7.52406 4.865807,-11.33709 -0.06255,-3.71104 -1.769983,-7.4846 -5.263893,-11.18576 a 1.6120649,1.6120649 0 1 1 2.34572,-2.20754 c 4.053195,4.30323 6.043598,8.81043 6.119261,13.33408 0.07896,4.64538 -1.865375,9.21179 -5.671823,13.51831 z" fill="#000" stroke="#fff" stroke-width="0.93"/>`;
    return svgH(`<rect width="260" height="150" fill="#eef3fb"/>
      <path d="M 44 54 C 44 34 100 38 116 26 M 130 66 V 26 M 216 54 C 216 34 160 38 144 26" fill="none" stroke="#2f7cf6" stroke-width="2" stroke-dasharray="4 4" class="tcard-dash" style="--dash: 8"/>
      <rect x="108" y="8" width="44" height="18" rx="3" fill="#2b2d31" stroke="#000"/><rect x="112" y="11" width="24" height="4" rx="1" fill="#444"/><rect x="112" y="19" width="24" height="4" rx="1" fill="#444"/><circle cx="144" cy="13" r="1.6" fill="#4cd964"/><circle cx="144" cy="21" r="1.6" fill="#4cd964" class="tcard-pulse"/>
      ${txt(158, 20, "Server", { fs: 7, fill: "#1c1c1e", anchor: "start" })}
      <path d="M 80 90 H 64" stroke="#0d7a30" stroke-width="2.5" stroke-dasharray="4 4" class="tcard-dash" style="--dash: 8"/><path d="M 180 90 H 196" stroke="#2f7cf6" stroke-width="2.5" stroke-dasharray="4 4" class="tcard-dash" style="--dash: 8"/>
      <image href="${asset("screenshots/moblin-800.webp")}" x="82" y="66" width="96" height="${(96 * 389) / 800}"/>
      <rect x="26" y="54" width="36" height="70" rx="4" fill="#2b2d31" stroke="#000"/><rect x="29" y="57" width="30" height="64" rx="2" fill="#0a0f0a"/><circle cx="44" cy="60.5" r="1.6" fill="#000" stroke="#444" stroke-width=".6"/>${[5, 9, 13].map((r) => `<path d="M ${44 - r} 94 A ${r} ${r} 0 0 1 ${44 + r} 94" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round"/>`).join("")}<circle cx="44" cy="95" r="2" fill="#fff"/><path d="M 37 117 l -2.5 -1.5 l 2.5 -1.5 z" fill="#888"/><circle cx="44" cy="115.5" r="1.5" fill="none" stroke="#888" stroke-width=".8"/><rect x="50" y="114" width="3" height="3" fill="none" stroke="#888" stroke-width=".8"/>
      <rect x="198" y="54" width="36" height="70" rx="9" fill="#1c1c1e" stroke="#9a9aa0" stroke-width="1.2"/><rect x="201" y="57" width="30" height="64" rx="7" fill="#0a0f0a"/><rect x="210" y="59.5" width="12" height="3.6" rx="1.8" fill="#000"/>${face(216, 88, 11, { extra: antenna })}<rect x="210" y="117.5" width="12" height="1.3" rx=".65" fill="#777"/>
      ${(
        [
          [44, "Hotspot", "A"],
          [130, "Moblin", "B"],
          [216, "Moblink", "C"],
        ] as [number, string, string][]
      )
        .map(
          ([x, t, c]) =>
            `${txt(x, 135, t, { fs: 7, fill: "#1c1c1e" })}${txt(x, 143, `carrier ${c}`, { fs: 5.5, fill: "#777", w: 600 })}`,
        )
        .join("")}`);
  },
  photoshoot() {
    return svgH(
      `<rect width="260" height="150" fill="#12101a"/>
      <path d="M 20 -10 L 90 150 L 170 150 Z" fill="url(#spot)" opacity=".8"/><path d="M 240 -10 L 170 150 L 90 150 Z" fill="url(#spot)" opacity=".8"/>
      <rect x="92" y="34" width="86" height="86" fill="none" stroke="#fff" opacity=".2"/><rect x="100" y="28" width="86" height="86" fill="none" stroke="#fff" opacity=".35"/><rect x="108" y="22" width="86" height="86" rx="2" fill="none" stroke="#fff" opacity=".6"/>
      ${face(140, 88, 48, { eyes: "wink", mouth: "open", rot: -10 })}
      <g transform="translate(228 22)">${[0, 45, 90, 135, 180, 225, 270, 315].map((a) => `<path d="M 0 0 l 0 -14" stroke="#fff" stroke-width="2.5" stroke-linecap="round" transform="rotate(${a})"/>`).join("")}<circle r="4" fill="#fff"/></g>
      ${txt(140, 144, "interval 3s", { fs: 8, fill: "#bbb", font: MONO })}`,
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
    return svgH(`<rect width="260" height="150" fill="#f6f6f1"/>
      <path d="M 20 140 H 250 M 20 140 V 20" stroke="#bbb" stroke-width="1"/>
      <path d="${d} L 244 140 L 20 140 Z" fill="#0d7a30" opacity=".15"/>
      <path d="${d}" fill="none" stroke="#0d7a30" stroke-width="3" stroke-linejoin="round" stroke-linecap="round"/>
      ${pts.map((p) => `<circle cx="${p[0]}" cy="${p[1]}" r="2.5" fill="#0d7a30"/>`).join("")}
      <ellipse cx="146" cy="116" rx="22" ry="5" fill="#ff9f0a" stroke="#7a4a00"/>
      ${face(146, 96, 18, { eyes: "open", look: [3, 2], mouth: "o", rot: 12 })}
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
      <image href="${asset("logos/logo-looking.png")}" x="${244 - 13}" y="${226 - (26 * 573) / 500}" width="26" height="${(26 * 573) / 500}" transform="rotate(48 244 214) translate(488 0) scale(-1 1)"/>
      ${txt(140, 380, "one rope. zero fear.", { fs: 9, fill: "#ffe7a3" })}`,
      `<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#0b1a33"/><stop offset="1" stop-color="#1f4a7a"/></linearGradient>`,
    );
  },
  selfie() {
    return svgH(
      `<rect width="260" height="150" fill="#dfeee6"/>
      <path d="M 36 150 L 172 48" stroke="#333" stroke-width="7" stroke-linecap="round"/><path d="M 36 150 L 80 118" stroke="#1c1c1e" stroke-width="12" stroke-linecap="round"/>
      <g transform="rotate(53 180 42)"><rect x="138" y="22" width="84" height="40" rx="8" fill="#1c1c1e" stroke="#000"/><g clip-path="url(#selfiescreen)"><rect x="144" y="26" width="72" height="32" fill="url(#selfiesky)"/>
      <path d="M 140 58 L 140 55 Q 170 50 200 56 L 220 54 L 220 58 Z" fill="#4f8f4a"/><path d="M 204 50 a 2.5 2.5 0 0 1 4 -1.5 a 3 3 0 0 1 5 1 a 2 2 0 0 1 0 4 h -9 a 2 2 0 0 1 0 -3.5 z" fill="#fff"/>
      <path d="M 151 39 Q 180 20 209 39" fill="none" stroke="#f4501a" stroke-width="6.5" stroke-linecap="round"/><path d="M 158 32 l 1.5 2.5 M 166 28.5 l 1 2.8 M 174 26.8 l 0.3 3 M 182 26.6 l -0.3 3 M 190 27.6 l -0.8 2.9 M 198 30.2 l -1.3 2.7" stroke="#ffd166" stroke-width="1.1"/>
      <path d="M 152 41 L 175 47 M 166 33 L 177 45 M 194 33 L 183 45 M 208 41 L 185 47" stroke="#333" stroke-width="0.6"/>
      ${face(180, 50, 8.5, { eyes: "open", mouth: "open" })}</g>
      <rect x="140.5" y="35" width="2.5" height="14" rx="1.2" fill="#444"/></g>
      <circle cx="62" cy="128" r="12" fill="#ff3b30" stroke="#7a0f0f" stroke-width="2"/><circle cx="62" cy="128" r="6" fill="#fff" opacity=".4"/>
      <path d="M 76 136 l 30 0" stroke="#333" stroke-width="1.5"/>${pill(152, 136, "press: switch scene", "#0d7a30", "#fff", 8, 92)}`,
      `<linearGradient id="selfiesky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#6fb3ff"/><stop offset="1" stop-color="#d7ecff"/></linearGradient>
      <clipPath id="selfiescreen"><rect x="144" y="26" width="72" height="32" rx="5"/></clipPath>`,
    );
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
          `<path d="M ${140 - r} ${236} a ${r} ${r} 0 0 1 ${2 * r} 0" fill="none" stroke="#ff2d3a" stroke-width="3" stroke-linecap="round" stroke-dasharray="6 5" class="tcard-dash" style="--dash: 11"/>`,
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
  const lens = (x: number, y: number) =>
    `<circle cx="${x}" cy="${y}" r="8" fill="#1c1c1e" stroke="#4a4a4e" stroke-width="1"/><circle cx="${x}" cy="${y}" r="5" fill="#0d1a2e"/><circle cx="${x - 1.8}" cy="${y - 1.8}" r="1.4" fill="#fff" opacity="0.6"/>`;
  return `<svg viewBox="0 0 280 392" role="img">
    <defs>
      <radialGradient id="green-glow" cx="50%" cy="48%" r="60%"><stop offset="0" stop-color="#1c6b31"/><stop offset="1" stop-color="#07230f"/></radialGradient>
      <linearGradient id="green-phone" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#5b5e63"/><stop offset="0.5" stop-color="#3e4146"/><stop offset="1" stop-color="#2c2e32"/></linearGradient>
    </defs>
    <rect width="280" height="392" fill="url(#green-glow)"/>
    <rect x="10" y="10" width="260" height="372" rx="10" fill="none" stroke="#f3e9c6" stroke-width="2"/>
    <rect x="16" y="16" width="248" height="360" rx="7" fill="none" stroke="#f3e9c6" stroke-opacity="0.45" stroke-width="1"/>
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
  return backPhoneOnGreen(DISPLAY, 600);
}
