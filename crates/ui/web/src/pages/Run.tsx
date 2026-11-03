import { ChevronRight, ScrollText, ShieldCheck } from "lucide-react";
import { Link, useParams } from "react-router";
import { Page, PageHeader } from "@/components/page";
import { ScorePill } from "@/components/review";
import { HarnessLogo, RunFooter, RunLog, RunStanding } from "@/components/run-view";
import { Card } from "@/components/ui/card";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { useHarnesses } from "@/hooks/use-agents";
import { useReviewCards, useTranscript } from "@/hooks/use-run";
import { duration, type Run as RunData } from "@/lib/agents";
import { dated, shortFolder } from "@/lib/reviews";

function Fact({ label, children, mono }: { label: string; children: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <dt className="text-2xs text-muted-foreground">{label}</dt>
      <dd className={mono ? "truncate font-mono text-xs" : "truncate text-sm"}>{children}</dd>
    </div>
  );
}

function ReviewCard({ hash }: { hash: string }) {
  const cards = useReviewCards(hash);
  return (
    <Link to={`/reviews/${hash}`} className="group">
      <Card size="sm" className="flex-row items-center gap-4 px-(--card-spacing) transition-shadow hover:shadow-(--shadow-dialog)">
        <span className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-fill-default">
          <ShieldCheck className="size-4" />
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="text-sm font-medium">Security review</span>
          <span className="truncate text-xs text-muted-foreground">What this run left in the folder, scored.</span>
        </span>
        {cards === undefined && <Spinner className="size-4 text-muted-foreground" />}
        {cards?.map((card) => (
          <span key={card.key} className="flex flex-col items-center gap-1" title={card.name}>
            <ScorePill score={card.rating?.score ?? null} riskier={card.riskier} />
            <span className="text-2xs text-muted-foreground">{card.short}</span>
          </span>
        ))}
        <ChevronRight className="size-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5" />
      </Card>
    </Link>
  );
}

function Facts({ run }: { run: RunData }) {
  const { byId } = useHarnesses();
  const harness = byId.get(run.harness);
  const took = duration(run.started, run.ended);
  return (
    <dl className="grid grid-cols-2 gap-x-6 gap-y-4 sm:grid-cols-4">
      <Fact label="Harness">
        <span className="flex items-center gap-1.5">
          <HarnessLogo id={run.harness} harness={harness} size="sm" className="size-5 rounded-md [&_img]:size-3" />
          {harness?.label ?? run.harness}
        </span>
      </Fact>
      <Fact label="Model">{run.model ? (harness?.models.find((one) => one.id === run.model)?.label ?? run.model) : "Its own pick"}</Fact>
      <Fact label="Started">{dated(run.started)}</Fact>
      <Fact label={run.ended ? "Took" : "Running for"}>{took ?? "—"}</Fact>
      <div className="col-span-2 sm:col-span-4">
        <Fact label="Folder" mono>
          <span title={run.folder}>{shortFolder(run.folder)}</span>
        </Fact>
      </div>
    </dl>
  );
}

export default function Run() {
  const { id = "" } = useParams();
  const { transcript, error } = useTranscript(id);
  const run = transcript.run;
  const back = run?.agent ? { to: `/agents/${encodeURIComponent(run.agent)}`, label: run.agent } : { to: "/agents", label: "Agents" };

  if (error && !run) {
    return (
      <Page className="max-w-3xl">
        <PageHeader back={back} title="Run" />
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ScrollText />
            </EmptyMedia>
            <EmptyTitle>No run at this address</EmptyTitle>
            <EmptyDescription>{error}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button variant="outline" asChild>
              <Link to="/agents">All agents</Link>
            </Button>
          </EmptyContent>
        </Empty>
      </Page>
    );
  }

  if (!run) {
    return (
      <Page className="max-w-3xl">
        <PageHeader back={back} title={<Skeleton className="h-7 w-64" />} />
        <Skeleton className="h-64 w-full rounded-2xl" />
      </Page>
    );
  }

  const hash = transcript.reviewed?.hash ?? run.review;

  return (
    <Page className="max-w-3xl">
      <PageHeader back={back} title={<span className="line-clamp-2">{run.prompt}</span>}>
        <RunStanding run={run} />
      </PageHeader>
      <Facts run={run} />
      <section className="flex flex-col gap-2" aria-label="Log">
        <RunLog transcript={transcript} className="max-h-[36rem]" />
        {run.state !== "ended" && <RunFooter transcript={transcript} />}
      </section>
      {hash && run.state === "ended" && <ReviewCard hash={hash} />}
    </Page>
  );
}
