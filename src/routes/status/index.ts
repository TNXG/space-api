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
		const data = await getNcmNowPlay(qNumber)
			.then((response) => {
				return {
					id: response.data.id,
					user: {
						id: response.data.userId,
						avatar: response.data.avatar,
						name: response.data.userName,
					},
					song: {
						name: response.data.song.name,
						id: response.data.song.id,
						artists: response.data.song.artists.map(artist => ({
							id: artist.id,
							name: artist.name,
						})),
						album: {
							name: response.data.song.album.name,
							id: response.data.song.album.id,
							image: response.data.song.album.picUrl,
							publishTime: new Date(response.data.song.album.publishTime).toISOString(),
							artists: response.data.song.album.artists.map(artist => ({
								id: artist.id,
								name: artist.name,
							})),
						},
					},
				};
			});
		const response: ApiResponse = {
			code: "200",
			status: "success",
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
