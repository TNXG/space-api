import process from "node:process";
import { db_find, db_insert, db_update } from "@/utils/db";

import { getNcmNowPlay } from "@/utils/ncm-nowplay";
import dotenv from "dotenv";

dotenv.config();

interface User {
	id: string;
	avatar: string;
	name: string;
	active: boolean;
}

interface Song {
	name: string;
	transNames: string[];
	alias: string[];
	id: string;
	artists: { id: string; name: string }[];
	album: {
		name: string;
		id: string;
		image: string;
		publishTime: string;
		artists: { id: string; name: string }[];
	};
}

interface NowPlayingData {
	id: number;
	user: User;
	song?: Song;
	lastUpdate: string;
}

const generateResponse = <T>(status: string, message: string, data: T | null, code: string = "200"): ApiResponse<T> => {
	return {
		code,
		status,
		message,
		data,
	};
};

export default eventHandler(async (event) => {
	const query = getQuery(event);
	const s = query.s || query.source || "codetime";
	const q = query.q || query.query || 515522946;
	const sse = query.sse === "true";
	const interval = Number(query.interval) || Number(query.i) || 5000;

	if (interval < 1000) {
		const response = generateResponse("error", "Invalid interval: must be at least 1000ms", null, "400");
		return new Response(JSON.stringify(response), {
			status: 400,
			headers: {
				"Content-Type": "application/json",
			},
		});
	}

	if (typeof s !== "string") {
		const response = generateResponse("error", "Invalid s parameter: must be a string", null, "400");
		return new Response(JSON.stringify(response), {
			status: 400,
			headers: {
				"Content-Type": "application/json",
			},
		});
	}

	if (Number.isNaN(Number(q))) {
		const response = generateResponse("error", "Invalid q parameter: must be a number", null, "400");
		return new Response(JSON.stringify(response), {
			status: 400,
			headers: {
				"Content-Type": "application/json; charset=utf-8",
			},
		});
	}

	const qNumber = Number(q);
	const sString = String(s);
	const currentTime = new Date().toISOString();

	if (sString === "ncm" || sString === "n" || sString === "netease") {
		const nowPlayingData = await getNcmNowPlay(qNumber);
		const userId = String(nowPlayingData.data.userId);
		const songId = nowPlayingData.data.song.id;

		const isInactive = await handleCache(userId, songId, currentTime);

		const data: NowPlayingData = {
			id: nowPlayingData.data.id,
			user: {
				id: userId,
				avatar: nowPlayingData.data.avatar,
				name: nowPlayingData.data.userName,
				active: !isInactive,
			},
			lastUpdate: currentTime,
		};

		if (!isInactive) {
			data.song = {
				name: nowPlayingData.data.song.name,
				transNames: nowPlayingData.data.song.extProperties?.transNames || [],
				alias: nowPlayingData.data.song.alias || [],
				id: nowPlayingData.data.song.id,
				artists: nowPlayingData.data.song.artists.map((artist: { id: any; name: any }) => ({
					id: artist.id,
					name: artist.name,
				})),
				album: {
					name: nowPlayingData.data.song.album.name,
					id: nowPlayingData.data.song.album.id,
					image: nowPlayingData.data.song.album.picUrl,
					publishTime: new Date(nowPlayingData.data.song.album.publishTime).toISOString(),
					artists: nowPlayingData.data.song.album.artists.map(artist => ({
						id: artist.id,
						name: artist.name,
					})),
				},
			};
		}

		if (sse) {
			const stream = new ReadableStream({
				async start(controller) {
					let lastData = null;
					const encoder = new TextEncoder();

					const sendData = async () => {
						const nowPlayingData = await getNcmNowPlay(qNumber);
						const userId = String(nowPlayingData.data.userId);
						const songId = nowPlayingData.data.song.id;
						const isInactive = await handleCache(userId, songId, new Date().toISOString());

						data.lastUpdate = new Date().toISOString();
						data.user.active = !isInactive;
						if (!isInactive) {
							data.song = {
								name: nowPlayingData.data.song.name,
								transNames: nowPlayingData.data.song.extProperties?.transNames || [],
								alias: nowPlayingData.data.song.alias || [],
								id: nowPlayingData.data.song.id,
								artists: nowPlayingData.data.song.artists.map((artist: { id: any; name: any }) => ({
									id: artist.id,
									name: artist.name,
								})),
								album: {
									name: nowPlayingData.data.song.album.name,
									id: nowPlayingData.data.song.album.id,
									image: nowPlayingData.data.song.album.picUrl,
									publishTime: new Date(nowPlayingData.data.song.album.publishTime).toISOString(),
									artists: nowPlayingData.data.song.album.artists.map(artist => ({
										id: artist.id,
										name: artist.name,
									})),
								},
							};
						} else {
							delete data.song;
						}

						const currentData = JSON.stringify(data);
						if (!lastData || lastData !== currentData) {
							controller.enqueue(encoder.encode(`data: ${currentData}\n\n`));
							lastData = currentData;
						}
					};

					const sendHeartbeat = () => {
						controller.enqueue(encoder.encode(": heartbeat\n\n"));
					};

					await sendData();
					const dataInterval = setInterval(sendData, interval);
					const heartbeatInterval = setInterval(sendHeartbeat, 30000);

					return () => {
						clearInterval(dataInterval);
						clearInterval(heartbeatInterval);
					};
				},
			});

			return new Response(stream, {
				headers: {
					"Content-Type": "text/event-stream; charset=utf-8",
					"Cache-Control": "no-cache",
					"Connection": "keep-alive",
				},
			});
		}

		const response = generateResponse("success", "Netease Music Now Playing Status", data);
		return new Response(JSON.stringify(response), {
			status: 200,
			headers: {
				"Content-Type": "application/json",
			},
		});
	} else {
		const data = await fetch("https://api.codetime.dev/stats/latest", {
			headers: {
				Cookie: `CODETIME_SESSION=${process.env.CODETIME_SESSION}`,
			},
		});

		const jsonData = await data.json();

		if (sse) {
			const stream = new ReadableStream({
				async start(controller) {
					let lastData = null;
					const encoder = new TextEncoder();

					const sendData = async () => {
						const data = await fetch("https://api.codetime.dev/stats/latest", {
							headers: {
								Cookie: `CODETIME_SESSION=${process.env.CODETIME_SESSION}`,
							},
						});
						const jsonData = await data.json();
						const currentData = JSON.stringify(jsonData);
						if (!lastData || lastData !== currentData) {
							controller.enqueue(encoder.encode(`data: ${currentData}\n\n`));
							lastData = currentData;
						}
					};

					const sendHeartbeat = () => {
						controller.enqueue(encoder.encode(": heartbeat\n\n"));
					};

					await sendData();
					const dataInterval = setInterval(sendData, interval);
					const heartbeatInterval = setInterval(sendHeartbeat, 30000);

					return () => {
						clearInterval(dataInterval);
						clearInterval(heartbeatInterval);
					};
				},
			});

			return new Response(stream, {
				headers: {
					"Content-Type": "text/event-stream; charset=utf-8",
					"Cache-Control": "no-cache",
					"Connection": "keep-alive",
				},
			});
		}

		const response = generateResponse("success", "codetime", jsonData);
		return new Response(JSON.stringify(response), {
			status: 200,
			headers: {
				"Content-Type": "application/json",
			},
		});
	}
});

async function handleCache(userId: string, songId: string, currentTime: string): Promise<boolean> {
	const cachedData = await db_find("space-api", "ncm_status", { userId });
	let isInactive = false;

	if (cachedData) {
		const lastUpdate = new Date(cachedData.timestamp);
		const timeDiff = new Date().getTime() - lastUpdate.getTime();

		if (timeDiff > 5 * 60 * 1000 && cachedData.songId === songId) {
			isInactive = true;
		}

		if (cachedData.songId !== songId) {
			await db_update("space-api", "ncm_status", { userId }, { songId, timestamp: currentTime });
		}
	} else {
		await db_insert("space-api", "ncm_status", {
			userId,
			songId,
			timestamp: currentTime,
		});
	}

	return isInactive;
};
