import { useRef, useEffect, useState, useCallback } from "react";
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
  keywords: string[];
  summary: string;
}
interface KnowledgeEdge {
  from: string;
  to: string;
  relation: string;
  evidenceRef: string;
}
interface KnowledgeGraphData {
  projectId: string;
  nodes: KnowledgeNode[];
  edges: KnowledgeEdge[];
  sourceCount: number;
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
  keywords?: string[];
}

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
  const [graphMode, setGraphMode] = useState<"entities" | "brain">("entities");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<Node2D | null>(null);
  const [nodes, setNodes] = useState<Node2D[]>([]);
  const [edges, setEdges] = useState<{ source: number; target: number; label: string }[]>([]);
  const [viewBox, setViewBox] = useState("0 0 800 600");

  const loadGraph = useCallback(async (mode: "entities" | "brain") => {
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
            ? [{ source, target, label: edge.relation }]
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
      const simEdges: { source: number; target: number; label: string }[] = [];
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

  const handleNodeClick = useCallback((node: Node2D) => {
    setSelectedNode(node);
  }, []);

  const handleAskBrain = useCallback(() => {
    if (!selectedNode) return;
    invoke(Commands.askProjectBrain, {
      query: `Tell me everything about ${selectedNode.name}`,
    }).catch(console.error);
  }, [selectedNode]);

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
                  setLoadError(null);
                  setNodes([]);
                  setEdges([]);
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
        {loadError ? (
          <div className="p-4 text-xs text-danger">{loadError}</div>
        ) : nodes.length === 0 ? (
          <div className="p-4 text-xs text-text-muted">No graph nodes available yet.</div>
        ) : (
          <svg ref={svgRef} viewBox={viewBox} className="w-full h-[calc(100%-36px)]">
          {edges.map((edge, index) => (
            <line
              key={`edge-${index}`}
              x1={nodes[edge.source]?.x ?? 0}
              y1={nodes[edge.source]?.y ?? 0}
              x2={nodes[edge.target]?.x ?? 0}
              y2={nodes[edge.target]?.y ?? 0}
              stroke="#3D3934"
              strokeWidth={1}
              strokeOpacity={0.6}
            />
          ))}
          {nodes.map((node) => (
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
                fillOpacity={0.8}
                stroke={selectedNode?.id === node.id ? "#E4DAC8" : COLORS[node.category] || COLORS.default}
                strokeWidth={selectedNode?.id === node.id ? 2 : 1}
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
          ))}
        </svg>
        )}
      </div>
      {selectedNode && (
        <div className="w-56 border-l border-border-subtle p-3 space-y-2 overflow-y-auto">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-text-primary">{selectedNode.name}</span>
            <button
              onClick={() => setSelectedNode(null)}
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
        </div>
      )}
    </div>
  );
}
