export function StatusBar() {
  return (
    <footer className="h-[22px] bg-[var(--accent)] flex items-center px-3 text-[11px] text-white/90 shrink-0">
      <span className="font-medium">QWenMaid</span>
      <span className="mx-2 text-white/30">│</span>
      <span>代理运行中</span>
      <span className="ml-auto opacity-70">v0.1.0</span>
    </footer>
  );
}
