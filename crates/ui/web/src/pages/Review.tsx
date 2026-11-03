import { useParams } from "react-router";
import { Page, PageHeader } from "@/components/page";

export default function Review() {
  const { hash = "" } = useParams();
  return (
    <Page>
      <PageHeader back={{ to: "/reviews", label: "Reviews" }} title="Security review" description={hash} />
    </Page>
  );
}
