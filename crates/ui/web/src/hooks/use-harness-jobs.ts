import { useCallback, useEffect, useRef, useState } from "react";
import { type Action, answerJob, followJob, type Harness, type JobEvent, type JobSummary, startJob } from "@/lib/harnesses";

const KEPT_LINES = 2000;

export type JobView = {
  id: string;
  harness: string;
  action: Action;
  started: number;
  ended?: number;
  lines: string[];
  link?: string;
  code?: string;
  error?: string;
  sent: number;
  done?: { ok: boolean; message?: string };
};

const apply = (job: JobView, event: JobEvent): JobView => {
  switch (event.type) {
    case "line": {
      const lines = job.lines.length >= KEPT_LINES ? job.lines.slice(-KEPT_LINES + 1) : job.lines.slice();
      lines.push(event.text);
      return { ...job, lines };
    }
    case "link":
      return { ...job, link: job.link ?? event.url };
    case "code":
      return { ...job, code: event.code };
    case "error":
      return { ...job, error: event.error };
    case "done":
      return { ...job, done: { ok: event.ok, message: event.message }, ended: Date.now() };
  }
};

export function useHarnessJobs(onFinished: () => void) {
  const [jobs, setJobs] = useState<Record<string, JobView>>({});
  const controllers = useRef(new Map<string, AbortController>());
  const finished = useRef(onFinished);
  useEffect(() => {
    finished.current = onFinished;
  }, [onFinished]);

  useEffect(() => {
    const open = controllers.current;
    return () => {
      open.forEach((controller) => controller.abort());
      open.clear();
    };
  }, []);

  const follow = useCallback((summary: JobSummary) => {
    if (controllers.current.has(summary.id)) return;
    const controller = new AbortController();
    controllers.current.set(summary.id, controller);
    setJobs((all) => ({
      ...all,
      [summary.harness]: {
        id: summary.id,
        harness: summary.harness,
        action: summary.action,
        started: Date.now(),
        lines: [],
        sent: 0,
      },
    }));
    let lastEventId: string | undefined;
    let ended = false;
    const update = (event: JobEvent) =>
      setJobs((all) => {
        const job = all[summary.harness];
        if (!job || job.id !== summary.id) return all;
        return { ...all, [summary.harness]: apply(job, event) };
      });
    const run = async () => {
      for (let attempt = 0; !ended && !controller.signal.aborted && attempt < 20; attempt += 1) {
        try {
          await followJob(
            summary.id,
            (event, id) => {
              lastEventId = id ?? lastEventId;
              if (event.type === "done") ended = true;
              update(event);
            },
            { signal: controller.signal, lastEventId },
          );
        } catch {
          if (controller.signal.aborted) return;
        }
        if (!ended) await new Promise((resolve) => window.setTimeout(resolve, 1000));
      }
      controllers.current.delete(summary.id);
      if (!ended && !controller.signal.aborted)
        update({ type: "done", ok: false, message: "This page lost touch with the job." });
      if (!controller.signal.aborted) finished.current();
    };
    void run();
  }, []);

  const start = useCallback(
    async (harness: Harness, action: Action) => {
      const { job } = await startJob(harness.id, action);
      follow(job);
    },
    [follow],
  );

  const answer = useCallback(async (harness: string, text: string) => {
    const job = jobs[harness];
    if (!job) return;
    setJobs((all) => ({ ...all, [harness]: { ...all[harness], error: undefined } }));
    await answerJob(job.id, text);
    setJobs((all) => ({ ...all, [harness]: { ...all[harness], sent: all[harness].sent + 1 } }));
  }, [jobs]);

  const dismiss = useCallback((harness: string) => {
    setJobs((all) => {
      const job = all[harness];
      if (!job || !job.done) return all;
      const rest = { ...all };
      delete rest[harness];
      return rest;
    });
  }, []);

  return { jobs, follow, start, answer, dismiss };
}
