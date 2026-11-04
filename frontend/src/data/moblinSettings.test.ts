import { describe, expect, test } from "vitest";
import {
  defaultScene,
  defaultWidget,
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
