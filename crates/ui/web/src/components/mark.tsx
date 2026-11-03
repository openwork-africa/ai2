import { cn } from "cn";

export function Mark({ className }: { className?: string }) {
  return <img alt="" aria-hidden src="/logo-128.png" className={cn("size-7 shrink-0 object-contain", className)} />;
}
