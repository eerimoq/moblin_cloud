import { createSignal, Show, type JSX } from "solid-js";
import Icon, { type IconName } from "../Icon";
import { rowClass, Toggle } from "../MoblinParts";
import {
  LinkRow,
  Secret,
  Section,
  TextEditPage,
  ToggleRow,
  useReadOnly,
} from "../sceneEditor/Form";
import { NameRow } from "../sceneEditor/settings/SceneSettings";
import type { Stack } from "../sceneEditor/Stack";
import { inUse, isLive, liveOrRecording, type StreamFormProps } from "./StreamsSettings";
import { StreamUrlSettings } from "./Url";
import { SrtSettings } from "./Srt";
import { RtmpSettings } from "./Rtmp";
import { RistSettings } from "./Rist";
import { WhipSettings } from "./Whip";
import { MobcamSettings } from "./Mobcam";
import { MultiStreamingRow } from "./MultiStreaming";
import { VideoSettings } from "./Video";
import { AudioSettings } from "./Audio";
import { RecordingSettings } from "./Recording";
import { SnapshotSettings } from "./Snapshot";
import { PreviewStreamSettings } from "./PreviewStream";
import { ReplaySettings } from "./Replay";
import { EmotesSettings } from "./Emotes";
import { GoLiveNotificationSettings } from "./GoLiveNotification";
import { RealtimeIrlSettings } from "./RealtimeIrl";
import { SoopSettings } from "./Soop";
import { OpenStreamingPlatformSettings } from "./OpenStreamingPlatform";
import { ObsRemoteControlSettings } from "./ObsRemoteControl";
import { TwitchSettings } from "./Twitch";
import { KickSettings } from "./Kick";
import { YouTubeSettings } from "./YouTube";
import { streamProtocol } from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

export function sensitive(value: string) {
  return value === "" ? "" : "••••••";
}

export function parseInt32(text: string) {
  if (!/^[+-]?\d+$/.test(text)) {
    return null;
  }
  const value = Number(text);
  return value >= -2147483648 && value <= 2147483647 ? value : null;
}

export function parseFloat32(text: string) {
  if (text === "") {
    return null;
  }
  const value = Number(text);
  return Number.isNaN(value) ? null : value;
}

export function showAllSettings(props: StreamFormProps) {
  return props.streams.state.options?.showAllSettings ?? false;
}

export function liveDisabled(props: StreamFormProps) {
  return inUse(props) && isLive(props.remote);
}

export function liveOrRecordingDisabled(props: StreamFormProps) {
  return inUse(props) && liveOrRecording(props.remote);
}

export function Lines(props: { lines: string[] }) {
  return (
    <span class="flex flex-col">
      {props.lines.map((line) => (line === "" ? <br /> : <span>{line}</span>))}
    </span>
  );
}

export function TextBlock(props: { children: JSX.Element }) {
  return <div class="px-4 py-2 text-[17px]">{props.children}</div>;
}

export function ToggleLinkRow(props: {
  label: string;
  on: boolean;
  disabled?: boolean;
  onToggle: (on: boolean) => void;
  onClick: () => void;
}) {
  const readOnly = useReadOnly();
  return (
    <div class={rowClass}>
      <button
        type="button"
        class="min-w-0 flex-1 truncate text-left"
        onClick={() => props.onClick()}
      >
        {props.label}
      </button>
      <Toggle
        label={props.label}
        on={props.on}
        disabled={(props.disabled ?? false) || readOnly()}
        onToggle={props.onToggle}
      />
      <button
        type="button"
        aria-label={props.label}
        class="shrink-0"
        onClick={() => props.onClick()}
      >
        <Icon name="chevron-right" class="size-[16px] text-phone-border" />
      </button>
    </div>
  );
}

