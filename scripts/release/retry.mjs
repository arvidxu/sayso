// Any real failure, or a cancellation for another reason, stops the retry, so a broken build is
// never rerun.
import { setting, isPositive } from "./settings.mjs";
import { env, summary, run, capture } from "./github.mjs";

const shortageMessage = "not acquired by Runner";

// A job that never got a machine has no runner and ran no step. One that never started without the
// shortage message, such as the release job after a cancelled build, only follows from the others.
function classifyJobs(jobs, hasShortageMessage) {
  const neverStarted = ({ runner_name: runnerName, steps }) =>
    !runnerName && (steps ?? []).length === 0;
  return {
    shortages: jobs.filter(
      (job) => job.conclusion === "cancelled" && neverStarted(job) && hasShortageMessage(job),
    ),
    others: jobs.filter((job) => !neverStarted(job)),
  };
}

export function retry() {
  const runId = env.RUN_ID;
  const attempt = Number(env.RUN_ATTEMPT);
  const attempts = setting("retryAttempts", isPositive);
  if (attempt >= attempts) {
    summary(`No rerun: attempt ${attempt} of ${attempts} was the last.`);
    return;
  }
  const api = (path) => JSON.parse(capture("gh", ["api", "--paginate", "--slurp", path]));
  const jobs = api(`repos/${env.GITHUB_REPOSITORY}/actions/runs/${runId}/attempts/${attempt}/jobs`)
    .flatMap((page) => page.jobs)
    .filter(({ conclusion }) => conclusion === "failure" || conclusion === "cancelled");
  const { shortages, others } = classifyJobs(jobs, ({ id }) =>
    api(`repos/${env.GITHUB_REPOSITORY}/check-runs/${id}/annotations`)
      .flat()
      .some(({ message }) => message?.includes(shortageMessage)),
  );
  if (shortages.length === 0 || others.length > 0) {
    summary(
      `No rerun: ${others.length === 0 ? "no job was cancelled for lack of a machine" : `not a machine shortage: ${others.map(({ name, conclusion }) => `${name} (${conclusion})`).join(", ")}`}.`,
    );
    return;
  }
  summary(
    `Rerunning ${shortages.map(({ name }) => name).join(", ")}: no machine was available (attempt ${attempt} of ${attempts}).`,
  );
  // Only failed and cancelled jobs and their dependents rerun; finished builds keep their artifacts.
  run("gh", ["run", "rerun", runId, "--failed", "--repo", env.GITHUB_REPOSITORY]);
}
