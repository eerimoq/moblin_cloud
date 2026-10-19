import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import Icon from "../components/Icon";
import {
  Band,
  cardClass,
  CloudHeader,
  ghostButtonClass,
  Hero,
  Notice,
  Spinner,
} from "../components/PageParts";
import {
  fetchAccount,
  fetchAdminStats,
  readSession,
  saveSession,
  signInUrl,
  signOutEverywhere,
  takeSignInResult,
  type Account,
  type AdminStats,
} from "../data/account";
import { BackendError } from "../data/remoteControl";

type Counts = { today: number; week: number; total: number };

const periods: [keyof Counts, string][] = [
  ["today", "Today"],
  ["week", "7 days"],
  ["total", "All time"],
];

const zero: Counts = { today: 0, week: 0, total: 0 };

const add = (a: Counts, b: Counts): Counts => ({
  today: a.today + b.today,
  week: a.week + b.week,
  total: a.total + b.total,
});

const reasonLabels: Record<string, string> = {
  "unknown-key": "Unknown or replaced key",
  "feature-off": "Turned off on the page",
  offline: "Streamer offline",
  "requests-off": "Song requests off in Moblin",
  "no-wheel": "No wheel in Moblin",
  "invalid-link": "Not an Apple Music song link",
  "too-many-requests": "Over 10 song orders a minute on the page",
  "too-many-tests": "Over 10 tests a minute on the page",
  "song-not-found": "Song not on the streamer's Apple Music",
  "not-found": "Song not found by Moblin",
  "invalid-message": "Message over 200 characters",
  "amount-too-low": "Below the smallest suggested tip",
  "invalid-option": "Unknown spin option",
  "payments-unavailable": "PayPal or Stripe not connected",
  provider: "PayPal or Stripe failed",
  expired: "Order unknown or expired",
  "not-approved": "Viewer had not approved the payment",
  "already-used": "Payment already used",
  pending: "Provider still reviewing the payment",
  unknown: "Provider did not answer, settled later",
  "not-saved": "Database write failed",
  "moblin-timeout": "Moblin did not answer",
  "moblin-error": "Moblin answered with an error",
  crashed: "Backend error",
  account: "Not signed in",
  "not-signed-in": "Viewer not signed in with Twitch",
  "not-subscribed": "Viewer not subscribed on Twitch",
  "no-minutes-left": "Viewer used this month's minutes",
  "no-spins-left": "Viewer used this month's spins",
  "tier-excluded": "Viewer's tier gets nothing on the page",
  "too-long": "Song longer than the viewer's minutes left",
  "not-premium": "Not a premium account",
  "invalid-image": "Unreadable or too small image",
};

const problemReasons = [
  "provider",
  "unknown",
  "not-saved",
  "payments-unavailable",
  "moblin-timeout",
  "moblin-error",
  "not-found",
  "crashed",
];

type Problem = { label: string; hint: string; matches: (name: string, reason: string) => boolean };

const problems: Problem[] = [
  {
    label: "PayPal or Stripe failed",
    hint: "The provider could not start or complete a payment.",
    matches: (_, reason) => reason === "provider",
  },
  {
    label: "PayPal or Stripe not connected",
    hint: "A viewer tried to pay on a page whose streamer has no working payment account.",
    matches: (_, reason) => reason === "payments-unavailable",
  },
  {
    label: "Moblin did not answer",
    hint: "Moblin was connected but did not reply in time. It may need an update.",
    matches: (_, reason) => reason === "moblin-timeout",
  },
  {
    label: "Moblin answered with an error",
    hint: "Moblin refused a song lookup or a delivery. Deliveries are retried every 10 seconds, so one stuck request counts many times.",
    matches: (_, reason) => reason === "moblin-error",
  },
  {
    label: "Song not found by Moblin",
    hint: "The song was found before paying or the subscriber's request but not when adding it, so it was dropped.",
    matches: (_, reason) => reason === "not-found",
  },
  {
    label: "Backend error",
    hint: "Something crashed while handling a request.",
    matches: (_, reason) => reason === "crashed",
  },
  {
    label: "Given up after 7 days",
    hint: "Paid and subscriber requests that never reached a device in 7 days and were dropped.",
    matches: (name) => name.endsWith("Expiries"),
  },
];

