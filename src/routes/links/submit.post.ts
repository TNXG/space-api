import { db_find, db_insert } from "@/utils/db";
import { verifyCode } from "@/utils/verification";

interface LinkSubmission {
	name: string;
	url: string;
	avatar: string;
	description: string;
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
				status: "failed",
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
				status: "failed",
				message: "Invalid verification code",
			};
			return new Response(JSON.stringify(response), {
				status: 401,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 规范化URL
		const normalizedUrl = body.url.replace(/\/$/, ""); // 移除末尾的斜杠

		// 检查是否包含子目录
		const urlParts = new URL(normalizedUrl).pathname.split("/");
		if (urlParts.length > 1 && urlParts[1] !== "") {
			const response: ApiResponse = {
				code: "400",
				status: "failed",
				message: "URL不能包含子目录",
			};
			return new Response(JSON.stringify(response), {
				status: 400,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 检查URL是否已存在
		const existingLink = await db_find("space-api", "links", { url: normalizedUrl });
		if (existingLink) {
			const response: ApiResponse = {
				code: "409",
				status: "failed",
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
			url: normalizedUrl,
			avatar: body.avatar,
			description: body.description,
			state: 1,
			created: body.created || new Date().toISOString(),
			rssurl: body.rssurl || "",
			techstack: body.techstack || [],
			email: body.email,
		};

		// 插入数据到space-api数据库
		const successSpaceApi = await db_insert("space-api", "links", linkData);
		if (!successSpaceApi) {
			const response: ApiResponse = {
				code: "500",
				status: "failed",
				message: "Failed to insert link to space-api database",
			};
			return new Response(JSON.stringify(response), {
				status: 500,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 准备mx-space数据库的数据格式
		const mxSpaceData = {
			name: body.name,
			url: normalizedUrl,
			avatar: body.avatar,
			description: body.description,
			email: body.email,
			type: 0,
			state: 1,
			created: new Date(),
		};

		// 插入数据到mx-space数据库
		const successMxSpace = await db_insert("mx-space", "links", mxSpaceData);
		if (!successMxSpace) {
			console.error("Failed to insert link to mx-space database");
			// 继续执行，不影响主流程
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
	} catch (error) {
		const response: ApiResponse = {
			code: "500",
			status: "failed",
			message: `Internal server error: ${error instanceof Error ? error.message : "Unknown error"}`,
		};
		return new Response(JSON.stringify(response), {
			status: 500,
			headers: { "Content-Type": "application/json" },
		});
	}
});
