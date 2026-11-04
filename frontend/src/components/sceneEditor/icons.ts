import type { IconName } from "../Icon";
import type { SettingsAlignment, SettingsWidgetType } from "../../data/moblinSettings";
import { widgetTypeSymbol } from "../../data/moblinSettings";

const symbols: Record<string, IconName> = {
  textformat: "textformat",
  globe: "globe",
  video: "video",
  photo: "image",
  megaphone: "megaphone",
  map: "map",
  "camera.aperture": "camera-aperture",
  message: "chat",
  "hands.clap": "hands-clap",
  "photo.on.rectangle": "photo-on-rectangle",
  "play.rectangle": "play-rectangle",
  "person.crop.circle": "person-circle",
  "person.crop.circle.dashed": "person-circle-dashed",
  qrcode: "qrcode",
  "rectangle.split.2x1": "rectangle-split",
  burn: "wheel",
  "square.grid.3x3.square": "grid",
  crop: "crop",
  timer: "timer",
  "sun.max": "sparkles",
  "bolt.circle": "bolt",
  graduationcap: "star",
  "text.book.closed": "list",
  "arrowtriangle.right.circle": "play",
  "brain.head.profile": "user",
  pencil: "sliders",
  flame: "flame",
  target: "target",
  "cup.and.saucer": "clock",
  "figure.dance": "user",
  "figure.walk": "user",
  "music.note": "waveform",
  leaf: "sparkles",
  gamecontroller: "gamepad",
  "fork.knife": "more",
};

export const symbolIcon = (symbol: string): IconName => symbols[symbol] ?? "sparkles";

export const widgetIcon = (type: SettingsWidgetType) => symbolIcon(widgetTypeSymbol(type));

export const alignmentIcon = (alignment: SettingsAlignment): IconName => `align-${alignment}`;
