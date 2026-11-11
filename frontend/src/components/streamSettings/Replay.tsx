import { Show } from "solid-js";
import Icon from "../Icon";
import {
  MenuRow,
  Section,
  ToggleRow,
  useReadOnly,
  ValueEditCompact,
  ValueRow,
} from "../sceneEditor/Form";
import { savedLayout, setSavedLayout } from "../sceneEditor/settings/SceneWidgetSettings";
import { secondsItems } from "../sceneEditor/settings/widgets/shared";
import type { StreamFormProps } from "./StreamsSettings";
import {
  alignmentName,
  isHorizontalCenter,
  isHorizontalOrVerticalCenter,
  isVerticalCenter,
  mirrorPositionHorizontally,
  mirrorPositionVertically,
  type SettingsAlignment,
} from "../../data/moblinSettings";
import type { ReplayTransitionType, SettingsStreamReplay } from "../../data/moblinStreamSettings";
import { readStored } from "../../storage";
import { t } from "../../i18n";

const alignments: SettingsAlignment[][] = [
  ["TopLeft", "TopCenter", "TopRight"],
  ["LeftCenter", "Center", "RightCenter"],
  ["BottomLeft", "BottomCenter", "BottomRight"],
];

const transitionTypes = (): { id: ReplayTransitionType; name: string }[] => [
  { id: "fade", name: t("Fade") },
  { id: "stingers", name: t("Stingers") },
  { id: "none", name: t("None") },
];

function LayoutSection(props: StreamFormProps) {
  const readOnly = useReadOnly();
  const numericInput = readStored("sceneNumericInput") === "1";
  const replay = () => props.stream.replay;
  const dimensions = () => {
    const [width, height] = props.stream.resolution.split("x").map(Number);
    return props.stream.portrait ? { width: height, height: width } : { width, height };
  };
  const horizontalIncrement = () => 100 / dimensions().width;
  const verticalIncrement = () => 100 / dimensions().height;
  const edit = (change: (replay: SettingsStreamReplay) => void) =>
    props.edit((draft) => change(draft.replay));
  const setXBasedOnYIfLocked = (layout: SettingsStreamReplay) => {
    if (layout.positioningLock) {
      layout.x = (layout.y * horizontalIncrement()) / verticalIncrement();
    }
  };
  const setYBasedOnXIfLocked = (layout: SettingsStreamReplay) => {
    if (layout.positioningLock) {
      layout.y = (layout.x * verticalIncrement()) / horizontalIncrement();
    }
  };
  const positionX = (linked: boolean) => (
    <ValueEditCompact
      value={replay().x}
      minimum={0}
      maximum={100}
      increment={horizontalIncrement()}
      mirror={mirrorPositionHorizontally(replay().alignment)}
      numericInput={numericInput}
      decrementIcon="arrow-left-circle"
      incrementIcon="arrow-right-circle"
      onChange={(value) =>
        edit((draft) => {
          draft.x = value;
          if (linked) {
            setYBasedOnXIfLocked(draft);
          }
        })
      }
    />
  );
  const positionY = (linked: boolean) => (
    <ValueEditCompact
      value={replay().y}
      minimum={0}
      maximum={100}
      increment={verticalIncrement()}
      mirror={mirrorPositionVertically(replay().alignment)}
      numericInput={numericInput}
      decrementIcon="arrow-up-circle"
      incrementIcon="arrow-down-circle"
      onChange={(value) =>
        edit((draft) => {
          draft.y = value;
          if (linked) {
            setXBasedOnYIfLocked(draft);
          }
        })
      }
    />
  );
  return (
    <Section header={t("Layout")}>
      <fieldset disabled={readOnly()} class="contents">
        <div class="flex items-center gap-3 px-4 py-2">
          <div class="flex flex-1 flex-col gap-2">
            <button
              type="button"
              class="rounded-[8px] bg-phone-separator px-3 py-1.5 text-[15px] text-phone-tint"
              onClick={() => {
                const { x, y, size, alignment, positioningLock } = props.stream.replay;
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
                  props.edit((draft) => Object.assign(draft.replay, saved));
                }
              }}
            >
              {t("Load layout")}
            </button>
          </div>
          <div class="h-[100px] w-px bg-phone-separator" />
          <div class="flex flex-col gap-[5px]">
            {alignments.map((row) => (
              <div class="flex gap-[3px]">
                {row.map((alignment) => (
                  <button
                    type="button"
                    aria-label={alignmentName(alignment)}
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
                      name={replay().alignment === alignment ? "square-fill" : "square"}
                      class="size-[26px]"
                    />
                  </button>
                ))}
              </div>
            ))}
          </div>
        </div>
        <Show
          when={!isHorizontalCenter(replay().alignment) && !isVerticalCenter(replay().alignment)}
          fallback={
            <>
              <Show when={!isHorizontalCenter(replay().alignment)}>{positionX(false)}</Show>
              <Show when={!isVerticalCenter(replay().alignment)}>{positionY(false)}</Show>
            </>
          }
        >
          <div class="flex items-center">
            <div class="flex min-w-0 flex-1 flex-col gap-[10px]">
              {positionX(true)}
              {positionY(true)}
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
              <Icon name={replay().positioningLock ? "lock" : "lock-open"} class="size-[26px]" />
            </button>
          </div>
        </Show>
        <ValueEditCompact
          value={replay().size}
          minimum={1}
          maximum={100}
          increment={1 / 15}
          numericInput={numericInput}
          decrementIcon="resize-down"
          incrementIcon="resize-up"
          onChange={(value) =>
            edit((draft) => {
              draft.size = value;
            })
          }
        />
      </fieldset>
    </Section>
  );
}

export function ReplaySettings(props: StreamFormProps) {
  const replay = () => props.stream.replay;
  return (
    <>
      <Section>
        <ToggleRow
          label={t("Enabled")}
          on={replay().enabled}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.replay.enabled = on;
            })
          }
        />
      </Section>
      <Show when={props.streams.state.options?.showAllSettings}>
        <LayoutSection {...props} />
        <Section
          footer={
            replay().transitionType === "stingers" ? t("Pick videos on the phone.") : undefined
          }
        >
          <MenuRow
            label={t("Transition")}
            items={transitionTypes()}
            selected={replay().transitionType}
            onPick={(id) =>
              props.edit((draft) => {
                draft.replay.transitionType = id as ReplayTransitionType;
              })
            }
          />
          <Show when={replay().transitionType === "stingers"}>
            <ValueRow label={t("In video")} value={replay().inStinger.name} />
            <ValueRow label={t("Out video")} value={replay().outStinger.name} />
          </Show>
        </Section>
        <Section
          footer={t("Seconds to record after the Instant replay/Save replay button is pressed.")}
        >
          <MenuRow
            label={t("Post trigger delay")}
            items={secondsItems([2, 3, 4, 5])}
            selected={String(replay().postTriggerDelay)}
            onPick={(id) =>
              props.edit((draft) => {
                draft.replay.postTriggerDelay = Number(id);
              })
            }
          />
        </Section>
      </Show>
    </>
  );
}
