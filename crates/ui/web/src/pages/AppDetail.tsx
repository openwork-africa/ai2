import { cn } from "cn";
import { Copy, ExternalLink, FileJson, Pencil, Play, RefreshCw, RotateCw, Square, Trash2, TriangleAlert } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router";
import { AppForm } from "@/components/app-form";
import { ApiBadge, AppIcon, RemoveDialog, StandingBadge } from "@/components/app-parts";
import { Code, Page, PageHeader } from "@/components/page";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { Switch, SwitchControl, SwitchHiddenInput, SwitchThumb } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useApp } from "@/hooks/use-apps";
import type { ApiError } from "@/lib/api";
import {
  type App,
  checkApp,
  getDocument,
  getLogs,
  home,
  type LogLine,
  logEvents,
  type OpenApiDocument,
  openApp,
  openDocument,
  operations,
  removeApp,
  restartApp,
  stopApp,
  updateApp,
  uptime,
} from "@/lib/apps";

const LIMIT = 2000;

const clock = (t: number | null) =>
  t === null ? "" : new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });

const METHOD_TONE: Record<string, string> = {
  GET: "bg-sky-500/10 text-sky-700 dark:text-sky-300",
  POST: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
  PUT: "bg-amber-500/10 text-amber-700 dark:text-amber-300",
  PATCH: "bg-violet-500/10 text-violet-700 dark:text-violet-300",
  DELETE: "bg-rose-500/10 text-rose-700 dark:text-rose-300",
};

function Logs({ app }: { app: App }) {
  const [lines, setLines] = useState<LogLine[]>([]);
  const [follow, setFollow] = useState(true);
  const [live, setLive] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const controller = new AbortController();
    setLines([]);
    const run = async () => {
      while (!controller.signal.aborted) {
        setLive(true);
        await logEvents(app.id, (line) => setLines((all) => [...all.slice(-(LIMIT - 1)), line]), controller.signal).catch(() => undefined);
        setLive(false);
        if (controller.signal.aborted) return;
        await new Promise((resolve) => window.setTimeout(resolve, 2000));
        const { lines } = await getLogs(app.id).catch(() => ({ lines: [] as LogLine[] }));
        setLines(lines);
      }
    };
    run();
    return () => controller.abort();
  }, [app.id]);

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines, follow]);

  return (
    <div className="flex flex-col gap-3">
      <div
        ref={box}
        data-testid="logs"
        onScroll={(event) => {
          const el = event.currentTarget;
          const bottom = el.scrollHeight - el.scrollTop - el.clientHeight < 8;
          if (!bottom && follow) setFollow(false);
        }}
        className="h-[22rem] overflow-auto rounded-2xl bg-neutral-950 px-4 py-3 font-mono text-[0.75rem] leading-5 text-neutral-200 shadow-(--shadow-card) ring-1 ring-black/10 [font-variant-ligatures:none] dark:bg-black/60 dark:ring-white/10"
      >
        {lines.length === 0 ? (
          <p className="text-neutral-500">{app.running ? "Listening. The app hasn't said anything yet." : "Nothing yet. Logs are whatever the app prints while it runs; press Start and they land here."}</p>
        ) : (
          lines.map((line, at) => (
            <div key={at} className="flex gap-3 whitespace-pre-wrap break-all">
              <span className="w-16 shrink-0 text-neutral-500 select-none">{clock(line.t)}</span>
              <span className={cn("min-w-0", line.stream === "err" && "text-rose-300", line.line.startsWith("--- ") && "text-neutral-500")}>{line.line}</span>
            </div>
          ))
        )}
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2 text-sm">
          <Switch checked={follow} onCheckedChange={({ checked }) => setFollow(checked)}>
            <SwitchControl>
              <SwitchThumb />
            </SwitchControl>
            <SwitchHiddenInput />
          </Switch>
          Follow
        </label>
        <Button size="sm" variant="ghost" onClick={() => navigator.clipboard?.writeText(lines.map((line) => line.line).join("\n"))}>
          <Copy />
          Copy
        </Button>
        <Button size="sm" variant="ghost" onClick={() => setLines([])}>
          Clear
        </Button>
        <span className="ml-auto flex items-center gap-1.5 text-xs text-muted-foreground">
          <span className={cn("size-1.5 rounded-full", live ? "animate-pulse bg-success" : "bg-muted-foreground/50")} />
          {live ? "Live" : "Reconnecting"}
        </span>
      </div>
    </div>
  );
}

