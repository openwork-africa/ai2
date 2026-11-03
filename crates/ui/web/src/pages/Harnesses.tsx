import { Bot } from "lucide-react";
import { Code, Page, PageHeader } from "@/components/page";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";

export default function Harnesses() {
  return (
    <Page>
      <PageHeader
        title="Harnesses"
        description="The coding agents ai2 can install, sign in to and run headless."
      />
      <Empty className="rounded-3xl border border-dashed py-16">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Bot />
          </EmptyMedia>
          <EmptyTitle>Harnesses appear here</EmptyTitle>
          <EmptyDescription>Installed or not, signed in or not, with the models each one offers.</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Code>ai2 harness list</Code>
        </EmptyContent>
      </Empty>
    </Page>
  );
}
