import { ChevronRight } from "lucide-react";
import { Link, useNavigate } from "react-router";
import { RunStanding, Scores } from "@/components/run-view";
import { Card } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import type { Run } from "@/lib/agents";
import { dated, when } from "@/lib/reviews";

export function RunRows({ runs, empty }: { runs: Run[] | null; empty: string }) {
  const navigate = useNavigate();
  if (!runs) {
    return (
      <Card className="gap-0 py-0">
        {[0, 1].map((row) => (
          <div key={row} className="flex items-center gap-4 border-b px-5 py-3.5 last:border-0">
            <Skeleton className="h-4 flex-1" />
            <Skeleton className="h-4 w-20" />
          </div>
        ))}
      </Card>
    );
  }
  if (runs.length === 0) {
    return <p className="rounded-2xl border border-dashed px-5 py-6 text-center text-sm text-muted-foreground">{empty}</p>;
  }
  return (
    <Card className="gap-0 py-0">
      {runs.map((run) => (
        <div
          key={run.id}
          data-run={run.id}
          onClick={() => navigate(`/runs/${run.id}`)}
          className="group flex cursor-pointer items-center gap-4 border-b px-4 py-3.5 transition-colors last:border-0 hover:bg-fill-subtle sm:px-5"
        >
          <span className="flex min-w-0 flex-1 flex-col gap-1">
            <Link
              to={`/runs/${run.id}`}
              onClick={(event) => event.stopPropagation()}
              className="truncate text-sm outline-none focus-visible:underline"
            >
              {run.prompt}
            </Link>
            <span className="flex min-w-0 items-center gap-3">
              <RunStanding run={run} />
              {run.review && <Scores hash={run.review} chevron={false} className="shrink-0 sm:hidden" />}
              <span className="text-2xs text-muted-foreground tabular-nums sm:hidden" title={dated(run.started)}>
                {when(run.started)}
              </span>
            </span>
          </span>
          {run.review && <Scores hash={run.review} chevron={false} className="hidden shrink-0 sm:inline-flex" />}
          <span className="hidden w-16 shrink-0 text-right text-xs text-muted-foreground tabular-nums sm:block" title={dated(run.started)}>
            {when(run.started)}
          </span>
          <ChevronRight className="size-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5" />
        </div>
      ))}
    </Card>
  );
}
