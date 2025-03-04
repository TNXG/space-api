export default defineEventHandler(async (event) => {
	const url = getRequestURL(event);
	const baseUrl = `${url.protocol}//${url.hostname}${url.port ? `:${url.port}` : ""}`;
	const encoder = new TextEncoder();
	let isConnectionActive = true;
	let lastData = null;
	let lastFallbackData = null;

	const stream = new ReadableStream({
		async start(controller) {
			const fetchAndSendData = async () => {
				if (!isConnectionActive)
					return;

				try {
					const response = await fetch("https://mx.tnxg.top/api/v2/fn/ps/update", {
						method: "POST",
					});
					const returndata = await response.json();

					const fallbackResponse = await fetch(`${baseUrl}/status/?s=n`);
					const fallbackData = await fallbackResponse.json();

					if (!returndata.mediaInfo && fallbackData.data?.user?.active === true) {
						returndata.mediaInfo = {
							AlbumArtist: fallbackData.data.song.artists
								.map((artist: any) => artist.name)
								.join(" / "),
							AlbumTitle: fallbackData.data.song.album.name,
							SourceAppName: "Netease Music NowPlaying Function",
							artist: fallbackData.data.song.artists
								.map((artist: any) => artist.name)
								.join(" / "),
							title: fallbackData.data.song.name,
							AlbumThumbnail: fallbackData.data.song.album.image,
						};
					}

					const isDataChanged = !lastData || JSON.stringify(lastData) !== JSON.stringify(returndata);
					const isFallbackChanged = !lastFallbackData || JSON.stringify(lastFallbackData) !== JSON.stringify(fallbackData);

					if (isConnectionActive && (isDataChanged || isFallbackChanged)) {
						controller.enqueue(encoder.encode(`data: ${JSON.stringify(returndata)}\n\n`));
						lastData = returndata;
						lastFallbackData = fallbackData;
					}
				}
				catch {
					if (isConnectionActive) {
						controller.enqueue(
							encoder.encode(`data: ${JSON.stringify({ error: "Failed to fetch data" })}\n\n`),
						);
					}
				}
			};

			const sendHeartbeatAndCheckData = async () => {
				if (isConnectionActive) {
					controller.enqueue(encoder.encode(": heartbeat\n\n"));
					await fetchAndSendData();
				}
			};

			await fetchAndSendData();

			const heartbeatInterval = setInterval(sendHeartbeatAndCheckData, 30000);

			return () => {
				isConnectionActive = false;
				clearInterval(heartbeatInterval);
				controller.close();
			};
		},
		cancel() {
			isConnectionActive = false;
		},
	});

	return new Response(stream, {
		headers: {
			"Content-Type": "text/event-stream; charset=utf-8",
			"Cache-Control": "no-cache",
			"Connection": "keep-alive",
		},
	});
});
