import { useRef, useEffect, useState, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Commands } from "../protocol";

interface GraphEntity {
  id: string;
  name: string;
  category: string;
  description: string;
}
interface GraphRelationship {
  source: string;
  target: string;
  label: string;
}
interface GraphChapter {
  title: string;
  summary: string;
  status: string;
  word_count: number;
}
interface GraphData {
  entities: GraphEntity[];
  relationships: GraphRelationship[];
  chapters: GraphChapter[];
}
interface KnowledgeNode {
  id: string;
  kind: string;
  label: string;
  sourceRef: string;
  sourceRevision?: string | null;
  sourceKind?: string | null;
  chunkIndex?: number | null;
  archived?: boolean;
  keywords: string[];
  summary: string;
}
interface KnowledgeEdge {
  from: string;
  to: string;
  relation: string;
  evidenceRef: string;
}
interface KnowledgeSourceRevision {
  revision: string;
  nodeCount: number;
  chunkIndexes: number[];
  active?: boolean;
}
interface KnowledgeSourceHistory {
  sourceRef: string;
  sourceKind: string;
  revisions: KnowledgeSourceRevision[];
  nodeCount: number;
  chunkCount: number;
  latestSummary: string;
}
interface KnowledgeGraphData {
  projectId: string;
  nodes: KnowledgeNode[];
  edges: KnowledgeEdge[];
  sourceHistory?: KnowledgeSourceHistory[];
  sourceCount: number;
}
interface KnowledgeSourceCompareRevision {
  revision: string;
  active: boolean;
  nodeCount: number;
  chunkCount: number;
  chunkIndexes: number[];
  keywords: string[];
  summary: string;
}
interface KnowledgeSourceCompare {
  sourceRef: string;
  sourceKind: string;
  activeRevision?: string | null;
  revisions: KnowledgeSourceCompareRevision[];
  addedKeywords: string[];
  removedKeywords: string[];
  sharedKeywords: string[];
  addedSummary: string[];
  removedSummary: string[];
  evidenceRefs: string[];
}

interface Node2D {
  id: string;
  name: string;
  category: string;
  description: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  sourceRef?: string;
  sourceRevision?: string | null;
  sourceKind?: string | null;
  chunkIndex?: number | null;
  archived?: boolean;
  keywords?: string[];
}
interface Edge2D {
  source: number;
  target: number;
  label: string;
  evidenceRef?: string;
}
interface VisibleEdge2D extends Edge2D {
  sourceId: string;
  targetId: string;
}
type GraphMode = "entities" | "brain";

const COLORS: Record<string, string> = {
  character: "#D4943A",
  character_trait: "#8B5CF6",
  lore: "#D4943A",
  outline: "#5A8A6A",
  chunk: "#8B5CF6",
  default: "#6B7280",
};

function forceSimulation(
  nodes: Node2D[],
  edges: { source: number; target: number }[],
  width: number,
  height: number,
) {
  const alpha = { current: 0.5 };
  const centerX = width / 2;
  const centerY = height / 2;

  for (let iter = 0; iter < 200; iter++) {
    // Repulsion between all nodes
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const dx = nodes[j].x - nodes[i].x;
        const dy = nodes[j].y - nodes[i].y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const force = (alpha.current * 800) / (dist * dist);
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        nodes[i].vx -= fx;
        nodes[i].vy -= fy;
        nodes[j].vx += fx;
        nodes[j].vy += fy;
      }
    }

    // Attraction along edges
    for (const edge of edges) {
      const dx = nodes[edge.target].x - nodes[edge.source].x;
      const dy = nodes[edge.target].y - nodes[edge.source].y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const force = (dist - 120) * alpha.current * 0.02;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;
      nodes[edge.source].vx += fx;
      nodes[edge.source].vy += fy;
      nodes[edge.target].vx -= fx;
      nodes[edge.target].vy -= fy;
    }

    // Center gravity + velocity application + damping
    for (const n of nodes) {
      n.vx += (centerX - n.x) * 0.001;
      n.vy += (centerY - n.y) * 0.001;
      n.vx *= 0.85;
      n.vy *= 0.85;
      n.x += n.vx;
      n.y += n.vy;
    }

    alpha.current *= 0.97;
  }
}

