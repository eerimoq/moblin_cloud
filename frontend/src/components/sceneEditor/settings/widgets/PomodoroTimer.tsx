import Icon from "../../../Icon";
import { ColorRow, MenuRow, PickerRow, Section, SliderRow, TextRow } from "../../Form";
import { symbolIcon } from "../../icons";
import { pickerItems, type WidgetFormProps } from "./shared";
import { pomodoroBreakIcons, pomodoroFocusIcons } from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

const focusIconNames: Record<string, string> = {
  "sun.max": t("Sun"),
  "bolt.circle": t("Bolt"),
  graduationcap: t("Cap"),
  "text.book.closed": t("Book"),
  "arrowtriangle.right.circle": t("Play"),
  "brain.head.profile": t("Brain"),
  pencil: t("Pencil"),
  flame: t("Flame"),
  timer: t("Timer"),
  target: t("Target"),
};

const breakIconNames: Record<string, string> = {
  "cup.and.saucer": t("Cup"),
  "figure.dance": t("Dance"),
  "figure.walk": t("Walk"),
  "music.note": t("Music"),
  leaf: t("Leaf"),
  gamecontroller: t("Controller"),
  "fork.knife": t("Fork and knife"),
};

export function PomodoroTimerForm(props: WidgetFormProps) {
  const timer = () => props.widget.pomodoroTimer;
  const sounds = () => {
    const gallery = props.editor.state.gallery;
    return gallery ? [...gallery.bundledSounds, ...gallery.customSounds] : [];
  };
  const soundItems = (selected: string | null) => [
    ...(selected === null ? [{ id: "", name: t("-- None --") }] : []),
    ...sounds().map((sound) => ({ id: sound.id, name: sound.name })),
  ];
  return (
    <>
      <Section header={t("Durations")}>
        <MenuRow
          label={t("Focus")}
          items={pickerItems([1, 2, 3, 5, 10, 15, 20, 25, 30, 45, 60, 90, 120], " min")}
          selected={String(timer().focusDuration)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusDuration = Number(id);
            })
          }
        />
        <MenuRow
          label={t("Break")}
          items={pickerItems([1, 2, 3, 5, 7, 10, 15, 20, 25, 30], " min")}
          selected={String(timer().breakDuration)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakDuration = Number(id);
            })
          }
        />
      </Section>
      <Section header={t("Names")}>
        <TextRow
          stack={props.stack}
          label={t("Focus")}
          value={timer().focusName}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusName = value;
            })
          }
        />
        <TextRow
          stack={props.stack}
          label={t("Break")}
          value={timer().breakName}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakName = value;
            })
          }
        />
      </Section>
      <Section header={t("Icons")}>
        <div class="flex items-center">
          <Icon name={symbolIcon(timer().focusIcon)} class="ml-4 size-[18px] text-phone-tint" />
          <div class="min-w-0 flex-1">
            <MenuRow
              label={t("Focus")}
              items={pomodoroFocusIcons.map((icon) => ({
                id: icon,
                name: focusIconNames[icon] ?? icon,
              }))}
              selected={timer().focusIcon}
              onPick={(id) =>
                props.edit((draft) => {
                  draft.pomodoroTimer.focusIcon = id;
                })
              }
            />
          </div>
        </div>
        <div class="flex items-center">
          <Icon name={symbolIcon(timer().breakIcon)} class="ml-4 size-[18px] text-phone-tint" />
          <div class="min-w-0 flex-1">
            <MenuRow
              label={t("Break")}
              items={pomodoroBreakIcons.map((icon) => ({
                id: icon,
                name: breakIconNames[icon] ?? icon,
              }))}
              selected={timer().breakIcon}
              onPick={(id) =>
                props.edit((draft) => {
                  draft.pomodoroTimer.breakIcon = id;
                })
              }
            />
          </div>
        </div>
      </Section>
      <Section>
        <SliderRow
          label={t("Width")}
          value={timer().width}
          minimum={1}
          maximum={5}
          step={0.05}
          onChange={(width) =>
            props.edit((draft) => {
              draft.pomodoroTimer.width = width;
            })
          }
        />
      </Section>
      <Section header={t("Colors")}>
        <ColorRow
          label={t("Background")}
          color={timer().backgroundColor}
          opacity
          onChange={(color) =>
            props.edit((draft) => {
              draft.pomodoroTimer.backgroundColor = color;
            })
          }
        />
        <ColorRow
          label={t("Text")}
          color={timer().foregroundColor}
          onChange={(color) =>
            props.edit((draft) => {
              draft.pomodoroTimer.foregroundColor = color;
            })
          }
        />
        <ColorRow
          label={t("Focus")}
          color={timer().focusColor}
          onChange={(color) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusColor = color;
            })
          }
        />
        <ColorRow
          label={t("Break")}
          color={timer().breakColor}
          onChange={(color) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakColor = color;
            })
          }
        />
      </Section>
      <Section header={t("Sounds")}>
        <PickerRow
          stack={props.stack}
          label={t("Focus to break")}
          title={t("Sound")}
          items={soundItems(timer().focusToBreakSoundId)}
          selected={timer().focusToBreakSoundId ?? ""}
          onPick={(id) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusToBreakSoundId = id === "" ? null : id;
            })
          }
        />
        <PickerRow
          stack={props.stack}
          label={t("Break to focus")}
          title={t("Sound")}
          items={soundItems(timer().breakToFocusSoundId)}
          selected={timer().breakToFocusSoundId ?? ""}
          onPick={(id) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakToFocusSoundId = id === "" ? null : id;
            })
          }
        />
      </Section>
      <Section header={t("Chat messages")}>
        <TextRow
          stack={props.stack}
          label={t("Focus to break")}
          value={timer().focusToBreakChatMessage}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusToBreakChatMessage = value;
            })
          }
        />
        <TextRow
          stack={props.stack}
          label={t("Break to focus")}
          value={timer().breakToFocusChatMessage}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakToFocusChatMessage = value;
            })
          }
        />
      </Section>
    </>
  );
}
