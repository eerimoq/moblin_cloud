import { describe, expect, test } from "vitest";
import {
  defaultEffect,
  defaultScene,
  defaultWidget,
  encodeEffect,
  encodeScene,
  encodeWidget,
  makeUniqueName,
  normalizeScene,
  normalizeWidget,
  widgetTypes,
} from "./moblinSettings";

const widgetId = "6F1C3A2B-0D4E-4F5A-8B7C-9D0E1F2A3B4C";
const sceneId = "1A2B3C4D-5E6F-4A7B-8C9D-0E1F2A3B4C5D";

const roundTrip = <T>(value: T) => JSON.parse(JSON.stringify(value)) as Record<string, unknown>;

describe("normalizeWidget", () => {
  test.each(widgetTypes)("fills a bare %s widget with the defaults", (type) => {
    const widget = normalizeWidget({ id: widgetId, type, name: "Widget" });
    expect(widget.type).toBe(type);
    expect(widget.name).toBe("Widget");
    expect(Object.keys(widget).sort()).toEqual(
      Object.keys({ ...defaultWidget(widgetId, type), name: "Widget" }).sort(),
    );
  });

  test.each(widgetTypes)("normalizing a %s widget again changes nothing", (type) => {
    const widget = normalizeWidget({ id: widgetId, type });
    expect(normalizeWidget(roundTrip(widget))).toEqual(widget);
  });

  test.each(widgetTypes)("normalizing a default %s widget changes nothing", (type) => {
    const widget = defaultWidget(widgetId, type);
    expect(normalizeWidget(roundTrip(widget))).toEqual(widget);
  });

  test("ids derived for missing UUIDs are the same every time", () => {
    const raw = {
      id: widgetId,
      type: "Wheel of luck",
      wheelOfLuck: { options: [{ text: "One" }, { text: "Two" }] },
      effects: [{}],
    };
    const first = normalizeWidget(roundTrip(raw));
    const second = normalizeWidget(roundTrip(raw));
    expect(first).toEqual(second);
    const [one, two] = first.wheelOfLuck.options;
    expect(one!.text).toBe("One");
    expect(one!.id).not.toBe(two!.id);
    expect(first.effects).toHaveLength(1);
  });

  test("keeps fields it does not know", () => {
    const widget = normalizeWidget({
      id: widgetId,
      type: "Text",
      fromNewerMoblin: 1,
      text: { formatString: "{time}", alsoNewer: true },
    });
    expect(widget.text.formatString).toBe("{time}");
    expect((widget as unknown as Record<string, unknown>).fromNewerMoblin).toBe(1);
    expect((widget.text as unknown as Record<string, unknown>).alsoNewer).toBe(true);
  });

  test("a scene widget's scene is optional", () => {
    const sceneOf = (scene?: unknown) =>
      normalizeWidget({ id: widgetId, type: "Scene", scene }).scene.sceneId;
    expect(defaultWidget(widgetId, "Scene").scene.sceneId).toBeNull();
    expect(sceneOf()).toBeNull();
    expect(sceneOf({})).toBeNull();
    expect(sceneOf({ sceneId: null })).toBeNull();
    expect(sceneOf({ sceneId })).toBe(sceneId);
    expect(encodeWidget(defaultWidget(widgetId, "Scene")).scene).toEqual({ sceneId: null });
  });

  test("an unknown type is read as a text widget", () => {
    expect(normalizeWidget({ id: widgetId, type: "Hologram" }).type).toBe("Text");
  });
});

describe("normalizeScene", () => {
  test("fills a bare scene with the defaults", () => {
    const scene = normalizeScene({ id: sceneId, name: "Main" });
    expect(scene).toEqual({ ...defaultScene(sceneId), name: "Main", cameraPosition: "Back" });
  });

  test("normalizing again changes nothing", () => {
    const scene = normalizeScene({
      id: sceneId,
      enabled: false,
      widgets: [{ widgetId, x: 10 }, { widgetId }],
    });
    expect(normalizeScene(roundTrip(scene))).toEqual(scene);
    expect(scene.enabled).toBe(false);
    expect(scene.widgets.map((widget) => widget.x)).toEqual([10, 0]);
    expect(scene.widgets[0]!.id).not.toBe(scene.widgets[1]!.id);
  });
});

