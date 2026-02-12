import { useEffect, useState } from "react";
import {
  Check,
  ExternalLink,
  Chrome,
  Key,
  Shield,
  Keyboard,
  Eye,
  EyeOff,
  Trash2,
  RefreshCw,
  X,
  Loader2,
  Mic,
  RotateCcw,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useAgentStore } from "../stores/agentStore";

interface PermissionsCheck {
  accessibility: "granted" | "denied" | "notAsked" | "notNeeded";
  screenRecording: "granted" | "denied" | "notAsked" | "notNeeded";
  microphone: "granted" | "denied" | "notAsked" | "notNeeded";
}

interface BrowserProfileStatus {
  exists: boolean;
  path: string;
  sessions: string[];
}

interface ApiKeyStatus {
  anthropic: boolean;
  deepgram: boolean;
  elevenlabs: boolean;
}

interface VoiceSettings {
  elevenlabsVoiceId: string | null;
}

function PermissionRow({
  label,
  status,
  onRequest,
  onOpenSettings,
}: {
  label: string;
  status: "granted" | "denied" | "notAsked" | "notNeeded";
  onRequest: () => void;
  onOpenSettings: () => void;
}) {
  const isGranted = status === "granted" || status === "notNeeded";

  return (
    <div className="flex items-center justify-between py-2.5">
      <div className="flex items-center gap-3">
        <div
          className={`w-2 h-2 rounded-full ${
            isGranted
              ? "bg-emerald-400"
              : status === "notAsked"
              ? "bg-white/20"
              : "bg-red-400"
          }`}
        />
        <span className="text-[13px] text-white/90">{label}</span>
      </div>
      <div className="flex items-center gap-2">
        <span
          className={`text-[11px] ${
            isGranted
              ? "text-emerald-400/70"
              : status === "notAsked"
              ? "text-white/30"
              : "text-red-400/70"
          }`}
        >
          {status === "granted"
            ? "Granted"
            : status === "notAsked"
            ? "Not Asked"
            : status === "notNeeded"
            ? "OK"
            : "Denied"}
        </span>
        {!isGranted && (
          <button
            onClick={status === "denied" ? onOpenSettings : onRequest}
            className="px-2 py-1 text-[10px] rounded-md bg-white/10 hover:bg-white/20 text-white/60 hover:text-white/90 transition-colors"
          >
            {status === "denied" ? "Fix" : "Grant"}
          </button>
        )}
      </div>
    </div>
  );
}

function ApiKeyRow({
  label,
  isSet,
  onSave,
}: {
  label: string;
  isSet: boolean;
  onSave: (key: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState("");
  const [showKey, setShowKey] = useState(false);

  const handleSave = () => {
    if (value.trim()) {
      onSave(value.trim());
      setValue("");
      setEditing(false);
    }
  };

  return (
    <div className="flex items-center justify-between py-2.5">
      <div className="flex items-center gap-3">
        <Key size={14} className="text-white/50" />
        <span className="text-[13px] text-white/90">{label}</span>
      </div>

      {editing ? (
        <div className="flex items-center gap-2">
          <div className="relative">
            <input
              type={showKey ? "text" : "password"}
              value={value}
              onChange={(e) => setValue(e.target.value)}
              placeholder="sk-..."
              autoFocus
              className="w-[140px] px-2 py-1 text-[11px] bg-white/5 border border-white/10 rounded-md text-white/90 placeholder-white/30 focus:outline-none focus:border-white/30"
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSave();
                if (e.key === "Escape") {
                  setEditing(false);
                  setValue("");
                }
              }}
            />
            <button
              onClick={() => setShowKey(!showKey)}
              className="absolute right-1.5 top-1/2 -translate-y-1/2 text-white/30 hover:text-white/60"
            >
              {showKey ? <EyeOff size={10} /> : <Eye size={10} />}
            </button>
          </div>
          <button
            onClick={handleSave}
            disabled={!value.trim()}
            className="px-2 py-1 text-[10px] rounded-md bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-400 transition-colors disabled:opacity-50"
          >
            Save
          </button>
          <button
            onClick={() => {
              setEditing(false);
              setValue("");
            }}
              title="Cancel API key editing"
              aria-label="Cancel API key editing"
            className="text-white/30 hover:text-white/60"
          >
            <X size={12} />
          </button>
        </div>
      ) : (
        <div className="flex items-center gap-2">
          {isSet ? (
            <>
              <span className="text-[11px] text-white/40 font-mono">
                ••••••••••••
              </span>
              <Check size={12} className="text-emerald-400" />
            </>
          ) : (
            <span className="text-[11px] text-white/30">Not set</span>
          )}
          <button
            onClick={() => setEditing(true)}
            className="px-2 py-1 text-[10px] rounded-md bg-white/10 hover:bg-white/20 text-white/60 hover:text-white/90 transition-colors"
          >
            {isSet ? "Edit" : "Add"}
          </button>
        </div>
      )}
    </div>
  );
}