type Kind = {
  kind: string;
  label: string;
  orders: string[];
  payments: string[];
  subscribers?: { event: string; reasons: string[] };
  tests: string[];
  deliveries: string[];
};

const kinds: Kind[] = [
  {
    kind: "song",
    label: "Songs",
    orders: [
      "feature-off",
      "payments-unavailable",
      "invalid-link",
      "too-many-requests",
      "offline",
      "requests-off",
      "song-not-found",
      "moblin-timeout",
      "moblin-error",
      "provider",
    ],
    payments: [
      "expired",
      "offline",
      "requests-off",
      "not-approved",
      "pending",
      "payments-unavailable",
      "provider",
    ],
    subscribers: {
      event: "subSongs",
      reasons: [
        "feature-off",
        "not-signed-in",
        "not-subscribed",
        "tier-excluded",
        "no-minutes-left",
        "too-long",
        "invalid-message",
        "invalid-link",
        "too-many-requests",
        "offline",
        "requests-off",
        "song-not-found",
        "moblin-timeout",
        "moblin-error",
        "crashed",
        "not-saved",
      ],
    },
    tests: [
      "account",
      "too-many-tests",
      "invalid-link",
      "offline",
      "requests-off",
      "song-not-found",
      "moblin-timeout",
      "moblin-error",
      "invalid-message",
      "crashed",
    ],
    deliveries: ["not-found", "moblin-error"],
  },
  {
    kind: "tip",
    label: "Tips",
    orders: [
      "feature-off",
      "payments-unavailable",
      "amount-too-low",
      "invalid-message",
      "provider",
    ],
    payments: ["expired", "not-approved", "pending", "payments-unavailable", "provider"],
    tests: ["account", "too-many-tests", "invalid-message", "crashed"],
    deliveries: ["moblin-error"],
  },
  {
    kind: "spin",
    label: "Wheel spins",
    orders: [
      "feature-off",
      "no-wheel",
      "offline",
      "payments-unavailable",
      "invalid-option",
      "invalid-message",
      "provider",
    ],
    payments: [
      "expired",
      "no-wheel",
      "not-approved",
      "pending",
      "payments-unavailable",
      "provider",
    ],
    subscribers: {
      event: "subSpins",
      reasons: [
        "feature-off",
        "not-signed-in",
        "not-subscribed",
        "tier-excluded",
        "no-spins-left",
        "no-wheel",
        "offline",
        "invalid-message",
        "crashed",
        "not-saved",
      ],
    },
    tests: ["account", "too-many-tests", "offline", "no-wheel", "invalid-message", "crashed"],
    deliveries: ["moblin-error"],
  },
];

type Tone = "normal" | "muted" | "problem";

type Row = { label: string; hint?: string; values: number[]; tone?: Tone };

const format = (value: number) => value.toLocaleString("en");

const plural = (count: number, one: string, many: string) =>
  `${format(count)} ${count === 1 ? one : many}`;

class Counters {
  private byName = new Map<string, Counts>();

  constructor(counters: AdminStats["counters"]) {
    for (const counter of counters) {
      this.byName.set(counter.name, counter);
    }
  }

  get(name: string): Counts {
    return this.byName.get(name) ?? zero;
  }

  sum(matches: (name: string, reason: string) => boolean): Counts {
    let sum = zero;
    for (const [name, counts] of this.byName) {
      const [event, reason] = name.split("Failed:");
      if (!event.includes("Test") && matches(event, reason ?? "")) {
        sum = add(sum, counts);
      }
    }
    return sum;
  }

  failures(event: string, expected: string[]): string[] {
    const prefix = `${event}Failed:`;
    const unexpected = [...this.byName.keys()]
      .filter((name) => name.startsWith(prefix))
      .map((name) => name.slice(prefix.length))
      .filter((reason) => !expected.includes(reason));
    return [...expected, ...unexpected];
  }
}

