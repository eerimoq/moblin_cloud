import { createMemo, createSignal, For, Show } from "solid-js";
import Icon, { type IconName } from "../Icon";
import { ask, choose } from "../Modal";
import { widgetIcon } from "./icons";
import { Popover, PopoverDivider, PopoverList, PopoverRow, PopoverTitle } from "./Popover";
import type { Editor } from "./store";
import { defaultSceneWidget, sameId, type SettingsScene } from "../../data/moblinSettings";
import { SceneResolver } from "../../data/sceneResolver";
import type { WidgetOutline } from "../../data/widgetOutlines";
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
      class={[
        "grid size-[44px] shrink-0 place-items-center disabled:opacity-35",
        props.red ? "text-phone-live" : "text-phone-tint",
      ]}
      onClick={() => props.onClick()}
    >
      <Icon name={props.icon} class="size-[22px]" />
    </button>
  );
}

function Divider(props: { vertical: boolean }) {
  return (
    <div
      class={[
        "shrink-0 bg-phone-border",
        props.vertical ? "my-px h-px w-[32px]" : "mx-px h-[32px] w-px",
      ]}
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
  vertical: boolean;
  width: number;
  height: number;
  onClose: () => void;
  onCreateScene: () => void;
  onCreateWidget: () => void;
}) {
  const [open, setOpen] = createSignal<"scenes" | "add" | "widgets" | null>(null);
  const [popoverOffset, setPopoverOffset] = createSignal(0);
  const close = () => setOpen(null);
  const openAt = (kind: "scenes" | "add" | "widgets") => (offset: number) => {
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
      props.editor.set((state) => {
        state.selection = null;
      });
    } else if (picked === 1) {
      props.editor.set((state) => {
        state.selection = null;
      });
      props.editor.removeWidget(selected.widget.id);
    }
  };

  const deleteScene = async () => {
    const scene = props.scene;
    if (!scene) {
      return;
    }
    if (await ask(t("Delete scene {name}?", { name: scene.name }), t("Delete"))) {
      props.editor.set((state) => {
        state.selection = null;
      });
      props.editor.removeScene(scene.id);
    }
  };

  const toggleSceneEnabled = () => {
    const scene = props.scene;
    if (!scene) {
      return;
    }
    props.editor.editScene(scene.id, (draft) => {
      draft.enabled = !draft.enabled;
    });
    props.editor.selectScene(scene.id);
  };

  const inScene = () => props.outlines.filter((outline) => outline.inScene);

  const pickScene = (scene: SettingsScene) => {
    const widgetId = props.scene?.widgets.find(
      (candidate) => candidate.id === props.editor.state.selection,
    )?.widgetId;
    props.editor.set((state) => {
      state.selection =
        scene.widgets.find((candidate) => sameId(candidate.widgetId, widgetId))?.id ?? null;
    });
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
    props.editor.set((state) => {
      state.selection = sceneWidget.id;
    });
    close();
  };

  const toggleSettings = () =>
    props.editor.set((state) => {
      state.showingSettings = !state.showingSettings;
    });
  const side = () => (props.vertical ? "left" : "above");

  return (
    <div class={["relative shrink-0", props.vertical ? "h-full w-[52px]" : "h-[52px] w-full"]}>
      <div
        class={[
          "flex h-full w-full items-center gap-[2px] [scrollbar-width:none]",
          props.vertical ? "flex-col overflow-y-auto" : "overflow-x-auto",
        ]}
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
                  <Show when={!props.editor.readOnly()}>
                    <ToolButton
                      icon={scene().enabled ? "eye" : "eye-off"}
                      label={scene().enabled ? t("Disable scene") : t("Enable scene")}
                      onClick={toggleSceneEnabled}
                    />
                    <PopoverButton
                      icon="plus"
                      label={t("Add widget")}
                      vertical={props.vertical}
                      onOpen={openAt("add")}
                    />
                  </Show>
                  <PopoverButton
                    icon="list"
                    label={t("Select widget")}
                    disabled={inScene().length === 0}
                    vertical={props.vertical}
                    onOpen={openAt("widgets")}
                  />
                  <ToolButton icon="sliders" label={t("Scene settings")} onClick={toggleSettings} />
                  <Show when={!props.editor.readOnly()}>
                    <ToolButton
                      icon="trash"
                      label={t("Delete scene")}
                      red
                      onClick={() => void deleteScene()}
                    />
                  </Show>
                </>
              }
            >
              {(selected) => (
                <Show
                  when={!props.editor.readOnly()}
                  fallback={
                    <>
                      <Divider vertical={props.vertical} />
                      <ToolButton
                        icon="sliders"
                        label={t("Widget settings")}
                        onClick={toggleSettings}
                      />
                    </>
                  }
                >
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
                </Show>
              )}
            </Show>
          )}
        </Show>
      </div>
      <div
        class={["absolute", props.vertical ? "right-[48px]" : "bottom-[48px]"]}
        style={props.vertical ? { top: `${popoverOffset()}px` } : { left: `${popoverOffset()}px` }}
      >
        <Popover open={open() === "scenes"} side={side()} onClose={close}>
          <PopoverList>
            <Show when={!props.editor.readOnly()}>
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
            </Show>
            <PopoverTitle text={t("Scenes")} />
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
            <PopoverTitle text={t("Widgets")} />
            <Show when={addable().length === 0}>
              <p class="p-[11px] text-[17px] text-phone-border">
                {t("All are already in this scene. Create a new one.")}
              </p>
            </Show>
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
        <Popover open={open() === "widgets"} side={side()} onClose={close}>
          <PopoverList>
            <PopoverTitle text={t("Widgets")} />
            <For each={inScene()}>
              {(outline) => (
                <PopoverRow
                  icon={widgetIcon(outline.widget.type)}
                  text={outline.widget.name}
                  onClick={() => {
                    props.editor.set((state) => {
                      state.selection = outline.id;
                    });
                    close();
                  }}
                />
              )}
            </For>
          </PopoverList>
        </Popover>
      </div>
    </div>
  );
}
