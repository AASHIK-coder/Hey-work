import ReactDOM from "react-dom/client";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import MainWindow from "./MainWindow";
import VoiceWindow from "./VoiceWindow";
import BorderOverlay from "./BorderOverlay";
import ApiKeyOnboarding from "./components/ApiKeyOnboarding";
import "./index.css";

const params = new URLSearchParams(window.location.search);
const isVoice = params.has("voice");
const isBorder = params.has("border");

let Component = MainWindow;
if (isVoice) Component = VoiceWindow;
if (isBorder) Component = BorderOverlay;

function MainAppGate() {
  const [loading, setLoading] = useState(true);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;

    const check = async () => {
      try {
        const status = await invoke<{ anthropic: boolean }>("get_api_key_status");
        const onboardingDone = localStorage.getItem("heywork_onboarding_complete") === "true";
        if (!cancelled) {
          setReady(status.anthropic && onboardingDone);
          setLoading(false);
        }
      } catch {
        if (!cancelled) {
          setReady(false);
          setLoading(false);
        }
      }
    };

    check();
    return () => {
      cancelled = true;
    };
  }, []);

  // Ensure window is visible and properly sized for non-main states
  useEffect(() => {
    if (loading) return;
    if (!ready) {
      // Onboarding: show window at a proper size so user can enter API key
      invoke("set_window_state", { width: 480, height: 500, centered: true }).catch(() => {});
    }
    // When ready, MainWindow will call set_window_state itself
  }, [loading, ready]);

  if (loading) {
    return <div className="min-h-screen bg-black text-white flex items-center justify-center">Loading...</div>;
  }

  if (!ready) {
    return <ApiKeyOnboarding onComplete={() => setReady(true)} />;
  }

  return <MainWindow />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  isVoice || isBorder ? <Component /> : <MainAppGate />
);
