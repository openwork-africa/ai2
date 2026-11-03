import { cn } from "cn";
import { Monitor, ScrollText, WifiOff } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { Mark } from "@/components/mark";
import { DoneCard } from "@/components/setup/done-card";
import { InputCard } from "@/components/setup/input-card";
import { LogView } from "@/components/setup/log-view";
import { StepRow } from "@/components/setup/step-row";
import { plain } from "@/components/setup/text";
import { Button } from "@/components/ui/button";
import { Progress, ProgressRange, ProgressTrack } from "@/components/ui/progress";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import {
  type Branch,
  disconnected,
  fullLog,
  fullLogWhy,
  hostLine,
  idleText,
  localOnly,
  machineName,
  type StepCopy,
  stepCopy,
  STEPS,
  type StepId,
  strip,
} from "@/copy/setup";
import { useSetup } from "@/hooks/use-setup";
import { headers, NONCE_HEADER } from "@/lib/api";
import type { Host, StepInfo } from "@/lib/setup";
import { progress } from "@/lib/setup-reducer";

const HOLD_MS = 2200;

const branchOf = (host: Host | null): Branch => (host?.os === "linux" ? "linux" : "macos");

const copyFor = (id: string, host: Host | null, info: Record<string, StepInfo>): StepCopy => {
  if (STEPS.includes(id as StepId)) {
    const branch = id === "containers" && host && !host.supports_containers ? "unsupported" : branchOf(host);
    return stepCopy(id as StepId, branch);
  }
  const given = info[id];
  return given ? { title: given.title, why: given.why, fact: given.fact } : { title: id, why: "", fact: "" };
};

const describe = (host: Host) => {
  const parts: string[] = [];
  if (host.os === "macos") {
    parts.push(host.version ? `macOS ${host.version}` : "macOS");
    if (host.arm64 !== null) parts.push(host.arm64 ? "Apple silicon" : "Intel");
  } else if (host.os === "linux") {
    parts.push("Linux");
    parts.push(host.systemd ? `systemd ${host.systemd}` : "no systemd");
  } else {
    parts.push(host.os);
  }
  if (host.user) parts.push(host.user);
  return parts.join(" · ");
};

function useNow(active: boolean) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const timer = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(timer);
  }, [active]);
  return now;
}

