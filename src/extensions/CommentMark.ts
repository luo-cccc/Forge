import { Mark, type RawCommands } from "@tiptap/core";

interface CommentMarkOptions {
  HTMLAttributes: Record<string, unknown>;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    commentMark: {
      setComment: (commentId: string) => ReturnType;
      unsetComment: () => ReturnType;
    };
  }
}

const CommentMark = Mark.create<CommentMarkOptions>({
  name: "comment",

  addOptions() {
    return { HTMLAttributes: {} };
  },

  addAttributes() {
    return {
      commentId: {
        default: null,
        parseHTML: (el) => el.getAttribute("data-comment-id"),
        renderHTML: (attrs) => ({
          "data-comment-id": attrs.commentId,
        }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "mark.comment-mark" }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "mark",
      {
        class:
          "comment-mark bg-yellow-500/30 text-yellow-100 rounded-sm px-0.5 cursor-pointer border-b-2 border-yellow-500/60 transition-colors hover:bg-yellow-500/50",
        ...HTMLAttributes,
      },
      0,
    ];
  },

  addCommands() {
    return {
      setComment:
        (commentId: string) =>
        ({ commands }) =>
          commands.setMark(this.name, { commentId }),
      unsetComment:
        () =>
        ({ commands }) =>
          commands.unsetMark(this.name),
    } satisfies Partial<RawCommands>;
  },
});

export default CommentMark;
