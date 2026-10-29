import { create } from "zustand";

interface StreamerModeState {
  enabled: boolean;
  setEnabled: (enabled: boolean) => void;
}

export const useStreamerModeStore = create<StreamerModeState>((set) => ({
  enabled: false,
  setEnabled: (enabled) => set({ enabled }),
}));
