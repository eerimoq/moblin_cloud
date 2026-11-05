import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { runInNewContext } from "node:vm";

const directory = mkdtempSync(join(tmpdir(), "moblin-config-"));
const output = join(directory, "config.js");
const script = readFileSync(new URL("../runtime-config.sh", import.meta.url), "utf8").replace(
  "/srv/config.js",
  output,
);
const flags = ["SELF_HOSTED_LANDING_PAGE", "TIPPING_ENABLED"];
const env = Object.fromEntries(
  Object.entries(process.env).filter(
    ([key]) => ![...flags, "INSTANCE_NAME", "TERMS_URL", "PRIVACY_POLICY_URL"].includes(key),
  ),
);
const run = (values) =>
  spawnSync("sh", ["-s", "--", "true"], {
    input: script,
    env: { ...env, ...values },
    encoding: "utf8",
  });
const config = () => {
  const context = { window: {} };
  runInNewContext(readFileSync(output, "utf8"), context);
  return JSON.parse(JSON.stringify(context.window.config));
};
try {
  assert.equal(run({}).status, 0);
  assert.deepEqual(config(), {
    backendUrl: "/api",
    selfHostedLandingPage: false,
    tippingEnabled: true,
    instanceName: "",
    termsUrl: "",
    privacyPolicyUrl: "",
  });
  assert.equal(run(Object.fromEntries(flags.map((key) => [key, "false"]))).status, 0);
  assert.deepEqual(config(), {
    backendUrl: "/api",
    selfHostedLandingPage: false,
    tippingEnabled: false,
    instanceName: "",
    termsUrl: "",
    privacyPolicyUrl: "",
  });
  assert.equal(run({ SELF_HOSTED_LANDING_PAGE: "true" }).status, 0);
  assert.equal(config().selfHostedLandingPage, true);
  assert.equal(
    run({
      TERMS_URL: 'https://example.com/terms?text="hello"',
      PRIVACY_POLICY_URL: "https://example.com/privacy",
    }).status,
    0,
  );
  assert.equal(config().termsUrl, 'https://example.com/terms?text="hello"');
  assert.equal(config().privacyPolicyUrl, "https://example.com/privacy");
  for (const name of [
    "MrDemonWolf",
    'Quotes " and \\ slashes\n\t\u0001',
    'Wolf 🐺"; throw Error("injection"); //',
  ]) {
    assert.equal(run({ INSTANCE_NAME: name }).status, 0);
    assert.equal(config().instanceName, name);
  }
  assert.equal(run(Object.fromEntries(flags.map((key) => [key, "false"]))).status, 0);
  for (const flag of flags) {
    const result = run({ [flag]: "false; echo unsafe" });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must be true or false/);
    assert.equal(config().tippingEnabled, false);
  }
  console.log("PASS: runtime defaults, disabled features and invalid environment values");
} finally {
  rmSync(directory, { recursive: true, force: true });
}
