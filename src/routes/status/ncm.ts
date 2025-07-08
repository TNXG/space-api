import { db_find, db_insert, db_update } from "@/utils/db";
import { getNcmNowPlay } from "~/utils/ncm-nowplay";

interface User {
	id: number;
	avatar: string;
	name: string;
	active: boolean;
}
interface Song {
	name: string;
	transNames: string[];
	alias: string[];
	id: number;
	artists: { id: number; name: string }[];
	album: {
		name: string;
		id: number;
		image: string;
		publishTime: string;
		artists: { id: number; name: string }[];
	};
}
interface NowPlayingData {
	id: number;
	user: User;
	song?: Song;
	lastUpdate: string;
}
const generateResponse = <T>(status: "success" | "failed", message: string, data: T | null, code: string = "200"): ApiResponse<T> => {
	return {
		code,
		status,
		message,
		data,
	};
};

export default eventHandler(async (event) => {
	const query = getQuery(event);
	const q = query.q || query.query || 515522946;
	const sse = query.sse === "true";
	const interval = Number(query.interval) || Number(query.i) || 5000;
	if (interval < 1000) {
		return new Response(
			JSON.stringify(
				generateResponse(
					"failed",
					"Invalid interval: must be at least 1000ms",
					null,
					"400",
				),
			),
			{
				status: 400,
				headers: { "Content-Type": "application/json" },
			},
		);
	}
	if (Number.isNaN(Number(q))) {
		return new Response(
			JSON.stringify(
				generateResponse(
					"failed",
					"Invalid q parameter: must be a number",
					null,
					"400",
				),
			),
			{
				status: 400,
				headers: { "Content-Type": "application/json; charset=utf-8" },
			},
		);
	}
	const qNumber = Number(q);
	const currentTime = new Date().toISOString();
	const nowPlayingData = await getNcmNowPlay(qNumber);
	// 检查 data 是否为 null
	if (!nowPlayingData.data) {
		return new Response(
			JSON.stringify(
				generateResponse("failed", "User not found", null, "404"),
			),
			{
				status: 404,
				headers: { "Content-Type": "application/json; charset=utf-8" },
			},
		);
	}
	const userId = nowPlayingData.data.userId;
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
		const song = nowPlayingData.data.song;
		data.song = {
			name: song.name,
			// 优先使用 song.transNames，如果不存在则尝试从 extProperties 获取
			transNames: song.transNames || song.extProperties?.transNames || [],
			alias: song.alias || [],
			id: song.id,
			artists: song.artists.map(artist => ({
				id: artist.id,
				name: artist.name,
			})),
			album: {
				name: song.album.name,
				id: song.album.id,
				image: song.album.picUrl,
				publishTime: new Date(song.album.publishTime).toISOString(),
				artists: song.album.artists.map(artist => ({
					id: artist.id,
					name: artist.name,
				})),
			},
		};
	}

	if (sse) {
		const stream = new ReadableStream({
			async start(controller) {
				let lastSongId: number | null = null;
				let lastActive: boolean | null = null;
				const encoder = new TextEncoder();

				const sendData = async () => {
					const currentNcmData = await getNcmNowPlay(qNumber);
					const currentUserId = currentNcmData.data.userId;
					const currentSongId = currentNcmData.data.song.id;
					const currentIsInactive = await handleCache(currentUserId, currentSongId, new Date().toISOString());

					// 只有当歌曲ID或活跃状态发生变化时才发送数据
					if (lastSongId !== currentSongId || lastActive !== !currentIsInactive) {
						const sseData: NowPlayingData = {
							id: currentNcmData.data.id,
							user: {
								id: currentUserId,
								avatar: currentNcmData.data.avatar,
								name: currentNcmData.data.userName,
								active: !currentIsInactive,
							},
							lastUpdate: new Date().toISOString(),
						};

						if (!currentIsInactive) {
							sseData.song = {
								name: currentNcmData.data.song.name,
								transNames: currentNcmData.data.song.extProperties?.transNames || [],
								alias: currentNcmData.data.song.alias || [],
								id: currentNcmData.data.song.id,
								artists: currentNcmData.data.song.artists.map((artist: { id: any; name: any }) => ({
									id: artist.id,
									name: artist.name,
								})),
								album: {
									name: currentNcmData.data.song.album.name,
									id: currentNcmData.data.song.album.id,
									image: currentNcmData.data.song.album.picUrl,
									publishTime: new Date(currentNcmData.data.song.album.publishTime).toISOString(),
									artists: currentNcmData.data.song.album.artists.map(artist => ({
										id: artist.id,
										name: artist.name,
									})),
								},
							};
						}

						controller.enqueue(encoder.encode(`data: ${JSON.stringify(sseData)}\n\n`));

						// 更新上一次的状态
						lastSongId = currentSongId;
						lastActive = !currentIsInactive;
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
	} else {
		const response = generateResponse("success", "Netease Music Now Playing Status", data);
		return new Response(JSON.stringify(response), {
			status: 200,
			headers: { "Content-Type": "application/json" },
		});
	}
});

async function handleCache(userId: number, songId: number, currentTime: string): Promise<boolean> {
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
}
