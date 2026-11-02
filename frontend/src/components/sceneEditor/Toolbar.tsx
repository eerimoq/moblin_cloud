import { createMemo, createSignal, For, Show } from "solid-js";
import Icon, { type IconName } from "../Icon";
import { ask, choose } from "../Modal";
import { alignmentIcon, widgetIcon } from "./icons";
import { Popover, PopoverDivider, PopoverList, PopoverRow } from "./Popover";
import type { Editor } from "./store";
import {
  defaultSceneWidget,
  hasAlignment,
  sameId,
  type SettingsAlignment,
  type SettingsScene,
} from "../../data/moblinSettings";
import { SceneResolver } from "../../data/sceneResolver";
import { sceneEditorAlignedLayout } from "../../data/sceneEditorGeometry";
import type { Size, WidgetOutline } from "../../data/widgetOutlines";
import { t } from "../../i18n";

export function ToolButton(props: {
  icon: IconName;
  label: string;
  disabled?: boolean;
  red?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={props.label}
      title={props.label}
      disabled={props.disabled}
      class={`grid size-[44px] shrink-0 place-items-center disabled:opacity-35 ${
        props.red ? "text-phone-live" : "text-phone-tint"
      }`}
      onClick={() => props.onClick()}
    >
      <Icon name={props.icon} class="size-[22px]" />
    </button>
  );
}

const alignments: SettingsAlignment[][] = [
  ["TopLeft", "TopCenter", "TopRight"],
  ["LeftCenter", "Center", "RightCenter"],
  ["BottomLeft", "BottomCenter", "BottomRight"],
];

function Divider(props: { vertical: boolean }) {
  return (
    <div
      class={`shrink-0 bg-phone-border ${props.vertical ? "my-px h-px w-[32px]" : "mx-px h-[32px] w-px"}`}
    />
  );
}

function PopoverButton(props: {
  icon: IconName;
  label: string;
  red?: boolean;
  disabled?: boolean;
  vertical: boolean;
  onOpen: (offset: number) => void;
}) {
  let element: HTMLDivElement | undefined;
  return (
    <div ref={(node) => (element = node)} class="shrink-0">
      <ToolButton
        icon={props.icon}
        label={props.label}
        red={props.red}
        disabled={props.disabled}
        onClick={() =>
          props.onOpen(
            props.vertical
              ? (element?.offsetTop ?? 0) - (element?.parentElement?.scrollTop ?? 0)
              : (element?.offsetLeft ?? 0) - (element?.parentElement?.scrollLeft ?? 0),
          )
        }
      />
    </div>
  );
}

