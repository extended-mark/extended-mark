// A failure in a reader's browser is the team's to hear about.

export function report(kind: string, message: string): void {
  console.error(`[pages] ${kind}: ${message}`);
  void fetch("/api/report", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ kind, message, agent: navigator.userAgent }) }).catch(() => {});
}
