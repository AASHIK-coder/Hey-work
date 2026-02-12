import { useEffect, useReducer, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ChatView from "./components/ChatView";
import { useAgent } from "./hooks/useAgent";
import { useAgentStore } from "./stores/agentStore";
import { X, Send, Volume2, Mic, Maximize2, Bot, Brain, Zap } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import SwarmPanel from "./components/SwarmPanel";
import SkillsPanel from "./components/SkillsPanel";

// main window states
type State =
  | { mode: "idle" }
  | { mode: "revealing" }       // orb expands to show brand name
  | { mode: "expanded" }
  | { mode: "running" }
  | { mode: "help"; screenshot: string }
  | { mode: "voiceResponse" }
  | { mode: "spotlight" };

type Action =
  | { type: "EXPAND" }          // idle → revealing
  | { type: "REVEAL_DONE" }     // revealing → expanded
  | { type: "COLLAPSE" }
  | { type: "HELP"; screenshot: string }
  | { type: "HELP_CANCEL" }
  | { type: "HELP_SUBMIT" }
  | { type: "AGENT_START" }
  | { type: "AGENT_STOP" }
  | { type: "VOICE_RESPONSE" }
  | { type: "VOICE_DISMISS" }
  | { type: "VOICE_EXPAND" }
  | { type: "SPOTLIGHT" }
  | { type: "SPOTLIGHT_CANCEL" }
  | { type: "SPOTLIGHT_SUBMIT" };

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "EXPAND":
      return { mode: "revealing" };
    case "REVEAL_DONE":
      return { mode: "expanded" };
    case "COLLAPSE":
      return { mode: "idle" };
    case "HELP":
      return { mode: "help", screenshot: action.screenshot };
    case "HELP_CANCEL":
      return { mode: "idle" };
    case "HELP_SUBMIT":
      return { mode: "expanded" };
    case "AGENT_START":
      return state.mode === "voiceResponse" ? state : { mode: "running" };
    case "AGENT_STOP":
      if (state.mode === "voiceResponse") return state;
      return state.mode === "running" ? { mode: "expanded" } : state;
    case "VOICE_RESPONSE":
      return { mode: "voiceResponse" };
    case "VOICE_DISMISS":
      return { mode: "idle" };
    case "VOICE_EXPAND":
      return { mode: "expanded" };
    case "SPOTLIGHT":
      return { mode: "spotlight" };
    case "SPOTLIGHT_CANCEL":
      return { mode: "idle" };
    case "SPOTLIGHT_SUBMIT":
      return { mode: "expanded" };
    default:
      return state;
  }
}

// size configs
const SIZES: Record<string, { w: number; h: number; centered?: boolean }> = {
  idle: { w: 52, h: 52 },
  revealing: { w: 300, h: 52 },
  expanded: { w: 420, h: 540 },
  running: { w: 420, h: 540 },
  help: { w: 520, h: 420, centered: true },
  voiceResponse: { w: 340, h: 420 },
  spotlight: { w: 600, h: 72, centered: true },
};

