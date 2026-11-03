import { cn } from "cn";

export function Mark({ className }: { className?: string }) {
  return (
    <svg aria-hidden viewBox="0 0 820 820" className={cn("size-7 shrink-0", className)}>
      <rect width="820" height="820" rx="184" fill="#1c1b1c" />
      <path
        fill="#fff"
        fillRule="evenodd"
        d="M154 457.5a161 177.5 0 1 0 322 0a161 177.5 0 1 0-322 0ZM227 457.5a97 114.5 0 1 0 194 0a97 114.5 0 1 0-194 0Z"
      />
      <rect x="421" y="285" width="70" height="348" rx="14" fill="#fff" />
      <rect x="565" y="312" width="73" height="321" rx="14" fill="#fff" />
      <rect x="561" y="185" width="81" height="81" rx="15" fill="#6f9b70" />
    </svg>
  );
}
