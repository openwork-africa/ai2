import { cn } from "cn";
import { Box, ListPlus, Plus, Trash2 } from "lucide-react";
import { type FormEvent, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { enqueue, useVaultQuery } from "@/hooks/use-vault";
import { type Sandbox, type SandboxForm, sandboxRequest } from "@/lib/vault";
import { Chip, Failed, Mono, Nothing, Panel, Rows, Toolbar, Undo, useQueued } from "@/pages/vault/common";

type SandboxList = { sandboxes: Sandbox[] };

export default function Sandboxes() {
  const { data, error } = useVaultQuery<SandboxList>("/sandboxes");
  const queued = useQueued();
  const [adding, setAdding] = useState(false);
  const sandboxes = data?.sandboxes ?? [];

  return (
    <div className="flex flex-col gap-4">
      <Toolbar
        actions={
          <Button onClick={() => setAdding(true)}>
            <Plus data-icon="inline-start" />
            Add sandbox
          </Button>
        }
      >
        A sandbox is a place calls come from: its own socket, or a TCP address that answers only to its peers. A token
        works in one sandbox and nowhere else.
      </Toolbar>
      {error && !data ? (
        <Failed message={error} />
      ) : !data ? (
        <Rows count={2} />
      ) : sandboxes.length === 0 ? (
        <Nothing icon={Box} title="No sandboxes yet">
          A sandbox is where a token is allowed to call from. Setup makes <Mono>host</Mono> for the apps on this machine; each container or agent run gets one of its own.
        </Nothing>
      ) : (
        <Panel>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="pl-4">Sandbox</TableHead>
                <TableHead>Transport</TableHead>
                <TableHead>Endpoint</TableHead>
                <TableHead className="pr-4 text-right">
                  <span className="sr-only">Actions</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {sandboxes.map((sandbox) => {
                const removing = queued("remove_sandbox", sandbox.name);
                return (
                <TableRow key={sandbox.name} className={cn(removing && "bg-destructive/5 opacity-70 hover:bg-destructive/5")}>
                  <TableCell className="py-3 pl-4">
                    <Mono className="text-sm font-medium">{sandbox.name}</Mono>
                  </TableCell>
                  <TableCell className="py-3">
                    <Badge variant="outline">{sandbox.transport === "unix" ? "Unix socket" : "TCP"}</Badge>
                  </TableCell>
                  <TableCell className="max-w-96 py-3">
                    {sandbox.transport === "unix" ? (
                      <Mono className="block truncate text-muted-foreground" title={sandbox.path}>
                        {sandbox.path}
                      </Mono>
                    ) : (
                      <span className="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
                        <Mono className="text-foreground">{sandbox.listen}</Mono>
                        from
                        {sandbox.peers.map((peer) => (
                          <Chip key={peer}>{peer}</Chip>
                        ))}
                      </span>
                    )}
                  </TableCell>
                  <TableCell className="py-3 pr-4 text-right">
                    {removing ? (
                      <Undo entry={removing} label="Queued to remove" />
                    ) : (
                      <Button
                        variant="ghost"
                        className="text-muted-foreground hover:text-destructive"
                        size="icon-sm"
                        aria-label={`Remove ${sandbox.name}`}
                        onClick={() => enqueue({ action: "remove_sandbox", name: sandbox.name })}
                      >
                        <Trash2 />
                      </Button>
                    )}
                  </TableCell>
                </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Panel>
      )}
      <AddSandbox open={adding} onOpenChange={setAdding} />
    </div>
  );
}

const EMPTY: SandboxForm = { name: "", transport: "unix", socket: "", listen: "", peers: "" };

function AddSandbox({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [form, setForm] = useState<SandboxForm>(EMPTY);
  const [problemText, setProblem] = useState("");
  const set = <K extends keyof SandboxForm>(field: K, value: SandboxForm[K]) => setForm((prev) => ({ ...prev, [field]: value }));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const built = sandboxRequest(form);
    if ("problem" in built) return setProblem(built.problem);
    setProblem("");
    enqueue({ action: "add_sandbox", ...built.request });
    setForm(EMPTY);
    onOpenChange(false);
  };

  return (
    <Sheet open={open} onOpenChange={(details) => onOpenChange(details.open)}>
      <SheetContent className="w-full sm:max-w-md">
        <form onSubmit={submit} autoComplete="off" className="flex min-h-0 flex-1 flex-col">
          <SheetHeader className="pb-2">
            <SheetTitle>Add a sandbox</SheetTitle>
            <SheetDescription>It joins your pending changes. On Apply the vault opens the door.</SheetDescription>
          </SheetHeader>
          <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-6 py-2">
            <FieldGroup className="gap-5">
              <Field>
                <FieldLabel>Name</FieldLabel>
                <Input
                  value={form.name}
                  onChange={(event) => set("name", event.target.value)}
                  placeholder="agent-1"
                  spellCheck={false}
                  className="font-mono"
                  autoFocus
                />
              </Field>
              <Tabs
                value={form.transport}
                onValueChange={(details) => set("transport", details.value as SandboxForm["transport"])}
              >
                <TabsList className="w-full">
                  <TabsTrigger value="unix" className="flex-1">
                    Unix socket
                  </TabsTrigger>
                  <TabsTrigger value="tcp" className="flex-1">
                    TCP
                  </TabsTrigger>
                </TabsList>
              </Tabs>
              {form.transport === "unix" ? (
                <Field>
                  <FieldLabel>Socket path</FieldLabel>
                  <Input
                    value={form.socket}
                    onChange={(event) => set("socket", event.target.value)}
                    placeholder={`…/sockets/${form.name.trim() || "name"}/gateway.sock`}
                    spellCheck={false}
                    className="font-mono"
                  />
                  <FieldDescription>Optional. Left empty, it lands in the vault's sockets folder, which is what a container mounts.</FieldDescription>
                </Field>
              ) : (
                <>
                  <Field>
                    <FieldLabel>Listen on</FieldLabel>
                    <Input
                      value={form.listen}
                      onChange={(event) => set("listen", event.target.value)}
                      placeholder="192.168.64.1:7801"
                      spellCheck={false}
                      className="font-mono"
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Peers</FieldLabel>
                    <Input
                      value={form.peers}
                      onChange={(event) => set("peers", event.target.value)}
                      placeholder="192.168.64.5"
                      spellCheck={false}
                      className="font-mono"
                    />
                    <FieldDescription>The only addresses it will answer, separated by spaces or commas.</FieldDescription>
                  </Field>
                </>
              )}
            </FieldGroup>
          </div>
          <SheetFooter className="border-t pt-4">
            {problemText && <p className="text-xs text-destructive">{problemText}</p>}
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                Cancel
              </Button>
              <Button type="submit">
                <ListPlus data-icon="inline-start" />
                Queue sandbox
              </Button>
            </div>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}
