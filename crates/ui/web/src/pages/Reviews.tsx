import { ShieldCheck } from "lucide-react";
import { Code, Page, PageHeader } from "@/components/page";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";

export default function Reviews() {
  return (
    <Page>
      <PageHeader
        title="Reviews"
        description="Every security review ai2 has run on this machine: the Modern Security Rating and the Run Risk Factor."
      />
      <Empty className="rounded-3xl border border-dashed py-16">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <ShieldCheck />
          </EmptyMedia>
          <EmptyTitle>No reviews yet</EmptyTitle>
          <EmptyDescription>Run ai2 in a folder after an agent has worked in it.</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Code>ai2 --objective "Add a pricing page"</Code>
        </EmptyContent>
      </Empty>
    </Page>
  );
}
