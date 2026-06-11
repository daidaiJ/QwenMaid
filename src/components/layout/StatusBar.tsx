export function StatusBar() {
  return (
    <footer className="h-[24px] bg-[var(--bg-sidebar)]/80 backdrop-blur-sm flex items-center px-3 text-[11px] text-[var(--text-secondary)] shrink-0 border-t border-[var(--border)]">
      <span className="w-2 h-2 rounded-full bg-[var(--color-success)] mr-1.5" />
      <span className="font-medium text-[var(--text-primary)]">QWenMaid</span>
      <span className="mx-2 text-[var(--border)]">│</span>
      <span>代理运行中</span>
      <span className="ml-auto text-[var(--text-muted)]">v{__APP_VERSION__}</span>
    </footer>
  );
}
