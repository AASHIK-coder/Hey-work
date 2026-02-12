import { create } from "zustand";
import { SwarmTask, SwarmEvent, SwarmSubtask } from "../types";

interface SwarmState {
  tasks: SwarmTask[];
  activeTaskId: string | null;
  isInitialized: boolean;
  
  // Actions
  addTask: (task: SwarmTask) => void;
  updateTask: (taskId: string, updates: Partial<SwarmTask>) => void;
  updateSubtask: (taskId: string, subtaskId: string, updates: Partial<SwarmSubtask>) => void;
  removeTask: (taskId: string) => void;
  clearCompleted: () => void;
  setActiveTask: (taskId: string | null) => void;
  setInitialized: (initialized: boolean) => void;
  handleSwarmEvent: (event: SwarmEvent) => void;
}

export const useSwarmStore = create<SwarmState>((set, get) => ({
  tasks: [],
  activeTaskId: null,
  isInitialized: false,

  addTask: (task) =>
    set((state) => ({
      tasks: [task, ...state.tasks],
      activeTaskId: task.id,
    })),

  updateTask: (taskId, updates) =>
    set((state) => ({
      tasks: state.tasks.map((t) =>
        t.id === taskId ? { ...t, ...updates } : t
      ),
    })),

  updateSubtask: (taskId, subtaskId, updates) =>
    set((state) => ({
      tasks: state.tasks.map((t) => {
        if (t.id !== taskId) return t;
        return {
          ...t,
          subtasks: t.subtasks.map((st) =>
            st.id === subtaskId ? { ...st, ...updates } : st
          ),
        };
      }),
    })),

  removeTask: (taskId) =>
    set((state) => ({
      tasks: state.tasks.filter((t) => t.id !== taskId),
      activeTaskId:
        state.activeTaskId === taskId
          ? state.tasks.find((t) => t.id !== taskId)?.id || null
          : state.activeTaskId,
    })),

  clearCompleted: () =>
    set((state) => ({
      tasks: state.tasks.filter(
        (t) => t.status !== "Completed" && t.status !== "Failed"
      ),
    })),

  setActiveTask: (taskId) => set({ activeTaskId: taskId }),
  
  setInitialized: (initialized) => set({ isInitialized: initialized }),

  handleSwarmEvent: (event) => {
    const { tasks, addTask, updateTask, updateSubtask } = get();
    
    switch (event.type) {
      case "task_started": {
        const existing = tasks.find((t) => t.id === event.task_id);
        if (!existing) {
          addTask({
            id: event.task_id,
            description: "Complex task", // Would come from event
            status: "Executing",
            subtasks: [],
            created_at: new Date().toISOString(),
          });
        }
        break;
      }
      
      case "subtask_started": {
        const task = tasks.find((t) => t.id === event.task_id);
        if (task && event.subtask_id && event.agent) {
          const existingSubtask = task.subtasks.find((st) => st.id === event.subtask_id);
          if (!existingSubtask) {
            updateTask(event.task_id, {
              subtasks: [
                ...task.subtasks,
                {
                  id: event.subtask_id,
                  description: "Subtask", // Would come from event
                  agent_type: event.agent,
                  status: "Executing",
                  dependencies: [],
                  retry_count: 0,
                  max_retries: 3,
                },
              ],
            });
          } else {
            updateSubtask(event.task_id, event.subtask_id, { status: "Executing" });
          }
        }
        break;
      }
      
      case "subtask_completed": {
        if (event.subtask_id) {
          updateSubtask(event.task_id, event.subtask_id, {
            status: "Completed",
            output: event.output,
          });
        }
        break;
      }
      
      case "subtask_failed": {
        if (event.subtask_id) {
          updateSubtask(event.task_id, event.subtask_id, {
            status: "Failed",
            error: event.error,
          });
        }
        break;
      }
      
      case "verification": {
        if (event.subtask_id) {
          updateSubtask(event.task_id, event.subtask_id, {
            status: event.passed ? "Completed" : "NeedsRetry",
          });
        }
        break;
      }
      
      case "recovery": {
        if (event.subtask_id) {
          updateSubtask(event.task_id, event.subtask_id, {
            status: "NeedsRetry",
          });
        }
        break;
      }
      
      case "task_completed": {
        updateTask(event.task_id, {
          status: event.success ? "Completed" : "Failed",
          completed_at: new Date().toISOString(),
        });
        break;
      }
    }
  },
}));
