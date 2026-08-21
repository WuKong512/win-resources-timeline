import { Layout } from "./components/Layout";
import { CrashesPage } from "./pages/CrashesPage";
import { SettingsPage } from "./pages/SettingsPage";
import { TimelinePage } from "./pages/TimelinePage";
import { UsagePage } from "./pages/UsagePage";
import { useUiStore } from "./stores/uiStore";

export default function App() {
  const page = useUiStore((state) => state.page);

  return (
    <Layout>
      {page === "timeline" && <TimelinePage />}
      {page === "usage" && <UsagePage />}
      {page === "crashes" && <CrashesPage />}
      {page === "settings" && <SettingsPage />}
    </Layout>
  );
}
