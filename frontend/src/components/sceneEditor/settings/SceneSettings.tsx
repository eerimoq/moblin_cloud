import { createMemo, onMount, Show } from "solid-js";
import Icon from "../../Icon";
import {
  ButtonRow,
  ColorRow,
  MenuRow,
  PickerRow,
  ReorderableList,
  Section,
  TextRow,
  ToggleRow,
} from "../Form";
import { widgetIcon } from "../icons";
import type { Stack } from "../Stack";
import type { Editor } from "../store";
import {
  applyVideoSource,
  videoSourceItemId,
  videoSourceItems,
  videoSourceOf,
} from "../videoSources";
import { SceneWidgetSettings } from "./SceneWidgetSettings";
import type { DeviceSettings } from "../../../data/account";
import {
  defaultSceneWidget,
  rgb,
  sameId,
  type SettingsScene,
  type VideoStabilizationMode,
} from "../../../data/moblinSettings";
import { SceneResolver } from "../../../data/sceneResolver";
import type { Size } from "../../../data/widgetOutlines";
import { t } from "../../../i18n";

const stabilizationModes = (): { id: VideoStabilizationMode; name: string }[] => [
  { id: "Off", name: t("Off") },
  { id: "Standard", name: t("Standard") },
  { id: "Cinematic", name: t("Cinematic") },
  { id: "Cinematic extended enhanced", name: t("Cinematic extended enhanced") },
];

export function NameRow(props: {
  stack: Stack;
  name: string;
  existing: string[];
  onSubmit: (name: string) => void;
}) {
  return (
    <TextRow
      stack={props.stack}
      label={t("Name")}
      value={props.name}
      validate={(value) => {
        if (value.trim() === "") {
          return t("Empty names are not allowed.");
        }
        if (value !== props.name && props.existing.includes(value)) {
          return t("The name '{name}' is already in use.", { name: value });
        }
        return null;
      }}
      onSubmit={props.onSubmit}
    />
  );
}

function DefaultMic(props: {
  editor: Editor;
  micId: string;
  edit: (change: (scene: SettingsScene) => void) => void;
}) {
  onMount(() => {
    const current = props.editor.state.currentMic;
    if (props.micId === "" && current) {
      props.edit((draft) => {
        draft.micId = current;
      });
    }
  });
  return null;
}

