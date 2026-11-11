import { createSignal, For, onCleanup, Show } from "solid-js";
import Icon from "../Icon";
import { copyText } from "../CopyButton";
import { ButtonRow, ErrorText, Section, useReadOnly } from "../sceneEditor/Form";
import type { Stack } from "../sceneEditor/Stack";
import { liveOrRecording, type StreamFormProps } from "./StreamsSettings";
import { sameId } from "../../data/moblinSettings";
import { mobcamStreamPort } from "../../data/moblinStreamSettings";
import { cleanUrl, isValidUrl } from "../../data/streamUrls";
import { t } from "../../i18n";

export type UrlExample = { title: string; url: string };

export const rtmpExamples = (): UrlExample[] => [
  {
    title: t("Twitch"),
    url: "rtmp://arn03.contribute.live-video.net/app/live_123321_sdfopjfwjfpawjefpjawef",
  },
  { title: t("YouTube"), url: "rtmp://a.rtmp.youtube.com/live2/1bk2-0d03-9683-7k65-e4d3" },
  {
    title: t("Facebook"),
    url: "rtmps://live-api-s.facebook.com:443/rtmp/FB-11152522122511115-0-BctNCp9jzzz-AAA",
  },
  {
    title: t("Kick"),
    url: "rtmps://fa723fc1b171.global-contribute.live-video.net/sk_us-west-123hu43ui34hrkjh",
  },
  { title: t("RTMP server"), url: "rtmp://foobar.org:3321/app/5678" },
];

const srtExamples = (): UrlExample[] => [
  { title: t("OBS Media Source (SRT)"), url: "srt://134.20.342.12:5000" },
  {
    title: t("BELABOX cloud SRTLA"),
    url: "srtla://uk.srt.belabox.net:5000?streamid=NtlPUqXGFV4Bcm448wgc4fUuLdvDB3",
  },
  {
    title: t("BELABOX cloud SRT"),
    url: "srt://uk.srt.belabox.net:4000?streamid=NtlPUqXGFV4Bcm448wgc4fUuLdvDB3",
  },
  { title: t("SRTLA server"), url: "srtla://foobar.org:4432" },
  { title: t("SRT Live Server (SLS)"), url: "srt://120.12.32.12:4000?streamid=publish/live/feed" },
];

export const whipExamples = (): UrlExample[] => [
  { title: t("MediaMTX WHIP"), url: "whip://120.12.32.12:8889/mystream/whip" },
  { title: t("MESHCAST.IO WHIP"), url: "whips://de1.meshcast.io/whip/mystream" },
];

const mobcamExamples = (): UrlExample[] => [
  { title: t("Computer over USB cable"), url: `mobcam://localhost:${mobcamStreamPort}` },
];

function Examples(props: { examples: UrlExample[] }) {
  return (
    <For each={props.examples}>
      {(example) => (
        <Section header={example.title}>
          <div class="flex min-h-[44px] items-center gap-3 px-4 py-2 text-[15px]">
            <span class="min-w-0 flex-1 break-all">{example.url}</span>
            <button
              type="button"
              aria-label={t("Copy")}
              class="shrink-0 text-phone-tint"
              onClick={() => void copyText(example.url)}
            >
              <Icon name="copy" class="size-[18px]" />
            </button>
          </div>
        </Section>
      )}
    </For>
  );
}

export function UrlPage(props: {
  stack: Stack;
  value: string;
  placeholder: string;
  allowedSchemes?: string[];
  examples: UrlExample[];
  disabled: boolean;
  onSubmit: (url: string) => void;
}) {
  const readOnly = useReadOnly();
  const [value, setValue] = createSignal(props.value);
  const [changed, setChanged] = createSignal(false);
  let submitted = props.value;
  const error = () => (changed() ? isValidUrl(value(), props.allowedSchemes) : null);
  const submit = () => {
    if (!changed()) {
      return;
    }
    const url = cleanUrl(value());
    if (url === submitted || isValidUrl(url, props.allowedSchemes) !== null) {
      return;
    }
    submitted = url;
    props.onSubmit(url);
  };
  onCleanup(submit);
  return (
    <>
      <Section footer={<ErrorText text={error()} />}>
        <textarea
          class="min-h-[88px] w-full resize-none bg-transparent px-4 py-2 text-[17px] outline-none placeholder:text-phone-border disabled:opacity-50"
          value={value()}
          placeholder={props.placeholder}
          disabled={props.disabled || readOnly()}
          autocapitalize="off"
          autocorrect="off"
          spellcheck={false}
          autofocus
          onInput={(event) => {
            setValue(event.currentTarget.value.replaceAll("\n", ""));
            setChanged(true);
          }}
          onBlur={submit}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              submit();
              props.stack.pop();
            }
          }}
        />
      </Section>
      <Section>
        <ButtonRow
          label={t("Examples")}
          onClick={() =>
            props.stack.push({
              title: t("Examples"),
              render: () => <Examples examples={props.examples} />,
            })
          }
        />
      </Section>
    </>
  );
}

export function StreamUrlSettings(props: StreamFormProps) {
  return (
    <UrlPage
      stack={props.stack}
      value={props.stream.url}
      placeholder="srtla://foobar.org:4432"
      examples={[...rtmpExamples(), ...srtExamples(), ...whipExamples(), ...mobcamExamples()]}
      disabled={liveOrRecording(props.remote)}
      onSubmit={(url) =>
        props.edit((draft) => {
          draft.url = url;
        })
      }
    />
  );
}

export function MultiStreamingUrlSettings(props: StreamFormProps & { destinationId: string }) {
  const destination = () =>
    props.stream.multiStreaming.destinations.find((candidate) =>
      sameId(candidate.id, props.destinationId),
    );
  return (
    <Show when={destination()}>
      {(current) => (
        <UrlPage
          stack={props.stack}
          value={current().url}
          placeholder="rtmp://foobar.org:3321/app/5678"
          allowedSchemes={["rtmp", "rtmps"]}
          examples={rtmpExamples()}
          disabled={liveOrRecording(props.remote)}
          onSubmit={(url) =>
            props.edit((draft) => {
              const target = draft.multiStreaming.destinations.find((candidate) =>
                sameId(candidate.id, props.destinationId),
              );
              if (target) {
                target.url = url;
              }
            })
          }
        />
      )}
    </Show>
  );
}
