import type { H3Event } from "h3";
import { handleImageRequest } from "@/utils/image-utils";

export default eventHandler(async (event: H3Event) => {
	const query = getQuery(event);
	const acceptHeader = getRequestHeader(event, "Accept") || getRequestHeader(event, "accept");

	let avatarUrl: string;

	const source = query.s || query.source;

	if (source === "qq" || source === "QQ") {
		avatarUrl = "https://q1.qlogo.cn/g?b=qq&nk=2271225249&s=640";
	}
	else if (source === "github" || source === "GitHub" || source === "gh" || source === "GH") {
		avatarUrl = "https://avatars.githubusercontent.com/u/69001561";
	}
	else {
		avatarUrl = "https://cdn.tnxg.top/images/avatar/main/Texas.png";
	}

	try {
		const response = await fetch(avatarUrl);

		const blob = await response.blob();

		const { body, headers } = await handleImageRequest(blob, acceptHeader);

		return new Response(body, {
			headers,
		});
	}
	catch (error) {
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
});
