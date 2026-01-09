import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { postCookie, getBrowserCookie, getCookieStatus } from "../../api";
import Button from "../common/Button";
import LoadingSpinner from "../common/LoadingSpinner";

interface QuickActionsProps {
  onCookieSubmitted?: () => void;
  onRefresh?: () => void;
}

const QuickActions: React.FC<QuickActionsProps> = ({
  onCookieSubmitted,
  onRefresh,
}) => {
  const { t } = useTranslation();
  const [isImporting, setIsImporting] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [showCookieInput, setShowCookieInput] = useState(false);
  const [cookieValue, setCookieValue] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);

  const handleImportFromBrowser = async () => {
    setIsImporting(true);
    setMessage(null);

    try {
      const response = await getBrowserCookie();

      if (response.found && response.cookie) {
        // Auto-submit the cookie
        await postCookie(response.cookie);
        setMessage({
          type: "success",
          text: t("cookieSubmit.browserImport.success", {
            browser: response.browser,
            profile: response.profile,
          }),
        });
        onCookieSubmitted?.();
      } else {
        setMessage({
          type: "error",
          text: response.message || t("cookieSubmit.browserImport.notFound"),
        });
      }
    } catch (e) {
      setMessage({
        type: "error",
        text: t("cookieSubmit.browserImport.error", {
          message: e instanceof Error ? e.message : "Unknown error",
        }),
      });
    } finally {
      setIsImporting(false);
    }
  };

  const handleRefresh = async () => {
    setIsRefreshing(true);
    try {
      await getCookieStatus(true);
      onRefresh?.();
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleSubmitCookie = async () => {
    if (!cookieValue.trim()) return;

    setIsSubmitting(true);
    setMessage(null);

    try {
      await postCookie(cookieValue.trim());
      setMessage({ type: "success", text: t("cookieSubmit.success") });
      setCookieValue("");
      setShowCookieInput(false);
      onCookieSubmitted?.();
    } catch (e) {
      setMessage({
        type: "error",
        text: e instanceof Error ? e.message : "Failed to submit cookie",
      });
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider">
        {t("dashboard.quickActions")}
      </h3>

      <div className="space-y-2">
        {/* Import from Browser */}
        <Button
          onClick={handleImportFromBrowser}
          disabled={isImporting}
          variant="secondary"
          className="w-full justify-start text-left"
        >
          {isImporting ? (
            <>
              <LoadingSpinner />
              <span className="ml-2">{t("cookieSubmit.browserImport.importing")}</span>
            </>
          ) : (
            <>
              <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9" />
              </svg>
              {t("cookieSubmit.browserImport.button")}
            </>
          )}
        </Button>

        {/* Submit Cookie Toggle */}
        {!showCookieInput ? (
          <Button
            onClick={() => setShowCookieInput(true)}
            variant="secondary"
            className="w-full justify-start text-left"
          >
            <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
            {t("dashboard.submitCookie")}
          </Button>
        ) : (
          <div className="bg-gray-700/50 rounded-lg p-3 space-y-2">
            <textarea
              value={cookieValue}
              onChange={(e) => setCookieValue(e.target.value)}
              placeholder={t("cookieSubmit.placeholder")}
              className="w-full h-20 bg-gray-800 border border-gray-600 rounded px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-cyan-500 resize-none"
            />
            <div className="flex gap-2">
              <Button
                onClick={handleSubmitCookie}
                disabled={isSubmitting || !cookieValue.trim()}
                isLoading={isSubmitting}
                className="flex-1 text-sm"
              >
                {t("cookieSubmit.submitButton")}
              </Button>
              <Button
                onClick={() => {
                  setShowCookieInput(false);
                  setCookieValue("");
                }}
                variant="secondary"
                className="text-sm"
              >
                {t("auth.clear")}
              </Button>
            </div>
          </div>
        )}

        {/* Refresh Status */}
        <Button
          onClick={handleRefresh}
          disabled={isRefreshing}
          variant="secondary"
          className="w-full justify-start text-left"
        >
          {isRefreshing ? (
            <>
              <LoadingSpinner />
              <span className="ml-2">{t("cookieStatus.refreshing")}</span>
            </>
          ) : (
            <>
              <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
              {t("cookieStatus.refresh")}
            </>
          )}
        </Button>
      </div>

      {/* Status message */}
      {message && (
        <div
          className={`text-sm p-2 rounded ${
            message.type === "success"
              ? "bg-green-900/30 text-green-400 border border-green-800"
              : "bg-red-900/30 text-red-400 border border-red-800"
          }`}
        >
          {message.text}
        </div>
      )}
    </div>
  );
};

export default QuickActions;
