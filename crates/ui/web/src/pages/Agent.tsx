import { cn } from "cn";
import { ArrowUp, BotMessageSquare, ExternalLink, Folder, MoreHorizontal, ScrollText, Trash2 } from "lucide-react";
import { type KeyboardEvent, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Page, PageHeader } from "@/components/page";
import { RunRows } from "@/components/run-parts";
import { HarnessLogo, RunFooter, RunLog } from "@/components/run-view";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import { useHarnesses, useRuns } from "@/hooks/use-agents";
import { useTranscript } from "@/hooks/use-run";
import { type Agent as AgentData, getAgent, personalityLine, removeAgent, type Run, startRun } from "@/lib/agents";
import { ApiError } from "@/lib/api";
import type { Harness } from "@/lib/harnesses";
import { shortFolder } from "@/lib/reviews";

function Exchange({ run, agent, harness, onDone }: { run: Run; agent: AgentData; harness?: Harness; onDone: () => void }) {
  const { transcript } = useTranscript(run.id, run);
  const ended = transcript.run?.state === "ended";
  useEffect(() => {
    if (ended) onDone();
  }, [ended, onDone]);
  return (
    <section className="flex flex-col gap-4" aria-label="This run">
      <div className="ml-auto max-w-[85%] rounded-2xl rounded-br-md bg-primary px-4 py-2.5 text-sm whitespace-pre-wrap text-primary-foreground shadow-(--shadow-primary-control)">
        {run.prompt}
      </div>
      <div className="flex gap-3">
        <HarnessLogo id={agent.harness} harness={harness} size="sm" className="mt-0.5" />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <RunLog transcript={transcript} />
          <div className="flex items-center gap-3">
            <div className="min-w-0 flex-1">
              <RunFooter transcript={transcript} />
            </div>
            <Button size="xs" variant="ghost" asChild>
              <Link to={`/runs/${run.id}`}>
                <ExternalLink data-icon="inline-start" />
                Open
              </Link>
            </Button>
          </div>
        </div>
      </div>
    </section>
  );
}

function Composer({ agent, onStarted }: { agent: AgentData; onStarted: (run: Run) => void }) {
  const [prompt, setPrompt] = useState("");
  const [folder, setFolder] = useState(agent.folder ?? "");
  const [sending, setSending] = useState(false);
  const [problem, setProblem] = useState<{ field: "prompt" | "folder" | null; message: string } | null>(null);

  const send = async () => {
    if (!prompt.trim() || sending) return;
    setSending(true);
    setProblem(null);
    try {
      const { run } = await startRun({ agent: agent.name, prompt: prompt.trim(), folder: folder.trim() || undefined });
      setPrompt("");
      onStarted(run);
    } catch (failure) {
      const error = failure as ApiError;
      const field = error.code?.includes("folder") ? "folder" : error.code === "prompt_required" ? "prompt" : null;
      setProblem({ field, message: error.message });
    } finally {
      setSending(false);
    }
  };

  const keys = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void send();
    }
  };

  return (
    <Card className="gap-0 overflow-hidden py-0 focus-within:ring-2 focus-within:ring-ring/20">
      <Textarea
        value={prompt}
        onChange={(event) => setPrompt(event.target.value)}
        onKeyDown={keys}
        placeholder={`Ask ${agent.name} to…`}
        aria-label="Message"
        className="min-h-24 rounded-none bg-transparent px-4 pt-4 text-sm outline-none focus-visible:ring-0"
        disabled={sending}
      />
      <div className="flex items-center gap-2 border-t bg-fill-subtle px-3 py-2">
        <label className={cn("flex min-w-0 flex-1 items-center gap-1.5 text-muted-foreground", problem?.field === "folder" && "text-destructive")}>
          <Folder className="size-3.5 shrink-0" />
          <span className="sr-only">Folder</span>
          <Input
            value={folder}
            onChange={(event) => setFolder(event.target.value)}
            placeholder="/Users/you/Documents/project"
            aria-invalid={problem?.field === "folder" || undefined}
            className="h-7 min-w-0 flex-1 bg-transparent font-mono text-2xs outline-transparent"
            disabled={sending}
          />
        </label>
        <Button size="sm" onClick={() => void send()} disabled={sending || !prompt.trim()}>
          {sending ? <Spinner data-icon="inline-start" /> : <ArrowUp data-icon="inline-start" />}
          Send
        </Button>
      </div>
      {problem && <p className="border-t px-4 py-2 text-xs text-destructive">{problem.message}</p>}
    </Card>
  );
}

