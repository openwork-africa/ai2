import { useEffect, useState } from "react";
import { emptyTranscript, fold, followRun, getRun, type Run, type Transcript } from "@/lib/agents";
import { getReview, type Card } from "@/lib/reviews";

export function useTranscript(id: string | null, seed: Run | null = null) {
  const [state, setState] = useState<{ id: string | null; transcript: Transcript; error: string | null }>({
    id,
    transcript: emptyTranscript(seed),
    error: null,
  });

  useEffect(() => {
    if (!id) return;
    const controller = new AbortController();
    let seen = emptyTranscript(seed?.id === id ? seed : null);
    let finished = false;
    setState({ id, transcript: seen, error: null });
    const loop = async () => {
      if (!seen.run) {
        try {
          seen = { ...seen, run: (await getRun(id)).run };
          setState({ id, transcript: seen, error: null });
        } catch (failure) {
          setState({ id, transcript: seen, error: (failure as Error).message });
          return;
        }
      }
      while (!controller.signal.aborted && !finished) {
        await followRun(
          id,
          (event, at) => {
            if (event.event === "done") finished = true;
            seen = fold(seen, event, at);
            setState({ id, transcript: seen, error: null });
          },
          { signal: controller.signal, lastEventId: seen.last },
        ).catch(() => undefined);
        if (controller.signal.aborted || finished) return;
        await new Promise((resolve) => window.setTimeout(resolve, 1500));
      }
    };
    loop();
    return () => controller.abort();
  }, [id, seed]);

  return state.id === id ? state : { id, transcript: emptyTranscript(seed), error: null };
}

export function useReviewCards(hash: string | null | undefined) {
  const [found, setFound] = useState<{ hash: string; cards: Card[] | null } | null>(null);
  useEffect(() => {
    if (!hash) return;
    let alive = true;
    getReview(hash)
      .then((review) => alive && setFound({ hash, cards: review.ratings }))
      .catch(() => alive && setFound({ hash, cards: null }));
    return () => {
      alive = false;
    };
  }, [hash]);
  if (!hash || found?.hash !== hash) return undefined;
  return found.cards;
}
