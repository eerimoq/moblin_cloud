import { mkdir, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { crc32 } from "node:zlib";
import { chromium } from "playwright-core";
import { preview } from "vite";

const out = resolve(process.argv[2] ?? "../cards-print");
const width = 822;
const height = 1122;
const dpi = 300;
const scale = 750 / 280;
const bleed = 36 / scale;
const safe = 36 / scale;
const backShrink = (140 - safe) / 131;
const pixelRatio = 6;

function withDpi(png: Buffer): Buffer {
  const data = Buffer.alloc(9);
  const ppm = Math.round(dpi / 0.0254);
  data.writeUInt32BE(ppm, 0);
  data.writeUInt32BE(ppm, 4);
  data[8] = 1;
  const type = Buffer.from("pHYs");
  const chunk = Buffer.alloc(21);
  chunk.writeUInt32BE(9, 0);
  type.copy(chunk, 4);
  data.copy(chunk, 8);
  chunk.writeUInt32BE(crc32(Buffer.concat([type, data])), 17);
  const parts = [png.subarray(0, 8)];
  for (let at = 8; at < png.length;) {
    const end = at + 12 + png.readUInt32BE(at);
    const name = png.toString("ascii", at + 4, at + 8);
    if (name !== "pHYs") {
      parts.push(png.subarray(at, end));
    }
    if (name === "IHDR") {
      parts.push(chunk);
    }
    at = end;
  }
  return Buffer.concat(parts);
}

const server = await preview({ preview: { port: 0, strictPort: false, open: false } });
const browser = await chromium.launch({ channel: "chrome" });
let problems = 0;
const fronts: { id: string; png: Buffer }[] = [];
try {
  const page = await browser.newPage({
    viewport: { width: 1400, height: 1000 },
    deviceScaleFactor: pixelRatio,
    reducedMotion: "reduce",
  });
  await page.goto(new URL("cards/", server.resolvedUrls!.local[0]).href, {
    waitUntil: "networkidle",
  });
  await page.evaluate(() => document.fonts.ready);
  const count = await page.evaluate(() => {
    const stage = document.createElement("div");
    stage.id = "print-stage";
    stage.style.cssText = "position:fixed;left:0;top:0;z-index:2147483647";
    document.body.append(stage);
    return document.querySelectorAll("main .tcard").length;
  });
  await rm(out, { recursive: true, force: true });
  await mkdir(out, { recursive: true });
  for (let i = 0; i < count; i++) {
    const card = await page.evaluate(
      ({ i, bleed, safe, backShrink }) => {
        const stage = document.getElementById("print-stage")!;
        const c = document.querySelectorAll("main .tcard")[i].cloneNode(true) as HTMLElement;
        stage.replaceChildren(c);
        c.classList.add("tcard-print");
        c.style.setProperty("--bleed", `${bleed}px`);
        c.style.setProperty("--safe", `${safe}px`);
        const back = c.classList.contains("tcard-back");
        const art = back
          ? c
          : c.classList.contains("tcard-full")
            ? c.querySelector(".tcard-art")
            : null;
        const svg = art?.querySelector("svg");
        if (svg) {
          const shrink = back ? backShrink : 1;
          const w = (280 + 2 * bleed) / shrink;
          const h = (392 + 2 * bleed) / shrink;
          const x = 140 - w / 2;
          const y = 196 - h / 2;
          svg.setAttribute("viewBox", `${x} ${y} ${w} ${h}`);
          for (const r of svg.querySelectorAll(":scope > rect")) {
            if (
              r.getAttribute("width") === "280" &&
              r.getAttribute("height") === "392" &&
              !Number(r.getAttribute("x")) &&
              !Number(r.getAttribute("y"))
            ) {
              r.setAttribute("x", String(x));
              r.setAttribute("y", String(y));
              r.setAttribute("width", String(w));
              r.setAttribute("height", String(h));
            }
          }
        }
        const problems: string[] = [];
        if (!back) {
          const box = c.getBoundingClientRect();
          const inset = bleed + safe;
          for (const e of c.children) {
            if (e.classList.contains("tcard-art") && c.classList.contains("tcard-full")) {
              continue;
            }
            const r = e.getBoundingClientRect();
            if (
              r.left < box.left + inset - 0.5 ||
              r.top < box.top + inset - 0.5 ||
              r.right > box.right - inset + 0.5 ||
              r.bottom > box.bottom - inset + 0.5
            ) {
              problems.push(`${e.className} is outside the safe area`);
            }
          }
          for (const e of c.querySelectorAll<HTMLElement>(
            ".tcard-text, .tcard-name h3, .tcard-type span, .tcard-credit",
          )) {
            if (e.scrollHeight > e.clientHeight + 1 || e.scrollWidth > e.clientWidth + 1) {
              problems.push(`${e.className || e.tagName.toLowerCase()} overflows`);
            }
          }
        }
        return { id: back ? null : c.dataset.card!, problems };
      },
      { i, bleed, safe, backShrink },
    );
    const name = card.id ?? "back";
    for (const problem of card.problems) {
      console.warn(`${name}: ${problem}`);
      problems++;
    }
    const shot = await page.screenshot({
      clip: { x: 0, y: 0, width: width / scale + 1, height: height / scale + 1 },
      animations: "disabled",
    });
    const resized = await page.evaluate(
      async ({ data, width, height, source }) => {
        const image = new Image();
        image.src = `data:image/png;base64,${data}`;
        await image.decode();
        const canvas = document.createElement("canvas");
        canvas.width = width;
        canvas.height = height;
        const context = canvas.getContext("2d")!;
        context.imageSmoothingQuality = "high";
        context.drawImage(image, 0, 0, source * width, source * height, 0, 0, width, height);
        return canvas.toDataURL("image/png").split(",")[1];
      },
      { data: shot.toString("base64"), width, height, source: pixelRatio / scale },
    );
    const png = withDpi(Buffer.from(resized, "base64"));
    if (card.id === null) {
      await writeFile(`${out}/back.png`, png);
      console.log("back.png");
    } else {
      fronts.push({ id: card.id, png });
    }
  }
  let number = 0;
  for (const front of fronts) {
    const copies = front.id === "wifiaware" ? 3 : 5;
    for (let copy = 0; copy < copies; copy++) {
      number++;
      const name = `${String(number).padStart(3, "0")}-${front.id}.png`;
      await writeFile(`${out}/${name}`, front.png);
      console.log(name);
    }
  }
} finally {
  await browser.close();
  await server.close();
}
if (problems > 0) {
  process.exitCode = 1;
}
