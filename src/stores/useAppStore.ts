import { useEffect } from 'react';
import { create } from 'zustand';
import { ipc } from '../ipc';
import type { Folder, Rule, Schedule, Settings, SubfolderConfig } from '../ipc/types';

interface AppState {
  // Data
  folders: Folder[];
  rules: Rule[];
  settings: Settings | null;
  isLoading: boolean;
  error: string | null;
  clearError: () => void;
  reportError: (error: string) => void;

  // Actions
  loadFolders: () => Promise<void>;
  loadRules: () => Promise<void>;
  loadSettings: () => Promise<void>;
  addFolder: () => Promise<void>;
  removeFolder: (id: string) => Promise<void>;
  updateFolderName: (id: string, name: string) => Promise<void>;
  updateFolderMode: (
    id: string,
    mode: 'single' | 'auto' | 'scheduled',
    disableSubfolders?: boolean,
  ) => Promise<boolean>;
  updateFolderSchedule: (id: string, schedule: Schedule) => Promise<boolean>;
  updateFolderSubfolders: (id: string, config: SubfolderConfig) => Promise<boolean>;
  updateFolderExclusions: (
    id: string,
    excludeFolders: string[],
    excludePatterns: string[],
  ) => Promise<boolean>;
  updateFolderConfiguration: (
    id: string,
    config: SubfolderConfig,
    excludeFolders: string[],
    excludePatterns: string[],
    ruleIds: string[],
    confirmRecursive?: boolean,
  ) => Promise<boolean>;
  updateFolderRules: (
    id: string,
    currentRuleIds: string[],
    nextRuleIds: string[],
  ) => Promise<boolean>;
  applyRecommendedRules: (id: string) => Promise<boolean>;
  createRule: (rule: Rule) => Promise<{ success: boolean; error?: string }>;
  updateRule: (id: string, updates: Partial<Rule>) => Promise<{ success: boolean; error?: string }>;
  deleteRule: (id: string) => Promise<void>;
  toggleRule: (id: string) => Promise<void>;
  updateSettings: (partial: Partial<Settings>) => Promise<boolean>;
}

