import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  label?: string;
}

interface State {
  error: Error | null;
  info: ErrorInfo | null;
}

/// 兜住 render 期异常,白屏变可读错误。
/// 主要目的是把"设置白屏"这类情况暴露出来(组件 throw → React 卸载整棵树 → 屏幕全白)。
export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, info: null };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    this.setState({ error, info });
    console.error(`[ErrorBoundary${this.props.label ? ` · ${this.props.label}` : ""}]`, error, info);
  }

  reset = () => this.setState({ error: null, info: null });

  render() {
    const { error, info } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="min-h-[200px] p-4 flex flex-col gap-2 text-[11px] text-stone-700 overflow-auto">
        <div className="text-red-600 font-semibold text-[12px]">
          {this.props.label ? `${this.props.label} — 渲染错误` : "渲染错误"}
        </div>
        <div className="font-mono text-red-700 whitespace-pre-wrap break-all">
          {error.name}: {error.message}
        </div>
        {error.stack && (
          <details className="text-stone-500">
            <summary className="cursor-pointer hover:text-stone-700">调用栈</summary>
            <pre className="mt-1 font-mono text-[10px] whitespace-pre-wrap break-all">
              {error.stack}
            </pre>
          </details>
        )}
        {info?.componentStack && (
          <details className="text-stone-500">
            <summary className="cursor-pointer hover:text-stone-700">组件栈</summary>
            <pre className="mt-1 font-mono text-[10px] whitespace-pre-wrap break-all">
              {info.componentStack}
            </pre>
          </details>
        )}
        <button
          onClick={this.reset}
          className="self-start mt-2 px-2 py-1 text-[11px] bg-stone-900 text-white rounded hover:bg-stone-800"
        >
          重试
        </button>
      </div>
    );
  }
}
