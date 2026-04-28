import React, { useEffect, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "react-hot-toast";
import { deleteCodexAuth, listCodexAuth } from "../../api";
import type {
  CodexAuthSummary,
  CodexAuthStatus,
} from "../../types/codex.types";
import Button from "../common/Button";
import LoadingSpinner from "../common/LoadingSpinner";
import StatusMessage from "../common/StatusMessage";

const statusBadge = (
  s: CodexAuthStatus,
): { color: string; label: string } => {
  switch (s.kind) {
    case "valid":
      return { color: "bg-green-700", label: "valid" };
    case "rate_limited":
      return {
        color: "bg-yellow-700",
        label: `rate-limited until ${new Date(
          s.until * 1000,
        ).toLocaleTimeString()}`,
      };
    case "expired":
      return { color: "bg-orange-700", label: "expired" };
    case "invalid":
      return { color: "bg-red-800", label: "invalid (re-login)" };
    case "banned":
      return { color: "bg-red-900", label: "banned" };
  }
};

const formatLastUsed = (ts: number | null): string => {
  if (!ts) return "—";
  return new Date(ts * 1000).toLocaleString();
};

const CodexAuthList: React.FC = () => {
  const { t } = useTranslation();
  const [items, setItems] = useState<CodexAuthSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const list = await listCodexAuth();
      setItems(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleDelete = async (id: string) => {
    if (!confirm(t("codexList.deleteConfirm"))) return;
    setDeletingId(id);
    try {
      await deleteCodexAuth(id);
      toast.success(t("codexList.deleted"));
      await refresh();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setDeletingId(null);
    }
  };

  if (items === null && !error) return <LoadingSpinner />;
  if (error) return <StatusMessage type="error" message={error} />;
  if (items && items.length === 0) {
    return <StatusMessage type="info" message={t("codexList.empty")} />;
  }

  return (
    <div className="space-y-3">
      {items!.map((item) => {
        const badge = statusBadge(item.status);
        return (
          <div
            key={item.id}
            className="bg-gray-800 border border-gray-700 rounded-md p-4 flex items-center justify-between gap-4"
          >
            <div className="flex-1 min-w-0">
              <div className="flex items-center flex-wrap gap-2 mb-1">
                <span className="font-medium text-gray-100 truncate">
                  {item.label || item.id.slice(0, 8)}
                </span>
                <span
                  className={`px-2 py-0.5 text-xs rounded ${badge.color} text-white`}
                >
                  {badge.label}
                </span>
                {item.plan && (
                  <span className="px-2 py-0.5 text-xs rounded bg-gray-700 text-gray-200">
                    {item.plan}
                  </span>
                )}
              </div>
              <div className="text-xs text-gray-400 flex flex-wrap gap-x-3 gap-y-1">
                <span>
                  id: <code>{item.id.slice(0, 12)}</code>
                </span>
                <span>
                  account: <code>{item.account_id_prefix}…</code>
                </span>
                <span>last used: {formatLastUsed(item.last_used_at)}</span>
              </div>
            </div>
            <Button
              type="button"
              variant="danger"
              disabled={deletingId === item.id}
              isLoading={deletingId === item.id}
              onClick={() => handleDelete(item.id)}
            >
              {t("codexList.delete")}
            </Button>
          </div>
        );
      })}
    </div>
  );
};

export default CodexAuthList;
