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
const env = Object.fromEntries(
  Object.entries(process.env).filter(
    ([key]) => !["SELF_HOSTED_LANDING_PAGE", "TERMS_URL", "PRIVACY_POLICY_URL"].includes(key),
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
    termsUrl: "",
    privacyPolicyUrl: "",
  });
  assert.equal(run({ SELF_HOSTED_LANDING_PAGE: "true" }).status, 0);
  assert.equal(config().selfHostedLandingPage, true);
  const url = 'https://example.com/policy?text="hello"\n\u0001';
  assert.equal(run({ TERMS_URL: url, PRIVACY_POLICY_URL: url }).status, 0);
  assert.equal(config().termsUrl, url);
  assert.equal(config().privacyPolicyUrl, url);
  assert.notEqual(run({ SELF_HOSTED_LANDING_PAGE: "invalid" }).status, 0);
  console.log("PASS: policy defaults, URL escaping and invalid flags");
} finally {
  rmSync(directory, { recursive: true, force: true });
}
