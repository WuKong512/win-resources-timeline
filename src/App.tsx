import { Layout } from "./components/Layout";
import { RuntimeErrorBoundary } from "./components/RuntimeErrorBoundary";
import { CrashesPage } from "./pages/CrashesPage";
import { SettingsPage } from "./pages/SettingsPage";
import { TimelinePage } from "./pages/TimelinePage";
import { UsagePage } from "./pages/UsagePage";
import { useI18n } from "./i18n";
import { useUiStore } from "./stores/uiStore";

export default function App() {
  const page = useUiStore((state) => state.page);
  const { t } = useI18n();

  return (
    <Layout>
      <RuntimeErrorBoundary title={t("runtimeErrorTitle")} message={t("runtimeErrorMessage")} retryLabel={t("retry")}>
        {page === "timeline" && <TimelinePage />}
        {page === "usage" && <UsagePage />}
        {page === "crashes" && <CrashesPage />}
        {page === "settings" && <SettingsPage />}
      </RuntimeErrorBoundary>
    </Layout>
  );
}
