import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "react-hot-toast";
import { addCodexAuth } from "../../api";
import Button from "../common/Button";
import FormInput from "../common/FormInput";
import StatusMessage from "../common/StatusMessage";

interface Props {
  onAdded: () => void;
}

const AddCodexAuthForm: React.FC<Props> = ({ onAdded }) => {
  const { t } = useTranslation();
  const [authJson, setAuthJson] = useState("");
  const [label, setLabel] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!authJson.trim()) {
      setError(t("codexAdd.errorEmpty"));
      return;
    }
    setSubmitting(true);
    try {
      const summary = await addCodexAuth(
        authJson,
        label.trim() || undefined,
      );
      toast.success(
        t("codexAdd.success", { id: summary.id.slice(0, 8) }),
      );
      setAuthJson("");
      setLabel("");
      onAdded();
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Unknown error";
      setError(msg);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <FormInput
        id="codex-label"
        name="codex-label"
        type="text"
        value={label}
        onChange={(e) => setLabel(e.target.value)}
        label={t("codexAdd.labelField")}
        placeholder={t("codexAdd.labelPlaceholder")}
      />
      <div>
        <label
          htmlFor="codex-authjson"
          className="block text-sm font-medium mb-1 text-gray-300"
        >
          {t("codexAdd.authJsonField")}
        </label>
        <textarea
          id="codex-authjson"
          rows={10}
          value={authJson}
          onChange={(e) => setAuthJson(e.target.value)}
          placeholder={t("codexAdd.authJsonPlaceholder")}
          className="w-full font-mono text-xs p-3 rounded-md bg-gray-700 border border-gray-600 text-gray-200 focus:ring-2 focus:ring-cyan-500 focus:border-cyan-500 placeholder-gray-400"
        />
        <p className="text-xs text-gray-400 mt-1">{t("codexAdd.help")}</p>
      </div>
      {error && <StatusMessage type="error" message={error} />}
      <Button type="submit" disabled={submitting} isLoading={submitting}>
        {submitting ? t("codexAdd.submitting") : t("codexAdd.submit")}
      </Button>
    </form>
  );
};

export default AddCodexAuthForm;
