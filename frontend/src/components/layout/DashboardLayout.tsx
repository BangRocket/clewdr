import React, { ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import Sidebar from "./Sidebar";
import LanguageSelector from "./LanguageSelector";
import ToastProvider from "../common/ToastProvider";

interface DashboardLayoutProps {
  children: ReactNode;
  version: string;
  activeSection: string;
  onSectionChange: (section: string) => void;
}

const DashboardLayout: React.FC<DashboardLayoutProps> = ({
  children,
  version,
  activeSection,
  onSectionChange,
}) => {
  const { t } = useTranslation();
  const [sidebarOpen, setSidebarOpen] = useState(false);

  return (
    <div className="min-h-screen bg-gradient-to-b from-gray-900 to-gray-800 text-white">
      <ToastProvider />

      <div className="flex h-screen overflow-hidden">
        {/* Sidebar */}
        <Sidebar
          activeSection={activeSection}
          onSectionChange={onSectionChange}
          isOpen={sidebarOpen}
          onClose={() => setSidebarOpen(false)}
        />

        {/* Main content area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Top bar (mobile) */}
          <header className="lg:hidden flex items-center justify-between p-4 border-b border-gray-700 bg-gray-800/95 backdrop-blur">
            <button
              onClick={() => setSidebarOpen(true)}
              className="p-2 text-gray-400 hover:text-white"
            >
              <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
              </svg>
            </button>
            <h1 className="text-lg font-bold text-transparent bg-clip-text bg-gradient-to-r from-cyan-400 to-purple-500">
              {t("app.title")}
            </h1>
            <LanguageSelector />
          </header>

          {/* Desktop top bar */}
          <header className="hidden lg:flex items-center justify-between p-4 border-b border-gray-700">
            <div className="flex items-center gap-4">
              <span className="text-sm font-mono text-gray-400">{version}</span>
            </div>
            <LanguageSelector />
          </header>

          {/* Main content */}
          <main className="flex-1 overflow-auto p-4 lg:p-6">
            {children}
          </main>

          {/* Footer */}
          <footer className="p-4 border-t border-gray-700 text-center text-sm text-gray-500">
            <span>{t("app.footer", { year: new Date().getFullYear() })}</span>
            <span className="mx-2">|</span>
            <a
              href="https://ko-fi.com/xerxes2"
              target="_blank"
              rel="noopener noreferrer"
              className="text-cyan-400 hover:text-cyan-300 transition-colors"
            >
              {t("app.buyMeCoffee")}
            </a>
          </footer>
        </div>
      </div>
    </div>
  );
};

export default DashboardLayout;
