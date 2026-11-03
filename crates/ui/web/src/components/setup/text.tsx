import { Code } from "@/components/page";

export function Rich({ text }: { text: string }) {
  const parts = text.split("`");
  return (
    <>
      {parts.map((part, index) => (index % 2 === 1 ? <Code key={index}>{part}</Code> : part))}
    </>
  );
}

export const plain = (text: string) => text.replaceAll("`", "");

export const duration = (ms: number | undefined) => {
  if (ms === undefined) return "";
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const minutes = Math.floor(ms / 60_000);
  const seconds = Math.round((ms % 60_000) / 1000);
  return `${minutes}m ${seconds}s`;
};

export const clock = (ms: number) => {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
};
