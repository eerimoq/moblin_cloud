import { Show, type Component, type JSX } from "solid-js";
import type { SetStoreFunction } from "solid-js/store";
import { rowClass } from "../../MoblinParts";
import { ButtonRow, ErrorText, Footer, Section } from "../../sceneEditor/Form";
import type { Stack } from "../../sceneEditor/Stack";
import type { CreateStreamWizard } from "../../../data/streamWizard";
import { t } from "../../../i18n";

export type Wizard = {
  state: CreateStreamWizard;
  set: SetStoreFunction<CreateStreamWizard>;
  stack: Stack;
  uniqueName: (name: string) => string;
  create: () => void;
};

export type WizardPageProps = { wizard: Wizard };

export const pushPage = (wizard: Wizard, title: string, Page: Component<WizardPageProps>) =>
  wizard.stack.push({ title, render: () => <Page wizard={wizard} /> });

export function Field(props: {
  header: string;
  value: string;
  placeholder: string;
  error?: string;
  footer?: JSX.Element;
  onInput: (value: string) => void;
}) {
  return (
    <Section
      header={props.header}
      footer={
        props.error || props.footer ? (
          <span class="flex flex-col gap-1">
            <ErrorText text={props.error || null} />
            {props.footer}
          </span>
        ) : undefined
      }
    >
      <input
        type="text"
        class={`${rowClass} bg-transparent outline-none placeholder:text-phone-border`}
        value={props.value}
        placeholder={props.placeholder}
        autocapitalize="off"
        autocomplete="off"
        spellcheck={false}
        onInput={(event) => props.onInput(event.currentTarget.value)}
      />
    </Section>
  );
}

export function SkipRow(props: { onClick: () => void }) {
  return (
    <Section>
      <ButtonRow label={t("Skip")} onClick={props.onClick} />
    </Section>
  );
}

export function LoginFooter() {
  return <Footer>{t("Log in on the phone after creating the stream.")}</Footer>;
}

export function ExternalLink(props: { href: string; children?: JSX.Element }) {
  return (
    <a class="underline" href={props.href} target="_blank" rel="noopener">
      <Show when={props.children} fallback={props.href}>
        {props.children}
      </Show>
    </a>
  );
}
