import { db_find } from "@/utils/db";
import { sendVerificationCode } from "@/utils/email";
import { createVerificationCode } from "@/utils/verification";

export default eventHandler(async (event) => {
	try {
		const body = await readBody<{ email: string; method: "links" | "login" }>(event);

		if (!body.email) {
			const response: ApiResponse = {
				code: "400",
				status: "failed",
				message: "Email is required",
			};
			return new Response(JSON.stringify(response), {
				status: 400,
				headers: { "Content-Type": "application/json" },
			});
		}

		if (!body.method || (body.method !== "links" && body.method !== "login")) {
			const response: ApiResponse = {
				code: "400",
				status: "failed",
				message: "Method is required and must be either 'links' or 'login'",
			};
			return new Response(JSON.stringify(response), {
				status: 400,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 生成验证码
		const code = await createVerificationCode(body.email, body.method);
		if (!code) {
			// 检查是否是因为频率限制
			const existingCode = await db_find("space-api", "verification_codes", { email: body.email, method: body.method });
			if (existingCode) {
				const createdAt = new Date(existingCode.createdAt);
				const now = new Date();
				const diffSeconds = Math.floor((now.getTime() - createdAt.getTime()) / 1000);

				if (diffSeconds < 60) {
					const response: ApiResponse = {
						code: "429",
						status: "failed",
						message: `请求过于频繁，请在${60 - diffSeconds}秒后重试`,
					};
					return new Response(JSON.stringify(response), {
						status: 429,
						headers: { "Content-Type": "application/json" },
					});
				}
			}

			const response: ApiResponse = {
				code: "500",
				status: "failed",
				message: "Failed to generate verification code",
			};
			return new Response(JSON.stringify(response), {
				status: 500,
				headers: { "Content-Type": "application/json" },
			});
		}

		// 发送验证码邮件
		const sent = await sendVerificationCode(body.email, code, body.method);
		if (!sent) {
			const response: ApiResponse = {
				code: "500",
				status: "failed",
				message: "Failed to send verification code",
			};
			return new Response(JSON.stringify(response), {
				status: 500,
				headers: { "Content-Type": "application/json" },
			});
		}

		const response: ApiResponse = {
			code: "200",
			status: "success",
			message: "Verification code sent successfully",
		};

		return new Response(JSON.stringify(response), {
			status: 200,
			headers: { "Content-Type": "application/json" },
		});
	} catch (error) {
		const response: ApiResponse = {
			code: "500",
			status: "failed",
			message: `Internal server error: ${error instanceof Error ? error.message : String(error)}`,
		};
		return new Response(JSON.stringify(response), {
			status: 500,
			headers: { "Content-Type": "application/json" },
		});
	}
});
