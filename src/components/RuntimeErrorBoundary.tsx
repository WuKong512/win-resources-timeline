import { Component, type ReactNode } from "react";
import { Button } from "./ui/Button";

type RuntimeErrorBoundaryProps = {
  children: ReactNode;
  title: string;
  message: string;
  retryLabel: string;
};

type RuntimeErrorBoundaryState = {
  hasError: boolean;
};

export class RuntimeErrorBoundary extends Component<RuntimeErrorBoundaryProps, RuntimeErrorBoundaryState> {
  state: RuntimeErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): RuntimeErrorBoundaryState {
    return { hasError: true };
  }

  reset = () => {
    this.setState({ hasError: false });
  };

  render() {
    if (this.state.hasError) {
      return <div role="alert" className="error-surface rounded-lg border px-5 py-6"><div className="font-semibold">{this.props.title}</div><p className="mt-1 text-sm opacity-80">{this.props.message}</p><Button className="mt-4" variant="outline" onClick={this.reset}>{this.props.retryLabel}</Button></div>;
    }
    return this.props.children;
  }
}
