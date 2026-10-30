import { createEffect, createMemo, createSignal, For, type JSX, onCleanup, Show } from "solid-js";
import Activity from "../components/Activity";
import Graphs from "../components/Graphs";
import Icon, { type IconName } from "../components/Icon";
import MoblinScreen, { topLeftIcons, topRightIcons } from "../components/MoblinScreen";
import MomMessage from "../components/MomMessage";
import { Spinner } from "../components/PageParts";
import Tiles, {
  type Leaf,
  type LeafActions,
  type Pane,
  type PanelKind,
  panelKinds,
  sanitize,
} from "../components/Tiles";
import { useDeviceFeed, usePreviewFeed, type DeviceFeed } from "../components/deviceFeed";
import {
  type Account,
  type Device,
  fetchMonitorLayout,
  saveMonitorLayout,
  type StatusItem,
} from "../data/account";
import { BackendError } from "../data/backend";
import { t } from "../i18n";

const defaultLayout = (): Pane => ({
  split: "row",
  sizes: [0.6, 0.4],
  children: [
    {
      split: "column",
      sizes: [0.65, 0.35],
      children: [{ panel: "phone" }, { panel: "graphs" }],
    },
    {
      split: "column",
      sizes: [0.7, 0.3],
      children: [{ panel: "activity" }, { panel: "status" }],
    },
  ],
});

const panelNames = (): Record<PanelKind, string> => ({
  phone: t("Phone"),
  activity: t("Activity"),
  graphs: t("Graphs"),
  status: t("Status"),
  message: t("Message to the streamer"),
  web: t("Web page"),
});

const panelIcons: Record<PanelKind, IconName> = {
  phone: "screen",
  activity: "list",
  graphs: "chart",
  status: "info",
  message: "chat",
  web: "globe",
};

const devicePanels: PanelKind[] = ["phone", "graphs", "status", "message"];

const layouts = new Map<string, Pane>();
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let unsaved: (() => void) | null = null;

const flush = () => {
  clearTimeout(saveTimer);
  unsaved?.();
  unsaved = null;
};

addEventListener("pagehide", flush);

const twitchChatUrl = (login: string) =>
  `https://www.twitch.tv/embed/${encodeURIComponent(login)}/chat?parent=${location.hostname}${
    matchMedia("(prefers-color-scheme: light)").matches ? "" : "&darkpopout"
  }`;

const menuItemClass =
  "flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[14px] text-ink transition-colors hover:bg-ghost disabled:opacity-50";

function PaneMenu(props: { leaf: Leaf; actions: LeafActions; onReset: () => void }) {
  const [open, setOpen] = createSignal(false);
  let menu: HTMLDivElement | undefined;

  createEffect(() => {
    if (!open()) {
      return;
    }
    const onPointerDown = (event: PointerEvent) => {
      if (!menu?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKey);
    onCleanup(() => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKey);
    });
  });

  const run = (action: () => void) => {
    setOpen(false);
    action();
  };

  return (
    <div ref={(element) => (menu = element)} class="relative">
      <button
        type="button"
        aria-label={t("Pane menu")}
        aria-haspopup="menu"
        aria-expanded={open()}
        onClick={() => setOpen(!open())}
        class="grid size-7 place-items-center rounded-full text-ink-2 transition-colors hover:bg-ghost-hover hover:text-ink aria-expanded:bg-ghost-hover"
      >
        <Icon name="more" class="size-4" />
      </button>
      <Show when={open()}>
        <div
          role="menu"
          class="absolute top-full right-0 z-30 mt-1 flex w-max min-w-48 flex-col rounded-xl border-[1.5px] border-line bg-card py-1.5 shadow-xl"
        >
          <button
            type="button"
            role="menuitem"
            class={menuItemClass}
            onClick={() => run(() => props.actions.split("row"))}
          >
            {t("Split right")}
          </button>
          <button
            type="button"
            role="menuitem"
            class={menuItemClass}
            onClick={() => run(() => props.actions.split("column"))}
          >
            {t("Split down")}
          </button>
          <Show when={props.leaf.panel === "web" && (props.leaf.url || props.leaf.twitchChat)}>
            <button
              type="button"
              role="menuitem"
              class={menuItemClass}
              onClick={() => run(() => props.actions.change({ panel: "web" }))}
            >
              {t("Change web page")}
            </button>
          </Show>
          <button
            type="button"
            role="menuitem"
            class={menuItemClass}
            disabled={!props.actions.remove}
            onClick={() => run(() => props.actions.remove?.())}
          >
            {t("Remove pane")}
          </button>
          <div class="my-1 border-t border-line" />
          <button
            type="button"
            role="menuitem"
            class={menuItemClass}
            onClick={() => run(props.onReset)}
          >
            {t("Reset layout")}
          </button>
        </div>
      </Show>
    </div>
  );
}

