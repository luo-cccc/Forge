import { Extension, type RawCommands } from "@tiptap/core";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { WriterOperation } from "../protocol";

export interface GhostTextState {
  requestId: string;
  proposalId?: string;
  operation?: WriterOperation;
  position: number;
  text: string;
  intent?: string;
  candidates: GhostTextCandidate[];
  activeIndex: number;
}

export interface GhostTextCandidate {
  id: string;
  label: string;
  text: string;
  evidence?: { source: string; snippet: string }[];
}

interface GhostTextMeta {
  type: "set" | "append" | "clear" | "next";
  requestId?: string;
  proposalId?: string;
  operation?: WriterOperation;
  position?: number;
  text?: string;
  intent?: string;
  candidates?: GhostTextCandidate[];
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    ghostText: {
      setGhostText: (state: GhostTextState) => ReturnType;
      appendGhostText: (requestId: string, position: number, text: string) => ReturnType;
      clearGhostText: () => ReturnType;
      nextGhostCandidate: () => ReturnType;
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
      const current = state.candidates[state.activeIndex] ?? {
        label: "A",
        text: state.text,
      };
      span.dataset.intent = state.intent ?? "";
      const evidenceSource = current.evidence?.[0]?.source ?? "";
      let badge = "";
      if (state.candidates.length > 1) {
        const extra = evidenceSource ? ` · ${evidenceSource}` : "";
        badge = `[${current.label} · ${state.activeIndex + 1}/${state.candidates.length}${extra}]`;
      } else if (evidenceSource) {
        badge = `[${evidenceSource}]`;
      }
      span.textContent = badge ? ` ${current.text}  ${badge}` : state.text;
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
      nextGhostCandidate:
        () =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(ghostTextPluginKey, { type: "next" } satisfies GhostTextMeta);
            dispatch(tr);
          }
          return true;
        },
      acceptGhostText:
        () =>
        ({ editor }) => {
          const state = ghostTextPluginKey.getState(editor.state);
          if (!state?.text) return false;
          const candidate = state.candidates[state.activeIndex];
          const text = candidate?.text ?? state.text;

          editor
            .chain()
            .focus()
            .clearGhostText()
            .insertContentAt(state.position, text)
            .setTextSelection(state.position + text.length)
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
              const candidates = meta.candidates?.length
                ? meta.candidates
                : [{ id: "a", label: "A", text: meta.text ?? "" }];
              return {
                requestId: meta.requestId,
                proposalId: meta.proposalId,
                operation: meta.operation,
                position: meta.position,
                text: candidates[0]?.text ?? meta.text ?? "",
                intent: meta.intent,
                candidates,
                activeIndex: 0,
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
                candidates: value.candidates.length
                  ? value.candidates.map((candidate, index) =>
                      index === value.activeIndex
                        ? { ...candidate, text: `${candidate.text}${meta.text ?? ""}` }
                        : candidate,
                    )
                  : [],
              };
            }

            if (meta?.type === "next" && value?.candidates.length) {
              const activeIndex = (value.activeIndex + 1) % value.candidates.length;
              return {
                ...value,
                activeIndex,
                text: value.candidates[activeIndex]?.text ?? value.text,
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
