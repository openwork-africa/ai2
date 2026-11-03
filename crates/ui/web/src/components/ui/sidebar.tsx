"use client";

import { wire } from "@wirekyt/react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "cn";
import { PanelLeftIcon } from "lucide-react";
import * as React from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useIsMobile } from "@/hooks/use-mobile";

const SIDEBAR_COOKIE_NAME = "sidebar_state";
const SIDEBAR_COOKIE_MAX_AGE = 60 * 60 * 24 * 7;
const SIDEBAR_WIDTH = "16.25rem";
const SIDEBAR_WIDTH_MOBILE = "18rem";
const SIDEBAR_WIDTH_ICON = "3rem";
const SIDEBAR_KEYBOARD_SHORTCUT = "b";

type SidebarContextProps = {
  state: "expanded" | "collapsed";
  open: boolean;
  setOpen: (open: boolean) => void;
  openMobile: boolean;
  setOpenMobile: (open: boolean) => void;
  isMobile: boolean;
  toggleSidebar: () => void;
};

const SidebarContext = React.createContext<SidebarContextProps | null>(null);

function useSidebar() {
  const context = React.useContext(SidebarContext);
  if (!context) {
    throw new Error("useSidebar must be used within a SidebarProvider.");
  }

  return context;
}

function SidebarProvider({
  defaultOpen = true,
  open: openProp,
  onOpenChange: setOpenProp,
  className,
  style,
  children,
  ...props
}: React.ComponentProps<"div"> & {
  defaultOpen?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}) {
  const isMobile = useIsMobile();
  const [openMobile, setOpenMobile] = React.useState(false);

  const [_open, _setOpen] = React.useState(defaultOpen);
  const open = openProp ?? _open;
  const setOpen = React.useCallback(
    (value: boolean | ((value: boolean) => boolean)) => {
      const openState = typeof value === "function" ? value(open) : value;
      if (setOpenProp) {
        setOpenProp(openState);
      } else {
        _setOpen(openState);
      }

      document.cookie = `${SIDEBAR_COOKIE_NAME}=${openState}; path=/; max-age=${SIDEBAR_COOKIE_MAX_AGE}`;
    },
    [setOpenProp, open]
  );

  const toggleSidebar = React.useCallback(() => {
    return isMobile ? setOpenMobile((open) => !open) : setOpen((open) => !open);
  }, [isMobile, setOpen, setOpenMobile]);

  React.useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.key === SIDEBAR_KEYBOARD_SHORTCUT &&
        (event.metaKey || event.ctrlKey)
      ) {
        event.preventDefault();
        toggleSidebar();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [toggleSidebar]);

  const state = open ? "expanded" : "collapsed";

  const contextValue = React.useMemo<SidebarContextProps>(
    () => ({
      state,
      open,
      setOpen,
      isMobile,
      openMobile,
      setOpenMobile,
      toggleSidebar,
    }),
    [state, open, setOpen, isMobile, openMobile, setOpenMobile, toggleSidebar]
  );

  return (
    <SidebarContext.Provider value={contextValue}>
      <div
        data-slot="sidebar-wrapper"
        style={
          {
            "--sidebar-width": SIDEBAR_WIDTH,
            "--sidebar-width-icon": SIDEBAR_WIDTH_ICON,
            ...style,
          } as React.CSSProperties
        }
        className={cn("cn-sidebar-wrapper flex min-h-svh w-full has-data-[variant=inset]:bg-sidebar", className)}
        {...props}
      >
        {children}
      </div>
    </SidebarContext.Provider>
  );
}