export default function Setup() {
  const navigate = useNavigate();
  const { state, connected, host, info, failure, run, answer, cancel } = useSetup();
  const [logOpen, setLogOpen] = useState(false);
  const watched = useRef(false);
  const machine = machineName(host?.os ?? (/Mac/i.test(navigator.userAgent) ? "macos" : "linux"));
  const busy = state.phase === "running" || state.phase === "checking";
  const now = useNow(busy);
  const counts = progress(state);
  const copies = useMemo(
    () => Object.fromEntries(state.steps.map((step) => [step.id, copyFor(step.id, host, info)])),
    [state.steps, host, info],
  );
  const titles = useMemo(
    () => Object.fromEntries(Object.entries(copies).map(([id, copy]) => [id, copy.title])),
    [copies],
  );
  const address = `${window.location.origin}/?nonce=${headers()[NONCE_HEADER] ?? ""}`;
  const done = state.phase === "done";
  const waitingFor = state.input?.step;

  useEffect(() => {
    if (busy) watched.current = true;
  }, [busy]);

  useEffect(() => {
    if (!done || !watched.current || state.check || !state.ok || state.warnings.length > 0) return;
    const timer = window.setTimeout(() => navigate("/"), HOLD_MS);
    return () => window.clearTimeout(timer);
  }, [done, state.check, state.ok, state.warnings.length, navigate]);

  const heading =
    state.phase === "idle"
      ? strip.idle(machine)
      : state.phase === "checking"
        ? strip.checking(machine)
        : state.phase === "running"
          ? strip.running(machine)
          : state.check
            ? strip.checked
            : state.ok
              ? strip.ready
              : strip.stopped;
  const counter = done ? strip.of(counts.done, counts.total) : strip.step(counts.current, counts.total);
  const allLines = state.steps.flatMap((step) => [
    { t: 0, stream: "cmd" as const, line: `== ${plain(titles[step.id] ?? step.id)}` },
    ...step.log,
  ]);

  return (
    <div className="flex min-h-svh flex-col bg-background">
      <header className="sticky top-0 z-10 border-b border-border/60 bg-background/85 backdrop-blur-md">
        <div className="mx-auto flex w-full max-w-2xl flex-col gap-3 px-4 pt-4 pb-3.5 sm:px-6">
          <div className="flex items-center gap-3">
            <button type="button" onClick={() => navigate("/")} aria-label="ai2 home" className="rounded-lg">
              <Mark />
            </button>
            <div className="flex min-w-0 flex-col leading-tight">
              <span className="truncate font-heading text-sm font-semibold tracking-tight" data-heading>
                {heading}
              </span>
            </div>
            <span className="ml-auto flex items-center gap-2 text-xs text-muted-foreground tabular-nums">
              {!connected && state.loaded && (
                <span className="flex items-center gap-1 text-warning">
                  <WifiOff className="size-3.5" />
                  <span className="hidden sm:inline">{disconnected}</span>
                </span>
              )}
              {state.phase !== "idle" && counter}
            </span>
          </div>
          <Progress value={counts.value} max={counts.total} aria-label={counter}>
            <ProgressTrack className="h-1 bg-fill-default">
              <ProgressRange
                className={cn(
                  "rounded-full duration-500",
                  done && state.ok && !state.check ? "bg-success" : done && !state.ok && !state.check ? "bg-destructive" : "bg-primary",
                )}
              />
            </ProgressTrack>
          </Progress>
        </div>
      </header>

      <main className="mx-auto flex w-full max-w-2xl flex-1 flex-col gap-6 px-4 py-7 sm:px-6 sm:py-10">
        <div className="flex items-center gap-2.5 px-1 text-xs text-muted-foreground">
          <Monitor className="size-3.5 shrink-0" />
          {host ? (
            <span data-host>{describe(host)}</span>
          ) : (
            <>
              <span>{hostLine.title}</span>
              <Skeleton className="h-3 w-40 rounded-full" />
            </>
          )}
        </div>

        {done && (
          <DoneCard
            state={state}
            machine={machine}
            titles={titles}
            address={address}
            onOpen={() => navigate("/")}
            onRun={(check) => run({ check, from: state.steps.find((step) => step.status === "failed")?.id })}
          />
        )}

        {state.phase === "idle" && state.loaded && (
          <section className="flex flex-col gap-4 rounded-3xl bg-card p-5 shadow-(--shadow-card) ring-1 ring-border sm:flex-row sm:items-center sm:justify-between">
            <div className="flex flex-col gap-1">
              <h2 className="font-heading text-base font-semibold tracking-tight">{idleText.title}</h2>
              <p className="text-[0.8125rem] leading-5 text-pretty text-muted-foreground">{idleText.why}</p>
              {failure && <p className="text-xs font-medium text-destructive">{failure}</p>}
            </div>
            <div className="flex shrink-0 gap-2">
              <Button variant="outline" size="lg" onClick={() => run({ check: true })}>
                {idleText.check}
              </Button>
              <Button size="lg" onClick={() => run({ check: false })}>
                {idleText.start}
              </Button>
            </div>
          </section>
        )}

        {state.input && !waitingFor && (
          <ul>
            <InputCard
              key={state.input.id}
              input={state.input}
              onAnswer={(values) => answer(state.input!.id, values)}
              onCancel={() => cancel(state.input!.id)}
            />
          </ul>
        )}

        <ol className={cn("flex flex-col", done && "opacity-90")} aria-label="Setup steps">
          {state.steps.map((step, index) => {
            const last = index === state.steps.length - 1;
            if (state.input && waitingFor === step.id) {
              return (
                <InputCard
                  key={state.input.id}
                  input={state.input}
                  stepTitle={titles[step.id]}
                  onAnswer={(values) => answer(state.input!.id, values)}
                  onCancel={() => cancel(state.input!.id)}
                />
              );
            }
            return (
              <StepRow
                key={step.id}
                step={step}
                copy={copies[step.id]}
                now={now}
                since={state.since[step.id]}
                last={last}
                onRetry={done ? () => run({ check: false, from: step.id }) : undefined}
              />
            );
          })}
        </ol>

        <footer className="mt-auto flex flex-wrap items-center justify-between gap-3 px-1 pt-2 text-xs text-muted-foreground">
          <span>{localOnly}</span>
          <Button variant="ghost" size="sm" onClick={() => setLogOpen(true)} className="-mr-2 text-muted-foreground">
            <ScrollText />
            {fullLog}
          </Button>
        </footer>
      </main>

      <Sheet open={logOpen} onOpenChange={(details) => setLogOpen(details.open)}>
        <SheetContent side="right" className="w-full sm:max-w-xl">
          <SheetHeader className="pb-3">
            <SheetTitle>{fullLog}</SheetTitle>
            <SheetDescription>{fullLogWhy}</SheetDescription>
          </SheetHeader>
          <div className="flex min-h-0 flex-1 px-6 pb-6">
            <LogView lines={allLines} fit={false} className="h-full w-full" />
          </div>
        </SheetContent>
      </Sheet>
    </div>
  );
}
