import { createMemo, Match, Show, Switch } from "solid-js";
import { Section } from "../Form";
import type { Stack } from "../Stack";
import type { Editor } from "../store";
import { NameRow } from "./SceneSettings";
import { BingoCardForm } from "./widgets/BingoCard";
import { BrowserForm } from "./widgets/Browser";
import { ChatEmoteComboForm } from "./widgets/ChatEmoteCombo";
import { ChatForm } from "./widgets/Chat";
import { CropForm } from "./widgets/Crop";
import { ImageForm } from "./widgets/Image";
import { MapForm } from "./widgets/Map";
import { PngTuberForm } from "./widgets/PngTuber";
import { PomodoroTimerForm } from "./widgets/PomodoroTimer";
import { QrCodeForm } from "./widgets/QrCode";
import { SceneForm } from "./widgets/Scene";
import { SlideshowForm } from "./widgets/Slideshow";
import { SnapshotForm } from "./widgets/Snapshot";
import { VideoSourceForm } from "./widgets/VideoSource";
import { VTuberForm } from "./widgets/VTuber";
import { AlertsForm } from "./widgets/Alerts";
import { ScoreboardForm } from "./widgets/Scoreboard";
import { TextForm } from "./widgets/Text";
import { WheelOfLuckForm } from "./widgets/WheelOfLuck";
import { sameId, type SettingsWidget } from "../../../data/moblinSettings";

export function WidgetSettings(props: { editor: Editor; stack: Stack; widgetId: string }) {
  const editor = props.editor;
  const widget = createMemo(() =>
    editor.state.widgets.find((candidate) => sameId(candidate.id, props.widgetId)),
  );
  const edit = (change: (widget: SettingsWidget) => void) =>
    editor.editWidget(props.widgetId, change);
  return (
    <Show when={widget()}>
      {(current) => {
        const form = {
          editor,
          stack: props.stack,
          get widget() {
            return current();
          },
          edit,
        };
        return (
          <>
            <Section>
              <NameRow
                stack={props.stack}
                name={current().name}
                existing={editor.state.widgets.map((candidate) => candidate.name)}
                onSubmit={(name) =>
                  edit((draft) => {
                    draft.name = name;
                  })
                }
              />
            </Section>
            <Switch>
              <Match when={current().type === "Text"}>
                <TextForm {...form} />
              </Match>
              <Match when={current().type === "Alerts"}>
                <AlertsForm {...form} />
              </Match>
              <Match when={current().type === "Scoreboard"}>
                <ScoreboardForm {...form} />
              </Match>
              <Match when={current().type === "Browser"}>
                <BrowserForm {...form} />
              </Match>
              <Match when={current().type === "Crop"}>
                <CropForm {...form} />
              </Match>
              <Match when={current().type === "Map"}>
                <MapForm {...form} />
              </Match>
              <Match when={current().type === "Scene"}>
                <SceneForm {...form} />
              </Match>
              <Match when={current().type === "QR code"}>
                <QrCodeForm {...form} />
              </Match>
              <Match when={current().type === "Snapshot"}>
                <SnapshotForm {...form} />
              </Match>
              <Match when={current().type === "Chat emote combo"}>
                <ChatEmoteComboForm {...form} />
              </Match>
              <Match when={current().type === "Image"}>
                <ImageForm {...form} />
              </Match>
              <Match when={current().type === "Slideshow"}>
                <SlideshowForm {...form} />
              </Match>
              <Match when={current().type === "Video source"}>
                <VideoSourceForm {...form} />
              </Match>
              <Match when={current().type === "VTuber"}>
                <VTuberForm {...form} />
              </Match>
              <Match when={current().type === "PNGTuber"}>
                <PngTuberForm {...form} />
              </Match>
              <Match when={current().type === "Chat"}>
                <ChatForm {...form} />
              </Match>
              <Match when={current().type === "Wheel of luck"}>
                <WheelOfLuckForm {...form} />
              </Match>
              <Match when={current().type === "Bingo card"}>
                <BingoCardForm {...form} />
              </Match>
              <Match when={current().type === "Pomodoro timer"}>
                <PomodoroTimerForm {...form} />
              </Match>
            </Switch>
          </>
        );
      }}
    </Show>
  );
}
