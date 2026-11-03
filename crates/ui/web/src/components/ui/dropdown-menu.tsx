"use client";

import { Portal, wire } from "@wirekyt/react";
import { Menu as DropdownMenuPrimitive } from "@wirekyt/react/menu";
import { cn } from "cn";
import { CheckIcon, ChevronRightIcon } from "lucide-react";
import * as React from "react";

function DropdownMenu({
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Root>) {
  return (
    <DropdownMenuPrimitive.Root
      data-slot="dropdown-menu"
      lazyMount
      unmountOnExit
      {...props}
    />
  );
}

function DropdownMenuPortal({ children }: { children: React.ReactNode }) {
  return <Portal>{children}</Portal>;
}

function DropdownMenuTrigger({
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Trigger>) {
  return (
    <DropdownMenuPrimitive.Trigger
      data-slot="dropdown-menu-trigger"
      {...props}
    />
  );
}

function DropdownMenuContent({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Content>) {
  return (
    <DropdownMenuPortal>
      <DropdownMenuPrimitive.Positioner className="isolate outline-none">
        <DropdownMenuPrimitive.Content
          data-slot="dropdown-menu-content"
          className={cn(
            "data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 data-closed:zoom-out-95 data-open:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 ring-foreground/10 bg-popover text-popover-foreground min-w-32 rounded-lg p-1 shadow-md ring-1 duration-100 data-[side=inline-start]:slide-in-from-right-2 data-[side=inline-end]:slide-in-from-left-2 rounded-14 bg-popover text-popover-foreground ring-border data-[side=bottom]:slide-in-from-top-2 data-[side=inline-end]:slide-in-from-left-2 data-[side=inline-start]:slide-in-from-right-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95 z-[calc(50_+_var(--layer-index,0))] max-h-(--available-height) w-(--reference-width) min-w-32 origin-(--transform-origin) overflow-x-hidden overflow-y-auto p-1 shadow-(--shadow-popover) ring-1 duration-100 outline-none data-closed:overflow-hidden",
            className
          )}
          {...props}
        />
      </DropdownMenuPrimitive.Positioner>
    </DropdownMenuPortal>
  );
}

function DropdownMenuGroup({
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.ItemGroup>) {
  return (
    <DropdownMenuPrimitive.ItemGroup
      data-slot="dropdown-menu-group"
      {...props}
    />
  );
}

function DropdownMenuItem({
  className,
  inset,
  variant = "default",
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Item> & {
  inset?: boolean;
  variant?: "default" | "destructive";
}) {
  return (
    <DropdownMenuPrimitive.Item
      data-slot="dropdown-menu-item"
      data-inset={inset}
      data-variant={variant}
      className={cn(
        "focus:bg-accent data-highlighted:bg-accent focus:text-accent-foreground data-highlighted:text-accent-foreground data-[variant=destructive]:text-destructive data-[variant=destructive]:focus:bg-destructive/10 data-[variant=destructive]:data-highlighted:bg-destructive/10 dark:data-[variant=destructive]:focus:bg-destructive/20 dark:data-[variant=destructive]:data-highlighted:bg-destructive/20 data-[variant=destructive]:focus:text-destructive data-[variant=destructive]:data-highlighted:text-destructive data-[variant=destructive]:*:[svg]:text-destructive not-data-[variant=destructive]:focus:**:text-accent-foreground not-data-[variant=destructive]:data-highlighted:**:text-accent-foreground min-h-7 gap-2 rounded-md px-2 py-1 text-xs/relaxed data-inset:pl-7.5 [&_svg:not([class*='size-'])]:size-3.5 group/dropdown-menu-item focus:bg-accent focus:text-accent-foreground not-data-[variant=destructive]:focus:**:not-in-data-[slot=avatar]:text-accent-foreground data-[variant=destructive]:text-destructive-foreground data-[variant=destructive]:focus:bg-destructive/10 data-[variant=destructive]:focus:text-destructive-foreground dark:data-[variant=destructive]:focus:bg-destructive/20 data-[variant=destructive]:*:[svg]:text-destructive-foreground relative flex cursor-default items-center gap-2 rounded-lg px-2 py-1.5 text-sm outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 data-inset:pl-9 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&>svg:not([class*='ml-auto'])]:mx-0.5",
        className
      )}
      {...props}
    />
  );
}

function DropdownMenuCheckboxItem({
  className,
  children,
  checked,
  inset,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.CheckboxItem> & {
  inset?: boolean;
}) {
  return (
    <DropdownMenuPrimitive.CheckboxItem
      data-slot="dropdown-menu-checkbox-item"
      data-inset={inset}
      className={cn(
        "focus:bg-accent data-highlighted:bg-accent focus:text-accent-foreground data-highlighted:text-accent-foreground focus:**:text-accent-foreground data-highlighted:**:text-accent-foreground min-h-7 gap-2 rounded-md py-1.5 pr-8 pl-2 text-xs data-inset:pl-7.5 [&_svg:not([class*='size-'])]:size-3.5 focus:bg-accent focus:text-accent-foreground focus:**:not-in-data-[slot=avatar]:text-accent-foreground relative flex cursor-default items-center gap-2 rounded-lg py-1.5 pr-8 pl-2 text-sm outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 data-inset:pl-9 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&>svg:not([class*='ml-auto'])]:mx-0.5",
        className
      )}
      checked={checked}
      {...props}
    >
      <wire.span
        className="absolute right-2 flex items-center justify-center pointer-events-none absolute right-2 flex items-center justify-center"
        data-slot="dropdown-menu-checkbox-item-indicator"
      >
        <DropdownMenuPrimitive.ItemIndicator>
          <CheckIcon />
        </DropdownMenuPrimitive.ItemIndicator>
      </wire.span>
      {children}
    </DropdownMenuPrimitive.CheckboxItem>
  );
}

function DropdownMenuRadioGroup({
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.RadioItemGroup>) {
  return (
    <DropdownMenuPrimitive.RadioItemGroup
      data-slot="dropdown-menu-radio-group"
      {...props}
    />
  );
}

function DropdownMenuRadioItem({
  className,
  children,
  inset,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.RadioItem> & {
  inset?: boolean;
}) {
  return (
    <DropdownMenuPrimitive.RadioItem
      data-slot="dropdown-menu-radio-item"
      data-inset={inset}
      className={cn(
        "focus:bg-accent data-highlighted:bg-accent focus:text-accent-foreground data-highlighted:text-accent-foreground focus:**:text-accent-foreground data-highlighted:**:text-accent-foreground min-h-7 gap-2 rounded-md py-1.5 pr-8 pl-2 text-xs data-inset:pl-7.5 [&_svg:not([class*='size-'])]:size-3.5 focus:bg-accent focus:text-accent-foreground focus:**:not-in-data-[slot=avatar]:text-accent-foreground relative flex cursor-default items-center gap-2 rounded-lg py-1.5 pr-8 pl-2 text-sm outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 data-inset:pl-9 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&>svg:not([class*='ml-auto'])]:mx-0.5",
        className
      )}
      {...props}
    >
      <wire.span
        className="absolute right-2 flex items-center justify-center pointer-events-none absolute right-2 flex items-center justify-center"
        data-slot="dropdown-menu-radio-item-indicator"
      >
        <DropdownMenuPrimitive.ItemIndicator>
          <CheckIcon />
        </DropdownMenuPrimitive.ItemIndicator>
      </wire.span>
      {children}
    </DropdownMenuPrimitive.RadioItem>
  );
}

function DropdownMenuLabel({
  className,
  inset,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.ItemGroupLabel> & {
  inset?: boolean;
}) {
  return (
    <DropdownMenuPrimitive.ItemGroupLabel
      data-slot="dropdown-menu-label"
      data-inset={inset}
      className={cn(
        "text-muted-foreground px-2 py-1.5 text-xs data-inset:pl-7.5 text-2xs text-muted-foreground px-2 py-1 data-inset:pl-9",
        className
      )}
      {...props}
    />
  );
}

function DropdownMenuSeparator({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Separator>) {
  return (
    <DropdownMenuPrimitive.Separator
      data-slot="dropdown-menu-separator"
      className={cn(
        "bg-border/50 -mx-1 my-1 h-px bg-border -mx-1 mt-1 mb-[3px] h-px",
        className
      )}
      {...props}
    />
  );
}

function DropdownMenuShortcut({
  className,
  ...props
}: React.ComponentProps<typeof wire.span>) {
  return (
    <wire.span
      data-slot="dropdown-menu-shortcut"
      className={cn(
        "text-muted-foreground group-focus/dropdown-menu-item:text-accent-foreground group-data-highlighted/dropdown-menu-item:text-accent-foreground ml-auto text-[0.625rem] tracking-widest text-2xs text-muted-foreground group-focus/dropdown-menu-item:text-accent-foreground ml-auto tracking-widest",
        className
      )}
      {...props}
    />
  );
}

function DropdownMenuSub({
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Root>) {
  return (
    <DropdownMenuPrimitive.Root
      data-slot="dropdown-menu-sub"
      lazyMount
      unmountOnExit
      {...props}
    />
  );
}

function DropdownMenuSubTrigger({
  className,
  inset,
  children,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.TriggerItem> & {
  inset?: boolean;
}) {
  return (
    <DropdownMenuPrimitive.TriggerItem
      data-slot="dropdown-menu-sub-trigger"
      data-inset={inset}
      className={cn(
        "focus:bg-accent data-highlighted:bg-accent focus:text-accent-foreground data-highlighted:text-accent-foreground data-open:bg-accent data-open:text-accent-foreground not-data-[variant=destructive]:focus:**:text-accent-foreground not-data-[variant=destructive]:data-highlighted:**:text-accent-foreground min-h-7 gap-2 rounded-md px-2 py-1 text-xs data-inset:pl-7.5 [&_svg:not([class*='size-'])]:size-3.5 focus:bg-accent focus:text-accent-foreground not-data-[variant=destructive]:focus:**:not-in-data-[slot=avatar]:not-data-[slot=dropdown-menu-sub-trigger-icon]:text-accent-foreground data-open:bg-accent data-open:text-accent-foreground flex cursor-default items-center gap-2 rounded-lg px-2 py-1.5 text-sm outline-hidden select-none data-inset:pl-9 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&>svg:not([class*='ml-auto'])]:mx-0.5",
        className
      )}
      {...props}
    >
      {children}
      <ChevronRightIcon
        data-slot="dropdown-menu-sub-trigger-icon"
        className="text-muted-foreground ml-auto rtl:-scale-x-100"
      />
    </DropdownMenuPrimitive.TriggerItem>
  );
}

function DropdownMenuSubContent({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Content>) {
  return (
    <Portal>
      <DropdownMenuPrimitive.Positioner className="isolate outline-none">
        <DropdownMenuPrimitive.Content
          data-slot="dropdown-menu-sub-content"
          className={cn(
            "data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 data-closed:zoom-out-95 data-open:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 ring-foreground/10 bg-popover text-popover-foreground min-w-32 rounded-lg p-1 shadow-md ring-1 duration-100 data-[side=inline-start]:slide-in-from-right-2 data-[side=inline-end]:slide-in-from-left-2 rounded-14 bg-popover text-popover-foreground ring-border data-[side=bottom]:slide-in-from-top-2 data-[side=inline-end]:slide-in-from-left-2 data-[side=inline-start]:slide-in-from-right-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95 z-[calc(50_+_var(--layer-index,0))] max-h-(--available-height) w-auto min-w-[96px] origin-(--transform-origin) overflow-x-hidden overflow-y-auto p-1 shadow-(--shadow-popover) ring-1 duration-100 outline-none data-closed:overflow-hidden",
            className
          )}
          {...props}
        />
      </DropdownMenuPrimitive.Positioner>
    </Portal>
  );
}

function DropdownMenuArrow({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Arrow>) {
  return (
    <DropdownMenuPrimitive.Arrow
      data-slot="dropdown-menu-arrow"
      className={cn("", className)}
      {...props}
    >
      <DropdownMenuPrimitive.ArrowTip />
    </DropdownMenuPrimitive.Arrow>
  );
}

function DropdownMenuItemText({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.ItemText>) {
  return (
    <DropdownMenuPrimitive.ItemText
      data-slot="dropdown-menu-item-text"
      className={cn("", className)}
      {...props}
    />
  );
}

const DropdownMenuContext = DropdownMenuPrimitive.Context;
const DropdownMenuRootProvider = DropdownMenuPrimitive.RootProvider;

export {
  DropdownMenu,
  DropdownMenuPortal,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuItem,
  DropdownMenuCheckboxItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubTrigger,
  DropdownMenuSubContent,
  DropdownMenuArrow,
  DropdownMenuItemText,
  DropdownMenuContext,
  DropdownMenuRootProvider,
};

export {
  useMenu as useDropdownMenu,
  useMenuContext as useDropdownMenuContext,
} from "@wirekyt/react/menu";
