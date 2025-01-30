import { db_find, db_insert, db_update } from "@/utils/db";
import { getNcmNowPlay } from "@/utils/ncm-nowplay";

const RuntimeConfig = useRuntimeConfig();

export default eventHandler(async (event) => {
	const query = getQuery(event);

	const s = query.s || query.source || "codetime";
	const q = query.q || query.query || 515522946;

	// 校验 s 是否为字符串
	if (typeof s !== "string") {
		const response: ApiResponse = {
			code: "400",
			status: "error",
			message: "Invalid s parameter: must be a string",
		};

		return new Response(JSON.stringify(response), {
			status: 400,
			headers: {
				"Content-Type": "application/json",
			},
		});
	}

	// 校验 q 是否为数字
	if (Number.isNaN(Number(q))) {
		const response: ApiResponse = {
			code: "400",
			status: "error",
			message: "Invalid q parameter: must be a number",
		};

		return new Response(JSON.stringify(response), {
			status: 400,
			headers: {
				"Content-Type": "application/json",
			},
		});
	}

	const qNumber = Number(q); // 将 q 转换为数字
	const sString = String(s); // 将 s 转换为字符串

	if (sString === "ncm" || sString === "n" || sString === "netease") {
		const nowPlayingData = await getNcmNowPlay(qNumber);
		const userId = nowPlayingData.data.userId;
		const songId = nowPlayingData.data.song.id;
		const currentTime = new Date().toISOString();

		// 检查缓存
		const cachedData = await db_find("space-api", "ncm_status", { userId });
		let isInactive = false;

		if (cachedData) {
			const lastUpdate = new Date(cachedData.timestamp);
			const timeDiff = new Date().getTime() - lastUpdate.getTime();

			// 如果超过5分钟还是同一首歌，标记为不活跃
			if (timeDiff > 5 * 60 * 1000 && cachedData.songId === songId) {
				isInactive = true;
			}
			else {
				await db_update("space-api", "ncm_status", { userId }, { songId, timestamp: currentTime });
			}
		}
		else {
			// 新用户，创建缓存
			await db_insert("space-api", "ncm_status", {
				userId,
				songId,
				timestamp: currentTime,
			});
		}

		let data;
		if (isInactive) {
			// 如果不活跃，只返回 id 和 user
			data = {
				id: nowPlayingData.data.id,
				user: {
					id: nowPlayingData.data.userId,
					avatar: nowPlayingData.data.avatar,
					name: nowPlayingData.data.userName,
					active: !isInactive,
				},
			};
		}
		else {
			// 如果活跃，返回完整数据
			data = {
				id: nowPlayingData.data.id,
				user: {
					id: nowPlayingData.data.userId,
					avatar: nowPlayingData.data.avatar,
					name: nowPlayingData.data.userName,
					active: !isInactive,
				},
				song: {
					name: nowPlayingData.data.song.name,
					transNames: nowPlayingData.data.song.extProperties?.transNames || [],
					alias: nowPlayingData.data.song.alias || [],
					id: nowPlayingData.data.song.id,
					artists: nowPlayingData.data.song.artists.map(artist => ({
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
				},
				lastUpdate: currentTime,
			};
		}

		const response: ApiResponse = {
			code: "200",
			status: "success",
			message: "Netease Music Now Playing Status",
			data,
		};

		return new Response(JSON.stringify(response), {
			status: 200,
			headers: {
				"Content-Type": "application/json",
			},
		});
	}
	else {
		const data = await fetch("https://api.codetime.dev/stats/latest", {
			headers: {
				Cookie: `CODETIME_SESSION=${RuntimeConfig.CODETIME_SESSION}`,
			},
		});

		const response: ApiResponse = {
			code: "200",
			status: "success",
			message: "codetime",
			data: await data.json(),
		};

		return new Response(JSON.stringify(response), {
			status: 200,
			headers: {
				"Content-Type": "application/json",
			},
		});
	}
});