type PickerOption = { value: string; label: string; icon?: IconName; dot?: string };

function PanePicker(props: {
  label: string;
  value: string;
  options: PickerOption[];
  shown: string;
  icon?: IconName;
  plain?: boolean;
  onChange: (value: string) => void;
}) {
  const [open, setOpen] = createSignal(false);
  let picker: HTMLDivElement | undefined;
  let button: HTMLButtonElement | undefined;
  let list: HTMLDivElement | undefined;

  const items = () => [...(list?.querySelectorAll<HTMLButtonElement>("[role=option]") ?? [])];
  const focusItem = (index: number) => {
    const all = items();
    all[(index + all.length) % all.length]?.focus();
  };
  const close = () => {
    setOpen(false);
    button?.focus();
  };

  createEffect(() => {
    if (!open()) {
      return;
    }
    const selected = items().findIndex((item) => item.getAttribute("aria-selected") === "true");
    focusItem(Math.max(selected, 0));
    const onPointerDown = (event: PointerEvent) => {
      if (!picker?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    onCleanup(() => document.removeEventListener("pointerdown", onPointerDown));
  });

  const onKeyDown = (event: KeyboardEvent) => {
    const index = items().indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape") {
      close();
    } else if (event.key === "ArrowDown") {
      focusItem(index + 1);
    } else if (event.key === "ArrowUp") {
      focusItem(index - 1);
    } else if (event.key === "Home") {
      focusItem(0);
    } else if (event.key === "End") {
      focusItem(-1);
    } else if (event.key === "Tab") {
      setOpen(false);
      return;
    } else {
      return;
    }
    event.preventDefault();
  };

  return (
    <div ref={(element) => (picker = element)} class="relative min-w-0">
      <button
        ref={(element) => (button = element)}
        type="button"
        aria-label={props.label}
        aria-haspopup="listbox"
        aria-expanded={open()}
        onClick={() => setOpen(!open())}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            setOpen(true);
          }
        }}
        class={`flex h-7 max-w-full min-w-0 items-center gap-1 rounded-md px-1.5 text-ink transition-colors outline-none hover:bg-ghost focus-visible:bg-ghost aria-expanded:bg-ghost ${
          props.plain ? "" : "font-semibold"
        }`}
      >
        <Show when={props.icon}>
          {(icon) => <Icon name={icon()} class="size-4 shrink-0 text-muted" />}
        </Show>
        <span class="truncate">{props.shown}</span>
        <Icon
          name="chevron-down"
          class={`size-3 shrink-0 text-ink-2 transition-transform ${open() ? "rotate-180" : ""}`}
        />
      </button>
      <Show when={open()}>
        <div
          ref={(element) => (list = element)}
          role="listbox"
          aria-label={props.label}
          onKeyDown={onKeyDown}
          class="absolute top-full left-0 z-30 mt-1 flex max-h-80 w-max max-w-72 min-w-44 flex-col overflow-y-auto rounded-xl border-[1.5px] border-line bg-card py-1.5 shadow-xl"
        >
          <For each={props.options}>
            {(option) => (
              <button
                type="button"
                role="option"
                aria-selected={option.value === props.value}
                onClick={() => {
                  close();
                  if (option.value !== props.value) {
                    props.onChange(option.value);
                  }
                }}
                class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[14px] text-ink transition-colors outline-none hover:bg-ghost focus-visible:bg-ghost"
              >
                <Show when={option.icon}>
                  {(icon) => <Icon name={icon()} class="size-4 shrink-0 text-muted" />}
                </Show>
                <Show when={option.dot}>
                  {(dot) => <span class={`size-2 shrink-0 rounded-full ${dot()}`} />}
                </Show>
                <span class="min-w-0 flex-1 truncate">{option.label}</span>
                <Icon
                  name="check"
                  class={`size-4 shrink-0 text-leaf ${option.value === props.value ? "" : "invisible"}`}
                />
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

const connectionDot = (device: Device) =>
  device.connected ? "bg-leaf" : device.reconnecting ? "bg-amber" : "bg-muted";

function PaneFrame(props: {
  leaf: Leaf;
  actions: LeafActions;
  devices: Device[];
  device: Device | undefined;
  onReset: () => void;
  children: JSX.Element;
}) {
  const hasDevice = () => devicePanels.includes(props.leaf.panel) && props.devices.length > 1;
  return (
    <section class="flex size-full min-h-0 min-w-0 flex-col rounded-xl border border-line bg-card">
      <header
        {...props.actions.drag}
        class="flex h-9 shrink-0 items-center gap-1 border-b border-line pr-1 text-[13px]"
      >
        <span
          title={t("Drag to move this pane")}
          class="w-1 shrink-0 cursor-grab self-stretch active:cursor-grabbing"
        />
        <PanePicker
          label={t("Panel")}
          value={props.leaf.panel}
          shown={panelNames()[props.leaf.panel]}
          icon={panelIcons[props.leaf.panel]}
          options={panelKinds.map((kind) => ({
            value: kind,
            label: panelNames()[kind],
            icon: panelIcons[kind],
          }))}
          onChange={(panel) => props.actions.change({ panel: panel as PanelKind })}
        />
        <Show when={hasDevice()}>
          <span class="text-muted">·</span>
          <PanePicker
            plain
            label={t("Device")}
            value={props.leaf.device ?? ""}
            shown={
              props.leaf.device
                ? (props.device?.name ?? "")
                : t("Automatic ({name})", { name: props.device?.name ?? "" })
            }
            options={[
              {
                value: "",
                label: t("Automatic ({name})", { name: props.device?.name ?? "" }),
              },
              ...props.devices.map((device) => ({
                value: device.id,
                label: device.name,
                dot: connectionDot(device),
              })),
            ]}
            onChange={(device) =>
              props.actions.change({ ...props.leaf, device: device || undefined })
            }
          />
        </Show>
        <Show when={devicePanels.includes(props.leaf.panel) && props.device}>
          {(device) => (
            <span
              title={
                device().connected
                  ? t("Connected")
                  : device().reconnecting
                    ? t("Reconnecting")
                    : t("Not connected")
              }
              class={`size-2 shrink-0 rounded-full ${connectionDot(device())}`}
            />
          )}
        </Show>
        <span
          title={t("Drag to move this pane")}
          class="flex-1 cursor-grab self-stretch active:cursor-grabbing"
        />
        <PaneMenu leaf={props.leaf} actions={props.actions} onReset={props.onReset} />
      </header>
      <div class="relative min-h-0 flex-1 overflow-auto rounded-b-xl">{props.children}</div>
    </section>
  );
}

function Note(props: { children: JSX.Element }) {
  return <p class="px-3 py-3 text-[14px] text-muted">{props.children}</p>;
}

function PhonePanel(props: {
  session: string;
  page: string;
  device: Device;
  account: Account;
  feed: DeviceFeed;
}) {
  const previewFeed = createMemo(() =>
    props.account.preview ? usePreviewFeed(props.session, props.page, props.device.id) : null,
  );
  const online = () => props.feed.connected() && props.feed.streaming();
  const control = () => props.account.remoteControl === "control";
  return (
    <div class="flex h-full min-h-0 flex-col gap-2 p-2">
      <Show
        when={
          !online() || !props.account.preview || !control() || (!props.account.goLive && control())
        }
      >
        <div class="flex flex-wrap gap-x-3 text-[12px] text-muted">
          <Show when={props.feed.loaded() && !online()}>
            <span class="font-semibold text-amber">
              {t("{name} is not connected", { name: props.device.name })}
            </span>
          </Show>
          <Show when={!props.account.preview}>
            <span>{t("The streamer has not let you see the stream preview.")}</span>
          </Show>
          <Show when={!control()}>
            <span>{t("The streamer has not let you control Moblin.")}</span>
          </Show>
          <Show when={control() && !props.account.goLive}>
            <span>{t("The streamer has not let you go live.")}</span>
          </Show>
        </div>
      </Show>
      <MoblinScreen
        fit
        page={props.page}
        state={props.feed.state()}
        status={props.feed.status()}
        settings={props.feed.settings()}
        preview={props.feed.connected() ? (previewFeed()?.() ?? null) : null}
        goLive={props.account.goLive}
        remoteControl={control()}
        connected={online()}
        pending={props.feed.pending()}
        onControl={(control) => props.feed.control(control)}
      />
    </div>
  );
}

function StatusRow(props: { icon: IconName; item: StatusItem }) {
  return (
    <li
      class={`flex min-w-0 items-center gap-2 ${props.item.ok === false ? "text-danger" : "text-ink"}`}
      title={props.item.message}
    >
      <Icon name={props.icon} class="size-3.5 shrink-0 text-muted" />
      <span class="truncate">{props.item.message}</span>
    </li>
  );
}

function StatusPanel(props: { device: Device; feed: DeviceFeed }) {
  const status = () => props.feed.status();
  const state = () => props.feed.state();
  const flags = () => {
    const general = status().general;
    const items: { icon: IconName; item: StatusItem }[] = [];
    if (state().streaming !== undefined) {
      items.push({
        icon: "broadcast",
        item: { message: state().streaming ? t("Live") : t("Not live"), ok: true },
      });
    }
    if (state().recording) {
      items.push({ icon: "record", item: { message: t("Recording") } });
    }
    if (state().muted) {
      items.push({ icon: "mic-off", item: { message: t("Muted"), ok: false } });
    }
    if (general.batteryLevel !== undefined) {
      items.push({
        icon: "bolt",
        item: {
          message: general.batteryCharging
            ? t("Battery {level} %, charging", { level: general.batteryLevel })
            : t("Battery {level} %", { level: general.batteryLevel }),
          ok: general.batteryCharging || general.batteryLevel > 15,
        },
      });
    }
    if (general.flame) {
      items.push({
        icon: "flame",
        item: {
          message:
            general.flame === "Red"
              ? t("Very hot")
              : general.flame === "Yellow"
                ? t("Hot")
                : t("Thermal state normal"),
          ok: general.flame !== "Red",
        },
      });
    }
    if (general.wiFiSsid) {
      items.push({ icon: "connection", item: { message: general.wiFiSsid } });
    }
    return items;
  };
  const left = () => topLeftIcons.filter(([key]) => status().topLeft[key] !== undefined);
  const right = () => topRightIcons.filter(([key]) => status().topRight[key] !== undefined);

  return (
    <Show
      when={props.feed.connected()}
      fallback={
        <Note>
          {props.device.reconnecting
            ? t("{name} is reconnecting.", { name: props.device.name })
            : t("{name} is not connected", { name: props.device.name })}
        </Note>
      }
    >
      <ul class="grid grid-cols-[repeat(auto-fill,minmax(12rem,1fr))] gap-x-4 gap-y-1 p-3 text-[13px]">
        <For each={flags()}>{(entry) => <StatusRow icon={entry.icon} item={entry.item} />}</For>
        <For each={left()}>
          {([key, icon]) => <StatusRow icon={icon} item={status().topLeft[key]!} />}
        </For>
        <For each={right()}>
          {([key, icon]) => <StatusRow icon={icon} item={status().topRight[key]!} />}
        </For>
      </ul>
    </Show>
  );
}

function WebPanel(props: { leaf: Leaf; account: Account; actions: LeafActions }) {
  const [address, setAddress] = createSignal("");
  const source = () =>
    props.leaf.twitchChat
      ? props.account.twitch
        ? twitchChatUrl(props.account.twitch.login)
        : null
      : (props.leaf.url ?? null);
  const valid = () => {
    try {
      return new URL(address().trim()).protocol === "https:";
    } catch {
      return false;
    }
  };
  return (
    <Show
      when={source()}
      fallback={
        <div class="flex flex-col gap-3 p-3 text-[14px]">
          <Show when={props.leaf.twitchChat}>
            <p class="text-muted">{t("The streamer has no Twitch account linked.")}</p>
          </Show>
          <form
            class="flex gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              if (valid()) {
                props.actions.change({ panel: "web", url: address().trim() });
              }
            }}
          >
            <input
              type="url"
              value={address()}
              onInput={(event) => setAddress(event.currentTarget.value)}
              placeholder="https://"
              aria-label={t("Web page address")}
              class="h-9 min-w-0 flex-1 rounded-xl border-[1.5px] border-line bg-well px-3 text-[15px] text-ink placeholder:text-muted focus:border-leaf focus:outline-none"
            />
            <button
              type="submit"
              disabled={!valid()}
              class="h-9 rounded-full bg-moss px-4 font-display text-[15px] font-semibold text-white hover:bg-moss-bright disabled:opacity-60"
            >
              {t("Show")}
            </button>
          </form>
          <Show when={props.account.twitch}>
            <button
              type="button"
              onClick={() => props.actions.change({ panel: "web", twitchChat: true })}
              class="inline-flex h-9 items-center gap-2 self-start rounded-full border-[1.5px] border-ghost-line bg-ghost px-4 font-display text-[15px] font-semibold text-ink hover:bg-ghost-hover"
            >
              <Icon name="twitch" class="size-4" />
              {t("Twitch chat")}
            </button>
          </Show>
          <p class="text-[13px] text-muted">
            {t("Some sites do not allow being shown inside another page.")}
          </p>
        </div>
      }
    >
      {(src) => (
        <iframe
          src={src()}
          title={props.leaf.twitchChat ? t("Twitch chat") : t("Web page")}
          sandbox="allow-scripts allow-same-origin allow-popups allow-forms"
          class="absolute inset-0 size-full border-0"
        />
      )}
    </Show>
  );
}

