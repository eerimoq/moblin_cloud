import { createMemo, createSignal, Show } from "solid-js";
import Icon from "../../Icon";
import { LinkRow, Section, ToggleRow, ValueEditCompact } from "../Form";
import type { Stack } from "../Stack";
import type { Editor } from "../store";
import { SceneSettings } from "./SceneSettings";
import { WidgetSettings } from "./WidgetSettings";
import type { DeviceSettings } from "../../../data/account";
import {
  hasAlignment,
  hasPosition,
  hasSize,
  isHorizontalCenter,
  isHorizontalOrVerticalCenter,
  isVerticalCenter,
  mirrorPositionHorizontally,
  mirrorPositionVertically,
  sameId,
  widgetTypeName,
  type SettingsAlignment,
  type SettingsScene,
  type SettingsSceneWidget,
} from "../../../data/moblinSettings";
import type { Size } from "../../../data/widgetOutlines";
import { t } from "../../../i18n";

const [savedLayout, setSavedLayout] = createSignal<Omit<
  SettingsSceneWidget,
  "id" | "widgetId"
> | null>(null);

const alignments: SettingsAlignment[][] = [
  ["TopLeft", "TopCenter", "TopRight"],
  ["LeftCenter", "Center", "RightCenter"],
  ["BottomLeft", "BottomCenter", "BottomRight"],
];

