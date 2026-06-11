// 渲染错误边界 — crash 时显示优雅降级 UI，内联样式不依赖 app CSS

import { Component, type ReactNode } from "react";

interface State {
  crashed: boolean;
}

export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { crashed: false };

  static getDerivedStateFromError(): State {
    return { crashed: true };
  }

  componentDidCatch(error: unknown) {
    console.error("Unhandled render error:", error);
  }

  render() {
    if (!this.state.crashed) return this.props.children;
    return (
      <div
        role="alert"
        style={{
          position: "fixed",
          inset: 0,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          gap: 14,
          background: "var(--bg-body, #f5f5f7)",
          color: "var(--text-primary, #1d1d1f)",
          fontFamily: "Segoe UI Variable, -apple-system, BlinkMacSystemFont, sans-serif",
          textAlign: "center",
          padding: 32,
        }}
      >
        <div style={{ fontSize: 16, fontWeight: 600 }}>
          应用遇到了错误
        </div>
        <div style={{ fontSize: 13, opacity: 0.6 }}>
          渲染过程中发生了未捕获的异常，请刷新页面重试。
        </div>
        <button
          onClick={() => location.reload()}
          style={{
            marginTop: 6,
            padding: "7px 16px",
            fontSize: 13,
            fontWeight: 500,
            borderRadius: 8,
            border: "1px solid var(--border-strong, #d1d1d6)",
            background: "var(--bg-panel, #fff)",
            color: "var(--text-primary, #1d1d1f)",
            cursor: "pointer",
          }}
        >
          刷新页面
        </button>
      </div>
    );
  }
}
