import { cn } from "cn";
import { ImageUp, Plus, TriangleAlert, X } from "lucide-react";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { RadioGroup, RadioGroupItem, RadioGroupItemControl, RadioGroupItemHiddenInput, RadioGroupItemText } from "@/components/ui/radio-group";
import { Spinner } from "@/components/ui/spinner";
import { Switch, SwitchControl, SwitchHiddenInput, SwitchThumb } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { ApiError } from "@/lib/api";
import { type App, type AppFields, iconUrl, type Restart } from "@/lib/apps";

const ICON_TYPES = "image/png,image/svg+xml,image/x-icon,image/vnd.microsoft.icon,image/webp,image/jpeg";
const ICON_LIMIT = 512 * 1024;

const RESTARTS: { value: Restart; label: string; why: string }[] = [
  { value: "never", label: "Never", why: "It outlives ai squared." },
  { value: "on-failure", label: "When it fails", why: "After a crash, ai squared brings it back; it stops when ai squared stops." },
  { value: "always", label: "Always", why: "After any exit, ai squared brings it back; it stops when ai squared stops." },
];

type FieldName = "name" | "description" | "icon" | "command" | "cwd" | "port" | "openapi" | "env";
type Errors = Partial<Record<FieldName, string>>;
type Pair = { key: string; value: string };

const FIELD_OF: Record<string, FieldName> = {
  name_required: "name",
  description_required: "description",
  command_required: "command",
  openapi_required: "openapi",
  icon_required: "icon",
  icon_too_large: "icon",
  icon_type: "icon",
  icon_unsafe: "icon",
  icon_invalid: "icon",
  port_required: "port",
  port_invalid: "port",
  cwd_required: "cwd",
  env_invalid: "env",
};

