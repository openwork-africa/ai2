import { useParams } from "react-router";
import { Page, PageHeader } from "@/components/page";

export default function AppDetail() {
  const { id = "" } = useParams();
  return (
    <Page>
      <PageHeader back={{ to: "/", label: "Apps" }} title={id} />
    </Page>
  );
}
