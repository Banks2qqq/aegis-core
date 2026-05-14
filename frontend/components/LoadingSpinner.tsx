export default function LoadingSpinner({ label = 'Loading...' }: { label?: string }) {
  return (
    <div className="flex items-center justify-center gap-3 text-white/50 font-mono tracking-widest">
      <div className="w-4 h-4 rounded-full border border-white/20 border-t-white/70 animate-spin" />
      <span className="text-xs">{label}</span>
    </div>
  );
}

