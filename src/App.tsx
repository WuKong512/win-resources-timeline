import { Layout } from "./components/Layout";
import { AppResourcePage } from "./pages/AppResourcePage";
import { ResourcePage } from "./pages/ResourcePage";
import { SettingsPage } from "./pages/SettingsPage";
import { TimelinePage } from "./pages/TimelinePage";
import { TodayPage } from "./pages/TodayPage";
import { useUiStore } from "./stores/uiStore";

export default function App() {
  const page = useUiStore((state) => state.page);

  return (
    <Layout>
      {page === "today" && <TodayPage />}
      {page === "timeline" && <TimelinePage />}
      {page === "resources" && <ResourcePage />}
      {page === "apps" && <AppResourcePage />}
      {page === "settings" && <SettingsPage />}
    </Layout>
  );
}
