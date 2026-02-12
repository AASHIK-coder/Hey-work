import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";
import { 
  Bot, 
  CheckCircle2, 
  XCircle, 
  Loader2, 
  RefreshCw,
  ChevronDown,
  ChevronUp,
  Trash2,
  X
} from "lucide-react";
import { useSwarmStore } from "../stores/swarmStore";
import { SwarmTask, AgentType } from "../types";

const agentColors: Record<AgentType, string> = {
  Planner: "bg-blue-500/20 text-blue-300 border-blue-500/30",
  Executor: "bg-green-500/20 text-green-300 border-green-500/30",
  Verifier: "bg-purple-500/20 text-purple-300 border-purple-500/30",
  Critic: "bg-amber-500/20 text-amber-300 border-amber-500/30",
  Recovery: "bg-red-500/20 text-red-300 border-red-500/30",
  Coordinator: "bg-cyan-500/20 text-cyan-300 border-cyan-500/30",
  Specialist: "bg-pink-500/20 text-pink-300 border-pink-500/30",
};

const statusIcons = {
  Pending: <div className="w-2 h-2 rounded-full bg-white/30" />,
  Ready: <div className="w-2 h-2 rounded-full bg-blue-400/60" />,
  Executing: <Loader2 size={14} className="animate-spin text-blue-400" />,
  Completed: <CheckCircle2 size={14} className="text-green-400" />,
  Failed: <XCircle size={14} className="text-red-400" />,
  Verifying: <Loader2 size={14} className="animate-spin text-purple-400" />,
  NeedsRetry: <RefreshCw size={14} className="text-yellow-400" />,
  Blocked: <div className="w-2 h-2 rounded-full bg-gray-500/60" />,
};

interface SwarmPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function SwarmPanel({ isOpen, onClose }: SwarmPanelProps) {
  const {
    tasks,
    setActiveTask,
    handleSwarmEvent,
    clearCompleted,
    isInitialized,
    setInitialized,
  } = useSwarmStore();

  const [expandedTasks, setExpandedTasks] = useState<Set<string>>(new Set());

  // Listen for swarm events - auto-initialize on first complex task
  useEffect(() => {
    const listeners = [
      listen<{ task_id: string; description: string }>("swarm:task_started", (e) => {
        const { task_id, description } = e.payload;
        handleSwarmEvent({ type: "task_started", task_id, description: description as any });
        setInitialized(true);
      }),
      listen<{ task_id: string; subtask_id: string; agent: string }>("swarm:subtask_started", (e) => {
        const { task_id, subtask_id, agent } = e.payload;
        handleSwarmEvent({ type: "subtask_started", task_id, subtask_id, agent: agent as AgentType });
      }),
      listen<{ task_id: string; subtask_id: string; success: boolean }>("swarm:subtask_completed", (e) => {
        const { task_id, subtask_id, success } = e.payload;
        handleSwarmEvent({ type: "subtask_completed", task_id, subtask_id, success });
      }),
      listen<{ task_id: string; subtask_id: string; error: string }>("swarm:subtask_failed", (e) => {
        const { task_id, subtask_id, error } = e.payload;
        handleSwarmEvent({ type: "subtask_failed", task_id, subtask_id, error });
      }),
      listen<{ task_id: string; subtask_id: string; passed: boolean; score: number }>("swarm:verification", (e) => {
        const { task_id, subtask_id, passed, score } = e.payload;
        handleSwarmEvent({ type: "verification", task_id, subtask_id, passed, score });
      }),
      listen<{ task_id: string; subtask_id: string; strategy: string }>("swarm:recovery", (e) => {
        const { task_id, subtask_id, strategy } = e.payload;
        handleSwarmEvent({ type: "recovery", task_id, subtask_id, strategy });
      }),
      listen<{ task_id: string; success: boolean }>("swarm:task_completed", (e) => {
        const { task_id, success } = e.payload;
        handleSwarmEvent({ type: "task_completed", task_id, success });
      }),
    ];

    return () => {
      listeners.forEach((p) => p.then((fn) => fn()));
    };
  }, [handleSwarmEvent, setInitialized]);

  const toggleTask = (taskId: string) => {
    setExpandedTasks((prev) => {
      const next = new Set(prev);
      if (next.has(taskId)) {
        next.delete(taskId);
      } else {
        next.add(taskId);
      }
      return next;
    });
    setActiveTask(taskId);
  };



  const activeTasks = tasks.filter(
    (t) => t.status !== "Completed" && t.status !== "Failed"
  );
  const completedTasks = tasks.filter(
    (t) => t.status === "Completed" || t.status === "Failed"
  );

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
              <Bot size={18} className="text-blue-400" />
              <span className="font-medium text-white/90">Agent Swarm</span>
              {activeTasks.length > 0 && (
                <span className="px-2 py-0.5 rounded-full bg-blue-500/20 text-blue-300 text-xs">
                  {activeTasks.length} active
                </span>
              )}
            </div>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg hover:bg-white/10 transition-colors"
            >
              <X size={18} className="text-white/60" />
            </button>
          </div>

          {/* Status Bar */}
          {!isInitialized && (
            <div className="px-4 py-2 border-b border-white/10 bg-white/5">
              <p className="text-xs text-white/50 flex items-center gap-2">
                <span className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
                Auto-activates for complex tasks
              </p>
            </div>
          )}

