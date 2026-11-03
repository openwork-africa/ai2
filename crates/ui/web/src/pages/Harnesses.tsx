import { cn } from "cn";
import { Bot, Download, LogIn, RefreshCw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { HarnessJob } from "@/components/harness-job";
import { Code, Dot, Page, PageHeader } from "@/components/page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { type JobView, useHarnessJobs } from "@/hooks/use-harness-jobs";
import { type Action, type Harness, type HarnessState, listHarnesses, stateOf } from "@/lib/harnesses";

const STATE: Record<HarnessState, { tone: "success" | "warning" | "muted"; label: string }> = {
  ready: { tone: "success", label: "Ready" },
  "sign-in": { tone: "warning", label: "Needs sign-in" },
  missing: { tone: "muted", label: "Not installed" },
};

const signInLine = (harness: Harness) => {
  if (!harness.can_sign_in) return harness.kind === "agent" ? "Signs in the first time it runs" : "Needs no sign-in";
  if (harness.asks === "key") return "Signs in with an API key";
  if (harness.asks === "code") return "Signs in with a code from the browser";
  return "Signs in through the browser";
};

const without = <T,>(all: Record<string, T>, key: string) => {
  const rest = { ...all };
  delete rest[key];
  return rest;
};

const monogram = (harness: Harness) => (harness.id.length <= 2 ? harness.id : harness.label.charAt(0).toUpperCase());

function Logo({ harness }: { harness: Harness }) {
  const [broken, setBroken] = useState(false);
  return (
    <span className="flex size-11 shrink-0 items-center justify-center rounded-xl bg-white shadow-(--shadow-secondary-control) ring-1 ring-black/8 dark:ring-white/10">
      {harness.logo && !broken ? (
        <img src={`/harnesses/${harness.logo}`} alt="" className="size-6" onError={() => setBroken(true)} />
      ) : (
        <span className="font-heading text-base font-semibold tracking-tight text-neutral-800">{monogram(harness)}</span>
      )}
    </span>
  );
}

function StateBadge({ state }: { state: HarnessState }) {
  const { tone, label } = STATE[state];
  return (
    <Badge variant="outline" className="gap-1.5 pl-1.5">
      <Dot tone={tone} className="ring-0" />
      {label}
    </Badge>
  );
}

const standing = (harness: Harness) => {
  if (!harness.installed) return "Not installed yet";
  if (!harness.can_sign_in) return "Installed";
  return harness.signed_in ? "Signed in" : "Installed, not signed in";
};

function Models({ harness }: { harness: Harness }) {
  if (harness.kind !== "agent") return <p className="text-xs text-muted-foreground">On the PATH of every run.</p>;
  if (harness.models.length === 0) return <p className="text-xs text-muted-foreground">Uses the model set in its own settings.</p>;
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {harness.models.map((model) => (
        <Badge key={model.id} variant="secondary">
          {model.label}
        </Badge>
      ))}
    </div>
  );
}

function Actions({
  harness,
  job,
  starting,
  onStart,
}: {
  harness: Harness;
  job?: JobView;
  starting: Action | null;
  onStart: (action: Action) => void;
}) {
  const busy = Boolean(job && !job.done) || starting !== null;
  const needsSignIn = harness.installed && harness.can_sign_in && !harness.signed_in;
  return (
    <div className="flex items-center gap-2">
      {harness.can_sign_in && harness.installed && (
        <Button
          type="button"
          size="sm"
          variant={needsSignIn ? "default" : "ghost"}
          disabled={busy}
          onClick={() => onStart("sign-in")}
        >
          {starting === "sign-in" ? <Spinner /> : <LogIn />}
          {harness.signed_in ? "Sign in again" : "Sign in"}
        </Button>
      )}
      <Button
        type="button"
        size="sm"
        variant={harness.installed ? "ghost" : "default"}
        disabled={busy}
        onClick={() => onStart("install")}
      >
        {starting === "install" ? <Spinner /> : harness.installed ? <RefreshCw /> : <Download />}
        {harness.installed ? "Update" : "Install"}
      </Button>
    </div>
  );
}

function HarnessCard({
  harness,
  job,
  starting,
  failure,
  onStart,
  onAnswer,
  onDismiss,
}: {
  harness: Harness;
  job?: JobView;
  starting: Action | null;
  failure?: string;
  onStart: (action: Action) => void;
  onAnswer: (text: string) => Promise<void>;
  onDismiss: () => void;
}) {
  const wide = Boolean(job);
  return (
    <Card
      size="sm"
      className={cn("gap-4 transition-shadow", wide && "md:col-span-2 xl:col-span-3")}
      data-harness={harness.id}
    >
      <div className={cn("flex flex-col gap-4 px-(--card-spacing)", wide && "md:flex-row md:items-center")}>
        <div className="flex min-w-0 flex-1 items-start gap-3">
          <Logo harness={harness} />
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            <div className="flex items-center gap-2">
              <h3 className="truncate font-heading text-sm font-semibold tracking-tight">{harness.label}</h3>
              {!wide && <span className="ml-auto"><StateBadge state={stateOf(harness)} /></span>}
            </div>
            <p className="truncate text-xs text-muted-foreground">
              <Code className="bg-transparent px-0 text-muted-foreground">{harness.command}</Code>
              <span className="px-1.5 text-muted-foreground/50">·</span>
              {signInLine(harness)}
            </p>
          </div>
        </div>
        {wide && (
          <div className="flex items-center gap-3 md:justify-end">
            <StateBadge state={stateOf(harness)} />
            {job?.done?.ok && <Actions harness={harness} job={job} starting={starting} onStart={onStart} />}
          </div>
        )}
      </div>
      {wide && job ? (
        <div className="px-(--card-spacing)">
          <HarnessJob
            harness={harness}
            job={job}
            onAnswer={onAnswer}
            onRetry={() => onStart(job.action)}
            onDismiss={onDismiss}
          />
        </div>
      ) : (
        <div className="flex flex-col gap-4 px-(--card-spacing)">
          <Models harness={harness} />
          <div className="-mx-(--card-spacing) flex items-center justify-between gap-2 border-t px-(--card-spacing) pt-3">
            <span className={cn("truncate text-2xs", failure ? "text-destructive" : "text-muted-foreground")}>
              {failure ?? standing(harness)}
            </span>
            <Actions harness={harness} job={job} starting={starting} onStart={onStart} />
          </div>
        </div>
      )}
    </Card>
  );
}

