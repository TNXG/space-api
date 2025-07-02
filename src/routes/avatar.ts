import type { H3Event } from "h3";
import { Buffer } from "node:buffer";
import { formatAccept, handleImageRequest } from "@/utils/image-utils";

interface CachedImageData {
	body: {
		type: "Buffer";
		data: number[];
	};
	headers: {
		"Content-Type": string;
	};
}

export default eventHandler(async (event: H3Event) => {
	const query = getQuery(event);
	const acceptHeader = getRequestHeader(event, "Accept") || getRequestHeader(event, "accept");
	const source = query.s || query.source;

	// 生成缓存键
	const cacheKey = `avatar:${source}:${formatAccept(acceptHeader)}`;

	// 尝试从缓存获取
	const cached = await useStorage().getItem<CachedImageData>(cacheKey);
	if (cached) {
		return new Response(Buffer.from(cached.body.data), {
			headers: {
				...cached.headers,
				"Api-Cache": "HIT",
				"Cache-Control": "public, max-age=259200, s-maxage=172800",
			},
		});
	}

	let avatarUrl: string;
	if (source === "qq" || source === "QQ") {
		avatarUrl = "https://q1.qlogo.cn/g?b=qq&nk=2271225249&s=640";
	} else if (source === "github" || source === "GitHub" || source === "gh" || source === "GH") {
		avatarUrl = "https://avatars.githubusercontent.com/u/69001561";
	} else {
		avatarUrl = "https://cdn.tnxg.top/images/avatar/main/Texas.png";
	}

	try {
		const response = await fetch(avatarUrl);
		const blob = await response.blob();
		const result = await handleImageRequest(blob, acceptHeader);

		await useStorage().setItem(cacheKey, result, {
			ttl: 60 * 60 * 6,
		});

		return new Response(result.body, {
			headers: {
				...result.headers,
				"Api-Cache": "MISS",
				"Cache-Control": "public, max-age=259200, s-maxage=172800",
			},
		});
	} catch (error) {
		console.error("Error fetching avatar:", error);

		const errorResponse: ApiResponse = {
			code: "500",
			message: "Error fetching avatar",
			status: "failed",
		};

		return new Response(JSON.stringify(errorResponse), {
			status: 500,
			headers: { "Content-Type": "application/json" },
		});
	}
});
