// Usage: node scripts/release/release.mjs <step> [arguments]
import { check, checkTag, plan, runners } from "./plan.mjs";
import { desktopPackage } from "./build.mjs";
import { publish } from "./publish.mjs";
import { retry } from "./retry.mjs";
import { waitForCheck } from "./wait.mjs";

const steps = {
  "check-tag": checkTag,
  plan,
  check,
  runners,
  "wait-for-check": waitForCheck,
  "desktop-package": desktopPackage,
  publish,
  retry,
};
const [step, ...args] = process.argv.slice(2);
if (!steps[step]) {
  throw new Error(
    `Usage: node scripts/release/release.mjs <${Object.keys(steps).join("|")}> [arguments]`,
  );
}
await steps[step](...args);
