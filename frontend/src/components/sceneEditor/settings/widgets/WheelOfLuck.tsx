import { onMount, Show } from "solid-js";
import Icon from "../../../Icon";
import {
  ButtonRow,
  LinkRow,
  MenuRow,
  ReorderableList,
  Section,
  TextRow,
  ToggleRow,
  useReadOnly,
} from "../../Form";
import { Lines, pickerItems, type WidgetFormProps } from "./shared";
import {
  defaultWheelOption,
  newUuid,
  sameId,
  type GimbalMotion,
  type SettingsWidget,
  type WheelOfLuckOption,
} from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

export function WheelOfLuckQuickControls(props: {
  edit: (change: (widget: SettingsWidget) => void) => void;
}) {
  const readOnly = useReadOnly();
  return (
    <div class="flex justify-end px-4 py-2">
      <button
        type="button"
        aria-label={t("Shuffle")}
        title={t("Shuffle")}
        class="text-phone-tint disabled:opacity-50"
        disabled={readOnly()}
        onClick={() =>
          props.edit((draft) => {
            const options = draft.wheelOfLuck.options;
            for (let index = options.length - 1; index > 0; index--) {
              const other = Math.floor(Math.random() * (index + 1));
              [options[index], options[other]] = [options[other]!, options[index]!];
            }
          })
        }
      >
        <Icon name="shuffle" class="size-[28px]" />
      </button>
    </div>
  );
}

const weights = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 40, 60, 80, 100];

const generalFunctions = (): { id: string; name: string }[] => [
  { id: "Unused", name: t("Unused") },
  { id: "Record", name: t("Record") },
  { id: "Stream", name: t("Stream") },
  { id: "Gimbal animate", name: t("Gimbal animate") },
  { id: "Gimbal tracking", name: t("Gimbal tracking") },
  { id: "Mute", name: t("Mute") },
  { id: "Torch", name: t("Torch") },
  { id: "Black screen", name: t("Stealth mode") },
  { id: "Scene", name: t("Scene") },
  { id: "Switch scene", name: t("Switch scene") },
  { id: "Widget", name: t("Widget") },
  { id: "Macro", name: t("Macro") },
  { id: "Stream deck layout", name: t("Stream Deck layout") },
  { id: "Instant replay", name: t("Instant replay") },
  { id: "Stop replay", name: t("Stop replay") },
  { id: "Snapshot", name: t("Snapshot") },
  { id: "Pause TTS", name: t("Pause TTS") },
];

const filterFunctions = (): { id: string; name: string }[] => [
  { id: "Pixellate", name: t("Pixellate") },
  { id: "Movie", name: t("Movie") },
  { id: "Gray scale", name: t("Gray scale") },
  { id: "Sepia", name: t("Sepia") },
  { id: "Triple", name: t("Triple") },
  { id: "Twin", name: t("Twin") },
  { id: "4:3", name: t("4:3") },
  { id: "Pinch", name: t("Pinch") },
  { id: "Whirlpool", name: t("Whirlpool") },
  { id: "Poll", name: t("Poll") },
  { id: "Blur faces", name: t("Blur faces") },
  { id: "Privacy", name: t("Blur background") },
  { id: "Beauty", name: t("Beauty") },
  { id: "Camera man", name: t("Camera man") },
];

const motions = (): { id: string; name: string; motion: GimbalMotion }[] => [
  { id: "kapow", name: t("Kapow"), motion: { kapow: {} } },
  { id: "yes", name: t("Yes"), motion: { yes: {} } },
  { id: "no", name: t("No"), motion: { no: {} } },
  { id: "wakeup", name: t("Wakeup"), motion: { wakeup: {} } },
];

const optionsFromText = (text: string, existing: WheelOfLuckOption[]) =>
  text
    .trim()
    .split("\n")
    .map((line, index) => defaultWheelOption(existing[index]?.id ?? newUuid(), line.trim()));

