import { Extension, type RawCommands } from "@tiptap/core";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

export interface GhostTextState {
  requestId: string;
  position: number;
  text: string;
}

interface GhostTextMeta {
  type: "set" | "append" | "clear";
  requestId?: string;
  position?: number;
  text?: string;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    ghostText: {
      setGhostText: (state: GhostTextState) => ReturnType;
      appendGhostText: (requestId: string, position: number, text: string) => ReturnType;
      clearGhostText: () => ReturnType;
      acceptGhostText: () => ReturnType;
    };
  }
}

export const ghostTextPluginKey = new PluginKey<GhostTextState | null>("ghostText");

function createGhostDecoration(doc: ProseMirrorNode, state: GhostTextState): DecorationSet {
  const widget = Decoration.widget(
    state.position,
    () => {
      const span = document.createElement("span");
      span.className = "ghost-text";
      span.textContent = state.text;
      return span;
    },
    { side: 1, key: `ghost-${state.requestId}` },
  );

  return DecorationSet.create(doc, [widget]);
}

const GhostText = Extension.create({
  name: "ghostText",

  addCommands() {
    return {
      setGhostText:
        (state: GhostTextState) =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(ghostTextPluginKey, { type: "set", ...state } satisfies GhostTextMeta);
            dispatch(tr);
          }
          return true;
        },
      appendGhostText:
        (requestId: string, position: number, text: string) =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(ghostTextPluginKey, {
              type: "append",
              requestId,
              position,
              text,
            } satisfies GhostTextMeta);
            dispatch(tr);
          }
          return true;
        },
      clearGhostText:
        () =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(ghostTextPluginKey, { type: "clear" } satisfies GhostTextMeta);
            dispatch(tr);
          }
          return true;
        },
      acceptGhostText:
        () =>
        ({ editor }) => {
          const state = ghostTextPluginKey.getState(editor.state);
          if (!state?.text) return false;

          editor
            .chain()
            .focus()
            .clearGhostText()
            .insertContentAt(state.position, state.text)
            .setTextSelection(state.position + state.text.length)
            .run();
          return true;
        },
    } satisfies Partial<RawCommands>;
  },

  addProseMirrorPlugins() {
    return [
      new Plugin<GhostTextState | null>({
        key: ghostTextPluginKey,
        state: {
          init: () => null,
          apply(tr, value) {
            const meta = tr.getMeta(ghostTextPluginKey) as GhostTextMeta | undefined;

            if (meta?.type === "clear") {
              return null;
            }

            if (meta?.type === "set" && meta.requestId && meta.position !== undefined) {
              return {
                requestId: meta.requestId,
                position: meta.position,
                text: meta.text ?? "",
              };
            }

            if (
              meta?.type === "append" &&
              value &&
              meta.requestId === value.requestId &&
              meta.position === value.position
            ) {
              return {
                ...value,
                text: `${value.text}${meta.text ?? ""}`,
              };
            }

            if (tr.docChanged || tr.selectionSet) {
              return null;
            }

            return value;
          },
        },
        props: {
          decorations(state) {
            const ghost = ghostTextPluginKey.getState(state);
            if (!ghost?.text) return DecorationSet.empty;
            return createGhostDecoration(state.doc, ghost);
          },
        },
      }),
    ];
  },
});

export default GhostText;