function Table(props: { title: string; hint?: string; columns: string[]; rows: Row[] }) {
  const cell = (tone: Tone | undefined, value: number, first: boolean) =>
    `w-28 px-4 py-2.5 text-right align-top font-mono text-[15px] tabular-nums ${
      tone === "problem" && value > 0 && first
        ? "font-bold text-amber"
        : tone === "muted"
          ? "text-muted"
          : ""
    }`;

  return (
    <section class="flex flex-col gap-3">
      <div class="flex flex-col gap-1">
        <h2 class="font-display text-2xl font-bold">{props.title}</h2>
        <Show when={props.hint}>
          <p class="text-[15px] text-muted text-pretty">{props.hint}</p>
        </Show>
      </div>
      <div class={`${cardClass} overflow-x-auto p-0! sm:p-0!`}>
        <table class="w-full">
          <thead>
            <tr class="border-b-[1.5px] border-line text-[13px] font-semibold tracking-[0.06em] text-muted uppercase">
              <th class="px-4 py-3" />
              <For each={props.columns}>
                {(column) => <th class="w-28 px-4 py-3 text-right">{column}</th>}
              </For>
            </tr>
          </thead>
          <tbody>
            <For each={props.rows}>
              {(row) => (
                <tr class="border-t border-line/50 transition-colors first:border-t-0 hover:bg-ghost-hover">
                  <td class="px-4 py-2.5 align-top">
                    <span
                      class={`block text-[15px] font-semibold ${
                        row.tone === "problem"
                          ? "text-amber-ink"
                          : row.tone === "muted"
                            ? "text-muted"
                            : "text-ink-2"
                      }`}
                    >
                      {row.label}
                    </span>
                    <Show when={row.hint}>
                      <span class="block text-[13px] text-muted text-pretty">{row.hint}</span>
                    </Show>
                  </td>
                  <For each={row.values}>
                    {(value, index) => (
                      <td class={cell(row.tone, value, index() === 0)}>{format(value)}</td>
                    )}
                  </For>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>
    </section>
  );
}

function Tile(props: { label: string; value: number; hint: string; attention?: boolean }) {
  return (
    <div class={`${cardClass} gap-1! p-5!`}>
      <span
        class={`font-display text-[36px] leading-none font-bold ${
          props.attention && props.value > 0 ? "text-amber" : ""
        }`}
      >
        {format(props.value)}
      </span>
      <span class="text-[15px] font-semibold text-ink-2">{props.label}</span>
      <span class="text-[13px] text-muted text-pretty">{props.hint}</span>
    </div>
  );
}

function Status(props: { counters: Counters }) {
  const issues = () => {
    const issues: string[] = [];
    for (const problem of problems) {
      const today = props.counters.sum(problem.matches).today;
      if (today > 0) {
        issues.push(`${problem.label}: ${plural(today, "time", "times")} today`);
      }
    }
    return issues;
  };

  return (
    <Show
      when={issues().length > 0}
      fallback={
        <div class="flex items-center gap-4 rounded-3xl border-[1.5px] border-moss/40 bg-moss/10 px-6 py-5">
          <span class="flex size-11 shrink-0 items-center justify-center rounded-full bg-moss text-white">
            <Icon name="check" class="size-6" />
          </span>
          <div class="flex flex-col gap-0.5">
            <span class="font-display text-xl font-bold">No problems today</span>
            <span class="text-[15px] text-ink-2">
              Nothing failed on our side or at PayPal, Stripe or Moblin since midnight UTC.
            </span>
          </div>
        </div>
      }
    >
      <div class="flex items-start gap-4 rounded-3xl border-[1.5px] border-amber-line bg-amber-bg px-6 py-5 text-amber-ink">
        <span class="flex size-11 shrink-0 items-center justify-center rounded-full bg-amber text-page">
          <Icon name="warning" class="size-6" />
        </span>
        <div class="flex flex-col gap-1.5">
          <span class="font-display text-xl font-bold">
            {plural(issues().length, "problem", "problems")} today
          </span>
          <ul class="flex flex-col gap-0.5 text-[15px]">
            <For each={issues()}>{(issue) => <li>{issue}</li>}</For>
          </ul>
        </div>
      </div>
    </Show>
  );
}

function Loaded(props: { stats: AdminStats }) {
  const counters = () => new Counters(props.stats.counters);
  const values = (counts: Counts) => periods.map(([period]) => counts[period]);
  const columns = periods.map(([, label]) => label);

  const step = (name: string, label: string, hint: string, expected: string[]): Row[] => [
    { label, hint, values: values(counters().get(name)) },
    ...counters()
      .failures(name, expected)
      .map((reason) => ({
        label: `${label} failed: ${reasonLabels[reason] ?? reason}`,
        values: values(counters().get(`${name}Failed:${reason}`)),
        tone: (problemReasons.includes(reason) ? "problem" : "muted") as Tone,
      })),
  ];

  const flow = (kind: Kind): Row[] => {
    const paid = (event: string, hint: string, expected: string[]) =>
      step(`${kind.kind}${event}`, event, hint, expected);
    return [
      ...paid(
        "Orders",
        "Payments started at PayPal or Stripe, before the viewer approves.",
        kind.orders,
      ),
      ...paid(
        "Payments",
        "Completed captures, so the streamer got the money and the request was queued. One order can fail several times before it succeeds.",
        kind.payments,
      ),
      ...(kind.subscribers
        ? step(
            kind.subscribers.event,
            "Subscribers",
            "Requests by Twitch subscribers, queued without a payment.",
            kind.subscribers.reasons,
          )
        : []),
      ...paid(
        "Deliveries",
        "Paid and subscriber requests Moblin accepted. A stuck request is retried every 10 seconds and counts each time.",
        kind.deliveries,
      ),
      {
        label: "Given up",
        hint: "Dropped after 7 days without reaching any device.",
        values: values(counters().get(`${kind.kind}Expiries`)),
        tone: "problem",
      },
    ];
  };

  const tests = (kind: Kind): Row[] => [
    ...step(
      `${kind.kind}Tests`,
      `${kind.label}: tests`,
      "Free test sends by streamers from their dashboard.",
      kind.tests,
    ),
    ...step(
      `${kind.kind}TestDeliveries`,
      `${kind.label}: deliveries`,
      "Test sends Moblin accepted.",
      kind.deliveries,
    ),
    {
      label: `${kind.label}: given up`,
      hint: "Test sends dropped after 7 days without reaching any device.",
      values: values(counters().get(`${kind.kind}TestExpiries`)),
      tone: "muted",
    },
  ];

  return (
    <div class="flex flex-col gap-10">
      <Status counters={counters()} />
      <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Tile
          label="Connected devices"
          value={props.stats.moblin.connectedDevices}
          hint="Moblin devices with an open Moblin Cloud connection."
        />
        <Tile
          label="Unfinished payments"
          value={props.stats.pending.orders}
          hint="Orders started in the last 3 hours whose payment has not completed."
        />
        <Tile
          label="Waiting for a device"
          value={props.stats.pending.deliveries}
          hint="Songs, tips and spins not yet accepted by every device they go to. Kept for 7 days."
          attention
        />
        <Tile
          label="Database bytes"
          value={props.stats.database.bytes}
          hint="Disk usage of the SQLite database and its WAL files: streamers, accounts, pages, orders, deliveries and counters."
        />
      </div>
      <Table
        title="Problems"
        hint="Failures on our side or at PayPal, Stripe or Moblin, across songs, tips and wheel spins. Viewer mistakes and streamers being offline are not counted here."
        columns={columns}
        rows={problems.map((problem) => ({
          label: problem.label,
          hint: problem.hint,
          values: values(counters().sum(problem.matches)),
          tone: "problem" as Tone,
        }))}
      />
      <For each={kinds}>
        {(kind) => <Table title={kind.label} columns={columns} rows={flow(kind)} />}
      </For>
      <Table
        title="Tests"
        hint="Free songs, tips and wheel spins streamers send themselves from the dashboard. Not counted as problems above."
        columns={columns}
        rows={kinds.flatMap(tests)}
      />
      <Table
        title="Moblin"
        columns={columns}
        rows={[
          {
            label: "Connections",
            hint: "Moblin connecting to Moblin Cloud with the key of a current device.",
            values: values(counters().get("moblinConnections")),
          },
          ...counters()
            .failures("moblinConnections", ["unknown-key"])
            .map((reason) => ({
              label: `Connections failed: ${reasonLabels[reason] ?? reason}`,
              values: values(counters().get(`moblinConnectionsFailed:${reason}`)),
              tone: "muted" as Tone,
            })),
        ]}
      />
      <Table
        title="Banner images"
        columns={columns}
        rows={[
          {
            label: "Uploads",
            hint: "Banner images uploaded by premium accounts for their tip pages.",
            values: values(counters().get("imageUploads")),
          },
          ...counters()
            .failures("imageUploads", ["invalid-image", "not-premium", "not-saved"])
            .map((reason) => ({
              label: `Uploads failed: ${reasonLabels[reason] ?? reason}`,
              values: values(counters().get(`imageUploadsFailed:${reason}`)),
              tone: "muted" as Tone,
            })),
        ]}
      />
      <Table
        title="Accounts"
        columns={["Now"]}
        rows={[
          {
            label: "Accounts",
            hint: "Twitch users who have signed in to Moblin Cloud.",
            values: [props.stats.accounts.total],
          },
          {
            label: "With a page",
            hint: "Accounts that have opened the dashboard and got a page and a first Moblin device.",
            values: [props.stats.accounts.withPage],
          },
          {
            label: "PayPal connected",
            hint: "Accounts with a PayPal merchant that can take payments.",
            values: [props.stats.accounts.paypal],
          },
          {
            label: "Stripe connected",
            hint: "Accounts whose Stripe account takes card payments.",
            values: [props.stats.accounts.stripe],
          },
          {
            label: "With a banner image",
            hint: "Pages showing a banner image uploaded by a premium account.",
            values: [props.stats.accounts.images],
          },
        ]}
      />
    </div>
  );
}

function Stats(props: { session: string; onSignOut: () => void }) {
  const [stats, setStats] = createSignal<AdminStats | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const refresh = async () => {
    try {
      setStats(await fetchAdminStats(props.session));
      setError(null);
    } catch (error) {
      if (error instanceof BackendError && error.status === 401) {
        props.onSignOut();
        return;
      }
      setError(
        error instanceof BackendError && error.status === 403
          ? "This account is not an admin."
          : "The statistics could not be loaded.",
      );
    }
  };

  onMount(() => {
    refresh();
    const timer = setInterval(refresh, 10_000);
    onCleanup(() => clearInterval(timer));
  });

  return (
    <Show
      when={error()}
      fallback={
        <Show when={stats()} fallback={<Spinner />}>
          {(stats) => <Loaded stats={stats()} />}
        </Show>
      }
    >
      {(error) => <Notice status>{error()}</Notice>}
    </Show>
  );
}

export default function Admin() {
  const [session, setSession] = createSignal<{ session: string; account: Account } | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [signInFailed, setSignInFailed] = createSignal(false);

  onMount(async () => {
    const result = takeSignInResult();
    if (result.session) {
      saveSession(result.session);
    }
    setSignInFailed(result.failed);
    const stored = readSession();
    if (stored) {
      try {
        setSession({ session: stored, account: await fetchAccount(stored) });
      } catch (error) {
        if (error instanceof BackendError && error.status === 401) {
          saveSession(null);
        }
      }
    }
    setLoading(false);
  });

  const signOut = () => {
    saveSession(null);
    setSession(null);
  };

  return (
    <>
      <CloudHeader>
        <Show
          when={session()}
          fallback={
            <Show when={!loading()}>
              <a
                href={signInUrl}
                class="inline-flex h-11 items-center justify-center gap-2 rounded-full bg-twitch px-5 font-display text-base font-semibold whitespace-nowrap text-white transition-colors hover:bg-twitch-bright"
              >
                <Icon name="twitch" class="size-5" />
                Sign in
              </a>
            </Show>
          }
        >
          {(current) => (
            <button
              type="button"
              class={`${ghostButtonClass} h-11! px-5! text-base!`}
              onClick={async () => {
                try {
                  await signOutEverywhere(current().session);
                } catch {}
                signOut();
              }}
            >
              Sign out
            </button>
          )}
        </Show>
      </CloudHeader>
      <main>
        <Hero title="Admin">
          <p class="text-lg text-ink-2 text-pretty sm:text-[21px]">
            How Moblin Cloud is doing. Numbers refresh every ten seconds; days are UTC.
          </p>
        </Hero>
        <Band tight>
          <Show when={!loading()} fallback={<Spinner />}>
            <Show
              when={session()}
              keyed
              fallback={
                <Show
                  when={signInFailed()}
                  fallback={
                    <p class="text-[17px] text-ink-2">
                      Sign in with an admin Twitch account to see the statistics.
                    </p>
                  }
                >
                  <Notice status>Signing in failed. Try again.</Notice>
                </Show>
              }
            >
              {(current) => (
                <Show
                  when={current.account.admin}
                  fallback={<Notice status>This account is not an admin.</Notice>}
                >
                  <Stats session={current.session} onSignOut={signOut} />
                </Show>
              )}
            </Show>
          </Show>
        </Band>
      </main>
    </>
  );
}
