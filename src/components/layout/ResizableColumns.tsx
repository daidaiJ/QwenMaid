import {
  Group,
  Panel,
  Separator,
  useDefaultLayout,
} from "react-resizable-panels";
import { type ReactNode } from "react";

interface ColumnProps {
  children: ReactNode;
  defaultSize?: number;
  minSize?: number;
  maxSize?: number;
  collapsible?: boolean;
  className?: string;
}

interface ResizableColumnsProps {
  autoSaveId: string;
  left?: ColumnProps;
  center: ColumnProps;
  right?: ColumnProps;
  className?: string;
}

function ResizeHandle() {
  return (
    <Separator className="w-1 bg-[var(--border)]/50 hover:bg-[var(--accent)]/40 active:bg-[var(--accent)]/60 transition-colors cursor-col-resize" />
  );
}

export function ResizableColumns({
  autoSaveId,
  left,
  center,
  right,
  className = "",
}: ResizableColumnsProps) {
  const ids = {
    left: `${autoSaveId}-left`,
    center: `${autoSaveId}-center`,
    right: `${autoSaveId}-right`,
  };

  // 计算默认百分比并归一化到 100
  const rawL = left?.defaultSize ?? 0;
  const rawC = center.defaultSize ?? 50;
  const rawR = right?.defaultSize ?? 0;
  const total = rawL + rawC + rawR || 1;
  const pct = (v: number) => (v / total) * 100;

  // 构建默认布局
  const fallbackLayout: Record<string, number> = {};
  if (left) fallbackLayout[ids.left] = pct(rawL);
  fallbackLayout[ids.center] = pct(rawC);
  if (right) fallbackLayout[ids.right] = pct(rawR);

  const panelIds = [
    ...(left ? [ids.left] : []),
    ids.center,
    ...(right ? [ids.right] : []),
  ];

  const persisted = useDefaultLayout({
    id: autoSaveId,
    panelIds,
  });
  const defaultLayout = persisted.defaultLayout ?? fallbackLayout;

  return (
    <Group
      orientation="horizontal"
      className={`w-full h-full ${className}`}
      defaultLayout={defaultLayout}
      onLayoutChanged={persisted.onLayoutChanged}
    >
      {left && (
        <>
          <Panel
            id={ids.left}
            minSize={`${left.minSize ?? 10}%`}
            maxSize={`${left.maxSize ?? 30}%`}
            className={left.className}
          >
            {left.children}
          </Panel>
          <ResizeHandle />
        </>
      )}
      <Panel
        id={ids.center}
        minSize={`${center.minSize ?? 20}%`}
        className={center.className}
      >
        {center.children}
      </Panel>
      {right && (
        <>
          <ResizeHandle />
          <Panel
            id={ids.right}
            minSize={`${right.minSize ?? 10}%`}
            maxSize={`${right.maxSize ?? 40}%`}
            collapsible={right.collapsible}
            className={right.className}
          >
            {right.children}
          </Panel>
        </>
      )}
    </Group>
  );
}
