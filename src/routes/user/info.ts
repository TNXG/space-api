import dotenv from "dotenv";
import { createError, eventHandler, getQuery } from "h3";
import { db_delete, db_find } from "@/utils/db";

dotenv.config();

export default eventHandler(async (event) => {
	const query = getQuery(event);
	const code = query.code as string;

	if (!code) {
		throw createError({
			statusCode: 400,
			statusMessage: "Temporary code is required",
		});
	}

	try {
		// 查找临时代码
		const tempCodeRecord = await db_find("space-api", "temp_codes", { code, used: false });

		if (!tempCodeRecord) {
			throw createError({
				statusCode: 404,
				statusMessage: "Invalid or expired temporary code",
			});
		}

		// 检查是否过期
		if (new Date() > new Date(tempCodeRecord.expires_at)) {
			throw createError({
				statusCode: 410,
				statusMessage: "Temporary code has expired",
			});
		}

		// 获取用户信息
		const user = await db_find("space-api", "users", { qq_openid: tempCodeRecord.qq_openid });

		if (!user) {
			throw createError({
				statusCode: 404,
				statusMessage: "User not found",
			});
		}

		// 移除临时代码
		await db_delete("space-api", "temp_codes", { _id: tempCodeRecord._id });

		// 返回用户信息（不包含敏感信息）
		const response: ApiResponse<{
			user_id: string;
			qq_openid: string;
			nickname: string;
			avatar?: string;
			gender?: string;
			created_at: Date;
			updated_at: Date;
		}> = {
			code: "200",
			message: "User information retrieved successfully",
			status: "success",
			data: {
				user_id: user._id.toString(),
				qq_openid: user.qq_openid,
				nickname: user.nickname,
				avatar: user.avatar,
				gender: user.gender,
				created_at: user.created_at,
				updated_at: user.updated_at,
			},
		};

		return response;
	}
 catch (error) {
		// 如果是已知错误，直接抛出
		if (error && typeof error === "object" && "statusCode" in error) {
			throw error;
		}

		// 其他错误
		throw createError({
			statusCode: 500,
			statusMessage: `Internal server error: ${error instanceof Error ? error.message : "Unknown error"}`,
		});
	}
});