export function SceneWidgetSettings(props: {
  editor: Editor;
  stack: Stack;
  scene: SettingsScene;
  sceneWidgetId: string;
  widgetId: string;
  canvas: Size;
  settings: DeviceSettings | null;
  onCreateWidget: () => void;
}) {
  const editor = props.editor;
  const numericInput = () => editor.state.options?.numericInput ?? false;
  const setNumericInput = (enabled: boolean) => {
    editor.set("options", "numericInput", enabled);
    void editor.send({ setNumericInput: { enabled } });
  };
  const scene = createMemo(() =>
    editor.state.scenes.find((candidate) => sameId(candidate.id, props.scene.id)),
  );
  const sceneWidget = createMemo(() =>
    scene()?.widgets.find((candidate) => candidate.id === props.sceneWidgetId),
  );
  const widget = createMemo(() =>
    editor.state.widgets.find((candidate) => sameId(candidate.id, props.widgetId)),
  );
  const edit = (change: (layout: SettingsSceneWidget) => void) =>
    editor.editScene(props.scene.id, (draft) => {
      const target = draft.widgets.find((candidate) => candidate.id === props.sceneWidgetId);
      if (target) {
        change(target);
      }
    });
  const horizontalIncrement = () => 100 / props.canvas.width;
  const verticalIncrement = () => 100 / props.canvas.height;
  const setYBasedOnXIfLocked = (layout: SettingsSceneWidget) => {
    if (layout.positioningLock) {
      layout.y = (layout.x * verticalIncrement()) / horizontalIncrement();
    }
  };
  const setXBasedOnYIfLocked = (layout: SettingsSceneWidget) => {
    if (layout.positioningLock) {
      layout.x = (layout.y * horizontalIncrement()) / verticalIncrement();
    }
  };

  const positionX = (layout: SettingsSceneWidget) => (
    <ValueEditCompact
      value={layout.x}
      minimum={0}
      maximum={100}
      increment={horizontalIncrement()}
      mirror={mirrorPositionHorizontally(layout.alignment)}
      numericInput={numericInput()}
      decrementIcon="arrow-left-circle"
      incrementIcon="arrow-right-circle"
      onChange={(value) =>
        edit((draft) => {
          draft.x = value;
          setYBasedOnXIfLocked(draft);
        })
      }
    />
  );
  const positionY = (layout: SettingsSceneWidget) => (
    <ValueEditCompact
      value={layout.y}
      minimum={0}
      maximum={100}
      increment={verticalIncrement()}
      mirror={mirrorPositionVertically(layout.alignment)}
      numericInput={numericInput()}
      decrementIcon="arrow-up-circle"
      incrementIcon="arrow-down-circle"
      onChange={(value) =>
        edit((draft) => {
          draft.y = value;
          setXBasedOnYIfLocked(draft);
        })
      }
    />
  );

  return (
    <Show when={sceneWidget() && widget() && { layout: sceneWidget()!, widget: widget()! }}>
      {(current) => (
        <>
          <Show
            when={
              hasAlignment(current().widget.type) ||
              hasPosition(current().widget.type) ||
              hasSize(current().widget.type)
            }
          >
            <Section
              header={t("Layout")}
              footer={t(
                "Use save/load layout to position a widget in the same place in multiple scenes. Alternatively, use a Scene widget to easily show the same widgets in multiple scenes.",
              )}
            >
              <div class="flex items-center gap-3 px-4 py-2">
                <div class="flex flex-1 flex-col gap-2">
                  <button
                    type="button"
                    class="rounded-[8px] bg-phone-separator px-3 py-1.5 text-[15px] text-phone-tint"
                    onClick={() => {
                      const { x, y, size, alignment, positioningLock } = current().layout;
                      setSavedLayout({ x, y, size, alignment, positioningLock });
                    }}
                  >
                    {t("Save layout")}
                  </button>
                  <button
                    type="button"
                    class="rounded-[8px] bg-phone-separator px-3 py-1.5 text-[15px] text-phone-tint disabled:opacity-35"
                    disabled={savedLayout() === null}
                    onClick={() => {
                      const saved = savedLayout();
                      if (saved) {
                        edit((draft) => Object.assign(draft, saved));
                      }
                    }}
                  >
                    {t("Load layout")}
                  </button>
                </div>
                <Show when={hasAlignment(current().widget.type)}>
                  <div class="h-[100px] w-px bg-phone-separator" />
                  <div class="flex flex-col gap-[5px]">
                    {alignments.map((row) => (
                      <div class="flex gap-[3px]">
                        {row.map((alignment) => (
                          <button
                            type="button"
                            aria-label={alignment}
                            class="text-phone-tint"
                            onClick={() =>
                              edit((draft) => {
                                draft.alignment = alignment;
                                if (isHorizontalOrVerticalCenter(alignment)) {
                                  draft.positioningLock = false;
                                }
                              })
                            }
                          >
                            <Icon
                              name={
                                current().layout.alignment === alignment ? "square-fill" : "square"
                              }
                              class="size-[26px]"
                            />
                          </button>
                        ))}
                      </div>
                    ))}
                  </div>
                </Show>
              </div>
              <Show when={hasPosition(current().widget.type)}>
                <Show
                  when={
                    !isHorizontalCenter(current().layout.alignment) &&
                    !isVerticalCenter(current().layout.alignment)
                  }
                  fallback={
                    <>
                      <Show when={!isHorizontalCenter(current().layout.alignment)}>
                        {positionX(current().layout)}
                      </Show>
                      <Show when={!isVerticalCenter(current().layout.alignment)}>
                        {positionY(current().layout)}
                      </Show>
                    </>
                  }
                >
                  <div class="flex items-center">
                    <div class="flex min-w-0 flex-1 flex-col gap-[10px]">
                      {positionX(current().layout)}
                      {positionY(current().layout)}
                    </div>
                    <button
                      type="button"
                      aria-label={t("Positioning lock")}
                      class="grid w-[44px] shrink-0 place-items-center text-phone-tint"
                      onClick={() =>
                        edit((draft) => {
                          draft.positioningLock = !draft.positioningLock;
                          setYBasedOnXIfLocked(draft);
                        })
                      }
                    >
                      <Icon
                        name={current().layout.positioningLock ? "lock" : "lock-open"}
                        class="size-[26px]"
                      />
                    </button>
                  </div>
                </Show>
              </Show>
              <Show when={hasSize(current().widget.type)}>
                <ValueEditCompact
                  value={current().layout.size}
                  minimum={1}
                  maximum={100}
                  increment={1 / 15}
                  numericInput={numericInput()}
                  decrementIcon="resize-down"
                  incrementIcon="resize-up"
                  onChange={(value) =>
                    edit((draft) => {
                      draft.size = value;
                    })
                  }
                />
              </Show>
              <ToggleRow
                label={t("Numeric input")}
                on={numericInput()}
                onToggle={setNumericInput}
              />
            </Section>
          </Show>
          <Section header={t("Shortcut")}>
            <LinkRow
              label={t("Widget")}
              onClick={() =>
                props.stack.push({
                  title: t("{type} widget", { type: widgetTypeName(current().widget.type) }),
                  render: () => (
                    <WidgetSettings
                      editor={editor}
                      stack={props.stack}
                      widgetId={current().widget.id}
                      settings={props.settings}
                    />
                  ),
                })
              }
            />
            <Show when={current().widget.type === "Scene"}>
              <LinkRow
                label={t("Scene")}
                onClick={() => {
                  const nested = editor.state.scenes.find((candidate) =>
                    sameId(candidate.id, current().widget.scene.sceneId),
                  );
                  if (nested) {
                    props.stack.push({
                      title: t("Scene"),
                      render: () => (
                        <SceneSettings
                          editor={editor}
                          stack={props.stack}
                          sceneId={nested.id}
                          canvas={props.canvas}
                          settings={props.settings}
                          onCreateWidget={props.onCreateWidget}
                        />
                      ),
                    });
                  }
                }}
              />
            </Show>
          </Section>
        </>
      )}
    </Show>
  );
}
