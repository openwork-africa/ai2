import { cn } from "cn";

export function Mark({ className }: { className?: string }) {
  return (
    <span
      aria-hidden
      className={cn(
        "flex size-7 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-(--shadow-primary-control)",
        className,
      )}
    >
      <svg viewBox="0 0 32 32" className="size-5">
        <path
          d="M9 22.5 13.6 9.5h1.8l4.6 13M10.6 18h7.8"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <circle cx="23" cy="20.5" r="2.1" fill="currentColor" />
      </svg>
    </span>
  );
}
