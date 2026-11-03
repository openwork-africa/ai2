import { LockKeyhole } from "lucide-react";
import { useSyncExternalStore } from "react";
import { Route, Routes } from "react-router";
import { Code } from "@/components/page";
import { Shell } from "@/components/shell";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { isLocked, onLocked } from "@/lib/api";
import Agent from "@/pages/Agent";
import Agents from "@/pages/Agents";
import AppDetail from "@/pages/AppDetail";
import AppNew from "@/pages/AppNew";
import Apps from "@/pages/Apps";
import Harnesses from "@/pages/Harnesses";
import NotFound from "@/pages/NotFound";
import Review from "@/pages/Review";
import Reviews from "@/pages/Reviews";
import Run from "@/pages/Run";
import Setup from "@/pages/Setup";
import Vault from "@/pages/Vault";
import Welcome from "@/pages/Welcome";

function Locked() {
  return (
    <Empty className="min-h-svh">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <LockKeyhole />
        </EmptyMedia>
        <EmptyTitle>Open the address that <Code>ai2 ui</Code> printed</EmptyTitle>
        <EmptyDescription>This page only answers the link with its nonce, so no other site can use it.</EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Code>ai2 ui address</Code>
      </EmptyContent>
    </Empty>
  );
}

export default function App() {
  const locked = useSyncExternalStore(onLocked, isLocked);
  if (locked) return <Locked />;
  return (
    <Routes>
      <Route path="/welcome" element={<Welcome />} />
      <Route path="/setup" element={<Setup />} />
      <Route element={<Shell />}>
        <Route index element={<Apps />} />
        <Route path="/apps/new" element={<AppNew />} />
        <Route path="/apps/:id" element={<AppDetail />} />
        <Route path="/agents" element={<Agents />} />
        <Route path="/agents/:name" element={<Agent />} />
        <Route path="/runs/:id" element={<Run />} />
        <Route path="/reviews" element={<Reviews />} />
        <Route path="/reviews/:hash" element={<Review />} />
        <Route path="/vault" element={<Vault />} />
        <Route path="/harnesses" element={<Harnesses />} />
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  );
}
