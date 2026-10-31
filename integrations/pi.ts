// Managed by Loom: session correlation only. No credentials, payload edits or network interception.
export default function loomTracing(pi: any) {
  pi.on("before_provider_headers", (event: any, ctx: any) => {
    if (ctx.model?.provider !== "hub") return;
    event.headers["x-loom-harness"] = "pi";
    event.headers["x-loom-session-id"] = ctx.sessionManager.getSessionId();
  });
}