          {/* Active Tasks */}
          <div className="flex-1 overflow-y-auto">
            {activeTasks.length === 0 && completedTasks.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-64 text-white/40">
                <Bot size={48} className="mb-4 opacity-30" />
                <p className="text-sm">No active tasks</p>
                <p className="text-xs mt-1 opacity-60">
                  Complex tasks will appear here
                </p>
              </div>
            ) : (
              <>
                {/* Active Tasks Section */}
                {activeTasks.length > 0 && (
                  <div className="p-3">
                    <h3 className="text-xs font-medium text-white/50 uppercase tracking-wider mb-2 px-1">
                      Active Tasks
                    </h3>
                    <div className="space-y-2">
                      {activeTasks.map((task) => (
                        <TaskCard
                          key={task.id}
                          task={task}
                          isExpanded={expandedTasks.has(task.id)}
                          onToggle={() => toggleTask(task.id)}
                        />
                      ))}
                    </div>
                  </div>
                )}

                {/* Completed Tasks Section */}
                {completedTasks.length > 0 && (
                  <div className="p-3 border-t border-white/10">
                    <div className="flex items-center justify-between mb-2 px-1">
                      <h3 className="text-xs font-medium text-white/50 uppercase tracking-wider">
                        Completed
                      </h3>
                      <button
                        onClick={clearCompleted}
                        className="text-xs text-white/40 hover:text-white/60 flex items-center gap-1"
                      >
                        <Trash2 size={12} />
                        Clear
                      </button>
                    </div>
                    <div className="space-y-2">
                      {completedTasks.slice(0, 5).map((task) => (
                        <TaskCard
                          key={task.id}
                          task={task}
                          isExpanded={expandedTasks.has(task.id)}
                          onToggle={() => toggleTask(task.id)}
                          isCompleted
                        />
                      ))}
                    </div>
                  </div>
                )}
              </>
            )}
          </div>

          {/* Footer Stats */}
          <div className="px-4 py-3 border-t border-white/10 bg-white/5">
            <div className="flex items-center justify-between text-xs text-white/40">
              <span>Total: {tasks.length} tasks</span>
              <span>Active: {activeTasks.length}</span>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

interface TaskCardProps {
  task: SwarmTask;
  isExpanded: boolean;
  onToggle: () => void;
  isCompleted?: boolean;
}

function TaskCard({ task, isExpanded, onToggle, isCompleted }: TaskCardProps) {
  const completedCount = task.subtasks.filter((st) => st.status === "Completed").length;
  const progress = task.subtasks.length > 0 
    ? (completedCount / task.subtasks.length) * 100 
    : 0;

  return (
    <motion.div
      layout
      className={`rounded-xl border overflow-hidden ${
        isCompleted
          ? "bg-white/5 border-white/5"
          : "bg-white/5 border-white/10"
      }`}
    >
      {/* Task Header */}
      <button
        onClick={onToggle}
        className="w-full px-3 py-2.5 flex items-center gap-2 hover:bg-white/5 transition-colors"
      >
        {isExpanded ? (
          <ChevronUp size={14} className="text-white/40" />
        ) : (
          <ChevronDown size={14} className="text-white/40" />
        )}
        
        <div className="flex-1 text-left">
          <p className="text-sm text-white/80 truncate">
            {task.description.slice(0, 50)}
            {task.description.length > 50 ? "..." : ""}
          </p>
          <div className="flex items-center gap-2 mt-1">
            <span
              className={`text-[10px] px-1.5 py-0.5 rounded-full ${
                task.status === "Completed"
                  ? "bg-green-500/20 text-green-300"
                  : task.status === "Failed"
                  ? "bg-red-500/20 text-red-300"
                  : "bg-blue-500/20 text-blue-300"
              }`}
            >
              {task.status}
            </span>
            <span className="text-[10px] text-white/40">
              {completedCount}/{task.subtasks.length} steps
            </span>
          </div>
        </div>

        {!isCompleted && (
          <div className="w-16 h-1 bg-white/10 rounded-full overflow-hidden">
            <motion.div
              initial={{ width: 0 }}
              animate={{ width: `${progress}%` }}
              className="h-full bg-blue-400 rounded-full"
            />
          </div>
        )}
      </button>

      {/* Expanded Subtasks */}
      <AnimatePresence>
        {isExpanded && task.subtasks.length > 0 && (
          <motion.div
            initial={{ height: 0 }}
            animate={{ height: "auto" }}
            exit={{ height: 0 }}
            className="overflow-hidden border-t border-white/5"
          >
            <div className="p-2 space-y-1">
              {task.subtasks.map((subtask, idx) => (
                <div
                  key={subtask.id}
                  className="flex items-center gap-2 px-2 py-1.5 rounded-lg bg-white/5"
                >
                  <span className="text-[10px] text-white/30 w-4">{idx + 1}</span>
                  {statusIcons[subtask.status]}
                  <span className="flex-1 text-xs text-white/60 truncate">
                    {subtask.description.slice(0, 40)}
                  </span>
                  <span
                    className={`text-[9px] px-1.5 py-0.5 rounded border ${
                      agentColors[subtask.agent_type]
                    }`}
                  >
                    {subtask.agent_type}
                  </span>
                </div>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}
