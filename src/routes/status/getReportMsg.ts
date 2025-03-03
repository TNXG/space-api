export default defineEventHandler(async (event) => {
	const url = getRequestURL(event);
	const encoder = new TextEncoder();
	let isConnectionActive = true;
	let lastData = null;
	let lastFallbackData = null; // 添加 fallback 数据的缓存

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

					// 获取 fallback 数据
					const fallbackResponse = await fetch(`${url}/status/?s=n`);
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
					// 比较主数据和 fallback 数据是否都没有变化
					const isDataChanged = !lastData || JSON.stringify(lastData) !== JSON.stringify(returndata);
					const isFallbackChanged = !lastFallbackData || JSON.stringify(lastFallbackData) !== JSON.stringify(fallbackData);

					if (isConnectionActive && (isDataChanged || isFallbackChanged)) {
						controller.enqueue(encoder.encode(`data: ${JSON.stringify(returndata)}\n\n`));
						lastData = returndata;
						lastFallbackData = fallbackData;
					}
				}
				catch (error) {
					console.error("Fetch error:", error);
					if (isConnectionActive) {
						controller.enqueue(
							encoder.encode(`data: ${JSON.stringify({ error: "Failed to fetch data" })}\n\n`),
						);
					}
				}
			};

			const sendHeartbeat = () => {
				if (isConnectionActive) {
					controller.enqueue(encoder.encode(": heartbeat\n\n"));
				}
			};

			await fetchAndSendData();

			const dataInterval = setInterval(fetchAndSendData, 5000);

			const heartbeatInterval = setInterval(sendHeartbeat, 30000);

			return () => {
				isConnectionActive = false;
				clearInterval(dataInterval);
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
