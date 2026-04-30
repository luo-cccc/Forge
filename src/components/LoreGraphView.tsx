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

interface Node2D {
  id: string;
  name: string;
  category: string;
  description: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
}

const COLORS: Record<string, string> = {
  character: "#D4943A",
  character_trait: "#8B5CF6",
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
  const [selectedNode, setSelectedNode] = useState<Node2D | null>(null);
  const [nodes, setNodes] = useState<Node2D[]>([]);
  const [edges, setEdges] = useState<{ source: number; target: number; label: string }[]>([]);
  const [viewBox, setViewBox] = useState("0 0 800 600");

  useEffect(() => {
    const load = async () => {
      try {
        const data = await invoke<GraphData>(Commands.getProjectGraphData);
        setGraphData(data);

        const w = 700;
        const h = 500;
        setViewBox(`0 0 ${w} ${h}`);

        const simNodes: Node2D[] = data.entities.map((e) => ({
          id: e.id,
          name: e.name,
          category: e.category,
          description: e.description,
          x: w / 2 + (Math.random() - 0.5) * 200,
          y: h / 2 + (Math.random() - 0.5) * 200,
          vx: 0,
          vy: 0,
        }));

        const nameToIdx = new Map<string, number>();
        simNodes.forEach((n, i) => nameToIdx.set(n.name, i));

        const simEdges: { source: number; target: number; label: string }[] = [];
        for (const rel of data.relationships) {
          const s = nameToIdx.get(rel.source);
          const t = nameToIdx.get(rel.target);
          if (s !== undefined && t !== undefined && s !== t) {
            simEdges.push({ source: s, target: t, label: rel.label });
          }
        }

        forceSimulation(simNodes, simEdges, w, h);
        setNodes(simNodes);
        setEdges(simEdges);
      } catch (e) {
        console.error("Failed to load graph data:", e);
      }
    };
    load();
  }, []);

  const handleNodeClick = useCallback((node: Node2D) => {
    setSelectedNode(node);
  }, []);

  const handleAskBrain = useCallback(() => {
    if (!selectedNode) return;
    // Trigger a Project Brain query
    invoke(Commands.askProjectBrain, {
      query: `Tell me everything about ${selectedNode.name}`,
    }).catch(console.error);
  }, [selectedNode]);

  if (!graphData) {
    return (
      <div className="flex items-center justify-center h-full text-text-muted text-xs">
        Loading graph...
      </div>
    );
  }

  return (
    <div className="flex h-full">
      <div className="flex-1 relative">
        <div className="px-3 py-2 border-b border-border-subtle text-xs text-text-secondary font-display tracking-wider">
          Entity Graph
        </div>
        <svg ref={svgRef} viewBox={viewBox} className="w-full h-[calc(100%-36px)]">
          {edges.map((e, i) => (
            <line
              key={`edge-${i}`}
              x1={nodes[e.source]?.x ?? 0}
              y1={nodes[e.source]?.y ?? 0}
              x2={nodes[e.target]?.x ?? 0}
              y2={nodes[e.target]?.y ?? 0}
              stroke="#3D3934"
              strokeWidth={1}
              strokeOpacity={0.6}
            />
          ))}
          {nodes.map((n) => (
            <g
              key={n.id}
              onClick={() => handleNodeClick(n)}
              className="cursor-pointer"
            >
              <circle
                cx={n.x}
                cy={n.y}
                r={n.category === "character" ? 12 : 8}
                fill={COLORS[n.category] || COLORS.default}
                fillOpacity={0.8}
                stroke={selectedNode?.id === n.id ? "#E4DAC8" : COLORS[n.category] || COLORS.default}
                strokeWidth={selectedNode?.id === n.id ? 2 : 1}
              />
              <text
                x={n.x}
                y={n.y + 20}
                textAnchor="middle"
                fill="#8A8278"
                fontSize={10}
                fontFamily="system-ui"
              >
                {n.name.length > 10 ? n.name.substring(0, 10) + "…" : n.name}
              </text>
            </g>
          ))}
        </svg>
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
          <button
            onClick={handleAskBrain}
            className="w-full text-xs px-2 py-1.5 rounded-sm bg-accent/20 hover:bg-accent/30 text-accent transition-colors"
          >
            Ask Brain about {selectedNode.name}
          </button>
        </div>
      )}
    </div>
  );
}