export default function LoreGraphView() {
  const svgRef = useRef<SVGSVGElement>(null);
  const [graphData, setGraphData] = useState<GraphData | null>(null);
  const [knowledgeGraph, setKnowledgeGraph] = useState<KnowledgeGraphData | null>(null);
  const [graphMode, setGraphMode] = useState<GraphMode>("entities");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<Node2D | null>(null);
  const [nodes, setNodes] = useState<Node2D[]>([]);
  const [edges, setEdges] = useState<Edge2D[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [kindFilter, setKindFilter] = useState("all");
  const [viewBox, setViewBox] = useState("0 0 800 600");
  const [sourceCompare, setSourceCompare] = useState<KnowledgeSourceCompare | null>(null);
  const [sourceCompareError, setSourceCompareError] = useState<string | null>(null);
  const [sourceCompareLoading, setSourceCompareLoading] = useState(false);

  const loadGraph = useCallback(async (mode: GraphMode) => {
    const w = 700;
    const h = 500;

    try {
      if (mode === "brain") {
        const data = await invoke<KnowledgeGraphData>(Commands.getProjectBrainKnowledgeGraph);
        setKnowledgeGraph(data);
        const simNodes: Node2D[] = data.nodes.map((node) => ({
          id: node.id,
          name: node.label,
          category: node.kind,
          description: node.summary,
          sourceRef: node.sourceRef,
          sourceRevision: node.sourceRevision,
          sourceKind: node.sourceKind,
          chunkIndex: node.chunkIndex,
          archived: node.archived,
          keywords: node.keywords,
          x: w / 2 + (Math.random() - 0.5) * 200,
          y: h / 2 + (Math.random() - 0.5) * 200,
          vx: 0,
          vy: 0,
        }));
        const idToIdx = new Map<string, number>();
        simNodes.forEach((node, index) => idToIdx.set(node.id, index));
        const simEdges = data.edges.flatMap((edge) => {
          const source = idToIdx.get(edge.from);
          const target = idToIdx.get(edge.to);
          return source !== undefined && target !== undefined && source !== target
            ? [{ source, target, label: edge.relation, evidenceRef: edge.evidenceRef }]
            : [];
        });
        forceSimulation(simNodes, simEdges, w, h);
        setLoadError(null);
        setViewBox(`0 0 ${w} ${h}`);
        setNodes(simNodes);
        setEdges(simEdges);
        return;
      }

      const data = await invoke<GraphData>(Commands.getProjectGraphData);
      setGraphData(data);
      const simNodes: Node2D[] = data.entities.map((entity) => ({
        id: entity.id,
        name: entity.name,
        category: entity.category,
        description: entity.description,
        x: w / 2 + (Math.random() - 0.5) * 200,
        y: h / 2 + (Math.random() - 0.5) * 200,
        vx: 0,
        vy: 0,
      }));

      const nameToIdx = new Map<string, number>();
      simNodes.forEach((node, index) => nameToIdx.set(node.name, index));
      const simEdges: Edge2D[] = [];
      for (const rel of data.relationships) {
        const source = nameToIdx.get(rel.source);
        const target = nameToIdx.get(rel.target);
        if (source !== undefined && target !== undefined && source !== target) {
          simEdges.push({ source, target, label: rel.label });
        }
      }

      forceSimulation(simNodes, simEdges, w, h);
      setLoadError(null);
      setViewBox(`0 0 ${w} ${h}`);
      setNodes(simNodes);
      setEdges(simEdges);
    } catch (e) {
      setNodes([]);
      setEdges([]);
      setLoadError(String(e));
      console.error("Failed to load graph data:", e);
    }
  }, []);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      void loadGraph(graphMode);
    }, 0);
    return () => window.clearTimeout(timeout);
  }, [graphMode, loadGraph]);

  const resetSourceCompare = useCallback(() => {
    setSourceCompare(null);
    setSourceCompareError(null);
  }, []);

  const handleNodeClick = useCallback((node: Node2D) => {
    resetSourceCompare();
    setSelectedNode(node);
  }, [resetSourceCompare]);

  const handleAskBrain = useCallback(() => {
    if (!selectedNode) return;
    invoke(Commands.askProjectBrain, {
      query: `Tell me everything about ${selectedNode.name}`,
    }).catch(console.error);
  }, [selectedNode]);

  const availableKinds = useMemo(() => {
    const kinds = new Set(nodes.map((node) => node.category).filter(Boolean));
    return ["all", ...Array.from(kinds).sort()];
  }, [nodes]);

  const visibleNodeIndexes = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();

    return nodes
      .map((node, index) => ({ node, index }))
      .filter(({ node, index }) => {
        if (kindFilter !== "all" && node.category !== kindFilter) return false;
        if (!query) return true;

        const searchable = [
          node.name,
          node.category,
          node.description,
          node.sourceRef ?? "",
          node.sourceRevision ?? "",
          node.sourceKind ?? "",
          node.chunkIndex === null || node.chunkIndex === undefined ? "" : String(node.chunkIndex),
          ...(node.keywords ?? []),
          ...edges.flatMap((edge) => {
            if (edge.source !== index && edge.target !== index) return [];
            return [edge.label, edge.evidenceRef ?? ""];
          }),
        ].join(" ").toLowerCase();
        return searchable.includes(query);
      });
  }, [edges, kindFilter, nodes, searchQuery]);

  const visibleIndexLookup = useMemo(() => {
    const lookup = new Map<number, number>();
    visibleNodeIndexes.forEach(({ index }, visibleIndex) => {
      lookup.set(index, visibleIndex);
    });
    return lookup;
  }, [visibleNodeIndexes]);

  const visibleNodes = useMemo(
    () => visibleNodeIndexes.map(({ node }) => node),
    [visibleNodeIndexes],
  );

  const visibleEdges = useMemo<VisibleEdge2D[]>(() => (
    edges.flatMap((edge) => {
      const source = visibleIndexLookup.get(edge.source);
      const target = visibleIndexLookup.get(edge.target);
      const sourceNode = nodes[edge.source];
      const targetNode = nodes[edge.target];

      return source !== undefined && target !== undefined && sourceNode && targetNode
        ? [{
          ...edge,
          source,
          target,
          sourceId: sourceNode.id,
          targetId: targetNode.id,
        }]
        : [];
    })
  ), [edges, nodes, visibleIndexLookup]);

  const selectedReferences = useMemo(() => {
    if (!selectedNode) return [];
    const selectedIndex = nodes.findIndex((node) => node.id === selectedNode.id);
    if (selectedIndex < 0) return [];

    return edges.flatMap((edge) => {
      const isSource = edge.source === selectedIndex;
      const isTarget = edge.target === selectedIndex;
      if (!isSource && !isTarget) return [];

      const linkedNode = nodes[isSource ? edge.target : edge.source];
      return linkedNode
        ? [{
          node: linkedNode,
          relation: edge.label,
          direction: isSource ? "out" : "in",
          evidenceRef: edge.evidenceRef,
        }]
        : [];
    });
  }, [edges, nodes, selectedNode]);

  const selectedNeighborIds = useMemo(
    () => new Set(selectedReferences.map((reference) => reference.node.id)),
    [selectedReferences],
  );

  const selectedSourceHistory = useMemo(() => {
    if (!selectedNode?.sourceRef || graphMode !== "brain") return null;
    return knowledgeGraph?.sourceHistory?.find(
      (source) => source.sourceRef === selectedNode.sourceRef,
    ) ?? null;
  }, [graphMode, knowledgeGraph, selectedNode]);

  const handleCompareSource = async () => {
    if (!selectedNode?.sourceRef || graphMode !== "brain") return;
    setSourceCompareLoading(true);
    setSourceCompareError(null);
    try {
      const result = await invoke<KnowledgeSourceCompare>(
        Commands.compareProjectBrainSourceRevisions,
        { sourceRef: selectedNode.sourceRef },
      );
      setSourceCompare(result);
    } catch (error) {
      setSourceCompare(null);
      setSourceCompareError(String(error));
    } finally {
      setSourceCompareLoading(false);
    }
  };

  const graphSummary = `${visibleNodes.length}/${nodes.length} nodes · ${visibleEdges.length}/${edges.length} edges`;

  if (!graphData && graphMode === "entities" && !loadError) {
    return (
      <div className="flex items-center justify-center h-full text-text-muted text-xs">
        Loading graph...
      </div>
    );
  }

  return (
    <div className="flex h-full">
      <div className="flex-1 relative">
        <div className="px-3 py-2 border-b border-border-subtle text-xs text-text-secondary font-display tracking-wider flex items-center justify-between gap-2">
          <span>{graphMode === "brain" ? "Project Brain Knowledge Graph" : "Entity Graph"}</span>
          <div className="flex rounded bg-bg-deep border border-border-subtle p-0.5">
            {(["entities", "brain"] as const).map((mode) => (
              <button
                key={mode}
                onClick={() => {
                  setSelectedNode(null);
                  resetSourceCompare();
                  setLoadError(null);
                  setNodes([]);
                  setEdges([]);
                  setSearchQuery("");
                  setKindFilter("all");
                  setGraphMode(mode);
                }}
                className={`px-2 py-0.5 rounded-sm text-[10px] ${
                  graphMode === mode
                    ? "bg-accent text-bg-deep"
                    : "text-text-muted hover:text-text-secondary"
                }`}
              >
                {mode === "entities" ? "Entities" : "Brain"}
              </button>
            ))}
          </div>
        </div>
        <div className="px-3 py-2 border-b border-border-subtle flex flex-wrap items-center gap-2">
          <input
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder={graphMode === "brain" ? "Search source, revision, keyword, summary" : "Search entity or relation"}
            className="min-w-0 flex-1 rounded-sm border border-border-subtle bg-bg-deep px-2 py-1 text-xs text-text-primary placeholder:text-text-muted outline-none focus:border-accent"
          />
          <div className="flex flex-wrap gap-1">
            {availableKinds.map((kind) => (
              <button
                key={kind}
                onClick={() => setKindFilter(kind)}
                className={`rounded-sm px-2 py-1 text-[10px] ${
                  kindFilter === kind
                    ? "bg-accent text-bg-deep"
                    : "bg-bg-deep text-text-muted hover:text-text-secondary"
                }`}
              >
                {kind === "all" ? "All" : kind}
              </button>
            ))}
          </div>
          <span className="text-[10px] text-text-muted">{graphSummary}</span>
        </div>
        {loadError ? (
          <div className="p-4 text-xs text-danger">{loadError}</div>
        ) : nodes.length === 0 ? (
          <div className="p-4 text-xs text-text-muted">No graph nodes available yet.</div>
        ) : visibleNodes.length === 0 ? (
          <div className="p-4 text-xs text-text-muted">No matching graph nodes.</div>
        ) : (
          <svg ref={svgRef} viewBox={viewBox} className="w-full h-[calc(100%-84px)]">
          {visibleEdges.map((edge, index) => {
            const selectedEdge = selectedNode
              ? edge.sourceId === selectedNode.id || edge.targetId === selectedNode.id
              : false;
            return (
            <line
              key={`edge-${index}`}
              x1={visibleNodes[edge.source]?.x ?? 0}
              y1={visibleNodes[edge.source]?.y ?? 0}
              x2={visibleNodes[edge.target]?.x ?? 0}
              y2={visibleNodes[edge.target]?.y ?? 0}
              stroke={selectedEdge ? "#D4943A" : "#3D3934"}
              strokeWidth={selectedEdge ? 2 : 1}
              strokeOpacity={selectedEdge ? 0.9 : 0.6}
            />
            );
          })}
          {visibleNodes.map((node) => {
            const isSelected = selectedNode?.id === node.id;
            const isNeighbor = selectedNeighborIds.has(node.id);
            return (
            <g
              key={node.id}
              onClick={() => handleNodeClick(node)}
              className="cursor-pointer"
            >
              <circle
                cx={node.x}
                cy={node.y}
                r={node.category === "character" || node.category === "lore" ? 12 : 8}
                fill={COLORS[node.category] || COLORS.default}
                fillOpacity={isSelected || isNeighbor ? 0.95 : 0.75}
                stroke={isSelected ? "#E4DAC8" : isNeighbor ? "#D4943A" : COLORS[node.category] || COLORS.default}
                strokeWidth={isSelected || isNeighbor ? 2 : 1}
              />
              <text
                x={node.x}
                y={node.y + 20}
                textAnchor="middle"
                fill="#8A8278"
                fontSize={10}
                fontFamily="system-ui"
              >
                {node.name.length > 10 ? node.name.substring(0, 10) + "..." : node.name}
              </text>
            </g>
            );
          })}
        </svg>
        )}
      </div>
      {selectedNode && (
        <div className="w-56 border-l border-border-subtle p-3 space-y-2 overflow-y-auto">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-text-primary">{selectedNode.name}</span>
            <button
              onClick={() => {
                setSelectedNode(null);
                resetSourceCompare();
              }}
              className="text-text-muted hover:text-text-primary text-xs"
            >
              ✕
            </button>
          </div>
          <span
            className="text-[10px] px-1.5 py-0.5 rounded-sm inline-block"
            style={{
              backgroundColor: (COLORS[selectedNode.category] || COLORS.default) + "30",
              color: COLORS[selectedNode.category] || COLORS.default,
            }}
          >
            {selectedNode.category}
          </span>
          <p className="text-xs text-text-secondary leading-relaxed">
            {selectedNode.description}
          </p>
          {selectedNode.sourceRef && (
            <div className="text-[10px] text-text-muted">
              Source: {selectedNode.sourceRef}
            </div>
          )}
          {graphMode === "brain" && (
            <div className="grid grid-cols-1 gap-1 rounded-sm border border-border-subtle bg-bg-deep p-2 text-[10px] text-text-muted">
              {selectedNode.sourceKind && (
                <div className="flex min-w-0 justify-between gap-2">
                  <span>Source kind</span>
                  <span className="truncate text-text-secondary">{selectedNode.sourceKind}</span>
                </div>
              )}
              {selectedNode.sourceRevision && (
                <div className="flex min-w-0 justify-between gap-2">
                  <span>Revision</span>
                  <span className="truncate text-text-secondary" title={selectedNode.sourceRevision}>
                    {selectedNode.sourceRevision}
                  </span>
                </div>
              )}
              {selectedNode.chunkIndex !== null && selectedNode.chunkIndex !== undefined && (
                <div className="flex min-w-0 justify-between gap-2">
                  <span>Chunk</span>
                  <span className="text-text-secondary">#{selectedNode.chunkIndex + 1}</span>
                </div>
              )}
              {selectedNode.archived && (
                <div className="flex min-w-0 justify-between gap-2">
                  <span>Status</span>
                  <span className="text-text-secondary">Archived revision</span>
                </div>
              )}
            </div>
          )}
          {selectedSourceHistory && (
            <div className="space-y-1 rounded-sm border border-border-subtle bg-bg-deep p-2 text-[10px] text-text-muted">
              <div className="flex justify-between gap-2">
                <span>Source history</span>
                <span className="text-text-secondary">
                  {selectedSourceHistory.revisions.length} rev · {selectedSourceHistory.chunkCount} chunks
                </span>
              </div>
              {selectedSourceHistory.latestSummary && (
                <p className="line-clamp-2 text-text-secondary">{selectedSourceHistory.latestSummary}</p>
              )}
              {selectedSourceHistory.revisions.slice(0, 3).map((revision) => (
                <div key={revision.revision} className="min-w-0 rounded-sm border border-border-subtle px-1.5 py-1">
                  <div className="flex min-w-0 justify-between gap-2">
                    <div className="truncate text-text-secondary" title={revision.revision}>
                      {revision.revision}
                    </div>
                    {revision.active && <span className="shrink-0 text-[9px] text-accent">active</span>}
                  </div>
                  <div className="mt-0.5 text-[9px] text-text-muted">
                    {revision.nodeCount} nodes
                    {revision.chunkIndexes.length > 0
                      ? ` · chunks ${revision.chunkIndexes.map((index) => `#${index + 1}`).join(", ")}`
                      : ""}
                  </div>
                </div>
              ))}
              {selectedSourceHistory.revisions.length > 1 && (
                <button
                  onClick={handleCompareSource}
                  disabled={sourceCompareLoading}
                  className="w-full rounded-sm border border-border-subtle px-2 py-1 text-left text-[10px] text-text-secondary hover:border-accent/50 disabled:opacity-60"
                >
                  {sourceCompareLoading ? "Comparing revisions..." : "Compare source revisions"}
                </button>
              )}
              {sourceCompareError && <div className="text-danger">{sourceCompareError}</div>}
              {sourceCompare && (
                <div className="space-y-1 rounded-sm border border-border-subtle bg-bg p-2">
                  <div className="flex justify-between gap-2 text-text-secondary">
                    <span>Revision compare</span>
                    <span>{sourceCompare.revisions.length} rev</span>
                  </div>
                  <div className="text-[9px] text-text-muted">
                    Active: {sourceCompare.activeRevision ?? "none"}
                  </div>
                  {(sourceCompare.addedKeywords.length > 0 || sourceCompare.removedKeywords.length > 0) && (
                    <div className="space-y-1">
                      {sourceCompare.addedKeywords.length > 0 && (
                        <div className="line-clamp-2 text-[9px] text-text-secondary">
                          Added: {sourceCompare.addedKeywords.slice(0, 6).join(", ")}
                        </div>
                      )}
                      {sourceCompare.removedKeywords.length > 0 && (
                        <div className="line-clamp-2 text-[9px] text-text-muted">
                          Removed: {sourceCompare.removedKeywords.slice(0, 6).join(", ")}
                        </div>
                      )}
                    </div>
                  )}
                  {sourceCompare.revisions.slice(0, 3).map((revision) => (
                    <div key={revision.revision} className="rounded-sm border border-border-subtle px-1.5 py-1">
                      <div className="flex min-w-0 justify-between gap-2">
                        <span className="truncate text-text-secondary" title={revision.revision}>
                          {revision.revision}
                        </span>
                        <span className="shrink-0 text-[9px] text-text-muted">
                          {revision.active ? "active" : "archived"}
                        </span>
                      </div>
                      <div className="mt-0.5 text-[9px] text-text-muted">
                        {revision.chunkCount} chunks
                        {revision.keywords.length > 0 ? ` · ${revision.keywords.slice(0, 4).join(", ")}` : ""}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
          {(selectedNode.keywords?.length ?? 0) > 0 && (
            <div className="flex flex-wrap gap-1">
              {selectedNode.keywords?.slice(0, 8).map((keyword) => (
                <span key={keyword} className="rounded bg-bg-deep px-1.5 py-0.5 text-[10px] text-text-muted">
                  {keyword}
                </span>
              ))}
            </div>
          )}
          <button
            onClick={handleAskBrain}
            className="w-full text-xs px-2 py-1.5 rounded-sm bg-accent/20 hover:bg-accent/30 text-accent transition-colors"
          >
            Ask Brain about {selectedNode.name}
          </button>
          {graphMode === "brain" && knowledgeGraph && (
            <div className="rounded bg-bg-deep p-2 text-[10px] text-text-muted">
              {knowledgeGraph.nodes.length} nodes · {knowledgeGraph.edges.length} edges · {knowledgeGraph.sourceCount} sources
            </div>
          )}
          <div className="space-y-1">
            <div className="text-[10px] uppercase tracking-wider text-text-muted">
              References
            </div>
            {selectedReferences.length === 0 ? (
              <div className="rounded bg-bg-deep p-2 text-[10px] text-text-muted">
                No linked references.
              </div>
            ) : (
              selectedReferences.slice(0, 8).map((reference) => (
                <button
                  key={`${reference.direction}-${reference.relation}-${reference.node.id}`}
                  onClick={() => setSelectedNode(reference.node)}
                  className="w-full rounded-sm border border-border-subtle bg-bg-deep p-2 text-left hover:border-accent/50"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate text-[11px] text-text-secondary">{reference.node.name}</span>
                    <span className="shrink-0 text-[9px] text-text-muted">
                      {reference.direction === "out" ? "out" : "back"}
                    </span>
                  </div>
                  <div className="mt-1 line-clamp-2 text-[10px] text-text-muted">
                    {reference.relation}
                    {reference.evidenceRef ? ` · ${reference.evidenceRef}` : ""}
                  </div>
                  {graphMode === "brain" && (reference.node.sourceRef || reference.node.sourceRevision) && (
                    <div className="mt-1 truncate text-[9px] text-text-muted">
                      {reference.node.sourceRef}
                      {reference.node.sourceRevision ? ` · ${reference.node.sourceRevision}` : ""}
                    </div>
                  )}
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