export const useAppStore = create<AppState>((set, get) => ({
  folders: [],
  rules: [],
  settings: null,
  isLoading: false,
  error: null,
  clearError: () => set({ error: null }),
  reportError: (error) => set({ error }),

  loadFolders: async () => {
    const res = await ipc.folders.list();
    if (res.success && res.data) {
      set({ folders: res.data });
    } else if (!res.success && res.error !== 'USER_CANCELED_FOLDER_SELECT') {
      set({ error: res.error ?? null });
    }
  },

  loadRules: async () => {
    const res = await ipc.rules.list();
    if (res.success && res.data) {
      set({ rules: res.data });
    } else if (res.error) {
      set({ error: res.error });
    }
  },

  loadSettings: async () => {
    const res = await ipc.settings.get();
    if (res.success && res.data) {
      set({ settings: res.data });
    } else if (res.error) {
      set({ error: res.error });
    }
  },

  addFolder: async () => {
    set({ isLoading: true, error: null });
    const res = await ipc.folders.add();
    if (res.success && res.data) {
      const { folders } = get();
      const exists = folders.some((f) => f.id === res.data!.id);
      set({
        folders: exists
          ? folders.map((f) => (f.id === res.data!.id ? res.data! : f))
          : [...folders, res.data!],
        isLoading: false,
      });
    } else {
      set({ isLoading: false });
      if (res.error !== 'USER_CANCELED_FOLDER_SELECT') {
        set({ error: res.error ?? null });
      }
    }
  },

  removeFolder: async (id) => {
    const res = await ipc.folders.remove(id);
    if (res.success) {
      set({ folders: get().folders.filter((f) => f.id !== id) });
    } else {
      set({ error: res.error ?? null });
    }
  },

  updateFolderName: async (id, name) => {
    const res = await ipc.folders.updateName(id, name);
    if (res.success && res.data) {
      set({
        folders: get().folders.map((folder) => (folder.id === id ? res.data! : folder)),
      });
    } else if (res.error) {
      set({ error: res.error });
    }
  },

  updateFolderMode: async (id, mode, disableSubfolders = false) => {
    const res = await ipc.folders.updateMode(id, mode, disableSubfolders);
    if (res.success && res.data) {
      set({ folders: get().folders.map((f) => (f.id === id ? res.data! : f)) });
      return true;
    } else if (res.error) {
      set({ error: res.error });
    }
    return false;
  },

  updateFolderSchedule: async (id, schedule) => {
    const res = await ipc.folders.updateSchedule(id, schedule);
    if (res.success && res.data) {
      set({ folders: get().folders.map((f) => (f.id === id ? res.data! : f)) });
      return true;
    }
    if (res.error) {
      set({ error: res.error });
    }
    return false;
  },

  updateFolderSubfolders: async (id, config) => {
    const res = await ipc.folders.updateSubfoldersConfig(id, config);
    if (res.success && res.data) {
      set({ folders: get().folders.map((folder) => (folder.id === id ? res.data! : folder)) });
      return true;
    }
    if (res.error) set({ error: res.error });
    return false;
  },

  updateFolderExclusions: async (id, excludeFolders, excludePatterns) => {
    const res = await ipc.folders.updateExclusions(id, excludeFolders, excludePatterns);
    if (res.success && res.data) {
      set({ folders: get().folders.map((folder) => (folder.id === id ? res.data! : folder)) });
      return true;
    }
    if (res.error) set({ error: res.error });
    return false;
  },

  updateFolderConfiguration: async (
    id,
    config,
    excludeFolders,
    excludePatterns,
    ruleIds,
    confirmRecursive = false,
  ) => {
    const res = await ipc.folders.updateConfiguration(
      id,
      config,
      excludeFolders,
      excludePatterns,
      ruleIds,
      confirmRecursive,
    );
    if (res.success && res.data) {
      set({ folders: get().folders.map((folder) => (folder.id === id ? res.data! : folder)) });
      return true;
    }
    if (res.error) set({ error: res.error });
    return false;
  },

  updateFolderRules: async (id, currentRuleIds, nextRuleIds) => {
    const added = nextRuleIds.filter((ruleId) => !currentRuleIds.includes(ruleId));
    const removed = currentRuleIds.filter((ruleId) => !nextRuleIds.includes(ruleId));
    let updated: Folder | undefined;

    if (added.length > 0) {
      const res = await ipc.folders.assignRules(id, added);
      if (!res.success) {
        if (res.error) set({ error: res.error });
        await get().loadFolders();
        return false;
      }
      updated = res.data;
    }
    if (removed.length > 0) {
      const res = await ipc.folders.unassignRules(id, removed);
      if (!res.success) {
        if (res.error) set({ error: res.error });
        await get().loadFolders();
        return false;
      }
      updated = res.data;
    }
    if (updated) {
      set({ folders: get().folders.map((folder) => (folder.id === id ? updated! : folder)) });
    }
    return true;
  },

  applyRecommendedRules: async (id) => {
    const res = await ipc.folders.applyRecommendedRules(id);
    if (res.success && res.data) {
      set({ folders: get().folders.map((folder) => (folder.id === id ? res.data! : folder)) });
      await get().loadRules();
      return true;
    }
    if (res.error) set({ error: res.error });
    return false;
  },

  createRule: async (rule) => {
    const res = await ipc.rules.create(rule);
    if (res.success) {
      await get().loadRules();
      return { success: true };
    }
    if (res.error) set({ error: res.error });
    return { success: false, error: res.error };
  },

  updateRule: async (id, updates) => {
    const res = await ipc.rules.update(id, updates);
    if (res.success) {
      await get().loadRules();
      return { success: true };
    }
    if (res.error) set({ error: res.error });
    return { success: false, error: res.error };
  },

  deleteRule: async (id) => {
    const res = await ipc.rules.delete(id);
    if (res.success) {
      await get().loadRules();
    } else if (res.error) {
      set({ error: res.error });
    }
  },

  toggleRule: async (id) => {
    const res = await ipc.rules.toggle(id);
    if (res.success) {
      await get().loadRules();
    } else if (res.error) {
      set({ error: res.error });
    }
  },

  updateSettings: async (partial) => {
    const res = await ipc.settings.set(partial);
    if (res.success) {
      await get().loadSettings();
      return true;
    } else if (res.error) {
      set({ error: res.error });
    }
    return false;
  },
}));

// Re-export for convenience
export function useFolders() {
  return useAppStore((s) => s.folders);
}

export function useRules() {
  return useAppStore((s) => s.rules);
}

// Auto-load folders and rules on first use.
let initialized = false;
export function useAutoInit() {
  useEffect(() => {
    if (initialized) return;
    initialized = true;
    useAppStore.getState().loadFolders();
    useAppStore.getState().loadRules();
  }, []);
}
