import { Show } from "solid-js";
import {
  MenuRow,
  Section,
  ToggleRow,
  useReadOnly,
  ValueEditCompact,
  ValueRow,
} from "../sceneEditor/Form";
import { CopyPasteLayoutAndAlignment } from "../sceneEditor/settings/SceneWidgetSettings";
import { secondsItems } from "../sceneEditor/settings/widgets/shared";
import type { StreamFormProps } from "./StreamsSettings";
import {
  isHorizontalCenter,
  isVerticalCenter,
  mirrorPositionHorizontally,
  mirrorPositionVertically,
} from "../../data/moblinSettings";
import type { ReplayTransitionType, SettingsStreamReplay } from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

const transitionTypes = (): { id: ReplayTransitionType; name: string }[] => [
  { id: "fade", name: t("Fade") },
  { id: "stingers", name: t("Stingers") },
  { id: "none", name: t("None") },
];

function LayoutSection(props: StreamFormProps) {
  const readOnly = useReadOnly();
  const replay = () => props.stream.replay;
  const dimensions = () => {
    const [width, height] = props.stream.resolution.split("x").map(Number);
    return props.stream.portrait ? { width: height, height: width } : { width, height };
  };
  const edit = (change: (replay: SettingsStreamReplay) => void) =>
    props.edit((draft) => change(draft.replay));
  return (
    <Section header={t("Layout")}>
      <fieldset disabled={readOnly()} class="contents">
        <CopyPasteLayoutAndAlignment
          layout={replay()}
          onPaste={(layout) => edit((draft) => Object.assign(draft, layout))}
          onAlignment={(alignment) =>
            edit((draft) => {
              draft.alignment = alignment;
            })
          }
        />
        <Show when={!isHorizontalCenter(replay().alignment)}>
          <ValueEditCompact
            value={replay().x}
            minimum={-100}
            maximum={100}
            total={dimensions().width!}
            mirror={mirrorPositionHorizontally(replay().alignment)}
            decrementIcon="arrow-left-circle"
            incrementIcon="arrow-right-circle"
            onChange={(value) =>
              edit((draft) => {
                draft.x = value;
              })
            }
          />
        </Show>
        <Show when={!isVerticalCenter(replay().alignment)}>
          <ValueEditCompact
            value={replay().y}
            minimum={-100}
            maximum={100}
            total={dimensions().height!}
            mirror={mirrorPositionVertically(replay().alignment)}
            decrementIcon="arrow-up-circle"
            incrementIcon="arrow-down-circle"
            onChange={(value) =>
              edit((draft) => {
                draft.y = value;
              })
            }
          />
        </Show>
        <ValueEditCompact
          value={replay().size}
          minimum={1}
          maximum={100}
          total={dimensions().width!}
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