function Sidebar({
  side = "left",
  variant = "sidebar",
  collapsible = "offcanvas",
  className,
  children,
  dir,
  ...props
}: React.ComponentProps<"div"> & {
  side?: "left" | "right";
  variant?: "sidebar" | "floating" | "inset";
  collapsible?: "offcanvas" | "icon" | "none";
}) {
  const { isMobile, state, openMobile, setOpenMobile } = useSidebar();

  if (collapsible === "none") {
    return (
      <div
        data-slot="sidebar"
        className={cn("cn-sidebar-static flex h-full w-(--sidebar-width) flex-col bg-sidebar text-sidebar-foreground", className)}
        {...props}
      >
        {children}
      </div>
    );
  }

  if (isMobile) {
    return (
      <Sheet
        open={openMobile}
        onOpenChange={(details) => setOpenMobile(details.open)}
      >
        <SheetContent
          dir={dir}
          data-sidebar="sidebar"
          data-slot="sidebar"
          data-mobile="true"
          className={cn("cn-sidebar-mobile w-(--sidebar-width) bg-sidebar p-0 text-sidebar-foreground data-[side=left]:w-(--sidebar-width) data-[side=right]:w-(--sidebar-width) [&>button]:hidden", className)}
          style={
            {
              "--sidebar-width": SIDEBAR_WIDTH_MOBILE,
            } as React.CSSProperties
          }
          side={side}
          {...props}
        >
          <SheetHeader className="cn-sidebar-mobile-header sr-only">
            <SheetTitle>Sidebar</SheetTitle>
            <SheetDescription>Displays the mobile sidebar.</SheetDescription>
          </SheetHeader>
          <div className="cn-sidebar-mobile-inner flex h-full w-full flex-col bg-sidebar">{children}</div>
        </SheetContent>
      </Sheet>
    );
  }

  return (
    <div
      className="cn-sidebar-root hidden text-sidebar-foreground md:block"
      data-state={state}
      data-collapsible={state === "collapsed" ? collapsible : ""}
      data-variant={variant}
      data-side={side}
      data-slot="sidebar"
    >
      <div data-slot="sidebar-gap" className="cn-sidebar-gap relative w-(--sidebar-width) bg-transparent transition-[width] duration-200 ease-linear [.cn-sidebar-root[data-collapsible=offcanvas]_&]:w-0 [.cn-sidebar-root[data-side=right]_&]:rotate-180 [.cn-sidebar-root[data-collapsible=icon]_&]:w-(--sidebar-width-icon) [.cn-sidebar-root:is([data-variant=floating],_[data-variant=inset])[data-collapsible=icon]_&]:w-[calc(var(--sidebar-width-icon)+(--spacing(4)))]" />
      <div
        data-slot="sidebar-container"
        data-side={side}
        className={cn("cn-sidebar-container fixed inset-y-0 z-10 hidden h-svh w-(--sidebar-width) transition-[left,right,width] duration-200 ease-linear md:flex [&[data-side=left]]:left-0 [&[data-side=right]]:right-0 [.cn-sidebar-root[data-collapsible=offcanvas]_&[data-side=left]]:left-[calc(var(--sidebar-width)*-1)] [.cn-sidebar-root[data-collapsible=offcanvas]_&[data-side=right]]:right-[calc(var(--sidebar-width)*-1)] [.cn-sidebar-root:is([data-variant=floating],_[data-variant=inset])_&]:p-2 [.cn-sidebar-root:is([data-variant=floating],_[data-variant=inset])[data-collapsible=icon]_&]:w-[calc(var(--sidebar-width-icon)+(--spacing(4))+2px)] [.cn-sidebar-root[data-variant=sidebar][data-collapsible=icon]_&]:w-(--sidebar-width-icon) [.cn-sidebar-root[data-variant=sidebar]_&[data-side=left]]:border-r [.cn-sidebar-root[data-variant=sidebar]_&[data-side=right]]:border-l", className)}
        {...props}
      >
        <div
          data-sidebar="sidebar"
          data-slot="sidebar-inner"
          className="cn-sidebar-inner flex size-full flex-col bg-sidebar [.cn-sidebar-root[data-variant=floating]_&]:rounded-2xl [.cn-sidebar-root[data-variant=floating]_&]:shadow-[0_1px_2px_0_oklch(0_0_0/4%)] [.cn-sidebar-root[data-variant=floating]_&]:ring-1 [.cn-sidebar-root[data-variant=floating]_&]:ring-border"
        >
          {children}
        </div>
      </div>
    </div>
  );
}

