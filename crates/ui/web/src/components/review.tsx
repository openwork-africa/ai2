import { cn } from "cn";
import { CircleCheck, CircleDot, FolderGit2, Info, Sparkles } from "lucide-react";
import type * as React from "react";
import { useSearchParams } from "react-router";
import { Code } from "@/components/page";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { HoverCard, HoverCardContent, HoverCardTrigger } from "@/components/ui/hover-card";
import { Progress, ProgressRange, ProgressTrack } from "@/components/ui/progress";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger, TabsTriggerCount } from "@/components/ui/tabs";
import {
  bySeverity,
  dated,
  type Card as RatingCard,
  type Factor,
  isFixed,
  percent,
  type Review,
  type Tone,
  severityTone,
  shortFolder,
  tone,
  toneFill,
  toneSoft,
  toneStroke,
  toneText,
  toneWord,
} from "@/lib/reviews";

export function ScorePill({ score, riskier, className }: { score: number | null; riskier: boolean; className?: string }) {
  const t = tone(score, riskier);
  return (
    <span
      className={cn(
        "inline-flex h-6 min-w-12 items-center justify-center rounded-full px-2 text-xs font-semibold tabular-nums",
        toneSoft[t],
        className,
      )}
    >
      {score === null ? "—" : percent(score)}
    </span>
  );
}

function Bar({ value, riskier, className }: { value: number; riskier: boolean; className?: string }) {
  return (
    <Progress value={Math.min(100, Math.max(0, value))} className={cn("gap-0", className)}>
      <ProgressTrack className="h-1.5 bg-fill-default">
        <ProgressRange className={cn("rounded-full", toneFill[tone(value, riskier)])} />
      </ProgressTrack>
    </Progress>
  );
}

const CIRCUMFERENCE = 2 * Math.PI * 15.5;

function Ring({ value, tone: t }: { value: number; tone: Tone }) {
  const filled = (Math.min(100, Math.max(0, value)) / 100) * CIRCUMFERENCE;
  return (
    <svg viewBox="0 0 36 36" className="size-12 shrink-0 -rotate-90" role="img" aria-label={percent(value)}>
      <circle cx="18" cy="18" r="15.5" fill="none" strokeWidth="4" className="stroke-fill-default" />
      <circle
        cx="18"
        cy="18"
        r="15.5"
        fill="none"
        strokeWidth="4"
        strokeLinecap="round"
        strokeDasharray={`${filled} ${CIRCUMFERENCE}`}
        className={toneStroke[t]}
      />
    </svg>
  );
}

function FactorRow({ factor, riskier }: { factor: Factor; riskier: boolean }) {
  return (
    <li className="flex flex-col gap-1.5 py-3 first:pt-0 last:pb-0">
      <div className="flex items-baseline justify-between gap-3">
        <span className="font-medium">{factor.name}</span>
        <span className={cn("text-sm font-semibold tabular-nums", toneText[tone(factor.score, riskier)])}>
          {percent(factor.score)}
        </span>
      </div>
      <Bar value={factor.score} riskier={riskier} />
      <p className="text-xs leading-relaxed text-muted-foreground">{factor.why}</p>
    </li>
  );
}

