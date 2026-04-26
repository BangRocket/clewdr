import React from "react";
import { useTranslation } from "react-i18next";
import LanguageSelector from "./LanguageSelector";

interface HeaderProps {
  version: string;
}

const Header: React.FC<HeaderProps> = ({ version }) => {
  const { t } = useTranslation();
  // Backend ships version as multi-line: first line is "v<x> by <authors>",
  // remaining lines are build metadata ("| profile: ...", "| mode: ...",
  // "| no_fs: ..."). On small viewports the long author/email string wraps
  // awkwardly and the metadata lines pile up, so show only the version+authors
  // line below md and reveal the full block on md+.
  const lines = version ? version.split("\n") : [];
  const headLine = lines[0] ?? version;
  const metaLines = lines.slice(1);

  return (
    <header className="mb-6 text-center sm:mb-10">
      <div className="flex justify-end mb-2">
        <LanguageSelector />
      </div>
      <h1 className="text-4xl font-bold mb-2 text-transparent bg-clip-text bg-gradient-to-r from-cyan-400 to-purple-500">
        {t("app.title")}
      </h1>
      <h2 className="text-xs font-mono text-gray-400 break-words sm:text-sm">
        <span className="block break-words">{headLine}</span>
        {metaLines.length > 0 && (
          <span className="hidden md:block whitespace-pre-line">
            {metaLines.join("\n")}
          </span>
        )}
      </h2>
    </header>
  );
};

export default Header;