function IconPicker({ app, file, onFile, error }: { app?: App; file: File | null; onFile: (file: File | null) => void; error?: string }) {
  const input = useRef<HTMLInputElement>(null);
  const [preview, setPreview] = useState<string | null>(null);
  useEffect(() => {
    if (file) {
      const url = URL.createObjectURL(file);
      setPreview(url);
      return () => URL.revokeObjectURL(url);
    }
    if (!app) {
      setPreview(null);
      return;
    }
    let alive = true;
    iconUrl(app.id, app.updated).then((url) => alive && setPreview(url)).catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [file, app]);
  return (
    <div className="flex items-center gap-3">
      <button
        type="button"
        onClick={() => input.current?.click()}
        aria-label="Choose the icon"
        className={cn(
          "flex size-14 shrink-0 items-center justify-center overflow-hidden rounded-2xl bg-white shadow-(--shadow-secondary-control) ring-1 ring-black/8 transition hover:ring-black/20 dark:ring-white/10",
          !preview && "border border-dashed border-border bg-fill-subtle shadow-none ring-0",
          error && "ring-2 ring-destructive/40",
        )}
      >
        {preview ? <img src={preview} alt="" className="size-9" /> : <ImageUp className="size-5 text-muted-foreground" />}
      </button>
      <div className="flex min-w-0 flex-col gap-1">
        <div className="flex items-center gap-2">
          <Button type="button" size="sm" variant="outline" onClick={() => input.current?.click()}>
            {preview ? "Change" : "Choose a file…"}
          </Button>
          {file && (
            <Button type="button" size="sm" variant="ghost" onClick={() => onFile(null)}>
              <X />
              Clear
            </Button>
          )}
        </div>
        <span className="truncate text-xs text-muted-foreground">
          {file ? `${file.name} · ${Math.max(1, Math.round(file.size / 1024))} KiB` : "png, svg, ico, webp or jpg, up to 512 KiB"}
        </span>
      </div>
      <input
        ref={input}
        type="file"
        accept={ICON_TYPES}
        className="sr-only"
        tabIndex={-1}
        data-testid="icon-input"
        onChange={(event) => {
          onFile(event.target.files?.[0] ?? null);
          event.target.value = "";
        }}
      />
    </div>
  );
}

function Env({ pairs, onChange }: { pairs: Pair[]; onChange: (pairs: Pair[]) => void }) {
  const set = (at: number, change: Partial<Pair>) => onChange(pairs.map((pair, i) => (i === at ? { ...pair, ...change } : pair)));
  return (
    <div className="flex flex-col gap-2">
      {pairs.map((pair, at) => (
        <div key={at} className="flex items-center gap-2">
          <Input
            aria-label="Name"
            placeholder="KEY"
            value={pair.key}
            onChange={(event) => set(at, { key: event.target.value })}
            className="w-40 font-mono"
          />
          <span className="text-muted-foreground">=</span>
          <Input
            aria-label="Value"
            placeholder="value"
            value={pair.value}
            onChange={(event) => set(at, { value: event.target.value })}
            className="min-w-0 flex-1 font-mono"
          />
          <Button type="button" variant="ghost" size="icon-sm" aria-label="Remove" onClick={() => onChange(pairs.filter((_, i) => i !== at))}>
            <X />
          </Button>
        </div>
      ))}
      <Button type="button" variant="ghost" size="sm" className="w-fit" onClick={() => onChange([...pairs, { key: "", value: "" }])}>
        <Plus data-icon="inline-start" />
        Add a variable
      </Button>
    </div>
  );
}

const toPairs = (env: Record<string, string>): Pair[] => Object.entries(env).map(([key, value]) => ({ key, value }));

export function AppForm({
  app,
  onSave,
  onCancel,
  submitLabel = "Save app",
}: {
  app?: App;
  onSave: (fields: AppFields, icon: File | null) => Promise<void>;
  onCancel: () => void;
  submitLabel?: string;
}) {
  const [name, setName] = useState(app?.name ?? "");
  const [description, setDescription] = useState(app?.description ?? "");
  const [command, setCommand] = useState(app?.command ?? "");
  const [cwd, setCwd] = useState(app?.cwd ?? "");
  const [port, setPort] = useState(app ? String(app.port) : "");
  const [setsPort, setSetsPort] = useState(app?.set_port ?? true);
  const [openapi, setOpenapi] = useState(app?.openapi ?? "/openapi.json");
  const [restart, setRestart] = useState<Restart>(app?.restart ?? "never");
  const [pairs, setPairs] = useState<Pair[]>(toPairs(app?.env ?? {}));
  const [icon, setIcon] = useState<File | null>(null);
  const [errors, setErrors] = useState<Errors>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const validate = (): Errors => {
    const found: Errors = {};
    if (!name.trim()) found.name = "The app needs a name.";
    if (!description.trim()) found.description = "One line on what the app is for.";
    if (!command.trim()) found.command = "The command that runs the app.";
    if (!cwd.trim().startsWith("/")) found.cwd = "The app's folder, as a full path.";
    if (!openapi.trim()) found.openapi = "Where the OpenAPI document lives.";
    if (!app && !icon) found.icon = "The app needs an icon.";
    if (icon && icon.size > ICON_LIMIT) found.icon = `That icon is ${Math.round(icon.size / 1024)} KiB; 512 KiB is the ceiling.`;
    if (port.trim() && !(Number.isInteger(Number(port)) && Number(port) > 0 && Number(port) < 65536)) found.port = "A port between 1 and 65535.";
    if (pairs.some((pair) => !pair.key.trim() && pair.value)) found.env = "Each value needs a name.";
    return found;
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const found = validate();
    setErrors(found);
    setFailure(null);
    if (Object.keys(found).length) return;
    const fields: AppFields = {
      name: name.trim(),
      description: description.trim(),
      command: command.trim(),
      openapi: openapi.trim(),
      env: Object.fromEntries(pairs.filter((pair) => pair.key.trim()).map((pair) => [pair.key.trim(), pair.value])),
      set_port: setsPort,
      restart,
      cwd: cwd.trim(),
      ...(port.trim() ? { port: Number(port) } : {}),
    };
    setSaving(true);
    try {
      await onSave(fields, icon);
    } catch (caught) {
      const error = caught as ApiError;
      const field = FIELD_OF[error.code] ?? (error.code?.startsWith("port") ? "port" : undefined);
      if (field) setErrors({ [field]: error.message });
      else setFailure(error.message);
    } finally {
      setSaving(false);
    }
  };

  const restartWhy = RESTARTS.find((one) => one.value === restart)?.why;

  return (
    <form onSubmit={submit} noValidate className="flex flex-col gap-8">
      {failure && (
        <Alert variant="destructive">
          <TriangleAlert />
          <AlertTitle>Not saved</AlertTitle>
          <AlertDescription>{failure}</AlertDescription>
        </Alert>
      )}
      <FieldGroup className="gap-6">
        <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.name)}>
          <FieldLabel>Name</FieldLabel>
          <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="Workspot" autoFocus={!app} />
          <FieldError className="text-destructive">{errors.name}</FieldError>
        </Field>
        <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.description)}>
          <FieldLabel>Description</FieldLabel>
          <Textarea
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder="Listings for the workspot site"
            className="min-h-12"
          />
          <FieldError className="text-destructive">{errors.description}</FieldError>
        </Field>
        <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.icon)}>
          <FieldLabel>Icon</FieldLabel>
          <IconPicker app={app} file={icon} onFile={setIcon} error={errors.icon} />
          <FieldError className="text-destructive">{errors.icon}</FieldError>
        </Field>
      </FieldGroup>

      <FieldGroup className="gap-6 border-t pt-8">
        <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.command)}>
          <FieldLabel>Run command</FieldLabel>
          <Input value={command} onChange={(event) => setCommand(event.target.value)} placeholder="deno task start" className="font-mono" />
          <FieldDescription>It runs in a shell, inside the folder below.</FieldDescription>
          <FieldError className="text-destructive">{errors.command}</FieldError>
        </Field>
        <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.cwd)}>
          <FieldLabel>Folder</FieldLabel>
          <Input value={cwd} onChange={(event) => setCwd(event.target.value)} placeholder="/Users/you/Documents/workspot" className="font-mono" />
          <FieldDescription>Where the command runs, as a full path.</FieldDescription>
          <FieldError className="text-destructive">{errors.cwd}</FieldError>
        </Field>
        <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.port)}>
          <FieldLabel>Port</FieldLabel>
          <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
            <Input
              value={port}
              onChange={(event) => setPort(event.target.value.replace(/[^0-9]/g, ""))}
              placeholder="7791"
              inputMode="numeric"
              className="w-28 font-mono"
            />
            <label className="flex items-center gap-2 text-sm">
              <Switch checked={setsPort} onCheckedChange={({ checked }) => setSetsPort(checked)}>
                <SwitchControl>
                  <SwitchThumb />
                </SwitchControl>
                <SwitchHiddenInput />
              </Switch>
              ai squared sets <code className="font-mono text-xs">PORT</code>
            </label>
          </div>
          <FieldDescription>Leave it empty and ai squared works it out from the command or the code.</FieldDescription>
          <FieldError className="text-destructive">{errors.port}</FieldError>
        </Field>
        <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.openapi)}>
          <FieldLabel>OpenAPI</FieldLabel>
          <Input value={openapi} onChange={(event) => setOpenapi(event.target.value)} placeholder="/openapi.json" className="font-mono" />
          <FieldDescription>A path under the app's URL, or a file on disk. Once the app is running, ai squared reads it from there.</FieldDescription>
          <FieldError className="text-destructive">{errors.openapi}</FieldError>
        </Field>
      </FieldGroup>

      <FieldGroup className="gap-6 border-t pt-8">
        <Field>
          <FieldLabel>Restart</FieldLabel>
          <RadioGroup
            value={restart}
            onValueChange={({ value }) => value && setRestart(value as Restart)}
            className="flex flex-wrap gap-x-5 gap-y-2"
          >
            {RESTARTS.map((one) => (
              <RadioGroupItem key={one.value} value={one.value}>
                <RadioGroupItemControl />
                <RadioGroupItemText>{one.label}</RadioGroupItemText>
                <RadioGroupItemHiddenInput />
              </RadioGroupItem>
            ))}
          </RadioGroup>
          <FieldDescription>{restartWhy}</FieldDescription>
        </Field>
        <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.env)}>
          <FieldLabel>Environment</FieldLabel>
          <Env pairs={pairs} onChange={setPairs} />
          <FieldError className="text-destructive">{errors.env}</FieldError>
        </Field>
      </FieldGroup>

      <div className="flex items-center justify-end gap-2 border-t pt-6">
        <Button type="button" variant="ghost" onClick={onCancel} disabled={saving}>
          Cancel
        </Button>
        <Button type="submit" disabled={saving}>
          {saving && <Spinner />}
          {submitLabel}
        </Button>
      </div>
    </form>
  );
}