function ApiTab({ app, onChecked }: { app: App; onChecked: () => void }) {
  const [document, setDocument] = useState<OpenApiDocument | null>(null);
  const [missing, setMissing] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    let alive = true;
    getDocument(app.id)
      .then((doc) => alive && (setDocument(doc), setMissing(null)))
      .catch((failure: Error) => alive && (setDocument(null), setMissing(failure.message)));
    return () => {
      alive = false;
    };
  }, [app.id, app.api.checked]);

  const ops = useMemo(() => (document ? operations(document) : []), [document]);

  const check = async () => {
    setChecking(true);
    await checkApp(app.id).catch(() => undefined);
    setChecking(false);
    onChecked();
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <ApiBadge api={app.api} />
        {document?.info?.title && (
          <span className="text-sm">
            {document.info.title}
            {document.info.version && <span className="text-muted-foreground"> · v{document.info.version}</span>}
          </span>
        )}
        <span className="ml-auto flex items-center gap-2">
          <Button size="sm" variant="ghost" onClick={check} disabled={checking}>
            {checking ? <Spinner /> : <RefreshCw />}
            Check again
          </Button>
          <Button size="sm" variant="outline" onClick={() => openDocument(app.id)} disabled={!document}>
            <FileJson />
            Raw JSON
          </Button>
        </span>
      </div>
      {app.api.error && (
        <Alert>
          <TriangleAlert />
          <AlertTitle>The last check {app.api.state === "invalid" ? "found a problem" : "couldn't reach the app"}</AlertTitle>
          <AlertDescription>{app.api.error}</AlertDescription>
        </Alert>
      )}
      {document ? (
        <Card size="sm" className="py-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-24 pl-5">Method</TableHead>
                <TableHead>Path</TableHead>
                <TableHead className="pr-5">Summary</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {ops.map((op) => (
                <TableRow key={`${op.method} ${op.path}`}>
                  <TableCell className="pl-5">
                    <span className={cn("rounded-md px-1.5 py-0.5 font-mono text-2xs font-semibold", METHOD_TONE[op.method] ?? "bg-fill-default")}>{op.method}</span>
                  </TableCell>
                  <TableCell className="font-mono text-xs">{op.path}</TableCell>
                  <TableCell className="pr-5 text-muted-foreground">{op.summary ?? op.operationId ?? ""}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      ) : (
        <Empty className="rounded-3xl border border-dashed py-12">
          <EmptyHeader>
            <EmptyTitle>No OpenAPI document yet</EmptyTitle>
            <EmptyDescription>{missing ?? "The OpenAPI document is what tells your agents what this app can do. Once the app is running, ai squared fetches it from the path in Settings."}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Code>{app.openapi}</Code>
          </EmptyContent>
        </Empty>
      )}
    </div>
  );
}

function Setting({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid gap-1 py-3 sm:grid-cols-[10rem_1fr] sm:gap-4">
      <dt className="text-sm text-muted-foreground">{label}</dt>
      <dd className="min-w-0 text-sm break-words">{children}</dd>
    </div>
  );
}

const RESTART_LABEL = { never: "Never; it outlives ai squared", "on-failure": "When it fails", always: "Always" };

function Settings({ app, onEdit, onRemove }: { app: App; onEdit: () => void; onRemove: () => void }) {
  const env = Object.entries(app.env);
  return (
    <div className="flex flex-col gap-4">
      <Card size="sm" className="py-1">
        <dl className="divide-y px-(--card-spacing)">
          <Setting label="Run command"><Code>{app.command}</Code></Setting>
          <Setting label="Folder"><span className="font-mono text-xs">{app.cwd}</span></Setting>
          <Setting label="Address">{app.url} {app.set_port ? <span className="text-muted-foreground">· ai squared sets PORT={app.port}</span> : null}</Setting>
          <Setting label="OpenAPI"><span className="font-mono text-xs">{app.openapi}</span></Setting>
          <Setting label="Restart">{RESTART_LABEL[app.restart]}</Setting>
          <Setting label="Environment">
            {env.length === 0 ? (
              <span className="text-muted-foreground">Nothing extra</span>
            ) : (
              <span className="flex flex-col gap-1 font-mono text-xs">
                {env.map(([key, value]) => (
                  <span key={key}>
                    {key}=<span className="text-muted-foreground">{value}</span>
                  </span>
                ))}
              </span>
            )}
          </Setting>
          <Setting label="Added">{new Date(app.added).toLocaleString()}</Setting>
        </dl>
      </Card>
      <div className="flex items-center gap-2">
        <Button variant="outline" size="sm" onClick={onEdit}>
          <Pencil />
          Edit
        </Button>
        <Button variant="destructive" size="sm" onClick={onRemove}>
          <Trash2 />
          Remove
        </Button>
      </div>
    </div>
  );
}

export default function AppDetail() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const { app, error, refresh, setApp } = useApp(id);
  const [busy, setBusy] = useState<string | null>(null);
  const [failure, setFailure] = useState<{ message: string; lines?: LogLine[] } | null>(null);
  const [removing, setRemoving] = useState(false);
  const [, tick] = useState(0);
  const tab = params.get("tab") ?? "logs";
  const editing = params.get("edit") === "1";

  useEffect(() => {
    const timer = window.setInterval(() => tick((n) => n + 1), 30000);
    return () => window.clearInterval(timer);
  }, []);

  const setParam = (key: string, value: string | null) =>
    setParams(
      (all) => {
        const next = new URLSearchParams(all);
        if (value === null) next.delete(key);
        else next.set(key, value);
        return next;
      },
      { replace: true },
    );

  if (error && error.status === 404) {
    return (
      <Page>
        <PageHeader back={{ to: "/", label: "Apps" }} title="No app by that name" description={error.message} />
      </Page>
    );
  }

  if (!app) {
    return (
      <Page>
        <PageHeader back={{ to: "/", label: "Apps" }} title={<Skeleton className="h-7 w-48" />} />
        <Skeleton className="h-96 rounded-2xl" />
      </Page>
    );
  }

  const act = async (name: string, run: () => Promise<unknown>) => {
    setBusy(name);
    setFailure(null);
    try {
      await run();
    } catch (caught) {
      const problem = caught as ApiError;
      const lines = problem.code === "exited" ? (await getLogs(app.id, 20).catch(() => ({ lines: [] }))).lines : undefined;
      setFailure({ message: problem.message, lines });
    } finally {
      setBusy(null);
      refresh();
    }
  };

  const up = uptime(app.since);

  return (
    <Page>
      <PageHeader
        back={{ to: "/", label: "Apps" }}
        title={
          <span className="flex items-center gap-3">
            <AppIcon app={app} size="lg" />
            <span className="flex min-w-0 flex-col gap-1">
              <span className="truncate">{app.name}</span>
              <span className="text-sm font-normal tracking-normal text-muted-foreground">{app.description}</span>
            </span>
          </span>
        }
        actions={
          <>
            <Button onClick={() => act("play", () => openApp(app, setApp))} disabled={busy !== null}>
              {busy === "play" ? <Spinner /> : app.running ? <ExternalLink /> : <Play />}
              {app.running ? "Open" : "Start"}
            </Button>
            {app.started_here && (
              <Button variant="outline" onClick={() => act("restart", () => restartApp(app.id).then(({ app }) => setApp(app)))} disabled={busy !== null}>
                {busy === "restart" ? <Spinner /> : <RotateCw />}
                Restart
              </Button>
            )}
            {app.started_here && (
              <Button variant="outline" onClick={() => act("stop", () => stopApp(app.id).then(({ app }) => setApp(app)))} disabled={busy !== null}>
                {busy === "stop" ? <Spinner /> : <Square />}
                Stop
              </Button>
            )}
          </>
        }
      >
        <div className="flex flex-col gap-3 pt-2">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5 text-sm text-muted-foreground">
            <StandingBadge app={app} />
            <Badge variant="secondary" className="font-mono font-normal">:{app.port}</Badge>
            {app.pid !== null && <span className="font-mono text-xs">pid {app.pid}</span>}
            {app.running && up && <span className="text-xs">· up {up}</span>}
            {app.last_exit && !app.running && (
              <span className="text-xs">· last exit {app.last_exit.code !== null ? `code ${app.last_exit.code}` : `signal ${app.last_exit.signal}`}</span>
            )}
          </div>
          <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-muted-foreground">
            <Code className="break-all">{app.command}</Code>
            <span>in</span>
            <span className="font-mono text-xs">{home(app.cwd)}</span>
          </p>
          <div className="flex flex-wrap items-center gap-2 text-sm">
            <ApiBadge api={app.api} />
            <span className="font-mono text-xs text-muted-foreground">{app.openapi}</span>
            {app.api.state === "valid" && (
              <Button size="xs" variant="ghost" onClick={() => openDocument(app.id)}>
                <FileJson />
                View
              </Button>
            )}
          </div>
        </div>
      </PageHeader>

      {failure && (
        <Alert variant="destructive">
          <TriangleAlert />
          <AlertTitle>{failure.message}</AlertTitle>
          {failure.lines && failure.lines.length > 0 && (
            <AlertDescription>
              <pre className="mt-2 max-h-48 overflow-auto rounded-lg bg-fill-subtle p-2 font-mono text-2xs whitespace-pre-wrap">{failure.lines.map((line) => line.line).join("\n")}</pre>
            </AlertDescription>
          )}
        </Alert>
      )}

      <Tabs value={tab} onValueChange={({ value }) => setParam("tab", value === "logs" ? null : value)}>
        <TabsList>
          <TabsTrigger value="logs">Logs</TabsTrigger>
          <TabsTrigger value="api">API{app.api.operations ? <span className="ml-1 text-muted-foreground tabular-nums">{app.api.operations}</span> : null}</TabsTrigger>
          <TabsTrigger value="settings">Settings</TabsTrigger>
        </TabsList>
        <TabsContent value="logs" className="pt-4">
          <Logs app={app} />
        </TabsContent>
        <TabsContent value="api" className="pt-4">
          <ApiTab app={app} onChecked={refresh} />
        </TabsContent>
        <TabsContent value="settings" className="pt-4">
          <Settings app={app} onEdit={() => setParam("edit", "1")} onRemove={() => setRemoving(true)} />
        </TabsContent>
      </Tabs>

      <p className="text-xs text-muted-foreground">
        {app.started_here && app.since
          ? `Started by ai squared at ${new Date(app.since).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })} · ${app.restarts} ${app.restarts === 1 ? "restart" : "restarts"}`
          : app.running
            ? "Running, but something else started it, so ai squared leaves it be."
            : `Added ${new Date(app.added).toLocaleDateString()}`}
      </p>

      <Sheet open={editing} onOpenChange={({ open }) => !open && setParam("edit", null)}>
        <SheetContent className="w-full overflow-y-auto sm:max-w-xl">
          <SheetHeader>
            <SheetTitle>Edit {app.name}</SheetTitle>
            <SheetDescription>Changes take effect the next time the app starts.</SheetDescription>
          </SheetHeader>
          <div className="px-4 pb-6">
            {editing && (
              <AppForm
                app={app}
                submitLabel="Save changes"
                onCancel={() => setParam("edit", null)}
                onSave={async (fields, icon) => {
                  const { app: saved } = await updateApp(app.id, fields, icon);
                  setApp(saved);
                  setParam("edit", null);
                }}
              />
            )}
          </div>
        </SheetContent>
      </Sheet>

      <RemoveDialog
        app={app}
        open={removing}
        onOpenChange={setRemoving}
        onRemove={() => act("remove", () => removeApp(app.id).then(() => navigate("/")))}
      />
    </Page>
  );
}
