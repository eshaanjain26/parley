import { describe, expect, it } from "vitest";
import { FOLDER_NAME_MAX, Folder, checkFolderName } from "./folders";

const existing: Folder[] = [{ id: "f1", name: "Acme", meetingCount: 2 }];

describe("checkFolderName", () => {
  it("collapses whitespace the way the backend does", () => {
    expect(checkFolderName("  Q4   planning ", existing)).toEqual({ name: "Q4 planning" });
  });

  it("rejects empty, too long and duplicate names (ignoring case)", () => {
    expect(checkFolderName("   ", existing)).toHaveProperty("error");
    expect(checkFolderName("x".repeat(FOLDER_NAME_MAX + 1), existing)).toHaveProperty("error");
    expect(checkFolderName("acme", existing)).toEqual({ error: 'A folder called "acme" already exists.' });
  });

  it("lets a folder keep its own name when renaming", () => {
    expect(checkFolderName("ACME", existing, "f1")).toEqual({ name: "ACME" });
  });
});