function Loading() {
  return (
    <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
      {Array.from({ length: 6 }, (_, at) => (
        <Card key={at} size="sm" className="gap-4">
          <div className="flex items-start gap-3 px-(--card-spacing)">
            <Skeleton className="size-11 rounded-xl" />
            <div className="flex flex-1 flex-col gap-2 pt-1">
              <Skeleton className="h-3.5 w-28" />
              <Skeleton className="h-3 w-40" />
            </div>
          </div>
          <div className="flex gap-1.5 px-(--card-spacing)">
            <Skeleton className="h-5 w-16 rounded-full" />
            <Skeleton className="h-5 w-20 rounded-full" />
          </div>
          <div className="flex justify-end border-t px-(--card-spacing) pt-3">
            <Skeleton className="h-7 w-20 rounded-md" />
          </div>
        </Card>
      ))}
    </div>
  );
}

function Section({ title, count, children }: { title: string; count: number; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <h2 className="flex items-baseline gap-2 text-xs font-medium tracking-wide text-muted-foreground uppercase">
        {title}
        <span className="text-muted-foreground/60 tabular-nums">{count}</span>
      </h2>
      <div className="grid grid-flow-row-dense gap-4 md:grid-cols-2 xl:grid-cols-3">{children}</div>
    </section>
  );
}

function Summary({ harnesses }: { harnesses: Harness[] }) {
  const count = (state: HarnessState) => harnesses.filter((harness) => stateOf(harness) === state).length;
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {(["ready", "sign-in", "missing"] as const).map((state) => (
        <Badge key={state} variant="outline" size="lg" className="gap-1.5 pl-2">
          <Dot tone={STATE[state].tone} className="ring-0" />
          <span className="tabular-nums">{count(state)}</span>
          <span className="text-muted-foreground">{STATE[state].label.toLowerCase()}</span>
        </Badge>
      ))}
    </div>
  );
}

export default function Harnesses() {
  const [harnesses, setHarnesses] = useState<Harness[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [starting, setStarting] = useState<Record<string, Action>>({});
  const [refused, setRefused] = useState<Record<string, string>>({});

  const load = useCallback(
    () =>
      listHarnesses()
        .then(({ harnesses }) => {
          setHarnesses(harnesses);
          setFailed(null);
          return harnesses;
        })
        .catch((error: Error) => {
          setFailed(error.message);
          return null;
        }),
    [],
  );

  const { jobs, follow, start, answer, dismiss } = useHarnessJobs(() => void load());

  useEffect(() => {
    void load().then((listed) => {
      listed?.forEach((harness) => {
        if (harness.job?.running) follow(harness.job);
      });
    });
  }, [load, follow]);

  const begin = (harness: Harness, action: Action) => {
    setStarting((all) => ({ ...all, [harness.id]: action }));
    setRefused((all) => without(all, harness.id));
    start(harness, action)
      .catch((error: Error) => setRefused((all) => ({ ...all, [harness.id]: error.message })))
      .finally(() => setStarting((all) => without(all, harness.id)));
  };

  const agents = harnesses?.filter((harness) => harness.kind === "agent") ?? [];
  const tools = harnesses?.filter((harness) => harness.kind === "tool") ?? [];

  const card = (harness: Harness) => (
    <HarnessCard
      key={harness.id}
      harness={harness}
      job={jobs[harness.id]}
      starting={starting[harness.id] ?? null}
      failure={refused[harness.id]}
      onStart={(action) => begin(harness, action)}
      onAnswer={(text) => answer(harness.id, text)}
      onDismiss={() => dismiss(harness.id)}
    />
  );

  return (
    <Page className="max-w-6xl">
      <PageHeader
        title="Harnesses"
        description="The coding agents and tools ai2 installs, signs in to and runs headless. Installs and sign-ins run on this machine, under your user."
        actions={harnesses && <Summary harnesses={harnesses} />}
      />
      {failed && !harnesses ? (
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <TriangleAlert />
            </EmptyMedia>
            <EmptyTitle>The harness list did not load</EmptyTitle>
            <EmptyDescription>{failed}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button type="button" variant="outline" size="sm" onClick={() => void load()}>
              <RefreshCw />
              Try again
            </Button>
          </EmptyContent>
        </Empty>
      ) : !harnesses ? (
        <Loading />
      ) : harnesses.length === 0 ? (
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <Bot />
            </EmptyMedia>
            <EmptyTitle>No harnesses in this build</EmptyTitle>
            <EmptyDescription>This ai2 was built without a harness registry.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Code>ai2 harness list</Code>
          </EmptyContent>
        </Empty>
      ) : (
        <div className="flex flex-col gap-10">
          {agents.length > 0 && (
            <Section title="Coding agents" count={agents.length}>
              {agents.map(card)}
            </Section>
          )}
          {tools.length > 0 && (
            <Section title="Tools" count={tools.length}>
              {tools.map(card)}
            </Section>
          )}
        </div>
      )}
    </Page>
  );
}
