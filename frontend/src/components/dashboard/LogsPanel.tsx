import React, { useState, useEffect, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { getLogs } from "../../api";
import { useWebSocket } from "../../hooks/useWebSocket";
import Button from "../common/Button";
import LoadingSpinner from "../common/LoadingSpinner";

type LogLevel = "all" | "error" | "warn" | "info" | "debug";
type ConnectionMode = "websocket" | "polling";

const MAX_LOG_LINES = 2000;

const LogsPanel: React.FC = () => {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [autoScroll, setAutoScroll] = useState(true);
  const [filter, setFilter] = useState<LogLevel>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [connectionMode, setConnectionMode] = useState<ConnectionMode>("websocket");
  const logContainerRef = useRef<HTMLDivElement>(null);

  // Get auth token
  const token = localStorage.getItem("authToken") || "";

  // Build WebSocket URL
  const wsProtocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const wsUrl = `${wsProtocol}//${window.location.host}/api/ws/logs?token=${encodeURIComponent(token)}&initial_lines=500`;

  // WebSocket message handler
  const handleMessage = useCallback((message: { type: string; line?: string }) => {
    if (message.type === "log" && message.line) {
      setLogs((prev) => {
        const newLogs = [...prev, message.line!];
        // Keep only the last MAX_LOG_LINES
        if (newLogs.length > MAX_LOG_LINES) {
          return newLogs.slice(-MAX_LOG_LINES);
        }
        return newLogs;
      });
    } else if (message.type === "init_complete") {
      setLoading(false);
    }
  }, []);

  const { status, connect, disconnect, isConnected } = useWebSocket({
    url: wsUrl,
    onMessage: handleMessage,
    onOpen: () => {
      setLogs([]);
      setLoading(true);
    },
    onClose: () => {
      // If WebSocket fails, fall back to polling
      if (connectionMode === "websocket" && logs.length === 0) {
        setConnectionMode("polling");
      }
    },
  });

  // WebSocket connection effect
  useEffect(() => {
    if (connectionMode === "websocket" && token) {
      connect();
      return () => disconnect();
    }
  }, [connectionMode, token, connect, disconnect]);

  // Polling fallback
  const fetchLogs = useCallback(async () => {
    try {
      const data = await getLogs(2000);
      setLogs(data.logs);
    } catch (error) {
      console.error("Failed to fetch logs:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (connectionMode === "polling") {
      fetchLogs();
      const interval = setInterval(fetchLogs, 5000);
      return () => clearInterval(interval);
    }
  }, [connectionMode, fetchLogs]);

  // Auto-scroll effect
  useEffect(() => {
    if (autoScroll && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  const getLogLevel = (line: string): LogLevel => {
    if (line.includes(" ERROR ") || line.includes("[ERROR]")) return "error";
    if (line.includes(" WARN ") || line.includes("[WARN]")) return "warn";
    if (line.includes(" INFO ") || line.includes("[INFO]")) return "info";
    if (line.includes(" DEBUG ") || line.includes("[DEBUG]")) return "debug";
    return "info";
  };

  const getLogLevelStyle = (level: LogLevel): string => {
    switch (level) {
      case "error":
        return "bg-red-900/30 text-red-400 border-l-4 border-red-500";
      case "warn":
        return "bg-amber-900/30 text-amber-400 border-l-4 border-amber-500";
      case "info":
        return "bg-blue-900/30 text-blue-400 border-l-4 border-blue-500";
      case "debug":
        return "bg-gray-900/30 text-gray-400 border-l-4 border-gray-500";
      default:
        return "text-gray-300";
    }
  };

  const filteredLogs = logs.filter((line) => {
    const level = getLogLevel(line);
    const matchesFilter = filter === "all" || level === filter;
    const matchesSearch =
      !searchQuery || line.toLowerCase().includes(searchQuery.toLowerCase());
    return matchesFilter && matchesSearch;
  });

  const levelCounts = {
    error: logs.filter((l) => getLogLevel(l) === "error").length,
    warn: logs.filter((l) => getLogLevel(l) === "warn").length,
    info: logs.filter((l) => getLogLevel(l) === "info").length,
    debug: logs.filter((l) => getLogLevel(l) === "debug").length,
  };

  const getStatusBadge = () => {
    if (connectionMode === "polling") {
      return (
        <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-amber-900/50 text-amber-400">
          <span className="w-2 h-2 rounded-full bg-amber-400" />
          Polling
        </span>
      );
    }

    switch (status) {
      case "connected":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-green-900/50 text-green-400">
            <span className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
            Live
          </span>
        );
      case "connecting":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-blue-900/50 text-blue-400">
            <span className="w-2 h-2 rounded-full bg-blue-400 animate-pulse" />
            Connecting...
          </span>
        );
      case "error":
      case "disconnected":
        return (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-red-900/50 text-red-400">
            <span className="w-2 h-2 rounded-full bg-red-400" />
            Disconnected
          </span>
        );
    }
  };

  return (
    <div className="h-full flex flex-col bg-gray-800/50 rounded-xl border border-gray-700 overflow-hidden">
      {/* Header */}
      <div className="flex-shrink-0 p-4 border-b border-gray-700">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-medium text-white flex items-center gap-2">
            <svg className="w-5 h-5 text-cyan-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
            </svg>
            {t("logs.title")}
            {getStatusBadge()}
          </h2>
          <div className="flex items-center gap-2">
            <span className="text-xs text-gray-500">
              {t("logs.totalLines", { count: filteredLogs.length })}
            </span>
            <Button
              onClick={() => {
                if (logContainerRef.current) {
                  logContainerRef.current.scrollTop = 0;
                  setAutoScroll(false);
                }
              }}
              variant="secondary"
              className="text-xs px-2 py-1"
            >
              {t("logs.scrollToTop")}
            </Button>
            <Button
              onClick={() => {
                if (logContainerRef.current) {
                  logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
                  setAutoScroll(true);
                }
              }}
              variant="secondary"
              className="text-xs px-2 py-1"
            >
              {t("logs.scrollToBottom")}
            </Button>
            {connectionMode === "websocket" && !isConnected && (
              <Button
                onClick={connect}
                variant="secondary"
                className="text-xs px-2 py-1"
              >
                Reconnect
              </Button>
            )}
          </div>
        </div>

        {/* Filters */}
        <div className="flex flex-wrap items-center gap-3">
          {/* Search */}
          <div className="flex-1 min-w-48">
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t("dashboard.searchLogs")}
              className="w-full bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-cyan-500"
            />
          </div>

          {/* Level filters */}
          <div className="flex gap-1">
            {(["all", "error", "warn", "info", "debug"] as LogLevel[]).map((level) => {
              const count = level === "all" ? logs.length : levelCounts[level];
              const isActive = filter === level;
              const colorMap: Record<LogLevel, string> = {
                all: isActive ? "bg-gray-600" : "bg-gray-700 hover:bg-gray-600",
                error: isActive ? "bg-red-600" : "bg-gray-700 hover:bg-red-900/50",
                warn: isActive ? "bg-amber-600" : "bg-gray-700 hover:bg-amber-900/50",
                info: isActive ? "bg-blue-600" : "bg-gray-700 hover:bg-blue-900/50",
                debug: isActive ? "bg-gray-500" : "bg-gray-700 hover:bg-gray-600",
              };

              return (
                <button
                  key={level}
                  onClick={() => setFilter(level)}
                  className={`px-2 py-1 rounded text-xs font-medium transition-colors ${colorMap[level]} ${
                    isActive ? "text-white" : "text-gray-300"
                  }`}
                >
                  {level.toUpperCase()}
                  {count > 0 && (
                    <span className="ml-1 opacity-75">({count})</span>
                  )}
                </button>
              );
            })}
          </div>

          {/* Auto-scroll toggle */}
          <label className="flex items-center gap-2 text-xs text-gray-400 cursor-pointer">
            <input
              type="checkbox"
              checked={autoScroll}
              onChange={(e) => setAutoScroll(e.target.checked)}
              className="rounded border-gray-600 bg-gray-700 text-cyan-500 focus:ring-cyan-500 w-3.5 h-3.5"
            />
            {t("logs.autoScroll")}
          </label>
        </div>
      </div>

      {/* Logs content */}
      <div
        ref={logContainerRef}
        className="flex-1 overflow-y-auto p-4 font-mono text-xs leading-relaxed"
        onScroll={(e) => {
          const target = e.target as HTMLDivElement;
          const isAtBottom =
            target.scrollHeight - target.scrollTop <= target.clientHeight + 50;
          setAutoScroll(isAtBottom);
        }}
      >
        {loading ? (
          <div className="flex items-center justify-center py-12">
            <LoadingSpinner />
            <span className="ml-2 text-gray-400">{t("common.loading")}</span>
          </div>
        ) : filteredLogs.length === 0 ? (
          <div className="text-gray-500 text-center py-8">
            {searchQuery || filter !== "all"
              ? t("dashboard.noMatchingLogs")
              : t("logs.noLogs")}
          </div>
        ) : (
          <div className="space-y-0.5">
            {filteredLogs.map((line, index) => {
              const level = getLogLevel(line);
              return (
                <div
                  key={index}
                  className={`px-2 py-1 rounded whitespace-pre-wrap break-all hover:brightness-110 transition-all ${getLogLevelStyle(level)}`}
                >
                  {line}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

export default LogsPanel;
