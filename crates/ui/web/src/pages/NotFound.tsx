import { Compass } from "lucide-react";
import { Link } from "react-router";
import { Page } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";

export default function NotFound() {
  return (
    <Page>
      <Empty className="py-24">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Compass />
          </EmptyMedia>
          <EmptyTitle>Nothing lives at this address</EmptyTitle>
          <EmptyDescription>The page may have moved, or the link may have a typo.</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button variant="outline" asChild>
            <Link to="/">Back to apps</Link>
          </Button>
        </EmptyContent>
      </Empty>
    </Page>
  );
}
