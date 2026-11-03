import { Page, PageHeader } from "@/components/page";

export default function AppNew() {
  return (
    <Page className="max-w-2xl">
      <PageHeader
        back={{ to: "/", label: "Apps" }}
        title="Add an app"
        description="Anything that listens on a port, started with one command."
      />
    </Page>
  );
}
