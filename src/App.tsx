import { useState, useEffect, useCallback, type CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, AlertCircle, Gauge } from "lucide-react";
import { PROVIDER_ICONS } from "./ProviderIcons";
import "./App.css";

// Types matching the Rust backend
interface MetricFormat {
  kind: "percent" | "dollars" | "count";
  suffix?: string;
}

interface ProgressLine {
  type: "progress";
  label: string;
  used: number;
  limit: number;
  format: MetricFormat;
  resetsAt?: string | null;
}

interface TextLine {
  type: "text";
  label: string;
  value: string;
}

interface BadgeLine {
  type: "badge";
  label: string;
  text: string;
  color?: string;
}

type MetricLine = ProgressLine | TextLine | BadgeLine;

interface ProviderResult {
  id: string;
  name: string;
  icon: string;
  brandColor: string;
  plan?: string | null;
  lines: MetricLine[];
  error?: string | null;
}

// Provider brand colors
const PROVIDER_STYLES: Record<string, { bg: string }> = {
  cursor: { bg: "#000000" },
  claude: { bg: "#D97757" },
  copilot: { bg: "#000000" },
  codex: { bg: "#000000" },
  windsurf: { bg: "#00B4D8" },
};

function formatValue(used: number, limit: number, format: MetricFormat): string {
  switch (format.kind) {
    case "percent":
      return `${Math.round(used)}%`;
    case "dollars":
      return `$${used.toFixed(2)} / $${limit.toFixed(2)}`;
    case "count":
      return `${Math.round(used)} / ${Math.round(limit)} ${format.suffix || ""}`;
    default:
      return `${used} / ${limit}`;
  }
}

function getProgressColor(pct: number): string {
  if (pct < 50) return "#22c55e";
  if (pct < 75) return "#f59e0b";
  if (pct < 90) return "#f97316";
  return "#ef4444";
}

function timeUntilReset(isoStr: string): string {
  const now = Date.now();
  const resetMs = new Date(isoStr).getTime();
  const diffMs = resetMs - now;
  if (diffMs <= 0) return "resetting...";
  const days = Math.floor(diffMs / 86400000);
  const hours = Math.floor((diffMs % 86400000) / 3600000);
  if (days > 0) return `Resets in ${days}d ${hours}h`;
  const mins = Math.floor((diffMs % 3600000) / 60000);
  if (hours > 0) return `Resets in ${hours}h ${mins}m`;
  return `Resets in ${mins}m`;
}

// Progress Bar Component
function ProgressMetric({ line }: { line: ProgressLine }) {
  const pct = line.limit > 0 ? Math.min((line.used / line.limit) * 100, 100) : 0;
  const color = getProgressColor(pct);

  return (
    <div className="progress-metric">
      <div className="progress-header">
        <span className="progress-label">{line.label}</span>
        <span className="progress-value">{formatValue(line.used, line.limit, line.format)}</span>
      </div>
      <div className="progress-track">
        <div
          className="progress-fill"
          style={{
            width: `${pct}%`,
            background: `linear-gradient(90deg, ${color}cc, ${color})`,
          }}
        />
      </div>
      {line.resetsAt && (
        <span className="progress-subtitle">{timeUntilReset(line.resetsAt)}</span>
      )}
    </div>
  );
}

// Badge Component
function BadgeMetric({ line }: { line: BadgeLine }) {
  const color = line.color || "#7da88c";
  return (
    <div className="badge-line">
      <span className="badge-label">{line.label}</span>
      <span className="badge-chip" style={{ color, borderColor: `${color}40` }}>
        {line.text}
      </span>
    </div>
  );
}

// Text Component
function TextMetric({ line }: { line: TextLine }) {
  return (
    <div className="text-metric">
      <span className="text-label">{line.label}</span>
      <span className="text-value">{line.value}</span>
    </div>
  );
}

