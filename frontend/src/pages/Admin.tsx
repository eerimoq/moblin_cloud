import { createSignal, For, onCleanup, onMount, Show, type JSX } from "solid-js";
import Icon, { type IconName } from "../components/Icon";
import { poll } from "../poll";
import {
  AccountMenu,
  Band,
  cardClass,
  CloudHeader,
  Hero,
  Notice,
  Spinner,
  TwitchSignIn,
} from "../components/PageParts";
import { createAccountSession, fetchAdminStats, type AdminStats } from "../data/account";
import { BackendError } from "../data/backend";

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
  "over-max-length": "Song longer than the page's longest song",
  "not-moderator": "Not the streamer or a moderator they turned on",
  "invalid-command": "Unknown queue command or song",
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

const providers = [
  { slug: "paypal", label: "PayPal" },
  { slug: "stripe", label: "Stripe" },
];

const problems: Problem[] = [
  ...providers.map((provider) => ({
    label: `${provider.label} failed`,
    hint: `${provider.label} could not start or complete a payment, or did not answer.`,
    matches: (name: string, reason: string) =>
      name.startsWith(provider.slug) && (reason === "provider" || reason === "unknown"),
  })),
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
  moderators?: { event: string; label: string; hint: string; reasons: string[] }[];
  tests: string[];
  deliveries: string[];
};

const captureReasons = [
  "payments-unavailable",
  "not-saved",
  "not-approved",
  "pending",
  "provider",
  "unknown",
  "crashed",
];

const kinds: Kind[] = [
  {
    kind: "tip",
    label: "Tips",
    orders: [
      "feature-off",
      "payments-unavailable",
      "amount-too-low",
      "invalid-message",
      "provider",
      "not-saved",
    ],
    payments: ["expired", ...captureReasons],
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
      "not-saved",
    ],
    payments: ["expired", "no-wheel", ...captureReasons],
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
      "over-max-length",
      "moblin-timeout",
      "moblin-error",
      "provider",
      "not-saved",
    ],
    payments: ["expired", "offline", "requests-off", ...captureReasons],
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
        "over-max-length",
        "moblin-timeout",
        "moblin-error",
        "crashed",
        "not-saved",
      ],
    },
    moderators: [
      {
        event: "modSongs",
        label: "Moderators",
        hint: "Songs added for free by the streamer's Twitch moderators from the dashboard.",
        reasons: [
          "account",
          "not-moderator",
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
      {
        event: "moderatorMusic",
        label: "Queue changes",
        hint: "Play, pause, play now, remove and clear sent to Moblin by streamers and moderators from the dashboard.",
        reasons: [
          "account",
          "not-moderator",
          "invalid-command",
          "offline",
          "moblin-timeout",
          "moblin-error",
        ],
      },
    ],
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
];

type Failure = { reason: string; label: string; counts: Counts; problem: boolean };

type Step = {
  label: string;
  hint?: string;
  counts: Counts;
  failures: Failure[];
  problem?: boolean;
};

const format = (value: number) => value.toLocaleString("en");

const formatBytes = (bytes: number) =>
  bytes >= 1_000_000_000
    ? `${(bytes / 1_000_000_000).toFixed(2)} GB`
    : bytes >= 1_000_000
      ? `${(bytes / 1_000_000).toFixed(1)} MB`
      : `${(bytes / 1_000).toFixed(0)} kB`;

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

  failures(event: string, expected: string[], labels: Record<string, string> = {}): Failure[] {
    const prefix = `${event}Failed:`;
    const unexpected = [...this.byName.keys()]
      .filter((name) => name.startsWith(prefix))
      .map((name) => name.slice(prefix.length))
      .filter((reason) => !expected.includes(reason));
    return [...expected, ...unexpected].map((reason) => ({
      reason,
      label: labels[reason] ?? reasonLabels[reason] ?? reason,
      counts: this.get(`${prefix}${reason}`),
      problem: problemReasons.includes(reason),
    }));
  }

  step(
    name: string,
    label: string,
    hint: string,
    expected: string[],
    labels?: Record<string, string>,
  ): Step {
    return {
      label,
      hint,
      counts: this.get(name),
      failures: this.failures(name, expected, labels),
    };
  }
}