export function EditRow(props: {
  stack: Stack;
  label: string;
  value: string;
  display?: string;
  masked?: boolean;
  placeholder?: string;
  footers?: string[];
  disabled?: boolean;
  validate?: (value: string) => string | null;
  onSubmit: (value: string) => void;
}) {
  return (
    <LinkRow
      label={props.label}
      value={props.masked ? sensitive(props.value) : (props.display ?? props.value)}
      disabled={props.disabled}
      onClick={() =>
        props.stack.push({
          title: props.label,
          render: () => (
            <TextEditPage
              value={props.value}
              placeholder={props.placeholder}
              footers={props.footers}
              validate={props.validate}
              onSubmit={props.onSubmit}
              onDone={() => props.stack.pop()}
            />
          ),
        })
      }
    />
  );
}

export function CommitSlider(props: {
  label?: string;
  value: number;
  minimum: number;
  maximum: number;
  step: number;
  disabled?: boolean;
  format?: (value: number) => string;
  onChange?: (value: number) => void;
  onSubmit: (value: number) => void;
}) {
  const [dragging, setDragging] = createSignal<number | null>(null);
  const value = () => dragging() ?? props.value;
  const readOnly = useReadOnly();
  return (
    <div class={`${rowClass} gap-3`}>
      <Show when={props.label}>
        <span class="shrink-0">{props.label}</span>
      </Show>
      <input
        type="range"
        class="min-w-0 flex-1 accent-white disabled:opacity-50"
        min={props.minimum}
        max={props.maximum}
        step={props.step}
        value={value()}
        disabled={props.disabled || readOnly()}
        onInput={(event) => {
          const next = Number(event.currentTarget.value);
          setDragging(next);
          props.onChange?.(next);
        }}
        onChange={(event) => {
          props.onSubmit(Number(event.currentTarget.value));
          setDragging(null);
        }}
      />
      <Show when={props.format}>
        {(format) => (
          <span class="min-w-[56px] shrink-0 text-right text-phone-border">
            {format()(value())}
          </span>
        )}
      </Show>
    </div>
  );
}

function BackgroundStreaming(props: StreamFormProps) {
  return (
    <Section footer={t("Live stream and record when the app is in background mode.")}>
      <ToggleLinkRow
        label={t("Background streaming")}
        on={props.stream.backgroundStreaming}
        onToggle={(on) =>
          props.edit((draft) => {
            draft.backgroundStreaming = on;
          })
        }
        onClick={() =>
          props.stack.push({
            title: t("Background streaming"),
            render: () => (
              <Section footer={t("Make built-in and USB cameras not freeze in background mode.")}>
                <ToggleRow
                  label={t("Show PiP")}
                  on={props.stream.backgroundStreamingPiP}
                  onToggle={(on) =>
                    props.edit((draft) => {
                      draft.backgroundStreamingPiP = on;
                    })
                  }
                />
              </Section>
            ),
          })
        }
      />
    </Section>
  );
}

