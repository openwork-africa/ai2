"use client";

import { ScrollArea as ScrollAreaPrimitive } from "@wirekyt/react/scroll-area";
import { cn } from "cn";
import type { ComponentProps } from "react";

const ScrollBar = ({
  className,
  orientation = "vertical",
  ref,
  ...props
}: ComponentProps<typeof ScrollAreaPrimitive.Scrollbar>) => (
  <ScrollAreaPrimitive.Scrollbar
    ref={ref}
    data-slot="scroll-area-scrollbar"
    data-orientation={orientation}
    orientation={orientation}
    className={cn("flex touch-none p-px transition-colors select-none data-horizontal:h-2.5 data-horizontal:flex-col data-vertical:h-full data-vertical:w-2.5", className)}
    {...props}
  >
    <ScrollAreaPrimitive.Thumb
      data-slot="scroll-area-thumb"
      className="relative flex-1 rounded-2xl bg-border"
    />
  </ScrollAreaPrimitive.Scrollbar>
);

const ScrollArea = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof ScrollAreaPrimitive.Root>) => (
  <ScrollAreaPrimitive.Root
    ref={ref}
    data-slot="scroll-area"
    className={cn("relative", className)}
    {...props}
  >
    <ScrollAreaPrimitive.Viewport
      data-slot="scroll-area-viewport"
      className="size-full [scrollbar-width:none] rounded-[inherit] transition-[color,box-shadow] outline-none [-ms-overflow-style:none] focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-ring focus-visible:outline-solid [&::-webkit-scrollbar]:hidden"
    >
      {children}
    </ScrollAreaPrimitive.Viewport>
    <ScrollBar />
    <ScrollAreaPrimitive.Corner />
  </ScrollAreaPrimitive.Root>
);

const ScrollAreaViewport = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof ScrollAreaPrimitive.Viewport>) => (
  <ScrollAreaPrimitive.Viewport
    ref={ref}
    data-slot="scroll-area-viewport"
    className={cn("size-full [scrollbar-width:none] rounded-[inherit] transition-[color,box-shadow] outline-none [-ms-overflow-style:none] focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-ring focus-visible:outline-solid [&::-webkit-scrollbar]:hidden", className)}
    {...props}
  />
);

const ScrollAreaContent = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof ScrollAreaPrimitive.Content>) => (
  <ScrollAreaPrimitive.Content
    ref={ref}
    data-slot="scroll-area-content"
    className={cn("", className)}
    {...props}
  />
);

const ScrollAreaCorner = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof ScrollAreaPrimitive.Corner>) => (
  <ScrollAreaPrimitive.Corner
    ref={ref}
    data-slot="scroll-area-corner"
    className={cn("", className)}
    {...props}
  />
);

const ScrollAreaThumb = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof ScrollAreaPrimitive.Thumb>) => (
  <ScrollAreaPrimitive.Thumb
    ref={ref}
    data-slot="scroll-area-thumb"
    className={cn("relative flex-1 rounded-2xl bg-border", className)}
    {...props}
  />
);


const ScrollAreaContext = ScrollAreaPrimitive.Context;
const ScrollAreaRootProvider = ScrollAreaPrimitive.RootProvider;

export {
  ScrollArea,
  ScrollBar,
  ScrollAreaContent,
  ScrollAreaContext,
  ScrollAreaCorner,
  ScrollAreaRootProvider,
  ScrollAreaThumb,
  ScrollAreaViewport,
};

export {
  useScrollArea,
  useScrollAreaContext,
} from "@wirekyt/react/scroll-area";
