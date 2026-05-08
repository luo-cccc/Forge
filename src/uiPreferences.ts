export const FORGE_THEME_STORAGE_KEY = "forge.theme";

export const FORGE_THEMES = [
  {
    value: "amber-dark",
    label: "琥珀深色",
    description: "默认深色写作环境，低亮度、暖色强调。",
  },
  {
    value: "paper-light",
    label: "纸张浅色",
    description: "接近纸面阅读，适合白天长时间编辑。",
  },
  {
    value: "slate-dark",
    label: "灰蓝深色",
    description: "更冷静的深色界面，降低暖色干扰。",
  },
] as const;

export type ForgeTheme = (typeof FORGE_THEMES)[number]["value"];

export const DEFAULT_FORGE_THEME: ForgeTheme = "amber-dark";

export function isForgeTheme(value: string | null): value is ForgeTheme {
  return FORGE_THEMES.some((theme) => theme.value === value);
}
