import { For, onCleanup, Show } from "solid-js";
import { Empty, Loading, PanelView, type Remote } from "../MoblinParts";
import { ButtonRow, ErrorText, LinkRow, ReadOnly, Section } from "../sceneEditor/Form";
import { createStack, pageTitle, type Page, type Stack } from "../sceneEditor/Stack";
import { StreamSettings } from "./StreamSettings";
import { StreamWizard } from "./wizard/StreamWizard";
import { createStreams, type StreamItem, type Streams } from "./store";
import { Toggle } from "../MoblinParts";
import { makeUniqueName, sameId } from "../../data/moblinSettings";
import { cloneStream, type SettingsStream } from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

export type StreamFormProps = {
  streams: Streams;
  remote: Remote;
  stack: Stack;
  stream: SettingsStream;
  edit: (change: (stream: SettingsStream) => void) => void;
};

export const isLive = (remote: Remote) => remote.state.streaming === true;

export const isRecording = (remote: Remote) => remote.state.recording === true;

export const liveOrRecording = (remote: Remote) => isLive(remote) || isRecording(remote);

export const inUse = (props: { remote: Remote; stream: SettingsStream }) =>
  sameId(props.remote.settings?.currentStreamId, props.stream.id);

export const formProps = (
  streams: Streams,
  remote: Remote,
  stack: Stack,
  id: string,
): StreamFormProps | undefined => {
  const stream = streams.stream(id);
  if (!stream) {
    return undefined;
  }
  return {
    streams,
    remote,
    stack,
    get stream() {
      return streams.stream(id) ?? stream;
    },
    edit: (change) => streams.edit(id, change),
  };
};

function StreamPage(props: { streams: Streams; remote: Remote; stack: Stack; id: string }) {
  void props.streams.load(props.id);
  return (
    <Show
      when={formProps(props.streams, props.remote, props.stack, props.id)}
      fallback={
        <Show
          when={props.streams.loading(props.id)}
          fallback={<ErrorText text={props.streams.state.error} />}
        >
          <Loading />
        </Show>
      }
    >
      {(form) => <StreamSettings {...form()} />}
    </Show>
  );
}

function StreamRow(props: { streams: Streams; remote: Remote; stack: Stack; item: StreamItem }) {
  const open = () =>
    props.stack.push({
      title: () =>
        props.streams.list().find((item) => item.id === props.item.id)?.name ?? props.item.name,
      render: () => (
        <StreamPage
          streams={props.streams}
          remote={props.remote}
          stack={props.stack}
          id={props.item.id}
        />
      ),
    });
  const duplicate = async () => {
    const stream = await props.streams.load(props.item.id);
    if (stream) {
      props.streams.add({
        ...cloneStream(stream),
        name: makeUniqueName(stream.name, props.streams.names()),
      });
    }
  };
  return (
    <div class="flex items-center">
      <div class="flex items-center pl-4">
        <Toggle
          label={props.item.name}
          on={props.item.enabled}
          disabled={props.item.enabled || liveOrRecording(props.remote) || props.remote.readOnly}
          onToggle={() => props.streams.select(props.item.id)}
        />
      </div>
      <div class="min-w-0 flex-1">
        <LinkRow label={props.item.name} onClick={open} />
      </div>
      <Show when={props.remote.secrets && !props.remote.readOnly}>
        <button
          type="button"
          class="shrink-0 px-2 text-[15px] text-phone-tint disabled:opacity-35"
          disabled={props.streams.loading(props.item.id)}
          onClick={() => void duplicate()}
        >
          {t("Duplicate")}
        </button>
      </Show>
      <button
        type="button"
        class="shrink-0 px-2 pr-4 text-[15px] text-phone-live disabled:opacity-35"
        disabled={props.item.enabled || props.remote.readOnly}
        onClick={() => props.streams.remove(props.item.id)}
      >
        {t("Delete")}
      </button>
    </div>
  );
}

const highestBitrate = (remote: Remote) => {
  const bitrates = remote.settings?.bitratePresets.map((preset) => preset.bitrate) ?? [];
  return bitrates.length > 0 ? Math.max(...bitrates) : undefined;
};

function StreamsList(props: { streams: Streams; remote: Remote; stack: Stack }) {
  return (
    <Show when={props.remote.settings} fallback={<Loading />}>
      <Section footer={<ErrorText text={props.streams.state.error} />}>
        <For each={props.streams.list()}>
          {(item) => (
            <StreamRow
              streams={props.streams}
              remote={props.remote}
              stack={props.stack}
              item={item}
            />
          )}
        </For>
        <Show when={props.remote.secrets && !props.remote.readOnly}>
          <ButtonRow
            label={t("Create")}
            disabled={liveOrRecording(props.remote)}
            onClick={() =>
              props.stack.push({
                title: t("Create stream"),
                render: () => (
                  <StreamWizard
                    streams={props.streams}
                    stack={props.stack}
                    highestBitrate={highestBitrate(props.remote)}
                    onCreated={(stream) => {
                      props.stack.reset();
                      props.streams.add(stream);
                      props.streams.select(stream.id);
                    }}
                  />
                ),
              })
            }
          />
        </Show>
      </Section>
    </Show>
  );
}

export function StackPanel(props: {
  stack: Stack;
  root: Page;
  back: { label: string; onBack: () => void };
}) {
  const top = () => props.stack.pages().at(-1) ?? props.root;
  const previous = () => {
    const pages = props.stack.pages();
    return pages.length === 0 ? undefined : pages.length === 1 ? props.root : pages.at(-2);
  };
  return (
    <PanelView
      title={pageTitle(top())}
      back={(() => {
        const page = previous();
        return page ? { label: pageTitle(page), onBack: () => props.stack.pop() } : props.back;
      })()}
    >
      <Show when={top()} keyed>
        {(page) => page.render()}
      </Show>
    </PanelView>
  );
}

export default function StreamsSettings(props: { remote: Remote; onBack: () => void }) {
  const stack = createStack();
  const request = props.remote.request;
  if (!request) {
    return (
      <PanelView title={t("Streams")} back={{ label: t("Settings"), onBack: props.onBack }}>
        <Empty text={t("This Moblin can't be configured remotely.")} />
      </PanelView>
    );
  }
  const streams = createStreams(
    request,
    props.remote.control,
    () => props.remote.settings,
    () => props.remote.readOnly,
  );
  onCleanup(() => {
    void streams.flush().finally(() => streams.dispose());
  });
  return (
    <ReadOnly.Provider value={() => props.remote.readOnly}>
      <StackPanel
        stack={stack}
        root={{
          title: t("Streams"),
          render: () => <StreamsList streams={streams} remote={props.remote} stack={stack} />,
        }}
        back={{ label: t("Settings"), onBack: props.onBack }}
      />
    </ReadOnly.Provider>
  );
}
