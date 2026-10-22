import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

// Mirrors UpdateInfo in src-tauri/src/update.rs.
export interface UpdateInfo {
  version: string;
  notes: string;
}

/** Checks GitHub Releases and downloads a newer version if there is one. */
export const checkForUpdate = () => invoke<UpdateInfo | null>("check_for_update");
/** Installs the downloaded update and relaunches Parley. */
export const installUpdate = () => invoke<void>("install_update");
/** Fires once an update has been downloaded and verified. */
export const onUpdateReady = (cb: (u: UpdateInfo) => void): Promise<UnlistenFn> =>
  listen<UpdateInfo>("update-ready", (e) => cb(e.payload));
