import { Ban, Plus, Ticket, TriangleAlert } from "lucide-react";
import { type FormEvent, useState } from "react";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldDescription, FieldGroup, FieldLabel, FieldLegend, FieldSet, FieldTitle } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import { useVaultQuery, vault } from "@/hooks/use-vault";
import {
  METHOD_PRESETS,
  type MethodsPreset,
  type NewToken,
  type Sandbox,
  segment,
  shownToken,
  type TokenForm,
  tokenRequest,
  type VaultKey,
  type VaultToken,
} from "@/lib/vault";
import { Chip, Confirm, CopyButton, Failed, Mono, Nothing, Panel, Rows, run, Segmented, Toolbar, Urls, When } from "@/pages/vault/common";

type TokenList = { tokens: VaultToken[] };

export default function Tokens({ mac }: { mac: boolean }) {
  const { data, error, reload } = useVaultQuery<TokenList>("/tokens");
  const [creating, setCreating] = useState(false);
  const [made, setMade] = useState<NewToken | null>(null);
  const [revoking, setRevoking] = useState<VaultToken | null>(null);
  const tokens = data?.tokens ?? [];

  const revoke = (token: VaultToken) =>
    run(() => vault("DELETE", `/tokens/${segment(token.id)}`, undefined, `revoke ${token.id}`), {
      done: `Revoked ${token.id}`,
      mac,
      after: reload,
    });

  return (
    <div className="flex flex-col gap-4">
      <Toolbar
        actions={
          <Button onClick={() => setCreating(true)}>
            <Plus data-icon="inline-start" />
            Create token
          </Button>
        }
      >
        A token lets one sandbox use some keys, for some methods and URLs, for a while. App tokens call straight
        through; every call on an agent token is vetted against its intent.
      </Toolbar>
      {error && !data ? (
        <Failed message={error} />
      ) : !data ? (
        <Rows />
      ) : tokens.length === 0 ? (
        <Nothing icon={Ticket} title="No tokens">
          Create one for an app or an agent run. The vault keeps only its hash.
        </Nothing>
      ) : (
        <Panel>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="pl-4">Token</TableHead>
                <TableHead>Keys</TableHead>
                <TableHead>Methods</TableHead>
                <TableHead>URLs</TableHead>
                <TableHead>Expires</TableHead>
                <TableHead>Uses</TableHead>
                <TableHead className="pr-4 text-right">
                  <span className="sr-only">Actions</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {tokens.map((token) => (
                <TableRow key={token.id}>
                  <TableCell className="max-w-64 py-3 pl-4 align-top">
                    <div className="flex flex-col gap-1">
                      <span className="flex items-center gap-2">
                        <Mono className="font-medium">{token.id}</Mono>
                        <Badge variant={token.mode === "agent" ? "secondary" : "outline"}>{token.mode}</Badge>
                      </span>
                      <span className="text-2xs text-muted-foreground">
                        in <Mono className="text-2xs">{token.sandbox}</Mono>
                      </span>
                      {token.intent && (
                        <span className="line-clamp-2 text-xs whitespace-normal text-muted-foreground" title={token.intent}>
                          “{token.intent}”
                        </span>
                      )}
                    </div>
                  </TableCell>
                  <TableCell className="py-3 align-top">
                    <span className="flex max-w-48 flex-wrap gap-1">
                      {token.keys.map((key) => (
                        <Chip key={key}>{key}</Chip>
                      ))}
                    </span>
                  </TableCell>
                  <TableCell className="py-3 align-top">
                    <Mono className="text-2xs text-muted-foreground">{token.methods.join(" ")}</Mono>
                  </TableCell>
                  <TableCell className="py-3 align-top">
                    <Urls urls={token.url_prefixes} empty="Same as the keys" />
                  </TableCell>
                  <TableCell className="py-3 align-top text-xs">
                    <When value={token.expires} />
                  </TableCell>
                  <TableCell className="py-3 align-top text-xs tabular-nums">
                    {token.uses_left ?? <span className="text-muted-foreground">∞</span>}
                  </TableCell>
                  <TableCell className="py-3 pr-4 text-right align-top">
                    <Button variant="outline" size="sm" onClick={() => setRevoking(token)}>
                      <Ban data-icon="inline-start" />
                      Revoke
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Panel>
      )}
      <CreateToken
        open={creating}
        onOpenChange={setCreating}
        mac={mac}
        after={reload}
        onMade={(token) => {
          setCreating(false);
          setMade(token);
        }}
      />
      <ShownOnce token={made} onDone={() => setMade(null)} />
      <Confirm
        open={revoking !== null}
        onOpenChange={(open) => !open && setRevoking(null)}
        icon={Ban}
        title={`Revoke ${revoking?.id ?? "the token"}?`}
        action="Revoke token"
        onConfirm={() => revoking && revoke(revoking)}
      >
        Calls made with it are refused from now on.
      </Confirm>
    </div>
  );
}

const EMPTY: TokenForm = {
  keys: [],
  sandbox: "",
  mode: "app",
  methods: "read",
  methodList: "",
  prefixes: "",
  intent: "",
  ttl: "",
  uses: "",
};

function CreateToken({
  open,
  onOpenChange,
  mac,
  after,
  onMade,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mac: boolean;
  after: () => Promise<void>;
  onMade: (token: NewToken) => void;
}) {
  const keys = useVaultQuery<{ keys: VaultKey[] }>(open ? "/keys" : null);
  const sandboxes = useVaultQuery<{ sandboxes: Sandbox[] }>(open ? "/sandboxes" : null);
  const [form, setForm] = useState<TokenForm>(EMPTY);
  const [busy, setBusy] = useState(false);
  const [problemText, setProblem] = useState("");
  const set = <K extends keyof TokenForm>(field: K, value: TokenForm[K]) => setForm((prev) => ({ ...prev, [field]: value }));
  const boxes = sandboxes.data?.sandboxes ?? [];
  const sandbox = form.sandbox || boxes[0]?.name || "";

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const built = tokenRequest({ ...form, sandbox });
    if ("problem" in built) return setProblem(built.problem);
    setProblem("");
    setBusy(true);
    const made = await run(
      () => vault<NewToken & { revoke?: string }>("POST", "/tokens", built.request, `create a token for ${built.request.keys.join(", ")}`),
      { done: "Token created", mac, after },
    );
    setBusy(false);
    if (made) {
      setForm(EMPTY);
      onMade(shownToken(made));
    }
  };

  return (
    <Sheet open={open} onOpenChange={(details) => onOpenChange(details.open)}>
      <SheetContent className="w-full sm:max-w-md">
        <form onSubmit={submit} autoComplete="off" className="flex min-h-0 flex-1 flex-col">
          <SheetHeader className="pb-2">
            <SheetTitle>Create a token</SheetTitle>
            <SheetDescription>It is shown once. The vault keeps only its hash.</SheetDescription>
          </SheetHeader>
          <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-6 py-2">
            <FieldGroup className="gap-5">
              <FieldSet>
                <FieldLegend variant="label" className="mb-2">
                  Keys
                </FieldLegend>
                {!keys.data ? (
                  <span className="text-xs text-muted-foreground">{keys.error ?? "Loading keys…"}</span>
                ) : keys.data.keys.length === 0 ? (
                  <span className="text-xs text-muted-foreground">Add a key first.</span>
                ) : (
                  <div className="flex flex-wrap gap-2">
                    {keys.data.keys.map((key) => {
                      const on = form.keys.includes(key.name);
                      return (
                        <Checkbox
                          key={key.name}
                          checked={on}
                          onCheckedChange={(details) =>
                            set(
                              "keys",
                              details.checked === true ? [...form.keys, key.name] : form.keys.filter((name) => name !== key.name),
                            )
                          }
                          className="rounded-lg border px-2.5 py-1.5 data-[state=checked]:border-primary/40 data-[state=checked]:bg-fill-subtle"
                        >
                          <span className="font-mono text-xs">{key.name}</span>
                        </Checkbox>
                      );
                    })}
                  </div>
                )}
              </FieldSet>
              <div className="flex flex-col gap-2">
                <FieldTitle>Mode</FieldTitle>
                <Segmented
                  value={form.mode}
                  onChange={(value) => set("mode", value as TokenForm["mode"])}
                  options={[
                    { value: "app", label: "App" },
                    { value: "agent", label: "Agent" },
                  ]}
                />
                <p className="text-xs text-muted-foreground">
                  {form.mode === "agent"
                    ? "Every call is vetted against the intent; writes wait for your approval."
                    : "Calls inside the limits go straight through."}
                </p>
              </div>
              <Field>
                <FieldLabel>Sandbox</FieldLabel>
                <NativeSelect className="w-full" value={sandbox} onChange={(event) => set("sandbox", event.target.value)}>
                  {boxes.map((box) => (
                    <NativeSelectOption key={box.name} value={box.name}>
                      {box.name}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
                {sandbox === "host" && mac && (
                  <p className="flex gap-2 rounded-lg bg-warning/10 px-3 py-2 text-xs text-foreground">
                    <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-warning" />
                    Not safe: any process on this Mac outside a container can use a host token.
                  </p>
                )}
              </Field>
              <div className="flex flex-col gap-2">
                <FieldTitle>Methods</FieldTitle>
                <Segmented
                  value={form.methods}
                  onChange={(value) => set("methods", value as MethodsPreset)}
                  options={METHOD_PRESETS.map((entry) => ({ value: entry.value, label: entry.label }))}
                />
                {form.methods === "custom" ? (
                  <Input
                    aria-label="Methods"
                    value={form.methodList}
                    onChange={(event) => set("methodList", event.target.value)}
                    placeholder="GET, POST"
                    spellCheck={false}
                    className="font-mono"
                    autoFocus
                  />
                ) : (
                  <p className="font-mono text-2xs text-muted-foreground">
                    {METHOD_PRESETS.find((entry) => entry.value === form.methods)?.hint}
                  </p>
                )}
              </div>
              <Field>
                <FieldLabel>URL prefixes</FieldLabel>
                <Textarea
                  value={form.prefixes}
                  onChange={(event) => set("prefixes", event.target.value)}
                  rows={2}
                  spellCheck={false}
                  className="font-mono"
                  placeholder="https://api.stripe.com/v1/refunds"
                />
                <FieldDescription>Optional, one per line. Empty means the keys' approved URLs.</FieldDescription>
              </Field>
              <Field>
                <FieldLabel>
                  Intent{form.mode === "agent" && <span className="text-destructive">*</span>}
                </FieldLabel>
                <Textarea
                  value={form.intent}
                  onChange={(event) => set("intent", event.target.value)}
                  rows={2}
                  placeholder="Refund order 1042, as the customer asked"
                />
                <FieldDescription>
                  {form.mode === "agent"
                    ? "Required. What the run is for, in the user's own words; each call is vetted against it."
                    : "Optional for app tokens."}
                </FieldDescription>
              </Field>
              <div className="grid grid-cols-2 gap-3">
                <Field>
                  <FieldLabel>Time to live</FieldLabel>
                  <Input
                    value={form.ttl}
                    onChange={(event) => set("ttl", event.target.value)}
                    placeholder={form.mode === "agent" ? "8h" : "30d"}
                    spellCheck={false}
                    className="font-mono"
                  />
                </Field>
                <Field>
                  <FieldLabel>Uses</FieldLabel>
                  <Input
                    type="number"
                    min={1}
                    step={1}
                    value={form.uses}
                    onChange={(event) => set("uses", event.target.value)}
                    placeholder="Unlimited"
                  />
                </Field>
              </div>
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
                Create token
              </Button>
            </div>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}

function ShownOnce({ token, onDone }: { token: NewToken | null; onDone: () => void }) {
  return (
    <AlertDialog open={token !== null} onOpenChange={(details) => !details.open && onDone()} closeOnInteractOutside={false}>
      <AlertDialogContent className="data-[size=default]:sm:max-w-lg">
        <AlertDialogHeader>
          <AlertDialogMedia className="bg-success/10 text-success">
            <Ticket />
          </AlertDialogMedia>
          <AlertDialogTitle>Copy the new token now</AlertDialogTitle>
          <AlertDialogDescription>
            It is shown this once. The vault keeps only its hash, so it cannot be shown again.
          </AlertDialogDescription>
        </AlertDialogHeader>
        {token && (
          <div className="flex min-w-0 flex-col gap-3">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <Mono>{token.id}</Mono>
              <span>
                {token.mode} · {token.sandbox} · expires <When value={token.expires} />
              </span>
            </div>
            <Input
              readOnly
              value={token.token}
              aria-label="New token"
              spellCheck={false}
              className="font-mono"
              onFocus={(event) => event.currentTarget.select()}
            />
            {token.warnings?.map((warning) => (
              <p key={warning} className="flex gap-2 rounded-lg bg-warning/10 px-3 py-2 text-xs">
                <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-warning" />
                {warning}
              </p>
            ))}
          </div>
        )}
        <AlertDialogFooter>
          <Button variant="outline" onClick={onDone}>
            Done
          </Button>
          {token && <CopyButton value={token.token} label="Copy token" />}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
