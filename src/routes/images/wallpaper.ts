import { eventHandler } from "h3";
import blurhashData from "@/data/blurhash.json";
import { handleImageRequest } from "@/utils/image-utils";

export default eventHandler(async (event) => {
	const query = getQuery(event);
	const acceptHeader = getRequestHeader(event, "Accept");

	const type = query.type || query.t;

	// 在1到136之间随机选择一个数字
	const imageId = Math.floor(Math.random() * 200) + 1;
	const imageIdStr = imageId.toString();

	// 根据 type 值决定执行不同的操作
	switch (type) {
		case "cdn":
			// 创建一个带有 Location 头部的重定向响应
			return new Response(null, {
				status: 302,
				headers: {
					Location: `https://cdn.tnxg.top/images/wallpaper/${imageIdStr}.jpg`,
				},
			});

		case "json": {
			interface ResponseData {
				image: string;
				blurhash: string;
			}
			const blurhash = blurhashData.weight;
			const response: ApiResponse<ResponseData> = {
				code: "200",
				status: "success",
				data: {
					image: `https://cdn.tnxg.top/images/wallpaper/${imageIdStr}.jpg`,
					blurhash: blurhash[`${imageIdStr}.jpg`],
				},
			};
			return new Response(JSON.stringify(response), {
				status: 200,
				headers: {
					"Content-Type": "application/json",
				},
			});
		}

		default: {
			try {
				const imageResponse = await fetch(`https://cdn.tnxg.top/images/wallpaper/${imageIdStr}.jpg`);
				const blob = await imageResponse.blob();
				const { body, headers } = await handleImageRequest(blob, acceptHeader);
				return new Response(body, {
					headers,
				});
			} catch (error) {
				console.error("Error fetching avatar:", error);
				const errorResponse: ApiResponse = {
					code: "500",
					message: "Error fetching avatar",
					status: "error",
				};
				return new Response(JSON.stringify(errorResponse), {
					status: 500,
					headers: { "Content-Type": "application/json" },
				});
			}
		}
	}
});