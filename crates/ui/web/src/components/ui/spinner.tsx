"use client";

import { cn } from "cn";
import { Loader2Icon } from "lucide-react";

type SpinnerProps = React.ComponentProps<"svg"> & {
  variant?: "default" | "gradient";
};

const Spinner = ({
  className,
  variant = "default",
  ...props
}: SpinnerProps) => {
  if (variant === "gradient") {
    return (
      <svg
        data-slot="spinner"
        data-variant="gradient"
        role="status"
        aria-label="Loading"
        viewBox="0 0 24 24"
        className={cn("size-4 animate-spin shrink-0 rounded-full", className)}
        {...props}
      />
    );
  }

  return (
    <Loader2Icon
      data-slot="spinner"
      role="status"
      aria-label="Loading"
      className={cn("size-4 animate-spin", className)}
      {...props}
    />
  );
};

export { Spinner };
