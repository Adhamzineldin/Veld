// Pure command wiring: which `veld` arguments each VS Code command runs. No `vscode` import, so
// it can be unit tested with plain Node (see test/cli.test.js).

export interface VeldCommand {
  /** The VS Code command id, also listed in package.json `contributes.commands`. */
  id: string;
  /** The title shown in the command palette. */
  title: string;
  /** Arguments after the executable. */
  args: string[];
}

/** Commands that run once and print into the output channel. */
export const RUN_COMMANDS: readonly VeldCommand[] = [
  { id: "veld.validate", title: "Veld: Validate Contract", args: ["check"] },
  { id: "veld.generate", title: "Veld: Generate Code", args: ["generate"] },
  { id: "veld.generateDryRun", title: "Veld: Generate (Dry Run)", args: ["generate", "--dry-run"] },
  { id: "veld.generateCheck", title: "Veld: Check Generated Code Is Up To Date", args: ["generate", "--check"] },
  { id: "veld.setup", title: "Veld: Setup (Wire Targets Into Apps)", args: ["setup"] },
  { id: "veld.doctor", title: "Veld: Doctor", args: ["doctor"] },
];

/** `veld export <format>` formats offered by "Veld: Export...". */
export const EXPORT_FORMATS: readonly string[] = ["openapi", "ir"];

export const WATCH: VeldCommand = {
  id: "veld.watch",
  title: "Veld: Watch (Generate On Change)",
  args: ["generate", "--watch"],
};

/** Every command id the extension registers, for the package.json consistency test. */
export const COMMAND_IDS: readonly string[] = [
  ...RUN_COMMANDS.map((c) => c.id),
  WATCH.id,
  "veld.exportOpenapi",
  "veld.export",
  "veld.restartServer",
  "veld.showOutput",
];

/** The executable from the `veld.path` setting; blank means `veld` on PATH. */
export function executable(configured: string | undefined): string {
  const value = (configured ?? "").trim();
  return value === "" ? "veld" : value;
}

/** The arguments for `veld export <format> -o <file> --force` (the save dialog already confirmed the overwrite). */
export function exportArgs(format: string, file: string): string[] {
  // `export ir` always overwrites its file; `export openapi` needs `--force` for an existing one.
  return format === "ir" ? ["export", format, "-o", file] : ["export", format, "-o", file, "--force"];
}

/** The default file name of an export, by format. */
export function exportFileName(format: string): string {
  switch (format) {
    case "openapi":
      return "openapi.json";
    case "ir":
      return "veld-ir.json";
    default:
      return `${format}.out`;
  }
}

export interface Summary {
  errors: number;
  warnings: number;
}

/**
 * Reads the summary line of `veld check` ("✗ 1 error, 0 warnings" or "✓ ..." variants).
 * Returns undefined when the output has no summary.
 */
export function parseSummary(output: string): Summary | undefined {
  const match = /(\d+) errors?,? (\d+) warnings?/.exec(output);
  if (!match) {
    return undefined;
  }
  return { errors: Number(match[1]), warnings: Number(match[2]) };
}

/** Quotes one argument for a shell command line (used by the watch task). */
export function shellQuote(arg: string): string {
  return /^[A-Za-z0-9_@%+=:,./\\-]+$/.test(arg) ? arg : `"${arg.replace(/(["\\$`])/g, "\\$1")}"`;
}
