import { Box, Plus, Trash2 } from "lucide-react";
import { type FormEvent, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useVaultQuery, vault } from "@/hooks/use-vault";
import { type Sandbox, type SandboxForm, sandboxRequest, segment } from "@/lib/vault";
import { Chip, Confirm, Failed, Mono, Nothing, Panel, Rows, run, Toolbar } from "@/pages/vault/common";

type SandboxList = { sandboxes: Sandbox[] };

export default function Sandboxes({ mac }: { mac: boolean }) {
  const { data, error, reload } = useVaultQuery<SandboxList>("/sandboxes");
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<Sandbox | null>(null);
  const sandboxes = data?.sandboxes ?? [];

  const remove = (sandbox: Sandbox) =>
    run(() => vault("DELETE", `/sandboxes/${segment(sandbox.name)}`, undefined, `remove ${sandbox.name}`), {
      done: `Removed ${sandbox.name}`,
      mac,
      after: reload,
    });

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
        A sandbox is where calls come from: its own socket, or a TCP address that answers only its peers. A token
        works in one sandbox only.
      </Toolbar>
      {error && !data ? (
        <Failed message={error} />
      ) : !data ? (
        <Rows count={2} />
      ) : sandboxes.length === 0 ? (
        <Nothing icon={Box} title="No sandboxes">
          Setup adds <Mono>host</Mono> for apps on this machine. Add one per container or agent run.
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
              {sandboxes.map((sandbox) => (
                <TableRow key={sandbox.name}>
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
                    <Button
                      variant="ghost" className="text-muted-foreground hover:text-destructive"
                      size="icon-sm"
                      aria-label={`Remove ${sandbox.name}`}
                      onClick={() => setRemoving(sandbox)}
                    >
                      <Trash2 />
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Panel>
      )}
      <AddSandbox open={adding} onOpenChange={setAdding} mac={mac} after={reload} />
      <Confirm
        open={removing !== null}
        onOpenChange={(open) => !open && setRemoving(null)}
        icon={Trash2}
        title={`Remove ${removing?.name ?? "the sandbox"}?`}
        action="Remove sandbox"
        onConfirm={() => removing && remove(removing)}
      >
        Its endpoint stops answering and its tokens stop working.
      </Confirm>
    </div>
  );
}

const EMPTY: SandboxForm = { name: "", transport: "unix", socket: "", listen: "", peers: "" };

function AddSandbox({
  open,
  onOpenChange,
  mac,
  after,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mac: boolean;
  after: () => Promise<void>;
}) {
  const [form, setForm] = useState<SandboxForm>(EMPTY);
  const [busy, setBusy] = useState(false);
  const [problemText, setProblem] = useState("");
  const set = <K extends keyof SandboxForm>(field: K, value: SandboxForm[K]) => setForm((prev) => ({ ...prev, [field]: value }));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const built = sandboxRequest(form);
    if ("problem" in built) return setProblem(built.problem);
    setProblem("");
    setBusy(true);
    const done = await run(() => vault("POST", "/sandboxes", built.request, `add the sandbox ${built.request.name}`), {
      done: `Added ${built.request.name}`,
      mac,
      after,
    });
    setBusy(false);
    if (done !== undefined) {
      setForm(EMPTY);
      onOpenChange(false);
    }
  };

  return (
    <Sheet open={open} onOpenChange={(details) => onOpenChange(details.open)}>
      <SheetContent className="w-full sm:max-w-md">
        <form onSubmit={submit} autoComplete="off" className="flex min-h-0 flex-1 flex-col">
          <SheetHeader className="pb-2">
            <SheetTitle>Add a sandbox</SheetTitle>
            <SheetDescription>The vault opens its endpoint at once.</SheetDescription>
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
                  <FieldDescription>Optional. Empty puts it in the vault's sockets folder, which is what a container mounts.</FieldDescription>
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
                    <FieldDescription>The only addresses it takes calls from, separated by spaces or commas.</FieldDescription>
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
              <Button type="submit" disabled={busy}>
                {busy && <Spinner data-icon="inline-start" />}
                Add sandbox
              </Button>
            </div>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}
