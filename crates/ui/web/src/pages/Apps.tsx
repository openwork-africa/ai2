import { ExternalLink, FileJson, LayoutGrid, MoreHorizontal, Pencil, Play, Plus, ScrollText, Search, Square, Trash2, TriangleAlert, X } from "lucide-react";
import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router";
import { ApiBadge, AppIcon, RemoveDialog, StandingBadge } from "@/components/app-parts";
import { Code, Page, PageHeader } from "@/components/page";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { useApps } from "@/hooks/use-apps";
import { type App, openApp, openDocument, removeApp, stopApp } from "@/lib/apps";

function Tile({ app, onChange, onRemoved, onError }: { app: App; onChange: (app: App) => void; onRemoved: (id: string) => void; onError: (message: string) => void }) {
  const navigate = useNavigate();
  const [busy, setBusy] = useState<"play" | "stop" | null>(null);
  const [removing, setRemoving] = useState(false);
  const detail = `/apps/${encodeURIComponent(app.id)}`;

  const play = async () => {
    setBusy("play");
    try {
      await openApp(app, onChange);
    } catch (failure) {
      onError(`${app.name}: ${(failure as Error).message}`);
    } finally {
      setBusy(null);
    }
  };

  const stop = async () => {
    setBusy("stop");
    try {
      onChange((await stopApp(app.id)).app);
    } catch (failure) {
      onError(`${app.name}: ${(failure as Error).message}`);
    } finally {
      setBusy(null);
    }
  };

  const choose = (value: string) => {
    if (value === "logs") navigate(detail);
    if (value === "api") navigate(`${detail}?tab=api`);
    if (value === "edit") navigate(`${detail}?edit=1`);
    if (value === "document") openDocument(app.id).catch((failure: Error) => onError(failure.message));
    if (value === "remove") setRemoving(true);
  };

  return (
    <Card size="sm" className="group/tile gap-4 transition-shadow hover:shadow-(--shadow-dialog)" data-app={app.id}>
      <div className="flex items-start gap-3 px-(--card-spacing)">
        <AppIcon app={app} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <Link to={detail} className="truncate font-heading text-sm font-semibold tracking-tight hover:underline hover:underline-offset-4">
            {app.name}
          </Link>
          <p className="line-clamp-2 min-h-10 text-xs leading-5 text-muted-foreground">{app.description}</p>
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-1.5 px-(--card-spacing)">
        <StandingBadge app={app} />
        <Badge variant="secondary" className="font-mono font-normal">:{app.port}</Badge>
        <ApiBadge api={app.api} />
      </div>
      <div className="flex items-center gap-2 border-t px-(--card-spacing) pt-3">
        <Button size="sm" variant={app.running ? "outline" : "default"} onClick={play} disabled={busy !== null}>
          {busy === "play" ? <Spinner /> : app.running ? <ExternalLink /> : <Play />}
          {app.running ? "Open" : busy === "play" ? "Starting" : "Start"}
        </Button>
        {app.started_here && (
          <Button size="sm" variant="ghost" onClick={stop} disabled={busy !== null}>
            {busy === "stop" ? <Spinner /> : <Square />}
            Stop
          </Button>
        )}
        <span className="ml-auto" />
        <DropdownMenu onSelect={({ value }) => choose(value)}>
          <DropdownMenuTrigger asChild>
            <Button size="icon-sm" variant="ghost" aria-label={`More for ${app.name}`}>
              <MoreHorizontal />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent className="min-w-44">
            <DropdownMenuItem value="logs">
              <ScrollText />
              Logs
            </DropdownMenuItem>
            <DropdownMenuItem value="api">
              <FileJson />
              API
            </DropdownMenuItem>
            <DropdownMenuItem value="edit">
              <Pencil />
              Edit
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem value="remove" variant="destructive">
              <Trash2 />
              Remove
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <RemoveDialog
        app={app}
        open={removing}
        onOpenChange={setRemoving}
        onRemove={() =>
          removeApp(app.id)
            .then(() => onRemoved(app.id))
            .catch((failure: Error) => onError(failure.message))
        }
      />
    </Card>
  );
}

function Loading() {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {[0, 1, 2].map((key) => (
        <Card key={key} size="sm" className="gap-4">
          <div className="flex items-start gap-3 px-(--card-spacing)">
            <Skeleton className="size-11 rounded-xl" />
            <div className="flex flex-1 flex-col gap-2">
              <Skeleton className="h-4 w-1/2" />
              <Skeleton className="h-3 w-5/6" />
            </div>
          </div>
          <Skeleton className="mx-(--card-spacing) h-5 w-2/3" />
          <Skeleton className="mx-(--card-spacing) h-7 w-20" />
        </Card>
      ))}
    </div>
  );
}

export default function Apps() {
  const { apps, error, replace, drop, setError } = useApps();
  const [filter, setFilter] = useState("");
  const shown = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!apps || !needle) return apps;
    return apps.filter((app) => [app.name, app.description, app.command, String(app.port)].some((text) => text.toLowerCase().includes(needle)));
  }, [apps, filter]);
  const running = apps?.filter((app) => app.running).length ?? 0;

  return (
    <Page className="max-w-6xl">
      <PageHeader
        title="Apps"
        description={
          apps && apps.length > 0
            ? `Every app on this machine is one press from running, and ${running} of ${apps.length} are up right now.`
            : "Every app on this machine is one press from running."
        }
        actions={
          <Button asChild>
            <Link to="/apps/new">
              <Plus data-icon="inline-start" />
              Add app
            </Link>
          </Button>
        }
      >
        {apps && apps.length > 3 && (
          <div className="relative w-full max-w-xs">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="Filter apps" className="w-full pl-8" aria-label="Filter apps" />
          </div>
        )}
      </PageHeader>

      {error && (
        <Alert variant="destructive">
          <TriangleAlert />
          <AlertTitle>That didn't go through</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
          <Button size="icon-xs" variant="ghost" className="absolute top-3 right-3" aria-label="Dismiss" onClick={() => setError(null)}>
            <X />
          </Button>
        </Alert>
      )}

      {!apps && !error && <Loading />}

      {apps && apps.length === 0 && (
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <LayoutGrid />
            </EmptyMedia>
            <EmptyTitle>The shelf is empty</EmptyTitle>
            <EmptyDescription>Your apps show up here, whether you add them or your agent does.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Code>ai2 apps register --openapi /openapi.json …</Code>
          </EmptyContent>
        </Empty>
      )}

      {shown && shown.length > 0 && (
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {shown.map((app) => (
            <Tile key={app.id} app={app} onChange={replace} onRemoved={drop} onError={setError} />
          ))}
          {!filter.trim() && (
            <Link
              to="/apps/new"
              className="flex min-h-48 flex-col items-center justify-center gap-2 rounded-3xl border border-dashed text-sm text-muted-foreground transition-colors hover:border-foreground/30 hover:bg-fill-subtle hover:text-foreground"
            >
              <span className="flex size-9 items-center justify-center rounded-full bg-fill-default">
                <Plus className="size-4" />
              </span>
              Add an app
              <span className="text-xs">or let an agent do it: ai2 apps register</span>
            </Link>
          )}
        </div>
      )}

      {apps && apps.length > 0 && shown?.length === 0 && (
        <p className="text-sm text-muted-foreground">Nothing here answers to “{filter}”.</p>
      )}
    </Page>
  );
}
