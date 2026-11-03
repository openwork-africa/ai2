import { useNavigate } from "react-router";
import { AppForm } from "@/components/app-form";
import { Page, PageHeader } from "@/components/page";
import { registerApp } from "@/lib/apps";

export default function AppNew() {
  const navigate = useNavigate();
  return (
    <Page className="max-w-2xl">
      <PageHeader
        back={{ to: "/", label: "Apps" }}
        title="Add an app"
        description="Any app that starts with one command can live here."
      />
      <AppForm
        onCancel={() => navigate("/")}
        onSave={async (fields, icon) => {
          if (!icon) return;
          const { app } = await registerApp(fields, icon);
          navigate(`/apps/${encodeURIComponent(app.id)}`);
        }}
      />
    </Page>
  );
}
