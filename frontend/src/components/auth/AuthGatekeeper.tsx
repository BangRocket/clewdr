// frontend/src/components/auth/AuthGatekeeper.tsx
import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import Button from "../common/Button";
import FormInput from "../common/FormInput";
import StatusMessage from "../common/StatusMessage";
import { useAuth } from "../../hooks/useAuth";
import { getOAuthProviders, type OAuthProviders } from "../../api";

// GitHub and Google SVG icons
const GitHubIcon = () => (
  <svg className="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
    <path d="M12 0C5.374 0 0 5.373 0 12c0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23A11.509 11.509 0 0112 5.803c1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576C20.566 21.797 24 17.3 24 12c0-6.627-5.373-12-12-12z"/>
  </svg>
);

const GoogleIcon = () => (
  <svg className="w-5 h-5" viewBox="0 0 24 24">
    <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
    <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
    <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
    <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
  </svg>
);

interface AuthGatekeeperProps {
  onAuthenticated?: (status: boolean) => void;
}

const AuthGatekeeper: React.FC<AuthGatekeeperProps> = ({ onAuthenticated }) => {
  const { t } = useTranslation();
  const {
    authToken,
    setAuthToken,
    isLoading,
    error,
    savedToken,
    login,
    logout,
  } = useAuth(onAuthenticated);

  const [statusMessage, setStatusMessage] = useState({
    type: "info" as "success" | "error" | "warning" | "info",
    message: "",
  });
  const [oauthProviders, setOAuthProviders] = useState<OAuthProviders | null>(null);
  const [oauthLoading, setOAuthLoading] = useState(false);

  // Fetch OAuth providers on mount
  useEffect(() => {
    getOAuthProviders()
      .then(setOAuthProviders)
      .catch(() => setOAuthProviders({ github: false, google: false }));
  }, []);

  // Check for OAuth callback token in URL
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const oauthToken = params.get("oauth_token");
    const oauthError = params.get("oauth_error");

    if (oauthToken) {
      // Store OAuth token as auth token
      localStorage.setItem("authToken", oauthToken);
      localStorage.setItem("oauthSession", "true");
      // Clear URL params
      window.history.replaceState({}, "", window.location.pathname);
      // Trigger auth check
      onAuthenticated?.(true);
      setStatusMessage({
        type: "success",
        message: t("auth.oauthSuccess"),
      });
    } else if (oauthError) {
      // Clear URL params
      window.history.replaceState({}, "", window.location.pathname);
      setStatusMessage({
        type: "error",
        message: t("auth.oauthError", { error: oauthError }),
      });
    }
  }, [onAuthenticated, t]);

  // Update status message when error changes
  useEffect(() => {
    if (error) {
      setStatusMessage({
        type: "error",
        message: error,
      });
    }
  }, [error]);

  // Show persistent success message after login
  // (we don't clear it automatically)
  const handleLoginSuccess = () => {
    setStatusMessage({
      type: "success",
      message: t("auth.success"),
    });
  };

  const handleSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setStatusMessage({ type: "info", message: "" });

    if (!authToken.trim()) {
      setStatusMessage({
        type: "warning",
        message: t("auth.enterToken"),
      });
      return;
    }

    try {
      await login(authToken);
      handleLoginSuccess();
    } catch {
      // Error is already handled in the useAuth hook
      // and will be displayed via the useEffect
    }
  };

  const handleClearToken = () => {
    logout();
    setStatusMessage({
      type: "info",
      message: t("auth.tokenCleared"),
    });
  };

  const handleOAuthLogin = (provider: "github" | "google") => {
    setOAuthLoading(true);
    // Redirect to OAuth login endpoint
    window.location.href = `/api/auth/oauth/${provider}`;
  };

  const hasOAuthProviders = oauthProviders && (oauthProviders.github || oauthProviders.google);

  return (
    <div>
      {/* OAuth Login Buttons */}
      {hasOAuthProviders && (
        <div className="mb-6 space-y-3">
          <p className="text-sm text-gray-400 text-center mb-4">
            {t("auth.oauthDescription")}
          </p>
          {oauthProviders.github && (
            <button
              type="button"
              onClick={() => handleOAuthLogin("github")}
              disabled={oauthLoading}
              className="w-full flex items-center justify-center gap-3 px-4 py-3 bg-gray-700 hover:bg-gray-600 text-white rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <GitHubIcon />
              <span>{t("auth.loginWithGitHub")}</span>
            </button>
          )}
          {oauthProviders.google && (
            <button
              type="button"
              onClick={() => handleOAuthLogin("google")}
              disabled={oauthLoading}
              className="w-full flex items-center justify-center gap-3 px-4 py-3 bg-white hover:bg-gray-100 text-gray-800 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <GoogleIcon />
              <span>{t("auth.loginWithGoogle")}</span>
            </button>
          )}
          <div className="relative my-6">
            <div className="absolute inset-0 flex items-center">
              <div className="w-full border-t border-gray-600"></div>
            </div>
            <div className="relative flex justify-center text-sm">
              <span className="px-2 bg-gray-800 text-gray-400">{t("auth.or")}</span>
            </div>
          </div>
        </div>
      )}

      {/* Token Auth Form */}
      <form onSubmit={handleSubmit} className="space-y-6">
        <FormInput
          id="authToken"
          name="authToken"
          type="password"
          value={authToken}
          onChange={(e) => setAuthToken(e.target.value)}
          label={t("auth.token")}
          placeholder={t("auth.tokenPlaceholder")}
          disabled={isLoading}
          onClear={() => setAuthToken("")}
        />

        {savedToken && (
          <div className="flex items-center justify-between mt-2">
            <p className="text-xs text-gray-400">
              {t("auth.previousToken")}{" "}
              <span className="font-mono">{savedToken}</span>
            </p>
            <button
              type="button"
              onClick={handleClearToken}
              className="text-xs text-red-400 hover:text-red-300"
              disabled={isLoading}
            >
              {t("auth.clear")}
            </button>
          </div>
        )}

        {statusMessage.message && (
          <StatusMessage
            type={statusMessage.type}
            message={statusMessage.message}
          />
        )}

        <Button
          type="submit"
          isLoading={isLoading}
          disabled={isLoading}
          className="w-full"
          variant="primary"
        >
          {isLoading ? t("auth.verifying") : t("auth.submitButton")}
        </Button>
      </form>
    </div>
  );
};

export default AuthGatekeeper;
