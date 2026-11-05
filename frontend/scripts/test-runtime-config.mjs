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
    ([key]) => !["SELF_HOSTED_LANDING_PAGE", "INSTANCE_NAME"].includes(key),
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
    instanceName: "",
  });
  assert.equal(run({ SELF_HOSTED_LANDING_PAGE: "true" }).status, 0);
  assert.equal(config().selfHostedLandingPage, true);
  for (const name of [
    "MrDemonWolf",
    'Quotes " and \\ slashes\n\t\u0001',
    'Wolf 🐺"; throw Error("injection"); //',
  ]) {
    assert.equal(run({ INSTANCE_NAME: name }).status, 0);
    assert.equal(config().instanceName, name);
  }
  const result = run({ SELF_HOSTED_LANDING_PAGE: "false; echo unsafe" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must be true or false/);
  console.log("PASS: landing defaults, name escaping and invalid flags");
} finally {
  rmSync(directory, { recursive: true, force: true });
}
