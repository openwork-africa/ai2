import { CircleAlert, CircleCheck, Play, Plus } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { ApiError, events } from "@/lib/api";
import { type JobEvent, startReview } from "@/lib/reviews";

type Run =
  | { state: "idle" }
  | { state: "running"; lines: string[] }
  | { state: "failed"; lines: string[]; message: string }
  | { state: "done"; lines: string[]; hash: string };

export function StartReview({ onDone }: { onDone: () => void }) {
  const [open, setOpen] = useState(false);
  const [folder, setFolder] = useState("");
  const [objective, setObjective] = useState("");
  const [run, setRun] = useState<Run>({ state: "idle" });
  const abort = useRef<AbortController | null>(null);
  const navigate = useNavigate();

  useEffect(() => () => abort.current?.abort(), []);

  const start = async () => {
    const lines: string[] = [];
    setRun({ state: "running", lines });
    try {
      const started = await startReview({ folder: folder.trim(), objective: objective.trim() || undefined });
      abort.current = new AbortController();
      let finished = false;
      await events(
        `/api/reviews/jobs/${started.job}/events`,
        (event) => {
          const data = event.data as JobEvent;
          if (data.event === "log") {
            lines.push(data.line);
            setRun({ state: "running", lines: [...lines] });
          } else if (data.event === "done") {
            finished = true;
            if (data.ok) {
              setRun({ state: "done", lines: [...lines], hash: data.hash });
              onDone();
            } else {
              setRun({
                state: "failed",
                lines: [...lines],
                message: data.message ?? `ai2 stopped with exit code ${data.code ?? "?"}.`,
              });
            }
          }
        },
        { signal: abort.current.signal },
      );
      if (!finished) setRun({ state: "failed", lines: [...lines], message: "The review stopped answering." });
    } catch (failure) {
      if ((failure as Error).name === "AbortError") return;
      const message = failure instanceof ApiError ? failure.message : (failure as Error).message;
      setRun({ state: "failed", lines: [...lines], message });
    }
  };

  const busy = run.state === "running";
  const lines = run.state === "idle" ? [] : run.lines;

  return (
    <Sheet open={open} onOpenChange={(details) => setOpen(details.open)}>
      <Button onClick={() => setOpen(true)}>
        <Plus data-icon="inline-start" />
        New review
      </Button>
      <SheetContent className="w-full sm:max-w-md">
        <SheetHeader className="pr-12">
          <SheetTitle>New review</SheetTitle>
          <SheetDescription>
            Rates a folder an agent has worked in: its stack, recent exploits and what the run changed.
          </SheetDescription>
        </SheetHeader>
        <form
          className="flex flex-1 flex-col gap-5 overflow-y-auto px-6"
          onSubmit={(event) => {
            event.preventDefault();
            if (!busy && folder.trim()) void start();
          }}
        >
          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-medium">Folder</span>
            <Input
              value={folder}
              onChange={(event) => setFolder(event.target.value)}
              placeholder="/Users/you/Documents/project"
              className="font-mono text-xs"
              disabled={busy}
              required
            />
            <span className="text-xs text-muted-foreground">The folder the run worked in, as a full path.</span>
          </label>
          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-medium">Objective</span>
            <Input
              value={objective}
              onChange={(event) => setObjective(event.target.value)}
              placeholder="Add a pricing page"
              disabled={busy}
            />
            <span className="text-xs text-muted-foreground">What the run was asked to do. Optional.</span>
          </label>
          {run.state !== "idle" && (
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-2 text-sm font-medium">
                {run.state === "running" && <Spinner className="size-4" />}
                {run.state === "done" && <CircleCheck className="size-4 text-success" />}
                {run.state === "failed" && <CircleAlert className="size-4 text-destructive" />}
                {run.state === "running" ? "Reviewing" : run.state === "done" ? "Review written" : "Review failed"}
              </div>
              <pre className="max-h-64 overflow-auto rounded-2xl bg-fill-default p-3 font-mono text-2xs leading-relaxed whitespace-pre-wrap text-muted-foreground">
                {lines.length ? lines.join("\n") : "Starting…"}
              </pre>
              {run.state === "failed" && <p className="text-sm text-destructive">{run.message}</p>}
            </div>
          )}
          <button type="submit" hidden />
        </form>
        <SheetFooter className="flex-row justify-end">
          {run.state === "done" ? (
            <Button
              onClick={() => {
                setOpen(false);
                navigate(`/reviews/${run.hash}`);
              }}
            >
              Open the review
            </Button>
          ) : (
            <Button disabled={busy || !folder.trim()} onClick={() => void start()}>
              {busy ? <Spinner data-icon="inline-start" /> : <Play data-icon="inline-start" />}
              {busy ? "Reviewing" : "Start review"}
            </Button>
          )}
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