function Cells(props: { counts: Counts; class: string; attention?: boolean }) {
  return (
    <For each={periods}>
      {([period], index) => {
        const value = () => props.counts[period];
        return (
          <td
            class={`px-2 text-right align-top font-mono tabular-nums sm:px-4 ${props.class} ${
              value() === 0
                ? "text-muted/40"
                : props.attention && index() === 0
                  ? "font-bold text-amber"
                  : ""
            }`}
          >
            {format(value())}
          </td>
        );
      }}
    </For>
  );
}

function Info(props: { icon?: IconName; label?: string; children: JSX.Element }) {
  const [open, setOpen] = createSignal(false);
  return (
    <span class="relative">
      <button
        type="button"
        aria-label={props.label ?? "What is this?"}
        aria-expanded={open()}
        class="peer ml-1.5 inline-flex size-6 items-center justify-center rounded-full align-middle text-muted transition-colors hover:bg-ghost hover:text-ink aria-expanded:bg-ghost aria-expanded:text-ink"
        onClick={() => setOpen(!open())}
        onBlur={() => setOpen(false)}
      >
        <Icon name={props.icon ?? "info"} class="size-4" />
      </button>
      <span
        role="tooltip"
        class={`absolute top-full left-0 z-10 mt-1.5 w-max max-w-[min(24rem,calc(100vw-4rem))] rounded-2xl border-[1.5px] border-line bg-card px-4 py-3 text-left font-body text-[14px] leading-snug font-normal text-ink-2 shadow-xl text-pretty ${
          open() ? "block" : "hidden peer-hover:block peer-focus-visible:block"
        }`}
      >
        {props.children}
      </span>
    </span>
  );
}

function StepTable(props: { steps: Step[] }) {
  return (
    <div class={`${cardClass} gap-0! p-0! sm:p-0!`}>
      <table class="w-full">
        <thead>
          <tr class="text-[12px] font-semibold tracking-[0.06em] text-muted uppercase">
            <th class="px-4 pt-3 pb-2 sm:px-5" />
            <For each={periods}>
              {([, label]) => (
                <th class="w-14 px-2 pt-3 pb-2 text-right whitespace-nowrap sm:w-24 sm:px-4">
                  {label}
                </th>
              )}
            </For>
          </tr>
        </thead>
        <For each={props.steps}>
          {(step) => {
            const seen = () =>
              step.failures
                .filter((failure) => failure.counts.total > 0)
                .sort(
                  (a, b) =>
                    Number(b.problem) - Number(a.problem) || b.counts.total - a.counts.total,
                );
            const never = () => step.failures.filter((failure) => failure.counts.total === 0);
            const failed = () => seen().reduce((sum, failure) => add(sum, failure.counts), zero);
            return (
              <tbody class="border-t-[1.5px] border-line [&>tr:last-child>td]:pb-3">
                <tr>
                  <td
                    class={`px-4 pt-3 pb-2 align-top text-[16px] font-semibold sm:px-5 ${
                      step.problem && step.counts.total > 0 ? "text-amber-ink" : "text-ink"
                    }`}
                  >
                    {step.label}
                    <Show when={step.hint}>{(hint) => <Info>{hint()}</Info>}</Show>
                    <Show when={never().length > 0}>
                      <Info icon="failed" label="Never failed with">
                        <span class="font-semibold">Never failed with</span>
                        <ul class="mt-1 list-disc pl-4">
                          <For each={never()}>{(failure) => <li>{failure.label}</li>}</For>
                        </ul>
                      </Info>
                    </Show>
                  </td>
                  <Cells
                    counts={step.counts}
                    class="pt-3 pb-2 text-[17px] font-semibold"
                    attention={step.problem}
                  />
                </tr>
                <Show when={seen().length > 0}>
                  <tr>
                    <td class="px-4 pt-1 pb-0.5 text-[12px] font-semibold tracking-[0.06em] text-muted uppercase sm:px-5">
                      Failed
                    </td>
                    <Cells counts={failed()} class="pt-1 pb-0.5 text-[13px] text-muted" />
                  </tr>
                  <For each={seen()}>
                    {(failure) => (
                      <tr>
                        <td class="px-4 py-0.5 text-[14px] sm:px-5">
                          <span
                            class={`flex items-baseline gap-2 ${
                              failure.problem ? "text-amber-ink" : "text-ink-2"
                            }`}
                          >
                            <span
                              class={`size-1.5 shrink-0 translate-y-[-2px] rounded-full ${
                                failure.problem ? "bg-amber" : "bg-muted/40"
                              }`}
                            />
                            {failure.label}
                          </span>
                        </td>
                        <Cells
                          counts={failure.counts}
                          class="py-0.5 text-[14px]"
                          attention={failure.problem}
                        />
                      </tr>
                    )}
                  </For>
                </Show>
              </tbody>
            );
          }}
        </For>
      </table>
    </div>
  );
}

