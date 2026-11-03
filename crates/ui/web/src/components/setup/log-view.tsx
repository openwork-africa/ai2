import { cn } from "cn";
import { useEffect, useRef } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { stepText } from "@/copy/setup";
import type { LogLine } from "@/lib/setup";

const tone: Record<LogLine["stream"], string> = {
  cmd: "text-foreground font-medium",
  say: "text-foreground/85",
  out: "text-muted-foreground",
  err: "text-muted-foreground",
};

export function LogLineText({ line, className }: { line: LogLine; className?: string }) {
  return (
    <span className={cn("block truncate font-mono text-[0.6875rem] leading-5 [font-variant-ligatures:none]", tone[line.stream], className)}>
      {line.stream === "say" ? `› ${line.line}` : line.line}
    </span>
  );
}

export function LogView({ lines, className }: { lines: LogLine[]; className?: string }) {
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const viewport = end.current?.closest('[data-slot="scroll-area-viewport"]');
    if (viewport) viewport.scrollTop = viewport.scrollHeight;
  }, [lines.length]);
  return (
    <ScrollArea className={cn("h-56 rounded-xl bg-fill-subtle ring-1 ring-border", className)}>
      <div className="flex flex-col px-3 py-2.5">
        {lines.length === 0 && <span className="text-xs text-muted-foreground">{stepText.noLog}</span>}
        {lines.map((line, index) => (
          <span
            key={index}
            className={cn(
              "font-mono text-[0.6875rem] leading-5 break-all whitespace-pre-wrap [font-variant-ligatures:none]",
              tone[line.stream],
            )}
          >
            {line.stream === "say" ? `› ${line.line}` : line.line}
          </span>
        ))}
        <div ref={end} />
      </div>
    </ScrollArea>
  );
}

export const logText = (lines: LogLine[]) =>
  lines.map((line) => (line.stream === "say" ? `ai2 setup: ${line.line}` : line.line)).join("\n");
