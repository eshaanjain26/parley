import { describe, expect, it } from "vitest";
import { sectionsOf } from "./TemplatesSection";

describe("template sections", () => {
  it("lists the ## headings in order and ignores the guidance under them", () => {
    expect(sectionsOf("## Summary\nTwo sentences.\n\n## Action items\n- [ ] Task\n### Not a section")).toEqual(["Summary", "Action items"]);
    expect(sectionsOf("No headings here")).toEqual([]);
  });
});
