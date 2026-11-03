import { KeyRound, LockKeyhole, MessageCircleQuestion } from "lucide-react";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input as TextInput } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { inputText, prompts } from "@/copy/setup";
import type { Input } from "@/lib/setup";

const icons = { sudo: LockKeyhole, passphrase: KeyRound } as Record<string, typeof KeyRound>;

const copyFor = (input: Input) => {
  const known = prompts[input.kind as keyof typeof prompts];
  return {
    title: input.title || known?.title || "",
    why: input.why || known?.why || "",
  };
};

export function InputCard({
  input,
  stepTitle,
  onAnswer,
  onCancel,
}: {
  input: Input;
  stepTitle?: string;
  onAnswer: (values: string[]) => Promise<unknown>;
  onCancel: () => Promise<unknown>;
}) {
  const [values, setValues] = useState<string[]>(() => input.fields.map(() => ""));
  const [mismatch, setMismatch] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const first = useRef<HTMLInputElement>(null);
  const copy = copyFor(input);
  const Icon = icons[input.kind] ?? MessageCircleQuestion;
  const twice = input.kind === "passphrase" && input.fields.length === 2;
  const empty = values.some((value) => value.length === 0);
  const error = failure ?? (mismatch ? inputText.differ : input.error);

  useEffect(() => {
    first.current?.focus();
  }, [input.id]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (empty || busy) return;
    if (twice && values[0] !== values[1]) {
      setMismatch(true);
      return;
    }
    setBusy(true);
    setFailure(null);
    try {
      await onAnswer(values);
    } catch (failed) {
      setFailure((failed as Error).message);
      setBusy(false);
    }
  };

  const cancel = async () => {
    setBusy(true);
    try {
      await onCancel();
    } catch (failed) {
      setFailure((failed as Error).message);
      setBusy(false);
    }
  };

  return (
    <li className="relative flex gap-3.5" data-input={input.kind}>
      <div className="flex w-6 shrink-0 flex-col items-center pt-[1.1875rem]">
        <span className="flex size-6 items-center justify-center rounded-full bg-primary/10 ring-4 ring-primary/5">
          <MessageCircleQuestion className="size-3.5 text-primary" />
        </span>
        <span aria-hidden className="mt-1.5 w-px flex-1 bg-border" />
      </div>
      <form
        onSubmit={submit}
        className="mb-3 flex min-w-0 flex-1 flex-col gap-5 rounded-3xl bg-card px-5 pt-4.5 pb-5 shadow-(--shadow-card) ring-1 ring-border"
      >
        <div className="flex items-start gap-3">
          <span className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-fill-default">
            <Icon className="size-4.5 text-foreground" />
          </span>
          <div className="flex min-w-0 flex-col gap-1">
            {stepTitle && <span className="text-2xs font-medium tracking-wide text-muted-foreground uppercase">{stepTitle}</span>}
            <h3 className="font-heading text-base font-semibold tracking-tight text-balance">{copy.title}</h3>
            {copy.why && <p className="text-[0.8125rem] leading-5 text-pretty text-muted-foreground">{copy.why}</p>}
          </div>
        </div>
        <div className="flex flex-col gap-3.5">
          {input.fields.map((field, index) => (
            <Field key={`${input.id}-${index}`} invalid={Boolean(error)} className="gap-1.5">
              <FieldLabel className="text-xs text-foreground/90">{field.label}</FieldLabel>
              <TextInput
                ref={index === 0 ? first : undefined}
                type={field.secret ? "password" : "text"}
                autoComplete={input.kind === "sudo" ? "current-password" : "new-password"}
                data-size="lg"
                value={values[index]}
                disabled={busy}
                onChange={(event) => {
                  const next = [...values];
                  next[index] = event.target.value;
                  setValues(next);
                  setMismatch(false);
                  setFailure(null);
                }}
              />
            </Field>
          ))}
          {error && (
            <p role="alert" className="text-xs font-medium text-destructive">
              {error}
            </p>
          )}
        </div>
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="ghost" size="lg" onClick={cancel} disabled={busy}>
            {inputText.cancel}
          </Button>
          <Button type="submit" size="lg" disabled={empty || busy}>
            {busy && <Spinner className="size-3.5" />}
            {inputText.continue}
          </Button>
        </div>
      </form>
    </li>
  );
}
