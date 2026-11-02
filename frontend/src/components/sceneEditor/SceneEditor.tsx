import { createEffect, createMemo, createSignal, Show, untrack } from "solid-js";
import Icon from "../Icon";
import Canvas from "./Canvas";
import { SceneSettings } from "./settings/SceneSettings";
import { SceneWidgetSettings } from "./settings/SceneWidgetSettings";
import { createStack, NavigationView } from "./Stack";
import type { Editor } from "./store";
import Toolbar from "./Toolbar";
import { SceneWizard } from "./wizards/SceneWizard";
import { EffectWizard } from "./wizards/EffectWizard";
import { WidgetWizard } from "./wizards/WidgetWizard";
import type { DeviceSettings, DeviceState } from "../../data/account";
import { clamped, sceneEditorLayout } from "../../data/sceneEditorGeometry";
import { SceneResolver } from "../../data/sceneResolver";
import {
  widgetOutlines,
  type Rect,
  type Size,
  type WidgetOutline,
} from "../../data/widgetOutlines";
import { t } from "../../i18n";

export const settingsHalfWidth = 350;
const toolbarWidth = 52;

export default function SceneEditor(props: {
  editor: Editor;
  preview: string | null;
  state: DeviceState;
  settings: DeviceSettings | null;
  width: number;
  height: number;
  onClose: () => void;
}) {
  const editor = props.editor;
  const canvas = (): Size => editor.state.options?.canvasSize ?? { width: 1920, height: 1080 };
  const scene = editor.scene;
  const resolver = createMemo(() => new SceneResolver(editor.state.scenes, editor.state.widgets));
  const outlines = createMemo(() => widgetOutlines(scene(), resolver(), canvas()));
  const selected = createMemo(() =>
    outlines().find((outline) => outline.id === editor.state.selection && outline.inScene),
  );
  const landscape = () => props.width > props.height;
  const panelHeight = () => props.height * 0.45;
  const area = (): Size =>
    landscape()
      ? {
          width:
            props.width - toolbarWidth - (editor.state.showingSettings ? settingsHalfWidth : 0),
          height: props.height,
        }
      : {
          width: props.width,
          height: props.height - toolbarWidth - (editor.state.showingSettings ? panelHeight() : 0),
        };
  const fit = createMemo((): Size => {
    const available = { width: area().width - 16, height: area().height - 16 };
    const size = canvas();
    if (available.width <= 0 || available.height <= 0 || size.width <= 0 || size.height <= 0) {
      return { width: 0, height: 0 };
    }
    const scale = Math.min(available.width / size.width, available.height / size.height);
    return { width: size.width * scale, height: size.height * scale };
  });

  createEffect(() => {
    editor.set("preview", props.preview);
    editor.set("portrait", canvas().height > canvas().width);
  });
  createEffect(() => {
    editor.set("currentMic", props.state.mic ?? null);
  });
  createEffect(() => {
    editor.set(
      "macros",
      (props.state.macros ?? []).map((macro) => ({ id: macro.id, name: macro.name })),
    );
  });

  const [wizard, setWizard] = createSignal<"scene" | "widget" | null>(null);

  const onMove = (outline: WidgetOutline, rect: Rect) => {
    const edited = scene();
    if (!edited) {
      return;
    }
    editor.editScene(edited.id, (draft) => {
      const sceneWidget = draft.widgets.find(
        (candidate) => candidate.id === outline.sceneWidget.id,
      );
      if (sceneWidget) {
        Object.assign(sceneWidget, sceneEditorLayout(sceneWidget, rect, canvas()));
      }
    });
  };
  const onResize = (outline: WidgetOutline, size: number) => {
    const edited = scene();
    if (!edited) {
      return;
    }
    editor.editScene(edited.id, (draft) => {
      const sceneWidget = draft.widgets.find(
        (candidate) => candidate.id === outline.sceneWidget.id,
      );
      if (sceneWidget) {
        sceneWidget.size = clamped(size, 1, 100);
      }
    });
  };

  const previewSource = () =>
    props.preview
      ? `data:image/${props.preview.startsWith("iVBOR") ? "png" : "jpeg"};base64,${props.preview}`
      : undefined;

  const panelKey = createMemo(() => selected()?.sceneWidget.id ?? scene()?.id ?? "");
  const panel = createMemo(() => {
    panelKey();
    return untrack(() => {
      const stack = createStack();
      const widgetOutline = selected();
      const edited = scene();
      const root = widgetOutline
        ? {
            title: widgetOutline.widget.name,
            render: () => (
              <SceneWidgetSettings
                editor={editor}
                stack={stack}
                scene={edited!}
                sceneWidgetId={widgetOutline.sceneWidget.id}
                widgetId={widgetOutline.widget.id}
                canvas={canvas()}
                settings={props.settings}
                onCreateWidget={() => setWizard("widget")}
              />
            ),
          }
        : edited
          ? {
              title: t("Scene"),
              render: () => (
                <SceneSettings
                  editor={editor}
                  stack={stack}
                  sceneId={edited.id}
                  canvas={canvas()}
                  settings={props.settings}
                  onCreateWidget={() => setWizard("widget")}
                />
              ),
            }
          : {
              title: "",
              render: () => (
                <p class="grid flex-1 place-items-center text-[17px] text-phone-border">
                  {t("No widget selected")}
                </p>
              ),
            };
      return { stack, root };
    });
  });

  return (
    <div class={`flex h-full w-full bg-black text-white ${landscape() ? "" : "flex-col"}`}>
      <div
        class="relative min-h-0 min-w-0 flex-1"
        style={{ width: `${area().width}px`, height: `${area().height}px` }}
      >
        <div
          class="absolute border border-phone-border/50"
          style={{
            width: `${fit().width}px`,
            height: `${fit().height}px`,
            left: `${(area().width - fit().width) / 2}px`,
            top: `${(area().height - fit().height) / 2}px`,
          }}
        >
          <Show when={previewSource()}>
            {(source) => (
              <img
                src={source()}
                alt={t("Stream preview")}
                class="absolute inset-0 size-full object-fill"
              />
            )}
          </Show>
          <Show when={scene() && !scene()!.enabled}>
            <div class="absolute inset-0 flex items-end justify-end bg-black p-[4px] text-[13px] text-white">
              {t("Disabled scene")}
            </div>
          </Show>
          <Show when={fit().width > 0}>
            <Canvas
              outlines={outlines()}
              canvas={canvas()}
              size={fit()}
              selected={editor.state.selection}
              onSelect={(id) => editor.set("selection", id)}
              onMove={onMove}
              onResize={onResize}
            />
          </Show>
        </div>
        <Show when={editor.state.error}>
          {(error) => (
            <div class="absolute right-[12px] bottom-[12px] left-[12px] flex items-start gap-2 rounded-[10px] bg-phone-live/90 px-3 py-2 text-[13px] font-semibold text-white">
              <span class="flex-1">{error()}</span>
              <button
                type="button"
                aria-label={t("Close")}
                onClick={() => editor.set("error", null)}
              >
                <Icon name="close" class="size-[14px]" />
              </button>
            </div>
          )}
        </Show>
        <Show when={editor.state.loading && !editor.state.loaded}>
          <p class="absolute top-[12px] left-[12px] text-[13px] text-phone-border">
            {t("Loading…")}
          </p>
        </Show>
      </div>
      <Show when={editor.state.showingSettings}>
        <div
          class={`relative shrink-0 overflow-hidden ${landscape() ? "h-full" : "w-full"}`}
          style={
            landscape() ? { width: `${settingsHalfWidth}px` } : { height: `${panelHeight()}px` }
          }
        >
          <NavigationView stack={panel().stack} root={panel().root} />
          <button
            type="button"
            aria-label={t("Close")}
            class="absolute top-[7px] right-[7px] grid size-[30px] place-items-center rounded-full border border-phone-border bg-phone-screen text-phone-border"
            onClick={() => editor.set("showingSettings", false)}
          >
            <Icon name="close" class="size-[14px]" />
          </button>
        </div>
      </Show>
      <Toolbar
        editor={editor}
        scene={scene()}
        selected={selected()}
        outlines={outlines()}
        canvas={canvas()}
        vertical={landscape()}
        width={props.width}
        height={props.height}
        onClose={() => props.onClose()}
        onCreateScene={() => setWizard("scene")}
        onCreateWidget={() => setWizard("widget")}
      />
      <Show when={wizard() === "scene"}>
        <SceneWizard editor={editor} onClose={() => setWizard(null)} />
      </Show>
      <Show when={wizard() === "widget"}>
        <WidgetWizard
          editor={editor}
          sceneId={scene()?.id ?? null}
          onClose={() => setWizard(null)}
        />
      </Show>
      <Show when={editor.state.effectWizard}>
        {(widgetId) => (
          <EffectWizard
            editor={editor}
            widgetId={widgetId()}
            onClose={() => editor.set("effectWizard", null)}
          />
        )}
      </Show>
    </div>
  );
}