// Provider Card Component
function ProviderCard({
  provider,
  onRefresh,
  isRefreshing,
}: {
  provider: ProviderResult;
  onRefresh: () => void;
  isRefreshing: boolean;
}) {
  const style = PROVIDER_STYLES[provider.id] || { bg: "#666" };
  const IconComponent = PROVIDER_ICONS[provider.id];
  const accent = provider.brandColor || style.bg;
  const providerStateLabel = provider.error
    ? "Connection needs attention"
    : isRefreshing
      ? "Refreshing usage..."
      : provider.lines.length > 0
        ? `${provider.lines.length} live signal${provider.lines.length === 1 ? "" : "s"}`
        : "Waiting for usage signals";
  const providerCardStyle = {
    "--provider-accent": accent,
    "--provider-accent-soft": `${accent}20`,
  } as CSSProperties;

  return (
    <div
      className={`provider-card ${provider.error ? "provider-card-error" : ""}`}
      style={providerCardStyle}
    >
      <div className="provider-card-header">
        <div className="provider-info">
          <div className="provider-icon" style={{ background: style.bg }}>
            {IconComponent ? <IconComponent /> : "?"}
          </div>
          <div className="provider-copy">
            <div className="provider-name-row">
              <div className="provider-name">{provider.name}</div>
              {provider.plan && <div className="provider-plan">{provider.plan}</div>}
            </div>
            <div className="provider-caption">{providerStateLabel}</div>
          </div>
        </div>
        <button
          className={`btn-icon provider-refresh ${isRefreshing ? "spinning" : ""}`}
          onClick={onRefresh}
          title="Refresh"
          aria-label={`Refresh ${provider.name}`}
        >
          <RefreshCw />
        </button>
      </div>

      {isRefreshing && (
        <div className="provider-loading">
          <div className="skeleton" style={{ width: "100%", marginBottom: 6 }} />
          <div className="skeleton" style={{ width: "60%" }} />
        </div>
      )}

      {!isRefreshing && provider.error && (
        <div className="provider-error">
          <div className="error-msg">
            <AlertCircle />
            <span>{provider.error}</span>
          </div>
        </div>
      )}

      {!isRefreshing && !provider.error && provider.lines.length > 0 && (
        <div className="metric-lines">
          {provider.lines.map((line, i) => {
            switch (line.type) {
              case "progress":
                return <ProgressMetric key={i} line={line} />;
              case "text":
                return <TextMetric key={i} line={line} />;
              case "badge":
                return <BadgeMetric key={i} line={line} />;
              default:
                return null;
            }
          })}
        </div>
      )}
    </div>
  );
}

// Main App
function App() {
  const [providers, setProviders] = useState<ProviderResult[]>([]);
  const [refreshing, setRefreshing] = useState<Set<string>>(new Set());
  const [lastRefresh, setLastRefresh] = useState<Date | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const connectedProviders = providers.filter((provider) => !provider.error && provider.lines.length > 0).length;
  const statusText = isLoading
    ? "Refreshing local usage"
    : providers.length === 0
      ? "Waiting for connected tools"
      : `${connectedProviders} of ${providers.length} providers reporting`;

  const refreshAll = useCallback(async () => {
    setIsLoading(true);
    try {
      const results = await invoke<ProviderResult[]>("probe_all");
      setProviders(results);
      setLastRefresh(new Date());
    } catch (e) {
      console.error("Failed to probe providers:", e);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const refreshSingle = useCallback(async (id: string) => {
    setRefreshing((prev) => new Set(prev).add(id));
    try {
      const result = await invoke<ProviderResult>("probe", { id });
      setProviders((prev) =>
        prev.map((p) => (p.id === id ? result : p))
      );
    } catch (e) {
      console.error(`Failed to probe ${id}:`, e);
    } finally {
      setRefreshing((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  }, []);

  useEffect(() => {
    refreshAll();

    // Auto-refresh every 15 minutes
    const interval = setInterval(refreshAll, 15 * 60 * 1000);
    return () => clearInterval(interval);
  }, [refreshAll]);

  return (
    <div className="app-shell">
      <div className="header">
        <div className="header-title">
          <div className="header-mark">
            <Gauge />
          </div>
          <div className="header-copy">
            <span className="header-product">UsageDock</span>
            <span className="header-subtitle">Quiet local usage signals for AI coding tools</span>
          </div>
        </div>
        <div className="header-actions">
          <div className={`header-status ${isLoading ? "header-status-live" : ""}`}>
            <span className="header-status-dot" />
            <span>{statusText}</span>
          </div>
          <button
            className={`btn-icon refresh-all ${isLoading ? "spinning" : ""}`}
            onClick={refreshAll}
            title="Refresh all"
            aria-label="Refresh all providers"
          >
            <RefreshCw />
          </button>
        </div>
      </div>

      <div className="provider-list">
        {providers.map((provider) => (
          <ProviderCard
            key={provider.id}
            provider={provider}
            onRefresh={() => refreshSingle(provider.id)}
            isRefreshing={refreshing.has(provider.id)}
          />
        ))}

        {!isLoading && providers.length === 0 && (
          <div className="empty-state">
            <Gauge />
            <p>No providers configured.<br />Sign into Cursor, Claude, Copilot, or Codex to get started.</p>
          </div>
        )}
      </div>

      <div className="footer">
        <span className="footer-text">
          {lastRefresh
            ? `Updated ${lastRefresh.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
            : "Loading provider activity"}
        </span>
        <span className="footer-text">Auto refresh every 15 min</span>
      </div>
    </div>
  );
}

export default App;