function RemoveAgent({ agent, open, onOpenChange }: { agent: AgentData; open: boolean; onOpenChange: (open: boolean) => void }) {
  const navigate = useNavigate();
  const [failure, setFailure] = useState<string | null>(null);
  return (
    <AlertDialog open={open} onOpenChange={({ open }) => onOpenChange(open)}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Remove {agent.name}?</AlertDialogTitle>
          <AlertDialogDescription>{failure ?? "The agent goes; its past runs and their logs stay."}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            onClick={() =>
              removeAgent(agent.name)
                .then(() => navigate("/agents"))
                .catch((error: Error) => setFailure(error.message))
            }
          >
            Remove
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

function Personality({ agent }: { agent: AgentData }) {
  const [open, setOpen] = useState(false);
  const content = agent.personality?.content;
  if (!content || content.trim() === personalityLine(agent)) return null;
  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex w-fit items-center gap-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
        aria-expanded={open}
      >
        <ScrollText className="size-3.5" />
        {open ? "Hide the full prompt" : "Show the full prompt"}
      </button>
      {open && (
        <pre className="max-h-72 overflow-auto rounded-2xl bg-fill-subtle p-4 font-sans text-xs leading-5 whitespace-pre-wrap">{content}</pre>
      )}
    </div>
  );
}

const back = { to: "/agents", label: "Agents" };

export default function Agent() {
  const { name = "" } = useParams();
  const [loaded, setLoaded] = useState<{ name: string; agent: AgentData | null; error: ApiError | null } | null>(null);
  const [live, setLive] = useState<Run | null>(null);
  const [removing, setRemoving] = useState(false);
  const { byId } = useHarnesses();
  const { runs, refresh } = useRuns(name);

  useEffect(() => {
    let alive = true;
    getAgent(name)
      .then(({ agent }) => alive && setLoaded({ name, agent, error: null }))
      .catch((error: ApiError) => alive && setLoaded({ name, agent: null, error }));
    return () => {
      alive = false;
    };
  }, [name]);

  const current = loaded?.name === name ? loaded : null;
  const agent = current?.agent;

  if (current?.error) {
    const missing = current.error.status === 404;
    return (
      <Page className="max-w-3xl">
        <PageHeader back={back} title={name} />
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <BotMessageSquare />
            </EmptyMedia>
            <EmptyTitle>{missing ? "No agent by this name" : "The agent wouldn't load"}</EmptyTitle>
            <EmptyDescription>{missing ? "It may have been removed." : current.error.message}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button variant="outline" asChild>
              <Link to="/agents">All agents</Link>
            </Button>
          </EmptyContent>
        </Empty>
      </Page>
    );
  }

  if (!agent) {
    return (
      <Page className="max-w-3xl">
        <PageHeader back={back} title={<Skeleton className="h-7 w-40" />} description={<Skeleton className="h-4 w-72" />} />
        <Skeleton className="h-36 w-full rounded-2xl" />
      </Page>
    );
  }

  const harness = byId.get(agent.harness);
  const model = agent.model ? (harness?.models.find((one) => one.id === agent.model)?.label ?? agent.model) : null;
  const earlier = (runs ?? []).filter((run) => run.id !== live?.id);

  return (
    <Page className="max-w-3xl">
      <PageHeader
        back={back}
        title={
          <span className="flex items-center gap-3">
            <HarnessLogo id={agent.harness} harness={harness} />
            <span className="truncate">{agent.name}</span>
          </span>
        }
        description={personalityLine(agent) ?? undefined}
      >
        <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <Badge variant="outline">{harness?.label ?? agent.harness}</Badge>
          {model && <Badge variant="secondary">{model}</Badge>}
          {agent.folder && (
            <span className="flex min-w-0 items-center gap-1 font-mono text-2xs" title={agent.folder}>
              <Folder className="size-3 shrink-0" />
              {shortFolder(agent.folder)}
            </span>
          )}
          <span className="ml-auto" />
          <DropdownMenu
            onSelect={({ value }) => {
              if (value === "remove") setRemoving(true);
            }}
          >
            <DropdownMenuTrigger asChild>
              <Button size="icon-sm" variant="ghost" aria-label={`More for ${agent.name}`}>
                <MoreHorizontal />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent className="min-w-40">
              <DropdownMenuItem value="remove" variant="destructive">
                <Trash2 />
                Remove agent
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <Personality agent={agent} />
      </PageHeader>

      {live && <Exchange key={live.id} run={live} agent={agent} harness={harness} onDone={refresh} />}

      <Composer key={agent.name} agent={agent} onStarted={(run) => setLive(run)} />

      <section className="flex flex-col gap-3">
        <h2 className="font-heading text-sm font-semibold tracking-tight">Past runs</h2>
        <RunRows runs={runs && earlier} empty={`Nothing has been asked of ${agent.name} yet.`} />
      </section>

      <RemoveAgent agent={agent} open={removing} onOpenChange={setRemoving} />
    </Page>
  );
}
