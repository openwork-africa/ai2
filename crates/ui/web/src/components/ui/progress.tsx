"use client";

import { Progress as ProgressPrimitive } from "@wirekyt/react/progress";
import { cn } from "cn";
import * as React from "react";

const Progress = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<typeof ProgressPrimitive.Root>
>(({ className, ...props }, ref) => (
  <ProgressPrimitive.Root
    ref={ref}
    data-slot="progress"
    className={cn("flex flex-wrap gap-3", className)}
    {...props}
  />
));
Progress.displayName = "Progress";

const ProgressTrack = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<typeof ProgressPrimitive.Track>
>(({ className, ...props }, ref) => (
  <ProgressPrimitive.Track
    ref={ref}
    data-slot="progress-track"
    className={cn("relative flex h-1 w-full items-center overflow-x-hidden rounded-2xl bg-muted", className)}
    {...props}
  />
));
ProgressTrack.displayName = "ProgressTrack";

const ProgressRange = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<typeof ProgressPrimitive.Range>
>(({ className, ...props }, ref) => (
  <ProgressPrimitive.Range
    ref={ref}
    data-slot="progress-range"
    className={cn("h-full bg-primary transition-all", className)}
    {...props}
  />
));
ProgressRange.displayName = "ProgressRange";

const ProgressLabel = React.forwardRef<
  HTMLLabelElement,
  React.ComponentProps<typeof ProgressPrimitive.Label>
>(({ className, ...props }, ref) => (
  <ProgressPrimitive.Label
    ref={ref}
    data-slot="progress-label"
    className={cn("text-sm font-medium", className)}
    {...props}
  />
));
ProgressLabel.displayName = "ProgressLabel";

const ProgressValueText = React.forwardRef<
  HTMLSpanElement,
  React.ComponentProps<typeof ProgressPrimitive.ValueText>
>(({ className, ...props }, ref) => (
  <ProgressPrimitive.ValueText
    ref={ref}
    data-slot="progress-value-text"
    className={cn("ml-auto text-sm text-muted-foreground tabular-nums", className)}
    {...props}
  />
));
ProgressValueText.displayName = "ProgressValueText";

const ProgressCircle = React.forwardRef<
  SVGSVGElement,
  React.ComponentProps<typeof ProgressPrimitive.Circle>
>(({ className, ...props }, ref) => (
  <ProgressPrimitive.Circle
    ref={ref}
    data-slot="progress-circle"
    className={cn("size-10 shrink-0 text-muted [--size:2.5rem] [--thickness:0.25rem]", className)}
    {...props}
  />
));
ProgressCircle.displayName = "ProgressCircle";

const ProgressCircleTrack = React.forwardRef<
  SVGCircleElement,
  React.ComponentProps<typeof ProgressPrimitive.CircleTrack>
>(({ className, ...props }, ref) => (
  <ProgressPrimitive.CircleTrack
    ref={ref}
    data-slot="progress-circle-track"
    className={cn("stroke-current", className)}
    {...props}
  />
));
ProgressCircleTrack.displayName = "ProgressCircleTrack";

const ProgressCircleRange = React.forwardRef<
  SVGCircleElement,
  React.ComponentProps<typeof ProgressPrimitive.CircleRange>
>(({ className, ...props }, ref) => (
  <ProgressPrimitive.CircleRange
    ref={ref}
    data-slot="progress-circle-range"
    className={cn("stroke-primary transition-[stroke-dashoffset]", className)}
    {...props}
  />
));
ProgressCircleRange.displayName = "ProgressCircleRange";

const ProgressView = React.forwardRef<
  HTMLSpanElement,
  React.ComponentProps<typeof ProgressPrimitive.View>
>(({ className, ...props }, ref) => (
  <ProgressPrimitive.View
    ref={ref}
    data-slot="progress-view"
    className={cn("relative text-foreground", className)}
    {...props}
  />
));
ProgressView.displayName = "ProgressView";

const ProgressContext = ProgressPrimitive.Context;
const ProgressRootProvider = ProgressPrimitive.RootProvider;

export {
  Progress,
  ProgressTrack,
  ProgressRange,
  ProgressLabel,
  ProgressValueText,
  ProgressCircle,
  ProgressCircleTrack,
  ProgressCircleRange,
  ProgressView,
  ProgressContext,
  ProgressRootProvider,
};
export {
  useProgress,
  useProgressContext,
  type ProgressValueChangeDetails,
} from "@wirekyt/react/progress";
