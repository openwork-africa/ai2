import { LayoutGrid, Plus } from "lucide-react";
import { Link } from "react-router";
import { Code, Page, PageHeader } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";

export default function Apps() {
  return (
    <Page>
      <PageHeader
        title="Apps"
        description="Local apps, one click to start."
        actions={
          <Button asChild>
            <Link to="/apps/new">
              <Plus data-icon="inline-start" />
              Add app
            </Link>
          </Button>
        }
      />
      <Empty className="rounded-3xl border border-dashed py-16">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <LayoutGrid />
          </EmptyMedia>
          <EmptyTitle>No apps yet</EmptyTitle>
          <EmptyDescription>Add one, or ask your agent to register the app it just built.</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Code>ai2 apps register --openapi /openapi.json …</Code>
        </EmptyContent>
      </Empty>
    </Page>
  );
}