export default function MainWindow() {
  const [state, dispatch] = useReducer(reducer, { mode: "idle" });
  const { submit } = useAgent();
  const selectedMode = useAgentStore((s) => s.selectedMode);

  const helpPromptRef = useRef("");
  const spotlightPromptRef = useRef("");
  const submitRef = useRef(submit);
  const stateRef = useRef(state);

  useEffect(() => {
    submitRef.current = submit;
  }, [submit]);

  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  // Manual panel drag: track mouse delta and move panel via native command.
  // This works reliably with NSPanel (Tauri's startDragging does not).
  const isDragging = useRef(false);
  const dragStart = useRef({ screenX: 0, screenY: 0, winX: 0, winY: 0 });

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (stateRef.current.mode !== "idle" && stateRef.current.mode !== "revealing") return;
      isDragging.current = true;
      // screenX/Y = mouse position on physical screen
      dragStart.current.screenX = e.screenX;
      dragStart.current.screenY = e.screenY;
      // window.screenX/Y = current window position
      dragStart.current.winX = window.screenX;
      dragStart.current.winY = window.screenY;
      e.preventDefault();
    };
    const onMove = (e: MouseEvent) => {
      if (!isDragging.current) return;
      const dx = e.screenX - dragStart.current.screenX;
      const dy = e.screenY - dragStart.current.screenY;
      // macOS screen coords: origin bottom-left, but screenY from JS is top-left.
      // NSWindow.setFrameOrigin uses bottom-left origin.
      // Convert: newY_bottom = screenHeight - (newY_top + windowHeight)
      const newX = dragStart.current.winX + dx;
      const newTopY = dragStart.current.winY + dy;
      const screenH = window.screen.height;
      const winH = window.outerHeight;
      const newY = screenH - newTopY - winH;
      invoke("move_panel_to", { x: newX, y: newY }).catch(() => {});
    };
    const onUp = () => {
      isDragging.current = false;
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, []);

  // sync window size/position with state
  useEffect(() => {
    const size = SIZES[state.mode];
    const centered = size.centered ?? false;
    invoke("set_window_state", {
      width: size.w,
      height: size.h,
      centered,
    }).catch(console.error);

    // Toggle #root clipping — orb/reveal modes need no box, others get rounded card
    const root = document.getElementById("root");
    if (root) {
      const noBox = state.mode === "idle" || state.mode === "revealing";
      root.style.borderRadius = noBox ? "0" : "12px";
      root.style.overflow = noBox ? "visible" : "hidden";
      root.style.background = "transparent";
    }
  }, [state.mode]);

  // Show window on initial mount
  useEffect(() => {
    invoke("set_window_state", {
      width: 52,
      height: 52,
      centered: false,
    }).catch(console.error);
  }, []);

  // state for speak text in voice response mode
  const [speakText, setSpeakText] = useState("");
  const [isRunning, setIsRunning] = useState(false);
  const [isPttActive, setIsPttActive] = useState(false);
  const [showSwarmPanel, setShowSwarmPanel] = useState(false);
  const [showSkillsPanel, setShowSkillsPanel] = useState(false);

  // startDrag kept for backwards compat but real drag is the global mousedown/move above
  const startDrag = () => {
    getCurrentWindow().startDragging().catch(() => {});
  };

  // tool messages for voice response mode
  const messages = useAgentStore((s) => s.messages);
  const toolMessages = messages.filter(
    (m) => m.type === "action" || m.type === "bash"
  ).slice(-5);

  const toolLogRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (toolLogRef.current) {
      toolLogRef.current.scrollTop = toolLogRef.current.scrollHeight;
    }
  }, [toolMessages.length]);

  // event listeners
  useEffect(() => {
    const listeners = [
      listen("agent:started", () => {
        dispatch({ type: "AGENT_START" });
        setIsRunning(true);
      }),
      listen("agent:stopped", () => {
        dispatch({ type: "AGENT_STOP" });
        setIsRunning(false);
      }),

      listen<{ screenshot: string | null }>("hotkey-help", (e) => {
        if (e.payload.screenshot) {
          dispatch({ type: "HELP", screenshot: e.payload.screenshot });
        }
      }),

      listen<{ text: string; screenshot: string | null; mode: string }>(
        "voice:response",
        async (e) => {
          dispatch({ type: "VOICE_RESPONSE" });
          setSpeakText("");
          useAgentStore.getState().setVoiceMode(true);
          await submitRef.current(e.payload.text, e.payload.screenshot ?? undefined, e.payload.mode);
        }
      ),

      listen<{ audio: string; text: string }>("agent:speak", (e) => {
        setSpeakText(e.payload.text);
      }),

      listen("hotkey-spotlight", () => {
        dispatch({ type: "SPOTLIGHT" });
      }),

      // FIX: Only hide panel if in spotlight mode; otherwise leave visible
      listen("window:blur", () => {
        if (stateRef.current.mode === "spotlight") {
          dispatch({ type: "SPOTLIGHT_CANCEL" });
          invoke("hide_main_window").catch(() => {});
        }
      }),

      // FIX: When tray icon re-shows the app, ensure React state resets to idle
      listen("tray:show", () => {
        if (stateRef.current.mode !== "idle") {
          dispatch({ type: "COLLAPSE" });
        }
        // Force re-apply window state even if already idle
        invoke("set_window_state", { width: 52, height: 52, centered: false }).catch(() => {});
      }),
    ];

    return () => {
      listeners.forEach((p) => p.then((fn) => fn()));
    };
  }, []);

  // Auto-advance from revealing → expanded after showing brand
  useEffect(() => {
    if (state.mode === "revealing") {
      const timer = setTimeout(() => dispatch({ type: "REVEAL_DONE" }), 1200);
      return () => clearTimeout(timer);
    }
  }, [state.mode]);

  // ═══════════════════════════════════════════════════════════════
  // IDLE — Just the orb, no background, pure floating icon
  // ═══════════════════════════════════════════════════════════════
  if (state.mode === "idle") {
    return (
      <motion.div
        data-tauri-drag-region
        onClick={() => dispatch({ type: "EXPAND" })}
        onMouseDown={startDrag}
        initial={{ opacity: 0, scale: 0.5 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: 0.5, ease: [0.23, 1, 0.32, 1] }}
        className="idle-icon"
      >
        <img
          data-tauri-drag-region
          src="/windows-computer-icon.png"
          alt="Hey work"
          draggable={false}
          className="idle-icon-img"
          onMouseDown={startDrag}
        />
      </motion.div>
    );
  }

  // ═══════════════════════════════════════════════════════════════
  // REVEALING — Orb expands into capsule showing brand name
  // ═══════════════════════════════════════════════════════════════
  if (state.mode === "revealing") {
    return (
      <motion.div
        data-tauri-drag-region
        initial={{ opacity: 0.9 }}
        animate={{ opacity: 1 }}
        className="reveal-row"
        onMouseDown={startDrag}
      >
        <img
          data-tauri-drag-region
          src="/windows-computer-icon.png"
          alt="Hey work"
          draggable={false}
          className="reveal-icon-img"
          onMouseDown={startDrag}
        />

        {/* Brand name slides in */}
        <motion.div
          className="reveal-brand"
          initial={{ opacity: 0, x: -12 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ duration: 0.4, delay: 0.15, ease: [0.23, 1, 0.32, 1] }}
            onMouseDown={startDrag}
        >
          <span className="reveal-brand-text">Hey work</span>
          <span className="reveal-brand-shimmer">Hey work</span>
        </motion.div>
      </motion.div>
    );
  }

  // ═══════════════════════════════════════════════════════════════
  // HELP MODE - screenshot + prompt (Cmd+Shift+H)
  // ═══════════════════════════════════════════════════════════════
  if (state.mode === "help") {
    const handleSubmit = async () => {
      const prompt = helpPromptRef.current;
      if (!prompt.trim()) return;
      dispatch({ type: "HELP_SUBMIT" });
      await submitRef.current(prompt, state.screenshot);
      helpPromptRef.current = "";
    };

    const handleCancel = () => {
      dispatch({ type: "HELP_CANCEL" });
      // Don't hide panel - just collapse to idle bar
    };

    return (
      <div className="h-full w-full flex items-center justify-center p-4 bg-transparent">
        <motion.div
          initial={{ opacity: 0, scale: 0.95, y: 10 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          transition={{ duration: 0.15 }}
          className="w-full max-w-[480px] bg-black/80 backdrop-blur-2xl rounded-2xl border border-white/10 overflow-hidden shadow-2xl shadow-black/50"
        >
          <div className="p-3 pb-2">
            <img
              src={`data:image/jpeg;base64,${state.screenshot}`}
              alt="Screenshot"
              className="w-full rounded-xl border border-white/5"
            />
          </div>
          <div className="px-3 pb-2">
            <input
              type="text"
              autoFocus
              onChange={(e) => (helpPromptRef.current = e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSubmit();
                if (e.key === "Escape") handleCancel();
              }}
              placeholder="What do you need help with?"
              className="w-full bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-sm text-white placeholder:text-white/40 focus:outline-none focus:border-blue-500/30"
            />
          </div>
          <div className="px-3 pb-3 flex gap-2">
            <button
              onClick={handleCancel}
              className="flex-1 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white/60 hover:bg-white/10 text-xs flex items-center justify-center gap-1.5 transition-colors"
            >
              <X size={14} /> Cancel
            </button>
            <button
              onClick={handleSubmit}
              className="flex-1 py-2.5 rounded-xl bg-blue-500/20 border border-blue-400/20 text-blue-200 hover:bg-blue-500/30 text-xs font-medium flex items-center justify-center gap-1.5 transition-colors"
            >
              <Send size={14} /> Send
            </button>
          </div>
        </motion.div>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════
  // SPOTLIGHT MODE - centered quick input
  // ═══════════════════════════════════════════════════════════════
  if (state.mode === "spotlight") {
    const handleSubmit = async () => {
      const prompt = spotlightPromptRef.current;
      if (!prompt.trim()) return;
      dispatch({ type: "SPOTLIGHT_SUBMIT" });
      await submitRef.current(prompt);
      spotlightPromptRef.current = "";
    };

    const handleCancel = () => {
      dispatch({ type: "SPOTLIGHT_CANCEL" });
      invoke("hide_main_window").catch(() => {});
    };

    return (
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: -8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={{ duration: 0.12, ease: "easeOut" }}
        className="h-full w-full flex items-center px-5 bg-black/80 backdrop-blur-2xl rounded-2xl border border-blue-500/15 shadow-2xl shadow-black/60"
      >
        <Zap size={16} className="text-blue-400/70 mr-3 flex-shrink-0" />
        <input
          type="text"
          autoFocus
          onChange={(e) => (spotlightPromptRef.current = e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleSubmit();
            if (e.key === "Escape") handleCancel();
          }}
          placeholder="What would you like me to do?"
          className="flex-1 bg-transparent text-white text-[15px] placeholder:text-white/40 focus:outline-none"
        />
        <div className="flex items-center gap-2">
          <kbd className="text-[10px] text-white/30 bg-white/5 px-1.5 py-0.5 rounded">esc</kbd>
          <button
            onClick={handleSubmit}
            className="p-2 rounded-lg bg-blue-500/15 hover:bg-blue-500/25 transition-colors"
            title="Submit"
          >
            <Send size={14} className="text-blue-300/70" />
          </button>
        </div>
      </motion.div>
    );
  }

  // ═══════════════════════════════════════════════════════════════
  // VOICE RESPONSE MODE
  // ═══════════════════════════════════════════════════════════════
  if (state.mode === "voiceResponse") {
    const handleDismiss = () => {
      dispatch({ type: "VOICE_DISMISS" });
      setSpeakText("");
      // Don't hide panel - just collapse to idle bar
    };

    const handleMicDown = () => {
      setIsPttActive(true);
      invoke("start_ptt", { mode: selectedMode }).catch(console.error);
    };

    const handleMicUp = () => {
      if (isPttActive) {
        setIsPttActive(false);
        invoke("stop_ptt").catch(console.error);
      }
    };

    return (
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        className="h-full w-full flex flex-col bg-black/85 backdrop-blur-2xl rounded-2xl border border-blue-500/10 overflow-hidden relative select-none"
      >
        {/* titlebar */}
        <div data-tauri-drag-region className="flex items-center justify-between px-3 py-2 border-b border-white/5">
          <div className="flex items-center gap-2">
            <Zap size={12} className="text-blue-400/70" />
            <span className="text-[11px] text-white/50 font-semibold tracking-wider">SUPER AGENT</span>
          </div>
          <div className="flex items-center gap-1">
            <button onClick={() => dispatch({ type: "VOICE_EXPAND" })} className="p-1.5 rounded-lg hover:bg-white/10 transition-colors" title="Expand to chat">
              <Maximize2 size={12} className="text-white/40" />
            </button>
            <button onClick={handleDismiss} className="p-1.5 rounded-lg hover:bg-white/10 transition-colors" title="Dismiss">
              <X size={12} className="text-white/40" />
            </button>
          </div>
        </div>

        {/* ambient glow */}
        <motion.div
          className="absolute inset-0 pointer-events-none"
          animate={{
            background: isRunning
              ? [
                  "radial-gradient(circle at 50% 50%, rgba(59,130,246,0.06) 0%, transparent 70%)",
                  "radial-gradient(circle at 50% 50%, rgba(59,130,246,0.15) 0%, transparent 70%)",
                  "radial-gradient(circle at 50% 50%, rgba(59,130,246,0.06) 0%, transparent 70%)",
                ]
              : "radial-gradient(circle at 50% 50%, rgba(59,130,246,0.04) 0%, transparent 70%)",
          }}
          transition={{ duration: 2.5, repeat: Infinity, ease: "easeInOut" }}
        />

        {/* speak text */}
        <div className="flex-1 flex items-center justify-center px-6 py-4 min-h-0">
          <AnimatePresence mode="wait">
            {speakText ? (
              <motion.p
                key={speakText}
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.95, y: -20 }}
                transition={{ duration: 0.3 }}
                className={`text-white/90 leading-relaxed text-center font-light ${
                  speakText.length > 200 ? "text-sm" : speakText.length > 100 ? "text-base" : "text-lg"
                }`}
              >
                {speakText}
              </motion.p>
            ) : (
              <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="flex flex-col items-center gap-3">
                {isRunning ? (
                  <>
                    <motion.div
                      className="w-2.5 h-2.5 rounded-full bg-blue-400"
                      animate={{ scale: [1, 1.4, 1], opacity: [0.5, 1, 0.5] }}
                      transition={{ duration: 2, repeat: Infinity, ease: "easeInOut" }}
                    />
                    <span className="text-white/30 text-xs tracking-wider">working...</span>
                  </>
                ) : (
                  <span className="text-white/20 text-sm">tap to expand</span>
                )}
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        {/* tool log */}
        {isRunning && toolMessages.length > 0 && (
          <div ref={toolLogRef} className="mx-3 mb-3 max-h-[120px] overflow-y-auto rounded-lg bg-white/5 border border-white/5" onClick={(e) => e.stopPropagation()}>
            {toolMessages.map((msg, i) => (
              <motion.div key={i} initial={{ opacity: 0, x: -10 }} animate={{ opacity: 1, x: 0 }} className="px-3 py-1.5 text-[11px] text-white/50 border-b border-white/5 last:border-0 flex items-center gap-2">
                <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${msg.pending ? "bg-blue-400 animate-pulse" : "bg-blue-300/50"}`} />
                <span className="truncate">{msg.content}</span>
              </motion.div>
            ))}
          </div>
        )}

        {/* bottom bar */}
        <div className="px-3 pb-3 flex items-center justify-between">
          <motion.div className="flex items-center gap-1.5 px-2 py-1 rounded-full bg-white/5" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.3 }}>
            <Volume2 size={10} className={`text-blue-400/60 ${isRunning ? "animate-pulse" : ""}`} />
            <span className="text-[9px] text-white/30">{isRunning ? "working" : "done"}</span>
          </motion.div>
          <div className="flex items-center gap-2">
            {!isRunning && <span className="text-[9px] text-white/20">{selectedMode === "browser" ? "⌃⇧B" : "⌃⇧C"}</span>}
            <motion.button
              onMouseDown={handleMicDown} onMouseUp={handleMicUp} onMouseLeave={handleMicUp}
              disabled={isRunning}
              className={`p-2.5 rounded-xl transition-colors ${isRunning ? "bg-white/5 text-white/20 cursor-not-allowed" : "bg-white/10 hover:bg-blue-500/20 text-white/60 hover:text-blue-300"}`}
              title={`Hold to speak (${selectedMode === "browser" ? "⌃⇧B" : "⌃⇧C"})`}
              whileHover={isRunning ? {} : { scale: 1.05 }}
              whileTap={isRunning ? {} : { scale: 0.95 }}
            >
              <Mic size={16} />
            </motion.button>
          </div>
        </div>
      </motion.div>
    );
  }

  // ═══════════════════════════════════════════════════════════════
  // EXPANDED / RUNNING - Main chat with proper header
  // ═══════════════════════════════════════════════════════════════
  return (
    <>
      <div className="h-full w-full flex flex-col bg-black/90 backdrop-blur-xl rounded-2xl border border-blue-500/10 overflow-hidden relative">
        <ChatView
          variant="compact"
          onCollapse={() => dispatch({ type: "COLLAPSE" })}
          headerRight={
            <div className="flex items-center gap-1">
              <button
                onClick={() => { setShowSkillsPanel(!showSkillsPanel); if (showSwarmPanel) setShowSwarmPanel(false); }}
                className={`p-1.5 rounded-lg transition-all ${
                  showSkillsPanel ? "bg-purple-500/20 text-purple-300" : "text-white/40 hover:bg-white/10 hover:text-white/60"
                }`}
                title="Skills Library"
              >
                <Brain size={14} />
              </button>
              <button
                onClick={() => { setShowSwarmPanel(!showSwarmPanel); if (showSkillsPanel) setShowSkillsPanel(false); }}
                className={`p-1.5 rounded-lg transition-all ${
                  showSwarmPanel ? "bg-blue-500/20 text-blue-300" : "text-white/40 hover:bg-white/10 hover:text-white/60"
                }`}
                title="Agent Swarm"
              >
                <Bot size={14} />
              </button>
            </div>
          }
        />
      </div>
      <SwarmPanel isOpen={showSwarmPanel} onClose={() => setShowSwarmPanel(false)} />
      <SkillsPanel isOpen={showSkillsPanel} onClose={() => setShowSkillsPanel(false)} />
    </>
  );
}
