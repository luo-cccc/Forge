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
          "ai-preview bg-emerald-900/40 text-emerald-200 rounded-sm px-0.5 border-b border-emerald-700/50",
      },
      0,
    ];
  },
});

export default AIPreviewMark;
