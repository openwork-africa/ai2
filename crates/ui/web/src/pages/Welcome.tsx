import { ArrowRight } from "lucide-react";
import { cn } from "cn";
import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "react-router";
import { Mark } from "@/components/mark";
import { Button } from "@/components/ui/button";
import { CHAPTERS, welcome } from "@/copy/welcome";

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

export default function Welcome() {
  const navigate = useNavigate();
  const track = useRef<HTMLDivElement>(null);
  const [index, setIndex] = useState(0);
  const last = index === CHAPTERS.length - 1;

  const go = useCallback((to: number) => {
    const el = track.current;
    if (!el) return;
    const target = Math.max(0, Math.min(CHAPTERS.length - 1, to));
    el.scrollTo({ left: target * el.clientWidth, behavior: reduced() ? "auto" : "smooth" });
  }, []);

  const next = useCallback(() => {
    if (last) navigate("/setup");
    else go(index + 1);
  }, [go, index, last, navigate]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.altKey || event.metaKey || event.ctrlKey) return;
      if (event.key === "ArrowRight") {
        event.preventDefault();
        go(index + 1);
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        go(index - 1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go, index]);

  const onScroll = () => {
    const el = track.current;
    if (!el || el.clientWidth === 0) return;
    setIndex(Math.round(el.scrollLeft / el.clientWidth));
  };

  return (
    <div className="surface-light flex h-svh min-h-120 flex-col bg-background text-foreground">
      <header className="flex items-center justify-between px-5 pt-5 sm:px-8 sm:pt-7">
        <Mark className="size-7" />
        <Button variant="ghost" size="sm" asChild className="text-muted-foreground hover:text-foreground">
          <Link to="/setup">{welcome.skip}</Link>
        </Button>
      </header>

      <main className="flex min-h-0 flex-1 flex-col">
        <div
          ref={track}
          onScroll={onScroll}
          aria-roledescription="carousel"
          aria-label={welcome.tagline}
          className="flex min-h-0 flex-1 snap-x snap-mandatory overflow-x-auto overflow-y-hidden overscroll-x-contain [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
        >
          {CHAPTERS.map((chapter, at) => (
            <section
              key={chapter.id}
              aria-roledescription="slide"
              aria-label={`${at + 1} of ${CHAPTERS.length}: ${chapter.label}`}
              aria-hidden={at !== index}
              className="flex w-full shrink-0 snap-center snap-always flex-col items-center justify-center px-6"
            >
              <img
                src={chapter.image}
                alt={chapter.alt}
                width={1024}
                height={1024}
                draggable={false}
                className="-mt-[calc(var(--art)*0.08)] -mb-[calc(var(--art)*0.14)] aspect-square w-(--art) mix-blend-multiply select-none [--art:min(50svh,26rem,88vw)]"
              />
              <div className="flex max-w-md flex-col items-center gap-3 text-center">
                <h1 className="font-heading text-2xl font-semibold tracking-tight text-balance sm:text-[1.75rem]">
                  {chapter.title}
                </h1>
                <p className="text-[0.9375rem] leading-relaxed text-pretty text-muted-foreground">{chapter.body}</p>
              </div>
            </section>
          ))}
        </div>

        <nav className="flex flex-col items-center gap-6 px-6 pt-8 pb-10 sm:pb-14">
          <span className="flex items-center gap-2">
            {CHAPTERS.map((chapter, at) => (
              <button
                key={chapter.id}
                type="button"
                onClick={() => go(at)}
                aria-label={welcome.go(chapter.label)}
                aria-current={at === index ? "step" : undefined}
                className="flex h-6 items-center rounded-full px-0.5 outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
              >
                <span
                  className={cn(
                    "block h-1.5 rounded-full transition-all duration-300 motion-reduce:transition-none",
                    at === index ? "w-5 bg-foreground" : "w-1.5 bg-foreground/20",
                  )}
                />
              </button>
            ))}
          </span>
          <Button
            size="xl"
            onClick={next}
            className="h-11 min-w-36 rounded-full bg-none px-6 text-sm shadow-none outline-transparent hover:bg-primary/90"
          >
            {last ? welcome.setUp : welcome.next}
            <ArrowRight data-icon="inline-end" />
          </Button>
        </nav>
      </main>
    </div>
  );
}
