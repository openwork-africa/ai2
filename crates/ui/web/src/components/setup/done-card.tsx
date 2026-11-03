import { cn } from "cn";
import { ArrowRight, CircleCheck, CircleX, ListChecks, RotateCcw, TriangleAlert } from "lucide-react";
import { duration } from "@/components/setup/text";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Code } from "@/components/page";
import { doneText, failureHint, stepText } from "@/copy/setup";
import type { SetupState } from "@/lib/setup-reducer";

export function DoneCard({
  state,
  machine,
  titles,
  address,
  onOpen,
  onRun,
}: {
  state: SetupState;
  machine: string;
  titles: Record<string, string>;
  address: string;
  onOpen: () => void;
  onRun: (check: boolean) => void;
}) {
  const failed = state.steps.find((step) => step.status === "failed");
  const needed = state.steps.filter((step) => step.status === "missing" || step.status === "unknown");
  const kind = state.check ? "checked" : state.ok ? "ok" : "stopped";
  const Icon = kind === "stopped" ? CircleX : kind === "checked" ? ListChecks : CircleCheck;
  const title =
    kind === "stopped"
      ? doneText.stopped(failed ? titles[failed.id] ?? failed.id : machine)
      : kind === "checked"
        ? doneText.checked(machine)
        : doneText.setUp(machine);
  const line =
    kind === "checked"
      ? needed.length
        ? doneText.needed(needed.length)
        : doneText.allReady
      : state.steps.map((step) => titles[step.id] ?? step.id).join(" · ");

  return (
    <section
      data-done={kind}
      className={cn(
        "flex flex-col gap-5 rounded-3xl bg-card p-5 shadow-(--shadow-card) ring-1 ring-border sm:p-6",
        kind === "stopped" && "ring-destructive/30",
      )}
    >
      <div className="flex items-start gap-3.5">
        <span
          className={cn(
            "flex size-10 shrink-0 items-center justify-center rounded-2xl",
            kind === "ok" && "bg-success/12 text-success",
            kind === "checked" && "bg-fill-default text-foreground",
            kind === "stopped" && "bg-destructive/10 text-destructive",
          )}
        >
          <Icon className="size-5" />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex items-baseline justify-between gap-3">
            <h2 className="font-heading text-lg font-semibold tracking-tight text-balance">{title}</h2>
            {state.ms !== undefined && (
              <span className="text-xs text-muted-foreground tabular-nums">{duration(state.ms)}</span>
            )}
          </div>
          <p className="text-[0.8125rem] leading-5 text-muted-foreground">{line}</p>
          {kind === "stopped" && (failed?.error || state.error) && (
            <p className="mt-1 text-[0.8125rem] leading-5 font-medium text-destructive">{failed?.error ?? state.error}</p>
          )}
          {kind === "stopped" && <p className="text-xs text-muted-foreground">{failureHint}</p>}
        </div>
      </div>

      {state.warnings.length > 0 && (
        <div className="flex flex-col gap-2">
          {state.warnings.map((warning) => (
            <Alert key={`${warning.step}-${warning.text}`} className="rounded-2xl bg-warning/8 ring-warning/25">
              <TriangleAlert className="text-warning" />
              <AlertTitle className="text-xs font-medium text-muted-foreground">{titles[warning.step] ?? warning.step}</AlertTitle>
              <AlertDescription className="text-[0.8125rem] text-foreground">{warning.text}</AlertDescription>
            </Alert>
          ))}
        </div>
      )}

      {kind === "ok" && (
        <div className="flex flex-col gap-1 rounded-2xl bg-fill-subtle px-4 py-3">
          <span className="text-2xs font-medium tracking-wide text-muted-foreground uppercase">{doneText.address}</span>
          <Code className="truncate bg-transparent px-0 text-xs">{address}</Code>
          <span className="text-xs text-muted-foreground">{doneText.bookmark}</span>
        </div>
      )}

      <div className="flex flex-wrap justify-end gap-2">
        {kind === "stopped" && (
          <Button size="lg" variant="outline" onClick={() => onRun(false)}>
            <RotateCcw />
            {stepText.tryAgain}
          </Button>
        )}
        {kind === "checked" && needed.length > 0 && (
          <Button size="lg" variant="outline" onClick={() => onRun(false)}>
            {doneText.setItUp}
          </Button>
        )}
        <Button size="lg" variant={kind === "stopped" ? "ghost" : "default"} onClick={onOpen}>
          {doneText.open}
          <ArrowRight data-icon="inline-end" />
        </Button>
      </div>
    </section>
  );
}
