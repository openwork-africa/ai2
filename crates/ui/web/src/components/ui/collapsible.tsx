"use client";

import { Collapsible as CollapsiblePrimitive } from "@wirekyt/react/collapsible";
import { cn } from "cn";
import type { ComponentProps } from "react";

const Collapsible = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof CollapsiblePrimitive.Root>) => (
  <CollapsiblePrimitive.Root
    ref={ref}
    data-slot="collapsible"
    className={cn("", className)}
    {...props}
  />
);

const CollapsibleTrigger = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof CollapsiblePrimitive.Trigger>) => (
  <CollapsiblePrimitive.Trigger
    ref={ref}
    data-slot="collapsible-trigger"
    className={cn("", className)}
    {...props}
  />
);

const CollapsibleContent = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof CollapsiblePrimitive.Content>) => (
  <CollapsiblePrimitive.Content
    ref={ref}
    data-slot="collapsible-content"
    className={cn("data-open:animate-accordion-down data-closed:animate-accordion-up overflow-hidden duration-300 ease-out [interpolate-size:allow-keywords] motion-reduce:animate-none", className)}
    {...props}
  />
);

const CollapsibleIndicator = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof CollapsiblePrimitive.Indicator>) => (
  <CollapsiblePrimitive.Indicator
    ref={ref}
    data-slot="collapsible-indicator"
    className={cn("", className)}
    {...props}
  />
);

const CollapsibleContext = CollapsiblePrimitive.Context;
const CollapsibleRootProvider = CollapsiblePrimitive.RootProvider;

export {
  Collapsible,
  CollapsibleContent,
  CollapsibleContext,
  CollapsibleIndicator,
  CollapsibleRootProvider,
  CollapsibleTrigger,
};

export {
  useCollapsible,
  useCollapsibleContext,
  type CollapsibleOpenChangeDetails,
} from "@wirekyt/react/collapsible";