test("makeUniqueName", () => {
  expect(makeUniqueName("Text", [])).toBe("Text");
  expect(makeUniqueName("Text", ["Text"])).toBe("Text 1");
  expect(makeUniqueName("Text", ["Text", "Text 1"])).toBe("Text 2");
});

describe("encodeWidget", () => {
  test.each(widgetTypes)("sends only the settings of a %s widget, like Moblin", (type) => {
    const widget = defaultWidget(widgetId, type);
    const encoded = encodeWidget(widget);
    const subObjects = Object.keys(encoded).filter(
      (key) => !["name", "id", "type", "enabled", "effects"].includes(key),
    );
    expect(subObjects.length).toBeLessThanOrEqual(1);
    if (type === "Image") {
      expect(subObjects).toEqual([]);
    } else {
      expect(subObjects).toHaveLength(1);
      expect(encoded[subObjects[0]!]).toBeDefined();
    }
    expect(normalizeWidget(roundTrip(encoded))).toEqual(widget);
  });

  test("keeps the settings of the widget's type and drops the others", () => {
    const widget = defaultWidget(widgetId, "Browser");
    widget.browser.url = "https://example.com";
    widget.text.formatString = "{time}";
    const encoded = encodeWidget(widget);
    expect(encoded.browser).toEqual(widget.browser);
    expect(encoded.text).toBeUndefined();
    expect(normalizeWidget(roundTrip(encoded)).text.formatString).toBe("{shortTime}");
  });

  test("keeps fields it does not know", () => {
    const widget = normalizeWidget({ id: widgetId, type: "Text", fromNewerMoblin: 1 });
    expect(encodeWidget(widget).fromNewerMoblin).toBe(1);
  });

  test("sends only the camera id of the video source's position", () => {
    const widget = defaultWidget(widgetId, "Video source");
    widget.videoSource.cameraPosition = "RTMP";
    widget.videoSource.rtmpCameraId = "rtmp";
    widget.videoSource.backCameraId = "back";
    const encoded = encodeWidget(widget).videoSource as Record<string, unknown>;
    expect(encoded.cameraPosition).toBe("RTMP");
    expect(encoded.rtmpCameraId).toBe("rtmp");
    expect(encoded.backCameraId).toBeUndefined();
    expect(encoded.externalCameraName).toBeUndefined();
    expect(encoded.rotation).toBe(widget.videoSource.rotation);
    const vTuber = defaultWidget(widgetId, "VTuber");
    vTuber.vTuber.cameraPosition = "External";
    expect(Object.keys(encodeWidget(vTuber).vTuber as object)).toEqual(
      expect.arrayContaining(["externalCameraId", "externalCameraName", "modelName"]),
    );
    expect(encodeWidget(vTuber).vTuber).not.toHaveProperty("frontCameraId");
    const pngTuber = defaultWidget(widgetId, "PNGTuber");
    pngTuber.pngTuber.cameraPosition = "None";
    expect(encodeWidget(pngTuber).pngTuber).not.toHaveProperty("backCameraId");
  });

  test("sends only the settings of each effect's type", () => {
    const widget = defaultWidget(widgetId, "Text");
    widget.effects = [defaultEffect("E1"), { ...defaultEffect("E2"), type: "grayScale" }];
    widget.effects[0]!.type = "opacity";
    const [opacity, grayScale] = encodeWidget(widget).effects as Record<string, unknown>[];
    expect(Object.keys(opacity!).sort()).toEqual(["enabled", "id", "opacity", "type"]);
    expect(Object.keys(grayScale!).sort()).toEqual(["enabled", "id", "type"]);
    expect(encodeEffect(widget.effects[0]!)).toEqual(opacity);
  });
});

describe("encodeScene", () => {
  test("sends only the camera id of the scene's position", () => {
    const scene = defaultScene(sceneId);
    scene.cameraPosition = "Front";
    scene.frontCameraId = "front";
    const encoded = encodeScene(scene);
    expect(encoded.frontCameraId).toBe("front");
    expect(encoded.backCameraId).toBeUndefined();
    expect(encoded.widgets).toEqual(scene.widgets);
    expect(normalizeScene(roundTrip(encoded))).toEqual(scene);
  });
});
