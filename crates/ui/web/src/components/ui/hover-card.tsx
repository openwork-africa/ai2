"use client";

import { Portal } from "@wirekyt/react";
import {
  HoverCard as HoverCardPrimitive,
  useHoverCard,
  useHoverCardContext,
  type HoverCardOpenChangeDetails,
} from "@wirekyt/react/hover-card";
import { cn } from "cn";
import * as React from "react";

function HoverCard({
  ...props
}: React.ComponentProps<typeof HoverCardPrimitive.Root>) {
  return <HoverCardPrimitive.Root data-slot="hover-card" {...props} />;
}

function HoverCardTrigger({
  ...props
}: React.ComponentProps<typeof HoverCardPrimitive.Trigger>) {
  return (
    <HoverCardPrimitive.Trigger data-slot="hover-card-trigger" {...props} />
  );
}

const HoverCardContent = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<typeof HoverCardPrimitive.Content>
>(({ className, ...props }, ref) => (
  <Portal>
    <HoverCardPrimitive.Positioner className="isolate z-50">
      <HoverCardPrimitive.Content
        ref={ref}
        data-slot="hover-card-content"
        className={cn("z-50 w-72 origin-(--transform-origin) rounded-14 bg-popover p-4 text-sm text-popover-foreground shadow-(--shadow-popover) ring-1 ring-border outline-hidden duration-100 data-[side=bottom]:slide-in-from-top-2 data-[side=inline-end]:slide-in-from-left-2 data-[side=inline-start]:slide-in-from-right-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95", className)}
        {...props}
      />
    </HoverCardPrimitive.Positioner>
  </Portal>
));
HoverCardContent.displayName = "HoverCardContent";

function HoverCardArrow({
  className,
  ...props
}: React.ComponentProps<typeof HoverCardPrimitive.Arrow>) {
  return (
    <HoverCardPrimitive.Arrow
      data-slot="hover-card-arrow"
      className={cn("[--arrow-background:var(--color-popover)] [--arrow-size:0.625rem]", className)}
      {...props}
    >
      <HoverCardPrimitive.ArrowTip />
    </HoverCardPrimitive.Arrow>
  );
}

const HoverCardContext = HoverCardPrimitive.Context;
const HoverCardRootProvider = HoverCardPrimitive.RootProvider;

export {
  HoverCard,
  HoverCardArrow,
  HoverCardContent,
  HoverCardContext,
  HoverCardRootProvider,
  HoverCardTrigger,
  useHoverCard,
  useHoverCardContext,
  type HoverCardOpenChangeDetails,
};
