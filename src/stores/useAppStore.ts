import { useEffect } from 'react';
import { create } from 'zustand';
import { ipc } from '../ipc';
import type { Folder, Rule, Settings } from '../ipc/types';

interface AppState {
  // Data
  folders: Folder[];
  rules: Rule[];
  settings: Settings | null;
  isLoading: boolean;
  error: string | null;

  // Actions
  loadFolders: () => Promise<void>;
  loadRules: () => Promise<void>;
  loadSettings: () => Promise<void>;
  addFolder: () => Promise<void>;
  removeFolder: (id: string) => Promise<void>;
  updateFolderName: (id: string, name: string) => Promise<void>;
  updateFolderMode: (id: string, mode: 'single' | 'auto' | 'scheduled') => Promise<void>;
  createRule: (rule: Rule) => Promise<{ success: boolean; error?: string }>;
  deleteRule: (id: string) => Promise<void>;
  toggleRule: (id: string) => Promise<void>;
  updateSettings: (partial: Partial<Settings>) => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => ({
  folders: [],
  rules: [],
  settings: null,
  isLoading: false,
  error: null,

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
    }
  },

  loadSettings: async () => {
    const res = await ipc.settings.get();
    if (res.success && res.data) {
      set({ settings: res.data });
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

  updateFolderMode: async (id, mode) => {
    const res = await ipc.folders.updateMode(id, mode);
    if (res.success && res.data) {
      set({ folders: get().folders.map((f) => (f.id === id ? res.data! : f)) });
    } else if (res.error) {
      set({ error: res.error });
    }
  },

  createRule: async (rule) => {
    const res = await ipc.rules.create(rule);
    if (res.success) {
      await get().loadRules();
      return { success: true };
    }
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
    }
  },

  updateSettings: async (partial) => {
    const res = await ipc.settings.set(partial);
    if (res.success) {
      await get().loadSettings();
    } else if (res.error) {
      set({ error: res.error });
    }
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
