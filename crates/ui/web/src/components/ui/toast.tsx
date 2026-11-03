"use client";

import {
  Toaster as ToasterPrimitive,
  createToaster,
  Toast,
} from "@wirekyt/react/toast";
import { cn } from "cn";
import {
  CircleCheckIcon,
  InfoIcon,
  Loader2Icon,
  OctagonXIcon,
  TriangleAlertIcon,
  XIcon,
} from "lucide-react";

const icons = {
  success: <CircleCheckIcon aria-hidden="true" className="size-4" />,
  info: <InfoIcon aria-hidden="true" className="size-4" />,
  warning: <TriangleAlertIcon aria-hidden="true" className="size-4" />,
  error: <OctagonXIcon aria-hidden="true" className="size-4" />,
  loading: <Loader2Icon aria-hidden="true" className="size-4 animate-spin" />,
} as const;

const toaster = createToaster({
  placement: "bottom-end",
  overlap: true,
  gap: 16,
});

function Toaster() {
  return (
    <ToasterPrimitive
      toaster={toaster}
      data-slot="toast"
      className="pointer-events-auto"
    >
      {(toast) => (
        <Toast.Root
          key={toast.id}
          className={cn(
            "border-border bg-popover text-popover-foreground pointer-events-auto z-(--z-index) flex h-(--height) w-[356px] translate-x-(--x) translate-y-(--y) scale-(--scale) items-start gap-3 rounded-2xl rounded-lg border p-4 opacity-(--opacity) shadow-lg ease-[cubic-bezier(0.21,1.02,0.73,1)] [will-change:translate,opacity,scale] [transition:translate_400ms,scale_400ms,opacity_400ms,height_400ms,box-shadow_200ms] data-[state=closed]:ease-[cubic-bezier(0.06,0.71,0.55,1)] data-[state=closed]:[transition:translate_400ms,scale_400ms,opacity_200ms]"
          )}
        >
          {toast.type && toast.type in icons && (
            <span className={cn("text-muted-foreground mt-0.5 shrink-0")}>
              {icons[toast.type as keyof typeof icons]}
            </span>
          )}
          <div className="flex flex-1 flex-col gap-1">
            {toast.title && (
              <Toast.Title className="text-sm font-semibold">
                {toast.title}
              </Toast.Title>
            )}
            {toast.description && (
              <Toast.Description className="text-muted-foreground text-sm">
                {toast.description}
              </Toast.Description>
            )}
            {toast.action && (
              <Toast.ActionTrigger className="text-primary hover:text-primary/80 mt-1 inline-flex w-fit text-sm font-medium">
                {toast.action.label}
              </Toast.ActionTrigger>
            )}
          </div>
          <Toast.CloseTrigger className="text-muted-foreground hover:text-foreground shrink-0 rounded-md p-0.5 opacity-70 transition-opacity hover:opacity-100">
            <XIcon aria-hidden="true" className="size-4" />
          </Toast.CloseTrigger>
        </Toast.Root>
      )}
    </ToasterPrimitive>
  );
}

export { Toaster, toaster as toast };

export { type ToastStatus } from "@wirekyt/react/toast";
