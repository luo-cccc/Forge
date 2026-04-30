import { Mark } from "@tiptap/core";

const AIPreviewMark = Mark.create({
  name: "aiPreview",

  parseHTML() {
    return [{ tag: "mark.ai-preview" }];
  },

  renderHTML() {
    return [
      "mark",
      {
        class:
          "ai-preview bg-accent-subtle text-accent rounded-sm px-0.5 border-b border-accent/60",
      },
      0,
    ];
  },
});

export default AIPreviewMark;
