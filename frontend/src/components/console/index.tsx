import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { getLogs } from "../../api";
import Button from "../common/Button";
import LoadingSpinner from "../common/LoadingSpinner";
import StatusMessage from "../common/StatusMessage";

interface LogsResponse {
  logs: string[];
  total: number;
  message?: string;
}

export default function LogsTab() {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const logContainerRef = useRef<HTMLDivElement>(null);

  const fetchLogs = async () => {
    try {
      setLoading(true);
      setError(null);
      const data: LogsResponse = await getLogs(2000);
      setLogs(data.logs);
      if (data.message) {
        setMessage(data.message);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to fetch logs");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchLogs();
  }, []);

  useEffect(() => {
    if (autoScroll && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  const scrollToTop = () => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = 0;
      setAutoScroll(false);
    }
  };

  const scrollToBottom = () => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
      setAutoScroll(true);
    }
  };

  const getLogLevelColor = (line: string): string => {
    if (line.includes(" ERROR ") || line.includes("[ERROR]")) {
      return "text-red-400";
    }
    if (line.includes(" WARN ") || line.includes("[WARN]")) {
      return "text-yellow-400";
    }
    if (line.includes(" INFO ") || line.includes("[INFO]")) {
      return "text-blue-400";
    }
    if (line.includes(" DEBUG ") || line.includes("[DEBUG]")) {
      return "text-gray-500";
    }
    return "text-gray-300";
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <LoadingSpinner />
        <span className="ml-2 text-gray-400">{t("common.loading")}</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-4">
        <StatusMessage type="error" message={error} />
        <Button onClick={fetchLogs} variant="secondary">
          {t("config.retry")}
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-medium text-white">{t("logs.title")}</h3>
        <div className="flex gap-2">
          <Button onClick={scrollToTop} variant="secondary" className="text-xs px-2 py-1">
            {t("logs.scrollToTop")}
          </Button>
          <Button onClick={scrollToBottom} variant="secondary" className="text-xs px-2 py-1">
            {t("logs.scrollToBottom")}
          </Button>
          <Button onClick={fetchLogs} variant="secondary" className="text-xs px-2 py-1">
            {t("cookieStatus.refresh")}
          </Button>
        </div>
      </div>

      {message && (
        <StatusMessage type="info" message={message} />
      )}

      <div className="text-sm text-gray-400">
        {t("logs.totalLines", { count: logs.length })}
      </div>

      <div
        ref={logContainerRef}
        className="bg-gray-900 rounded-lg p-4 h-96 overflow-y-auto font-mono text-xs leading-relaxed border border-gray-700"
        onScroll={(e) => {
          const target = e.target as HTMLDivElement;
          const isAtBottom = target.scrollHeight - target.scrollTop <= target.clientHeight + 50;
          setAutoScroll(isAtBottom);
        }}
      >
        {logs.length === 0 ? (
          <div className="text-gray-500 text-center py-8">
            {t("logs.noLogs")}
          </div>
        ) : (
          logs.map((line, index) => (
            <div
              key={index}
              className={`whitespace-pre-wrap break-all ${getLogLevelColor(line)}`}
            >
              {line}
            </div>
          ))
        )}
      </div>

      <div className="flex items-center gap-2 text-sm">
        <label className="flex items-center gap-2 text-gray-400 cursor-pointer">
          <input
            type="checkbox"
            checked={autoScroll}
            onChange={(e) => setAutoScroll(e.target.checked)}
            className="rounded border-gray-600 bg-gray-700 text-cyan-500 focus:ring-cyan-500"
          />
          {t("logs.autoScroll")}
        </label>
      </div>
    </div>
  );
}