export function StreamSettings(props: StreamFormProps) {
  const push = (title: string, render: () => JSX.Element) => props.stack.push({ title, render });
  const link = (label: string, render: () => JSX.Element, icon?: IconName, value?: string) => (
    <LinkRow label={label} icon={icon} value={value} onClick={() => push(label, render)} />
  );
  const protocol = () => streamProtocol(props.stream);
  return (
    <>
      <Section>
        <NameRow
          stack={props.stack}
          name={props.stream.name}
          existing={props.streams.names()}
          onSubmit={(name) =>
            props.edit((draft) => {
              draft.name = name;
            })
          }
        />
      </Section>
      <Section header={t("Destination")}>
        <Secret allowed={props.remote.secrets} label={t("URL")}>
          <LinkRow
            label={t("URL")}
            value={sensitive(props.stream.url)}
            onClick={() => push(t("URL"), () => <StreamUrlSettings {...props} />)}
          />
        </Secret>
        <Show when={showAllSettings(props)}>
          <Show when={protocol() === "SRT"}>
            {link(t("SRT(LA)"), () => (
              <SrtSettings {...props} />
            ))}
          </Show>
          <Show when={protocol() === "RTMP"}>
            {link(t("RTMP"), () => (
              <RtmpSettings {...props} />
            ))}
            <MultiStreamingRow {...props} />
          </Show>
          <Show when={protocol() === "RIST"}>
            {link(t("RIST"), () => (
              <RistSettings {...props} />
            ))}
          </Show>
          <Show when={protocol() === "WHIP"}>
            {link(t("WHIP"), () => (
              <WhipSettings {...props} />
            ))}
          </Show>
          <Show when={protocol() === "Mobcam"}>
            {link(t("Mobcam"), () => (
              <MobcamSettings {...props} />
            ))}
          </Show>
        </Show>
      </Section>
      <Section header={t("Media")}>
        {link(t("Video"), () => (
          <VideoSettings {...props} />
        ))}
        <Show when={showAllSettings(props)}>
          {link(t("Audio"), () => (
            <AudioSettings {...props} />
          ))}
          {link(
            t("Recording"),
            () => (
              <RecordingSettings {...props} />
            ),
            "record",
          )}
        </Show>
        {link(
          t("Replay"),
          () => (
            <ReplaySettings {...props} />
          ),
          "play",
        )}
        <Show when={showAllSettings(props)}>
          {link(
            t("Snapshot"),
            () => (
              <SnapshotSettings {...props} />
            ),
            "camera-aperture",
          )}
          {link(
            t("Preview stream"),
            () => (
              <PreviewStreamSettings {...props} />
            ),
            "video-circle",
          )}
        </Show>
        <ToggleRow
          label={t("Portrait")}
          on={props.stream.portrait}
          disabled={liveOrRecordingDisabled(props)}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.portrait = on;
            })
          }
        />
      </Section>
      <Section header={t("Streaming platforms")}>
        {link(
          t("Twitch"),
          () => (
            <TwitchSettings {...props} />
          ),
          "twitch",
          props.stream.twitchChannelName,
        )}
        {link(
          t("Kick"),
          () => (
            <KickSettings {...props} />
          ),
          "kick",
          props.stream.kickChannelName,
        )}
        {link(
          t("YouTube"),
          () => (
            <YouTubeSettings {...props} />
          ),
          "youtube",
          props.stream.youTubeHandle,
        )}
        {link(
          t("SOOP"),
          () => (
            <SoopSettings {...props} />
          ),
          undefined,
          props.stream.afreecaTvChannelName,
        )}
        {link(t("Open Streaming Platform"), () => (
          <OpenStreamingPlatformSettings {...props} />
        ))}
      </Section>
      <BackgroundStreaming {...props} />
      <Show when={protocol() === "Mobcam"}>
        <Section footer={t("Automatically go live when the app enters foreground.")}>
          <ToggleRow
            label={t("Auto go live")}
            on={props.stream.autoGoLive}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.autoGoLive = on;
              })
            }
          />
        </Section>
      </Show>
      <Section>
        <ToggleLinkRow
          label={t("OBS remote control")}
          on={props.stream.obsWebSocketEnabled}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.obsWebSocketEnabled = on;
            })
          }
          onClick={() =>
            push(t("OBS remote control"), () => <ObsRemoteControlSettings {...props} />)
          }
        />
        <Show when={showAllSettings(props)}>
          {link(t("Go live notification"), () => (
            <GoLiveNotificationSettings {...props} />
          ))}
          <ToggleLinkRow
            label={t("RealtimeIRL")}
            on={props.stream.realtimeIrlEnabled}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.realtimeIrlEnabled = on;
              })
            }
            onClick={() => push(t("RealtimeIRL"), () => <RealtimeIrlSettings {...props} />)}
          />
        </Show>
        {link(t("Emotes"), () => (
          <EmotesSettings {...props} />
        ))}
      </Section>
      <Show when={showAllSettings(props)}>
        <Section
          footer={t(
            "Estimated viewer delay, for example used to make it easier to take snapshots using the chat bot. It does not delay the stream.",
          )}
        >
          <EditRow
            stack={props.stack}
            label={t("Estimated viewer delay")}
            value={props.stream.estimatedViewerDelay.toFixed(1)}
            display={t("{count} sec", { count: Math.trunc(props.stream.estimatedViewerDelay) })}
            onSubmit={(value) => {
              const delay = parseFloat32(value);
              if (delay === null || delay < 0 || delay > 15) {
                return;
              }
              props.edit((draft) => {
                draft.estimatedViewerDelay = delay;
              });
            }}
          />
        </Section>
      </Show>
    </>
  );
}
