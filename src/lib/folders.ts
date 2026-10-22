import { invoke } from "@tauri-apps/api/core";

// Mirrors store::Folder in src-tauri/src/store.rs.
export interface Folder {
  id: string;
  name: string;
  meetingCount: number;
}

export const listFolders = () => invoke<Folder[]>("list_folders");
export const createFolder = (name: string) => invoke<Folder>("create_folder", { name });
export const renameFolder = (id: string, name: string) => invoke<void>("rename_folder", { id, name });
export const deleteFolder = (id: string) => invoke<void>("delete_folder", { id });
/** Files a meeting in a folder, or unfiles it with `null`. */
export const setMeetingFolder = (meetingId: string, folderId: string | null) =>
  invoke<void>("set_meeting_folder", { meetingId, folderId });

/** Mirrors FOLDER_NAME_MAX in store.rs. */
export const FOLDER_NAME_MAX = 60;

/** The name the backend will store, or an error to show before sending it. */
export function checkFolderName(name: string, existing: Folder[], exceptId?: string): { name: string } | { error: string } {
  const cleaned = name.split(/\s+/).filter(Boolean).join(" ");
  if (!cleaned) return { error: "Give the folder a name." };
  if ([...cleaned].length > FOLDER_NAME_MAX) return { error: `Folder names can be up to ${FOLDER_NAME_MAX} characters.` };
  const lower = cleaned.toLowerCase();
  if (existing.some((f) => f.id !== exceptId && f.name.toLowerCase() === lower)) {
    return { error: `A folder called "${cleaned}" already exists.` };
  }
  return { name: cleaned };
}