function RatingTile({ card }: { card: RatingCard }) {
  const rating = card.rating;
  const t = tone(rating?.score, card.riskier);
  return (
    <Card className="gap-0 py-0">
      <CardHeader className="flex flex-row items-start justify-between gap-4 border-b py-5">
        <div className="flex min-w-0 flex-col gap-1">
          <div className="flex items-center gap-2">
            <h2 className="font-heading text-base font-semibold tracking-tight">{card.name}</h2>
            <Badge variant="secondary">{card.short}</Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            {card.riskier ? "Lower is safer. A reviewer read what the run changed." : "Higher is safer. Fresh exploits against the stack, weighed."}
          </p>
        </div>
        {rating ? (
          <div className="flex shrink-0 items-center gap-3">
            <div className="flex flex-col items-end leading-none">
              <span className={cn("font-heading text-4xl font-semibold tracking-tight tabular-nums", toneText[t])}>
                {Math.round(rating.score)}
                <span className="text-xl">%</span>
              </span>
              <span className={cn("mt-1.5 text-2xs font-medium uppercase tracking-wider", toneText[t])}>
                {toneWord[t]}
              </span>
            </div>
            <Ring value={rating.score} tone={t} />
          </div>
        ) : (
          <div className="flex shrink-0 flex-col items-end leading-none text-muted-foreground">
            <span className="font-heading text-4xl font-semibold tracking-tight">–</span>
            <span className="mt-1.5 text-2xs font-medium uppercase tracking-wider">{toneWord.none}</span>
          </div>
        )}
      </CardHeader>
      <CardContent className="py-5">
        {rating ? (
          <ul className="flex flex-col divide-y">
            {rating.factors.map((factor) => (
              <FactorRow key={factor.name} factor={factor} riskier={card.riskier} />
            ))}
          </ul>
        ) : (
          <div className="flex items-start gap-2.5 rounded-2xl bg-fill-default p-3.5 text-sm text-muted-foreground">
            <Info className="mt-0.5 size-4 shrink-0" />
            <p>{card.missing || "No rating this time; the notes say why."}</p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function Panel({ children, className }: { children: React.ReactNode; className?: string }) {
  return <Card className={cn("gap-0 py-0", className)}>{children}</Card>;
}

function None({ children }: { children: React.ReactNode }) {
  return (
    <Panel>
      <p className="px-5 py-10 text-center text-sm text-muted-foreground">{children}</p>
    </Panel>
  );
}

function Heads({ names }: { names: string[] }) {
  return (
    <TableHeader>
      <TableRow className="hover:bg-transparent">
        {names.map((name, at) => (
          <TableHead
            key={name}
            className={cn("text-xs font-medium text-muted-foreground", at === 0 && "pl-5", at === names.length - 1 && "pr-5")}
          >
            {name}
          </TableHead>
        ))}
      </TableRow>
    </TableHeader>
  );
}

function Technologies({ review }: { review: Review }) {
  if (!review.technologies.length) return <None>No technologies turned up.</None>;
  const rows = [...review.technologies].sort((a, b) => b.risk - a.risk);
  return (
    <Panel>
      <Table>
        <Heads names={["Technology", "Kind", "Share of the risk"]} />
        <TableBody>
          {rows.map((tech) => (
            <TableRow key={tech.name}>
              <TableCell className="pl-5 font-medium">{tech.name}</TableCell>
              <TableCell>
                <Badge variant="outline" className="capitalize">
                  {tech.kind}
                </Badge>
              </TableCell>
              <TableCell className="w-1/2 pr-5">
                <div className="flex items-center gap-3">
                  <Progress value={Math.min(100, tech.risk)} className="flex-1 gap-0">
                    <ProgressTrack className="h-1.5 bg-fill-default">
                      <ProgressRange className="rounded-full bg-chart-2" />
                    </ProgressTrack>
                  </Progress>
                  <span className="w-10 text-right text-sm tabular-nums">{percent(tech.risk)}</span>
                </div>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Panel>
  );
}

function Severity({ value }: { value: string }) {
  return (
    <span className={cn("inline-flex h-5 items-center rounded-full px-2 text-2xs font-medium capitalize", severityTone(value))}>
      {value || "unknown"}
    </span>
  );
}

function Exploits({ review }: { review: Review }) {
  if (!review.exploits.length) return <None>No recent exploits in this stack.</None>;
  return (
    <Panel>
      <Table>
        <Heads names={["Id", "Technology", "Severity", "Fixed in", "Summary"]} />
        <TableBody>
          {bySeverity(review.exploits).map((exploit) => (
            <TableRow key={exploit.id}>
              <TableCell className="pl-5 align-top">
                <Code className="whitespace-nowrap">{exploit.id}</Code>
              </TableCell>
              <TableCell className="align-top font-medium">{exploit.technology}</TableCell>
              <TableCell className="align-top">
                <Severity value={exploit.severity} />
              </TableCell>
              <TableCell className="align-top tabular-nums">
                {exploit.fixed_in ? exploit.fixed_in : <span className="text-muted-foreground">No fix yet</span>}
              </TableCell>
              <TableCell className="min-w-72 pr-5 align-top whitespace-normal text-muted-foreground">
                {exploit.summary}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Panel>
  );
}

function Status({ fixed, label }: { fixed: boolean; label?: string }) {
  return (
    <span
      className={cn(
        "inline-flex h-5 items-center gap-1 rounded-full px-2 text-2xs font-medium capitalize",
        fixed ? toneSoft.good : toneSoft.fair,
      )}
    >
      {fixed ? <CircleCheck className="size-3" /> : <CircleDot className="size-3" />}
      {label || (fixed ? "fixed" : "open")}
    </span>
  );
}

function Findings({ review }: { review: Review }) {
  if (!review.findings.length) return <None>Nothing to report.</None>;
  const rows = bySeverity(review.findings).sort((a, b) => Number(isFixed(a)) - Number(isFixed(b)));
  return (
    <Panel>
      <Table>
        <Heads names={["Finding", "Severity", "Status", "Fix"]} />
        <TableBody>
          {rows.map((finding) => (
            <TableRow key={`${finding.title}-${finding.file}`}>
              <TableCell className="min-w-64 pl-5 align-top whitespace-normal">
                <div className="flex flex-col gap-1">
                  <span className="font-medium">{finding.title}</span>
                  <span className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                    {finding.file && <Code className="text-2xs">{finding.file}</Code>}
                    {finding.technology && <span>{finding.technology}</span>}
                  </span>
                </div>
              </TableCell>
              <TableCell className="align-top">
                <div className="flex flex-col items-start gap-1">
                  <Severity value={finding.severity} />
                  {(finding.likelihood || finding.impact) && (
                    <span className="text-2xs text-muted-foreground">
                      {[finding.likelihood && `${finding.likelihood} likelihood`, finding.impact && `${finding.impact} impact`]
                        .filter(Boolean)
                        .join(" · ")}
                    </span>
                  )}
                </div>
              </TableCell>
              <TableCell className="align-top">
                <Status fixed={isFixed(finding)} label={finding.status} />
              </TableCell>
              <TableCell className="min-w-72 pr-5 align-top whitespace-normal text-muted-foreground">
                {finding.fix || "—"}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Panel>
  );
}

function Items({ cards }: { cards: RatingCard[] }) {
  return (
    <div className="flex flex-col gap-6">
      {cards.map((card) => (
        <Panel key={card.key}>
          <div className="flex items-center gap-2 border-b px-5 py-3.5">
            <h3 className="text-sm font-semibold">{card.name}</h3>
            <span className="text-xs text-muted-foreground">per item</span>
          </div>
          <ol className="flex flex-col divide-y">
            {(card.items ?? []).map((item) => (
              <li key={item.index} className="flex items-start gap-4 px-5 py-3.5">
                <span className="w-5 pt-0.5 text-right text-xs text-muted-foreground tabular-nums">{item.index + 1}</span>
                <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                  <p className="text-sm">{item.text}</p>
                  <div className="flex flex-wrap gap-1.5">
                    {item.factors.map((factor) => (
                      <HoverCard key={factor.name} openDelay={150} closeDelay={80}>
                        <HoverCardTrigger asChild>
                          <button
                            type="button"
                            className="inline-flex h-5 items-center gap-1 rounded-full bg-fill-default px-2 text-2xs text-muted-foreground transition-colors hover:bg-fill-hover hover:text-foreground"
                          >
                            {factor.name}
                            <span className={cn("font-semibold tabular-nums", toneText[tone(factor.score, card.riskier)])}>
                              {percent(factor.score)}
                            </span>
                          </button>
                        </HoverCardTrigger>
                        <HoverCardContent className="w-80">
                          <div className="flex flex-col gap-2">
                            <div className="flex items-baseline justify-between">
                              <span className="font-medium">{factor.name}</span>
                              <span className={cn("font-semibold tabular-nums", toneText[tone(factor.score, card.riskier)])}>
                                {percent(factor.score)}
                              </span>
                            </div>
                            <Bar value={factor.score} riskier={card.riskier} />
                            <p className="text-xs leading-relaxed text-muted-foreground">{factor.why}</p>
                          </div>
                        </HoverCardContent>
                      </HoverCard>
                    ))}
                  </div>
                </div>
                <ScorePill score={item.score} riskier={card.riskier} />
              </li>
            ))}
          </ol>
        </Panel>
      ))}
    </div>
  );
}

function Notes({ review }: { review: Review }) {
  if (!review.notes.length) return <None>No notes.</None>;
  return (
    <Panel>
      <ul className="flex flex-col divide-y">
        {review.notes.map((note, at) => (
          <li key={at} className="flex items-start gap-3 px-5 py-3.5 text-sm">
            <Sparkles className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
            <span>{note}</span>
          </li>
        ))}
      </ul>
    </Panel>
  );
}

export function ReviewHeader({ review }: { review: Review }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm text-muted-foreground">
        <span className="flex min-w-0 items-center gap-1.5" title={review.folder}>
          <FolderGit2 className="size-4 shrink-0" />
          <span className="truncate font-mono text-xs text-foreground">{shortFolder(review.folder)}</span>
        </span>
        <span aria-hidden>·</span>
        <time dateTime={review.generated_at}>{dated(review.generated_at)}</time>
        {review.harness && (
          <Badge variant="outline" className="capitalize">
            {review.harness}
          </Badge>
        )}
        <Badge variant="secondary">v{review.version}</Badge>
      </div>
      {review.objective && (
        <blockquote className="max-w-prose border-l-2 border-foreground/20 pl-3 text-base text-foreground">
          {review.objective}
        </blockquote>
      )}
    </div>
  );
}

export function ReviewBody({ review }: { review: Review }) {
  const [params, setParams] = useSearchParams();
  const withItems = review.ratings.filter((card) => card.items?.length);
  const open = review.findings.filter((finding) => !isFixed(finding)).length;
  const tabs = [
    { value: "technology", label: "Technology", count: review.technologies.length },
    { value: "exploits", label: "Exploits", count: review.exploits.length },
    { value: "findings", label: "Findings", count: review.findings.length, note: open ? `${open} open` : undefined },
    ...(withItems.length
      ? [{ value: "items", label: "Items", count: withItems.reduce((sum, card) => sum + (card.items?.length ?? 0), 0) }]
      : []),
    { value: "notes", label: "Notes", count: review.notes.length },
  ];
  return (
    <div className="flex flex-col gap-8">
      <section className="grid gap-4 md:grid-cols-2">
        {review.ratings.map((card) => (
          <RatingTile key={card.key} card={card} />
        ))}
      </section>
      <Tabs
        value={tabs.some((tab) => tab.value === params.get("tab")) ? params.get("tab")! : "technology"}
        onValueChange={(details) => setParams({ tab: details.value }, { replace: true })}
        className="gap-4"
      >
        <div className="overflow-x-auto border-b">
          <TabsList variant="line" className="h-9 gap-1 pb-0">
            {tabs.map((tab) => (
              <TabsTrigger key={tab.value} value={tab.value} className="h-9 px-2.5 text-sm">
                {tab.label}
                <TabsTriggerCount className={cn(tab.note && "text-warning")}>{tab.note ?? tab.count}</TabsTriggerCount>
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        <TabsContent value="technology">
          <Technologies review={review} />
        </TabsContent>
        <TabsContent value="exploits">
          <Exploits review={review} />
        </TabsContent>
        <TabsContent value="findings">
          <Findings review={review} />
        </TabsContent>
        {withItems.length > 0 && (
          <TabsContent value="items">
            <Items cards={withItems} />
          </TabsContent>
        )}
        <TabsContent value="notes">
          <Notes review={review} />
        </TabsContent>
      </Tabs>
    </div>
  );
}