function DevicePanel(props: {
  session: string;
  page: string;
  device: Device;
  account: Account;
  leaf: Leaf;
  onSignOut: () => void;
}) {
  const feed = useDeviceFeed(props.session, props.page, props.device.id, props.onSignOut);
  return (
    <Show when={props.leaf.panel} keyed>
      {(panel) => {
        switch (panel) {
          case "phone":
            return (
              <PhonePanel
                session={props.session}
                page={props.page}
                device={props.device}
                account={props.account}
                feed={feed}
              />
            );
          case "graphs":
            return <Graphs compact history={feed.history()} />;
          case "status":
            return <StatusPanel device={props.device} feed={feed} />;
          default:
            return (
              <div class="flex flex-col gap-2 p-3">
                <MomMessage
                  compact
                  session={props.session}
                  page={props.page}
                  device={props.device.id}
                  enabled={
                    props.account.remoteControl === "control" &&
                    feed.connected() &&
                    feed.streaming()
                  }
                  onSignOut={props.onSignOut}
                />
                <p class="text-[13px] text-muted">
                  {t("Moblin shows the message in its chat on the phone.")}
                </p>
              </div>
            );
        }
      }}
    </Show>
  );
}

export default function Monitor(props: {
  session: string;
  page: string;
  account: Account;
  assisted: boolean;
  recentChanges: number;
  onSignOut: () => void;
}) {
  const failed = (error: unknown) => {
    if (error instanceof BackendError && error.status === 401) {
      props.onSignOut();
    }
  };

  const [layout, setLayout] = createSignal<Pane | null>(layouts.get(props.page) ?? null);
  const page = props.page;
  fetchMonitorLayout(props.session, page)
    .then((saved) => sanitize(saved.layout) ?? defaultLayout())
    .catch((error) => {
      failed(error);
      return layouts.get(page) ?? defaultLayout();
    })
    .then((loaded) => {
      if (!unsaved) {
        layouts.set(page, loaded);
        setLayout(loaded);
      }
    });

  const change = (next: Pane) => {
    layouts.set(page, next);
    setLayout(next);
    const session = props.session;
    unsaved = () => void saveMonitorLayout(session, page, next).catch(failed);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 1000);
  };
  onCleanup(flush);

  const deviceOf = (leaf: Leaf) => {
    const devices = props.account.devices;
    return (
      (leaf.device ? devices.find((device) => device.id === leaf.device) : undefined) ??
      devices.find((device) => device.connected) ??
      devices[0]
    );
  };

  const body = (leaf: () => Leaf, actions: LeafActions) => {
    const device = () => deviceOf(leaf());
    return (
      <Show when={leaf().panel} keyed>
        {(panel) =>
          panel === "activity" ? (
            <Activity
              session={props.session}
              page={props.assisted ? props.page : undefined}
              kinds={leaf().kinds ?? []}
              recentChanges={props.recentChanges}
              onKinds={(kinds) =>
                actions.change({ ...leaf(), kinds: kinds.length > 0 ? kinds : undefined })
              }
              onSignOut={props.onSignOut}
            />
          ) : panel === "web" ? (
            <WebPanel leaf={leaf()} account={props.account} actions={actions} />
          ) : props.account.remoteControl === "none" ? (
            <Note>{t("The streamer has not let you use the remote control.")}</Note>
          ) : (
            <Show
              when={device()?.id}
              keyed
              fallback={<Note>{t("The streamer has no Moblin devices yet.")}</Note>}
            >
              {(id) => (
                <DevicePanel
                  session={props.session}
                  page={props.page}
                  device={props.account.devices.find((device) => device.id === id)!}
                  account={props.account}
                  leaf={leaf()}
                  onSignOut={props.onSignOut}
                />
              )}
            </Show>
          )
        }
      </Show>
    );
  };

  return (
    <Show when={layout()} fallback={<Spinner />}>
      {(shown) => (
        <Tiles
          layout={shown()}
          onChange={change}
          leaf={(leaf, actions) => (
            <PaneFrame
              leaf={leaf()}
              actions={actions}
              devices={props.account.devices}
              device={deviceOf(leaf())}
              onReset={() => change(defaultLayout())}
            >
              {body(leaf, actions)}
            </PaneFrame>
          )}
        />
      )}
    </Show>
  );
}
