import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import {
  Download,
  Upload,
  Trash2,
  Wrench,
  CheckCircle2,
  XCircle,
  Loader2,
  X,
  FileJson,
  Brain,
} from "lucide-react";

interface Skill {
  id: string;
  name: string;
  description: string;
  pattern: {
    intent_keywords: string[];
    app_context: string | null;
  };
  success_rate: number;
  total_uses: number;
}

interface SkillsPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function SkillsPanel({ isOpen, onClose }: SkillsPanelProps) {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [importText, setImportText] = useState("");
  const [showImport, setShowImport] = useState(false);

  // Load skills on mount
  useEffect(() => {
    if (isOpen) {
      loadSkills();
    }
  }, [isOpen]);

  const loadSkills = async () => {
    setIsLoading(true);
    try {
      const loadedSkills = await invoke<Skill[]>("list_skills");
      setSkills(loadedSkills);
    } catch (e) {
      console.error("Failed to load skills:", e);
      setSkills([]);
    } finally {
      setIsLoading(false);
    }
  };

  const handleExport = async () => {
    try {
      const json = await invoke<string>("export_skills");
      
      // Download as file
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `heywork-skills-${new Date().toISOString().split("T")[0]}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      
      showMessage("success", "Skills exported successfully");
    } catch (e) {
      showMessage("error", `Export failed: ${e}`);
    }
  };

  const handleImport = async () => {
    if (!importText.trim()) return;
    
    try {
      const count = await invoke<number>("import_skills", { json: importText });
      showMessage("success", `Imported ${count} skills`);
      setImportText("");
      setShowImport(false);
      loadSkills();
    } catch (e) {
      showMessage("error", `Import failed: ${e}`);
    }
  };

  const showMessage = (type: "success" | "error", text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  };

  const handleFileImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    
    const reader = new FileReader();
    reader.onload = (event) => {
      setImportText(event.target?.result as string);
    };
    reader.readAsText(file);
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          initial={{ opacity: 0, x: 300 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 300 }}
          transition={{ type: "spring", damping: 25, stiffness: 200 }}
          className="fixed right-0 top-0 h-full w-[380px] bg-black/95 backdrop-blur-xl border-l border-white/10 shadow-2xl z-50 flex flex-col"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
            <div className="flex items-center gap-2">
              <Brain size={18} className="text-purple-400" />
              <span className="font-medium text-white/90">Skills Library</span>
            </div>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg hover:bg-white/10 transition-colors"
            >
              <X size={18} className="text-white/60" />
            </button>
          </div>

          {/* Message Toast */}
          <AnimatePresence>
            {message && (
              <motion.div
                initial={{ opacity: 0, y: -20 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -20 }}
                className={`mx-4 mt-3 px-3 py-2 rounded-lg flex items-center gap-2 text-sm ${
                  message.type === "success"
                    ? "bg-green-500/20 text-green-300 border border-green-500/30"
                    : "bg-red-500/20 text-red-300 border border-red-500/30"
                }`}
              >
                {message.type === "success" ? (
                  <CheckCircle2 size={16} />
                ) : (
                  <XCircle size={16} />
                )}
                {message.text}
              </motion.div>
            )}
          </AnimatePresence>

          {/* Actions */}
          <div className="p-4 border-b border-white/10 space-y-2">
            <div className="flex gap-2">
              <button
                onClick={handleExport}
                className="flex-1 py-2 px-3 rounded-xl bg-white/5 border border-white/10 text-white/70 hover:bg-white/10 transition-colors flex items-center justify-center gap-2 text-sm"
              >
                <Download size={16} />
                Export
              </button>
              <button
                onClick={() => setShowImport(!showImport)}
                className={`flex-1 py-2 px-3 rounded-xl border transition-colors flex items-center justify-center gap-2 text-sm ${
                  showImport
                    ? "bg-purple-500/20 border-purple-500/30 text-purple-300"
                    : "bg-white/5 border-white/10 text-white/70 hover:bg-white/10"
                }`}
              >
                <Upload size={16} />
                Import
              </button>
            </div>

            {/* Import Area */}
            <AnimatePresence>
              {showImport && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: "auto", opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  className="overflow-hidden"
                >
                  <div className="space-y-2 pt-2">
                    <textarea
                      value={importText}
                      onChange={(e) => setImportText(e.target.value)}
                      placeholder="Paste skills JSON here..."
                      className="w-full h-24 bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-xs text-white/80 placeholder:text-white/30 resize-none focus:outline-none focus:border-white/20"
                    />
                    <div className="flex gap-2">
                      <label className="flex-1 py-1.5 px-3 rounded-lg bg-white/5 border border-white/10 text-white/60 hover:bg-white/10 transition-colors flex items-center justify-center gap-2 text-xs cursor-pointer">
                        <FileJson size={14} />
                        Choose File
                        <input
                          type="file"
                          accept=".json"
                          onChange={handleFileImport}
                          className="hidden"
                        />
                      </label>
                      <button
                        onClick={handleImport}
                        disabled={!importText.trim()}
                        className="flex-1 py-1.5 px-3 rounded-lg bg-purple-500/20 border border-purple-500/30 text-purple-300 hover:bg-purple-500/30 transition-colors text-xs disabled:opacity-50"
                      >
                        Import Skills
                      </button>
                    </div>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </div>

          {/* Skills List */}
          <div className="flex-1 overflow-y-auto p-4">
            {isLoading ? (
              <div className="flex items-center justify-center h-32">
                <Loader2 size={24} className="animate-spin text-white/30" />
              </div>
            ) : skills.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-48 text-white/40">
                <Wrench size={48} className="mb-4 opacity-30" />
                <p className="text-sm">No custom skills yet</p>
                <p className="text-xs mt-1 opacity-60 max-w-[200px] text-center">
                  Skills are learned automatically from successful task executions
                </p>
              </div>
            ) : (
              <div className="space-y-2">
                {skills.map((skill) => (
                  <SkillCard key={skill.id} skill={skill} />
                ))}
              </div>
            )}
          </div>

          {/* Footer Stats */}
          <div className="px-4 py-3 border-t border-white/10 bg-white/5">
            <div className="flex items-center justify-between text-xs text-white/40">
              <span>Total: {skills.length} skills</span>
              <span className="text-white/30">
                Auto-learned from executions
              </span>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function SkillCard({ skill }: { skill: Skill }) {
  return (
    <motion.div
      layout
      className="p-3 rounded-xl bg-white/5 border border-white/10 hover:border-white/20 transition-colors"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex-1 min-w-0">
          <h4 className="text-sm font-medium text-white/80 truncate">
            {skill.name}
          </h4>
          <p className="text-xs text-white/50 mt-0.5 line-clamp-2">
            {skill.description}
          </p>
        </div>
        <button className="p-1.5 rounded-lg hover:bg-red-500/20 hover:text-red-400 text-white/30 transition-colors">
          <Trash2 size={14} />
        </button>
      </div>

      {/* Keywords */}
      <div className="flex flex-wrap gap-1 mt-2">
        {skill.pattern.intent_keywords.slice(0, 3).map((keyword) => (
          <span
            key={keyword}
            className="px-1.5 py-0.5 rounded text-[9px] bg-white/5 text-white/40"
          >
            {keyword}
          </span>
        ))}
        {skill.pattern.intent_keywords.length > 3 && (
          <span className="px-1.5 py-0.5 rounded text-[9px] bg-white/5 text-white/40">
            +{skill.pattern.intent_keywords.length - 3}
          </span>
        )}
      </div>

      {/* Stats */}
      <div className="flex items-center gap-3 mt-2 pt-2 border-t border-white/5">
        <div className="flex items-center gap-1">
          <div
            className={`w-1.5 h-1.5 rounded-full ${
              skill.success_rate > 0.8
                ? "bg-green-400"
                : skill.success_rate > 0.5
                ? "bg-yellow-400"
                : "bg-red-400"
            }`}
          />
          <span className="text-[10px] text-white/40">
            {Math.round(skill.success_rate * 100)}% success
          </span>
        </div>
        <span className="text-[10px] text-white/30">
          {skill.total_uses} uses
        </span>
      </div>
    </motion.div>
  );
}
