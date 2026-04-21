// StartupOverlay.tsx — Full-screen modal shown while the AutoDev fleet boots.
//
// Driven entirely by `useStartupStore`, which subscribes to the Rust-side
// `startup:progress` Tauri event. Each phase reported by the backend is
// rendered as a row with one of three states (pending / active / complete).
//
// The overlay self-dismisses ~600ms after reaching the "ready" phase (see
// startupStore.subscribe). Users cannot dismiss it manually while the fleet
// is still booting — this is intentional: invoking a Worker before core-orch
// is up would surface a confusing connectivity error.
//
// See: autodev/docs/development/startup_sequence_and_agent_persistence_analysis.md

import { useEffect } from "react";
import { Check, Loader2, Circle } from "lucide-react";
import { useStartupStore } from "../../stores/startupStore";
import "./StartupOverlay.css";

export default function StartupOverlay() {
  const phases         = useStartupStore((s) => s.phases);
  const currentIndex   = useStartupStore((s) => s.currentIndex);
  const currentMessage = useStartupStore((s) => s.currentMessage);
  const isComplete     = useStartupStore((s) => s.isComplete);
  const dismissed      = useStartupStore((s) => s.dismissed);
  const subscribe      = useStartupStore((s) => s.subscribe);

  // Subscribe once on mount. If the overlay has already been dismissed (e.g.
  // the user re-opened dev tools and hot-reloaded the bundle), we still want
  // to track further events — but they won't re-show the overlay because
  // `dismissed` latches true.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    subscribe().then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [subscribe]);

  if (dismissed) return null;

  const total = phases.length;
  const percent =
    currentIndex < 0 ? 0 : Math.round(((currentIndex + 1) / total) * 100);

  return (
    <div className="startup-overlay" role="dialog" aria-live="polite" aria-label="Starting AutoDev">
      <div className="startup-card">
        <div className="startup-header">
          <div className="startup-title">Starting AutoDev</div>
          <div className="startup-subtitle">
            {currentMessage ??
              (currentIndex >= 0 ? phases[currentIndex]?.label : "Initializing…")}
          </div>
        </div>

        <div
          className="startup-progress-track"
          role="progressbar"
          aria-label="AutoDev startup progress"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
        >
          <div
            className={`startup-progress-fill ${isComplete ? "is-complete" : ""}`}
            style={{ width: `${percent}%` }}
          />
        </div>

        <ul className="startup-phases">
          {phases.map((phase) => (
            <li key={phase.id} className={`startup-phase is-${phase.status}`}>
              <span className="startup-phase-icon" aria-hidden="true">
                {phase.status === "complete" && <Check size={14} />}
                {phase.status === "active"   && <Loader2 size={14} className="spin" />}
                {phase.status === "pending"  && <Circle size={8} />}
              </span>
              <span className="startup-phase-label">{phase.label}</span>
            </li>
          ))}
        </ul>

        <div className="startup-footer">
          <span className="startup-footer-percent">{percent}%</span>
          <span className="startup-footer-hint">
            Sidecars take ~15s to warm up on first launch
          </span>
        </div>
      </div>
    </div>
  );
}
