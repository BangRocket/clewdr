import React from "react";
import { useTranslation } from "react-i18next";

interface TokenInfo {
  expires_at: string;
  organization: {
    uuid: string;
  };
}

interface Cookie {
  cookie: string;
  token?: TokenInfo;
}

interface OAuthStatusProps {
  cookies: Cookie[];
  isLoading?: boolean;
}

const OAuthStatus: React.FC<OAuthStatusProps> = ({ cookies, isLoading }) => {
  const { t } = useTranslation();

  if (isLoading) {
    return (
      <div className="animate-pulse space-y-2">
        <div className="h-4 w-32 bg-gray-600 rounded" />
        <div className="h-3 w-24 bg-gray-600 rounded" />
      </div>
    );
  }

  const tokensCount = cookies.filter((c) => c.token).length;
  const expiredCount = cookies.filter((c) => {
    if (!c.token) return false;
    const expiresAt = new Date(c.token.expires_at);
    return expiresAt < new Date();
  }).length;
  const validCount = tokensCount - expiredCount;

  const getStatusColor = () => {
    if (tokensCount === 0) return "text-gray-400";
    if (expiredCount === tokensCount) return "text-red-400";
    if (expiredCount > 0) return "text-amber-400";
    return "text-green-400";
  };

  const getStatusIcon = () => {
    if (tokensCount === 0) {
      return (
        <svg className="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
        </svg>
      );
    }
    if (expiredCount === tokensCount) {
      return (
        <svg className="w-5 h-5 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
      );
    }
    return (
      <svg className="w-5 h-5 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
      </svg>
    );
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        {getStatusIcon()}
        <span className={`text-sm font-medium ${getStatusColor()}`}>
          {tokensCount === 0
            ? t("dashboard.oauth.noTokens")
            : expiredCount === tokensCount
            ? t("dashboard.oauth.allExpired")
            : expiredCount > 0
            ? t("dashboard.oauth.someExpired", { valid: validCount, expired: expiredCount })
            : t("dashboard.oauth.allValid", { count: validCount })}
        </span>
      </div>

      {tokensCount > 0 && (
        <div className="text-xs text-gray-500">
          {t("dashboard.oauth.totalTokens", { count: tokensCount })}
        </div>
      )}

      {/* Token details */}
      {cookies.filter((c) => c.token).slice(0, 3).map((cookie, index) => {
        if (!cookie.token) return null;
        const expiresAt = new Date(cookie.token.expires_at);
        const isExpired = expiresAt < new Date();
        const timeUntilExpiry = expiresAt.getTime() - Date.now();
        const hoursUntilExpiry = Math.floor(timeUntilExpiry / (1000 * 60 * 60));
        const minutesUntilExpiry = Math.floor((timeUntilExpiry % (1000 * 60 * 60)) / (1000 * 60));

        return (
          <div
            key={index}
            className={`text-xs p-2 rounded border ${
              isExpired
                ? "bg-red-900/20 border-red-800 text-red-400"
                : "bg-green-900/20 border-green-800 text-green-400"
            }`}
          >
            <div className="flex items-center justify-between">
              <span className="font-mono truncate max-w-[100px]">
                {cookie.cookie.substring(0, 10)}...
              </span>
              <span>
                {isExpired
                  ? t("dashboard.oauth.expired")
                  : hoursUntilExpiry > 0
                  ? t("dashboard.oauth.expiresIn", { hours: hoursUntilExpiry, minutes: minutesUntilExpiry })
                  : t("dashboard.oauth.expiresMinutes", { minutes: minutesUntilExpiry })}
              </span>
            </div>
          </div>
        );
      })}

      {tokensCount > 3 && (
        <div className="text-xs text-gray-500 text-center">
          {t("dashboard.oauth.andMore", { count: tokensCount - 3 })}
        </div>
      )}
    </div>
  );
};

export default OAuthStatus;
