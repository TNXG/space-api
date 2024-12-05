export default defineCachedEventHandler(
	async () => {
		const headers = {
			"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36 Edg/114.0.1823.82",
			"Content-Type": "application/javascript; charset=utf-8",
		};
		const response = await (await fetch("https://mx.tnxg.top/api/v2/snippets/js/sw", { headers })).text();

		return new Response(response, {
			headers: { "Content-Type": "application/javascript; charset=utf-8" },
		});
	},
	{ maxAge: 7200 },
);
