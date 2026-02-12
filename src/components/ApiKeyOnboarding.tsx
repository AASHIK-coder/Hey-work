import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ApiKeyOnboardingProps {
  onComplete: () => void;
}

export default function ApiKeyOnboarding({ onComplete }: ApiKeyOnboardingProps) {
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleContinue = async () => {
    const trimmed = apiKey.trim();
    if (!trimmed) {
      setError("Please enter a valid Anthropic API key.");
      return;
    }

    setSaving(true);
    setError(null);
    try {
      await invoke("save_api_key", { service: "anthropic", key: trimmed });
      await invoke("set_api_key", { apiKey: trimmed });
      localStorage.setItem("heywork_onboarding_complete", "true");
      onComplete();
    } catch (e) {
      setError(typeof e === "string" ? e : "Failed to save API key.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="min-h-screen bg-black text-white flex items-center justify-center px-6">
      <div className="w-full max-w-md rounded-2xl border border-white/10 bg-white/[0.03] p-6 space-y-4">
        <h1 className="text-xl font-semibold">Welcome to Hey work</h1>
        <p className="text-sm text-white/70">
          Enter your Anthropic API key to continue. It will be stored securely on this device.
        </p>
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="sk-ant-..."
          className="w-full rounded-lg border border-white/10 bg-black/40 px-3 py-2 text-sm outline-none focus:border-white/30"
          disabled={saving}
        />
        {error && <p className="text-xs text-red-400">{error}</p>}
        <button
          onClick={handleContinue}
          disabled={saving}
          className="w-full rounded-lg bg-white text-black font-medium py-2 disabled:opacity-60"
        >
          {saving ? "Saving..." : "Continue"}
        </button>
      </div>
    </div>
  );
}
