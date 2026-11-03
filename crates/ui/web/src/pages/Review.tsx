import { FileSearch } from "lucide-react";
import { useEffect, useState } from "react";
import { useParams } from "react-router";
import { Code, Page, PageHeader } from "@/components/page";
import { ReviewBody, ReviewHeader } from "@/components/review";
import { Card } from "@/components/ui/card";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { ApiError } from "@/lib/api";
import { getReview, type Review as ReviewData } from "@/lib/reviews";

type Loaded = { hash: string; review: ReviewData | null; error: ApiError | Error | null };

export default function Review() {
  const { hash = "" } = useParams();
  const [loaded, setLoaded] = useState<Loaded | null>(null);

  useEffect(() => {
    let alive = true;
    getReview(hash)
      .then((review) => alive && setLoaded({ hash, review, error: null }))
      .catch((error: Error) => alive && setLoaded({ hash, review: null, error }));
    return () => {
      alive = false;
    };
  }, [hash]);

  const current = loaded?.hash === hash ? loaded : null;
  const review = current?.review;
  const back = { to: "/reviews", label: "Security" };

  if (current?.error) {
    const missing = current.error instanceof ApiError && (current.error.status === 404 || current.error.status === 400);
    return (
      <Page>
        <PageHeader back={back} title="Security review" />
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <FileSearch />
            </EmptyMedia>
            <EmptyTitle>{missing ? "No review at this address" : "The review wouldn't load"}</EmptyTitle>
            <EmptyDescription>
              {missing ? "It may have been removed, or that folder hasn't had a review yet." : current.error.message}
            </EmptyDescription>
          </EmptyHeader>
          {missing && (
            <EmptyContent>
              <Code>ai2 --run-path &lt;folder&gt;</Code>
            </EmptyContent>
          )}
        </Empty>
      </Page>
    );
  }

  if (!review) {
    return (
      <Page>
        <PageHeader back={back} title="Security review" description={<Skeleton className="h-4 w-72" />} />
        <div className="grid gap-4 md:grid-cols-2">
          {[0, 1].map((tile) => (
            <Card key={tile} className="h-72 p-5">
              <Skeleton className="h-5 w-40" />
              <Skeleton className="h-3 w-56" />
              <Skeleton className="mt-6 h-2 w-full" />
              <Skeleton className="h-2 w-full" />
              <Skeleton className="h-2 w-2/3" />
            </Card>
          ))}
        </div>
      </Page>
    );
  }

  return (
    <Page>
      <PageHeader back={back} title="Security review">
        <ReviewHeader review={review} />
      </PageHeader>
      <ReviewBody review={review} />
    </Page>
  );
}
