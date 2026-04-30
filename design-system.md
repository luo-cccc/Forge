# Agent-Writer · Design System v2
> 哲学：「暗底上的纸墨温度 + 网格骨架」
> 方向：A（纸墨温度）+ B（暗流暗底）+ C（编辑室结构）
> 日期：2026-04-30

## 🎨 色板

| Token | HEX | 用途 |
|--------|-----|------|
| `--bg-deep` | `#12100E` | 最深层背景（编辑器底色） |
| `--bg-surface` | `#1A1816` | 面板/卡片底色 |
| `--bg-raised` | `#22201D` | 浮层/弹窗/激活态面板 |
| `--border-subtle` | `#2A2724` | 面板间分隔线（网格线） |
| `--border-active` | `#3D3934` | 激活/悬停边框 |
| `--text-primary` | `#E4DAC8` | 正文（暖米白，纸墨感） |
| `--text-secondary` | `#8A8278` | 辅助文字/元信息 |
| `--text-muted` | `#5C5650` | 占位符/禁用态 |
| `--accent` | `#D4943A` | 唯一强调色（琥珀暖金） |
| `--accent-subtle` | `#3D2E1A` | 强调色浅底（选中态/幽灵预览） |
| `--code-bg` | `#161310` | 代码块/内联代码底色 |
| `--success` | `#5A8A6A` | 生成成功/Accept |
| `--danger` | `#8A5050` | 拒绝/错误 |

## 字型排印

| Token | 字体栈 | 用途 |
|--------|---------|------|
| `--font-display` | `"Source Serif 4", "Noto Serif SC", Georgia, serif` | 工具栏标题、大纲标题 |
| `--font-body` | `system-ui, -apple-system, "Segoe UI", sans-serif` | UI 标签、按钮、面板 |
| `--font-editor` | `"JetBrains Mono", "Fira Code", ui-monospace, monospace` | 编辑器内容 |
| `--font-chat` | `system-ui, -apple-system, sans-serif` | Agent 对话 |

**字号层级**：10 / 11 / 12 / 13 / 14 / 16 / 20 / 28 / 36（黄金比间隔）

## 网格系统

- **基础单位**：`4px`
- **面板宽度**：左 208px · 中 flex · 右 384px
- **面板分隔**：`1px solid var(--border-subtle)` — 可见的结构线
- **内边距**：`16px` (p-4) 基准，编辑器内 `24px`
- **圆角**：`4px` (按钮/tag) / `6px` (卡片) / `0px` (面板 — 结构不圆角)

## 动效

- **hover**：`transition-colors 150ms` — 干净，不做 spring
- **bubble 弹出**：不弹跳，直接出现 + opacity
- **流式文字**：光标 `animate-pulse` 琥珀色，1.2s 周期

## 禁区

- ❌ 不用蓝色作为 accent（AI slop 重灾区）
- ❌ 不用 `#000000` 纯黑——永远用暖黑
- ❌ 不用 `#FFFFFF` 纯白文字——永远暖米白
- ❌ 不用 emoji 作 UI 图标（工具感破坏）
- ❌ 不用圆角卡片 + 左 border accent（slop）
- ❌ 不用紫/品红/蓝紫渐变
