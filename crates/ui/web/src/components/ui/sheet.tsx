"use client";

import { Portal, wire } from "@wirekyt/react";
import {
  Dialog as DialogPrimitive,
  useDialog as useSheet,
  useDialogContext as useSheetContext,
  type DialogOpenChangeDetails as SheetOpenChangeDetails,
} from "@wirekyt/react/dialog";
import { cn } from "cn";
import { XIcon } from "lucide-react";
import * as React from "react";

function Sheet({
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Root>) {
  return <DialogPrimitive.Root data-slot="sheet" {...props} />;
}

function SheetTrigger({
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Trigger>) {
  return <DialogPrimitive.Trigger data-slot="sheet-trigger" {...props} />;
}

function SheetClose({
  ...props
}: React.ComponentProps<typeof DialogPrimitive.CloseTrigger>) {
  return <DialogPrimitive.CloseTrigger data-slot="sheet-close" {...props} />;
}

function SheetOverlay({
  className,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Backdrop>) {
  return (
    <DialogPrimitive.Backdrop
      data-slot="sheet-overlay"
      className={cn("fixed inset-0 z-50 bg-black/30 transition-opacity duration-150 data-open:animate-in data-open:fade-in-0 data-closed:animate-out data-closed:fade-out-0 data-ending-style:opacity-0 data-starting-style:opacity-0 supports-backdrop-filter:backdrop-blur-sm", className)}
      {...props}
    />
  );
}

function SheetContent({
  className,
  children,
  side = "right",
  showCloseButton = true,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Content> & {
  side?: "top" | "right" | "bottom" | "left";
  showCloseButton?: boolean;
}) {
  return (
    <Portal>
      <SheetOverlay />
      <DialogPrimitive.Positioner className="fixed inset-0 z-50">
        <DialogPrimitive.Content
          data-slot="sheet-content"
          data-side={side}
          className={cn("fixed z-50 flex flex-col bg-popover text-sm text-popover-foreground shadow-(--shadow-dialog) ring-1 ring-border transition duration-200 ease-in-out data-open:animate-in data-open:fade-in-0 data-closed:animate-out data-closed:fade-out-0 data-ending-style:opacity-0 data-starting-style:opacity-0 data-[side=bottom]:inset-x-0 data-[side=bottom]:bottom-0 data-[side=bottom]:h-auto data-[side=bottom]:data-open:slide-in-from-bottom-[2.5rem] data-[side=bottom]:data-closed:slide-out-to-bottom-[2.5rem] data-[side=bottom]:data-ending-style:translate-y-[2.5rem] data-[side=bottom]:data-starting-style:translate-y-[2.5rem] data-[side=left]:inset-y-0 data-[side=left]:left-0 data-[side=left]:h-full data-[side=left]:w-3/4 data-[side=left]:data-open:slide-in-from-left-[2.5rem] data-[side=left]:data-closed:slide-out-to-left-[2.5rem] data-[side=left]:data-ending-style:translate-x-[-2.5rem] data-[side=left]:data-starting-style:translate-x-[-2.5rem] data-[side=right]:inset-y-0 data-[side=right]:right-0 data-[side=right]:h-full data-[side=right]:w-3/4 data-[side=right]:data-open:slide-in-from-right-[2.5rem] data-[side=right]:data-closed:slide-out-to-right-[2.5rem] data-[side=right]:data-ending-style:translate-x-[2.5rem] data-[side=right]:data-starting-style:translate-x-[2.5rem] data-[side=top]:inset-x-0 data-[side=top]:top-0 data-[side=top]:h-auto data-[side=top]:data-open:slide-in-from-top-[2.5rem] data-[side=top]:data-closed:slide-out-to-top-[2.5rem] data-[side=top]:data-ending-style:translate-y-[-2.5rem] data-[side=top]:data-starting-style:translate-y-[-2.5rem] data-[side=left]:sm:max-w-sm data-[side=right]:sm:max-w-sm", className)}
          {...props}
        >
          {children}
          {showCloseButton && (
            <DialogPrimitive.CloseTrigger
              data-slot="sheet-close"
              className="inline-flex shrink-0 items-center justify-center whitespace-nowrap text-xs font-medium outline-1 -outline-offset-1 outline-transparent transition-all select-none outline-solid focus-visible:ring-2 focus-visible:ring-ring/30 focus-visible:outline-ring active:not-aria-[haspopup]:translate-y-px disabled:pointer-events-none disabled:opacity-50 aria-invalid:ring-2 aria-invalid:ring-destructive/20 aria-invalid:outline-destructive dark:aria-invalid:ring-destructive/40 dark:aria-invalid:outline-destructive/50 **:data-badge:inline-flex **:data-badge:h-5 **:data-badge:min-w-5 **:data-badge:shrink-0 **:data-badge:items-center **:data-badge:justify-center **:data-badge:rounded-xs **:data-badge:px-1 **:data-badge:text-2xs **:data-badge:font-medium [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 text-foreground hover:bg-fill-default aria-expanded:bg-fill-default **:data-badge:bg-fill-default size-7 rounded-md absolute top-4 right-4"
            >
              <XIcon />
              <span className="sr-only">Close</span>
            </DialogPrimitive.CloseTrigger>
          )}
        </DialogPrimitive.Content>
      </DialogPrimitive.Positioner>
    </Portal>
  );
}

function SheetHeader({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) {
  return (
    <wire.div
      data-slot="sheet-header"
      className={cn("flex flex-col gap-1.5 p-6", className)}
      {...props}
    />
  );
}

function SheetFooter({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) {
  return (
    <wire.div
      data-slot="sheet-footer"
      className={cn("mt-auto flex flex-col gap-2 p-6", className)}
      {...props}
    />
  );
}

function SheetTitle({
  className,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Title>) {
  return (
    <DialogPrimitive.Title
      data-slot="sheet-title"
      className={cn("font-heading text-base font-medium text-foreground", className)}
      {...props}
    />
  );
}

function SheetDescription({
  className,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Description>) {
  return (
    <DialogPrimitive.Description
      data-slot="sheet-description"
      className={cn("text-sm text-muted-foreground", className)}
      {...props}
    />
  );
}

const SheetContext = DialogPrimitive.Context;
const SheetRootProvider = DialogPrimitive.RootProvider;

export {
  Sheet,
  SheetClose,
  SheetContent,
  SheetContext,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetOverlay,
  SheetRootProvider,
  SheetTitle,
  SheetTrigger,
  useSheet,
  useSheetContext,
  type SheetOpenChangeDetails,
};
