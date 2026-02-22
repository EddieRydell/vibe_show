import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[VibeLights] Uncaught error:", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      if (this.props.fallback) return this.props.fallback;
      return (
        <div className="bg-bg text-text flex h-full flex-col items-center justify-center gap-3 p-8">
          <div className="text-error text-lg font-semibold">Something went wrong</div>
          <pre className="bg-surface border-border max-w-lg overflow-auto rounded border p-4 text-xs">
            {this.state.error.message}
          </pre>
          <button
            onClick={() => this.setState({ error: null })}
            className="bg-primary rounded px-4 py-2 text-sm text-white hover:opacity-90"
          >
            Try again
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
