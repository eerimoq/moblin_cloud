import { describe, expect, test } from "vitest";
import {
  defaultScene,
  defaultWidget,
  newUuid,
  type SettingsScene,
  type SettingsWidget,
} from "./moblinSettings";
import { firstWidgets, resolvedEnabled, SceneResolver, type ResolvedScene } from "./sceneResolver";

const makeWidget = (name: string, enabled = true): SettingsWidget => ({
  ...defaultWidget(newUuid(), "Text"),
  name,
  enabled,
});

const makeSceneWidget = (name: string, scene: SettingsScene, enabled = true): SettingsWidget => {
  const widget = { ...defaultWidget(newUuid(), "Scene"), name, enabled };
  widget.scene.sceneId = scene.id;
  return widget;
};

const sceneWidgetOf = (widget: SettingsWidget) => ({
  id: newUuid(),
  widgetId: widget.id,
  x: 0,
  y: 0,
  size: 100,
  alignment: "TopLeft" as const,
  positioningLock: false,
});

const makeScene = (name: string, widgets: SettingsWidget[]): SettingsScene => ({
  ...defaultScene(newUuid()),
  name,
  widgets: widgets.map(sceneWidgetOf),
});

const drawnWidgets = (resolved: ResolvedScene) =>
  resolved.widgets.filter((widget) => widget.first && resolvedEnabled(widget));

const widgetNames = (resolved: { widget: SettingsWidget }[]) =>
  resolved.map((item) => item.widget.name);

describe("SceneResolverSuite", () => {
  test("nestedScene", () => {
    const text = makeWidget("Text");
    const image = makeWidget("Image");
    const nested = makeScene("Nested", [image]);
    const sceneWidget = makeSceneWidget("Scene", nested);
    const main = makeScene("Main", [text, sceneWidget]);
    const resolver = new SceneResolver([main, nested], [text, image, sceneWidget]);
    const resolved = resolver.resolve(main);
    expect(widgetNames(resolved.widgets)).toEqual(["Text", "Scene", "Image"]);
    expect(resolved.widgets.map((widget) => widget.scene.name)).toEqual(["Main", "Main", "Nested"]);
    expect(resolved.sceneIds).toEqual([main.id, nested.id]);
  });

  test("disabledSceneWidget", () => {
    const image = makeWidget("Image");
    const nested = makeScene("Nested", [image]);
    const sceneWidget = makeSceneWidget("Scene", nested, false);
    const main = makeScene("Main", [sceneWidget]);
    const resolver = new SceneResolver([main, nested], [image, sceneWidget]);
    expect(widgetNames(resolver.resolve(main).widgets)).toEqual(["Scene"]);
    expect(drawnWidgets(resolver.resolve(main))).toEqual([]);
    const resolved = resolver.resolve(main, true);
    expect(widgetNames(resolved.widgets)).toEqual(["Scene", "Image"]);
    expect(resolved.widgets.map(resolvedEnabled)).toEqual([false, false]);
    expect(drawnWidgets(resolved)).toEqual([]);
  });

  test("disabledNestedSceneWidgetDisablesItsContent", () => {
    const image = makeWidget("Image");
    const innerScene = makeScene("Inner", [image]);
    const innerWidget = makeSceneWidget("Inner scene", innerScene, false);
    const outerScene = makeScene("Outer", [innerWidget]);
    const outerWidget = makeSceneWidget("Outer scene", outerScene);
    const main = makeScene("Main", [outerWidget]);
    const resolver = new SceneResolver(
      [main, outerScene, innerScene],
      [image, innerWidget, outerWidget],
    );
    const resolved = resolver.resolve(main, true);
    expect(widgetNames(resolved.widgets)).toEqual(["Outer scene", "Inner scene", "Image"]);
    expect(resolved.widgets.map(resolvedEnabled)).toEqual([true, false, false]);
    expect(widgetNames(drawnWidgets(resolved))).toEqual(["Outer scene"]);
  });

  test("duplicatedWidgetIsDrawnOnce", () => {
    const text = makeWidget("Text");
    const nested = makeScene("Nested", [text]);
    const sceneWidget = makeSceneWidget("Scene", nested);
    const main = makeScene("Main", [text, sceneWidget]);
    const resolver = new SceneResolver([main, nested], [text, sceneWidget]);
    const resolved = resolver.resolve(main);
    expect(resolved.widgets.map((widget) => widget.first)).toEqual([true, true, false]);
    expect(firstWidgets(resolved).map((widget) => widget.scene.name)).toEqual(["Main", "Main"]);
  });

  test("cyclicScenes", () => {
    const first = makeScene("First", []);
    const second = makeScene("Second", []);
    const firstWidget = makeSceneWidget("First", first);
    const secondWidget = makeSceneWidget("Second", second);
    first.widgets = [sceneWidgetOf(secondWidget)];
    second.widgets = [sceneWidgetOf(firstWidget)];
    const resolver = new SceneResolver([first, second], [firstWidget, secondWidget]);
    expect(widgetNames(resolver.resolve(first).widgets)).toEqual(["Second", "First"]);
  });

  test("addableWidgets", () => {
    const text = makeWidget("Text");
    const image = makeWidget("Image");
    const nested = makeScene("Nested", [image]);
    const sceneWidget = makeSceneWidget("Scene", nested);
    const main = makeScene("Main", [text, sceneWidget]);
    const other = makeScene("Other", []);
    const resolver = new SceneResolver([main, nested, other], [text, image, sceneWidget]);
    expect(resolver.addableWidgets(main)).toEqual([]);
    expect(resolver.addableWidgets(nested)).toEqual([]);
    expect(resolver.addableWidgets(other).map((widget) => widget.name)).toEqual([
      "Text",
      "Image",
      "Scene",
    ]);
  });

  test("addableWidgetsWithOldDuplicates", () => {
    const text = makeWidget("Text");
    const image = makeWidget("Image");
    const main = makeScene("Main", [text, text]);
    const resolver = new SceneResolver([main], [text, image]);
    expect(resolver.addableWidgets(main).map((widget) => widget.name)).toEqual(["Image"]);
  });

  test("sceneWidgetWithoutScene", () => {
    const sceneWidget = { ...defaultWidget(newUuid(), "Scene"), name: "Scene" };
    const main = makeScene("Main", [sceneWidget]);
    const resolved = new SceneResolver([main], [sceneWidget]).resolve(main);
    expect(widgetNames(resolved.widgets)).toEqual(["Scene"]);
    expect(resolved.sceneIds).toEqual([main.id]);
  });

  test("selectableScenes", () => {
    const text = makeWidget("Text");
    const image = makeWidget("Image");
    const withText = makeScene("With text", [text]);
    const withImage = makeScene("With image", [image]);
    const sceneWidget = makeSceneWidget("Scene", withImage);
    const main = makeScene("Main", [text, sceneWidget]);
    const resolver = new SceneResolver([main, withText, withImage], [text, image, sceneWidget]);
    expect(resolver.selectableScenes(sceneWidget).map((scene) => scene.name)).toEqual([
      "Main",
      "With image",
    ]);
  });
});
