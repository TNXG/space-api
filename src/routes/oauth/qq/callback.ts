import crypto from "node:crypto";
import process from "node:process";
import { createError, eventHandler, getQuery, sendRedirect } from "h3";
import { db_find, db_insert, db_update } from "@/utils/db";
import { completeQQOAuth } from "@/utils/qq-oauth";

export default eventHandler(async (event) => {
	const query = getQuery(event);
	const code = query.code as string;
	const stateParam = query.state as string;
	
	// 解析state参数
	let returnUrl = process.env.DEFAULT_RETURN_URL || "http://localhost:3000";
	let originalState: string | undefined;
	
	if (stateParam) {
		try {
			const parsedState = JSON.parse(stateParam);
			returnUrl = parsedState.return_url || returnUrl;
			originalState = parsedState.original_state;
		} catch {
			// 如果state不是JSON格式，当作普通state处理
			originalState = stateParam;
		}
	}

	if (!code) {
		throw createError({
			statusCode: 400,
			statusMessage: "Authorization code is required",
		});
	}

	try {
		// 完成QQ OAuth认证
		const oauthResult = await completeQQOAuth(code);

		// 生成一次性临时代码
		const tempCode = crypto.randomBytes(32).toString("hex");
		const expiresAt = new Date(Date.now() + 10 * 60 * 1000); // 10分钟过期

		// 检查用户是否已存在
		const existingUser = await db_find("space_api", "users", { qq_openid: oauthResult.openId });

		let userId: string;
		if (existingUser) {
			// 更新现有用户信息
			userId = existingUser._id.toString();
			await db_update("space_api", "users", { qq_openid: oauthResult.openId }, {
				nickname: oauthResult.userInfo.nickname,
				avatar: oauthResult.userInfo.figureurl_qq_2 || oauthResult.userInfo.figureurl_2,
				gender: oauthResult.userInfo.gender,
				updated_at: new Date(),
			});
		} else {
			// 创建新用户
			const newUser = {
				qq_openid: oauthResult.openId,
				nickname: oauthResult.userInfo.nickname,
				avatar: oauthResult.userInfo.figureurl_qq_2 || oauthResult.userInfo.figureurl_2,
				gender: oauthResult.userInfo.gender,
				created_at: new Date(),
				updated_at: new Date(),
			};
			const insertResult = await db_insert("space_api", "users", newUser);
			if (!insertResult) {
				throw new Error("Failed to save user information");
			}

			// 获取新创建的用户ID
			const savedUser = await db_find("space_api", "users", { qq_openid: oauthResult.openId });
			userId = savedUser?._id.toString() || "";
		}

		// 保存临时代码
		await db_insert("space_api", "temp_codes", {
			code: tempCode,
			user_id: userId,
			created_at: new Date(),
			expires_at: expiresAt,
			used: false,
		});

		// 构建重定向URL，将临时代码附加在参数中
		const redirectUrl = new URL(returnUrl);
		redirectUrl.searchParams.set("code", tempCode);
		if (originalState) {
			redirectUrl.searchParams.set("state", originalState);
		}

		// 重定向到原网站
		return sendRedirect(event, redirectUrl.toString());
	} catch (error) {
		console.error("QQ OAuth callback error:", error);

		// 重定向到错误页面或返回错误参数
		const errorUrl = new URL(returnUrl);
		errorUrl.searchParams.set("error", "oauth_failed");
		errorUrl.searchParams.set("error_description", error instanceof Error ? error.message : "Unknown error");

		return sendRedirect(event, errorUrl.toString());
	}
});
