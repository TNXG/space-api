import process from "node:process";
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
	runtimeConfig: {
		public: {
			baseURL: process.env.NODE_ENV === "development" ? "http://localhost:3000" : "https://api-space.tnxg.top",
		},
	},
});
