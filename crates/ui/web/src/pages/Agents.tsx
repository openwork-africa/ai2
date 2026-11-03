import { BotMessageSquare, ChevronRight, Folder, TriangleAlert } from "lucide-react";
import { Link } from "react-router";
import { NewAgentSheet } from "@/components/agent-new";
import { Page, PageHeader } from "@/components/page";
import { HarnessLogo } from "@/components/run-view";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { useAgents, useHarnesses } from "@/hooks/use-agents";
import { type Agent, personalityLine } from "@/lib/agents";
import type { Harness } from "@/lib/harnesses";
import { shortFolder } from "@/lib/reviews";

const modelLabel = (agent: Agent, harness?: Harness) =>
  agent.model ? (harness?.models.find((model) => model.id === agent.model)?.label ?? agent.model) : null;

function Row({ agent, harness }: { agent: Agent; harness?: Harness }) {
  const line = personalityLine(agent);
  const model = modelLabel(agent, harness);
  return (
    <Link
      to={`/agents/${encodeURIComponent(agent.name)}`}
      data-agent={agent.name}
      className="group flex items-center gap-4 border-b px-4 py-4 outline-none transition-colors last:border-0 hover:bg-fill-subtle focus-visible:bg-fill-subtle sm:px-5"
    >
      <HarnessLogo id={agent.harness} harness={harness} />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate font-heading text-sm font-semibold tracking-tight">{agent.name}</span>
        <span className="truncate text-xs text-muted-foreground">{line ?? "No personality"}</span>
        <span className="flex min-w-0 items-center gap-1.5 pt-1 text-2xs text-muted-foreground sm:hidden">
          <span className="shrink-0">{harness?.label ?? agent.harness}</span>
          {model && <span className="shrink-0">· {model}</span>}
          {agent.folder && <span className="truncate font-mono">· {shortFolder(agent.folder)}</span>}
        </span>
      </span>
      <span className="hidden w-60 shrink-0 flex-col items-end gap-1.5 sm:flex">
        <span className="flex items-center gap-1.5">
          <span className="text-xs font-medium">{harness?.label ?? agent.harness}</span>
          {model && (
            <Badge variant="secondary" className="font-normal">
              {model}
            </Badge>
          )}
        </span>
        <span className="flex max-w-full min-w-0 items-center gap-1 text-2xs text-muted-foreground" title={agent.folder}>
          <Folder className="size-3 shrink-0" />
          <span className="truncate font-mono">{agent.folder ? shortFolder(agent.folder) : "No folder"}</span>
        </span>
      </span>
      <ChevronRight className="size-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5" />
    </Link>
  );
}

function Loading() {
  return (
    <Card className="gap-0 py-0">
      {[0, 1, 2].map((row) => (
        <div key={row} className="flex items-center gap-4 border-b px-5 py-4 last:border-0">
          <Skeleton className="size-11 rounded-xl" />
          <div className="flex flex-1 flex-col gap-2">
            <Skeleton className="h-4 w-32" />
            <Skeleton className="h-3 w-64" />
          </div>
          <Skeleton className="hidden h-4 w-40 sm:block" />
        </div>
      ))}
    </Card>
  );
}

export default function Agents() {
  const { agents, error, refresh } = useAgents();
  const { harnesses, byId } = useHarnesses();

  return (
    <Page>
      <PageHeader
        title="Agents"
        description="Each agent is a harness with its own model, personality and folder."
        actions={<NewAgentSheet harnesses={harnesses} onSaved={() => void refresh()} />}
      />
      {error && !agents && (
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <TriangleAlert />
            </EmptyMedia>
            <EmptyTitle>The agents wouldn't load</EmptyTitle>
            <EmptyDescription>{error}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      )}
      {!agents && !error && <Loading />}
      {agents && agents.length === 0 && (
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <BotMessageSquare />
            </EmptyMedia>
            <EmptyTitle>No agents yet</EmptyTitle>
            <EmptyDescription>Agents you make here, or with ai2 agents, are kept on this machine.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      )}
      {agents && agents.length > 0 && (
        <Card className="gap-0 py-0">
          {agents.map((agent) => (
            <Row key={agent.name} agent={agent} harness={byId.get(agent.harness)} />
          ))}
        </Card>
      )}
    </Page>
  );
}
