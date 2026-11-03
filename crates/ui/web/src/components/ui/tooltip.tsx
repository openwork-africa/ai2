"use client";

import { Portal } from "@wirekyt/react";
import { Tooltip as TooltipPrimitive } from "@wirekyt/react/tooltip";
import { cn } from "cn";
import * as React from "react";

const TooltipProvider = ({ children }: { children: React.ReactNode }) =>
  children;

const Tooltip = ({
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Root>) => (
  <TooltipPrimitive.Root data-slot="tooltip" {...props} />
);

const TooltipTrigger = ({
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Trigger>) => (
  <TooltipPrimitive.Trigger data-slot="tooltip-trigger" {...props} />
);

const TooltipPositioner = ({
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Positioner>) => (
  <TooltipPrimitive.Positioner data-slot="tooltip-positioner" {...props} />
);

const tooltipLabels = (children: React.ReactNode) => {
  const nodes: React.ReactNode[] = [];
  let text = "";
  const all = React.Children.toArray(children);

  for (const [index, child] of all.entries()) {
    if (typeof child !== "string" && typeof child !== "number") {
      nodes.push(child);
      continue;
    }

    text += child;
    const next = all[index + 1];

    if (typeof next === "string" || typeof next === "number") {
      continue;
    }

    if (text.trim()) {
      nodes.push(
        <span
          key={`label-${index}`}
          data-slot="tooltip-label"
          className="max-w-62"
        >
          {text}
        </span>
      );
    }

    text = "";
  }

  return nodes;
};

const TooltipArrow = ({
  className,
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Arrow>) => (
  <TooltipPrimitive.Arrow
    data-slot="tooltip-arrow"
    className={cn(
      "z-50 [--arrow-background:var(--color-foreground)] [--arrow-size:0.625rem]",
      className
    )}
    {...props}
  >
    <TooltipPrimitive.ArrowTip className="rounded-[2px]" />
  </TooltipPrimitive.Arrow>
);

const TooltipContent = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<typeof TooltipPrimitive.Content>
>(
  function TooltipContent({ className, children, ...props }, ref) {
    return (
      <Portal>
        <TooltipPrimitive.Positioner className="isolate z-50">
          <TooltipPrimitive.Content
            ref={ref}
            data-slot="tooltip-content"
            className={cn("z-50 flex w-fit max-w-80 origin-(--transform-origin) items-center gap-2 rounded-md bg-foreground px-2 py-1 text-2xs text-background has-data-[slot=kbd]:pr-1 data-[side=bottom]:slide-in-from-top-2 data-[side=inline-end]:slide-in-from-left-2 data-[side=inline-start]:slide-in-from-right-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95 **:data-[slot=kbd]:relative **:data-[slot=kbd]:isolate **:data-[slot=kbd]:z-50", className)}
            {...props}
          >
            {tooltipLabels(children)}
            <TooltipArrow />
          </TooltipPrimitive.Content>
        </TooltipPrimitive.Positioner>
      </Portal>
    );
  }
);
TooltipContent.displayName = "TooltipContent";

const TooltipContext = TooltipPrimitive.Context;
const TooltipRootProvider = TooltipPrimitive.RootProvider;

export {
  Tooltip,
  TooltipArrow,
  TooltipContent,
  TooltipContext,
  TooltipPositioner,
  TooltipProvider,
  TooltipRootProvider,
  TooltipTrigger,
};
export {
  useTooltip,
  useTooltipContext,
  type TooltipOpenChangeDetails,
} from "@wirekyt/react/tooltip";