const sameOptions = (first: WheelOfLuckOption[], second: WheelOfLuckOption[]) =>
  first.length === second.length &&
  first.every((option, index) => {
    const other = second[index]!;
    return (
      option.text === other.text &&
      option.weight === other.weight &&
      option.function === other.function &&
      option.sceneId === other.sceneId &&
      option.widgetId === other.widgetId &&
      option.gimbalPresetId === other.gimbalPresetId &&
      Object.keys(option.gimbalMotion)[0] === Object.keys(other.gimbalMotion)[0] &&
      option.macroId === other.macroId &&
      option.streamDeckLayoutId === other.streamDeckLayoutId
    );
  });

function SimpleOptions(props: WidgetFormProps) {
  const readOnly = useReadOnly();
  const wheel = () => props.widget.wheelOfLuck;
  const text = () =>
    wheel()
      .options.map((option) => option.text)
      .join("\n");
  const setText = (value: string) => {
    const rebuilt = optionsFromText(value, wheel().options);
    if (!sameOptions(rebuilt, wheel().options)) {
      props.edit((draft) => {
        draft.wheelOfLuck.options = rebuilt;
      });
    }
  };
  onMount(() => {
    if (!readOnly()) {
      setText(text());
    }
  });
  return (
    <Lines header={t("Options")} value={text()} placeholder={t("My text")} onChange={setText} />
  );
}

