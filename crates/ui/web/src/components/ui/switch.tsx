"use client";

import { Switch as SwitchPrimitive } from "@wirekyt/react/switch";
import { cn } from "cn";
import type { ComponentProps } from "react";

const Switch = ({
  className,
  ref,
  size = "default",
  ...props
}: ComponentProps<typeof SwitchPrimitive.Root> & {
  size?: "sm" | "default";
}) => (
  <SwitchPrimitive.Root
    ref={ref}
    data-slot="switch"
    data-size={size}
    className={cn("group/switch inline-flex items-center gap-2", className)}
    {...props}
  />
);

const SwitchControl = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof SwitchPrimitive.Control>) => (
  <SwitchPrimitive.Control
    ref={ref}
    data-slot="switch-control"
    className={cn("relative inline-flex shrink-0 items-center rounded-full border-2 transition-all outline-none group-has-[:focus-visible]/field-label:ring-0 after:absolute after:-inset-x-3 after:-inset-y-2 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30 data-focus-visible:border-ring data-focus-visible:ring-3 data-focus-visible:ring-ring/30 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 data-invalid:border-destructive data-invalid:ring-3 data-invalid:ring-destructive/20 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 dark:data-invalid:border-destructive/50 dark:data-invalid:ring-destructive/40 group-data-[size=default]/switch:h-5 group-data-[size=default]/switch:w-8 group-data-[size=sm]/switch:h-4 group-data-[size=sm]/switch:w-6 data-checked:border-primary data-checked:bg-primary group-has-[:focus-visible]/field-label:data-checked:border-primary data-checked:focus-visible:border-primary data-checked:data-focus-visible:border-primary data-unchecked:border-transparent data-unchecked:bg-fill-hover group-has-[:focus-visible]/field-label:data-unchecked:border-transparent data-disabled:cursor-not-allowed data-disabled:opacity-50 peer", className)}
    {...props}
  />
);

const SwitchThumb = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof SwitchPrimitive.Thumb>) => (
  <SwitchPrimitive.Thumb
    ref={ref}
    data-slot="switch-thumb"
    className={cn("pointer-events-none block rounded-full bg-primary-foreground shadow-sm inset-shadow-[0_0.5px_0_oklch(1_0_0/30%)] ring-0 transition-transform data-checked:inset-shadow-[0_0.5px_0_oklch(1_0_0/25%)] data-unchecked:bg-linear-to-b data-unchecked:from-white/8 data-unchecked:to-transparent data-unchecked:to-70% not-dark:bg-clip-padding group-data-[size=default]/switch:size-4 group-data-[size=sm]/switch:size-3 data-checked:translate-x-[calc(100%-4px)] data-unchecked:translate-x-0 dark:data-unchecked:bg-neutral-500", className)}
    {...props}
  />
);

const SwitchLabel = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof SwitchPrimitive.Label>) => (
  <SwitchPrimitive.Label
    ref={ref}
    data-slot="switch-label"
    className={cn(
      "text-sm leading-none font-medium select-none data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50",
      className
    )}
    {...props}
  />
);

const SwitchHiddenInput = SwitchPrimitive.HiddenInput;

const SwitchContext = SwitchPrimitive.Context;
const SwitchRootProvider = SwitchPrimitive.RootProvider;

export {
  Switch,
  SwitchControl,
  SwitchThumb,
  SwitchLabel,
  SwitchHiddenInput,
  SwitchContext,
  SwitchRootProvider,
};
export {
  useSwitch,
  useSwitchContext,
  type SwitchCheckedChangeDetails,
} from "@wirekyt/react/switch";
