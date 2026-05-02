import { Extension, type RawCommands } from "@tiptap/core";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

export interface InlinePreviewState {
  id: string;
  proposalId?: string;
  kind: "text.insert" | "text.replace";
  from: number;
  to: number;
  text: string;
  chapter: string;
  revision: string;
}

interface InlinePreviewMeta {
  type: "set" | "clear";
  preview?: InlinePreviewState;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    inlinePreview: {
      setInlinePreview: (preview: InlinePreviewState) => ReturnType;
      clearInlinePreview: () => ReturnType;
      applyInlinePreview: () => ReturnType;
    };
  }
}

export const inlinePreviewPluginKey = new PluginKey<InlinePreviewState | null>("inlinePreview");

function createPreviewWidget(preview: InlinePreviewState): HTMLElement {
  const span = document.createElement("span");
  span.className = `inline-ai-preview inline-ai-preview-${preview.kind === "text.replace" ? "replacement" : "insert"}`;
  span.dataset.chapter = preview.chapter;
  span.textContent = preview.kind === "text.replace" ? ` ${preview.text}` : preview.text;
  return span;
}

function createInlinePreviewDecorations(
  doc: ProseMirrorNode,
  preview: InlinePreviewState,
): DecorationSet {
  const decorations: Decoration[] = [];
  const from = Math.max(0, Math.min(preview.from, doc.content.size));
  const to = Math.max(from, Math.min(preview.to, doc.content.size));

  if (preview.kind === "text.replace" && from < to) {
    decorations.push(
      Decoration.inline(from, to, {
        class: "inline-ai-preview-target",
        nodeName: "span",
      }),
    );
  }

  decorations.push(
    Decoration.widget(to, () => createPreviewWidget(preview), {
      side: 1,
      key: `inline-preview-${preview.id}`,
    }),
  );

  return DecorationSet.create(doc, decorations);
}

const InlinePreview = Extension.create({
  name: "inlinePreview",

  addCommands() {
    return {
      setInlinePreview:
        (preview: InlinePreviewState) =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(inlinePreviewPluginKey, { type: "set", preview } satisfies InlinePreviewMeta);
            dispatch(tr);
          }
          return true;
        },
      clearInlinePreview:
        () =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(inlinePreviewPluginKey, { type: "clear" } satisfies InlinePreviewMeta);
            dispatch(tr);
          }
          return true;
        },
      applyInlinePreview:
        () =>
        ({ editor }) => {
          const preview = inlinePreviewPluginKey.getState(editor.state);
          if (!preview) return false;

          const target =
            preview.kind === "text.replace"
              ? { from: preview.from, to: preview.to }
              : preview.from;

          editor
            .chain()
            .focus()
            .clearInlinePreview()
            .insertContentAt(target, preview.text)
            .setTextSelection(preview.from + preview.text.length)
            .run();
          return true;
        },
    } satisfies Partial<RawCommands>;
  },

  addProseMirrorPlugins() {
    return [
      new Plugin<InlinePreviewState | null>({
        key: inlinePreviewPluginKey,
        state: {
          init: () => null,
          apply(tr, value) {
            const meta = tr.getMeta(inlinePreviewPluginKey) as InlinePreviewMeta | undefined;
            if (meta?.type === "clear") return null;
            if (meta?.type === "set" && meta.preview) return meta.preview;
            if (tr.docChanged) return null;
            return value;
          },
        },
        props: {
          decorations(state) {
            const preview = inlinePreviewPluginKey.getState(state);
            if (!preview) return DecorationSet.empty;
            return createInlinePreviewDecorations(state.doc, preview);
          },
        },
      }),
    ];
  },
});

export default InlinePreview;