function SidebarTrigger({
  className,
  onClick,
  ...props
}: React.ComponentProps<typeof Button>) {
  const { toggleSidebar } = useSidebar();

  return (
    <Button
      data-sidebar="trigger"
      data-slot="sidebar-trigger"
      variant="ghost"
      size="icon"
      className={cn("cn-sidebar-trigger", className)}
      onClick={(event) => {
        onClick?.(event);
        toggleSidebar();
      }}
      {...props}
    >
      <PanelLeftIcon className="cn-sidebar-trigger-icon rtl:-scale-x-100" />
      <span className="cn-sidebar-sr-only sr-only">Toggle Sidebar</span>
    </Button>
  );
}

function SidebarRail({ className, ...props }: React.ComponentProps<"button">) {
  const { toggleSidebar } = useSidebar();

  return (
    <button
      data-sidebar="rail"
      data-slot="sidebar-rail"
      aria-label="Toggle Sidebar"
      tabIndex={-1}
      onClick={toggleSidebar}
      title="Toggle Sidebar"
      className={cn("cn-sidebar-rail absolute inset-y-0 z-20 hidden w-4 -translate-x-1/2 transition-all ease-linear after:absolute after:inset-y-0 after:start-1/2 after:w-[2px] hover:after:bg-sidebar-border sm:flex [.cn-sidebar-root[data-side=left]_&]:-right-4 [.cn-sidebar-root[data-side=left]_&]:cursor-w-resize [.cn-sidebar-root[data-side=right]_&]:left-0 [.cn-sidebar-root[data-side=right]_&]:cursor-e-resize [.cn-sidebar-root[data-side=left][data-state=collapsed]_&]:cursor-e-resize [.cn-sidebar-root[data-side=right][data-state=collapsed]_&]:cursor-w-resize [.cn-sidebar-root[data-collapsible=offcanvas]_&]:translate-x-0 [.cn-sidebar-root[data-collapsible=offcanvas]_&]:after:left-full [.cn-sidebar-root[data-collapsible=offcanvas]_&]:hover:bg-sidebar [.cn-sidebar-root[data-side=left][data-collapsible=offcanvas]_&]:-right-2 [.cn-sidebar-root[data-side=right][data-collapsible=offcanvas]_&]:-left-2", className)}
      {...props}
    />
  );
}

function SidebarInset({ className, ...props }: React.ComponentProps<"main">) {
  return (
    <main
      data-slot="sidebar-inset"
      className={cn("cn-sidebar-inset relative flex w-full flex-1 flex-col bg-background [@media(width_>=_48rem)]:[.cn-sidebar-root[data-variant=inset]_~_&]:m-2 [@media(width_>=_48rem)]:[.cn-sidebar-root[data-variant=inset]_~_&]:ml-0 [@media(width_>=_48rem)]:[.cn-sidebar-root[data-variant=inset]_~_&]:rounded-2xl [@media(width_>=_48rem)]:[.cn-sidebar-root[data-variant=inset]_~_&]:shadow-sm [@media(width_>=_48rem)]:[.cn-sidebar-root[data-variant=inset][data-state=collapsed]_~_&]:ml-2 [@media(width_>=_48rem)]:[.cn-sidebar-host:has(>_.cn-sidebar-root[data-variant=inset])_~_&]:m-2 [@media(width_>=_48rem)]:[.cn-sidebar-host:has(>_.cn-sidebar-root[data-variant=inset])_~_&]:ml-0 [@media(width_>=_48rem)]:[.cn-sidebar-host:has(>_.cn-sidebar-root[data-variant=inset])_~_&]:rounded-2xl [@media(width_>=_48rem)]:[.cn-sidebar-host:has(>_.cn-sidebar-root[data-variant=inset])_~_&]:shadow-sm [@media(width_>=_48rem)]:[.cn-sidebar-host:has(>_.cn-sidebar-root[data-variant=inset][data-state=collapsed])_~_&]:ml-2", className)}
      {...props}
    />
  );
}

function SidebarInput({
  className,
  ...props
}: React.ComponentProps<typeof Input>) {
  return (
    <Input
      data-slot="sidebar-input"
      data-sidebar="input"
      className={cn("cn-sidebar-input h-8 w-full min-w-0 shadow-none outline-none file:inline-flex file:border-0 file:bg-transparent disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50", className)}
      {...props}
    />
  );
}

function SidebarHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-header"
      data-sidebar="header"
      className={cn("cn-sidebar-header flex flex-col gap-2 border-b border-sidebar-border p-2 pb-1.75 [&.border-b-0]:pb-2", className)}
      {...props}
    />
  );
}

function SidebarFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-footer"
      data-sidebar="footer"
      className={cn("cn-sidebar-footer flex flex-col gap-2 border-t border-sidebar-border p-2 pt-1.75 [&.border-t-0]:pt-2", className)}
      {...props}
    />
  );
}

function SidebarSeparator({
  className,
  ...props
}: React.ComponentProps<typeof Separator>) {
  return (
    <Separator
      data-slot="sidebar-separator"
      data-sidebar="separator"
      className={cn("cn-sidebar-separator mx-2 w-auto bg-sidebar-border", className)}
      {...props}
    />
  );
}

function SidebarContent({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-content"
      data-sidebar="content"
      className={cn("cn-sidebar-content no-scrollbar flex min-h-0 flex-1 flex-col overflow-auto [.cn-sidebar-root[data-collapsible=icon]_&]:overflow-x-hidden", className)}
      {...props}
    />
  );
}

function SidebarGroup({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-group"
      data-sidebar="group"
      className={cn("cn-sidebar-group relative flex w-full min-w-0 flex-col p-2 [.cn-sidebar-root[data-collapsible=icon]_&]:py-0", className)}
      {...props}
    />
  );
}

