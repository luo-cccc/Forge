import { Extension, type RawCommands } from "@tiptap/core";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { EditorEntityCard } from "../protocol";

interface EntityAnchorMeta {
  type: "set" | "clear";
  cards?: EditorEntityCard[];
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    entityAnchor: {
      setEntityAnchors: (cards: EditorEntityCard[]) => ReturnType;
      clearEntityAnchors: () => ReturnType;
    };
  }
}

export const entityAnchorPluginKey = new PluginKey<EditorEntityCard[]>("entityAnchor");

function buildDecorations(doc: ProseMirrorNode, cards: EditorEntityCard[]): DecorationSet {
  const decos: Decoration[] = [];
  const uniqueCards = cards
    .filter((card) => card.keyword.trim().length > 0)
    .filter(
      (card, index, arr) =>
        arr.findIndex((candidate) => candidate.keyword === card.keyword) === index,
    );

  doc.descendants((node, pos) => {
    if (!node.isText || !node.text) return;

    for (const card of uniqueCards) {
      let searchFrom = 0;
      while (searchFrom < node.text.length) {
        const found = node.text.indexOf(card.keyword, searchFrom);
        if (found === -1) break;
        const from = pos + found;
        const to = from + card.keyword.length;
        decos.push(
          Decoration.inline(from, to, {
            class: "entity-anchor",
            title: card.keyword,
            "data-entity-keyword": card.keyword,
          }),
        );
        searchFrom = found + card.keyword.length;
      }
    }
  });

  return DecorationSet.create(doc, decos);
}

const EntityAnchor = Extension.create({
  name: "entityAnchor",

  addCommands() {
    return {
      setEntityAnchors:
        (cards: EditorEntityCard[]) =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(entityAnchorPluginKey, {
              type: "set",
              cards,
            } satisfies EntityAnchorMeta);
            dispatch(tr);
          }
          return true;
        },
      clearEntityAnchors:
        () =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            tr.setMeta(entityAnchorPluginKey, { type: "clear" } satisfies EntityAnchorMeta);
            dispatch(tr);
          }
          return true;
        },
    } satisfies Partial<RawCommands>;
  },

  addProseMirrorPlugins() {
    return [
      new Plugin<EditorEntityCard[]>({
        key: entityAnchorPluginKey,
        state: {
          init: () => [],
          apply(tr, value) {
            const meta = tr.getMeta(entityAnchorPluginKey) as EntityAnchorMeta | undefined;
            if (meta?.type === "clear") return [];
            if (meta?.type === "set") return meta.cards ?? [];
            return value;
          },
        },
        props: {
          decorations(state) {
            const cards = entityAnchorPluginKey.getState(state);
            if (!cards?.length) return DecorationSet.empty;
            return buildDecorations(state.doc, cards);
          },
        },
      }),
    ];
  },
});

export default EntityAnchor;