function Section(props: { id: string; title: string; hint?: string; children: JSX.Element }) {
  return (
    <section id={props.id} class="flex scroll-mt-24 flex-col gap-3">
      <h2 class="font-display text-2xl font-bold">
        {props.title}
        <Show when={props.hint}>{(hint) => <Info>{hint()}</Info>}</Show>
      </h2>
      {props.children}
    </section>
  );
}

function Tile(props: { label: string; value: string; hint: string; attention?: boolean }) {
  return (
    <div class={`${cardClass} gap-1! p-5!`}>
      <span
        class={`font-display text-[32px] leading-none font-bold ${
          props.attention ? "text-amber" : ""
        }`}
      >
        {props.value}
      </span>
      <span class="self-start text-[15px] font-semibold text-ink-2">
        {props.label}
        <Info>{props.hint}</Info>
      </span>
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

type NavItem = { id: string; label: string; caption: string; attention?: boolean };

function Nav(props: { items: NavItem[] }) {
  return (
    <nav
      aria-label="Sections"
      class="flex flex-wrap gap-2 lg:sticky lg:top-24 lg:flex-col lg:flex-nowrap"
    >
      <For each={props.items}>
        {(item) => (
          <a
            href={`#${item.id}`}
            class="flex items-baseline gap-2 rounded-2xl border-[1.5px] border-ghost-line bg-ghost px-4 py-2 text-ink-2 transition-colors hover:bg-ghost-hover hover:text-ink lg:border-transparent lg:bg-transparent lg:py-2.5 lg:hover:bg-ghost"
          >
            <span class="font-display text-[16px] font-semibold">{item.label}</span>
            <span
              class={`text-[13px] whitespace-nowrap lg:ml-auto ${
                item.attention ? "font-semibold text-amber" : "text-muted"
              }`}
            >
              {item.caption}
            </span>
          </a>
        )}
      </For>
    </nav>
  );
}

function Loaded(props: { stats: AdminStats }) {
  const counters = () => new Counters(props.stats.counters);

  const flow = (kind: Kind): Step[] => {
    const paid = (event: string, hint: string, expected: string[]) =>
      counters().step(`${kind.kind}${event}`, event, hint, expected);
    return [
      paid(
        "Orders",
        "Payments started at PayPal or Stripe, before the viewer approves.",
        kind.orders,
      ),
      paid(
        "Payments",
        "Completed captures, so the streamer got the money and the request was queued. One order can fail several times before it succeeds.",
        kind.payments,
      ),
      ...(kind.subscribers
        ? [
            counters().step(
              kind.subscribers.event,
              "Subscribers",
              "Requests by Twitch subscribers, queued without a payment.",
              kind.subscribers.reasons,
            ),
          ]
        : []),
      ...(kind.moderators ?? []).map((moderators) =>
        counters().step(moderators.event, moderators.label, moderators.hint, moderators.reasons),
      ),
      paid(
        "Deliveries",
        "Paid, subscriber and moderator requests Moblin accepted. A stuck request is retried every 10 seconds and counts each time.",
        kind.deliveries,
      ),
      {
        label: "Given up",
        hint: "Dropped after 7 days without reaching any device.",
        counts: counters().get(`${kind.kind}Expiries`),
        failures: [],
        problem: true,
      },
    ];
  };

  const tests = (kind: Kind): Step[] => [
    counters().step(
      `${kind.kind}Tests`,
      "Tests",
      "Free test sends by streamers from their dashboard.",
      kind.tests,
    ),
    counters().step(
      `${kind.kind}TestDeliveries`,
      "Deliveries",
      "Test sends Moblin accepted.",
      kind.deliveries,
    ),
    {
      label: "Given up",
      hint: "Test sends dropped after 7 days without reaching any device.",
      counts: counters().get(`${kind.kind}TestExpiries`),
      failures: [],
    },
  ];

  const provider = ({ slug, label }: (typeof providers)[number]): Step[] => {
    const labels = {
      provider: `${label} failed`,
      pending: `${label} still reviewing the payment`,
      unknown: `${label} did not answer, settled later`,
    };
    return [
      counters().step(
        `${slug}Orders`,
        "Orders",
        `Payments started at ${label}, after the page's own checks passed.`,
        ["provider", "not-saved"],
        labels,
      ),
      counters().step(
        `${slug}Payments`,
        "Payments",
        `Payments ${label} completed, however they were found. Only what happened after ${label} was asked to capture counts as failed here.`,
        ["not-approved", "pending", "provider", "unknown", "expired"],
        labels,
      ),
    ];
  };

  const problemsToday = () =>
    problems.filter((problem) => counters().sum(problem.matches).today > 0).length;
  const today = (name: string) => counters().get(name).today;

  const navItems = (): NavItem[] => [
    {
      id: "problems",
      label: "Problems",
      caption: `${format(problemsToday())} today`,
      attention: problemsToday() > 0,
    },
    { id: "accounts", label: "Accounts", caption: format(props.stats.accounts.total) },
    {
      id: "providers",
      label: "Providers",
      caption: providers
        .map(({ slug, label }) => `${format(today(`${slug}Payments`))} ${label}`)
        .join(", "),
    },
    ...kinds.map((kind) => ({
      id: kind.kind,
      label: kind.label,
      caption: `${format(today(`${kind.kind}Payments`))} paid today`,
    })),
    {
      id: "tests",
      label: "Tests",
      caption: `${format(kinds.reduce((sum, kind) => sum + today(`${kind.kind}Tests`), 0))} today`,
    },
    { id: "moblin", label: "Moblin", caption: `${format(today("moblinConnections"))} today` },
  ];

  const accounts = [
    ["Accounts", "Twitch users who have signed in to Moblin Cloud.", props.stats.accounts.total],
    [
      "Premium",
      "Signed in accounts with the premium features: the admins and the premium accounts list.",
      props.stats.accounts.premium,
    ],
    ["PayPal connected", "A PayPal merchant that can take payments.", props.stats.accounts.paypal],
    ["Stripe connected", "A Stripe account that takes card payments.", props.stats.accounts.stripe],
  ] as const;

  return (
    <div class="flex flex-col gap-8">
      <Status counters={counters()} />
      <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Tile
          label="Connected devices"
          value={format(props.stats.moblin.connectedDevices)}
          hint="Moblin devices with an open Moblin Cloud connection."
        />
        <Tile
          label="Unfinished payments"
          value={format(props.stats.pending.orders)}
          hint="Orders started in the last 3 hours whose payment has not completed."
        />
        <Tile
          label="Waiting for a device"
          value={format(props.stats.pending.deliveries)}
          hint="Tips, spins and songs no device has received yet. A tip one of several devices got is not counted. Kept for 7 days."
          attention={props.stats.pending.deliveries > 0}
        />
        <Tile
          label="Database"
          value={formatBytes(props.stats.database.bytes)}
          hint={`${format(props.stats.database.bytes)} bytes on disk for the SQLite database and its WAL files.`}
        />
      </div>
      <div class="grid gap-6 lg:grid-cols-[220px_minmax(0,1fr)] lg:items-start">
        <Nav items={navItems()} />
        <div class="flex min-w-0 flex-col gap-10">
          <Section
            id="problems"
            title="Problems"
            hint="Failures on our side or at PayPal, Stripe or Moblin, across tips, wheel spins and songs. Viewer mistakes and streamers being offline are not counted here."
          >
            <StepTable
              steps={problems.map((problem) => ({
                label: problem.label,
                hint: problem.hint,
                counts: counters().sum(problem.matches),
                failures: [],
                problem: true,
              }))}
            />
          </Section>
          <Section id="accounts" title="Accounts" hint="Right now, not per day.">
            <div class="grid gap-4 sm:grid-cols-3 lg:grid-cols-5">
              <For each={accounts}>
                {([label, hint, value]) => <Tile label={label} value={format(value)} hint={hint} />}
              </For>
            </div>
          </Section>
          <Section
            id="providers"
            title="Providers"
            hint="Payments by provider, across tips, wheel spins and songs, so PayPal and Stripe can be compared. The failures before a provider is involved are under each kind below."
          >
            <div class="flex flex-col gap-5">
              <For each={providers}>
                {(item) => (
                  <div class="flex flex-col gap-2">
                    <h3 class="text-[17px] font-semibold text-ink-2">{item.label}</h3>
                    <StepTable steps={provider(item)} />
                  </div>
                )}
              </For>
            </div>
          </Section>
          <For each={kinds}>
            {(kind) => (
              <Section
                id={kind.kind}
                title={kind.label}
                hint="Every step shows its count, then how often it failed and why. Amber reasons are our, PayPal's, Stripe's or Moblin's fault; grey ones are viewer mistakes or offline streamers."
              >
                <StepTable steps={flow(kind)} />
              </Section>
            )}
          </For>
          <Section
            id="tests"
            title="Tests"
            hint="Free tips, wheel spins and songs streamers send themselves from the dashboard. Not counted as problems above."
          >
            <div class="flex flex-col gap-5">
              <For each={kinds}>
                {(kind) => (
                  <div class="flex flex-col gap-2">
                    <h3 class="text-[17px] font-semibold text-ink-2">{kind.label}</h3>
                    <StepTable steps={tests(kind)} />
                  </div>
                )}
              </For>
            </div>
          </Section>
          <Section id="moblin" title="Moblin">
            <StepTable
              steps={[
                counters().step(
                  "moblinConnections",
                  "Connections",
                  "Moblin connecting to Moblin Cloud with the key of a current device.",
                  ["unknown-key"],
                ),
              ]}
            />
          </Section>
        </div>
      </div>
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
    onCleanup(poll(refresh, 30_000));
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
  const { session, loading, signInFailed, signOut, signOutAll } = createAccountSession();

  return (
    <>
      <CloudHeader>
        <Show
          when={session()}
          fallback={
            <Show when={!loading()}>
              <TwitchSignIn>Sign in</TwitchSignIn>
            </Show>
          }
        >
          {(current) => (
            <AccountMenu
              name={current().account.name}
              avatar={current().account.avatar}
              onSignOut={signOutAll}
            />
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