export function WheelOfLuckForm(props: WidgetFormProps) {
  const readOnly = useReadOnly();
  const wheel = () => props.widget.wheelOfLuck;
  const totalWeight = () =>
    Math.max(
      wheel().options.reduce((sum, option) => sum + option.weight, 0),
      1,
    );
  const editOption = (optionId: string, change: (option: WheelOfLuckOption) => void) =>
    props.edit((draft) => {
      const option = draft.wheelOfLuck.options.find((candidate) => sameId(candidate.id, optionId));
      if (option) {
        change(option);
      }
    });
  const noneItem = { id: "", name: t("-- None --") };
  const openOption = (optionId: string) =>
    props.stack.push({
      title: t("Option"),
      render: () => {
        const option = () =>
          props.widget.wheelOfLuck.options.find((candidate) => sameId(candidate.id, optionId));
        return (
          <Show when={option()}>
            {(current) => (
              <>
                <Section>
                  <TextRow
                    stack={props.stack}
                    label={t("Text")}
                    value={current().text}
                    onSubmit={(text) =>
                      editOption(optionId, (option) => {
                        option.text = text;
                      })
                    }
                  />
                  <MenuRow
                    label={t("Weight")}
                    items={pickerItems(weights)}
                    selected={String(current().weight)}
                    onPick={(id) =>
                      editOption(optionId, (option) => {
                        option.weight = Number(id);
                      })
                    }
                  />
                </Section>
                <Section
                  header={t("Action")}
                  footer={t("Executed when the wheel stops on this option.")}
                >
                  <label class="flex min-h-[44px] w-full items-center gap-3 px-4 py-2 text-[17px]">
                    <span class="min-w-0 flex-1 truncate">{t("Function")}</span>
                    <select
                      class="max-w-[50%] truncate bg-transparent text-right text-phone-border outline-none disabled:opacity-50"
                      value={current().function}
                      disabled={readOnly()}
                      onChange={(event) =>
                        editOption(optionId, (option) => {
                          option.function = event.currentTarget.value;
                        })
                      }
                    >
                      <optgroup label={t("General")}>
                        {generalFunctions().map((item) => (
                          <option value={item.id} selected={item.id === current().function}>
                            {item.name}
                          </option>
                        ))}
                      </optgroup>
                      <optgroup label={t("Filters")}>
                        {filterFunctions().map((item) => (
                          <option value={item.id} selected={item.id === current().function}>
                            {item.name}
                          </option>
                        ))}
                      </optgroup>
                    </select>
                  </label>
                  <Show when={current().function === "Scene"}>
                    <MenuRow
                      label={t("Scene")}
                      items={[
                        noneItem,
                        ...props.editor.state.scenes.map((scene) => ({
                          id: scene.id,
                          name: scene.name,
                        })),
                      ]}
                      selected={current().sceneId ?? ""}
                      onPick={(id) =>
                        editOption(optionId, (option) => {
                          option.sceneId = id === "" ? null : id;
                        })
                      }
                    />
                  </Show>
                  <Show when={current().function === "Widget"}>
                    <MenuRow
                      label={t("Widget")}
                      items={[
                        noneItem,
                        ...props.editor.state.widgets.map((widget) => ({
                          id: widget.id,
                          name: widget.name,
                        })),
                      ]}
                      selected={current().widgetId ?? ""}
                      onPick={(id) =>
                        editOption(optionId, (option) => {
                          option.widgetId = id === "" ? null : id;
                        })
                      }
                    />
                  </Show>
                  <Show when={current().function === "Gimbal animate"}>
                    <MenuRow
                      label={t("Motion")}
                      items={motions()}
                      selected={Object.keys(current().gimbalMotion)[0] ?? "kapow"}
                      onPick={(id) =>
                        editOption(optionId, (option) => {
                          option.gimbalMotion = motions().find((motion) => motion.id === id)
                            ?.motion ?? { kapow: {} };
                        })
                      }
                    />
                  </Show>
                  <Show when={current().function === "Macro"}>
                    <MenuRow
                      label={t("Macro")}
                      items={[noneItem, ...props.editor.state.macros]}
                      selected={current().macroId ?? ""}
                      onPick={(id) =>
                        editOption(optionId, (option) => {
                          option.macroId = id === "" ? null : id;
                        })
                      }
                    />
                  </Show>
                  <Show when={current().function === "Stream deck layout"}>
                    <MenuRow
                      label={t("Layout")}
                      items={[noneItem, ...(props.editor.state.options?.streamDeckLayouts ?? [])]}
                      selected={current().streamDeckLayoutId ?? ""}
                      onPick={(id) =>
                        editOption(optionId, (option) => {
                          option.streamDeckLayoutId = id === "" ? null : id;
                        })
                      }
                    />
                  </Show>
                </Section>
              </>
            )}
          </Show>
        );
      },
    });

  return (
    <>
      <Show when={wheel().advanced} fallback={<SimpleOptions {...props} />}>
        <Section header={t("Options")}>
          <ReorderableList
            items={wheel().options}
            render={(option) => (
              <LinkRow
                label={option.text}
                value={`${Math.floor((100 * option.weight) / totalWeight())}%`}
                onClick={() => openOption(option.id)}
              />
            )}
            onMove={(from, to) =>
              props.edit((draft) => {
                const [moved] = draft.wheelOfLuck.options.splice(from, 1);
                draft.wheelOfLuck.options.splice(to, 0, moved!);
              })
            }
            onDelete={(option) =>
              props.edit((draft) => {
                draft.wheelOfLuck.options = draft.wheelOfLuck.options.filter(
                  (candidate) => !sameId(candidate.id, option.id),
                );
              })
            }
            deleteDisabled={() => wheel().options.length < 2}
          />
          <ButtonRow
            label={t("Create")}
            onClick={() =>
              props.edit((draft) => {
                draft.wheelOfLuck.options.push(defaultWheelOption(newUuid()));
              })
            }
          />
        </Section>
      </Show>
      <Section>
        <ToggleRow
          label={t("Advanced")}
          on={wheel().advanced}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.wheelOfLuck.advanced = on;
            })
          }
        />
      </Section>
      <Section>
        <ToggleRow
          label={t("Only show when spinning")}
          on={wheel().onlyShowWhenSpinning}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.wheelOfLuck.onlyShowWhenSpinning = on;
            })
          }
        />
      </Section>
    </>
  );
}
