import { cn } from "cn";
import { ChevronRight, ShieldCheck, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import { Code, Page, PageHeader } from "@/components/page";
import { ScorePill } from "@/components/review";
import { StartReview } from "@/components/review-start";
import { SettingsCog } from "@/components/settings-cog";
import { Card } from "@/components/ui/card";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { dated, folderName, listReviews, type ReviewList, type Score, shortFolder, when } from "@/lib/reviews";

const scale = (ratings: Score[], key: string) => ratings.find((rating) => rating.key === key);

function Loading() {
  return (
    <Card className="gap-0 py-0">
      {[0, 1, 2].map((row) => (
        <div key={row} className="flex items-center gap-4 border-b px-5 py-4 last:border-0">
          <div className="flex flex-1 flex-col gap-2">
            <Skeleton className="h-4 w-40" />
            <Skeleton className="h-3 w-64" />
          </div>
          <Skeleton className="h-6 w-12 rounded-full" />
          <Skeleton className="h-6 w-12 rounded-full" />
          <Skeleton className="h-4 w-14" />
        </div>
      ))}
    </Card>
  );
}

function NoReviews() {
  return (
    <Empty className="rounded-3xl border border-dashed py-16">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <ShieldCheck />
        </EmptyMedia>
        <EmptyTitle>Nothing has been reviewed yet</EmptyTitle>
        <EmptyDescription>Each agent run shows up here once it has been checked, using the command below.</EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Code>ai2 --objective "Add a pricing page"</Code>
      </EmptyContent>
    </Empty>
  );
}

export default function Reviews() {
  const [list, setList] = useState<ReviewList | null>(null);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  const load = useCallback(() => {
    listReviews()
      .then((loaded) => {
        setList(loaded);
        setError(null);
      })
      .catch((failure: Error) => setError(failure.message));
  }, []);

  useEffect(load, [load]);

  const keys = list?.reviews[0]?.ratings.map((rating) => ({ key: rating.key, short: rating.short })) ?? [
    { key: "msr", short: "MSR" },
    { key: "rrf", short: "RRF" },
  ];

  return (
    <Page>
      <PageHeader
        title="Security"
        description="Every agent run checked on this machine, with a safety score for each."
        actions={
          <>
            <SettingsCog page="security" />
            {list?.can_start && <StartReview onDone={load} />}
          </>
        }
      />
      {error && !list && (
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <TriangleAlert />
            </EmptyMedia>
            <EmptyTitle>The security checks wouldn't load</EmptyTitle>
            <EmptyDescription>{error}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      )}
      {!list && !error && <Loading />}
      {list && list.reviews.length === 0 && <NoReviews />}
      {list && list.reviews.length > 0 && (
        <Card className="gap-0 py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-4 text-xs text-muted-foreground sm:pl-5">Folder</TableHead>
                {keys.map((key) => (
                  <TableHead key={key.key} className="w-14 px-1 text-center text-xs text-muted-foreground sm:w-20 sm:px-3">
                    {key.short}
                  </TableHead>
                ))}
                <TableHead className="hidden w-28 text-xs text-muted-foreground sm:table-cell">Findings</TableHead>
                <TableHead className="hidden w-24 text-right text-xs text-muted-foreground sm:table-cell">When</TableHead>
                <TableHead className="w-6 pr-4 sm:w-10 sm:pr-5" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {list.reviews.map((review) => (
                <TableRow
                  key={review.hash}
                  className="group cursor-pointer"
                  onClick={() => navigate(`/reviews/${review.hash}`)}
                >
                  <TableCell className="max-w-0 py-3.5 pl-4 sm:pl-5">
                    <div className="flex min-w-0 flex-col gap-0.5">
                      <Link
                        to={`/reviews/${review.hash}`}
                        className="truncate font-medium outline-none focus-visible:underline"
                        onClick={(event) => event.stopPropagation()}
                      >
                        {folderName(review.folder)}
                      </Link>
                      <span className="truncate font-mono text-2xs text-muted-foreground" title={review.folder}>
                        {shortFolder(review.folder)}
                        <span className="font-sans sm:hidden"> · {when(review.generated_at)}</span>
                      </span>
                      {review.objective && (
                        <span className="truncate text-xs text-muted-foreground">“{review.objective}”</span>
                      )}
                    </div>
                  </TableCell>
                  {keys.map((key) => {
                    const rating = scale(review.ratings, key.key);
                    return (
                      <TableCell key={key.key} className="px-1 text-center sm:px-3">
                        <ScorePill score={rating?.score ?? null} riskier={rating?.riskier ?? false} />
                      </TableCell>
                    );
                  })}
                  <TableCell className="hidden text-xs sm:table-cell">
                    {review.findings === 0 ? (
                      <span className="text-muted-foreground">None</span>
                    ) : (
                      <span className="flex flex-col">
                        <span className={cn("font-medium", review.open_findings > 0 && "text-warning")}>
                          {review.open_findings > 0 ? `${review.open_findings} open` : "All fixed"}
                        </span>
                        <span className="text-muted-foreground">of {review.findings}</span>
                      </span>
                    )}
                  </TableCell>
                  <TableCell
                    className="hidden text-right text-xs text-muted-foreground tabular-nums sm:table-cell"
                    title={dated(review.generated_at)}
                  >
                    {when(review.generated_at)}
                  </TableCell>
                  <TableCell className="pr-4 sm:pr-5">
                    <ChevronRight className="size-4 text-muted-foreground transition-transform group-hover:translate-x-0.5" />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </Page>
  );
}
