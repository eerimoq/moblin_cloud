import { createSignal, For, Match, Show, Switch } from "solid-js";
import {
  channelLabel,
  channelUrl,
  snapshotUrl,
  streamers,
  type Streamer,
  type StreamerChannel,
  type StreamerPlatform,
} from "../data/streamers";
import BrandIcon from "./BrandIcon";
import { t } from "../i18n";

const platformName: Record<StreamerPlatform, string> = {
  twitch: "Twitch",
  youtube: "YouTube",
  kick: "Kick",
};

/** A stable hue (0-360) per handle, for the avatar. */
const hue = (name: string) => {
  let hash = 0;
  for (const char of name) hash = (hash * 31 + char.codePointAt(0)!) % 360;
  return hash;
};

/** Up to two initials, so "sofia.cycles" becomes "SC" and "TokyoTom" becomes "T". */
const initials = (label: string) =>
  label
    .split(/[\s._-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]!.toUpperCase())
    .join("");

/** The channel's initials on a color of its own. */
function Initials(props: { channel: StreamerChannel }) {
  return (
    <span
      class="flex size-11 shrink-0 items-center justify-center rounded-full font-display text-base font-bold text-white"
      style={{
        background: `linear-gradient(135deg, hsl(${hue(props.channel.name)} 55% 50%), hsl(${hue(props.channel.name) + 40} 60% 30%))`,
      }}
    >
      {initials(channelLabel(props.channel))}
    </span>
  );
}

/**
 * The channel's profile image, when the backend has found one and it loads,
 * and the initials otherwise.
 */
function Avatar(props: { channel: StreamerChannel }) {
  const [broken, setBroken] = createSignal(false);
  return (
    <span aria-hidden="true">
      <Show
        when={!broken() && props.channel.avatar}
        fallback={<Initials channel={props.channel} />}
      >
        {(src) => (
          <img
            src={src()}
            alt=""
            loading="lazy"
            referrerpolicy="no-referrer"
            class="size-11 shrink-0 rounded-full bg-ghost object-cover"
            onError={() => setBroken(true)}
          />
        )}
      </Show>
    </span>
  );
}

function ChannelRow(props: { channel: StreamerChannel }) {
  return (
    <li>
      <a
        href={channelUrl(props.channel)}
        target="_blank"
        rel="noopener"
        class="group relative flex items-center gap-3"
      >
        <span
          role="tooltip"
          class="pointer-events-none absolute bottom-full left-0 z-10 mb-2 hidden w-max max-w-lg rounded-xl border-[1.5px] border-line bg-card px-4 py-2 font-bold text-ink shadow-lg group-hover:block"
        >
          {props.channel.title ??
            t("{name} on {platform}", {
              name: channelLabel(props.channel),
              platform: platformName[props.channel.platform],
            })}
        </span>
        <span class="relative shrink-0">
          <Avatar channel={props.channel} />
          <BrandIcon
            name={props.channel.platform}
            class="absolute -right-1 -bottom-1 size-5 border-2 border-card"
          />
        </span>
        <span class="flex min-w-0 flex-col">
          <span class="truncate font-bold text-ink group-hover:text-leaf">
            {channelLabel(props.channel)}
          </span>
          <Show when={props.channel.category}>
            <span class="truncate text-sm text-muted">{props.channel.category}</span>
          </Show>
        </span>
      </a>
    </li>
  );
}

function Snapshot(props: { streamer: Streamer }) {
  const [broken, setBroken] = createSignal(false);
  return (
    <Show when={!broken() && snapshotUrl(props.streamer)}>
      {(src) => (
        <img
          src={src()}
          alt={t("{name}'s stream", { name: channelLabel(props.streamer.channels[0]!) })}
          loading="lazy"
          referrerpolicy="no-referrer"
          class="aspect-video w-full rounded-2xl bg-ghost object-contain"
          onError={() => setBroken(true)}
        />
      )}
    </Show>
  );
}

function StreamerCard(props: { streamer: Streamer }) {
  return (
    <li class="flex flex-col gap-4 rounded-3xl border-[1.5px] border-line bg-card p-5">
      <Snapshot streamer={props.streamer} />
      <ul class="flex flex-col gap-3">
        <For each={props.streamer.channels}>{(channel) => <ChannelRow channel={channel} />}</For>
      </ul>
    </li>
  );
}

/** Stands in for a card while the backend answers. */
function PlaceholderCard() {
  return (
    <li class="animate-pulse rounded-3xl border-[1.5px] border-line bg-card p-5" aria-hidden="true">
      <div class="flex items-center gap-3">
        <span class="size-11 shrink-0 rounded-full bg-ghost" />
        <span class="h-4 w-28 rounded-full bg-ghost" />
      </div>
    </li>
  );
}

const placeholderCount = 6;

export default function Streamers() {
  return (
    <section id="streamers" class="bg-band pt-10 pb-24">
      <div class="mx-auto flex max-w-[1120px] flex-col gap-10 px-5 sm:px-8">
        <div class="flex flex-col gap-4 md:flex-row md:items-end md:justify-between md:gap-6">
          <div class="flex max-w-[640px] flex-col gap-2.5">
            <span class="font-display text-[15px] font-semibold uppercase tracking-[0.08em] text-leaf">
              {t("Streamers")}
            </span>
            <h2 class="text-4xl font-bold sm:text-[46px]">{t("Who's streaming with Moblin.")}</h2>
          </div>
          <p class="max-w-[440px] text-lg text-muted">
            {t(
              "IRL streamers who recently went live with Moblin. Drop by their channels and say hi.",
            )}
          </p>
        </div>
        <Switch>
          <Match when={streamers.loading}>
            <ul class="grid gap-4 sm:grid-cols-2 sm:gap-6 lg:grid-cols-3" aria-busy="true">
              <For each={Array.from({ length: placeholderCount })}>{() => <PlaceholderCard />}</For>
            </ul>
          </Match>
          <Match when={streamers.error}>
            <p class="text-lg text-muted">
              {t("Couldn't load streamers right now. Try again in a bit.")}
            </p>
          </Match>
          <Match when={streamers()?.length === 0}>
            <p class="text-lg text-muted">
              {t("Nobody has gone live with Moblin lately. Be the first!")}
            </p>
          </Match>
          <Match when={streamers()}>
            <ul class="grid gap-4 sm:grid-cols-2 sm:gap-6 lg:grid-cols-3">
              <For each={streamers()}>{(streamer) => <StreamerCard streamer={streamer} />}</For>
            </ul>
          </Match>
        </Switch>
      </div>
    </section>
  );
}
