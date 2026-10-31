// Managed by Loom: V2 native HTTP hook. Session headers stay on the loopback gateway.
import { Plugin } from "@opencode/plugin";
export default Plugin.define({
  id: "loom.observability",
  async setup(ctx) {
    await ctx.session.hook("http.request", (event) => {
      const url = new URL(event.request.url);
      if (!["127.0.0.1", "localhost"].includes(url.hostname) || url.protocol !== "http:") return;
      event.request.headers.set("x-loom-harness", "opencode2");
      event.request.headers.set("x-loom-session-id", event.sessionID);
    }, { providerID: "hub" });
  },
});
