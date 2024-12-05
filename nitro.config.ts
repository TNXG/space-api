//https://nitro.unjs.io/config
import { fileURLToPath } from "node:url";

export default defineNitroConfig({
	srcDir: "src",
	compatibilityDate: "2024-12-04",
	routeRules: {
		"/**": { cors: true, headers: { server: "Nitro.js" } },
	},
	alias: {
		"@": fileURLToPath(new URL("./src", import.meta.url)),
	},
});
