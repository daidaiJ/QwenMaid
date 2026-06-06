import { useState, lazy, Suspense } from "react";
import { ActivityBar, type PanelId } from "./ActivityBar";
import { StatusBar } from "./StatusBar";

const ProviderPanel = lazy(() =>
  import("@/components/proxy/ProviderPanel").then((m) => ({
    default: m.ProviderPanel,
  }))
);

const panels: Record<PanelId, React.LazyExoticComponent<React.FC>> = {
  config: lazy(() =>
    import("@/components/config/ConfigPanel").then((m) => ({
      default: m.ConfigPanel,
    }))
  ),
  proxy: ProviderPanel,
  "proxy-status": lazy(() =>
    import("@/components/proxy/ProxyStatusPanel").then((m) => ({
      default: m.ProxyStatusPanel,
    }))
  ),
  cost: lazy(() =>
    import("@/components/analytics/AnalyticsPanel").then((m) => ({
      default: m.AnalyticsPanel,
    }))
  ),
  extensions: lazy(() =>
    import("@/components/extensions/ExtensionsPanel").then((m) => ({
      default: m.ExtensionsPanel,
    }))
  ),
  skills: lazy(() =>
    import("@/components/skills/SkillsPanel").then((m) => ({
      default: m.SkillsPanel,
    }))
  ),
  search: lazy(() =>
    import("@/components/search/SearchPanel").then((m) => ({
      default: m.SearchPanel,
    }))
  ),
  memory: lazy(() =>
    import("@/components/memory/MemoryPanel").then((m) => ({
      default: m.MemoryPanel,
    }))
  ),
  sessions: lazy(() =>
    import("@/components/sessions/SessionsPanel").then((m) => ({
      default: m.SessionsPanel,
    }))
  ),
  subagents: lazy(() =>
    import("@/components/subagents/SubAgentsPanel").then((m) => ({
      default: m.SubAgentsPanel,
    }))
  ),
  install: lazy(() =>
    import("@/components/installer/InstallPanel").then((m) => ({
      default: m.InstallPanel,
    }))
  ),
  about: lazy(() =>
    import("@/components/about/AboutPanel").then((m) => ({
      default: m.AboutPanel,
    }))
  ),
};

export function Shell() {
  const [active, setActive] = useState<PanelId>("cost");
  const [barWidth, setBarWidth] = useState(() => {
    const saved = localStorage.getItem("qwenmaid:activityBarWidthV2");
    return saved ? Number(saved) : 48;
  });
  const Panel = panels[active];

  const handleBarResize = (w: number) => {
    setBarWidth(w);
    localStorage.setItem("qwenmaid:activityBarWidthV2", String(w));
  };

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-[var(--bg-body)] text-[var(--text-primary)]">
      <div className="flex flex-1 min-h-0">
        <ActivityBar active={active} onSelect={setActive} width={barWidth} onResize={handleBarResize} />
        <main className="flex-1 min-w-0 min-h-0 overflow-auto bg-[var(--bg-panel)] rounded-tl-xl">
          <Suspense
            fallback={
              <div className="flex items-center justify-center h-full text-[var(--text-muted)] text-sm">
                加载中…
              </div>
            }
          >
            <Panel />
          </Suspense>
        </main>
      </div>
      <StatusBar />
    </div>
  );
}
