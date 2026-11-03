import { cn } from "cn";
import { RotateCw, ScrollText } from "lucide-react";
import { useMemo, useState } from "react";
import { Dot } from "@/components/page";
import { Button } from "@/components/ui/button";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useVaultQuery } from "@/hooks/use-vault";
import { type AuditEntry, decisionTone } from "@/lib/vault";
import { Chip, Failed, Mono, Nothing, Panel, Rows, Toolbar, When } from "@/pages/vault/common";

const TAILS = [50, 200, 1000];

export default function Audit() {
  const [tail, setTail] = useState(200);
  const { data, error, loading, reload } = useVaultQuery<{ entries: AuditEntry[] }>(`/audit?tail=${tail}`, 5000);
  const entries = useMemo(() => (data?.entries ?? []).slice().reverse(), [data]);

  return (
    <div className="flex flex-col gap-4">
      <Toolbar
        actions={
          <>
            <NativeSelect size="sm" aria-label="How many entries" value={String(tail)} onChange={(event) => setTail(Number(event.target.value))}>
              {TAILS.map((count) => (
                <NativeSelectOption key={count} value={String(count)}>
                  Last {count}
                </NativeSelectOption>
              ))}
            </NativeSelect>
            <Button variant="outline" size="sm" onClick={reload}>
              <RotateCw data-icon="inline-start" className={cn(loading && "animate-spin")} />
              Refresh
            </Button>
          </>
        }
      >
        Every use of your keys, newest first, and never the keys themselves.
      </Toolbar>
      {error && !data ? (
        <Failed message={error} />
      ) : !data ? (
        <Rows count={5} />
      ) : entries.length === 0 ? (
        <Nothing icon={ScrollText} title="Nothing has passed through yet">
          Every use of your keys is listed here, from the first one on.
        </Nothing>
      ) : (
        <Panel>
          <div className="max-h-[36rem] overflow-y-auto">
            <Table>
              <TableHeader className="sticky top-0 z-10 bg-card">
                <TableRow>
                  <TableHead className="pl-4">When</TableHead>
                  <TableHead>Decision</TableHead>
                  <TableHead>Call</TableHead>
                  <TableHead>Key</TableHead>
                  <TableHead>From</TableHead>
                  <TableHead className="pr-4 text-right">Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {entries.map((entry, index) => (
                  <TableRow key={`${entry.time}-${index}`}>
                    <TableCell className="py-2.5 pl-4 align-top text-xs text-muted-foreground">
                      <When value={entry.time} />
                    </TableCell>
                    <TableCell className="py-2.5 align-top">
                      <span className="flex flex-col gap-0.5">
                        <span className="flex items-center gap-1.5 text-xs font-medium">
                          <Dot tone={decisionTone(entry.decision)} className="ring-0" />
                          {entry.decision}
                        </span>
                        {entry.reason && <Mono className="text-2xs text-muted-foreground">{entry.reason}</Mono>}
                      </span>
                    </TableCell>
                    <TableCell className="max-w-96 py-2.5 align-top">
                      <span className="flex flex-col gap-0.5">
                        <span className="flex min-w-0 items-center gap-1.5">
                          <Mono className="shrink-0 font-medium">{entry.method}</Mono>
                          <Mono className="truncate text-muted-foreground" title={entry.url ?? ""}>
                            {entry.url ?? "—"}
                          </Mono>
                        </span>
                        {entry.request && (
                          <span className="line-clamp-1 text-2xs whitespace-normal text-muted-foreground" title={entry.request}>
                            “{entry.request}”
                          </span>
                        )}
                      </span>
                    </TableCell>
                    <TableCell className="py-2.5 align-top">{entry.key ? <Chip>{entry.key}</Chip> : "—"}</TableCell>
                    <TableCell className="py-2.5 align-top text-xs">
                      <span className="flex flex-col gap-0.5">
                        <Mono>{entry.sandbox}</Mono>
                        {entry.token && <Mono className="text-2xs text-muted-foreground">{entry.token}</Mono>}
                      </span>
                    </TableCell>
                    <TableCell className="py-2.5 pr-4 text-right align-top">
                      <Mono className={cn(entry.status >= 400 ? "text-destructive" : "text-muted-foreground")}>
                        {entry.status || "—"}
                      </Mono>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </Panel>
      )}
    </div>
  );
}