function LoadingSkeleton({ error }: { error?: string }) {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-3">
      {error ? (
        <>
          <span className="text-[12px] text-red-400">{error}</span>
          <span className="text-[10px] text-white/30">Retrying...</span>
        </>
      ) : (
        <>
          <Loader2 size={24} className="text-white/30 animate-spin" />
          <span className="text-[12px] text-white/30">Loading settings...</span>
        </>
      )}
    </div>
  );
}

const VOICE_PRESETS = [
  { id: "NOpBlnGInO9m6vDvFkFC", name: "Southern Grandpa" },
  { id: "PB6BdkFkZLbI39GHdnbQ", name: "$eX¥" },
  { id: "4O1sYUnmtThcBoSBrri7", name: "Maya" },
];

const DEFAULT_VOICE_ID = "NOpBlnGInO9m6vDvFkFC";

export default function SettingsContent() {
  const setApiKeySet = useAgentStore((s) => s.setApiKeySet);
  const [permissions, setPermissions] = useState<PermissionsCheck | null>(null);
  const [profile, setProfile] = useState<BrowserProfileStatus | null>(null);
  const [apiKeys, setApiKeys] = useState<ApiKeyStatus | null>(null);
  const [voiceSettings, setVoiceSettings] = useState<VoiceSettings | null>(null);
  const [resetting, setResetting] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [editingVoiceId, setEditingVoiceId] = useState(false);
  const [voiceIdInput, setVoiceIdInput] = useState("");

  useEffect(() => {
    let cancelled = false;
    const withTimeout = async <T,>(promise: Promise<T>, label: string, timeoutMs = 8000): Promise<T> => {
      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => reject(new Error(`${label} timed out`)), timeoutMs);
      });
      return Promise.race([promise, timeoutPromise]);
    };

    const check = async () => {
      try {
        if (!cancelled) {
          setLoadError(null);
        }
        
        const results = await Promise.allSettled([
          withTimeout(invoke<PermissionsCheck>("check_permissions"), "permissions"),
          withTimeout(invoke<BrowserProfileStatus>("get_browser_profile_status"), "browser profile"),
          withTimeout(invoke<ApiKeyStatus>("get_api_key_status"), "api key status"),
          withTimeout(invoke<VoiceSettings>("get_voice_settings"), "voice settings"),
        ]);
        if (cancelled) return;
        
        const [permsResult, profResult, keysResult, voiceResult] = results;
        
        if (permsResult.status === 'fulfilled') {
          setPermissions(permsResult.value);
        } else {
          console.error("Permissions check failed:", permsResult.reason);
          setLoadError("Permission check failed");
        }
        
        if (profResult.status === 'fulfilled') {
          setProfile(profResult.value);
        } else {
          console.error("Browser profile check failed:", profResult.reason);
        }
        
        if (keysResult.status === 'fulfilled') {
          setApiKeys(keysResult.value);
        } else {
          console.error("API key check failed:", keysResult.reason);
        }
        
        if (voiceResult.status === 'fulfilled') {
          setVoiceSettings(voiceResult.value);
        } else {
          console.error("Voice settings check failed:", voiceResult.reason);
        }

        const failedCount = results.filter((r) => r.status === "rejected").length;
        if (failedCount > 0) {
          setLoadError(`Some settings failed to load (${failedCount}/4). Retrying...`);
        }
      } catch (e) {
        console.error("Failed to check status:", e);
        if (!cancelled) {
          setLoadError("Failed to load settings");
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    check();
    // refresh every 5 seconds
    const interval = setInterval(check, 5000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  const handleRequestPermission = async (permission: string) => {
    await invoke("request_permission", { permission });
  };

  const handleOpenSettings = async (permission: string) => {
    await invoke("open_permission_settings", { permission });
  };

  const handleOpenProfile = async () => {
    await invoke("open_browser_profile");
  };

  const handleOpenDomain = async (domain: string) => {
    await invoke("open_browser_profile_url", { url: `https://${domain}` });
  };

  const handleClearDomain = async (domain: string) => {
    await invoke("clear_domain_cookies", { domain });
    const prof = await invoke<BrowserProfileStatus>("get_browser_profile_status");
    setProfile(prof);
  };

  const handleResetProfile = async () => {
    setResetting(true);
    try {
      await invoke("reset_browser_profile");
      const prof = await invoke<BrowserProfileStatus>(
        "get_browser_profile_status"
      );
      setProfile(prof);
    } finally {
      setResetting(false);
    }
  };

  const handleSaveApiKey = async (service: string, key: string) => {
    await invoke("save_api_key", { service, key });
    if (service === "anthropic") {
      await invoke("set_api_key", { apiKey: key });
      localStorage.setItem("heywork_onboarding_complete", "true");
      setApiKeySet(true);
    }
    const keys = await invoke<ApiKeyStatus>("get_api_key_status");
    setApiKeys(keys);
  };

  const handleSaveVoiceId = async () => {
    if (voiceIdInput.trim()) {
      await invoke("save_voice_settings", { voiceId: voiceIdInput.trim() });
      const voice = await invoke<VoiceSettings>("get_voice_settings");
      setVoiceSettings(voice);
      setVoiceIdInput("");
      setEditingVoiceId(false);
    }
  };

  const shortcuts = [
    { keys: "⌘⇧␣", label: "Spotlight" },
    { keys: "⌘⇧H", label: "Screenshot + Ask" },
    { keys: "⌘⇧V", label: "Push-to-Talk" },
    { keys: "⌃⇧C", label: "Voice → Computer Mode" },
    { keys: "⌃⇧B", label: "Voice → Browser Mode" },
    { keys: "⌘⇧S", label: "Stop Agent" },
    { keys: "⌘⇧Q", label: "Quit" },
  ];

  if (loading) {
    return <LoadingSkeleton error={loadError || undefined} />;
  }

  return (
    <div className="space-y-5">
      {/* permissions */}
      <section>
        <div className="flex items-center gap-2 mb-2">
          <Shield size={14} className="text-white/50" />
          <h3 className="text-[11px] font-medium uppercase tracking-wider text-white/50">
            Permissions
          </h3>
        </div>
        <div className="rounded-xl bg-white/[0.03] border border-white/5 px-4 divide-y divide-white/5">
          {permissions && (
            <>
              <PermissionRow
                label="Accessibility"
                status={permissions.accessibility}
                onRequest={() => handleRequestPermission("accessibility")}
                onOpenSettings={() => handleOpenSettings("accessibility")}
              />
              <PermissionRow
                label="Screen Recording"
                status={permissions.screenRecording}
                onRequest={() => handleRequestPermission("screenRecording")}
                onOpenSettings={() => handleOpenSettings("screenRecording")}
              />
              <PermissionRow
                label="Microphone"
                status={permissions.microphone}
                onRequest={() => handleRequestPermission("microphone")}
                onOpenSettings={() => handleOpenSettings("microphone")}
              />
            </>
          )}
        </div>
      </section>

      {/* api keys */}
      <section>
        <div className="flex items-center gap-2 mb-2">
          <Key size={14} className="text-white/50" />
          <h3 className="text-[11px] font-medium uppercase tracking-wider text-white/50">
            API Keys
          </h3>
        </div>
        <div className="rounded-xl bg-white/[0.03] border border-white/5 px-4 divide-y divide-white/5">
          {apiKeys && (
            <>
              <ApiKeyRow
                label="Anthropic"
                isSet={apiKeys.anthropic}
                onSave={(key) => handleSaveApiKey("anthropic", key)}
              />
              <ApiKeyRow
                label="Deepgram"
                isSet={apiKeys.deepgram}
                onSave={(key) => handleSaveApiKey("deepgram", key)}
              />
              <ApiKeyRow
                label="ElevenLabs"
                isSet={apiKeys.elevenlabs}
                onSave={(key) => handleSaveApiKey("elevenlabs", key)}
              />
            </>
          )}
        </div>
        <p className="text-[10px] text-white/40 mt-2 px-1">
          Keys are saved securely in your OS credential store
        </p>
      </section>

      {/* voice settings */}
      <section>
        <div className="flex items-center gap-2 mb-2">
          <Mic size={14} className="text-white/50" />
          <h3 className="text-[11px] font-medium uppercase tracking-wider text-white/50">
            Voice Settings
          </h3>
        </div>
        <div className="rounded-xl bg-white/[0.03] border border-white/5 px-4 py-3 space-y-3">
          {/* preset dropdown */}
          <div className="flex items-center justify-between">
            <span className="text-[13px] text-white/90">Voice</span>
            <select
              aria-label="Voice preset"
              value={
                VOICE_PRESETS.find(v => v.id === voiceSettings?.elevenlabsVoiceId)?.id ||
                (voiceSettings?.elevenlabsVoiceId ? "custom" : DEFAULT_VOICE_ID)
              }
              onChange={async (e) => {
                const value = e.target.value;
                if (value === "custom") {
                  setEditingVoiceId(true);
                  setVoiceIdInput(voiceSettings?.elevenlabsVoiceId || "");
                } else {
                  await invoke("save_voice_settings", { voiceId: value });
                  const voice = await invoke<VoiceSettings>("get_voice_settings");
                  setVoiceSettings(voice);
                }
              }}
              className="px-2 py-1.5 text-[12px] bg-white/5 border border-white/10 rounded-md text-white/90 focus:outline-none focus:border-white/30 cursor-pointer"
            >
              {VOICE_PRESETS.map((voice) => (
                <option key={voice.id} value={voice.id} className="bg-zinc-900">
                  {voice.name}
                </option>
              ))}
              <option value="custom" className="bg-zinc-900">Custom ID...</option>
            </select>
          </div>

          {/* custom voice id input */}
          {editingVoiceId && (
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={voiceIdInput}
                onChange={(e) => setVoiceIdInput(e.target.value)}
                placeholder="Enter voice ID..."
                autoFocus
                className="flex-1 px-2 py-1.5 text-[11px] bg-white/5 border border-white/10 rounded-md text-white/90 placeholder-white/30 focus:outline-none focus:border-white/30 font-mono"
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSaveVoiceId();
                  if (e.key === "Escape") {
                    setEditingVoiceId(false);
                    setVoiceIdInput("");
                  }
                }}
              />
              <button
                onClick={handleSaveVoiceId}
                disabled={!voiceIdInput.trim()}
                className="px-2 py-1 text-[10px] rounded-md bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-400 transition-colors disabled:opacity-50"
              >
                Save
              </button>
              <button
                onClick={() => {
                  setEditingVoiceId(false);
                  setVoiceIdInput("");
                }}
                title="Cancel custom voice ID"
                aria-label="Cancel custom voice ID"
                className="text-white/30 hover:text-white/60"
              >
                <X size={12} />
              </button>
            </div>
          )}
        </div>
      </section>

      {/* browser profile */}
      <section>
        <div className="flex items-center gap-2 mb-2">
          <Chrome size={14} className="text-white/50" />
          <h3 className="text-[11px] font-medium uppercase tracking-wider text-white/50">
            Browser Profile
          </h3>
        </div>
        <div className="rounded-xl bg-white/[0.03] border border-white/5 p-4">
          <p className="text-[12px] text-white/70 leading-relaxed mb-3">
            A dedicated Chrome profile for automation. Log into sites here and
            the agent will use those sessions.
          </p>

          {profile?.exists && profile.sessions.length > 0 && (
            <div className="mb-3">
              <div className="flex items-center justify-between mb-1.5">
                <span className="text-[10px] text-white/50 uppercase tracking-wider">
                  Cookies ({profile.sessions.length})
                </span>
                <span className="text-[9px] text-white/35 italic">
                  yes, ad trackers snuck in here too 🙄
                </span>
              </div>
              <div className="max-h-[140px] overflow-y-auto rounded-lg bg-black/30 divide-y divide-white/10">
                {profile.sessions.map((domain) => (
                  <div
                    key={domain}
                    className="flex items-center justify-between px-2 py-1.5 hover:bg-white/10 transition-colors group"
                  >
                    <button
                      onClick={() => handleOpenDomain(domain)}
                      className="flex items-center gap-2 text-[11px] text-white/80 hover:text-white transition-colors"
                    >
                      <span>{domain}</span>
                      <ExternalLink size={10} className="opacity-0 group-hover:opacity-70" />
                    </button>
                    <button
                      onClick={() => handleClearDomain(domain)}
                      className="p-1 rounded text-white/40 hover:text-red-400 hover:bg-red-500/20 transition-colors opacity-0 group-hover:opacity-100"
                      title="Remove session"
                    >
                      <X size={12} />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="flex gap-2">
            <button
              onClick={handleOpenProfile}
              className="flex-1 flex items-center justify-center gap-2 py-2 rounded-lg bg-white/10 hover:bg-white/15 text-white/80 hover:text-white text-[12px] transition-colors"
            >
              <ExternalLink size={12} />
              Open in Chrome
            </button>
            {profile?.exists && (
              <button
                onClick={handleResetProfile}
                disabled={resetting}
                className="flex items-center justify-center gap-2 px-3 py-2 rounded-lg bg-red-500/10 hover:bg-red-500/20 text-red-400/70 hover:text-red-400 text-[12px] transition-colors disabled:opacity-50"
              >
                {resetting ? (
                  <RefreshCw size={12} className="animate-spin" />
                ) : (
                  <Trash2 size={12} />
                )}
              </button>
            )}
          </div>
        </div>
      </section>

      {/* shortcuts */}
      <section>
        <div className="flex items-center gap-2 mb-2">
          <Keyboard size={14} className="text-white/50" />
          <h3 className="text-[11px] font-medium uppercase tracking-wider text-white/50">
            Shortcuts
          </h3>
        </div>
        <div className="rounded-xl bg-white/[0.03] border border-white/5 p-4 space-y-3">
          {shortcuts.map(({ keys, label }) => (
            <div key={keys} className="flex items-center justify-between">
              <span className="text-[13px] text-white/90">{label}</span>
              <kbd className="px-3 py-1 text-[12px] font-mono bg-white/10 rounded-md text-white/80 tracking-wider">
                {keys.split("").join(" ")}
              </kbd>
            </div>
          ))}
        </div>
      </section>

      {/* reset onboarding */}
      <section>
        <div className="flex items-center gap-2 mb-2">
          <RotateCcw size={14} className="text-white/50" />
          <h3 className="text-[11px] font-medium uppercase tracking-wider text-white/50">
            Setup
          </h3>
        </div>
        <div className="rounded-xl bg-white/[0.03] border border-white/5 p-4">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-[13px] text-white/90">Show Onboarding Again</p>
              <p className="text-[11px] text-white/40 mt-0.5">
                Restart the first-time setup flow
              </p>
            </div>
            <button
              onClick={() => {
                localStorage.removeItem("heywork_onboarding_complete");
                window.location.reload();
              }}
              className="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 text-white/70 hover:text-white text-[11px] transition-colors"
            >
              Reset
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}