export function SceneSettings(props: {
  editor: Editor;
  stack: Stack;
  sceneId: string;
  canvas: Size;
  settings: DeviceSettings | null;
  onCreateWidget: () => void;
}) {
  const editor = props.editor;
  const scene = createMemo(() =>
    editor.state.scenes.find((candidate) => sameId(candidate.id, props.sceneId)),
  );
  const edit = (change: (scene: SettingsScene) => void) => editor.editScene(props.sceneId, change);
  const resolver = createMemo(() => new SceneResolver(editor.state.scenes, editor.state.widgets));
  const addable = createMemo(() => {
    const current = scene();
    return current ? resolver().addableWidgets(current) : [];
  });
  const widgetOf = (widgetId: string) =>
    editor.state.widgets.find((widget) => sameId(widget.id, widgetId));
  const openSceneWidget = (sceneWidgetId: string) => {
    const current = scene();
    const sceneWidget = current?.widgets.find((candidate) => candidate.id === sceneWidgetId);
    const widget = sceneWidget ? widgetOf(sceneWidget.widgetId) : undefined;
    if (!current || !sceneWidget || !widget) {
      return;
    }
    props.stack.push({
      title: () => widgetOf(widget.id)?.name ?? "",
      render: () => (
        <SceneWidgetSettings
          editor={editor}
          stack={props.stack}
          scene={current}
          sceneWidgetId={sceneWidget.id}
          widgetId={widget.id}
          canvas={props.canvas}
        />
      ),
    });
  };
  const showAll = () => editor.state.options?.showAllSettings ?? true;
  const addWidget = () =>
    props.stack.push({
      title: t("Add widget"),
      render: () => (
        <Section>
          {addable().map((widget) => (
            <button
              type="button"
              class="flex min-h-[44px] w-full items-center gap-2 px-4 py-2 text-left text-[17px]"
              onClick={() => {
                if (addable().some((candidate) => sameId(candidate.id, widget.id))) {
                  edit((draft) => {
                    draft.widgets.push(
                      defaultSceneWidget(widget.id, widget.type, widget.scoreboard.type),
                    );
                  });
                }
                props.stack.pop();
              }}
            >
              <Icon name={widgetIcon(widget.type)} class="size-[18px] shrink-0 text-phone-tint" />
              <span class="min-w-0 flex-1 truncate">{widget.name}</span>
            </button>
          ))}
        </Section>
      ),
    });

  return (
    <Show when={scene()}>
      {(current) => (
        <>
          <Section>
            <NameRow
              stack={props.stack}
              name={current().name}
              existing={editor.state.scenes.map((candidate) => candidate.name)}
              onSubmit={(name) =>
                edit((draft) => {
                  draft.name = name;
                })
              }
            />
          </Section>
          <Section
            header={t("Widgets")}
            footer={t("Drag a widget to reorder it, and remove it with its trash button.")}
          >
            <ReorderableList
              items={current().widgets}
              render={(sceneWidget) => (
                <Show when={widgetOf(sceneWidget.widgetId)}>
                  {(widget) => (
                    <div class="flex min-h-[44px] items-center gap-3 pr-2 pl-3 text-[17px]">
                      <button
                        type="button"
                        class="flex min-w-0 flex-1 items-center gap-2 text-left"
                        onClick={() => openSceneWidget(sceneWidget.id)}
                      >
                        <Icon
                          name={widgetIcon(widget().type)}
                          class="size-[18px] shrink-0 text-phone-tint"
                        />
                        <span class="min-w-0 flex-1 truncate">{widget().name}</span>
                        <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
                      </button>
                      <button
                        type="button"
                        role="switch"
                        aria-checked={widget().enabled}
                        aria-label={widget().name}
                        class={`relative h-[31px] w-[51px] shrink-0 rounded-full transition-colors ${
                          widget().enabled ? "bg-phone-switch" : "bg-phone-separator"
                        }`}
                        onClick={() =>
                          editor.editWidget(widget().id, (draft) => {
                            draft.enabled = !draft.enabled;
                          })
                        }
                      >
                        <span
                          class={`absolute top-[2px] left-[2px] size-[27px] rounded-full bg-white shadow transition-transform ${
                            widget().enabled ? "translate-x-[20px]" : ""
                          }`}
                        />
                      </button>
                    </div>
                  )}
                </Show>
              )}
              onMove={(from, to) =>
                edit((draft) => {
                  const [moved] = draft.widgets.splice(from, 1);
                  draft.widgets.splice(to, 0, moved!);
                })
              }
              onDelete={(sceneWidget) =>
                edit((draft) => {
                  draft.widgets = draft.widgets.filter(
                    (candidate) => candidate.id !== sceneWidget.id,
                  );
                })
              }
            />
            <ButtonRow label={t("Add")} disabled={addable().length === 0} onClick={addWidget} />
            <ButtonRow label={t("Create")} onClick={() => props.onCreateWidget()} />
          </Section>
          <Section
            header={t("Video source")}
            footer={
              showAll()
                ? t(
                    "Enable Override video stabilization to override Settings → Camera → Video stabilization in this scene.",
                  )
                : undefined
            }
          >
            <PickerRow
              stack={props.stack}
              label={t("Name")}
              icon="camera"
              badge={
                (editor.state.options?.inactiveVideoSourceSceneIds ?? []).some((id) =>
                  sameId(id, props.sceneId),
                )
                  ? "cable-slash"
                  : undefined
              }
              items={videoSourceItems(editor.state.videoSources)}
              selected={videoSourceItemId(current())}
              footers={[
                t(
                  "The ultra wide camera does not perform well in low light conditions. Likewise, the auto cameras do not perform well when zoom is 0.5-1.0x since the ultra wide camera will be used.",
                ),
                "",
                t(
                  "Auto cameras use more energy as multiple cameras are powered on, even if only one is used at a time. This allows the phone to quickly change camera when zooming.",
                ),
              ]}
              onPick={(id) => {
                const source = videoSourceOf(editor.state.videoSources, id);
                if (source) {
                  edit((draft) => applyVideoSource(draft, source));
                }
              }}
            />
            <Show when={showAll() && current().cameraPosition !== "None"}>
              <MenuRow
                label={t("Rotation")}
                items={[0, 90, 180, 270].map((degrees) => ({
                  id: String(degrees),
                  name: `${degrees}°`,
                }))}
                selected={String(current().videoSourceRotation)}
                onPick={(id) =>
                  edit((draft) => {
                    draft.videoSourceRotation = Number(id);
                  })
                }
              />
            </Show>
            <Show when={showAll()}>
              <ToggleRow
                label={t("Mirror")}
                on={current().mirror}
                onToggle={(on) =>
                  edit((draft) => {
                    draft.mirror = on;
                  })
                }
              />
              <ToggleRow
                label={t("Override video stabilization")}
                on={current().overrideVideoStabilizationMode}
                onToggle={(on) =>
                  edit((draft) => {
                    draft.overrideVideoStabilizationMode = on;
                  })
                }
              />
            </Show>
            <Show when={showAll() && current().overrideVideoStabilizationMode}>
              <MenuRow
                label={t("Video stabilization")}
                items={stabilizationModes()}
                selected={current().videoStabilizationMode}
                onPick={(id) =>
                  edit((draft) => {
                    draft.videoStabilizationMode = id as VideoStabilizationMode;
                  })
                }
              />
            </Show>
            <Show when={showAll() && current().cameraPosition !== "None"}>
              <ToggleRow
                label={t("Fill frame")}
                on={current().fillFrame}
                onToggle={(on) =>
                  edit((draft) => {
                    draft.fillFrame = on;
                  })
                }
              />
            </Show>
          </Section>
          <Section
            footer={
              <span class="flex flex-col">
                <span>{t("Switching between scenes in the same group may be instant.")}</span>
                <Show when={editor.state.options?.forceSceneSwitchTransition}>
                  <span>&nbsp;</span>
                  <span>
                    {t(
                      "⚠️ Disable Settings → Scenes → Scene switching → Force transition to enable groups.",
                    )}
                  </span>
                </Show>
              </span>
            }
          >
            <MenuRow
              label={t("Quick switch group")}
              items={[
                { id: "", name: t("-- None --") },
                ...[1, 2, 3, 4].map((group) => ({ id: String(group), name: String(group) })),
              ]}
              selected={
                current().quickSwitchGroup === null ? "" : String(current().quickSwitchGroup)
              }
              onPick={(id) =>
                edit((draft) => {
                  draft.quickSwitchGroup = id === "" ? null : Number(id);
                })
              }
            />
          </Section>
          <Show when={showAll()}>
            <Section
              header={t("Mic")}
              footer={t(
                "Enable Override to automatically switch to selected mic (if available) when switching to this scene.",
              )}
            >
              <ToggleRow
                label={t("Override")}
                on={current().overrideMic}
                onToggle={(on) =>
                  edit((draft) => {
                    draft.overrideMic = on;
                  })
                }
              />
              <Show when={current().overrideMic}>
                <DefaultMic editor={editor} micId={current().micId} edit={edit} />
                <PickerRow
                  stack={props.stack}
                  label={t("Mic")}
                  items={props.settings?.mics ?? []}
                  selected={current().micId}
                  onPick={(id) =>
                    edit((draft) => {
                      draft.micId = id;
                    })
                  }
                />
              </Show>
            </Section>
          </Show>
          <Section
            header={t("Color")}
            footer={t("Background color of the scene button when selected.")}
          >
            <ColorRow
              label={t("Background")}
              color={current().backgroundColor}
              opacity
              onChange={(color) =>
                edit((draft) => {
                  draft.backgroundColor = color;
                })
              }
            />
            <ButtonRow
              label={t("Reset")}
              onClick={() =>
                edit((draft) => {
                  draft.backgroundColor = rgb(142, 142, 147, 0.6);
                })
              }
            />
          </Section>
        </>
      )}
    </Show>
  );
}
