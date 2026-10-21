import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import DOMPurify from "dompurify";
import { marked } from "marked";
import type { Meeting } from "./meetings";

// Mirrors Template in src-tauri/src/enhance.rs.
export interface Template {
  id: string;
  name: string;
  instructions: string;
  builtIn: boolean;
}

export interface EnhanceDelta {
  meetingId: string;
  text: string;
}

export const listTemplates = () => invoke<Template[]>("list_templates");
export const saveTemplate = (template: Template) => invoke<Template>("save_template", { template });
export const deleteTemplate = (id: string) => invoke<void>("delete_template", { id });
export const enhanceMeeting = (meetingId: string, templateId: string) =>
  invoke<Meeting>("enhance_meeting", { meetingId, templateId });
export const cancelEnhance = () => invoke<void>("cancel_enhance");
export const onEnhanceDelta = (cb: (d: EnhanceDelta) => void): Promise<UnlistenFn> =>
  listen<EnhanceDelta>("enhance-delta", (e) => cb(e.payload));

/** Model output is untrusted text: render its Markdown, then strip anything executable. */
export function renderMarkdown(markdown: string): string {
  const html = marked.parse(markdown, { async: false, gfm: true, breaks: false });
  return DOMPurify.sanitize(html, { USE_PROFILES: { html: true } });
}

/** A blank custom template the user can fill in, prefilled with a useful outline. */
export function newTemplate(): Template {
  return {
    id: "",
    name: "",
    instructions: "## Summary\n\n## Decisions\n\n## Action items\n- [ ] Task (owner, due date)",
    builtIn: false,
  };
}
