import { db_find } from "@/utils/db";

export default eventHandler(async (event) => {
	const query = getQuery(event);
	const qqOpenId = query.qq_openid || query.openid || query.id;

	if (!qqOpenId) {
		throw createError({
			statusCode: 400,
			statusMessage: "id is required",
		});
	}

	try {
		const user = await db_find("space-api", "users", { qq_openid: qqOpenId });

		if (!user) {
			throw createError({
				statusCode: 404,
				statusMessage: "User not found",
			});
		}

		const response: ApiResponse<{
			qq_openid: string;
			nickname: string;
			avatar?: string;
			createdAt: string;
		}> = {
			code: "200",
			status: "success",
			data: {
				qq_openid: user.qq_openid,
				nickname: user.nickname,
				avatar: user.avatar,
				createdAt: user.created_at,
			},
		};

		return response;
	} catch (error) {
		const response: ApiResponse = {
			code: "500",
			status: "failed",
			message: error.message || "Internal Server Error",
		};
		return response;
	}
});