function SidebarGroupLabel({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) {
  return (
    <wire.div
      data-slot="sidebar-group-label"
      data-sidebar="group-label"
      className={cn("cn-sidebar-group-label flex h-8 shrink-0 items-center rounded-lg px-2 text-sm font-medium text-muted-foreground outline-hidden transition-[margin,opacity] duration-200 ease-linear focus-visible:ring-3 focus-visible:inset-ring focus-visible:ring-ring/30 focus-visible:inset-ring-sidebar-ring data-[size=sm]:text-2xs [&>svg]:size-4 [&>svg]:shrink-0 [&>svg]:text-muted-foreground [.cn-sidebar-root[data-collapsible=icon]_&]:pointer-events-none [.cn-sidebar-root[data-collapsible=icon]_&]:-mt-8 [.cn-sidebar-root[data-collapsible=icon]_&]:opacity-0 [&:is(a,_button):hover]:bg-fill-default [&:is(a,_button):active]:bg-fill-hover", className)}
      {...props}
    />
  );
}

function SidebarGroupAction({
  className,
  ...props
}: React.ComponentProps<typeof wire.button>) {
  return (
    <wire.button
      data-slot="sidebar-group-action"
      data-sidebar="group-action"
      className={cn("cn-sidebar-group-action absolute top-3.5 right-3.5 flex aspect-square w-5 items-center justify-center rounded-md p-0 text-muted-foreground outline-hidden transition-transform after:absolute after:-inset-2 hover:bg-fill-default hover:text-sidebar-foreground focus-visible:ring-3 focus-visible:inset-ring focus-visible:ring-ring/30 focus-visible:inset-ring-sidebar-ring md:after:hidden [&>svg]:size-4 [&>svg]:shrink-0 [.cn-sidebar-root[data-collapsible=icon]_&]:hidden", className)}
      {...props}
    />
  );
}

function SidebarGroupContent({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-group-content"
      data-sidebar="group-content"
      className={cn("cn-sidebar-group-content w-full text-sm", className)}
      {...props}
    />
  );
}

function SidebarMenu({ className, ...props }: React.ComponentProps<"ul">) {
  return (
    <ul
      data-slot="sidebar-menu"
      data-sidebar="menu"
      className={cn("cn-sidebar-menu flex w-full min-w-0 flex-col gap-0", className)}
      {...props}
    />
  );
}

function SidebarMenuItem({ className, ...props }: React.ComponentProps<"li">) {
  return (
    <li
      data-slot="sidebar-menu-item"
      data-sidebar="menu-item"
      className={cn("cn-sidebar-menu-item relative", className)}
      {...props}
    />
  );
}

const sidebarMenuButtonVariants = cva("cn-sidebar-menu-button flex w-full items-center gap-2 overflow-hidden rounded-lg px-2 py-1.5 text-left text-sm whitespace-nowrap outline-hidden transition-[width,height,padding] duration-200 hover:bg-fill-default focus-visible:bg-sidebar focus-visible:ring-3 focus-visible:inset-ring focus-visible:ring-ring/30 focus-visible:inset-ring-sidebar-ring active:bg-fill-hover disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 data-active:bg-fill-hover data-active:font-medium data-active:hover:bg-fill-active [&_svg]:size-4 [&_svg]:shrink-0 [&>span:last-child]:truncate [&>svg]:text-muted-foreground hover:[&>svg]:text-sidebar-foreground data-active:[&>svg]:text-sidebar-foreground [.cn-sidebar-menu-item:has([data-sidebar=menu-action])_&]:pr-8 [.cn-sidebar-root[data-collapsible=icon]_&]:size-8! [.cn-sidebar-root[data-collapsible=icon]_&]:p-2!", {
  variants: {
    variant: {
      default: "cn-sidebar-menu-button-variant-default hover:bg-fill-default",
      outline: "cn-sidebar-menu-button-variant-outline bg-background shadow-[0_0_0_1px_var(--sidebar-border)]",
    },
    size: {
      default: "cn-sidebar-menu-button-size-default h-8",
      sm: "cn-sidebar-menu-button-size-sm h-7 text-2xs",
      lg: "cn-sidebar-menu-button-size-lg h-12 gap-2.5 group-data-[collapsible=icon]:p-0!",
    },
  },
  defaultVariants: {
    variant: "default",
    size: "default",
  },
});

function SidebarMenuButton({
  asChild = false,
  isActive = false,
  variant = "default",
  size = "default",
  tooltip,
  className,
  ...props
}: React.ComponentProps<"button"> & {
  asChild?: boolean;
  isActive?: boolean;
  tooltip?: string | React.ComponentProps<typeof TooltipContent>;
} & VariantProps<typeof sidebarMenuButtonVariants>) {
  const { isMobile, state } = useSidebar();

  const button = (
    <wire.button
      asChild={asChild}
      data-slot="sidebar-menu-button"
      data-sidebar="menu-button"
      data-size={size}
      data-active={isActive}
      className={cn(sidebarMenuButtonVariants({ variant, size }), className)}
      {...props}
    />
  );

  if (!tooltip) {
    return button;
  }

  if (typeof tooltip === "string") {
    tooltip = {
      children: tooltip,
    };
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent hidden={state !== "collapsed" || isMobile} {...tooltip} />
    </Tooltip>
  );
}

function SidebarMenuAction({
  className,
  showOnHover = false,
  ...props
}: React.ComponentProps<"button"> & {
  showOnHover?: boolean;
}) {
  return (
    <wire.button
      data-slot="sidebar-menu-action"
      data-sidebar="menu-action"
      className={cn(
        "cn-sidebar-menu-action absolute top-1.5 right-1.5 flex aspect-square w-5 items-center justify-center rounded-md p-0 text-muted-foreground outline-hidden transition-transform after:absolute after:-inset-2 hover:bg-fill-default hover:text-sidebar-foreground focus-visible:ring-3 focus-visible:inset-ring focus-visible:ring-ring/30 focus-visible:inset-ring-sidebar-ring md:after:hidden [&>svg]:size-4 [&>svg]:shrink-0 [.cn-sidebar-menu-button:hover_~_&]:text-sidebar-foreground [.cn-sidebar-menu-button[data-active=true]_~_&]:text-sidebar-foreground [.cn-sidebar-menu-button[data-size=lg]_~_&]:top-2.5 [.cn-sidebar-menu-button[data-size=sm]_~_&]:top-1 [.cn-sidebar-root[data-collapsible=icon]_&]:hidden",
        showOnHover && "cn-sidebar-menu-action-show-on-hover [@media(width_>=_48rem)]:opacity-0 [@media(width_>=_48rem)]:[.cn-sidebar-menu-item:focus-within_&]:opacity-100 [@media(width_>=_48rem)]:[.cn-sidebar-menu-item:hover_&]:opacity-100 [@media(width_>=_48rem)]:[&[aria-expanded=true]]:opacity-100 [@media(width_>=_48rem)]:[.cn-sidebar-menu-button[data-active=true]_~_&]:opacity-100",
        className
      )}
      {...props}
    />
  );
}

function SidebarMenuBadge({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-menu-badge"
      data-sidebar="menu-badge"
      className={cn("cn-sidebar-menu-badge [.cn-sidebar-menu-button[data-size=lg]_~_&]:top-2.5 [.cn-sidebar-menu-button[data-size=sm]_~_&]:top-1 pointer-events-none absolute top-1.5 right-2 flex h-5 min-w-5 items-center justify-center rounded-md px-1 text-2xs font-medium text-muted-foreground tabular-nums select-none [.cn-sidebar-menu-button:hover_~_&]:text-sidebar-foreground [.cn-sidebar-menu-button[data-active=true]_~_&]:text-sidebar-foreground [.cn-sidebar-root[data-collapsible=icon]_&]:hidden", className)}
      {...props}
    />
  );
}

function SidebarMenuSkeleton({
  className,
  showIcon = false,
  ...props
}: React.ComponentProps<"div"> & {
  showIcon?: boolean;
}) {
  const [width] = React.useState(() => {
    return `${Math.floor(Math.random() * 40) + 50}%`;
  });

  return (
    <div
      data-slot="sidebar-menu-skeleton"
      data-sidebar="menu-skeleton"
      className={cn("cn-sidebar-menu-skeleton flex h-8 items-center gap-2 rounded-lg px-2", className)}
      {...props}
    >
      {showIcon && (
        <Skeleton
          className="cn-sidebar-menu-skeleton-icon size-4 animate-pulse rounded-md bg-accent"
          data-sidebar="menu-skeleton-icon"
        />
      )}
      <Skeleton
        className="cn-sidebar-menu-skeleton-text h-4 max-w-(--skeleton-width) flex-1 animate-pulse rounded-md bg-accent"
        data-sidebar="menu-skeleton-text"
        style={
          {
            "--skeleton-width": width,
          } as React.CSSProperties
        }
      />
    </div>
  );
}

function SidebarMenuSub({ className, ...props }: React.ComponentProps<"ul">) {
  return (
    <ul
      data-slot="sidebar-menu-sub"
      data-sidebar="menu-sub"
      className={cn("cn-sidebar-menu-sub mt-0.5 ml-3.75 flex min-w-0 flex-col gap-1 border-l border-sidebar-border py-0.5 pl-2 [.cn-sidebar-root[data-collapsible=icon]_&]:hidden", className)}
      {...props}
    />
  );
}

function SidebarMenuSubItem({
  className,
  ...props
}: React.ComponentProps<"li">) {
  return (
    <li
      data-slot="sidebar-menu-sub-item"
      data-sidebar="menu-sub-item"
      className={cn("cn-sidebar-menu-sub-item relative", className)}
      {...props}
    />
  );
}

function SidebarMenuSubButton({
  size = "md",
  isActive = false,
  className,
  ...props
}: React.ComponentProps<typeof wire.a> & {
  size?: "sm" | "md";
  isActive?: boolean;
}) {
  return (
    <wire.a
      data-slot="sidebar-menu-sub-button"
      data-sidebar="menu-sub-button"
      data-size={size}
      data-active={isActive}
      className={cn("cn-sidebar-menu-sub-button flex h-7 min-w-0 items-center gap-2 overflow-hidden rounded-lg px-2 text-sidebar-foreground outline-hidden hover:bg-fill-default focus-visible:ring-3 focus-visible:inset-ring focus-visible:ring-ring/30 focus-visible:inset-ring-sidebar-ring active:bg-fill-hover disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 data-[size=md]:text-sm data-[size=sm]:text-2xs data-active:bg-fill-hover data-active:font-medium data-active:hover:bg-fill-active [&>span:last-child]:truncate [&>svg]:size-4 [&>svg]:shrink-0 [&>svg]:text-muted-foreground [.cn-sidebar-root[data-collapsible=icon]_&]:hidden", className)}
      {...props}
    />
  );
}

export {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInput,
  SidebarInset,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarProvider,
  SidebarRail,
  SidebarSeparator,
  SidebarTrigger,
  useSidebar,
};