export default function Toolbar(props: {
  editor: Editor;
  scene: SettingsScene | undefined;
  selected: WidgetOutline | undefined;
  outlines: WidgetOutline[];
  canvas: Size;
  vertical: boolean;
  width: number;
  height: number;
  onClose: () => void;
  onCreateScene: () => void;
  onCreateWidget: () => void;
}) {
  const [open, setOpen] = createSignal<"scenes" | "add" | "alignment" | null>(null);
  const [popoverOffset, setPopoverOffset] = createSignal(0);
  const close = () => setOpen(null);
  const openAt = (kind: "scenes" | "add" | "alignment") => (offset: number) => {
    setPopoverOffset(
      Math.max(4, Math.min(offset, (props.vertical ? props.height : props.width) - 240)),
    );
    setOpen(kind);
  };
  const resolver = createMemo(
    () => new SceneResolver(props.editor.state.scenes, props.editor.state.widgets),
  );
  const addable = createMemo(() => (props.scene ? resolver().addableWidgets(props.scene) : []));

  const drawOrderTarget = (up: boolean) => {
    const scene = props.scene;
    const selected = props.selected;
    if (!scene || !selected) {
      return null;
    }
    const index = scene.widgets.findIndex(
      (sceneWidget) => sceneWidget.id === selected.sceneWidget.id,
    );
    if (index < 0) {
      return null;
    }
    const shown = new Set(
      props.outlines.filter((outline) => outline.inScene).map((outline) => outline.id),
    );
    const step = up ? 1 : -1;
    let target = index + step;
    while (target >= 0 && target < scene.widgets.length && !shown.has(scene.widgets[target]!.id)) {
      target += step;
    }
    return target >= 0 && target < scene.widgets.length ? { index, target } : null;
  };
  const moveInDrawOrder = (up: boolean) => {
    const move = drawOrderTarget(up);
    const scene = props.scene;
    if (!move || !scene) {
      return;
    }
    props.editor.editScene(scene.id, (draft) => {
      const [sceneWidget] = draft.widgets.splice(move.index, 1);
      draft.widgets.splice(move.target, 0, sceneWidget!);
    });
  };

  const setAlignment = (alignment: SettingsAlignment) => {
    const selected = props.selected;
    const scene = props.scene;
    if (!selected || !scene) {
      return;
    }
    props.editor.editScene(scene.id, (draft) => {
      const sceneWidget = draft.widgets.find(
        (candidate) => candidate.id === selected.sceneWidget.id,
      );
      if (sceneWidget) {
        Object.assign(
          sceneWidget,
          sceneEditorAlignedLayout(sceneWidget, alignment, selected.rect, props.canvas),
        );
      }
    });
  };

  const removeWidget = async () => {
    const selected = props.selected;
    const scene = props.scene;
    if (!selected || !scene) {
      return;
    }
    const picked = await choose(selected.widget.name, [
      t("Remove widget from scene"),
      t("Delete widget"),
    ]);
    if (picked === 0) {
      props.editor.editScene(scene.id, (draft) => {
        draft.widgets = draft.widgets.filter(
          (candidate) => candidate.id !== selected.sceneWidget.id,
        );
      });
      props.editor.set("selection", null);
    } else if (picked === 1) {
      props.editor.set("selection", null);
      props.editor.removeWidget(selected.widget.id);
    }
  };

  const deleteScene = async () => {
    const scene = props.scene;
    if (!scene) {
      return;
    }
    if (await ask(t("Delete scene {name}?", { name: scene.name }), t("Delete"))) {
      props.editor.set("selection", null);
      props.editor.removeScene(scene.id);
    }
  };

  const pickScene = (scene: SettingsScene) => {
    const widgetId = props.scene?.widgets.find(
      (candidate) => candidate.id === props.editor.state.selection,
    )?.widgetId;
    props.editor.set(
      "selection",
      scene.widgets.find((candidate) => sameId(candidate.widgetId, widgetId))?.id ?? null,
    );
    props.editor.selectScene(scene.id);
    close();
  };

  const addWidget = (widgetId: string) => {
    const scene = props.scene;
    const widget = props.editor.state.widgets.find((candidate) => sameId(candidate.id, widgetId));
    if (!scene || !widget) {
      return;
    }
    const sceneWidget = defaultSceneWidget(widget.id, widget.type, widget.scoreboard.type);
    props.editor.editScene(scene.id, (draft) => {
      draft.widgets.push(sceneWidget);
    });
    props.editor.set("selection", sceneWidget.id);
    close();
  };

  const toggleSettings = () => props.editor.set("showingSettings", (showing) => !showing);
  const side = () => (props.vertical ? "left" : "above");

  return (
    <div class={`relative shrink-0 ${props.vertical ? "h-full w-[52px]" : "h-[52px] w-full"}`}>
      <div
        class={`flex h-full w-full items-center gap-[2px] [scrollbar-width:none] ${
          props.vertical ? "flex-col overflow-y-auto" : "overflow-x-auto"
        }`}
      >
        <ToolButton icon="close" label={t("Close")} onClick={() => props.onClose()} />
        <Divider vertical={props.vertical} />
        <PopoverButton
          icon="photo-on-rectangle"
          label={t("Scenes")}
          vertical={props.vertical}
          onOpen={openAt("scenes")}
        />
        <Show when={props.scene}>
          {(scene) => (
            <Show
              when={props.selected}
              fallback={
                <>
                  <PopoverButton
                    icon="plus"
                    label={t("Add widget")}
                    vertical={props.vertical}
                    onOpen={openAt("add")}
                  />
                  <ToolButton icon="sliders" label={t("Scene settings")} onClick={toggleSettings} />
                  <ToolButton
                    icon="trash"
                    label={t("Delete scene")}
                    red
                    onClick={() => void deleteScene()}
                  />
                </>
              }
            >
              {(selected) => (
                <>
                  <Divider vertical={props.vertical} />
                  <ToolButton
                    icon={selected().widget.enabled ? "eye" : "eye-off"}
                    label={selected().widget.enabled ? t("Disable widget") : t("Enable widget")}
                    onClick={() =>
                      props.editor.editWidget(selected().widget.id, (draft) => {
                        draft.enabled = !draft.enabled;
                      })
                    }
                  />
                  <Show when={hasAlignment(selected().widget.type)}>
                    <PopoverButton
                      icon={alignmentIcon(selected().sceneWidget.alignment)}
                      label={t("Alignment")}
                      vertical={props.vertical}
                      onOpen={openAt("alignment")}
                    />
                  </Show>
                  <ToolButton
                    icon="layers-up"
                    label={t("Move up")}
                    disabled={drawOrderTarget(true) === null}
                    onClick={() => moveInDrawOrder(true)}
                  />
                  <ToolButton
                    icon="layers-down"
                    label={t("Move down")}
                    disabled={drawOrderTarget(false) === null}
                    onClick={() => moveInDrawOrder(false)}
                  />
                  <ToolButton
                    icon="sliders"
                    label={t("Widget settings")}
                    onClick={toggleSettings}
                  />
                  <ToolButton
                    icon="trash"
                    label={t("Remove widget")}
                    red
                    onClick={() => void removeWidget()}
                  />
                  <span class="hidden">{scene().id}</span>
                </>
              )}
            </Show>
          )}
        </Show>
      </div>
      <div
        class={`absolute ${props.vertical ? "right-[48px]" : "bottom-[48px]"}`}
        style={props.vertical ? { top: `${popoverOffset()}px` } : { left: `${popoverOffset()}px` }}
      >
        <Popover open={open() === "scenes"} side={side()} onClose={close}>
          <PopoverList>
            <PopoverRow
              icon="plus-square-dashed"
              text={t("Create scene")}
              tinted
              onClick={() => {
                close();
                props.onCreateScene();
              }}
            />
            <PopoverDivider />
            <For each={props.editor.state.scenes}>
              {(scene) => (
                <PopoverRow
                  icon={scene.enabled ? "photo-on-rectangle" : "eye-off"}
                  text={scene.name}
                  checked={props.scene !== undefined && sameId(scene.id, props.scene.id)}
                  onClick={() => pickScene(scene)}
                />
              )}
            </For>
          </PopoverList>
        </Popover>
        <Popover open={open() === "add"} side={side()} onClose={close}>
          <PopoverList>
            <PopoverRow
              icon="plus-square-dashed"
              text={t("Create widget")}
              tinted
              onClick={() => {
                close();
                props.onCreateWidget();
              }}
            />
            <PopoverDivider />
            <For each={addable()}>
              {(widget) => (
                <PopoverRow
                  icon={widgetIcon(widget.type)}
                  text={widget.name}
                  onClick={() => addWidget(widget.id)}
                />
              )}
            </For>
          </PopoverList>
        </Popover>
        <Popover open={open() === "alignment"} side={side()} onClose={close}>
          <Show when={props.selected}>
            {(selected) => (
              <div class="flex flex-col p-[10px]">
                <For each={alignments}>
                  {(row) => (
                    <div class="flex">
                      <For each={row}>
                        {(alignment) => (
                          <button
                            type="button"
                            aria-label={alignment}
                            class="grid size-[44px] place-items-center text-phone-tint"
                            onClick={() => setAlignment(alignment)}
                          >
                            <Icon
                              name={
                                selected().sceneWidget.alignment === alignment
                                  ? "square-fill"
                                  : "square"
                              }
                              class="size-[28px]"
                            />
                          </button>
                        )}
                      </For>
                    </div>
                  )}
                </For>
              </div>
            )}
          </Show>
        </Popover>
      </div>
    </div>
  );
}
