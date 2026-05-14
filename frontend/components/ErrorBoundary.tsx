'use client';

import React from 'react';
import { AlertTriangle } from 'lucide-react';

type Props = {
  children: React.ReactNode;
  fallbackTitle?: string;
};

type State = { error: Error | null };

export default class ErrorBoundary extends React.Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error) {
    console.error('[AEGIS] UI error boundary:', error);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="glass-card rounded-3xl p-10 border border-[#ffb4ab]/30">
          <div className="flex items-center gap-3 text-[#ffb4ab] font-mono tracking-widest text-xs">
            <AlertTriangle className="w-4 h-4" />
            {this.props.fallbackTitle || 'UI RUNTIME ERROR'}
          </div>
          <div className="text-white/50 mt-4 font-mono text-xs whitespace-pre-wrap">
            {this.state.error.message}
          </div>
          <button
            type="button"
            onClick={() => this.setState({ error: null })}
            className="mt-6 px-6 py-2 text-xs font-mono tracking-widest border border-white/20 rounded-xl hover:bg-white/5"
          >
            RETRY
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

