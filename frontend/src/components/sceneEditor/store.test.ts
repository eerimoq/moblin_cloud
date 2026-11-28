import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { flush } from "solid-js";
import { createEditor } from "./store";
import type { Control } from "../../data/account";

const mainId = "1A2B3C4D-5E6F-4A7B-8C9D-0E1F2A3B4C5D";
const brbId = "2B3C4D5E-6F7A-4B8C-9D0E-1F2A3B4C5D6E";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

test("a read-only editor shows the scenes but changes and sends nothing", async () => {
  const sent: Control[] = [];
  const answers: Record<string, unknown> = {
    getScenes: {
      scenes: [
        { id: mainId, name: "Main", widgets: [] },
        { id: brbId, name: "BRB", widgets: [] },
      ],
    },
    getWidgets: { widgets: [] },
    getVideoSources: { videoSources: [] },
    getAlertsMediaGallery: { gallery: {} },
    getEditorOptions: { options: { scoreboardPlayers: [] } },
  };
  const request = async <T>(control: Control): Promise<T> => {
    sent.push(control);
    return answers[Object.keys(control)[0]] as T;
  };
  const control = async (control: Control) => {
    sent.push(control);
  };
  const editor = createEditor(
    request,
    control,
    () => mainId,
    () => false,
    () => true,
  );
  await vi.runAllTimersAsync();
  expect(editor.state.scenes.map((scene) => scene.name)).toEqual(["Main", "BRB"]);
  editor.editScene(mainId, (draft) => {
    draft.name = "Renamed";
  });
  editor.removeScene(brbId);
  editor.selectScene(brbId);
  await editor.flush();
  await vi.runAllTimersAsync();
  expect(editor.state.scenes.map((scene) => scene.name)).toEqual(["Main", "BRB"]);
  expect(editor.state.sceneId).toBe(brbId);
  expect(sent.map((control) => Object.keys(control)[0])).toEqual([
    "getScenes",
    "getWidgets",
    "getVideoSources",
    "getAlertsMediaGallery",
    "getEditorOptions",
  ]);
  editor.dispose();
});

test("an enabled scene is switched to once the device has it enabled", async () => {
  const sent: Control[] = [];
  const answers: Record<string, unknown> = {
    getScenes: {
      scenes: [
        { id: mainId, name: "Main", widgets: [] },
        { id: brbId, name: "BRB", enabled: false, widgets: [] },
      ],
    },
    getWidgets: { widgets: [] },
    getVideoSources: { videoSources: [] },
    getAlertsMediaGallery: { gallery: {} },
    getEditorOptions: { options: { scoreboardPlayers: [] } },
  };
  const request = async <T>(control: Control): Promise<T> => {
    sent.push(control);
    return (answers[Object.keys(control)[0]!] ?? {}) as T;
  };
  const control = async (control: Control) => {
    sent.push(control);
  };
  const editor = createEditor(
    request,
    control,
    () => mainId,
    () => false,
    () => false,
  );
  await vi.runAllTimersAsync();
  sent.length = 0;
  editor.editScene(brbId, (draft) => {
    draft.enabled = true;
  });
  editor.selectScene(brbId);
  flush();
  expect(editor.state.sceneId).toBe(brbId);
  await editor.flush();
  await vi.runAllTimersAsync();
  expect(sent.map((control) => Object.keys(control)[0])).toEqual(["updateScene", "setScene"]);
  expect(editor.state.sceneId).toBeNull();
  editor.dispose();
});
