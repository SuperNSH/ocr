// Plan for OCR in this repo. Phases run in order for each area:
// implement, audit, unit tests, device test, vision, smoke.
// Gates on every area: cargo fmt --check, cargo clippy -D warnings, cargo test.

export type Phase = "implement" | "audit" | "unit-tests" | "device-test" | "vision" | "smoke";

export interface Area {
  name: string;
  phases: Phase[];
  status: Record<Phase, "not-started" | "done" | "blocked">;
}

export const workflow = {
  repo: "SuperNSH/ocr",
  gates: [
    "cargo fmt --all -- --check",
    "cargo clippy -p nsh-ocr --all-targets --all-features -- -D warnings",
    "cargo test -p nsh-ocr --all-features --no-fail-fast",
  ],
  areas: [
    {
      name: "ocr",
      phases: ["implement", "audit", "unit-tests", "device-test", "vision", "smoke"],
      status: {
        "implement": "done",
        "audit": "done",
        "unit-tests": "done",
        "device-test": "not-started",
        "vision": "not-started",
        "smoke": "not-started",
      },
    },
  ] satisfies Area[],
};
