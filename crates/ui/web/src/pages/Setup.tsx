import { Circle } from "lucide-react";
import { Link } from "react-router";
import { Mark } from "@/components/mark";
import { Item, ItemContent, ItemDescription, ItemGroup, ItemMedia, ItemTitle } from "@/components/ui/item";
import { Progress, ProgressRange, ProgressTrack } from "@/components/ui/progress";
import { type Branch, localOnly, STEPS, stepCopy } from "@/copy/setup";

const branch = (): Branch => (/Mac/i.test(navigator.userAgent) ? "macos" : "linux");

export default function Setup() {
  const host = branch();
  const machine = host === "macos" ? "this Mac" : "this machine";
  return (
    <div className="flex min-h-svh flex-col bg-background">
      <header className="sticky top-0 z-10 border-b bg-background/85 backdrop-blur">
        <div className="mx-auto flex w-full max-w-2xl flex-col gap-3 px-5 pt-5 pb-4">
          <div className="flex items-center gap-3">
            <Link to="/" aria-label="ai2 home">
              <Mark />
            </Link>
            <span className="font-heading text-sm font-semibold">Setting up {machine}</span>
            <span className="ml-auto text-xs text-muted-foreground tabular-nums">
              Step 0 of {STEPS.length}
            </span>
          </div>
          <Progress value={0} max={STEPS.length}>
            <ProgressTrack>
              <ProgressRange />
            </ProgressTrack>
          </Progress>
        </div>
      </header>
      <main className="mx-auto flex w-full max-w-2xl flex-1 flex-col gap-6 px-5 py-8">
        <ItemGroup className="gap-1">
          {STEPS.map((id) => {
            const copy = stepCopy(id, host);
            return (
              <Item key={id} className="items-start">
                <ItemMedia variant="icon" className="mt-0.5 text-muted-foreground/60">
                  <Circle />
                </ItemMedia>
                <ItemContent>
                  <ItemTitle className="text-muted-foreground">{copy.title}</ItemTitle>
                  <ItemDescription className="text-muted-foreground/80">{copy.why}</ItemDescription>
                </ItemContent>
              </Item>
            );
          })}
        </ItemGroup>
        <p className="px-3 text-xs text-muted-foreground">{localOnly}</p>
      </main>
    </div>
  );
}
