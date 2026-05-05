import type {
  ChapterMissionSummary,
  StoryContractSummary,
} from "../protocol";


export interface StoryContractDraft {
  title: string;
  genre: string;
  targetReader: string;
  readerPromise: string;
  first30ChapterPromise: string;
  mainConflict: string;
  structuralBoundary: string;
  toneContract: string;
}

export interface ChapterMissionDraft {
  mission: string;
  mustInclude: string;
  mustNot: string;
  expectedEnding: string;
  status: string;
  sourceRef: string;
  blockedReason: string;
  retiredHistory: string;
}


export function emptyStoryContractDraft(): StoryContractDraft {
  return {
    title: "",
    genre: "",
    targetReader: "",
    readerPromise: "",
    first30ChapterPromise: "",
    mainConflict: "",
    structuralBoundary: "",
    toneContract: "",
  };
}

export function storyContractDraftFromSummary(
  contract: StoryContractSummary | null | undefined,
): StoryContractDraft {
  return {
    title: contract?.title ?? "",
    genre: contract?.genre ?? "",
    targetReader: contract?.targetReader ?? "",
    readerPromise: contract?.readerPromise ?? "",
    first30ChapterPromise: contract?.first30ChapterPromise ?? "",
    mainConflict: contract?.mainConflict ?? "",
    structuralBoundary: contract?.structuralBoundary ?? "",
    toneContract: contract?.toneContract ?? "",
  };
}

export function emptyChapterMissionDraft(): ChapterMissionDraft {
  return {
    mission: "",
    mustInclude: "",
    mustNot: "",
    expectedEnding: "",
    status: "active",
    sourceRef: "author",
    blockedReason: "",
    retiredHistory: "",
  };
}

export function chapterMissionDraftFromSummary(
  mission: ChapterMissionSummary | null | undefined,
): ChapterMissionDraft {
  return {
    mission: mission?.mission ?? "",
    mustInclude: mission?.mustInclude ?? "",
    mustNot: mission?.mustNot ?? "",
    expectedEnding: mission?.expectedEnding ?? "",
    status: mission?.status === "in_progress" ? "active" : mission?.status || "active",
    sourceRef: mission?.sourceRef || "author",
    blockedReason: mission?.blockedReason ?? "",
    retiredHistory: mission?.retiredHistory ?? "",
  };
}

export function hasStoryContractContent(draft: StoryContractDraft): boolean {
  return Object.values(draft).some((value) => value.trim().length > 0);
}

export function hasChapterMissionContent(draft: ChapterMissionDraft): boolean {
  return [
    draft.mission,
    draft.mustInclude,
    draft.mustNot,
    draft.expectedEnding,
  ].some((value) => value.trim().length > 0);
}

export function validateChapterMissionStatusExplanation(
  draft: ChapterMissionDraft,
): string | null {
  if (draft.status === "blocked" && draft.blockedReason.trim().length < 8) {
    return "Blocked Chapter Mission needs a concrete reason before saving.";
  }
  if (draft.status === "retired" && draft.retiredHistory.trim().length < 8) {
    return "Retired Chapter Mission needs a short history note before saving.";
  }
  return null;
}
