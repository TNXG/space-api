export default defineEventHandler(
	async () => {
		const headers = {
			"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36 Edg/114.0.1823.82",
			"Content-Type": "application/javascript; charset=utf-8",
		};

		try {
			const fetchResponse = await fetch("https://mx.tnxg.top/api/v2/snippets/js/sw", { headers });
			if (!fetchResponse.ok) {
				throw new Error(`HTTP error! status: ${fetchResponse.status}`);
			}
			const response = await fetchResponse.text();

			return new Response(response, {
				headers: { "Content-Type": "application/javascript; charset=utf-8" },
			});
		} catch (error) {
			console.error(error);
			return new Response(`// Failed to load service worker script: ${error}`, {
				headers: { "Content-Type": "application/javascript; charset=utf-8" },
				status: 500,
			});
		}
	},
);
