// A check rerun for lack of a machine is waited for again.
import { setTimeout as sleep } from "node:timers/promises";
import { setting, isPositive } from "./settings.mjs";
import { env, summary, capture } from "./github.mjs";

function checkState(run) {
  if (!run) return "missing";
  if (run.status !== "completed") return "running";
  return run.conclusion === "success" ? "passed" : "failed";
}

export async function waitForCheck() {
  const pollSeconds = setting("checkWait.pollSeconds", isPositive);
  // How long a missing or failed check may take to start again, as when it is rerun.
  const restartMinutes = setting("checkWait.restartMinutes", isPositive);
  const commit = env.GITHUB_SHA;
  const path = `repos/${env.GITHUB_REPOSITORY}/actions/workflows/check.yml/runs?head_sha=${commit}&event=push`;
  let stoppedSince = null;
  let unreachableSince = null;
  for (;;) {
    let runs;
    try {
      runs = JSON.parse(capture("gh", ["api", path])).workflow_runs;
      unreachableSince = null;
    } catch (error) {
      // A passing outage of the service is waited out like a check that has not started yet.
      unreachableSince ??= Date.now();
      if (Date.now() - unreachableSince >= restartMinutes * 60_000) {
        throw new Error(`Could not read the check of ${commit}: ${error.stderr ?? error.message}`);
      }
      await sleep(pollSeconds * 1000);
      continue;
    }
    // Newest first.
    const [run] = runs;
    const state = checkState(run);
    if (state === "passed") {
      summary(`The check of ${commit} passed: ${run.html_url}`);
      return;
    }
    if (state === "running") {
      stoppedSince = null;
    } else {
      stoppedSince ??= Date.now();
      if (Date.now() - stoppedSince >= restartMinutes * 60_000) {
        throw new Error(
          state === "missing"
            ? `No check ran for ${commit}, so nothing was built. Push a change the check covers, then tag that commit.`
            : `The check of ${commit} ended as ${run.conclusion}, so nothing was built: ${run.html_url}`,
        );
      }
    }
    await sleep(pollSeconds * 1000);
  }
}
