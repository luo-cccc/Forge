import { Extension, type RawCommands } from "@tiptap/core";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { EditorSemanticLint } from "../protocol";

interface SemanticLintMeta {
  type: "set" | "clear";
  lint?: EditorSemanticLint;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    semanticLint: {
      setSemanticLint: (lint: EditorSemanticLint) => ReturnType;
      clearSemanticLint: () => ReturnType;
    };
  }
}

export const semanticLintPluginKey = new PluginKey<EditorSemanticLint | null>("semanticLint");

function createLintDecoration(doc: ProseMirrorNode, lint: EditorSemanticLint): DecorationSet {
  const from = Math.max(0, Math.min(lint.from, doc.content.size));
  const to = Math.max(from, Math.min(lint.to, doc.content.size));
  if (from === to) return DecorationSet.empty;

  return DecorationSet.create(doc, [
    Decoration.inline(from, to, {
      class: `semantic-lint semantic-lint-${lint.severity}`,
      title: lint.message,
    }),
  ]);
}

const SemanticLint = Extension.create({
  name: "semanticLint",

  addCommands() {
    return {
      setSemanticLint:
        (lint: EditorSemanticLint) =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(semanticLintPluginKey, { type: "set", lint } satisfies SemanticLintMeta);
            dispatch(tr);
          }
          return true;
        },
      clearSemanticLint:
        () =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(semanticLintPluginKey, { type: "clear" } satisfies SemanticLintMeta);
            dispatch(tr);
          }
          return true;
        },
    } satisfies Partial<RawCommands>;
  },

  addProseMirrorPlugins() {
    return [
      new Plugin<EditorSemanticLint | null>({
        key: semanticLintPluginKey,
        state: {
          init: () => null,
          apply(tr, value) {
            const meta = tr.getMeta(semanticLintPluginKey) as SemanticLintMeta | undefined;
            if (meta?.type === "clear") return null;
            if (meta?.type === "set" && meta.lint) return meta.lint;
            if (tr.docChanged) return null;
            return value;
          },
        },
        props: {
          decorations(state) {
            const lint = semanticLintPluginKey.getState(state);
            if (!lint) return DecorationSet.empty;
            return createLintDecoration(state.doc, lint);
          },
        },
      }),
    ];
  },
});

export default SemanticLint;
