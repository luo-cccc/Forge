import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { TextPatch, PatchStatus } from "../protocol";

export interface PatchDecoration {
  from: number;
  to: number;
  patch: TextPatch;
  status: PatchStatus;
}

export const patchMarkPluginKey = new PluginKey<PatchDecoration[]>("patchMark");

function buildDecorations(decos: PatchDecoration[]): DecorationSet {
  const decs: Decoration[] = [];
  for (const d of decos) {
    if (d.status === "accepted" || d.status === "rejected") continue;
    const cls = d.status === "pending" ? "patch-pending" : "patch-review";
    decs.push(
      Decoration.inline(d.from, d.to, {
        class: `patch-mark ${cls}`,
        nodeName: "span",
        "data-patch-id": d.patch.id,
      }),
    );
  }
  const editorEl = document.querySelector(".ProseMirror");
  const view = (editorEl as any)?.pmViewDesc?.node;
  return DecorationSet.create(view, decs);
}

const PatchMark = Extension.create({
  name: "patchMark",

  addProseMirrorPlugins() {
    let decorations: PatchDecoration[] = [];

    return [
      new Plugin<PatchDecoration[]>({
        key: patchMarkPluginKey,
        state: {
          init: () => [],
          apply(tr, old) {
            const meta = tr.getMeta(patchMarkPluginKey) as PatchDecoration[] | undefined;
            if (meta) {
              decorations = meta;
              return meta;
            }
            return old;
          },
        },
        props: {
          decorations() {
            return buildDecorations(decorations);
          },
        },
      }),
    ];
  },
});

export default PatchMark;
