import { db_find, db_insert } from "@/utils/db";
import { verifyCode } from "@/utils/verification";

interface LinkSubmission {
	name: string;
	url: string;
	avatar: string;
	description: string;
	state: number;
	created: string;
	rssurl: string;
	techstack: string[];
	email: string;
	code: string;
}

export default eventHandler(async (event) => {
	try {
		const body = await readBody<LinkSubmission>(event);

		// 验证必填字段
		if (!body.name || !body.url || !body.avatar || !body.description || !body.email || !body.code) {
			const response: ApiResponse = {
				code: "400",
				status: "error",
				message: "Missing required fields",
			};
			return new Response(JSON.stringify(response), {
				status: 400,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 验证邮箱验证码
		const isCodeValid = await verifyCode(body.email, body.code);
		if (!isCodeValid) {
			const response: ApiResponse = {
				code: "401",
				status: "error",
				message: "Invalid verification code",
			};
			return new Response(JSON.stringify(response), {
				status: 401,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 检查URL是否已存在
		const existingLink = await db_find("space-api", "links", { url: body.url });
		if (existingLink) {
			const response: ApiResponse = {
				code: "409",
				status: "error",
				message: "URL already exists",
			};
			return new Response(JSON.stringify(response), {
				status: 409,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 准备要插入的数据
		const linkData = {
			name: body.name,
			url: body.url,
			avatar: body.avatar,
			description: body.description,
			state: body.state || 0,
			created: body.created || new Date().toISOString(),
			rssurl: body.rssurl || "",
			techstack: body.techstack || [],
			email: body.email,
		};

		// 插入数据
		const success = await db_insert("space-api", "links", linkData);
		if (!success) {
			const response: ApiResponse = {
				code: "500",
				status: "error",
				message: "Failed to insert link",
			};
			return new Response(JSON.stringify(response), {
				status: 500,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 从返回数据中移除email字段
		const { email, ...responseData } = linkData;
		const response: ApiResponse = {
			code: "200",
			status: "success",
			message: "Link submitted successfully",
			data: responseData,
		};

		return new Response(JSON.stringify(response), {
			status: 200,
			headers: { "Content-Type": "application/json" },
		});
	}
	catch (error) {
		const response: ApiResponse = {
			code: "500",
			status: "error",
			message: `Internal server error: ${error instanceof Error ? error.message : "Unknown error"}`,
		};
		return new Response(JSON.stringify(response), {
			status: 500,
			headers: { "Content-Type": "application/json" },
		});
	}
});
